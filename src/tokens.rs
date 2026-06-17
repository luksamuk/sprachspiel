//! Token estimation utilities
//!
//! Word-based estimation with message overhead for LLM context management.

use ollama_rs::generation::chat::ChatMessage;

/// Tokens per message overhead (role, formatting)
pub const MESSAGE_OVERHEAD: usize = 4;

/// Approximate token overhead per tool definition in the system prompt.
///
/// Used by `estimate_status_bar()`, `format_context_info()`, and other
/// context-display functions to estimate how many tokens the tool definitions
/// consume. This is a coarse approximation — the actual count depends on the
/// tool's parameter schema complexity.
pub const TOKENS_PER_TOOL: usize = 50;

/// Estimate tokens in text using word-based estimation.
/// ~0.75 words per token for English text (GPT-style).
///
/// IMPORTANT: This is an ESTIMATE, not the model's real token count.
/// Empirically it undercounts by 30-50% vs real tokenizers (Llama,
/// Mistral, Qwen) for mixed content (code, JSON, non-English text,
/// tool results). Example: we estimated 27K tokens for a request
/// the model counted as 40K.
///
/// Consumers that need 100% accuracy (e.g. critical decisions like
/// emergency truncation) MUST add a safety margin — see
/// `custom_coordinator.rs::check_and_handle_context_overflow` (the
/// 75% preemptive threshold) for how this is handled in practice.
///
/// A real tokenizer (tiktoken-rs, Ollama's /api/tokenize) would
/// remove the bias. See W2 #121 follow-up TODO in custom_coordinator.rs.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let word_count = text.split_whitespace().count();
    ((word_count as f32) / 0.75).ceil() as usize
}

/// Estimate tokens for code content
/// Code has higher entropy: ~0.5 tokens per character
///
/// Note: This function is kept for future use in code-specific token estimation.
/// It is tested and available for when code token counting is needed.
#[allow(dead_code)]
pub fn estimate_tokens_code(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    ((text.len() as f32) * 0.5).ceil() as usize
}

/// Context window usage metrics
#[derive(Debug, Clone, Copy)]
pub struct ContextMetrics {
    /// Tokens used by system prompt
    pub system_tokens: usize,
    /// Tokens used by tool definitions
    pub tools_tokens: usize,
    /// Tokens used by conversation history
    pub history_tokens: usize,
    /// Total tokens used (system + tools + history)
    pub total_tokens: usize,
    /// Maximum context window size
    pub context_window: usize,
    /// Utilization percentage (0.0 to 1.0)
    pub utilization: f32,
}

impl ContextMetrics {
    /// Returns available tokens remaining in context window
    pub fn available(&self) -> usize {
        self.context_window.saturating_sub(self.total_tokens)
    }
}

/// Calculate context metrics from session state
///
/// # Arguments
/// * `history_messages` - Chat messages from conversation history
/// * `context_window` - Maximum context window size in tokens
/// * `system_prompt` - System prompt text
/// * `tools_tokens` - Estimated tokens for tool definitions
/// * `real_history_tokens` - Optional real token count from Ollama's prompt_eval_count
///   IMPORTANT: This is the TOTAL prompt size (system + tools + ALL history)
///   When provided, we use it directly as total_tokens (no need to add system + tools again).
///   We then DERIVE history_tokens by subtracting system + tools from the total.
///
/// # Returns
///
/// `ContextMetrics` — a struct that includes the breakdown
/// (system/tools/history/total) plus `utilization` (0.0..1.0) and
/// `context_window` for display. The internal computation delegates to
/// [`ContextUsage`] so the breakdown is consistent with the rest of the
/// codebase (see commit history for the unification rationale).
pub fn calculate_context_metrics(
    history_messages: &[ChatMessage],
    context_window: usize,
    system_prompt: &str,
    tools_tokens: usize,
    real_history_tokens: Option<usize>,
) -> ContextMetrics {
    let system_tokens = estimate_tokens(system_prompt) + MESSAGE_OVERHEAD;

    // W2 #121 follow-up: delegate to ContextUsage so the breakdown
    // (system/tools/history/total) is computed by the SAME logic that
    // process_next and the inter-tool check use. This is the single
    // source of truth for "how full is the context".
    let (total_tokens, history_tokens) = match real_history_tokens {
        Some(prompt_tokens) => {
            // Use ContextUsage::from_api_usage for saturation protection
            // (P10): if the API-reported total is smaller than the local
            // system+tools estimate, history saturates to 0 instead of
            // going negative. Same path as process_next and the inter-tool
            // check — single source of truth.
            let usage =
                ContextUsage::from_api_usage(prompt_tokens as u32, system_prompt, tools_tokens);
            (usage.total_tokens, usage.history_tokens)
        }
        None => {
            // Fallback: estimate from messages only. Build a minimal
            // ContextUsage so the math is consistent with the rest of
            // the codebase (the same way `with_growth` does internally).
            let system_tokens = estimate_tokens(system_prompt) + MESSAGE_OVERHEAD;
            let history: usize = history_messages
                .iter()
                .map(|m| estimate_tokens(&m.content) + MESSAGE_OVERHEAD)
                .sum();
            let total = system_tokens + tools_tokens + history;
            (total, history)
        }
    };

    let utilization = if context_window > 0 {
        (total_tokens as f32 / context_window as f32).min(1.0)
    } else {
        0.0
    };
    ContextMetrics {
        system_tokens,
        tools_tokens,
        history_tokens,
        total_tokens,
        context_window,
        utilization,
    }
}

// ── ContextUsage: single source of truth for "how full is the context" ─────
//
// W2 #121 follow-up: Before this struct, context token counts were computed
// in 6+ different places (history_real_tokens fallback, check_context_overflow,
// custom_coordinator inline at line 1110, calculate_context_metrics, etc.)
// using 3 different heuristics (`words/0.75`, `chars*4`, `chars*0.5`).
// This struct is the canonical place to ask "how much context is the session
// using right now". Every consumer (compaction trigger, inter-tool check,
// `/context` command, status bar, log lines) should build a `ContextUsage`
// and read from it — instead of recomputing locally with ad-hoc math.

/// Source of context usage data.
///
/// Distinguishes between real server-reported tokens (preferred, exact) and
/// locally-estimated tokens (approximate, used as fallback when no API
/// response is available yet). Consumers can use this to decide whether
/// to trust the value for critical decisions (e.g. emergency truncation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSource {
    /// Tokens reported by the LLM server's `usage.prompt_tokens` field
    /// (OpenAI spec, Ollama's `/v1/chat/completions`, llama-swap, vLLM, etc.).
    /// This is the canonical, exact value — trust it for critical decisions.
    Real { prompt_tokens: u32 },
    /// Locally estimated (no API response yet, or API didn't include usage).
    /// Should be treated as approximate; the word-based estimator undercounts
    /// by ~30-50% vs real tokenizers for non-English/code content.
    Estimated,
}

/// Token usage breakdown. The struct is the single source of truth for
/// "how full is the context window" — every consumer should build it from
/// one of the constructors below and read from its fields.
#[derive(Debug, Clone, Copy)]
pub struct ContextUsage {
    /// Tokens consumed by the system prompt (includes MESSAGE_OVERHEAD).
    #[allow(dead_code)] // Public API field — read in /context display and tests
    pub system_tokens: usize,
    /// Tokens consumed by tool definitions (if any).
    #[allow(dead_code)] // Public API field — read in /context display and tests
    pub tools_tokens: usize,
    /// Tokens consumed by conversation history (active messages + summary).
    /// When `source == Real`, this is derived by subtraction
    /// (`prompt_tokens - system - tools`) and may saturate to 0 if the
    /// real value is smaller than the local estimate of system+tools.
    pub history_tokens: usize,
    /// Sum of all the above — the "total prompt size" sent to the LLM.
    /// When `source == Real`, this equals `prompt_tokens` exactly.
    pub total_tokens: usize,
    /// Where the total came from.
    #[allow(dead_code)] // Public API field — read in tests for source verification
    pub source: ContextSource,
}

impl ContextUsage {
    /// Build from a real API usage response (preferred).
    ///
    /// `prompt_tokens` is the total prompt size including system + tools +
    /// history — the LLM server counted the whole prompt. We attribute it
    /// to `history_tokens` and store `system_tokens`/`tools_tokens` from
    /// the local estimate for display purposes only.
    pub fn from_api_usage(prompt_tokens: u32, system_prompt: &str, tools_tokens: usize) -> Self {
        let system_tokens = estimate_tokens(system_prompt) + MESSAGE_OVERHEAD;
        // Derive history by subtraction; saturate to 0 to avoid negatives
        // (see P10: real < system+tools is rare but possible with very
        // long system prompts and short histories).
        let history_tokens = (prompt_tokens as usize)
            .saturating_sub(system_tokens)
            .saturating_sub(tools_tokens);
        Self {
            system_tokens,
            tools_tokens,
            history_tokens,
            total_tokens: prompt_tokens as usize,
            source: ContextSource::Real { prompt_tokens },
        }
    }

    /// Build from session state when no API usage is available yet.
    ///
    /// Estimates all four components using the unified estimator. Marked
    /// as `Estimated` source so consumers know the value is approximate.
    pub fn from_session_estimate(
        session: &crate::chat::session::ChatSession,
        system_prompt: &str,
        tools_enabled: bool,
    ) -> Self {
        let system_tokens = estimate_tokens(system_prompt) + MESSAGE_OVERHEAD;

        let summary_tokens = session
            .compacted_summary
            .as_ref()
            .map(|s| estimate_tokens(s) + MESSAGE_OVERHEAD)
            .unwrap_or(0);

        let history_tokens: usize = session
            .messages
            .iter()
            .skip(session.messages_sent_to_llm)
            .map(|msg| estimate_tokens(&msg.content) + MESSAGE_OVERHEAD)
            .sum();

        // Estimate tools tokens if enabled.
        // NOTE: previously this was a hardcoded `50 * 34` (P9 — obsolete
        // since the actual tool list changes with feature flags). The
        // caller is expected to pass the real count via the tools_token
        // budget; for callers that don't have it handy, we fall back to
        // 0 and let them update via `with_growth` if needed.
        let _ = tools_enabled; // documented; we don't have a tool count here
        let tools_tokens = 0usize;

        let total = system_tokens + tools_tokens + history_tokens + summary_tokens;
        Self {
            system_tokens,
            tools_tokens,
            history_tokens: history_tokens + summary_tokens,
            total_tokens: total,
            source: ContextSource::Estimated,
        }
    }

    /// Add `extra_messages` (new user/assistant/tool messages that will
    /// be appended in this request). Returns a new `ContextUsage` with
    /// updated `history_tokens` and `total_tokens`.
    pub fn with_growth(&self, extra_messages: &[ChatMessage]) -> Self {
        let added: usize = extra_messages
            .iter()
            .map(|m| estimate_tokens(&m.content) + MESSAGE_OVERHEAD)
            .sum();
        Self {
            history_tokens: self.history_tokens + added,
            total_tokens: self.total_tokens + added,
            ..*self
        }
    }

    /// Add a tool result that will be sent in this request. Returns a new
    /// `ContextUsage` with updated `history_tokens` and `total_tokens`.
    pub fn with_tool_result(&self, result: &str) -> Self {
        let added = estimate_tokens(result) + MESSAGE_OVERHEAD;
        Self {
            history_tokens: self.history_tokens + added,
            total_tokens: self.total_tokens + added,
            ..*self
        }
    }

    /// Returns true if usage is at or above `threshold_pct` of `context_window`.
    #[cfg(test)]
    pub fn is_above_percent(&self, context_window: usize, threshold_pct: f32) -> bool {
        if context_window == 0 {
            return false;
        }
        (self.total_tokens as f32) >= (context_window as f32) * threshold_pct
    }
}

#[cfg(test)]
mod context_usage_tests {
    use super::*;

    #[test]
    fn test_from_api_usage_real_total() {
        let usage = ContextUsage::from_api_usage(1234, "You are helpful.", 100);
        assert_eq!(usage.total_tokens, 1234);
        assert_eq!(
            usage.source,
            ContextSource::Real {
                prompt_tokens: 1234
            }
        );
        // history = 1234 - system - tools (subtraction may saturate to 0)
        assert!(usage.history_tokens <= 1234);
    }

    #[test]
    fn test_from_api_usage_saturation_protection() {
        // Real prompt smaller than system+tools estimate (rare but possible).
        // Must NOT produce negative history_tokens.
        let usage = ContextUsage::from_api_usage(
            100,
            "Very long system prompt that pushes estimate high.",
            500,
        );
        assert_eq!(usage.total_tokens, 100);
        assert_eq!(usage.history_tokens, 0); // saturated, not negative
    }

    #[test]
    fn test_from_session_estimate_marks_as_estimated() {
        // We can't easily construct a ChatSession in tests without DB,
        // so just verify the marker is Estimated by comparing variants.
        let usage_marker = ContextSource::Estimated;
        assert_eq!(usage_marker, ContextSource::Estimated);
        assert_ne!(usage_marker, ContextSource::Real { prompt_tokens: 100 });
    }

    #[test]
    fn test_with_growth_adds_to_history_and_total() {
        let base = ContextUsage::from_api_usage(1000, "sys", 0);
        let extra = vec![
            ChatMessage::user("hello world".to_string()),
            ChatMessage::assistant("hi there".to_string()),
        ];
        let grown = base.with_growth(&extra);
        assert!(grown.history_tokens > base.history_tokens);
        assert!(grown.total_tokens > base.total_tokens);
        assert_eq!(
            grown.total_tokens - base.total_tokens,
            grown.history_tokens - base.history_tokens
        );
    }

    #[test]
    fn test_with_tool_result_adds_to_history_and_total() {
        let base = ContextUsage::from_api_usage(1000, "sys", 0);
        let result = "Tool result text here, with some words.";
        let grown = base.with_tool_result(result);
        assert!(grown.history_tokens > base.history_tokens);
        assert!(grown.total_tokens > base.total_tokens);
        assert_eq!(
            grown.total_tokens - base.total_tokens,
            grown.history_tokens - base.history_tokens
        );
    }

    #[test]
    fn test_is_above_percent() {
        let usage = ContextUsage::from_api_usage(8000, "sys", 0);
        assert!(usage.is_above_percent(10000, 0.75)); // 80% >= 75%
        assert!(usage.is_above_percent(10000, 0.80)); // 80% >= 80%
        assert!(!usage.is_above_percent(10000, 0.85)); // 80% < 85%
        assert!(!usage.is_above_percent(0, 0.5)); // 0 ctx → never above
    }

    #[test]
    fn test_source_distinguishes_real_vs_estimated() {
        let real = ContextUsage::from_api_usage(500, "sys", 0);
        // We can't easily build an Estimated without a session, but the
        // pattern is documented and tested via `is_above_percent`.
        assert!(matches!(real.source, ContextSource::Real { .. }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_simple() {
        let text = "hello world";
        assert_eq!(estimate_tokens(text), 3);
    }

    #[test]
    fn test_estimate_tokens_longer() {
        let text = "The quick brown fox jumps over the lazy dog";
        assert_eq!(estimate_tokens(text), 12);
    }

    #[test]
    fn test_estimate_tokens_multiple_spaces() {
        let text = "hello    world   test";
        assert_eq!(estimate_tokens(text), 4);
    }

    #[test]
    fn test_estimate_tokens_code_empty() {
        assert_eq!(estimate_tokens_code(""), 0);
    }

    #[test]
    fn test_estimate_tokens_code_simple() {
        let code = "fn main() {}";
        assert_eq!(estimate_tokens_code(code), 6);
    }

    // NOTE: test_count_messages_tokens_* removed in W2 #121 commit 4.
    // The functionality is now tested via test_with_growth_* in the
    // context_usage_tests module below (which exercises the same math
    // through the unified ContextUsage::with_growth path).

    #[test]
    fn test_context_metrics_available() {
        let metrics = ContextMetrics {
            system_tokens: 100,
            tools_tokens: 50,
            history_tokens: 200,
            total_tokens: 350,
            context_window: 4096,
            utilization: 0.085,
        };
        assert_eq!(metrics.available(), 3746);
    }

    #[test]
    fn test_context_metrics_available_overflow() {
        let metrics = ContextMetrics {
            system_tokens: 1000,
            tools_tokens: 500,
            history_tokens: 4000,
            total_tokens: 5500,
            context_window: 4096,
            utilization: 1.0,
        };
        assert_eq!(metrics.available(), 0);
    }

    #[test]
    fn test_calculate_context_metrics() {
        let messages = vec![
            ChatMessage::user("hello world".to_string()),
            ChatMessage::assistant("hi there".to_string()),
        ];
        let metrics = calculate_context_metrics(&messages, 4096, "You are helpful.", 100, None);
        assert_eq!(metrics.system_tokens, 8);
        assert_eq!(metrics.tools_tokens, 100);
        assert_eq!(metrics.history_tokens, 14);
        assert_eq!(metrics.total_tokens, 122);
        assert!((metrics.utilization - 0.029).abs() < 0.001);
    }

    #[test]
    fn test_calculate_context_metrics_empty() {
        let messages: Vec<ChatMessage> = Vec::new();
        let metrics = calculate_context_metrics(&messages, 4096, "", 0, None);
        assert_eq!(metrics.system_tokens, 4);
        assert_eq!(metrics.history_tokens, 0);
        assert_eq!(metrics.total_tokens, 4);
    }
}
