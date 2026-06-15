//! Context overflow detection and 3-layer compaction handling
//!
//! Implements progressive compaction when context reaches threshold:
//! - Layer 1: Pre-pruning (strip long tool outputs before summarization)
//! - Layer 2: Chunked recursive summarization (split into chunks if too large)
//! - Layer 3: Fallback truncation (hard truncate as last resort)
//!
//! Uses percentage-based thresholds that scale with context window size,
//! with absolute minimum buffers for small contexts.
//!
//! # Percentage-Based Thresholds
//!
//! Research shows LLMs degrade significantly above 75-88% context usage
//! (LongICLBench study). Our thresholds adapt to context size:
//!
//! | Threshold | Percentage | 32K Context | 128K Context | Trigger |
//! |-----------|------------|-------------|---------------|---------|
//! | PRE_TOOL | 75% used | 8K remaining | 32K remaining | Warning |
//! | COMPACTION | 88% used | 4K remaining | 15K remaining | Auto-compact |
//! | INTER_TOOL | 94% used | 2K remaining | 8K remaining | Warning during tools |
//! | EMERGENCY | 97% used | 1K remaining | 4K remaining | Truncate |
//!
//! # Absolute Minimum Buffers
//!
//! For small contexts (< 8K), we use absolute minimums to ensure safety:
//! - PRE_TOOL_MIN: 2K tokens
//! - COMPACTION_MIN: 1K tokens
//! - INTER_TOOL_MIN: 512 tokens
//! - EMERGENCY_MIN: 256 tokens

use crate::chat::session::{ChatSession, MessageRole, SavedMessage};
use crate::tokens::{MESSAGE_OVERHEAD, estimate_tokens};
use ollama_rs::generation::chat::ChatMessage;

/// Percentage thresholds (as fractions of context window)
/// Based on LongICLBench research showing LLM degradation patterns.
pub const MODERATE_USAGE_PERCENT: f32 = 0.75; // 75% - Warning threshold
pub const CRITICAL_USAGE_PERCENT: f32 = 0.88; // 88% - Compaction threshold
pub const INTER_TOOL_USAGE_PERCENT: f32 = 0.94; // 94% - Warning during tools
pub const EMERGENCY_USAGE_PERCENT: f32 = 0.97; // 97% - Emergency truncation

/// Absolute minimum buffers (for small contexts)
/// These ensure safety even when percentage-based calculations are too small.
pub const PRE_TOOL_MIN: usize = 2_000;
pub const COMPACTION_MIN: usize = 1_000;
pub const INTER_TOOL_MIN: usize = 512;
pub const EMERGENCY_MIN: usize = 256;

/// Response margin (tokens reserved for model response)
/// Increased from 500 to 2000 based on typical response lengths.
pub const RESPONSE_MARGIN: usize = 2_000;

/// Default number of first messages to keep during compaction
pub const DEFAULT_KEEP_FIRST: usize = 5;

/// Default number of last messages to keep during compaction
pub const DEFAULT_KEEP_LAST: usize = 5;

/// Default overflow threshold (75%) - used for display and tests
/// Shows "OK" below 75%, "MODERATE" 75-88%, "CRITICAL" above 88%
#[allow(dead_code)]
pub const DEFAULT_OVERFLOW_THRESHOLD: f32 = MODERATE_USAGE_PERCENT;

// ── 3-Layer Compaction Constants ───────────────────────────────────
//
// Layer 1: Pre-pruning — strip long tool outputs before summarization
// Layer 2: Chunked recursive summarization — split oversized middle sections
// Layer 3: Fallback truncation — hard truncate as last resort

/// Ratio of context window used as maximum size for a single compaction prompt.
/// At 60%, we leave 40% for system prompt + compaction instructions + model response.
pub const COMPACTION_MAX_CONTEXT_RATIO: f32 = 0.60;

/// Maximum recursion depth for chunked recursive summarization.
/// Prevents infinite loops if summaries keep exceeding the window.
pub const MAX_RECURSION_DEPTH: usize = 3;

/// Minimum number of ESTIMATED tokens a tool result must have before
/// pre-pruning will truncate it. Shorter tool results are kept as-is.
///
/// W2 #121 follow-up: this used to be `PRUNE_TOOL_RESULT_THRESHOLD = 500`
/// (chars), but `chars != tokens`. JSON-structured tool results
/// (file reads, shell output) have ~2-3x higher token density per char
/// than prose. The threshold is now expressed in estimated tokens
/// (using `estimate_tokens`, see src/tokens.rs), so the trigger is
/// consistent regardless of content type.
///
/// Note: `estimate_tokens` is approximate (30-50% undercount vs real
/// tokenizers — see the W2 #121 TODO in src/tokens.rs). The 200-token
/// threshold is conservative: at the worst case (50% undercount) this
/// triggers at ~400 real tokens, still well below any single tool result
/// that's likely to cause compaction problems.
pub const PRUNE_TOOL_RESULT_THRESHOLD_TOKENS: usize = 200;

/// Number of ESTIMATED tokens to keep from the beginning of a truncated
/// tool result. The rest is replaced with a truncation notice.
///
/// W2 #121 follow-up: was `PRUNE_TOOL_RESULT_KEEP_CHARS = 100` (chars).
/// Now 40 estimated tokens — enough for the start of a file's content
/// or the first lines of shell output, which carry the most signal for
/// summarization.
pub const PRUNE_TOOL_RESULT_KEEP_TOKENS: usize = 40;

/// Ratio of context window targeted after fallback truncation.
/// Targets 50% of the window so there's plenty of room for the response.
pub const TRUNCATION_TARGET_RATIO: f32 = 0.50;

/// Overhead per message during compaction.
///
/// Compaction uses `build_conversation_text()` which formats messages as
/// `"User: {content}\n"`, `"Assistant: {content}\n"`, etc. The real overhead
/// per message includes: role tags, newlines, JSON formatting from Ollama's
/// chat API, and special tokens. We use 10 tokens per message (vs.
/// `MESSAGE_OVERHEAD = 4` used elsewhere) to account for the additional
/// formatting in compaction prompts.
pub const COMPACT_MSG_OVERHEAD: usize = 10;

/// Token overhead for compaction prompts.
///
/// Includes:
///
/// - System message in `compact_with_llm()` (~50 tokens)
/// - `SYSTEM_PROMPT_SUMMARIZE` in user message (~40 tokens)
/// - `COMPACTION_PROMPT` instructions (~120 tokens)
/// - `"Conversation:"` label + formatting (~30 tokens)
/// - Response allowance (~2000 tokens)
/// - Safety buffer for tokenization variance (~760 tokens)
///
/// Total: ~3000
pub const COMPACTION_PROMPT_OVERHEAD: usize = 3000;

/// Safety margin for token estimation during compaction.
///
/// Our word-based heuristic (`estimate_tokens`) underestimates by 10-40%
/// for mixed content (code, non-English text, tool JSON). A 20% buffer
/// ensures we don't send prompts that exceed the model's context window.
///
/// Applied multiplicatively: `estimated_total * ESTIMATION_SAFETY_MARGIN`.
/// If `estimate_tokens` says 160K tokens, we treat it as 192K tokens.
pub const ESTIMATION_SAFETY_MARGIN: f32 = 1.20;

/// Calculate threshold values for a given context window
/// Returns (pre_tool, compaction, inter_tool, emergency) buffers
pub fn calculate_thresholds(context_window: usize) -> (usize, usize, usize, usize) {
    let pre_tool =
        ((context_window as f32 * (1.0 - MODERATE_USAGE_PERCENT)) as usize).max(PRE_TOOL_MIN);
    let compaction =
        ((context_window as f32 * (1.0 - CRITICAL_USAGE_PERCENT)) as usize).max(COMPACTION_MIN);
    let inter_tool =
        ((context_window as f32 * (1.0 - INTER_TOOL_USAGE_PERCENT)) as usize).max(INTER_TOOL_MIN);
    let emergency =
        ((context_window as f32 * (1.0 - EMERGENCY_USAGE_PERCENT)) as usize).max(EMERGENCY_MIN);

    (pre_tool, compaction, inter_tool, emergency)
}

/// Check if context needs pre-tool warning
/// Returns true when usage exceeds MODERATE_THRESHOLD (75%).
pub fn needs_pre_tool_compaction(session: &ChatSession, context_window: usize) -> bool {
    let real_tokens = session.history_real_tokens();
    let (pre_tool, _, _, _) = calculate_thresholds(context_window);
    let threshold = context_window.saturating_sub(pre_tool);
    real_tokens >= threshold
}

/// Check if context needs compaction
/// Triggers auto-compaction when usage exceeds CRITICAL_THRESHOLD (88%).
pub fn needs_buffered_compaction(session: &ChatSession, context_window: usize) -> bool {
    let real_tokens = session.history_real_tokens();
    let (_, compaction, _, _) = calculate_thresholds(context_window);
    let threshold = context_window.saturating_sub(compaction);
    real_tokens >= threshold
}

/// Check if context needs inter-tool warning
/// Called after each tool result during multi-tool execution.
/// Returns true when usage exceeds INTER_TOOL_THRESHOLD (94%).
///
/// IMPORTANT: total_tokens should be the FULL prompt size from Ollama's prompt_eval_count
/// (includes system + tools + history). Do NOT add system_tokens again.
pub fn needs_inter_tool_compaction(total_tokens: usize, context_window: usize) -> bool {
    let (_, _, inter_tool, _) = calculate_thresholds(context_window);
    let threshold = context_window.saturating_sub(inter_tool);
    total_tokens >= threshold
}

/// Check if context is in emergency state
/// At this point, tool results must be truncated before adding to history.
///
/// IMPORTANT: total_tokens should be the FULL prompt size from Ollama's prompt_eval_count
/// (includes system + tools + history). Do NOT add system_tokens again.
pub fn is_emergency_context(total_tokens: usize, context_window: usize) -> bool {
    let (_, _, _, emergency) = calculate_thresholds(context_window);
    let threshold = context_window.saturating_sub(emergency);
    total_tokens >= threshold
}

/// Calculate available token budget for tool results
/// Returns the number of tokens available before reaching emergency limit.
///
/// IMPORTANT: total_tokens should be the FULL prompt size from Ollama's prompt_eval_count
/// (includes system + tools + history). Do NOT add system_tokens again.
pub fn calculate_available_budget(total_tokens: usize, context_window: usize) -> usize {
    let (_, _, _, emergency) = calculate_thresholds(context_window);
    let emergency_limit = context_window.saturating_sub(emergency);
    emergency_limit
        .saturating_sub(total_tokens)
        .saturating_sub(RESPONSE_MARGIN)
}

// NOTE: estimate_chat_messages_tokens was removed in W2 #121 commit 4.
// It was a parallel estimator that duplicated the logic now provided by
// `ContextUsage::with_growth` (in src/tokens.rs). All call sites have been
// migrated to the unified `ContextUsage` struct. The estimator logic
// lives in `tokens::estimate_tokens` + `tokens::MESSAGE_OVERHEAD`; the
// `with_growth` method applies both consistently.

/// Context overflow status
#[derive(Debug, Clone)]
pub enum ContextStatus {
    /// Context is within normal limits
    Ok {
        /// Total tokens used
        total_tokens: usize,
        /// Maximum tokens allowed (unused, kept for future config display)
        #[allow(dead_code)]
        max_tokens: usize,
    },
    /// Context is approaching limits (warning)
    Warning {
        /// Total tokens used
        total_tokens: usize,
        /// Maximum tokens allowed (unused, kept for future config display)
        #[allow(dead_code)]
        max_tokens: usize,
        /// Usage percentage
        usage_percent: u8,
    },
    /// Context has exceeded threshold (overflow)
    Overflow {
        /// Total tokens used
        total_tokens: usize,
        /// Maximum tokens allowed (unused, kept for future config display)
        #[allow(dead_code)]
        max_tokens: usize,
        /// Usage percentage
        usage_percent: u8,
    },
}

impl ContextStatus {
    /// Check if context needs compaction (Warning or Overflow)
    pub fn needs_compaction(&self) -> bool {
        matches!(
            self,
            ContextStatus::Warning { .. } | ContextStatus::Overflow { .. }
        )
    }

    /// Check if context is at warning level (≥72%)
    ///
    /// Returns true when context usage is between 72% and 80%.
    /// Used internally by auto-compaction to determine urgency.
    #[allow(dead_code)]
    pub fn is_warning(&self) -> bool {
        matches!(self, ContextStatus::Warning { .. })
    }

    /// Check if context is at overflow level (≥80%)
    ///
    /// Returns true when context usage is at or above 80%.
    /// Used internally by auto-compaction to determine urgency.
    #[allow(dead_code)]
    pub fn is_overflow(&self) -> bool {
        matches!(self, ContextStatus::Overflow { .. })
    }

    /// Get usage percentage
    pub fn usage_percent(&self) -> u8 {
        match self {
            ContextStatus::Ok { .. } => 0,
            ContextStatus::Warning { usage_percent, .. } => *usage_percent,
            ContextStatus::Overflow { usage_percent, .. } => *usage_percent,
        }
    }

    /// Get total tokens
    pub fn total_tokens(&self) -> usize {
        match self {
            ContextStatus::Ok { total_tokens, .. } => *total_tokens,
            ContextStatus::Warning { total_tokens, .. } => *total_tokens,
            ContextStatus::Overflow { total_tokens, .. } => *total_tokens,
        }
    }

    /// Get max tokens (context window size)
    pub fn max_tokens(&self) -> usize {
        match self {
            ContextStatus::Ok { max_tokens, .. } => *max_tokens,
            ContextStatus::Warning { max_tokens, .. } => *max_tokens,
            ContextStatus::Overflow { max_tokens, .. } => *max_tokens,
        }
    }
}

/// Check if context has overflowed the threshold (75% warning, 88% critical)
pub fn check_context_overflow(
    session: &ChatSession,
    system_prompt: &str,
    context_window: usize,
) -> ContextStatus {
    // Try to get real token count from Ollama's last prompt_eval_count
    // This is already the TOTAL prompt size (system + tools + history)
    let real_tokens = session.history_real_tokens();

    // Calculate total tokens
    // If real_tokens > 0, it's the cumulative prompt size from Ollama
    // (includes system prompt, tools definitions if injected, and history)
    let total_tokens = if real_tokens > 0 {
        // Use real value from Ollama
        real_tokens
    } else {
        // W2 #121 follow-up: delegate to ContextUsage::from_session_estimate
        // (defined in src/tokens.rs). This consolidates the fallback math
        // — no more hardcoded `50 * 34` tool estimate (P9) and the same
        // heuristic used everywhere else in the codebase.
        let usage = crate::tokens::ContextUsage::from_session_estimate(
            session,
            system_prompt,
            session.tools,
        );
        usage.total_tokens
    };

    let usage = total_tokens as f32 / context_window as f32;
    let usage_percent = (usage * 100.0).min(100.0) as u8;

    // Use percentage-based thresholds consistent with calculate_thresholds()
    // MODERATE (yellow): >= 75% used
    // CRITICAL (red): >= 88% used
    if usage >= CRITICAL_USAGE_PERCENT {
        ContextStatus::Overflow {
            total_tokens,
            max_tokens: context_window,
            usage_percent,
        }
    } else if usage >= MODERATE_USAGE_PERCENT {
        // Warning at 75% used (synchronizes with MODERATE color in /context)
        ContextStatus::Warning {
            total_tokens,
            max_tokens: context_window,
            usage_percent,
        }
    } else {
        ContextStatus::Ok {
            total_tokens,
            max_tokens: context_window,
        }
    }
}

/// Middle compaction result
#[derive(Debug, Clone)]
pub struct CompactionSuggestion {
    /// Number of messages to keep at the beginning
    pub keep_first: usize,
    /// Number of messages to keep at the end
    pub keep_last: usize,
    /// Indices of messages to compact (middle section)
    pub middle_indices: std::ops::Range<usize>,
}

/// Calculate which messages should be compacted using default keep values
pub fn get_compaction_range_default(session: &ChatSession) -> Option<CompactionSuggestion> {
    get_compaction_range(session, DEFAULT_KEEP_FIRST, DEFAULT_KEEP_LAST)
}

/// Calculate which messages should be compacted (middle compaction)
///
/// Returns None if there aren't enough messages to compact.
pub fn get_compaction_range(
    session: &ChatSession,
    keep_first: usize,
    keep_last: usize,
) -> Option<CompactionSuggestion> {
    let total = session.messages.len();

    // Need at least keep_first + keep_last + some messages in middle
    if total <= keep_first + keep_last {
        return None;
    }

    let middle_start = keep_first;
    let middle_end = total.saturating_sub(keep_last);

    if middle_start >= middle_end {
        return None;
    }

    Some(CompactionSuggestion {
        keep_first,
        keep_last,
        middle_indices: middle_start..middle_end,
    })
}

/// Estimate tokens that would be saved by compaction
///
/// Useful for deciding if compaction is worthwhile before invoking LLM.
/// Currently not used in auto-compaction flow, but planned for smart
/// auto-compaction that compares estimated savings vs. compaction cost.
///
/// # Arguments
/// * `session` - Chat session with messages
/// * `suggestion` - Compaction suggestion from `get_compaction_range()`
/// * `summary_overhead` - Estimated tokens for the summary (~500-1000)
///
/// # Returns
/// Estimated tokens saved by compacting the middle section
#[allow(dead_code)]
pub fn estimate_compaction_savings(
    session: &ChatSession,
    suggestion: &CompactionSuggestion,
    summary_overhead: usize,
) -> usize {
    let middle_tokens: usize = session.messages[suggestion.middle_indices.clone()]
        .iter()
        .map(|msg| estimate_tokens(&msg.content) + 4)
        .sum();

    // Savings = middle_tokens - summary_overhead
    middle_tokens.saturating_sub(summary_overhead)
}

/// Determine if we should use the summary context position
/// (after system, before recent messages)
///
/// According to "lost in the middle" research, important content should be
/// at BEGINNING or END, not middle. Summary should go after system prompt
/// (beginning) to avoid being lost.
///
/// Currently always returns true when summary exists. Planned for future
/// context optimization strategies that may place summary differently.
#[allow(dead_code)]
pub fn should_position_summary_after_system(session: &ChatSession) -> bool {
    // According to "lost in the middle" research, important content should be
    // at BEGINNING or END, not middle.
    // Summary should go after system prompt (beginning) to avoid being lost.

    session.compacted_summary.is_some()
}

// ── Layer 1: Pre-Compaction Pruning ────────────────────────────────
//
// Strips long tool outputs from messages before sending them to the
// compaction LLM. This reduces the compaction prompt size, often
// bringing it below the model's context window without losing important
// context (tool outputs are typically verbose and low-information-density).

/// Estimate how many characters to keep from `text` to stay at or under
/// `target_tokens` estimated tokens.
///
/// Uses binary search over the char count, calling `estimate_tokens` on the
/// prefix each iteration. The result is the largest char count such that
/// `estimate_tokens(prefix) <= target_tokens`.
///
/// `estimate_tokens` has a 30-50% undercount bias (see src/tokens.rs), so
/// this is approximate — the actual real-token count of the kept prefix
/// could be ~33% higher than `target_tokens`. This is acceptable for the
/// pre-pruning use case (we want a coarse budget, not exact sizing).
pub fn chars_for_tokens(text: &str, target_tokens: usize) -> usize {
    let total_chars = text.chars().count();
    if total_chars == 0 || target_tokens == 0 {
        return 0;
    }
    // Fast path: full text is already under budget.
    if estimate_tokens(text) <= target_tokens {
        return total_chars;
    }
    // Binary search for the largest prefix whose estimated tokens <= target.
    let mut lo: usize = 0;
    let mut hi: usize = total_chars;
    while lo < hi {
        let mid = (lo + hi).saturating_add(1) / 2; // upper mid to avoid infinite loop
        let candidate: String = text.chars().take(mid).collect();
        if estimate_tokens(&candidate) <= target_tokens {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// Prune long tool results from a list of messages.
///
/// Replaces tool message content exceeding `PRUNE_TOOL_RESULT_THRESHOLD_TOKENS`
/// (estimated tokens) with a truncated version that preserves the first
/// `PRUNE_TOOL_RESULT_KEEP_TOKENS` (estimated tokens) plus a notice of how
/// many characters were removed.
///
/// W2 #121 follow-up: this previously used `PRUNE_TOOL_RESULT_THRESHOLD = 500`
/// (chars) and `PRUNE_TOOL_RESULT_KEEP_CHARS = 100` (chars). The threshold
/// and keep values are now expressed in ESTIMATED TOKENS (using
/// `estimate_tokens` from src/tokens.rs), which is the unit that matters
/// for the compaction prompt size. The conversion from tokens to chars
/// is done by `chars_for_tokens()` via binary search.
///
/// User and Assistant messages are NEVER pruned — only Tool role messages.
/// This preserved critical context (user instructions, assistant decisions)
/// while dramatically reducing verbose tool output (file reads, shell
/// outputs, search results).
///
/// Returns a new `Vec<SavedMessage>` with pruned content.
pub fn pre_prune_messages(messages: &[SavedMessage]) -> Vec<SavedMessage> {
    messages
        .iter()
        .map(|msg| {
            if msg.role == MessageRole::Tool
                && estimate_tokens(&msg.content) > PRUNE_TOOL_RESULT_THRESHOLD_TOKENS
            {
                let target_chars = chars_for_tokens(&msg.content, PRUNE_TOOL_RESULT_KEEP_TOKENS);
                let total_chars = msg.content.chars().count();
                let truncated_chars = total_chars.saturating_sub(target_chars);
                let kept: String = msg.content.chars().take(target_chars).collect();
                let pruned_content = format!(
                    "{}…\n[{} characters truncated — tool output pruned for compaction (kept first {} estimated tokens)]",
                    kept, truncated_chars, PRUNE_TOOL_RESULT_KEEP_TOKENS
                );
                SavedMessage {
                    content: pruned_content,
                    ..msg.clone()
                }
            } else {
                msg.clone()
            }
        })
        .collect()
}

/// Estimate the total tokens in a list of messages.
///
/// Uses word-based heuristic (0.75 words per token) plus `MESSAGE_OVERHEAD`
/// per message. This is the same estimation used by `check_context_overflow`.
pub fn estimate_messages_tokens(messages: &[SavedMessage]) -> usize {
    messages
        .iter()
        .map(|msg| estimate_tokens(&msg.content) + MESSAGE_OVERHEAD)
        .sum()
}

/// Estimate tokens for compaction purposes with safety margin.
///
/// Uses `COMPACT_MSG_OVERHEAD` (10 tokens per message) instead of the
/// default `MESSAGE_OVERHEAD` (4 tokens) to account for role prefixes
/// and formatting in `build_conversation_text()`. Applies a 20% safety
/// margin (`ESTIMATION_SAFETY_MARGIN`) to compensate for underestimation
/// in mixed-content scenarios (code, non-English text, tool JSON).
///
/// This function should be used instead of `estimate_messages_tokens()`
/// in all compaction-related token calculations where accuracy is critical
/// for avoiding context window overflow.
pub fn estimate_compaction_tokens(messages: &[SavedMessage]) -> usize {
    let raw: usize = messages
        .iter()
        .map(|msg| estimate_tokens(&msg.content) + COMPACT_MSG_OVERHEAD)
        .sum();
    ((raw as f32) * ESTIMATION_SAFETY_MARGIN).ceil() as usize
}

/// Check if a list of messages fits within a token budget.
///
/// Returns `true` if the estimated tokens in `messages` (with safety margin)
/// plus `overhead_tokens` fit within `context_window`.
///
/// Uses `estimate_compaction_tokens()` which applies a 20% safety margin
/// and higher per-message overhead to account for tokenization variance.
pub fn fits_in_context(
    messages: &[SavedMessage],
    context_window: usize,
    overhead_tokens: usize,
) -> bool {
    let msg_tokens = estimate_compaction_tokens(messages);
    msg_tokens + overhead_tokens <= context_window
}

/// Check if an error from the LLM indicates a context/prompt overflow.
///
/// Matches common error patterns from Ollama and other LLM backends
/// when the prompt exceeds the model's context window. Used by
/// `compact_conversation()` to detect overflow and fall back to the
/// next compaction layer.
///
/// # Examples
///
/// ```
/// use sprachspiel::context_overflow::is_prompt_too_long_error;
///
/// assert!(is_prompt_too_long_error(
///     "The prompt is too long: 240047, model maximum context length: 202752"
/// ));
/// assert!(is_prompt_too_long_error("context_length_exceeded"));
/// assert!(!is_prompt_too_long_error("connection refused"));
/// ```
pub fn is_prompt_too_long_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();
    error_lower.contains("prompt is too long")
        || error_lower.contains("context length")
        || error_lower.contains("maximum context length")
        || error_lower.contains("exceeds context")
        || error_lower.contains("context_length_exceeded")
}

/// Calculate the maximum chunk size in tokens for recursive summarization.
///
/// Each chunk must leave room for:
/// - The system prompt for summarization (~200 tokens)
/// - The compaction prompt instructions (~300 tokens)
/// - The model's response (~2000 tokens)
///
/// Uses `COMPACTION_MAX_CONTEXT_RATIO` (60%) as the target ratio.
pub fn max_chunk_tokens(context_window: usize) -> usize {
    ((context_window as f32) * COMPACTION_MAX_CONTEXT_RATIO) as usize
}

// ── Layer 2: Chunked Recursive Summarization ──────────────────────
//
// When the pruned messages still exceed the model's context window,
// we split them into chunks that each fit, summarize each chunk
// independently, then combine the summaries. If the combined summaries
// still exceed the window, we recurse.

/// A chunk of messages created by `split_into_chunks`.
#[derive(Debug, Clone)]
pub struct MessageChunk {
    /// Messages in this chunk
    pub messages: Vec<SavedMessage>,
    /// Estimated tokens in this chunk
    pub token_count: usize,
}

/// Split messages into chunks that each fit within a token budget.
///
/// Each chunk contains consecutive messages whose combined token count
/// does not exceed `max_tokens`. Messages are never split mid-message.
///
/// Adjacent chunks overlap by one message: the last message of chunk N
/// is also the first message of chunk N+1. This maintains coherence at
/// chunk boundaries and prevents losing context between chunks.
///
/// Returns at least one chunk (even if it exceeds `max_tokens`).
pub fn split_into_chunks(messages: &[SavedMessage], max_tokens: usize) -> Vec<MessageChunk> {
    if messages.is_empty() {
        return vec![];
    }

    // If total tokens fit in one chunk, return as-is
    let total_tokens = estimate_messages_tokens(messages);
    if total_tokens <= max_tokens {
        return vec![MessageChunk {
            messages: messages.to_vec(),
            token_count: total_tokens,
        }];
    }

    let mut chunks = Vec::new();
    let mut current_messages: Vec<SavedMessage> = Vec::new();
    let mut current_tokens = 0usize;

    for msg in messages {
        let msg_tokens = estimate_tokens(&msg.content) + MESSAGE_OVERHEAD;

        // If adding this message would exceed the budget AND we already have
        // messages in the current chunk, start a new chunk
        if !current_messages.is_empty() && current_tokens + msg_tokens > max_tokens {
            chunks.push(MessageChunk {
                messages: current_messages.clone(),
                token_count: current_tokens,
            });

            // Overlap: include the last message of the current chunk as the
            // first message of the next chunk for coherence
            if let Some(last_msg) = current_messages.last() {
                let overlap_msg = last_msg.clone();
                let overlap_tokens = estimate_tokens(&overlap_msg.content) + MESSAGE_OVERHEAD;
                current_messages.clear();
                current_messages.push(overlap_msg);
                current_tokens = overlap_tokens;
            }
        }

        current_messages.push(msg.clone());
        current_tokens += msg_tokens;
    }

    // Don't forget the last chunk
    if !current_messages.is_empty() {
        chunks.push(MessageChunk {
            messages: current_messages,
            token_count: current_tokens,
        });
    }

    // Edge case: if we ended up with zero chunks (shouldn't happen with
    // non-empty input), return the original messages as a single chunk
    if chunks.is_empty() && !messages.is_empty() {
        chunks.push(MessageChunk {
            messages: messages.to_vec(),
            token_count: total_tokens,
        });
    }

    chunks
}

// ── Layer 3: Fallback Truncation ───────────────────────────────────
//
// When pre-pruning and recursive summarization both fail (model
// unavailable, timeout, max recursion exceeded), hard-truncate the
// oldest messages from the middle section to fit within the context window.

/// Result of fallback truncation.
#[derive(Debug)]
pub struct TruncationResult {
    /// The truncated messages (may be empty if all middle messages were dropped)
    pub remaining_messages: Vec<SavedMessage>,
    /// Number of messages that were dropped
    pub dropped_count: usize,
    /// Estimated tokens in remaining messages
    pub remaining_tokens: usize,
}

/// Truncate messages from the beginning to fit within a token budget.
///
/// Drops oldest messages first (preserving the most recent context)
/// until the total tokens fit within `context_window * TRUNCATION_TARGET_RATIO`.
///
/// This is the last resort when pre-pruning and recursive summarization
/// have both failed. It sacrifices the oldest context to ensure the
/// compaction prompt fits the model's window.
///
/// NEVER drops messages from the first `keep_first` or last `keep_last`
/// messages — these contain the user's original request and the most
/// recent context, which are critical for continuation.
pub fn fallback_truncate(
    messages: &[SavedMessage],
    context_window: usize,
    keep_first: usize,
    keep_last: usize,
) -> TruncationResult {
    let target_tokens = ((context_window as f32) * TRUNCATION_TARGET_RATIO) as usize;
    let total_tokens = estimate_messages_tokens(messages);
    let total = messages.len();

    // If messages already fit, nothing to do
    if total_tokens <= target_tokens || total <= keep_first + keep_last {
        return TruncationResult {
            remaining_messages: messages.to_vec(),
            dropped_count: 0,
            remaining_tokens: total_tokens,
        };
    }

    // Drop messages from the middle, starting from `keep_first`
    // and working towards the end, preserving the last `keep_last` messages.
    // We drop oldest-middle messages first (they contain the least
    // relevant context — "lost in the middle" research confirms this).
    let max_drop = total.saturating_sub(keep_first + keep_last);
    let mut dropped = 0;

    for drop_count in 1..=max_drop {
        // Build the remaining message list: first `keep_first` + messages
        // from `keep_first + drop_count` to end minus last `keep_last`
        let middle_start = keep_first + drop_count;
        let middle_end = total.saturating_sub(keep_last);

        if middle_start >= middle_end {
            // Can't drop any more from the middle
            break;
        }

        let remaining: Vec<SavedMessage> = messages[0..keep_first]
            .iter()
            .chain(messages[middle_start..middle_end].iter())
            .chain(messages[total - keep_last..total].iter())
            .cloned()
            .collect();

        let remaining_tokens = estimate_messages_tokens(&remaining);

        if remaining_tokens <= target_tokens {
            return TruncationResult {
                remaining_messages: remaining,
                dropped_count: drop_count,
                remaining_tokens,
            };
        }

        dropped = drop_count;
    }

    // If we couldn't reach the target even dropping all middle messages,
    // return just the kept messages
    let kept: Vec<SavedMessage> = messages[0..keep_first]
        .iter()
        .chain(messages[total - keep_last..total].iter())
        .cloned()
        .collect();
    let kept_tokens = estimate_messages_tokens(&kept);

    TruncationResult {
        remaining_messages: kept,
        dropped_count: dropped.max(1),
        remaining_tokens: kept_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::session::{MessageRole, SavedMessage};
    use chrono::Utc;

    fn create_test_session(message_count: usize) -> ChatSession {
        let mut session = ChatSession::new("test-model".to_string(), None, false);

        for i in 0..message_count {
            session.messages.push(SavedMessage {
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("Message {} content here with some tokens to count", i),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        session
    }

    #[test]
    fn test_needs_pre_tool_compaction_below_threshold() {
        // Session with low context usage (below 75%)
        let mut session = ChatSession::new("test-model".to_string(), None, false);

        // Add a few small messages (well below threshold)
        for i in 0..5 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: format!("Short message {}", i),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        let context_window = 128000; // Typical large context

        // Should NOT need pre-tool compaction (20K remaining, we used ~10K)
        assert!(
            !needs_pre_tool_compaction(&session, context_window),
            "Session with plenty of room should not need pre-tool compaction"
        );
    }

    #[test]
    fn test_needs_pre_tool_compaction_above_threshold() {
        // Session with high context usage (near limit)
        let mut session = ChatSession::new("test-model".to_string(), None, false);

        // Fill session with large content to exceed threshold
        // need_pre_tool_compaction triggers when 20K tokens remaining
        // For 128K context: trigger at 108K used
        // We use ~200K tokens to definitely exceed
        let large_content = "word ".repeat(50000); // ~67000 tokens
        for _ in 0..3 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: large_content.clone(),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        let context_window = 128000;

        // Should need pre-tool compaction (< 20K remaining)
        assert!(
            needs_pre_tool_compaction(&session, context_window),
            "Session with < 20K tokens remaining should need pre-tool compaction"
        );
    }

    #[test]
    fn test_context_status_percentages() {
        // Test that thresholds align correctly
        // Warning: 72% = 0.9 * 0.8 (90% of overflow threshold)
        // Overflow: 80%
        // Pre-tool: 75%

        // At 70%: OK
        let status_ok = ContextStatus::Ok {
            total_tokens: 7000,
            max_tokens: 10000,
        };
        assert!(!status_ok.needs_compaction());

        // At 75%: Warning (above pre-tool threshold)
        let status_warn = ContextStatus::Warning {
            total_tokens: 7500,
            max_tokens: 10000,
            usage_percent: 75,
        };
        assert!(status_warn.needs_compaction());
        assert!(!status_warn.is_overflow());

        // At 80%: Overflow
        let status_over = ContextStatus::Overflow {
            total_tokens: 8000,
            max_tokens: 10000,
            usage_percent: 80,
        };
        assert!(status_over.needs_compaction());
        assert!(status_over.is_overflow());
    }

    #[test]
    fn test_check_context_overflow_respects_compaction() {
        // Session without compaction - all messages should be counted
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add 10 messages with lots of content
        for _ in 0..10 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: "This is a long message with lots of content to test token counting in the context overflow check".into(),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        let status_no_compact = check_context_overflow(&session, "System prompt", 1000);
        let tokens_no_compact = status_no_compact.total_tokens();

        // Now compact first 5 messages
        session.messages_sent_to_llm = 5;
        session.compacted_summary = Some("This is a summary of the first 5 messages".into());

        let status_with_compact = check_context_overflow(&session, "System prompt", 1000);
        let tokens_with_compact = status_with_compact.total_tokens();

        // With compaction, should have fewer tokens
        assert!(
            tokens_with_compact < tokens_no_compact,
            "Compacted session should have fewer tokens: {} < {}",
            tokens_with_compact,
            tokens_no_compact
        );

        // Difference should be about 5 messages
        let diff = tokens_no_compact - tokens_with_compact;
        assert!(
            diff > 50,
            "Should have removed at least 50 tokens from compacted messages, got: {}",
            diff
        );
    }

    #[test]
    fn test_check_context_overflow_includes_summary() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add 2 messages (will all be sent to LLM, no compaction)
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "Hello".into(),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: "Hi".into(),
            timestamp: Utc::now(),
            ..Default::default()
        });

        let status_no_summary = check_context_overflow(&session, "System", 1000);
        let tokens_no_summary = status_no_summary.total_tokens();

        // Add a summary with proper compaction state
        // Use set_compacted_summary_with_range to properly set messages_sent_to_llm
        session.set_compacted_summary_with_range(
            "This is a summary of the previous conversation about important topics".into(),
            None, // Full compaction
        );

        let status_with_summary = check_context_overflow(&session, "System", 1000);
        let tokens_with_summary = status_with_summary.total_tokens();

        // Summary should add tokens
        // Note: After full compaction, messages_sent_to_llm == messages.len()
        // so history_tokens from messages is 0, but summary_tokens is counted
        // plus MESSAGE_OVERHEAD for the summary message
        assert!(
            tokens_with_summary > tokens_no_summary,
            "Summary should add tokens: {} > {}",
            tokens_with_summary,
            tokens_no_summary
        );
    }

    #[test]
    fn test_buffer_hierarchy() {
        // Threshold hierarchy should be: PRE_TOOL > COMPACTION > INTER_TOOL > EMERGENCY
        // This ensures correct trigger order for any context size
        let context_32k = 32768;
        let (pre_tool, compaction, inter_tool, emergency) = calculate_thresholds(context_32k);

        assert!(
            pre_tool > compaction,
            "Pre-tool buffer ({}) should be larger than compaction buffer ({})",
            pre_tool,
            compaction
        );
        assert!(
            compaction > inter_tool,
            "Compaction buffer ({}) should be larger than inter-tool buffer ({})",
            compaction,
            inter_tool
        );
        assert!(
            inter_tool > emergency,
            "Inter-tool buffer ({}) should be larger than emergency buffer ({})",
            inter_tool,
            emergency
        );
    }

    #[test]
    fn test_needs_inter_tool_compaction_below() {
        // Context with plenty of room (100K context, 75K total = 25K remaining)
        // 25K remaining > inter_tool threshold (6% of 100K = 6K), so should NOT trigger
        let total_tokens = 75_000;
        let context_window = 100_000;

        assert!(
            !needs_inter_tool_compaction(total_tokens, context_window),
            "Should not need inter-tool compaction when 25K tokens remaining"
        );
    }

    #[test]
    fn test_needs_inter_tool_compaction_above() {
        // Context near limit (100K context, 95K total = 5K remaining)
        // 5K remaining < inter_tool threshold (6% of 100K = 6K), so SHOULD trigger
        let total_tokens = 95_000;
        let context_window = 100_000;

        assert!(
            needs_inter_tool_compaction(total_tokens, context_window),
            "Should need inter-tool compaction when only 5K tokens remaining"
        );
    }

    #[test]
    fn test_check_context_overflow() {
        // Test with default threshold (75% used = Warning)
        let session = create_test_session(100);
        let status = check_context_overflow(&session, "System prompt", 4096);

        // 100 messages should use significant context
        // The function returns a valid status (we just check it doesn't panic)
        let _ = status.usage_percent();

        // Small session should be Ok
        let small_session = create_test_session(5);
        let small_status = check_context_overflow(&small_session, "System prompt", 4096);
        // With fallback estimation, small session might still exceed 75% of 4K
        // Just verify the function works
        let _ = small_status.usage_percent();
    }

    #[test]
    fn test_is_emergency_context_above() {
        // Context at emergency (100K context, 98K total = 2K remaining)
        // 2K remaining < emergency threshold (3% of 100K = 3K), so SHOULD be emergency
        let total_tokens = 98_000;
        let context_window = 100_000;

        assert!(
            is_emergency_context(total_tokens, context_window),
            "Should be emergency when only 2K tokens remaining"
        );
    }

    #[test]
    fn test_calculate_available_budget_normal() {
        // Context at 50% with emergency buffer and margin
        let total_tokens = 50_000;
        let context_window = 100_000;

        let available = calculate_available_budget(total_tokens, context_window);

        // emergency_threshold (3%) = 3% of 100K = 3000
        // emergency_limit = 100K - 3000 = 97K
        // available = 97K - 50K - 2K (response margin) = 45K
        // Note: Small rounding differences are acceptable
        assert!(
            available >= 44_990 && available <= 45_010,
            "Should calculate available budget correctly, got {}",
            available
        );
    }

    #[test]
    fn test_calculate_available_budget_plenty() {
        // Context at 10% with large context
        let total_tokens = 12_000;
        let context_window = 200_000;

        let available = calculate_available_budget(total_tokens, context_window);

        // emergency_threshold = 3% of 200K = 6K
        // emergency_limit = 200K - 6K = 194K
        // available = 194K - 12K - 2K = 180K
        assert!(
            available > 175_000,
            "Should have plenty of budget available: got {}",
            available
        );
    }

    #[test]
    fn test_threshold_relationships() {
        // Verify the buffer hierarchy using calculate_thresholds
        let context_window = 32768; // 32K
        let (pre_tool, compaction, inter_tool, emergency) = calculate_thresholds(context_window);

        // Verify hierarchy: PRE_TOOL > COMPACTION > INTER_TOOL > EMERGENCY
        assert!(pre_tool > compaction);
        assert!(compaction > inter_tool);
        assert!(inter_tool > emergency);

        // Verify specific values for 32K context
        // 75% usage = 8192 remaining (25%)
        // 88% usage = 3932 remaining (12%)
        // 94% usage = 1966 remaining (6%)
        // 97% usage = 983 remaining (3%)
        assert_eq!(
            pre_tool, 8192,
            "32K: pre_tool should be 8192 (25%% remaining)"
        );
        assert_eq!(
            compaction, 3932,
            "32K: compaction should be 3932 (12%% remaining)"
        );
        assert_eq!(
            inter_tool, 1966,
            "32K: inter_tool should be 1966 (6%% remaining)"
        );
        assert_eq!(
            emergency, 983,
            "32K: emergency should be 983 (3%% remaining)"
        );
    }

    // ── Compaction limits removed ──────────────────────────────

    #[test]
    fn test_compaction_thresholds_are_positive() {
        // Verify compaction thresholds exist and are positive.
        // This test also serves as a regression guard: if someone
        // re-introduces a MAX_SUMMARY_TOKENS-like constant with a
        // numeric assertion, the test name will make the intent clear.
        // The absence of token limits on summaries is a deliberate
        // architectural decision — see COMPACTION_PROMPT doc comment.
        assert!(COMPACTION_MIN > 0, "COMPACTION_MIN must be positive");
        assert!(PRE_TOOL_MIN > 0, "PRE_TOOL_MIN must be positive");
    }

    // ── Layer 1: Pre-Compaction Pruning Tests ──────────────────

    #[test]
    fn test_pre_prune_short_tool_result_unchanged() {
        // Tool results shorter than the threshold should be kept as-is
        let msg = SavedMessage {
            role: MessageRole::Tool,
            content: "Short tool result".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        let pruned = pre_prune_messages(&[msg]);
        assert_eq!(pruned[0].content, "Short tool result");
    }

    #[test]
    fn test_pre_prune_long_tool_result_truncated() {
        // W2 #121: tool results with > PRUNE_TOOL_RESULT_THRESHOLD_TOKENS
        // estimated tokens should be truncated. We use a string of repeated
        // "x " tokens — ~1000 chars ≈ 250 estimated tokens, just above
        // the 200 threshold.
        let long_content: String = "x ".repeat(500);
        let original_token_count = estimate_tokens(&long_content);
        assert!(
            original_token_count > PRUNE_TOOL_RESULT_THRESHOLD_TOKENS,
            "Test setup: long_content should exceed threshold tokens ({} vs {})",
            original_token_count,
            PRUNE_TOOL_RESULT_THRESHOLD_TOKENS
        );
        let msg = SavedMessage {
            role: MessageRole::Tool,
            content: long_content.clone(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        let pruned = pre_prune_messages(&[msg]);
        assert!(
            pruned[0].content.len() < long_content.len(),
            "Pruned content ({} chars) should be shorter than original ({} chars)",
            pruned[0].content.len(),
            long_content.len()
        );
        assert!(
            pruned[0].content.contains("truncated"),
            "Pruned content should mention truncation"
        );
        // The kept prefix should be ~PRUNE_TOOL_RESULT_KEEP_TOKENS estimated
        // tokens. We compare prefixes: the kept part should match the start
        // of the original.
        let kept_chars = chars_for_tokens(&long_content, PRUNE_TOOL_RESULT_KEEP_TOKENS);
        let kept_prefix: String = long_content.chars().take(kept_chars).collect();
        assert!(
            pruned[0].content.contains(&kept_prefix),
            "Pruned content should preserve the first {} chars (estimated {} tokens)",
            kept_chars,
            PRUNE_TOOL_RESULT_KEEP_TOKENS
        );
    }

    #[test]
    fn test_pre_prune_user_message_unchanged() {
        // User messages should NEVER be pruned, regardless of length
        let long_content: String = "x ".repeat(500);
        let msg = SavedMessage {
            role: MessageRole::User,
            content: long_content.clone(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        let pruned = pre_prune_messages(&[msg]);
        assert_eq!(pruned[0].content, long_content);
    }

    #[test]
    fn test_chars_for_tokens_basic() {
        // Short text under budget returns full char count
        let text = "hello world";
        assert_eq!(chars_for_tokens(text, 100), text.chars().count());

        // Empty text returns 0
        assert_eq!(chars_for_tokens("", 100), 0);

        // Zero target returns 0
        assert_eq!(chars_for_tokens("hello", 0), 0);

        // Long text over budget returns the largest prefix that fits.
        // "x " repeated 1000 times ≈ 200-300 estimated tokens, so
        // target=10 should keep only a small prefix.
        let long = "x ".repeat(1000);
        let kept = chars_for_tokens(&long, 10);
        assert!(kept < long.chars().count(), "Should truncate ({} of {})", kept, long.chars().count());
        // The kept prefix should estimate to <= 10 tokens.
        let prefix: String = long.chars().take(kept).collect();
        assert!(estimate_tokens(&prefix) <= 10,
                "Prefix of {} chars estimated at {} tokens, should be <= 10",
                prefix.chars().count(), estimate_tokens(&prefix));
    }

    #[test]
    fn test_pre_prune_assistant_message_unchanged() {
        // Assistant messages should NEVER be pruned, regardless of length
        // W2 #121: long content expressed in token budget (500 "x " pairs
        // ≈ 250 estimated tokens, well above PRUNE_TOOL_RESULT_THRESHOLD_TOKENS).
        let long_content: String = "x ".repeat(500);
        let msg = SavedMessage {
            role: MessageRole::Assistant,
            content: long_content.clone(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        let pruned = pre_prune_messages(&[msg]);
        assert_eq!(pruned[0].content, long_content);
    }

    #[test]
    fn test_pre_prune_preserves_message_metadata() {
        // Pruning should preserve all metadata fields (timestamp, prompt_tokens, etc.)
        let long_content: String = "x ".repeat(500);
        let msg = SavedMessage {
            role: MessageRole::Tool,
            content: long_content,
            timestamp: Utc::now(),
            prompt_tokens: Some(42),
            message_type: Some("normal".to_string()),
            ..Default::default()
        };
        let pruned = pre_prune_messages(&[msg]);
        assert_eq!(pruned[0].prompt_tokens, Some(42));
        assert_eq!(pruned[0].message_type, Some("normal".to_string()));
    }

    // ── Layer 2: Chunked Recursive Summarization Tests ──────────

    #[test]
    fn test_estimate_messages_tokens() {
        let messages = vec![SavedMessage {
            role: MessageRole::User,
            content: "Hello world this is a test".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        }];
        let tokens = estimate_messages_tokens(&messages);
        // Should be MESSAGE_OVERHEAD + estimated tokens for content
        assert!(tokens > MESSAGE_OVERHEAD, "Should include overhead");
        assert!(tokens < 100, "Should be small for short content");
    }

    #[test]
    fn test_split_into_chunks_single_chunk() {
        // Messages that fit in one chunk should return a single chunk
        let messages: Vec<SavedMessage> = (0..3)
            .map(|i| SavedMessage {
                role: MessageRole::User,
                content: format!("Message {}", i),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();

        let chunks = split_into_chunks(&messages, 10_000);
        assert_eq!(chunks.len(), 1, "Should fit in a single chunk");
        assert_eq!(chunks[0].messages.len(), 3);
    }

    #[test]
    fn test_split_into_chunks_multiple_chunks() {
        // Messages that exceed a small budget should be split
        let messages: Vec<SavedMessage> = (0..10)
            .map(|i| SavedMessage {
                role: MessageRole::User,
                content: format!(
                    "This is message number {} with enough content to have some tokens",
                    i
                ),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();

        // Set a small budget that forces multiple chunks
        let chunks = split_into_chunks(&messages, 50);
        assert!(chunks.len() > 1, "Should split into multiple chunks");
    }

    #[test]
    fn test_split_into_chunks_overlapping() {
        // Each chunk should overlap with the previous one (sharing the last message)
        let messages: Vec<SavedMessage> = (0..6)
            .map(|i| SavedMessage {
                role: MessageRole::User,
                content: format!("Message {} with enough text to have tokens", i),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();

        let chunks = split_into_chunks(&messages, 50);

        if chunks.len() > 1 {
            // The last message of chunk N should be the first message of chunk N+1
            for i in 0..chunks.len() - 1 {
                let last_of_current = &chunks[i].messages.last().unwrap().content;
                let first_of_next = &chunks[i + 1].messages.first().unwrap().content;
                assert_eq!(
                    last_of_current, first_of_next,
                    "Adjacent chunks should overlap by one message"
                );
            }
        }
    }

    #[test]
    fn test_max_chunk_tokens() {
        // Verify chunk size is 60% of context window
        let chunk_size = max_chunk_tokens(200_000);
        assert_eq!(chunk_size, 120_000, "Should be 60% of context window");
    }

    #[test]
    fn test_fits_in_context() {
        let short_msg = SavedMessage {
            role: MessageRole::User,
            content: "Hi".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        assert!(
            fits_in_context(&[short_msg], 100_000, 0),
            "Short content should fit in context"
        );
    }

    // ── Layer 3: Fallback Truncation Tests ──────────────────────

    #[test]
    fn test_fallback_truncate_no_truncation_needed() {
        // Messages that fit in budget should not be truncated
        let messages: Vec<SavedMessage> = (0..5)
            .map(|i| SavedMessage {
                role: MessageRole::User,
                content: format!("Short msg {}", i),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();

        let result = fallback_truncate(&messages, 100_000, 1, 1);
        assert_eq!(result.dropped_count, 0, "Should not drop any messages");
        assert_eq!(
            result.remaining_messages.len(),
            5,
            "Should keep all messages"
        );
    }

    #[test]
    fn test_fallback_truncate_drops_middle_messages() {
        // Should drop messages from the middle, preserving first and last
        let messages: Vec<SavedMessage> = (0..20)
            .map(|i| SavedMessage {
                role: MessageRole::User,
                // Each message is ~200 tokens to force truncation
                content: format!(
                    "Message number {} with lots of content words here to make it longer",
                    i
                ),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();

        let result = fallback_truncate(&messages, 500, 2, 2);
        assert!(result.dropped_count > 0, "Should drop some messages");
        // Should still preserve first 2 and last 2
        let first_content = &result.remaining_messages[0].content;
        assert!(
            first_content.starts_with("Message number 0"),
            "First preserved msg should be msg 0"
        );
    }

    #[test]
    fn test_fallback_truncate_preserves_boundaries() {
        // With only keep_first + keep_last messages, nothing to truncate
        let messages: Vec<SavedMessage> = (0..4)
            .map(|i| SavedMessage {
                role: MessageRole::User,
                content: format!("Message {}", i),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();

        // With keep_first=2 and keep_last=2, there are 0 middle messages
        let result = fallback_truncate(&messages, 100, 2, 2);
        assert_eq!(
            result.dropped_count, 0,
            "Should not drop when total == boundaries"
        );
    }

    // ── Compaction safety margin and error detection tests ─────────────

    #[test]
    fn test_estimate_compaction_tokens_empty() {
        assert_eq!(estimate_compaction_tokens(&[]), 0);
    }

    #[test]
    fn test_estimate_compaction_tokens_with_margin() {
        // COMPACT_MSG_OVERHEAD (10) per message + safety margin (1.20x)
        // "hello world" = 2 words → estimate_tokens = ceil(2/0.75) = 3
        // raw = 10 + 3 = 13, with 1.20x = ceil(15.6) = 16
        let msg = SavedMessage {
            role: MessageRole::User,
            content: "hello world".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        let tokens = estimate_compaction_tokens(&[msg]);
        let raw = COMPACT_MSG_OVERHEAD + 3; // 10 + 3 = 13
        let expected = ((raw as f32) * ESTIMATION_SAFETY_MARGIN).ceil() as usize; // ceil(15.6) = 16
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_estimate_compaction_tokens_multiple_messages() {
        // 2 messages: (10 + 3) + (10 + 8) = 31, with 1.20x = ceil(37.2) = 38
        let msg1 = SavedMessage {
            role: MessageRole::User,
            content: "hello world".to_string(), // 2 words → 3 tokens
            timestamp: Utc::now(),
            ..Default::default()
        };
        let msg2 = SavedMessage {
            role: MessageRole::Assistant,
            content: "The quick brown fox jumps over the lazy dog".to_string(), // 9 words → 12 tokens
            timestamp: Utc::now(),
            ..Default::default()
        };
        let tokens = estimate_compaction_tokens(&[msg1, msg2]);
        let raw = (COMPACT_MSG_OVERHEAD + 3) + (COMPACT_MSG_OVERHEAD + 12); // 13 + 22 = 35
        let expected = ((raw as f32) * ESTIMATION_SAFETY_MARGIN).ceil() as usize; // ceil(42.0) = 42
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_estimate_compaction_tokens_is_more_conservative_than_messages_tokens() {
        // estimate_compaction_tokens should always be >= estimate_messages_tokens
        // because it uses COMPACT_MSG_OVERHEAD (10) instead of MESSAGE_OVERHEAD (4)
        // and applies ESTIMATION_SAFETY_MARGIN (1.20x)
        let msg = SavedMessage {
            role: MessageRole::User,
            content: "This is a test message with some content for estimation".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        let standard = estimate_messages_tokens(&[msg.clone()]);
        let compaction = estimate_compaction_tokens(&[msg]);
        assert!(
            compaction > standard,
            "Compaction estimate ({}) should be more conservative than standard ({})",
            compaction,
            standard
        );
    }

    #[test]
    fn test_fits_in_context_uses_safety_margin() {
        // fits_in_context should now use estimate_compaction_tokens
        // which includes COMPACT_MSG_OVERHEAD (10) and ESTIMATION_SAFETY_MARGIN (1.20)
        let short_msg = SavedMessage {
            role: MessageRole::User,
            content: "hello".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        };

        // With a very small context window and no overhead, it should not fit
        // because COMPACT_MSG_OVERHEAD (10) * 1.20 = 12 even for "hello" (2 tokens)
        assert!(
            !fits_in_context(&[short_msg.clone()], 5, 0),
            "Should not fit in tiny context window"
        );

        // With a large context window, it should fit
        assert!(
            fits_in_context(&[short_msg], 100_000, 0),
            "Should fit in large context window"
        );
    }

    #[test]
    fn test_is_prompt_too_long_error_ollama() {
        assert!(is_prompt_too_long_error(
            "The prompt is too long: 240047, model maximum context length: 202752"
        ));
    }

    #[test]
    fn test_is_prompt_too_long_error_variants() {
        assert!(is_prompt_too_long_error("context length exceeded"));
        assert!(is_prompt_too_long_error("maximum context length: 4096"));
        assert!(is_prompt_too_long_error("exceeds context window"));
        assert!(is_prompt_too_long_error("context_length_exceeded"));
        assert!(is_prompt_too_long_error(
            "Error: The prompt exceeds context length of 8192"
        ));
    }

    #[test]
    fn test_is_prompt_too_long_error_non_overflow() {
        assert!(!is_prompt_too_long_error("connection refused"));
        assert!(!is_prompt_too_long_error("model not found"));
        assert!(!is_prompt_too_long_error("timeout expired"));
        assert!(!is_prompt_too_long_error("internal server error"));
        assert!(!is_prompt_too_long_error(""));
    }

    #[test]
    fn test_compaction_prompt_overhead_value() {
        // COMPACTION_PROMPT_OVERHEAD should be 3000 (increased from 2500)
        assert_eq!(COMPACTION_PROMPT_OVERHEAD, 3000);
    }

    #[test]
    fn test_compact_msg_overhead_greater_than_message_overhead() {
        // COMPACT_MSG_OVERHEAD (10) should be greater than MESSAGE_OVERHEAD (4)
        assert!(
            COMPACT_MSG_OVERHEAD > MESSAGE_OVERHEAD,
            "COMPACT_MSG_OVERHEAD ({}) should be > MESSAGE_OVERHEAD ({})",
            COMPACT_MSG_OVERHEAD,
            MESSAGE_OVERHEAD
        );
    }

    #[test]
    fn test_estimation_safety_margin_applied() {
        // Verify the safety margin is 1.20
        assert!(
            (ESTIMATION_SAFETY_MARGIN - 1.20).abs() < 0.01,
            "ESTIMATION_SAFETY_MARGIN should be 1.20, got {}",
            ESTIMATION_SAFETY_MARGIN
        );
    }
}
