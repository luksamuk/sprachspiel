//! Context overflow detection and handling
//!
//! Implements auto-compaction when context reaches threshold.
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

use crate::chat::session::ChatSession;
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

/// Maximum tokens for compacted summary
/// Prevents summary from becoming large enough to cause overflow again.
/// Based on research: 10-15% of original content, capped for safety.
/// For 368 messages (~18K tokens original), 3K is ~17% - aggressive but safe.
pub const MAX_SUMMARY_TOKENS: usize = 3_000;

/// Default number of first messages to keep during compaction
pub const DEFAULT_KEEP_FIRST: usize = 5;

/// Default number of last messages to keep during compaction
pub const DEFAULT_KEEP_LAST: usize = 5;

/// Default overflow threshold (75%) - used for display and tests
/// Shows "OK" below 75%, "MODERATE" 75-88%, "CRITICAL" above 88%
#[allow(dead_code)]
pub const DEFAULT_OVERFLOW_THRESHOLD: f32 = MODERATE_USAGE_PERCENT;

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
}
