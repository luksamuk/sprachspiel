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
#[derive(Debug, Clone, PartialEq)]
pub enum CompactionOutcome {
    /// The context buffer had room; nothing was compacted.
    NotNeeded,
    /// The summary was produced and applied to the in-memory session.
    /// The SQLite commit for a token-owned turn is deferred to the turn's
    /// terminal commit; token-less callers commit right here. Carries the
    /// yield the session governor consumes.
    Completed(CompactionYield),
    /// Every layer failed with a model/provider error; the session is
    /// untouched and the turn continues the way it always did.
    Failed,
    /// Ctrl+C reached a summarizing pass. Nothing was applied, no further
    /// LLM call was made, and the caller aborts the surrounding work.
    Cancelled,
    /// The session governor has auto-compaction paused. Nothing was
    /// applied and no model call was made; the caller decides whether the
    /// turn proceeds (below the overflow limit) or stops (at it).
    Paused,
}

impl CompactionOutcome {
    /// Whether compaction work was stopped by cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, CompactionOutcome::Cancelled)
    }
}

/// Homogeneous before/after measurement of one compaction pass.
///
/// Both sides use the SAME pure estimator (summary + active messages +
/// overhead), never the provider-reported prompt count on one side and an
/// estimate on the other — switching rulers between the two measurements
/// fabricates the yield (the apply itself rewrites/erases the stored
/// token counts). The fraction is relative to the context window, not to
/// the pre-compaction size, and may be negative when a pass freed nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionYield {
    /// Pure estimate before the summary was applied.
    pub tokens_before: usize,
    /// Pure estimate after the summary was applied.
    pub tokens_after: usize,
    /// `tokens_before - tokens_after`; negative when nothing was freed.
    pub freed_tokens: i64,
    /// `freed_tokens / context_window`, the governor's yield measure.
    pub yield_fraction: f32,
}

/// How many compaction passes a session may land below the yield bar
/// before auto-compaction pauses for that session.
pub const MAX_CONSECUTIVE_LOW_YIELD: usize = 2;

/// A compaction is low-yield below this fraction of the context window.
/// The bar itself does not count: exactly 10% of the window is high-yield.
pub const LOW_YIELD_FRACTION: f32 = 0.10;

/// Session-scoped auto-compaction pause state.
///
/// In-memory only — never persisted: a fresh process resumes with
/// auto-compaction enabled regardless of how the previous run ended.
/// A pause is a session-level circuit breaker against wasted compaction
/// work (passes that repeatedly free almost nothing in exchange for two
/// summarizer calls), not a per-turn decision; it engages for the session
/// and only a real recovery lifts it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutoCompactionGovernor {
    /// Consecutive finished compactions below the yield bar.
    consecutive_low_yield: usize,
    /// Whether auto-compaction is paused for this session.
    paused: bool,
}

impl AutoCompactionGovernor {
    /// Whether auto-compaction is paused for this session.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Feed one finished compaction's yield to the governor.
    ///
    /// Returns `true` exactly once, on the pass that crosses the pause
    /// threshold — the caller owes the user the single transition warning
    /// naming both yields. High-yield passes (at or above the bar) reset
    /// the consecutive count; zero and negative yields count as low-yield
    /// passes in their own right.
    ///
    /// Anything that did not finish cleanly (model failure, cancellation,
    /// "not needed") never reaches this method, so it can neither count
    /// toward the pause nor reset it.
    pub fn record_yield(&mut self, yield_fraction: f32) -> bool {
        if self.paused {
            return false;
        }
        if yield_fraction >= LOW_YIELD_FRACTION {
            self.consecutive_low_yield = 0;
            return false;
        }
        self.consecutive_low_yield += 1;
        if self.consecutive_low_yield >= MAX_CONSECUTIVE_LOW_YIELD {
            self.paused = true;
            self.consecutive_low_yield = 0;
            return true;
        }
        false
    }

    /// Lift the pause (a manual compaction ran to completion).
    ///
    /// The consecutive counter restarts from zero: the pause decision was
    /// about accumulating wasted work, and a deliberately-run, observed
    /// compaction is new evidence, not part of the old streak.
    pub fn resume(&mut self) {
        self.paused = false;
        self.consecutive_low_yield = 0;
    }

    /// Consecutive finished passes below the yield bar (inspection only —
    /// the pause decision itself lives in `record_yield`).
    pub fn low_yield_streak(&self) -> usize {
        self.consecutive_low_yield
    }
}

/// Single line telling the user auto-compaction is paused and what lifts it.
pub const AUTO_COMPACTION_PAUSED_MESSAGE: &str =
    "Auto-compaction paused; run `/compact` or start a new session.";

/// Whether a paused session sits at the context limit where sending a
/// request would overflow the window. The fail-closed rule: below this,
/// turn proceeds without auto-compaction; at it, no request leaves.
pub fn paused_at_window_limit(session: &ChatSession, context_window: usize) -> bool {
    let (_, _, _, emergency) = crate::context_overflow::calculate_thresholds(context_window);
    let emergency = emergency.min(context_window); // tiny synthetic windows
    session.history_real_tokens() >= context_window.saturating_sub(emergency)
}

impl std::fmt::Display for CompactionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompactionFailure::Model(message) => write!(f, "{message}"),
            CompactionFailure::Cancelled => write!(f, "cancelled by user"),
        }
    }
}

impl CompactionFailure {
    /// Whether this failure is a cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, CompactionFailure::Cancelled)
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

        // A paused session makes no auto-compaction call and no automatic
        // recovery move; what the caller does with the Paused outcome — push
        // through below the overflow limit or stop the turn at it — is the
        // caller's decision, not a silent mode here.
        if self.session.governor.is_paused() {
            log::debug!("Auto-compaction paused for this session; skipping pass");
            return CompactionOutcome::Paused;
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

        // Homogeneous ruler: the same pure estimator measures the session
        // before and after the summary lands, so the yield is computed with
        // one ruler instead of mixing a provider-reported count with an
        // estimate (the apply invalidates stored token counts anyway).
        let tokens_before = self.session.estimated_context_tokens();

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

                let tokens_after = self.session.estimated_context_tokens();
                let freed_tokens = tokens_before as i64 - tokens_after as i64;
                let yield_fraction =
                    freed_tokens as f32 / self.context_window.max(1) as f32;

                // Get compacted count
                let (first_preserved, last_preserved_start) =
                    range.unwrap_or((0, self.session.messages.len()));
                let compacted_count = last_preserved_start - first_preserved;
                let preserved_last = self.session.messages.len() - last_preserved_start;

                self.view
                    .show_compact_complete(compacted_count, first_preserved, preserved_last);

                if freed_tokens <= 0 {
                    log::warn!(
                        "[Compaction] Negative yield: {} -> {} tokens (freed {}); \
                         nothing was freed for two summarizer calls",
                        tokens_before,
                        tokens_after,
                        freed_tokens
                    );
                }
                log::debug!(
                    "[Compaction] Yield: {}K -> {}K tokens ({:+}K freed, {:.1}% of window, \
                     low-yield streak {})",
                    tokens_before / 1000,
                    tokens_after / 1000,
                    freed_tokens / 1000,
                    yield_fraction * 100.0,
                    self.session.governor.low_yield_streak()
                );

                // One warning, exactly on the pass that trips the pause, and
                // it names both sides of the measurement so the user can
                // judge the wasted work.
                if self
                    .session
                    .governor
                    .record_yield(yield_fraction)
                {
                    let message = format!(
                        "Auto-compaction paused: freed only {:+}K tokens across \
                         consecutive passes ({}K -> {}K on this one). {}",
                        freed_tokens / 1000,
                        tokens_before / 1000,
                        tokens_after / 1000,
                        AUTO_COMPACTION_PAUSED_MESSAGE
                    );
                    self.view.show_warning(&message);
                    log::warn!(
                        "Auto-compaction paused: {}K -> {}K tokens ({:+}K freed, \
                         {:.1}% of window)",
                        tokens_before / 1000,
                        tokens_after / 1000,
                        freed_tokens / 1000,
                        yield_fraction * 100.0
                    );
                    self.view.set_compaction_paused(true);
                }

                if self.cancel_token.is_none() && !self.session.anonymous {
                    let _ = self.session.save_sqlite();

                    // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                    if let Some(db) = self.session.db.as_ref() {
                        let _ = db.clear_conversation_prompt_tokens(&self.session.id);
                    }
                }

                CompactionOutcome::Completed(CompactionYield {
                    tokens_before,
                    tokens_after,
                    freed_tokens,
                    yield_fraction,
                })
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
