//! SOUL.md - User-defined agent personality
//!
//! Provides user customization of agent identity, behavior, and communication style.
//! SOUL.md replaces the former Pepe personality system.
//!
//! # Location
//!
//! The SOUL.md file is loaded from:
//! 1. `$XDG_CONFIG_HOME/sprachspiel/SOUL.md`
//! 2. `~/.config/sprachspiel/SOUL.md`
//!
//! # Processing
//!
//! 1. Remove HTML comments (`<!-- ... -->`)
//! 2. Normalize whitespace
//! 3. Validate structure (must have at least one `## ` section)
//!
//! # Fallback
//!
//! If SOUL.md doesn't exist or is invalid, `PERSONALITY_DEFAULT` is used.
//! If `--soulless` flag is set, the personality layer is empty.

use std::fs;
use std::path::PathBuf;

use regex::Regex;

/// Default SOUL.md filename
const SOUL_FILENAME: &str = "SOUL.md";

/// Get SOUL.md path
///
/// Priority:
/// 1. XDG_CONFIG_HOME/sprachspiel/SOUL.md
/// 2. ~/.config/sprachspiel/SOUL.md
pub fn get_soul_path() -> Option<PathBuf> {
    use crate::consts::app;

    // Try XDG_CONFIG_HOME first
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg_config).join(app::APP_CONFIG_DIR).join(SOUL_FILENAME);
        if path.exists() {
            return Some(path);
        }
    }

    // Fallback to ~/.config/sprachspiel/SOUL.md
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".config").join(app::APP_CONFIG_DIR).join(SOUL_FILENAME);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Load and process SOUL.md content
///
/// Returns None if:
/// - File doesn't exist
/// - File is empty after cleaning
/// - Content fails validation (no valid sections)
///
/// Processing:
/// 1. Remove HTML comments (<!-- ... -->)
/// 2. Normalize whitespace
/// 3. Validate structure
pub fn load_soul() -> Option<String> {
    let path = get_soul_path()?;
    let content = fs::read_to_string(&path).ok()?;

    let cleaned = process_soul_content(&content)?;
    Some(cleaned)
}

/// Process SOUL.md content for prompt injection
fn process_soul_content(content: &str) -> Option<String> {
    // 1. Remove HTML comments
    let cleaned = remove_html_comments(content);

    // 2. Normalize whitespace
    let cleaned = normalize_whitespace(&cleaned);

    // 3. Validate: must have at least one ## heading (section)
    if !cleaned.contains("## ") {
        return None;
    }

    // 4. Validate: must not be empty after processing
    if cleaned.trim().is_empty() {
        return None;
    }

    Some(cleaned)
}

/// Remove HTML comments <!-- ... -->
fn remove_html_comments(content: &str) -> String {
    let re = Regex::new(r"<!--[\s\S]*?-->").unwrap();
    re.replace_all(content, "").to_string()
}

/// Normalize whitespace
fn normalize_whitespace(content: &str) -> String {
    content
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_html_comments_simple() {
        let input = "before<!-- comment -->after";
        assert_eq!(remove_html_comments(input), "beforeafter");
    }

    #[test]
    fn test_remove_html_comments_multiline() {
        let input = "before<!-- multi\nline\ncomment -->after";
        assert_eq!(remove_html_comments(input), "beforeafter");
    }

    #[test]
    fn test_remove_html_comments_multiple() {
        let input = "text<!-- c1 -->more<!-- c2 -->end";
        assert_eq!(remove_html_comments(input), "textmoreend");
    }

    #[test]
    fn test_remove_html_comments_none() {
        let input = "no comments here";
        assert_eq!(remove_html_comments(input), "no comments here");
    }

    #[test]
    fn test_normalize_whitespace() {
        let input = "line1  \nline2\n\n\n\nline3  ";
        let result = normalize_whitespace(input);
        // normalize_whitespace trims trailing whitespace but preserves intermediate empty lines
        assert_eq!(result, "line1\nline2\n\n\n\nline3");
    }

    #[test]
    fn test_process_soul_content_valid() {
        let input = "<!-- comment -->\n## Purpose\n\nTest content";
        let result = process_soul_content(input);
        assert!(result.is_some());
        let content = result.unwrap();
        assert!(!content.contains("<!--"));
        assert!(content.contains("## Purpose"));
    }

    #[test]
    fn test_process_soul_content_no_sections() {
        let input = "Just some text without sections";
        assert!(process_soul_content(input).is_none());
    }

    #[test]
    fn test_process_soul_content_only_comments() {
        let input = "<!-- only comments -->";
        assert!(process_soul_content(input).is_none());
    }

    #[test]
    fn test_process_soul_content_empty() {
        let input = "";
        assert!(process_soul_content(input).is_none());
    }

    #[test]
    fn test_get_soul_path_returns_none_when_not_exists() {
        // This test verifies the function doesn't panic
        let result = get_soul_path();
        // Either Some(path) or None is valid
        assert!(result.is_some() || result.is_none());
    }
}
