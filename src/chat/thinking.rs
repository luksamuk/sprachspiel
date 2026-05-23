//! Thinking tag processing
//!
//! Handles extraction and display of thinking content from LLM responses.
//!
//! # Architecture
//!
//! - `extract_thinking()` — Pure data extraction (no I/O). Returns the extracted
//!   thinking content without rendering. Use this when a `ChatView` is available.
//! - `display_thinking()` — Standalone function that extracts AND renders to stderr.
//!   Uses the standalone monochrome markdown renderer.
//!   Use only in contexts without `ChatView` (e.g., query mode).

#![expect(clippy::print_stderr)] // display_thinking() prints to stderr

use regex::Regex;

/// ANSI color for thinking text (light gray) — used by display_thinking() only
const THINKING_COLOR: &str = "\x1B[37m";
/// ANSI dim/faint style — used by display_thinking() only
const DIM_STYLE: &str = "\x1B[2m";
/// ANSI reset — used by display_thinking() only
const RESET: &str = "\x1B[0m";
/// Left border for thinking block content (rich mode)
const THINKING_BORDER: &str = "│ ";
/// Left border for thinking block content (plain mode, no ANSI, pipe-safe)
const PLAIN_THINKING_BORDER: &str = "| ";
/// Visual width of the thinking border prefix (shared by both modes)
const THINKING_BORDER_WIDTH: usize = 2;

/// Processed thinking content
#[derive(Debug, Clone)]
pub struct ProcessedContent {
    /// The thinking content (if any)
    pub thinking: Option<String>,
    /// The main response content (with thinking tags removed)
    pub content: String,
}

/// Process content with thinking tags
///
/// Handles multiple thinking tag formats:
/// - Unicode thinking tags (special character pair)
/// - HTML think tag: <think attr="...">...</think>
/// - Standard thinking: <thinking>...</thinking>
/// - Orphan closing: content before </thinking>
#[expect(clippy::expect_used)] // static regex patterns, cannot fail at runtime
pub fn process_thinking(content: &str) -> ProcessedContent {
    let mut thinking_parts = Vec::new();
    let mut result = content.to_string();

    // Unicode thinking tags (the special character used by some models)
    let unicode_pattern =
        Regex::new(r"(?s)\u{6beb}(.*?)\u{6beb}").expect("Invalid unicode thinking regex");

    // HTML-style <think> tags with attributes
    let html_think_tag =
        Regex::new(r"(?si)<think[^>]*>(.*?)</think>").expect("Invalid HTML think regex");

    // Standard <thinking> tags
    let html_thinking_tag =
        Regex::new(r"(?si)<thinking>(.*?)</thinking>").expect("Invalid HTML thinking regex");

    // Orphan closing tag (LFM bug): everything before </thinking>
    let orphan_closing =
        Regex::new(r"(?si)^(.*?)</thinking>").expect("Invalid orphan closing regex");

    // Extract unicode thinking content
    for cap in unicode_pattern.captures_iter(&result.clone()) {
        if let Some(think_content) = cap.get(1) {
            thinking_parts.push(think_content.as_str().trim().to_string());
        }
    }
    result = unicode_pattern.replace_all(&result, "").to_string();

    // Extract HTML think tag content
    for cap in html_think_tag.captures_iter(&result.clone()) {
        if let Some(think_content) = cap.get(1) {
            thinking_parts.push(think_content.as_str().trim().to_string());
        }
    }
    result = html_think_tag.replace_all(&result, "").to_string();

    // Extract HTML thinking tag content
    for cap in html_thinking_tag.captures_iter(&result.clone()) {
        if let Some(think_content) = cap.get(1) {
            thinking_parts.push(think_content.as_str().trim().to_string());
        }
    }
    result = html_thinking_tag.replace_all(&result, "").to_string();

    // Handle orphan closing tag (LFM bug)
    if let Some(cap) = orphan_closing.captures(&result) {
        if let Some(orphan_content) = cap.get(1) {
            let orphan_thinking = orphan_content.as_str().trim();
            if !orphan_thinking.is_empty() {
                thinking_parts.push(orphan_thinking.to_string());
            }
        }
        result = orphan_closing.replace_all(&result, "").to_string();
    }

    // Clean up any remaining </thinking> tags
    result = result.replace("</thinking>", "");
    result = result.trim().to_string();

    // Combine thinking parts
    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join("\n\n"))
    };

    ProcessedContent {
        thinking,
        content: result,
    }
}

/// Strip thinking tags from content (returns only the response content)
pub fn strip_thinking_tags(content: &str) -> String {
    process_thinking(content).content
}

/// Extract thinking content without rendering.
///
/// Checks the API-provided thinking field first, then falls back to
/// extracting thinking tags from content.
///
/// Use this when a `ChatView` is available — call `view.show_thinking(content)`
/// with the returned thinking string.
///
/// # Arguments
/// * `content` - The full response content
/// * `thinking_field` - Optional thinking field from API response
///
/// # Returns
/// The extracted thinking content (if any)
pub fn extract_thinking(content: &str, thinking_field: Option<&String>) -> Option<String> {
    thinking_field.cloned().or_else(|| {
        let processed = process_thinking(content);
        processed.thinking
    })
}

/// Display thinking content to stderr with 🧠 header and │ border.
///
/// **Standalone function** — uses the monochrome markdown renderer.
/// For contexts without `ChatView` (query mode).
///
/// # Arguments
/// * `content` - The full response content
/// * `thinking_field` - Optional thinking field from API response
/// * `render_markdown` - Whether to render as markdown (tables, headings, etc.)
/// * `use_plain` - If true, output plain text with no ANSI codes (for `--plain` / pipe-safe output)
///
/// # Returns
/// The extracted thinking content (if any), for potential further use
pub fn display_thinking(
    content: &str,
    thinking_field: Option<&String>,
    render_markdown: bool,
    use_plain: bool,
) -> Option<String> {
    // In quiet mode (Error level only), suppress thinking display
    if !log::log_enabled!(log::Level::Info) {
        // Still extract the thinking content so it can be processed,
        // but don't display it
        return extract_thinking(content, thinking_field);
    }

    let thinking_content = extract_thinking(content, thinking_field);

    if let Some(ref thinking) = thinking_content {
        let terminal_width = crossterm::terminal::size()
            .map(|(cols, _)| cols as usize)
            .unwrap_or(80);
        let wrap_width = terminal_width.saturating_sub(THINKING_BORDER_WIDTH);

        if use_plain {
            // Plain mode: no ANSI codes, pipe-safe output
            eprintln!("[Thinking]");
            let rendered = crate::markdown::standalone::render_markdown_plain(thinking, wrap_width);
            for line in rendered.lines() {
                eprintln!("{PLAIN_THINKING_BORDER}{line}");
            }
        } else if render_markdown {
            // Rich mode with markdown rendering
            eprintln!("{DIM_STYLE}{THINKING_COLOR}🧠 Thinking{RESET}");
            let rendered = crate::markdown::standalone::render_markdown(thinking, wrap_width);
            for line in rendered.lines() {
                eprintln!("{DIM_STYLE}{THINKING_COLOR}{THINKING_BORDER}{RESET}{line}");
            }
        } else {
            // Rich mode without markdown rendering
            eprintln!("{DIM_STYLE}{THINKING_COLOR}🧠 Thinking{RESET}");
            let rendered = crate::markdown::standalone::render_markdown_plain(thinking, wrap_width);
            for line in rendered.lines() {
                eprintln!("{DIM_STYLE}{THINKING_COLOR}{THINKING_BORDER}{RESET}{line}");
            }
        }
        eprintln!();
    }

    thinking_content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_thinking() {
        let input = "\u{6beb}This is thinking\u{6beb}\n\nThis is the response.";
        let processed = process_thinking(input);
        assert_eq!(processed.content, "This is the response.");
        assert_eq!(processed.thinking, Some("This is thinking".to_string()));
    }

    #[test]
    fn test_html_think_tag() {
        let input = "<think attr=\"value\">This is thinking</think>\n\nThis is the response.";
        let processed = process_thinking(input);
        assert_eq!(processed.content, "This is the response.");
        assert_eq!(processed.thinking, Some("This is thinking".to_string()));
    }

    #[test]
    fn test_html_thinking_tag() {
        let input = "<thinking>This is thinking</thinking>\n\nThis is the response.";
        let processed = process_thinking(input);
        assert_eq!(processed.content, "This is the response.");
        assert_eq!(processed.thinking, Some("This is thinking".to_string()));
    }

    #[test]
    fn test_orphan_closing_tag() {
        let input = "This is thinking content\n</thinking>This is the response.";
        let processed = process_thinking(input);
        assert_eq!(processed.content, "This is the response.");
        assert_eq!(
            processed.thinking,
            Some("This is thinking content".to_string())
        );
    }

    #[test]
    fn test_no_thinking_tags() {
        let input = "This is just a regular response.";
        let processed = process_thinking(input);
        assert_eq!(processed.content, input);
        assert_eq!(processed.thinking, None);
    }

    #[test]
    fn test_strip_only() {
        let input = "\u{6beb}This is thinking\u{6beb}\n\nThis is the response.";
        let result = strip_thinking_tags(input);
        assert_eq!(result, "This is the response.");
    }
}
