//! Common utility functions
//!
//! Shared utilities for parameter parsing, I/O, and formatting.

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

/// Capitalize the first letter of a string
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str()
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
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("HELLO"), "Hello");
        assert_eq!(capitalize("h"), "H");
        assert_eq!(capitalize(""), "");
    }
}
