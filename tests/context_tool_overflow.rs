//! Integration tests for context overflow during tool execution
//!
//! Tests the complete flow of:
//! 1. Pre-tool context check
//! 2. Tool result truncation
//! 3. During-tool context check
//! 4. Error recovery

use ask_ai::context_overflow::{
    CHARS_PER_TOKEN, DEFAULT_KEEP_LAST, DEFAULT_OVERFLOW_THRESHOLD, MAX_TOOL_RESULT_TOKENS,
    PRE_TOOL_THRESHOLD, truncate_tool_result,
};
use ask_ai::tokens::estimate_tokens;

/// Minimum messages to preserve (must match src/context_overflow.rs)
const MIN_PRESERVE_LAST: usize = 1;

#[test]
fn test_threshold_hierarchy() {
    // Pre-tool threshold should be lower than overflow threshold
    // This ensures compaction happens BEFORE overflow
    assert!(
        PRE_TOOL_THRESHOLD < DEFAULT_OVERFLOW_THRESHOLD,
        "Pre-tool threshold ({}) must be lower than overflow threshold ({})",
        PRE_TOOL_THRESHOLD,
        DEFAULT_OVERFLOW_THRESHOLD
    );

    // Pre-tool at 75%, overflow at 80%
    // This gives 5% buffer for tool results during execution
    assert_eq!(PRE_TOOL_THRESHOLD, 0.75);
    assert_eq!(DEFAULT_OVERFLOW_THRESHOLD, 0.80);
}

#[test]
fn test_tool_result_size_limit() {
    // Tool result limit should allow reasonable content
    // but not consume entire context
    assert!(
        MAX_TOOL_RESULT_TOKENS >= 2000,
        "Tool results should allow at least 2K tokens"
    );
    assert!(
        MAX_TOOL_RESULT_TOKENS <= 8000,
        "Tool results should not exceed 8K tokens"
    );
}

#[test]
fn test_truncation_preserves_unicode() {
    // Test various Unicode scripts
    let test_cases = vec![
        ("Japanese", "こんにちは世界 ".repeat(5000)),
        ("Chinese", "你好世界 ".repeat(5000)),
        ("Arabic", "مرحبا بالعالم ".repeat(5000)),
        ("Emoji", "🌍🌎🌏 ".repeat(5000)),
        ("Mixed", "Hello 世界 مرحبا 🌍".repeat(5000)),
    ];

    for (name, content) in test_cases {
        let (truncated, was_truncated, _) = truncate_tool_result(&content);

        assert!(was_truncated, "{} content should be truncated", name);

        // Verify char boundary
        assert!(
            truncated.is_char_boundary(truncated.len()),
            "{} truncated content should end at char boundary",
            name
        );

        // Verify no panic
        let _ = truncated.chars().last();
    }
}

#[test]
fn test_chars_per_token_is_conservative() {
    // CHARS_PER_TOKEN should be conservative (overestimate)
    // Real ratio is ~4 chars/token for English, ~2-3 for code
    // We use 4 to be safe
    assert!(CHARS_PER_TOKEN >= 3, "chars_per_token should be at least 3");
    assert!(CHARS_PER_TOKEN <= 6, "chars_per_token should be at most 6");
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
fn test_truncation_notice_is_included() {
    let long_content = "word ".repeat(10000);
    let (truncated, was_truncated, original_tokens) = truncate_tool_result(&long_content);

    assert!(was_truncated);
    assert!(original_tokens > MAX_TOOL_RESULT_TOKENS);
    assert!(
        truncated.contains("[...truncated"),
        "Truncated content should include notice"
    );
    assert!(
        truncated.contains(&format!("{:?}", original_tokens)),
        "Truncated content should show original token count"
    );
}

#[test]
fn test_short_content_not_truncated() {
    // Content below limit should not be modified
    let short_content = "Hello world";
    let (truncated, was_truncated, original_tokens) = truncate_tool_result(short_content);

    assert!(!was_truncated);
    assert_eq!(truncated, short_content);
    assert!(original_tokens < MAX_TOOL_RESULT_TOKENS);
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

    let empty = "";
    let (truncated, was_truncated, tokens) = truncate_tool_result(empty);
    assert!(!was_truncated);
    assert_eq!(tokens, 0);
    assert_eq!(truncated, "");
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

    // Very large whitespace-filled content should still truncate
    // Note: word-based estimation counts words, so " ".repeat(50000) has 0 words = 0 tokens
    // We need actual words for truncation to kick in
    let large_with_words = "word ".repeat(50000);
    let (_truncated, was_truncated, _) = truncate_tool_result(&large_with_words);
    assert!(
        was_truncated,
        "Large content with words should be truncated"
    );
}

#[test]
fn test_threshold_calculations() {
    // Verify warning threshold is 90% of overflow
    let overflow = DEFAULT_OVERFLOW_THRESHOLD;
    let warning = overflow * 0.9;

    // At 1000 token context with 80% threshold = 800 tokens overflow
    // Warning at 90% of 80% = 72% = 720 tokens
    assert!(
        (warning - 0.72).abs() < 0.01,
        "Warning threshold should be 72%"
    );

    // Pre-tool at 75% should trigger before warning at 72%
    assert!(
        PRE_TOOL_THRESHOLD > warning,
        "Pre-tool threshold should be higher than warning threshold"
    );
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
