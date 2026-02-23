//! Thinking tag processing
//!
//! Handles extraction and display of thinking content from LLM responses.

use regex::Regex;
use termimad::MadSkin;
use unicode_width::UnicodeWidthChar;

/// ANSI color for thinking text (light gray)
const THINKING_COLOR: &str = "\x1B[37m";
/// ANSI dim/faint style
const DIM_STYLE: &str = "\x1B[2m";
/// ANSI reset
const RESET: &str = "\x1B[0m";
/// Indentation for thinking content (in characters)
const THINKING_INDENT: usize = 2;

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

/// Display thinking content to stderr with light gray color and optional markdown
///
/// Checks the API-provided thinking field first, then falls back to
/// extracting from content.
///
/// # Arguments
/// * `content` - The full response content
/// * `thinking_field` - Optional thinking field from API response
/// * `render_markdown` - Whether to render as markdown
///
/// # Returns
/// The extracted thinking content (if any), for potential further use
pub fn display_thinking(
    content: &str,
    thinking_field: Option<&String>,
    render_markdown: bool,
) -> Option<String> {
    let thinking_content = thinking_field.cloned().or_else(|| {
        let processed = process_thinking(content);
        processed.thinking
    });

    if let Some(ref thinking) = thinking_content {
        eprintln!("{DIM_STYLE}{THINKING_COLOR}[Thinking]{RESET}");

        // Get terminal width, accounting for indentation
        let terminal_width = termimad::terminal_size().0 as usize;
        let wrap_width = terminal_width.saturating_sub(THINKING_INDENT);

        if render_markdown {
            // Use MadSkin with proper wrapping
            let skin = MadSkin::default();
            let wrapped = skin.text(thinking, Some(wrap_width));
            for line in wrapped.to_string().lines() {
                eprintln!("{DIM_STYLE}{THINKING_COLOR}  {}{RESET}", line);
            }
        } else {
            // Manual word wrap for plain text
            let wrapped = wrap_text(thinking, wrap_width);
            for line in wrapped.lines() {
                eprintln!("{DIM_STYLE}{THINKING_COLOR}  {}{RESET}", line);
            }
        }
        eprintln!();
    }

    thinking_content
}

/// Wrap plain text to a given width, breaking at word boundaries
fn wrap_text(text: &str, width: usize) -> String {
    if width < 10 {
        return text.to_string();
    }

    // Preserve paragraph breaks (double newlines in original)
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    if paragraphs.len() > 1 {
        return paragraphs
            .iter()
            .map(|p| wrap_single_paragraph(p, width))
            .collect::<Vec<_>>()
            .join("\n\n");
    }

    wrap_single_paragraph(text, width)
}

/// Wrap a single paragraph (no internal double newlines)
fn wrap_single_paragraph(text: &str, width: usize) -> String {
    if width < 10 {
        return text.to_string();
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_len = 0;

    for word in text.split_whitespace() {
        let word_len = word.chars().map(|c| c.width().unwrap_or(0)).sum::<usize>();

        if current_len == 0 {
            current_line.push_str(word);
            current_len = word_len;
        } else if current_len + 1 + word_len <= width {
            current_line.push(' ');
            current_line.push_str(word);
            current_len += 1 + word_len;
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            current_len = word_len;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines.join("\n")
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
