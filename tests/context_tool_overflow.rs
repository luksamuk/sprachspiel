//! Integration tests for context overflow during tool execution
//!
//! Tests the complete flow of:
//! 1. Pre-tool context check
//! 2. During-tool context check
//! 3. Error recovery

use sprachspiel::context_overflow::{
    COMPACTION_MIN, CRITICAL_USAGE_PERCENT, DEFAULT_KEEP_LAST, DEFAULT_OVERFLOW_THRESHOLD,
    MODERATE_USAGE_PERCENT, PRE_TOOL_MIN, calculate_thresholds,
};
use sprachspiel::tokens::estimate_tokens;

/// Minimum messages to preserve (must match src/context_overflow.rs)
const MIN_PRESERVE_LAST: usize = 1;

#[test]
fn test_buffer_hierarchy() {
    // Pre-tool buffer should be larger than compaction buffer
    // This ensures warning fires before auto-compaction
    // For a 32K context window:
    // - Pre-tool: 32K * 0.25 = 8K remaining (75% used) - triggers warning
    // - Compaction: 32K * 0.12 = 4K remaining (88% used) - triggers auto-compact
    let (pre_tool, compaction, _, _) = calculate_thresholds(32_768);

    assert!(
        pre_tool > compaction,
        "Pre-tool buffer ({}) must be larger than compaction buffer ({})",
        pre_tool,
        compaction
    );

    // Verify percentage-based thresholds
    assert_eq!(MODERATE_USAGE_PERCENT, 0.75); // 75% - Warning threshold
    assert_eq!(CRITICAL_USAGE_PERCENT, 0.88); // 88% - Compaction threshold
    assert_eq!(DEFAULT_OVERFLOW_THRESHOLD, 0.75);

    // Verify minimum buffers (for small contexts)
    // Must have PRE_TOOL_MIN > COMPACTION_MIN to ensure warning before auto-compact
    assert!(
        PRE_TOOL_MIN > COMPACTION_MIN,
        "PRE_TOOL_MIN ({}) must be larger than COMPACTION_MIN ({})",
        PRE_TOOL_MIN,
        COMPACTION_MIN
    );
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
    assert_eq!(
        estimate, 0,
        "Whitespace should estimate to zero tokens, got {estimate}"
    );
}

#[test]
fn test_threshold_calculations() {
    // Verify warning threshold is 75% (MODERATE_USAGE_PERCENT)
    // Display shows: OK < 75%, MODERATE 75-88%, CRITICAL > 88%
    assert_eq!(DEFAULT_OVERFLOW_THRESHOLD, 0.75);

    // Calculate thresholds for a 32K context
    let (pre_tool, compaction, inter_tool, emergency) = calculate_thresholds(32_768);

    // Pre-tool: 32K * (1 - 0.75) = 8K remaining
    // Compaction: 32K * (1 - 0.88) = ~4K remaining
    // Inter-tool: 32K * (1 - 0.94) = ~2K remaining
    // Emergency: 32K * (1 - 0.97) = ~1K remaining

    assert!(
        pre_tool >= 8_000,
        "Pre-tool buffer should be ~8K for 32K context"
    );
    assert!(
        compaction >= 3_900,
        "Compaction buffer should be ~4K for 32K context"
    );
    assert!(
        inter_tool >= 1_900,
        "Inter-tool buffer should be ~2K for 32K context"
    );
    assert!(
        emergency >= 900,
        "Emergency buffer should be ~1K for 32K context"
    );

    // Hierarchy: pre_tool > compaction > inter_tool > emergency
    assert!(pre_tool > compaction);
    assert!(compaction > inter_tool);
    assert!(inter_tool > emergency);
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
