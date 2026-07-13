//! Compaction context for reducing context window overflow
//!
//! Provides `CompactionContext`, a struct that bundles the parameters needed
//! for auto-compaction. This replaces the 8-argument `auto_compact_if_needed`
//! function with a self-documenting struct, making call sites cleaner and
//! easier to extend.

use crate::provider::OpenAICompatibleProvider;

use crate::config::ModelConfig;
use crate::context_overflow::needs_buffered_compaction;
use crate::settings::Settings;

use super::core::compact_conversation;
use super::llm_event::LlmEvent;
use super::session::ChatSession;
use super::view::ChatView;

/// Bundles the parameters needed for auto-compaction.
///
/// Instead of passing 8 separate arguments to `auto_compact_if_needed`,
/// callers construct a `CompactionContext` with named fields. This makes
/// the intent self-documenting and allows adding new parameters without
/// changing every call site.
///
/// # Example
///
/// ```ignore
/// let mut ctx = CompactionContext {
///     ollama: &state.ollama,
///     model_config: &state.model_config,
///     session: &mut state.session,
///     settings: &state.settings,
///     agents_md: state.agents_md.as_deref(),
///     context_window,
///     view,
///     llm_tx,
/// };
/// ctx.compact_if_needed().await;
/// ```
pub struct CompactionContext<'a> {
    pub ollama: &'a OpenAICompatibleProvider,
    pub model_config: &'a ModelConfig,
    pub session: &'a mut ChatSession,
    pub settings: &'a Settings,
    pub agents_md: Option<&'a str>,
    pub context_window: usize,
    pub view: &'a mut dyn ChatView,
    pub llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
}

impl CompactionContext<'_> {
    /// Auto-compact the conversation if the context window is nearly full.
    ///
    /// Uses a buffer-based approach (15K tokens remaining) for predictable
    /// overflow prevention. All output rendering is delegated to the
    /// provided `ChatView`.
    pub async fn compact_if_needed(&mut self) {
        // Use buffer-based compaction trigger (more predictable than percentages)
        // Compacts when there are only COMPACTION_BUFFER tokens remaining
        if !needs_buffered_compaction(self.session, self.context_window) {
            return;
        }

        // Calculate usage percentage for display purposes
        let real_tokens = self.session.history_real_tokens();
        let usage_percent =
            ((real_tokens as f32 / self.context_window as f32) * 100.0).min(100.0) as u8;

        // Show indicator before starting compaction
        self.view.show_compact_progress(&format!(
            "Compacting context ({}% full, {}K remaining)...",
            usage_percent,
            (self.context_window.saturating_sub(real_tokens)) / 1000
        ));

        // Attempt auto-compaction with streaming
        match compact_conversation(
            self.ollama,
            self.model_config,
            self.session,
            self.settings,
            self.agents_md,
            self.llm_tx.clone(),
        )
        .await
        {
            Ok((summary, range)) => {
                self.session
                    .set_compacted_summary_with_range(summary.clone(), range);

                // Get compacted count
                let (first_preserved, last_preserved_start) =
                    range.unwrap_or((0, self.session.messages.len()));
                let compacted_count = last_preserved_start - first_preserved;
                let preserved_last = self.session.messages.len() - last_preserved_start;

                self.view
                    .show_compact_complete(compacted_count, first_preserved, preserved_last);

                if !self.session.anonymous {
                    let _ = self.session.save_sqlite();

                    // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                    if let Some(db) = self.session.db.as_ref() {
                        let _ = db.clear_conversation_prompt_tokens(&self.session.id);
                    }
                }
            }
            Err(e) => {
                self.view
                    .show_error(&format!("Auto-compaction failed: {}", e));
            }
        }
    }
}
