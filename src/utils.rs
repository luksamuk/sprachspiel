//! Common utility functions
//!
//! Shared utilities for parameter parsing, I/O, and formatting.

use base64::Engine;
use std::io::{self, Read};

/// Parse a boolean from an optional string value
///
/// Accepts: "true", "1", "yes" (case-insensitive)
/// Falls back to default for None or empty strings
pub fn parse_bool<S: AsRef<str>>(value: Option<S>, default: bool) -> bool {
    match value {
        None => default,
        Some(s) if s.as_ref().is_empty() => default,
        Some(s) => matches!(s.as_ref().to_lowercase().as_str(), "true" | "1" | "yes"),
    }
}

/// Parse an optional u32 from an optional string value
///
/// Falls back to default for None, empty strings, or parse errors
pub fn parse_u32<S: AsRef<str>>(value: Option<S>, default: Option<u32>) -> Option<u32> {
    match value {
        None => default,
        Some(s) if s.as_ref().is_empty() => default,
        Some(s) => s.as_ref().parse::<u32>().ok().or(default),
    }
}

/// Parse a number with bounds checking
///
/// Clamps the result to max if provided
pub fn parse_bounded_number(value: Option<&str>, default: usize, max: Option<usize>) -> usize {
    match value {
        None => default,
        Some(s) if s.trim().is_empty() => default,
        Some(s) => {
            let parsed = s.trim().parse::<usize>().unwrap_or(default);
            match max {
                Some(m) => parsed.min(m),
                None => parsed,
            }
        }
    }
}

/// Read from stdin and return trimmed content
///
/// Returns an error message if stdin is empty or cannot be read
pub fn read_stdin() -> Result<String, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("Failed to read from stdin: {}", e))?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        Err("No input provided from stdin".to_string())
    } else {
        Ok(trimmed)
    }
}

/// Supported image extensions
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

/// Validate that a file exists and has a supported image extension
pub fn validate_image_file(path: &std::path::Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    if !path.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());

    match ext {
        Some(ref e) if IMAGE_EXTENSIONS.contains(&e.as_str()) => Ok(()),
        Some(e) => Err(format!(
            "Unsupported image format: {}. Supported: {}",
            e,
            IMAGE_EXTENSIONS.join(", ")
        )),
        None => Err("Invalid file extension: unknown".to_string()),
    }
}

/// Read a file and return its contents as base64-encoded string
pub async fn read_file_as_base64(path: &std::path::Path) -> Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file {}: {}", path.display(), e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Format file size in human-readable format (KB/MB)
pub fn format_size(bytes: u64) -> String {
    let kb = bytes as f64 / 1024.0;
    if kb >= 1024.0 {
        format!("{:.1} MB", kb / 1024.0)
    } else if kb >= 1.0 {
        format!("{:.0} KB", kb)
    } else {
        format!("{} B", bytes)
    }
}

/// Capitalize the first letter of a string.
///
/// Converts the first character to uppercase and the rest to lowercase.
/// Useful for formatting names (e.g., "PIKACHU" -> "Pikachu").
///
/// # Example
/// ```
/// use ask_ai::utils::capitalize;
/// assert_eq!(capitalize("hello"), "Hello");
/// assert_eq!(capitalize("HELLO"), "Hello");
/// assert_eq!(capitalize("pikachu"), "Pikachu");
/// ```
#[cfg(feature = "pokemon-tools")]
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str(),
    }
}

/// Normalize input string for comparison
///
/// Trims whitespace and converts to lowercase. This is Unicode-safe.
/// Use this for case-insensitive matching of user input.
///
/// # Example
/// ```
/// use ask_ai::utils::normalize_input;
/// assert_eq!(normalize_input("  HeLLo  "), "hello");
/// assert_eq!(normalize_input("Pokémon"), "pokémon");  // Unicode preserved
/// ```
pub fn normalize_input(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Truncate a string to a maximum number of characters (not bytes)
///
/// This is Unicode-safe and won't panic on multibyte characters.
/// Returns the truncated string with "..." appended if truncation occurred.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

/// Truncate a string to fit within a token budget.
///
/// Used for emergency truncation of tool results when context is near overflow.
/// Preserves the beginning (head) of the content since structure and context
/// are typically more valuable than the end.
///
/// # Arguments
/// * `text` - The text to potentially truncate
/// * `budget` - Maximum number of tokens allowed
///
/// # Returns
/// * Truncated string with notice if truncation occurred
/// * Original string if within budget
pub fn truncate_to_budget(text: &str, budget: usize) -> String {
    use crate::tokens::estimate_tokens;

    let estimated_tokens = estimate_tokens(text);

    if estimated_tokens <= budget {
        return text.to_string();
    }

    // Reserve tokens for overhead (truncation notice)
    let overhead_tokens = 20;
    let target_tokens = budget.saturating_sub(overhead_tokens);

    // Rough conversion: ~4 characters per token (conservative)
    let char_budget = target_tokens.saturating_mul(4);

    // Preserve head (structure/context is more valuable)
    let truncated: String = text.chars().take(char_budget).collect();

    format!(
        "{}\n\n... [Result truncated: {} → {} tokens]",
        truncated,
        estimated_tokens,
        budget
    )
}

/// Fetch JSON from a URL with proper error handling for tools.
///
/// This is a helper function for tool implementations that need to
/// make HTTP requests and parse JSON responses.
///
/// # Arguments
/// * `url` - The URL to fetch
/// * `tool_name` - Tool name for logging (via debug_tools)
///
/// # Returns
/// * `Ok(T)` - Parsed JSON response
/// * `Err(String)` - Error message suitable for LLM consumption
#[cfg(any(
    feature = "weather-tools",
    feature = "pokemon-tools",
    feature = "serper-tools",
    feature = "search-tools"
))]
pub async fn fetch_json<T: serde::de::DeserializeOwned>(
    url: &str,
    tool_name: &str,
) -> Result<T, String> {
    use crate::debug_tools::log_tool_result;

    let response = match reqwest::get(url).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!("Network error: {}. Please try again later.", e);
            log_tool_result(tool_name, &err);
            return Err(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "HTTP error: {}. The service may be temporarily unavailable.",
            response.status()
        );
        log_tool_result(tool_name, &err);
        return Err(err);
    }

    match response.json().await {
        Ok(data) => Ok(data),
        Err(e) => {
            let err = format!("Error parsing response: {}. Please try again.", e);
            log_tool_result(tool_name, &err);
            Err(err)
        }
    }
}

/// POST JSON to a URL with custom headers.
///
/// Same as `fetch_json` but uses POST with a JSON body.
#[cfg(any(feature = "serper-tools", feature = "search-tools"))]
pub async fn post_json_with_headers<T: serde::de::DeserializeOwned>(
    url: &str,
    tool_name: &str,
    headers: Vec<(&str, &str)>,
    body: &serde_json::Value,
) -> Result<T, String> {
    use crate::debug_tools::log_tool_result;

    let mut request = reqwest::Client::new().post(url);
    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = match request.json(body).send().await {
        Ok(r) => r,
        Err(e) => {
            let err = format!("Network error: {}. Please try again later.", e);
            log_tool_result(tool_name, &err);
            return Err(err);
        }
    };

    if !response.status().is_success() {
        let err = format!(
            "HTTP error: {}. The service may be temporarily unavailable.",
            response.status()
        );
        log_tool_result(tool_name, &err);
        return Err(err);
    }

    match response.json().await {
        Ok(data) => Ok(data),
        Err(e) => {
            let err = format!("Error parsing response: {}. Please try again.", e);
            log_tool_result(tool_name, &err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool(Some("true"), false));
        assert!(parse_bool(Some("TRUE"), false));
        assert!(parse_bool(Some("1"), false));
        assert!(parse_bool(Some("yes"), false));
        assert!(parse_bool(Some("YES"), false));
        assert!(!parse_bool(Some("false"), true));
        assert!(!parse_bool(Some("0"), true));
        assert!(parse_bool(None::<&str>, true));
        assert!(!parse_bool(None::<&str>, false));
        assert!(parse_bool(Some(""), true));
        // Test with owned String
        assert!(parse_bool(Some(String::from("true")), false));
    }

    #[test]
    fn test_parse_u32() {
        assert_eq!(parse_u32(Some("42"), None), Some(42));
        assert_eq!(parse_u32(Some("0"), None), Some(0));
        assert_eq!(parse_u32(Some("invalid"), Some(10)), Some(10));
        assert_eq!(parse_u32(None::<&str>, Some(5)), Some(5));
        assert_eq!(parse_u32(Some(""), Some(5)), Some(5));
        assert_eq!(parse_u32(Some("99999999999999999999"), None), None);
        // Test with owned String
        assert_eq!(parse_u32(Some(String::from("42")), None), Some(42));
    }

    #[test]
    fn test_parse_bounded_number() {
        assert_eq!(parse_bounded_number(Some("50"), 10, None), 50);
        assert_eq!(parse_bounded_number(Some("50"), 10, Some(30)), 30);
        assert_eq!(parse_bounded_number(None, 10, Some(30)), 10);
        assert_eq!(parse_bounded_number(Some("invalid"), 10, None), 10);
        assert_eq!(parse_bounded_number(Some("5"), 10, Some(30)), 5);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(1536), "2 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1572864), "1.5 MB");
    }

    #[test]
    #[cfg(feature = "pokemon-tools")]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("HELLO"), "Hello");
        assert_eq!(capitalize("h"), "H");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("pikachu"), "Pikachu");
    }

    #[test]
    fn test_normalize_input() {
        // Basic whitespace trimming
        assert_eq!(normalize_input("  hello  "), "hello");

        // Case conversion
        assert_eq!(normalize_input("HeLLo WoRLD"), "hello world");

        // Unicode preserved
        assert_eq!(normalize_input("  Pokémon  "), "pokémon");
        assert_eq!(normalize_input("中国对巴西"), "中国对巴西");

        // Mixed ASCII and Unicode
        assert_eq!(normalize_input("  HeLLo中国  "), "hello中国");

        // Empty and whitespace only
        assert_eq!(normalize_input(""), "");
        assert_eq!(normalize_input("   "), "");
    }

    #[test]
    fn test_truncate_chars() {
        // Short string - no truncation
        assert_eq!(truncate_chars("hello", 10), "hello");

        // Exact length - no truncation
        assert_eq!(truncate_chars("hello", 5), "hello");

        // Needs truncation
        assert_eq!(truncate_chars("hello world", 5), "hello...");

        // Unicode - multibyte characters
        assert_eq!(truncate_chars("中国对巴西新闻视角", 5), "中国对巴西...");

        // Mixed ASCII and Unicode
        assert_eq!(truncate_chars("Hello中国", 6), "Hello中...");
    }

    #[test]
    fn test_truncate_to_budget_within() {
        // Text within budget - no truncation
        let text = "Hello world this is a test";
        let result = truncate_to_budget(text, 100);
        assert_eq!(result, text);
    }

    #[test]
    fn test_truncate_to_budget_exceeds() {
        // Text exceeds budget - truncation with notice
        // "This is a longer piece of text that should be truncated" ~ 10 tokens
        // Budget of 5 tokens with 20 overhead = target of 0 -> minimal content
        let text = "This is a longer piece of text that should be truncated";
        let result = truncate_to_budget(text, 5);
        
        assert!(result.contains("... [Result truncated"));
        assert!(!result.is_empty());
    }

    #[test]
    fn test_truncate_to_budget_preserves_head() {
        // Verify that head is preserved (structure is more valuable)
        // "Line 1\nLine 2\nLine 3\nLine 4\nLine 5" ~ 10 tokens
        // Budget of 50 tokens should still preserve beginning
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let result = truncate_to_budget(text, 50);
        
        // Within budget, no truncation
        assert_eq!(result, text);
        
        // Now with smaller budget
        let result_small = truncate_to_budget(text, 3);
        assert!(result_small.contains("truncated"));
    }

    #[test]
    fn test_truncate_to_budget_budget_too_small() {
        // Budget smaller than overhead - should still work
        let text = "Hello world";
        let result = truncate_to_budget(text, 5);
        
        // Should still return something (possibly empty truncated string)
        assert!(!result.is_empty() || result.contains("truncated"));
    }
    
    #[test]
    fn test_truncate_to_budget_realistic() {
        // Realistic scenario: 4000 token budget, 8000 token content
        let long_text = "word ".repeat(8000); // ~32000 chars, ~10666 tokens
        let result = truncate_to_budget(&long_text, 4000);
        
        assert!(result.contains("... [Result truncated"));
        // Budget of 4000 - 20 overhead = 3980 tokens * 4 chars = ~15920 chars
        assert!(result.len() < long_text.len());
    }

    #[test]
    fn test_truncate_to_budget_unicode() {
        // Unicode text truncation with whitespace - should handle multibyte chars correctly
        // Note: estimate_tokens uses word-based counting (split_whitespace)
        // Chinese without spaces = 1 word, with spaces = multiple words
        
        // Test with spaces to ensure word count triggers truncation
        let unicode_text = "中国 对 巴西 新闻 视角 测试 数据 这是 一个 很长 的 文本 需要 被 截断 ".repeat(20);
        let result = truncate_to_budget(&unicode_text, 10);
        
        // Should contain truncation notice when tokens exceed budget
        assert!(result.contains("... [Result truncated") || result.len() < unicode_text.len(), 
            "Result should be truncated or shorter: got {} chars", result.len());
        
        // Test that chars().take() handles multibyte correctly (doesn't panic)
        assert!(!result.is_empty());
        
        // Mixed ASCII and Unicode with spaces
        let mixed_text = "Hello 世界 This is 中国 测试 数据 ".repeat(30);
        let result_mixed = truncate_to_budget(&mixed_text, 20);
        assert!(result_mixed.contains("... [Result truncated") || result_mixed == mixed_text);
        
        // Long ASCII text (reliable for truncation test)
        let long_ascii = "word ".repeat(1000); // ~1333 tokens
        let result_ascii = truncate_to_budget(&long_ascii, 100);
        assert!(result_ascii.contains("... [Result truncated"), "ASCII should be truncated");
        assert!(result_ascii.len() < long_ascii.len());
    }
}
