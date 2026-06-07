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
    // Expand tilde if present
    let expanded_path = if path.to_str().map(|s| s.starts_with('~')).unwrap_or(false) {
        expand_tilde_path(path.to_str().unwrap_or(""))
    } else {
        path.to_path_buf()
    };

    if !expanded_path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    if !expanded_path.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }

    let ext = expanded_path
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
    // Expand tilde if present
    let expanded_path = if path.to_str().map(|s| s.starts_with('~')).unwrap_or(false) {
        expand_tilde_path(path.to_str().unwrap_or(""))
    } else {
        path.to_path_buf()
    };

    let bytes = tokio::fs::read(&expanded_path)
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

/// Strip ANSI escape codes from a string.
///
/// Removes all ANSI escape sequences (CSI sequences like colors, cursor
/// movement, etc.) and returns the plain text. This is used by the TUI
/// rendering path (RatatuiView) where ANSI codes would appear as literal
/// text since ratatui renders via styled `Span`s, not terminal escapes.
///
/// The parser handles:
/// - Simple codes: `\x1B[0m` (reset), `\x1B[1m` (bold), etc.
/// - 256-color: `\x1B[38;5;220m`, `\x1B[48;5;45m`
/// - 24-bit true color: `\x1B[38;2;245;213;122m`, `\x1B[48;2;0;0;0m`
/// - OSC and other sequences: `\x1B]...\\x1B\\` (bel-terminated)
///
/// No regex dependency — hand-parsed for zero-cost performance.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\x1B' {
            // Start of escape sequence
            if i + 1 < len && chars[i + 1] == '[' {
                // CSI sequence: \x1B[ ... <final byte>
                i += 2; // Skip ESC and '['
                while i < len {
                    let c = chars[i];
                    i += 1;
                    // Final byte: 0x40-0x7E (@A-Z[\]^_`a-z{|}~)
                    if ('\x40'..='\x7E').contains(&c) {
                        break;
                    }
                }
            } else if i + 1 < len && chars[i + 1] == ']' {
                // OSC sequence: \x1B] ... \x07 (BEL) or \x1B\\ (ST)
                i += 2; // Skip ESC and ']'
                while i < len {
                    if chars[i] == '\x07' {
                        // BEL terminates OSC
                        i += 1;
                        break;
                    }
                    if chars[i] == '\x1B' && i + 1 < len && chars[i + 1] == '\\' {
                        // ST (\x1B\\) terminates OSC
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            } else {
                // Other escape: \x1B followed by single char
                i += 1;
                if i < len {
                    i += 1; // Skip the character after ESC
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Capitalize the first letter of a string.
///
/// Converts the first character to uppercase and the rest to lowercase.
/// Useful for formatting names (e.g., "PIKACHU" -> "Pikachu").
///
/// # Example
/// ```
/// use sprachspiel::utils::capitalize;
/// assert_eq!(capitalize("hello"), "Hello");
/// assert_eq!(capitalize("HELLO"), "Hello");
/// assert_eq!(capitalize("pikachu"), "Pikachu");
/// ```
#[cfg(feature = "pokemon-tools")]
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str()
        }
    }
}

/// Normalize input string for comparison
///
/// Trims whitespace and converts to lowercase. This is Unicode-safe.
/// Use this for case-insensitive matching of user input.
///
/// # Example
/// ```
/// use sprachspiel::utils::normalize_input;
/// assert_eq!(normalize_input("  HeLLo  "), "hello");
/// assert_eq!(normalize_input("Pokémon"), "pokémon");  // Unicode preserved
/// ```
pub fn normalize_input(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Expand tilde (~) in file paths to the user's home directory.
///
/// This function handles:
/// - `~` alone -> home directory
/// - `~/path/to/file` -> home/path/to/file
/// - `/absolute/path` -> unchanged
/// - `relative/path` -> unchanged
///
/// # Example
/// ```
/// use sprachspiel::utils::expand_tilde_path;
/// use std::path::PathBuf;
///
/// // On Unix: expands to /home/user/file.txt
/// let expanded = expand_tilde_path("~/file.txt");
/// assert!(expanded.is_absolute());
/// ```
pub fn expand_tilde_path(path: &str) -> std::path::PathBuf {
    let trimmed = path.trim();

    if let Some(rest) = trimmed.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            // Handle ~ alone
            if rest.is_empty() || rest == "/" {
                return home;
            }

            // Handle ~/path
            let rest = rest.strip_prefix('/').unwrap_or(rest);

            if rest.is_empty() {
                home
            } else {
                home.join(rest)
            }
        } else {
            // Cannot expand, return as-is
            std::path::PathBuf::from(trimmed)
        }
    } else {
        std::path::PathBuf::from(trimmed)
    }
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

/// Truncate a string to a maximum visual width, using real Unicode column widths.
///
/// This is the most accurate truncation for terminal display: each character
/// is measured by its display width (CJK = 2 columns, most others = 1).
/// If truncation is needed, appends "…" (U+2026) which itself occupies 1 column.
///
/// # Arguments
/// * `s` - The string to potentially truncate
/// * `max_width` - Maximum visual columns (terminal width)
///
/// # Returns
/// * Original string if it fits within `max_width`
/// * Truncated string with "…" appended if it exceeds `max_width`
pub fn truncate_visual_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthStr;

    let ellipsis = "…";
    let ellipsis_width = UnicodeWidthStr::width(ellipsis);

    if max_width <= ellipsis_width {
        // Not enough space for ellipsis + at least 1 char
        return String::new();
    }

    let visual_width = UnicodeWidthStr::width(s);
    if visual_width <= max_width {
        return s.to_string();
    }

    // Need to truncate: reserve space for ellipsis
    let target_width = max_width - ellipsis_width;
    let mut result = String::new();
    let mut current_width = 0;

    for ch in s.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }

    result.push_str(ellipsis);
    result
}

/// Truncate a string at the first newline, appending ellipsis if truncated.
///
/// Used for recent context display where multi-line messages should collapse
/// to a single line with "…" at the newline position, rather than merging
/// lines with spaces.
///
/// # Examples
/// ```
/// use sprachspiel::utils::truncate_at_first_newline;
/// assert_eq!(truncate_at_first_newline("Hello\nWorld"), "Hello…");
/// assert_eq!(truncate_at_first_newline("No newline"), "No newline");
/// assert_eq!(truncate_at_first_newline("\nStarts with newline"), "…");
/// ```
pub fn truncate_at_first_newline(s: &str) -> String {
    if let Some(pos) = s.find('\n') {
        let before = &s[..pos];
        if before.is_empty() {
            "…".to_string()
        } else {
            format!("{before}…")
        }
    } else {
        s.to_string()
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
        truncated, estimated_tokens, budget
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
    fn test_truncate_visual_width_ascii() {
        // Short string - no truncation
        assert_eq!(truncate_visual_width("hello", 10), "hello");

        // Exact length - no truncation
        assert_eq!(truncate_visual_width("hello", 5), "hello");

        // Needs truncation
        assert_eq!(truncate_visual_width("hello world", 10), "hello wor…");

        // Very small width
        assert_eq!(truncate_visual_width("hello", 3), "he…");
    }

    #[test]
    fn test_truncate_visual_width_unicode() {
        // CJK chars take 2 columns each
        // "中国" = 4 columns, "中…" = 2+1 = 3 columns
        assert_eq!(truncate_visual_width("中国对巴西", 5), "中国…");

        // Mixed ASCII and CJK
        // "He中国" = 2+2+2 = 6 columns, truncate to 5 → "He中…" (2+2+1 = 5)
        assert_eq!(truncate_visual_width("He中国对", 5), "He中…");
    }

    #[test]
    fn test_truncate_visual_width_edge_cases() {
        // Zero width returns empty
        assert_eq!(truncate_visual_width("hello", 0), "");

        // Width 1 (only space for part of ellipsis)
        assert_eq!(truncate_visual_width("hello", 1), "");

        // Width 2 (ellipsis takes 1, only 1 char fits)
        assert_eq!(truncate_visual_width("hello", 2), "h…");

        // Empty string
        assert_eq!(truncate_visual_width("", 10), "");
    }

    #[test]
    fn test_truncate_at_first_newline_basic() {
        // No newline — return as-is
        assert_eq!(truncate_at_first_newline("Hello world"), "Hello world");

        // Single newline — truncate with ellipsis
        assert_eq!(truncate_at_first_newline("Hello\nWorld"), "Hello…");

        // Multiple newlines — truncate at first
        assert_eq!(
            truncate_at_first_newline("Line 1\nLine 2\nLine 3"),
            "Line 1…"
        );

        // Starts with newline — just ellipsis
        assert_eq!(truncate_at_first_newline("\nStarts with newline"), "…");

        // Empty string
        assert_eq!(truncate_at_first_newline(""), "");

        // Only newline
        assert_eq!(truncate_at_first_newline("\n"), "…");
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
        let unicode_text =
            "中国 对 巴西 新闻 视角 测试 数据 这是 一个 很长 的 文本 需要 被 截断 ".repeat(20);
        let result = truncate_to_budget(&unicode_text, 10);

        // Should contain truncation notice when tokens exceed budget
        assert!(
            result.contains("... [Result truncated") || result.len() < unicode_text.len(),
            "Result should be truncated or shorter: got {} chars",
            result.len()
        );

        // Test that chars().take() handles multibyte correctly (doesn't panic)
        assert!(!result.is_empty());

        // Mixed ASCII and Unicode with spaces
        let mixed_text = "Hello 世界 This is 中国 测试 数据 ".repeat(30);
        let result_mixed = truncate_to_budget(&mixed_text, 20);
        assert!(result_mixed.contains("... [Result truncated") || result_mixed == mixed_text);

        // Long ASCII text (reliable for truncation test)
        let long_ascii = "word ".repeat(1000); // ~1333 tokens
        let result_ascii = truncate_to_budget(&long_ascii, 100);
        assert!(
            result_ascii.contains("... [Result truncated"),
            "ASCII should be truncated"
        );
        assert!(result_ascii.len() < long_ascii.len());
    }

    #[test]
    fn test_expand_tilde_path() {
        // Test expansion with home directory
        let home = dirs::home_dir();

        if let Some(ref home_path) = home {
            // ~/file.txt -> home/file.txt
            let expanded = expand_tilde_path("~/file.txt");
            assert_eq!(expanded, home_path.join("file.txt"));

            // ~ alone -> home
            let expanded_alone = expand_tilde_path("~");
            assert_eq!(expanded_alone, *home_path);

            // ~/ alone -> home
            let expanded_slash = expand_tilde_path("~/");
            assert_eq!(expanded_slash, *home_path);
        }

        // Absolute path - unchanged
        let absolute = expand_tilde_path("/absolute/path");
        assert_eq!(absolute, std::path::PathBuf::from("/absolute/path"));

        // Relative path - unchanged
        let relative = expand_tilde_path("relative/path");
        assert_eq!(relative, std::path::PathBuf::from("relative/path"));

        // Path with spaces in home expansion
        let with_spaces = expand_tilde_path("~/path with spaces/file.txt");
        if let Some(home_path) = home {
            assert_eq!(with_spaces, home_path.join("path with spaces/file.txt"));
        }
    }

    // ── strip_ansi_codes tests ──────────────────────────────────────────

    #[test]
    fn test_strip_ansi_plain_text() {
        assert_eq!(strip_ansi_codes("hello world"), "hello world");
        assert_eq!(strip_ansi_codes(""), "");
        assert_eq!(
            strip_ansi_codes("simple text without codes"),
            "simple text without codes"
        );
    }

    #[test]
    fn test_strip_ansi_simple_codes() {
        // Reset
        assert_eq!(strip_ansi_codes("\x1B[0m"), "");
        // Bold
        assert_eq!(strip_ansi_codes("\x1B[1m"), "");
        // Dim
        assert_eq!(strip_ansi_codes("\x1B[2m"), "");
        // Red foreground
        assert_eq!(strip_ansi_codes("\x1B[31m"), "");
        // Cyan foreground
        assert_eq!(strip_ansi_codes("\x1B[36m"), "");
    }

    #[test]
    fn test_strip_ansi_256_color() {
        // Foreground 256-color (gold ~220)
        assert_eq!(strip_ansi_codes("\x1B[38;5;220m"), "");
        // Background 256-color
        assert_eq!(strip_ansi_codes("\x1B[48;5;45m"), "");
        // Text with 256-color codes
        assert_eq!(strip_ansi_codes("\x1B[38;5;220mSPRACH\x1B[0m"), "SPRACH");
    }

    #[test]
    fn test_strip_ansi_true_color() {
        // 24-bit foreground color
        assert_eq!(strip_ansi_codes("\x1B[38;2;245;213;122m"), "");
        // 24-bit background color
        assert_eq!(strip_ansi_codes("\x1B[48;2;0;0;0m"), "");
        // Full true-color sequence (from EXTENDED_MIND_ART)
        assert_eq!(strip_ansi_codes("\x1B[38;2;245;213;122m⢀\x1B[0m"), "⢀");
    }

    #[test]
    fn test_strip_ansi_mixed() {
        // BANNER_LOGO style: gold + cyan + reset
        let input = "\x1B[38;5;220m┏━┓\x1B[0m\x1B[38;5;45m┏━┓\x1B[0m";
        assert_eq!(strip_ansi_codes(input), "┏━┓┏━┓");

        // WelcomeInfo style: BOLD_CYAN + DIM + RESET
        let input = "\x1B[1;36mModel:\x1B[0m\x1B[2mglm-5:cloud\x1B[0m";
        assert_eq!(strip_ansi_codes(input), "Model:glm-5:cloud");

        // RecentContextInfo style: BOLD_CYAN + BOLD_YELLOW + DIM
        let input = "\x1B[1;36m👤 User\x1B[0m:\x1B[2mHello\x1B[0m";
        assert_eq!(strip_ansi_codes(input), "👤 User:Hello");
    }

    #[test]
    fn test_strip_ansi_banners() {
        // Simpler banner line with multiple ANSI sequences
        let input = "\x1B[38;5;220m┏━┓┏━┓┏━┓┏━┓┏━╸╻ ╻\x1B[0m\x1B[38;5;45m┏━┓┏━┓╻┏━╸╻  \x1B[0m";
        let clean = strip_ansi_codes(input);
        assert!(!clean.contains('\x1B'), "Should have no escape characters");
        assert!(clean.contains("┏━┓"), "Should preserve box drawing chars");
    }

    #[test]
    fn test_strip_ansi_multiple_codes_inline() {
        // From render_fact_list: CYAN + DIM + RESET
        let input = "  \x1B[36m#42\x1B[0m \x1B[2m[category]\x1B[0m fact content";
        assert_eq!(strip_ansi_codes(input), "  #42 [category] fact content");
    }

    #[test]
    fn test_strip_ansi_no_escapes_at_all() {
        assert_eq!(
            strip_ansi_codes("Line 1\nLine 2\nLine 3"),
            "Line 1\nLine 2\nLine 3"
        );
    }

    #[test]
    fn test_strip_ansi_osc_sequences() {
        // OSC title sequence: \x1B]0;title\x07
        assert_eq!(strip_ansi_codes("\x1B]0;My Title\x07Hello"), "Hello");
        // OSC sequence with ST terminator: \x1B]0;title\x1B\\
        assert_eq!(strip_ansi_codes("\x1B]0;Title\x1B\\World"), "World");
    }

    #[test]
    fn test_strip_ansi_preserves_unicode() {
        // Braille and CJK characters should be preserved
        let input = "⢀⣤⡀🧠🔧你好世界";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_strip_ansi_complex_real_world() {
        // Real-world welcome banner line from EXTENDED_MIND_ART
        let line = "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀\x1B[38;2;245;213;122m⢀\x1B[38;2;237;216;142m⣤\x1B[38;2;255;248;123m⡀⠀⠀⠀\x1B[0m";
        let clean = strip_ansi_codes(line);
        assert!(!clean.contains('\x1B'));
        assert!(clean.contains('⢀'));
        assert!(clean.contains('⣤'));
        assert!(clean.contains('⡀'));
        assert!(clean.contains('⠀'));
    }
}
