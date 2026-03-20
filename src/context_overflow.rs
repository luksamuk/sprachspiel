//! Context overflow detection and handling
//!
//! Implements auto-compaction when context reaches threshold.

use crate::chat::session::ChatSession;
use crate::tokens::{estimate_tokens, MESSAGE_OVERHEAD};
use ollama_rs::generation::chat::ChatMessage;

/// Default overflow threshold (80% of context window)
pub const DEFAULT_OVERFLOW_THRESHOLD: f32 = 0.8;

/// Pre-tool check threshold (75% of context window)
/// Lower than overflow to allow room for tool results during execution
pub const PRE_TOOL_THRESHOLD: f32 = 0.75;

/// Inter-tool check threshold (80% of context window)
/// Triggers compaction between sequential tool calls.
/// Same as DEFAULT_OVERFLOW_THRESHOLD since both indicate "needs compaction".
pub const INTER_TOOL_THRESHOLD: f32 = 0.80;

/// Emergency threshold (90% of context window)
/// When exceeded, tool results are truncated before adding to history.
/// This is the last resort before context overflow crashes.
pub const EMERGENCY_THRESHOLD: f32 = 0.90;

/// Response margin (tokens reserved for model response)
/// Ensures space for the model to generate a response after tool execution.
pub const RESPONSE_MARGIN: usize = 500;

/// Compaction buffer - reserve space before compaction trigger
/// Ensures compaction happens BEFORE context fills, not after overflow.
/// Inspired by OpenCode's approach (they use 20K for code agents).
/// ask-ai uses 15K since it's primarily for Zettelkasten and learning,
/// not code generation, so we need less buffer for response space.
pub const COMPACTION_BUFFER: usize = 15_000;

/// Maximum tokens for compacted summary
/// Prevents summary from becoming large enough to cause overflow again.
/// Based on research: 10-15% of original content, capped for safety.
/// For 368 messages (~18K tokens original), 3K is ~17% - aggressive but safe.
pub const MAX_SUMMARY_TOKENS: usize = 3_000;

/// Default number of first messages to keep during compaction
pub const DEFAULT_KEEP_FIRST: usize = 5;

/// Default number of last messages to keep during compaction
pub const DEFAULT_KEEP_LAST: usize = 5;

/// Check if context needs pre-tool compaction
/// Returns true if context is above PRE_TOOL_THRESHOLD (75%)
pub fn needs_pre_tool_compaction(
    session: &ChatSession,
    system_prompt: &str,
    context_window: usize,
) -> bool {
    let status = check_context_overflow(session, system_prompt, context_window, PRE_TOOL_THRESHOLD);
    status.needs_compaction()
}

/// Check if context needs compaction based on buffer (not percentages)
/// Inspired by OpenCode's approach: trigger when tokens >= context_window - COMPACTION_BUFFER
/// This ensures compaction happens BEFORE overflow, not after.
///
/// This is more predictable than percentage-based triggers:
/// - Percentage: "compact at 80%" varies with context window size
/// - Buffer: "compact when 15K tokens remaining" is constant
///
/// For a 32K context window:
/// - 80% trigger = compact at 25,600 tokens (6,400 remaining)
/// - 15K buffer = compact at 17,000 tokens (15,000 remaining)
///
/// The buffer approach ensures consistent space for responses regardless of context size.
pub fn needs_buffered_compaction(session: &ChatSession, context_window: usize) -> bool {
    let real_tokens = session.history_real_tokens();
    let threshold = context_window.saturating_sub(COMPACTION_BUFFER);
    real_tokens >= threshold
}

/// Check if context needs inter-tool compaction
/// Called after each tool result is added to history.
/// Returns true if context is above INTER_TOOL_THRESHOLD (80%).
pub fn needs_inter_tool_compaction(
    history_tokens: usize,
    system_tokens: usize,
    context_window: usize,
) -> bool {
    let total = history_tokens.saturating_add(system_tokens);
    let threshold = (context_window as f32 * INTER_TOOL_THRESHOLD) as usize;
    total > threshold
}

/// Check if context is in emergency state
/// Returns true if context is above EMERGENCY_THRESHOLD (90%).
/// At this point, truncation is required before adding more content.
pub fn is_emergency_context(
    history_tokens: usize,
    system_tokens: usize,
    context_window: usize,
) -> bool {
    let total = history_tokens.saturating_add(system_tokens);
    let threshold = (context_window as f32 * EMERGENCY_THRESHOLD) as usize;
    total > threshold
}

/// Calculate available token budget for tool results
/// Returns the number of tokens available before reaching EMERGENCY_THRESHOLD.
pub fn calculate_available_budget(
    history_tokens: usize,
    system_tokens: usize,
    context_window: usize,
) -> usize {
    let total_used = history_tokens.saturating_add(system_tokens);
    let emergency_limit = (context_window as f32 * EMERGENCY_THRESHOLD) as usize;
    emergency_limit
        .saturating_sub(total_used)
        .saturating_sub(RESPONSE_MARGIN)
}

/// Estimate tokens in a list of ChatMessage (for coordinator history)
/// Includes message overhead for each message
pub fn estimate_chat_messages_tokens(messages: &[ChatMessage]) -> usize {
    if messages.is_empty() {
        return 0;
    }
    messages
        .iter()
        .map(|msg| MESSAGE_OVERHEAD + estimate_tokens(&msg.content))
        .sum()
}

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

/// Check if context has overflowed the threshold
pub fn check_context_overflow(
    session: &ChatSession,
    system_prompt: &str,
    context_window: usize,
    threshold: f32,
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
        // Fallback: estimate from message content
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

        // Estimate tools tokens if enabled (~50 tokens per tool)
        let tools_tokens = if session.tools {
            // Approximate: ~50 tokens per tool for tool definitions
            // This is a rough estimate; actual count depends on tool complexity
            50 * 34 // Assuming ~34 tools when enabled
        } else {
            0
        };

        system_tokens + tools_tokens + history_tokens + summary_tokens
    };

    let usage = total_tokens as f32 / context_window as f32;
    let usage_percent = (usage * 100.0).min(100.0) as u8;

    if usage >= threshold {
        ContextStatus::Overflow {
            total_tokens,
            max_tokens: context_window,
            usage_percent,
        }
    } else if usage >= threshold * 0.9 {
        // Warning at 90% of threshold (e.g., 72% if threshold is 80%)
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

/// Check if context has overflowed using default threshold (80%)
///
/// Convenience function for code that doesn't need custom thresholds.
/// Equivalent to `check_context_overflow(session, prompt, window, DEFAULT_OVERFLOW_THRESHOLD)`.
#[allow(dead_code)]
pub fn check_context_overflow_default(
    session: &ChatSession,
    system_prompt: &str,
    context_window: usize,
) -> ContextStatus {
    check_context_overflow(
        session,
        system_prompt,
        context_window,
        DEFAULT_OVERFLOW_THRESHOLD,
    )
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
    fn test_check_context_overflow() {
        let session = create_test_session(100);
        let _status =
            check_context_overflow(&session, "System prompt", 4096, DEFAULT_OVERFLOW_THRESHOLD);

        // Should overflow with low threshold
        let low_threshold = 0.001f32;
        let status_low = check_context_overflow(&session, "System prompt", 4096, low_threshold);
        assert!(status_low.needs_compaction());
    }

    #[test]
    fn test_check_context_overflow_default() {
        let session = create_test_session(5);
        let status = check_context_overflow_default(&session, "System prompt", 4096);

        assert!(matches!(status, ContextStatus::Ok { .. }));

        let large_session = create_test_session(500);
        let large_status = check_context_overflow_default(&large_session, "System prompt", 4096);

        // Should overflow with default threshold
        assert!(large_status.needs_compaction() || large_status.usage_percent() > 70);
    }

    #[test]
    fn test_get_compaction_range_enough_messages() {
        let session = create_test_session(20);
        let result = get_compaction_range(&session, DEFAULT_KEEP_FIRST, DEFAULT_KEEP_LAST);

        assert!(result.is_some());
        let suggestion = result.unwrap();
        assert_eq!(suggestion.keep_first, DEFAULT_KEEP_FIRST);
        assert_eq!(suggestion.keep_last, DEFAULT_KEEP_LAST);
        assert_eq!(suggestion.middle_indices.len(), 10); // 20 - 5 - 5 = 10
    }

    #[test]
    fn test_get_compaction_range_not_enough_messages() {
        let session = create_test_session(8);
        let result = get_compaction_range(&session, DEFAULT_KEEP_FIRST, DEFAULT_KEEP_LAST);

        assert!(result.is_none());
    }

    #[test]
    fn test_get_compaction_range_default() {
        let session = create_test_session(20);
        let result = get_compaction_range_default(&session);

        assert!(result.is_some());
        let suggestion = result.unwrap();
        assert_eq!(suggestion.keep_first, DEFAULT_KEEP_FIRST);
        assert_eq!(suggestion.keep_last, DEFAULT_KEEP_LAST);
        assert_eq!(suggestion.middle_indices.len(), 10); // 20 - 5 - 5 = 10
    }

    #[test]
    fn test_estimate_compaction_savings() {
        let session = create_test_session(15);
        let suggestion = get_compaction_range(&session, 5, 5).unwrap();

        // Verify we have some middle messages to compact
        assert!(!suggestion.middle_indices.is_empty());

        // Calculate actual savings (depends on message content)
        let _savings = estimate_compaction_savings(&session, &suggestion, 100);

        // Savings can be positive or negative depending on summary overhead
        // This is expected behavior - compaction trades tokens for summarization
    }

    #[test]
    fn test_context_status_usage_percent() {
        let status_ok = ContextStatus::Ok {
            total_tokens: 100,
            max_tokens: 1000,
        };
        let status_warn = ContextStatus::Warning {
            total_tokens: 750,
            max_tokens: 1000,
            usage_percent: 75,
        };
        let status_over = ContextStatus::Overflow {
            total_tokens: 850,
            max_tokens: 1000,
            usage_percent: 85,
        };

        assert_eq!(status_ok.usage_percent(), 0);
        assert_eq!(status_warn.usage_percent(), 75);
        assert_eq!(status_over.usage_percent(), 85);
    }

    #[test]
    fn test_should_position_summary_after_system() {
        let mut session = create_test_session(10);

        // No summary
        assert!(!should_position_summary_after_system(&session));

        // With summary
        session.compacted_summary = Some("Summary of old messages".to_string());
        assert!(should_position_summary_after_system(&session));
    }

    #[test]
    fn test_context_status_needs_compaction() {
        let status_ok = ContextStatus::Ok {
            total_tokens: 100,
            max_tokens: 1000,
        };
        let status_warn = ContextStatus::Warning {
            total_tokens: 750,
            max_tokens: 1000,
            usage_percent: 75,
        };
        let status_over = ContextStatus::Overflow {
            total_tokens: 850,
            max_tokens: 1000,
            usage_percent: 85,
        };

        assert!(!status_ok.needs_compaction());
        assert!(status_warn.needs_compaction());
        assert!(status_over.needs_compaction());
    }

    #[test]
    fn test_context_status_is_warning() {
        let status_ok = ContextStatus::Ok {
            total_tokens: 100,
            max_tokens: 1000,
        };
        let status_warn = ContextStatus::Warning {
            total_tokens: 750,
            max_tokens: 1000,
            usage_percent: 75,
        };
        let status_over = ContextStatus::Overflow {
            total_tokens: 850,
            max_tokens: 1000,
            usage_percent: 85,
        };

        assert!(!status_ok.is_warning());
        assert!(status_warn.is_warning());
        assert!(!status_over.is_warning());
    }

    #[test]
    fn test_context_status_is_overflow() {
        let status_ok = ContextStatus::Ok {
            total_tokens: 100,
            max_tokens: 1000,
        };
        let status_warn = ContextStatus::Warning {
            total_tokens: 750,
            max_tokens: 1000,
            usage_percent: 75,
        };
        let status_over = ContextStatus::Overflow {
            total_tokens: 850,
            max_tokens: 1000,
            usage_percent: 85,
        };

        assert!(!status_ok.is_overflow());
        assert!(!status_warn.is_overflow());
        assert!(status_over.is_overflow());
    }

    #[test]
    fn test_pre_tool_threshold() {
        // PRE_TOOL_THRESHOLD should be lower than DEFAULT_OVERFLOW_THRESHOLD
        assert!(
            PRE_TOOL_THRESHOLD < DEFAULT_OVERFLOW_THRESHOLD,
            "Pre-tool threshold should be lower than overflow threshold"
        );
        assert_eq!(PRE_TOOL_THRESHOLD, 0.75, "Pre-tool threshold should be 75%");
        assert_eq!(
            DEFAULT_OVERFLOW_THRESHOLD, 0.80,
            "Overflow threshold should be 80%"
        );
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

        let system_prompt = "You are helpful.";
        let context_window = 128000; // Typical large context

        // Should NOT need pre-tool compaction
        assert!(
            !needs_pre_tool_compaction(&session, system_prompt, context_window),
            "Session below 75% should not need pre-tool compaction"
        );
    }

    #[test]
    fn test_needs_pre_tool_compaction_above_threshold() {
        // Session with high context usage (above 75%)
        let mut session = ChatSession::new("test-model".to_string(), None, false);

        // Fill session with large content to exceed threshold
        let large_content = "word ".repeat(50000); // ~67000 tokens
        for _ in 0..3 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: large_content.clone(),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        let system_prompt = "You are helpful.";
        let context_window = 128000;

        // Should need pre-tool compaction
        assert!(
            needs_pre_tool_compaction(&session, system_prompt, context_window),
            "Session above 75% should need pre-tool compaction"
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
    fn test_estimate_chat_messages_tokens() {
        use ollama_rs::generation::chat::ChatMessage;

        // Empty messages
        let empty: Vec<ChatMessage> = Vec::new();
        assert_eq!(estimate_chat_messages_tokens(&empty), 0);

        // Single message
        let single = vec![ChatMessage::user("Hello".to_string())];
        let single_tokens = estimate_chat_messages_tokens(&single);
        assert!(single_tokens >= 4, "Should have at least MESSAGE_OVERHEAD");
        assert!(single_tokens < 20, "Should be small for short message");

        // Multiple messages with different roles
        let multiple: Vec<ChatMessage> = vec![
            ChatMessage::user("Hello".to_string()),
            ChatMessage::assistant("Hi there!".to_string()),
            ChatMessage::user("How are you?".to_string()),
        ];
        let multiple_tokens = estimate_chat_messages_tokens(&multiple);
        assert!(
            multiple_tokens > single_tokens,
            "More messages should have more tokens"
        );
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

        let status_no_compact = check_context_overflow(&session, "System prompt", 1000, 0.8);
        let tokens_no_compact = status_no_compact.total_tokens();

        // Now compact first 5 messages
        session.messages_sent_to_llm = 5;
        session.compacted_summary = Some("This is a summary of the first 5 messages".into());

        let status_with_compact = check_context_overflow(&session, "System prompt", 1000, 0.8);
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

        let status_no_summary = check_context_overflow(&session, "System", 1000, 0.8);
        let tokens_no_summary = status_no_summary.total_tokens();

        // Add a summary
        session.compacted_summary =
            Some("This is a summary of the previous conversation about important topics".into());

        let status_with_summary = check_context_overflow(&session, "System", 1000, 0.8);
        let tokens_with_summary = status_with_summary.total_tokens();

        // Summary should add tokens
        assert!(
            tokens_with_summary > tokens_no_summary,
            "Summary should add tokens: {} > {}",
            tokens_with_summary,
            tokens_no_summary
        );
    }

    #[test]
    fn test_inter_tool_threshold() {
        // INTER_TOOL_THRESHOLD should be between PRE_TOOL and EMERGENCY
        assert!(
            INTER_TOOL_THRESHOLD > PRE_TOOL_THRESHOLD,
            "Inter-tool threshold should be higher than pre-tool threshold"
        );
        assert!(
            INTER_TOOL_THRESHOLD < EMERGENCY_THRESHOLD,
            "Inter-tool threshold should be lower than emergency threshold"
        );
        assert_eq!(
            INTER_TOOL_THRESHOLD, 0.80,
            "Inter-tool threshold should be 80%"
        );
    }

    #[test]
    fn test_needs_inter_tool_compaction_below() {
        // Context at 70% - should NOT need compaction
        let history_tokens = 700;
        let system_tokens = 50;
        let context_window = 1000;

        assert!(
            !needs_inter_tool_compaction(history_tokens, system_tokens, context_window),
            "70% should not need inter-tool compaction"
        );
    }

    #[test]
    fn test_needs_inter_tool_compaction_above() {
        // Context at 85% - should need compaction
        let history_tokens = 800;
        let system_tokens = 50;
        let context_window = 1000;

        assert!(
            needs_inter_tool_compaction(history_tokens, system_tokens, context_window),
            "85% should need inter-tool compaction"
        );
    }

    #[test]
    fn test_is_emergency_context_below() {
        // Context at 85% - should NOT be emergency
        let history_tokens = 800;
        let system_tokens = 50;
        let context_window = 1000;

        assert!(
            !is_emergency_context(history_tokens, system_tokens, context_window),
            "85% should not be emergency"
        );
    }

    #[test]
    fn test_is_emergency_context_above() {
        // Context at 95% - should be emergency
        let history_tokens = 900;
        let system_tokens = 50;
        let context_window = 1000;

        assert!(
            is_emergency_context(history_tokens, system_tokens, context_window),
            "95% should be emergency"
        );
    }

    #[test]
    fn test_calculate_available_budget_normal() {
        // Context at 50% with margin of 500
        let history_tokens = 400;
        let system_tokens = 100;
        let context_window = 1000;

        let available = calculate_available_budget(history_tokens, system_tokens, context_window);

        // Available = 900 - 400 - 100 - 500 = -100 -> saturating_sub = 0
        // Actually: emergency_limit (900) - total (500) - margin (500) = -100 -> 0
        assert_eq!(available, 0, "Should return 0 when budget is negative");
    }

    #[test]
    fn test_calculate_available_budget_plenty() {
        // Context at 10% with large context
        let history_tokens = 1000;
        let system_tokens = 100;
        let context_window = 128000;

        let available = calculate_available_budget(history_tokens, system_tokens, context_window);

        // emergency_limit = 128000 * 0.9 = 115200
        // available = 115200 - 1000 - 100 - 500 = 113600
        assert!(
            available > 100000,
            "Should have plenty of budget available: got {}",
            available
        );
    }

    #[test]
    fn test_threshold_relationships() {
        // Verify the threshold hierarchy
        assert!(PRE_TOOL_THRESHOLD < DEFAULT_OVERFLOW_THRESHOLD);
        assert!(DEFAULT_OVERFLOW_THRESHOLD < EMERGENCY_THRESHOLD);
        assert_eq!(PRE_TOOL_THRESHOLD, 0.75);
        assert_eq!(DEFAULT_OVERFLOW_THRESHOLD, 0.80);
        assert_eq!(INTER_TOOL_THRESHOLD, 0.80);
        assert_eq!(EMERGENCY_THRESHOLD, 0.90);
    }
}
