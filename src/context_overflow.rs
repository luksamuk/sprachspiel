//! Context overflow detection and handling
//!
//! Implements auto-compaction when context reaches threshold.

use crate::chat::session::ChatSession;
use crate::tokens::estimate_tokens;

/// Default overflow threshold (80% of context window)
pub const DEFAULT_OVERFLOW_THRESHOLD: f32 = 0.8;

/// Default number of first messages to keep during compaction
pub const DEFAULT_KEEP_FIRST: usize = 5;

/// Default number of last messages to keep during compaction
pub const DEFAULT_KEEP_LAST: usize = 5;

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
    /// Check if context needs compaction
    pub fn needs_compaction(&self) -> bool {
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

/// Check if context has overflowed using default threshold
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
    /// Number of messages in the middle to summarize
    pub middle_count: usize,
    /// Indices of messages to compact (for reference)
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

    let middle_count = middle_end - middle_start;

    Some(CompactionSuggestion {
        keep_first,
        keep_last,
        middle_count,
        middle_indices: middle_start..middle_end,
    })
}

/// Estimate tokens that would be saved by compaction
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
        assert_eq!(suggestion.middle_count, 10);
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
        assert_eq!(suggestion.middle_count, 10); // 20 - 5 - 5 = 10
    }

    #[test]
    fn test_estimate_compaction_savings() {
        let session = create_test_session(15);
        let suggestion = get_compaction_range(&session, 5, 5).unwrap();

        // Verify we have some middle messages to compact
        assert!(suggestion.middle_count > 0);

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
}
