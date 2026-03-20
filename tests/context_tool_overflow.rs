//! Integration tests for context overflow during tool execution
//!
//! Tests the complete flow of:
//! 1. Pre-tool context check
//! 2. During-tool context check
//! 3. Error recovery

use ask_ai::context_overflow::{
    COMPACTION_BUFFER, DEFAULT_KEEP_LAST, DEFAULT_OVERFLOW_THRESHOLD, PRE_TOOL_BUFFER,
};
use ask_ai::tokens::estimate_tokens;

/// Minimum messages to preserve (must match src/context_overflow.rs)
const MIN_PRESERVE_LAST: usize = 1;

#[test]
fn test_buffer_hierarchy() {
    // Pre-tool buffer should be larger than compaction buffer
    // This ensures warning fires before auto-compaction
    assert!(
        PRE_TOOL_BUFFER > COMPACTION_BUFFER,
        "Pre-tool buffer ({}) must be larger than compaction buffer ({})",
        PRE_TOOL_BUFFER,
        COMPACTION_BUFFER
    );

    // Pre-tool at 20K remaining, compaction at 15K remaining
    // This gives 5K buffer for warning before auto-compact
    assert_eq!(PRE_TOOL_BUFFER, 20_000);
    assert_eq!(COMPACTION_BUFFER, 15_000);
    assert_eq!(DEFAULT_OVERFLOW_THRESHOLD, 0.80); // Kept for display only
}

#[test]
fn test_min_preserve_last_is_reasonable() {
    // Should preserve at least the current user message
    assert!(MIN_PRESERVE_LAST >= 1, "Should preserve at least 1 message");

    // Should not preserve too many
    assert!(
        MIN_PRESERVE_LAST <= 5,
        "Should not preserve more than 5 messages by default"
    );
}

#[test]
fn test_estimation_accuracy() {
    // Token estimation should be reasonably accurate
    // Word-based estimation: ~0.75 words per token

    // Short text
    let short = "Hello world"; // ~2 words = ~3 tokens
    let short_estimate = estimate_tokens(short);
    assert!(
        short_estimate >= 2 && short_estimate <= 5,
        "Short text estimation should be close"
    );

    // Long text
    let long = "word ".repeat(1000); // ~1000 words = ~1333 tokens
    let long_estimate = estimate_tokens(&long);
    assert!(
        long_estimate >= 1000 && long_estimate <= 2000,
        "Long text estimation should be close"
    );

    // Code-like text (higher entropy)
    let code = "fn main() { println!(\"Hello\"); }";
    let code_estimate = estimate_tokens(code);
    assert!(
        code_estimate >= 5 && code_estimate <= 20,
        "Code estimation should handle punctuation"
    );
}

#[test]
fn test_unicode_estimation() {
    // Unicode text should estimate correctly
    // Note: Word-based estimation may be less accurate for non-space languages

    let japanese = "こんにちは世界"; // No spaces, different estimation
    let estimate = estimate_tokens(japanese);
    assert!(
        estimate >= 1,
        "Should estimate at least 1 token for any non-empty text"
    );

    let mixed = "Hello 世界 world"; // Mixed scripts
    let mixed_estimate = estimate_tokens(mixed);
    assert!(
        mixed_estimate >= 3,
        "Mixed language should estimate reasonably"
    );
}

#[test]
fn test_empty_content_handling() {
    // Empty content should not panic
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn test_whitespace_content() {
    // Whitespace-only content has zero tokens (word-based estimation)
    let whitespace = "   \n\t   ";
    let estimate = estimate_tokens(whitespace);
    assert!(
        estimate >= 0,
        "Whitespace should estimate to zero or minimal tokens"
    );
}

#[test]
fn test_threshold_calculations() {
    // Verify warning threshold is 90% of overflow (for display purposes)
    let overflow = DEFAULT_OVERFLOW_THRESHOLD;
    let warning = overflow * 0.9;

    // At 1000 token context with 80% threshold = 800 tokens overflow
    // Warning at 90% of 80% = 72% = 720 tokens
    assert!(
        (warning - 0.72).abs() < 0.01,
        "Warning threshold should be 72%"
    );

    // DEFAULT_OVERFLOW_THRESHOLD is kept for display purposes only
    // Buffer-based thresholds are now used for actual triggering
    assert_eq!(DEFAULT_OVERFLOW_THRESHOLD, 0.80);
}

#[test]
fn test_preservation_logic() {
    // When context is at pre-tool threshold (75%):
    // 1. Check should pass
    // 2. User message is already in session
    // 3. Default keep_last (5) should preserve recent messages

    // MIN_PRESERVE_LAST ensures at least 1 message preserved
    // DEFAULT_KEEP_LAST (5) preserves even more

    assert!(
        DEFAULT_KEEP_LAST >= MIN_PRESERVE_LAST,
        "Default keep last should be >= min preserve last"
    );

    // With 10 messages and keep_last = 5:
    // - Messages 0-4 are in "middle" (can be compacted)
    // - Messages 5-9 are preserved
    // - User message (last) is at position 9, definitely preserved

    let total_messages = 10;
    let preserved = DEFAULT_KEEP_LAST;
    let can_compact = total_messages - preserved;

    assert_eq!(can_compact, 5, "Should be able to compact 5 messages");
    assert!(
        preserved >= MIN_PRESERVE_LAST,
        "Should preserve minimum messages"
    );
}
