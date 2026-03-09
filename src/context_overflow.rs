//! Context overflow detection and handling
//!
//! Implements auto-compaction when context reaches threshold.

use crate::chat::session::ChatSession;
use crate::tokens::{estimate_tokens, MESSAGE_OVERHEAD};
use ollama_rs::generation::chat::ChatMessage;

/// Default overflow threshold (80% of context window)
pub const DEFAULT_OVERFLOW_THRESHOLD: f32 = 0.8;

/// Default number of first messages to keep during compaction
pub const DEFAULT_KEEP_FIRST: usize = 5;

/// Default number of last messages to keep during compaction
pub const DEFAULT_KEEP_LAST: usize = 5;

/// Estimate tokens in a list of SavedMessage
/// Includes message overhead for each message
pub fn estimate_messages_tokens(messages: &[crate::chat::session::SavedMessage]) -> usize {
    if messages.is_empty() {
        return 0;
    }
    messages
        .iter()
        .map(|msg| MESSAGE_OVERHEAD + estimate_tokens(&msg.content))
        .sum()
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
}

/// Check if context has overflowed the threshold
pub fn check_context_overflow(
    session: &ChatSession,
    system_prompt: &str,
    context_window: usize,
    threshold: f32,
) -> ContextStatus {
    // Calculate total tokens
    let system_tokens = estimate_tokens(system_prompt) + 4; // +4 for message overhead
    let history_tokens: usize = session
        .messages
        .iter()
        .map(|msg| estimate_tokens(&msg.content) + 4)
        .sum();

    let summary_tokens = session
        .compacted_summary
        .as_ref()
        .map(|s| estimate_tokens(s) + 4)
        .unwrap_or(0);

    let total_tokens = system_tokens + history_tokens + summary_tokens;
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
    fn test_estimate_messages_tokens() {
        // Empty messages
        let empty: Vec<SavedMessage> = Vec::new();
        assert_eq!(estimate_messages_tokens(&empty), 0);

        // Single message
        let single = vec![SavedMessage {
            role: MessageRole::User,
            content: "Hello world".to_string(), // ~2 tokens + 4 overhead = 6
            timestamp: Utc::now(),
            ..Default::default()
        }];
        let single_tokens = estimate_messages_tokens(&single);
        assert!(single_tokens >= 4, "Should have at least overhead");
        assert!(single_tokens < 20, "Should be small for short message");

        // Multiple messages
        let multiple: Vec<SavedMessage> = (0..10)
            .map(|i| SavedMessage {
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("Message {}", i),
                timestamp: Utc::now(),
                ..Default::default()
            })
            .collect();
        let multiple_tokens = estimate_messages_tokens(&multiple);
        assert!(
            multiple_tokens > single_tokens,
            "More messages should have more tokens"
        );

        // Each message should contribute roughly same amount
        // 10 messages with ~2 tokens each + 4 overhead = ~60 tokens
        assert!(
            multiple_tokens > 40,
            "10 short messages should have ~60 tokens"
        );
        assert!(
            multiple_tokens < 100,
            "10 short messages should have ~60 tokens"
        );
    }

    #[test]
    fn test_estimate_messages_tokens_with_long_content() {
        // Long message content
        let long_content = "word ".repeat(100); // 100 words
        let long_message = vec![SavedMessage {
            role: MessageRole::User,
            content: long_content.clone(),
            timestamp: Utc::now(),
            ..Default::default()
        }];

        let long_tokens = estimate_messages_tokens(&long_message);
        // 100 words / 0.75 = ~133 tokens + 4 overhead = ~137
        assert!(long_tokens > 100, "Should estimate more than 100 tokens");
        assert!(long_tokens < 200, "Should estimate less than 200 tokens");
    }
}
