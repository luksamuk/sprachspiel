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
use tokio_util::sync::CancellationToken;

use super::core::compact_conversation;
use super::llm_event::LlmEvent;
use super::session::ChatSession;
use super::view::ChatView;

/// Why a compaction pass failed to produce a summary.
///
/// Cancelled must be matched before anything else is retried: an aborted
/// compaction must not fall through to the next layer, chunk or fallback —
/// reclassifying it as a model error would fire more LLM calls after the
/// user asked for everything to stop.
#[derive(Debug)]
pub enum CompactionFailure {
    /// The provider rejected or failed the request after every layer;
    /// carries the layer diagnostics. Falls through to more defensive
    /// layers the same way the old string-based result did.
    Model(String),
    /// Ctrl+C observed while summarizing: stop summarizing, do not start
    /// another pass, and let the caller abort the turn or the operation.
    Cancelled,
}

/// Terminal state of an auto-compaction attempt within a turn.
///
/// Typed from end to end: layers propagate [`CompactionFailure`], the
/// context resolves it, and callers match on this enum — no error string
/// is ever recognized afterwards to decide what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionOutcome {
    /// The context buffer had room; nothing was compacted.
    NotNeeded,
    /// The summary was produced and applied to the in-memory session.
    /// The SQLite commit for a token-owned turn is deferred to the turn's
    /// terminal commit; token-less callers commit right here.
    Completed,
    /// Every layer failed with a model/provider error; the session is
    /// untouched and the turn continues the way it always did.
    Failed,
    /// Ctrl+C reached a summarizing pass. Nothing was applied, no further
    /// LLM call was made, and the caller aborts the surrounding work.
    Cancelled,
}

impl CompactionFailure {
    /// Whether this failure is a cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, CompactionFailure::Cancelled)
    }
}

impl CompactionOutcome {
    /// Whether compaction work was stopped by cancellation.
    pub fn is_cancelled(self) -> bool {
        matches!(self, CompactionOutcome::Cancelled)
    }
}

impl std::fmt::Display for CompactionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompactionFailure::Model(message) => write!(f, "{message}"),
            CompactionFailure::Cancelled => write!(f, "cancelled by user"),
        }
    }
}

impl std::error::Error for CompactionFailure {}

/// Bundles the parameters needed for auto-compaction.
///
/// Instead of passing 8 separate arguments to `auto_compact_if_needed`,
/// callers construct a `CompactionContext` with named fields. This makes
/// the intent self-documenting and allows adding new parameters without
/// changing every call site.
///
/// `cancel_token` links the compaction to the turn's Ctrl+C token: turns
/// that own one pass `Some` so cancellation stops summarization and the
/// summary commit is deferred to the turn's terminal commit; token-less
/// callers (non-TUI paths) pass `None` and keep the old immediate-commit
/// behavior.
///
/// # Example
///
/// ```ignore
/// let mut ctx = CompactionContext {
///     provider: &state.provider,
///     model_config: &state.model_config,
///     session: &mut state.session,
///     settings: &state.settings,
///     agents_md: state.agents_md.as_deref(),
///     context_window,
///     view,
///     llm_tx,
///     cancel_token: None,
/// };
/// let outcome = ctx.compact_if_needed().await;
/// ```
pub struct CompactionContext<'a> {
    pub provider: &'a OpenAICompatibleProvider,
    pub model_config: &'a ModelConfig,
    pub session: &'a mut ChatSession,
    pub settings: &'a Settings,
    pub agents_md: Option<&'a str>,
    pub context_window: usize,
    pub view: &'a mut dyn ChatView,
    pub llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
    pub cancel_token: Option<&'a CancellationToken>,
}

impl CompactionContext<'_> {
    /// Auto-compact the conversation if the context window is nearly full.
    ///
    /// Uses a buffer-based approach (15K tokens remaining) for predictable
    /// overflow prevention. All output rendering is delegated to the
    /// provided `ChatView`.
    ///
    /// With a turn token, the summary lives in the in-memory session only —
    /// the SQLite save plus the prompt-token reset are the token-owner's
    /// terminal commit, so a cancelled turn discards the whole compaction
    /// as one unit. Without a token, a successful compaction commits here
    /// exactly as before.
    pub async fn compact_if_needed(&mut self) -> CompactionOutcome {
        // Use buffer-based compaction trigger (more predictable than percentages)
        // Compacts when there are only COMPACTION_BUFFER tokens remaining
        if !needs_buffered_compaction(self.session, self.context_window) {
            return CompactionOutcome::NotNeeded;
        }

        // Cancelled before anything started: no progress display, no LLM call.
        if self.cancel_token.is_some_and(CancellationToken::is_cancelled) {
            return CompactionOutcome::Cancelled;
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
            self.provider,
            self.model_config,
            self.session,
            self.settings,
            self.agents_md,
            self.llm_tx.clone(),
            self.cancel_token.cloned(),
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

                if self.cancel_token.is_none() && !self.session.anonymous {
                    let _ = self.session.save_sqlite();

                    // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                    if let Some(db) = self.session.db.as_ref() {
                        let _ = db.clear_conversation_prompt_tokens(&self.session.id);
                    }
                }

                CompactionOutcome::Completed
            }
            Err(CompactionFailure::Cancelled) => {
                // Nothing was applied to the session — the summary never
                // reached the state, so there is no partial commit to undo.
                CompactionOutcome::Cancelled
            }
            Err(CompactionFailure::Model(e)) => {
                self.view
                    .show_error(&format!("Auto-compaction failed: {}", e));
                CompactionOutcome::Failed
            }
        }
    }
}
