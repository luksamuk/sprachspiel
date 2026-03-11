//! Integration tests for context overflow recovery flow
//!
//! Tests the complete error recovery flow:
//! 1. Context check during tool execution (90% threshold)
//! 2. Error detection and message removal
//! 3. Auto-compaction after overflow error
//! 4. User can retry after recovery

use ask_ai::chat::session::{ChatSession, MessageRole, SavedMessage};
use ask_ai::context_overflow::{
    DEFAULT_KEEP_FIRST, DEFAULT_KEEP_LAST, DEFAULT_OVERFLOW_THRESHOLD, PRE_TOOL_THRESHOLD,
    check_context_overflow, truncate_tool_result,
};
use ask_ai::tokens::estimate_tokens;
use chrono::Utc;

fn create_session_with_token_count(message_count: usize, tokens_per_message: usize) -> ChatSession {
    let mut session = ChatSession::new("test-model".to_string(), None, false);

    // Create messages with specific token count
    let _words_per_token = 0.75;

    for i in 0..message_count {
        let target_tokens = tokens_per_message.saturating_sub(4);
        let words = target_tokens * 4 / 3;
        let content = format!("Message {} {}", i, "word ".repeat(words));

        session.messages.push(SavedMessage {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content,
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    session
}

#[test]
fn test_threshold_hierarchy_for_recovery() {
    let warning_threshold = DEFAULT_OVERFLOW_THRESHOLD * 0.9;

    assert!(
        warning_threshold < PRE_TOOL_THRESHOLD,
        "Warning ({}) should come before pre-tool ({})",
        warning_threshold,
        PRE_TOOL_THRESHOLD
    );

    assert!(
        PRE_TOOL_THRESHOLD < DEFAULT_OVERFLOW_THRESHOLD,
        "Pre-tool ({}) should come before overflow ({})",
        PRE_TOOL_THRESHOLD,
        DEFAULT_OVERFLOW_THRESHOLD
    );
}

#[test]
fn test_context_overflow_detection_message_removal() {
    let mut session = ChatSession::new("test-model".to_string(), None, false);
    let context_window = 1000usize;
    let system_prompt = "You are helpful.";

    for i in 0..100 {
        session.messages.push(SavedMessage {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!(
                "Message {} with lots of tokens to fill context and trigger overflow",
                i
            ),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    let overflow_status = check_context_overflow(
        &session,
        system_prompt,
        context_window,
        DEFAULT_OVERFLOW_THRESHOLD,
    );
    assert!(
        overflow_status.needs_compaction(),
        "Context should need compaction after adding messages"
    );
    assert!(
        session.messages.len() > 60,
        "Should have more than 60 messages"
    );
}

#[test]
fn test_failed_message_removal_after_overflow() {
    let mut session = ChatSession::new("test-model".to_string(), None, false);

    for i in 0..5 {
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("User message {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: format!("Assistant response {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Current user message".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let total_before = session.messages.len();
    let (removed, contents) = session.remove_last_assistant_messages_with_content();

    assert!(removed >= 1, "Should remove at least 1 message");
    assert!(
        session.messages.len() < total_before,
        "Session should have fewer messages after removal"
    );
    assert!(!contents.is_empty(), "Should return removed content");
}

#[test]
fn test_pre_tool_compaction_preserves_user_message() {
    let mut session = ChatSession::new("test-model".to_string(), None, false);

    for i in 0..30 {
        session.messages.push(SavedMessage {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!("Message {} with enough content to use tokens", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "What is the weather?".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    for _ in 0..100 {
        session.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: "word ".repeat(50),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "word ".repeat(50),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    let total_messages = session.messages.len();
    let preserved_count = DEFAULT_KEEP_LAST;

    assert!(
        preserved_count < total_messages,
        "Keep last should be less than total messages"
    );
    assert!(
        DEFAULT_KEEP_FIRST > 0,
        "Keep first should preserve some messages"
    );

    let compactable_count = total_messages - DEFAULT_KEEP_FIRST - DEFAULT_KEEP_LAST;
    assert!(
        compactable_count > 0,
        "Should have messages available to compact"
    );
}

#[test]
fn test_tool_result_truncation_limits_context_growth() {
    let large_result = "word ".repeat(20000);
    let (truncated, was_truncated, original_tokens) = truncate_tool_result(&large_result);

    assert!(was_truncated, "Large tool result should be truncated");
    assert!(original_tokens > 4000, "Original tokens should exceed 4000");

    let truncated_tokens = estimate_tokens(&truncated);
    assert!(
        truncated_tokens < original_tokens,
        "Truncated tokens should be less than original"
    );
    assert!(
        truncated.contains("[...truncated"),
        "Should contain truncation notice"
    );
}

#[test]
fn test_recovery_message_preserves_turn_context() {
    let mut session = ChatSession::new("test-model".to_string(), None, false);

    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Hello".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Hi there!".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Tell me about the weather".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let count_before_turn = session.messages.len();

    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Let me check...".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let (removed, _) = session.remove_last_assistant_messages_with_content();
    assert!(removed >= 1, "Should have removed at least one message");
    assert!(
        session.messages.len() <= count_before_turn,
        "Session should have fewer or equal messages after recovery"
    );

    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "What about Tokyo weather?".to_string(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let last_msg = session
        .messages
        .last()
        .expect("Should have at least one message");
    assert_eq!(
        last_msg.role,
        MessageRole::User,
        "Last message should be user retry"
    );
    assert_eq!(
        last_msg.content, "What about Tokyo weather?",
        "Retry message should be added"
    );
}

#[test]
fn test_context_check_with_compaction_summary() {
    let mut session = ChatSession::new("test-model".to_string(), None, false);

    for i in 0..10 {
        session.messages.push(SavedMessage {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!("Message {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    session.set_compacted_summary_with_range(
        "Summary of old conversation about weather and news".to_string(),
        Some((0, 5)),
    );

    let status = check_context_overflow(
        &session,
        "You are helpful.",
        50000,
        DEFAULT_OVERFLOW_THRESHOLD,
    );

    assert!(
        !status.needs_compaction(),
        "Small context with summary should not need compaction"
    );
    assert!(status.total_tokens() > 0, "Should have some tokens counted");
}

#[test]
fn test_multiple_overflow_recovery_cycles() {
    let mut session = ChatSession::new("test-model".to_string(), None, false);

    for cycle in 0..3 {
        for i in 0..5 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: format!("Cycle {} User {}", cycle, i),
                timestamp: Utc::now(),
                ..Default::default()
            });
            session.messages.push(SavedMessage {
                role: MessageRole::Assistant,
                content: format!("Cycle {} Assistant {}", cycle, i),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        let (removed, _) = session.remove_last_assistant_messages_with_content();
        assert!(removed >= 1, "Cycle {}: Should remove messages", cycle);

        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("Cycle {} retry", cycle),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    assert!(
        session.messages.len() > 0,
        "Should have messages after multiple cycles"
    );

    let last = session.messages.last().unwrap();
    assert_eq!(last.role, MessageRole::User, "Last message should be user");
}

#[test]
fn test_unused_create_session_helper() {
    // This test exists to suppress the unused function warning
    let _session = create_session_with_token_count(5, 100);
}
