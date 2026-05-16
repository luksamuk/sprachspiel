//! Chat area component — scrollable message display
//!
//! Renders all chat messages in a continuous flow, differentiated by style
//! rather than bracketed labels:
//!
//! - **User**: `>>> ` prefix in bold cyan
//! - **Assistant**: no prefix, markdown rendered (blank line before)
//! - **Thinking**: `[Thinking]` label in dim cyan, content indented 4 spaces dim
//! - **Tool**: dim text (already has 🔧 from debug_tools), multi-line aware
//! - **System**: dim text, no prefix, multi-line aware
//! - **Error**: `✗` prefix in bold red
//! - **Banner**: responsive braille art layout
//!
//! Blank lines are inserted before Assistant and Thinking messages to
//! visually separate the user's prompt from the response.
//!
//! # Scrolling
//!
//! The chat area supports both auto-scroll (default) and manual scroll:
//! - **Auto-scroll**: `ScrollState::auto_scroll` is true, the viewport shows
//!   the bottom of content (newest messages). The scroll offset is computed
//!   from `count_wrapped_lines()` which accounts for word-wrapping, so the
//!   newest content is always visible regardless of terminal width.
//! - **Manual scroll**: When the user presses PageUp/PageDown/Home/End,
//!   `auto_scroll` is disabled and `manual_offset` controls how far above
//!   the bottom the viewport is positioned, using Paragraph::scroll().
//! - Any user input (typing a message) resets to auto-scroll.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use unicode_width::UnicodeWidthChar;

use super::super::markdown::{MarkdownTheme, render_markdown};
use super::super::styles;
use crate::chat::app::ScrollState;

/// Message type determines how a chat message is rendered.
///
/// Each variant maps to a distinct visual style in the chat area,
/// replacing the old bracketed `[Label]` format for a cleaner,
/// continuous-flow appearance.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    /// User message — displayed with `>>> ` prefix in bold cyan.
    User,
    /// Assistant response (complete) — no prefix, rendered as markdown.
    Assistant,
    /// Assistant response (streaming) — no prefix, plain text.
    AssistantStreaming,
    /// Thinking block — `[Thinking]` label in dim cyan, content indented dim.
    Thinking,
    /// Tool call/result — dim text, no prefix (content already has 🔧 emoji).
    Tool,
    /// System info — dim text, no prefix (tokens, compact, etc.).
    System,
    /// Error — `✗` prefix, bold red text.
    Error,
    /// Welcome banner — responsive braille art layout.
    Banner,
}

/// A single message in the chat area.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// What type of message this is (determines rendering style).
    pub msg_type: MessageType,
    /// Message content (plain text during streaming, markdown after completion).
    pub content: String,
}

impl ChatMessage {
    /// Create a user message (rendered with `>>> ` prefix in bold cyan).
    pub fn user(content: String) -> Self {
        Self {
            msg_type: MessageType::User,
            content,
        }
    }

    /// Create an assistant streaming message (plain text, no prefix).
    #[allow(dead_code)] // PR3: Will be used for streaming assistant messages in TUI
    pub fn assistant_streaming(content: String) -> Self {
        Self {
            msg_type: MessageType::AssistantStreaming,
            content,
        }
    }

    /// Create an assistant message (markdown after completion, no prefix).
    pub fn assistant_markdown(content: String) -> Self {
        Self {
            msg_type: MessageType::Assistant,
            content,
        }
    }

    /// Create a thinking block (`[Thinking]` label, content indented dim).
    pub fn thinking(content: String) -> Self {
        Self {
            msg_type: MessageType::Thinking,
            content,
        }
    }

    /// Create a tool call/result message (dim text, content has 🔧 emoji).
    pub fn tool(content: String) -> Self {
        Self {
            msg_type: MessageType::Tool,
            content,
        }
    }

    /// Create a system info message (dim text, no prefix).
    pub fn system(content: String) -> Self {
        Self {
            msg_type: MessageType::System,
            content,
        }
    }

    /// Create an error message (`✗` prefix, bold red).
    pub fn error(content: String) -> Self {
        Self {
            msg_type: MessageType::Error,
            content,
        }
    }

    /// Create a banner message (responsive braille art layout).
    ///
    /// The content should contain plain session info lines (no ANSI codes),
    /// one per line. The chat_area renderer handles responsive layout.
    pub fn banner(content: String) -> Self {
        Self {
            msg_type: MessageType::Banner,
            content,
        }
    }
}

/// Indent prefix for thinking block content (4 spaces).
const THINKING_INDENT: &str = "    ";

/// Word-wrap a line of text to fit within `width` visual columns,
/// breaking at spaces when possible. Unicode-aware: CJK characters
/// count as 2 columns, combining characters as 0, etc.
///
/// If a single word exceeds `width`, it is hard-broken at the column limit.
/// Returns owned Strings because Unicode-aware slicing cannot return &str.
fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }

    let visual_len: usize = line.chars().map(|c| c.width().unwrap_or(0)).sum();
    if visual_len <= width {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0usize;

    for word in line.split_whitespace() {
        let word_width: usize = word.chars().map(|c| c.width().unwrap_or(0)).sum();

        if current_width == 0 {
            // First word on the line
            if word_width <= width {
                current_line.push_str(word);
                current_width = word_width;
            } else {
                // Word is wider than available width — hard-break it
                let chunks = hard_break_word(word, width);
                result.extend(chunks);
            }
        } else if current_width + 1 + word_width <= width {
            // Word fits on current line with a space
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Word doesn't fit — push current line and start new one
            result.push(current_line);

            if word_width <= width {
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Word is wider than available width — hard-break it
                let chunks = hard_break_word(word, width);
                result.extend(chunks);
                current_line = String::new();
                current_width = 0;
            }
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    result
}

/// Detect markdown table syntax in content.
///
/// A markdown table has:
/// - At least one data line starting and ending with `|`
/// - At least one separator line (`|---|---|` or `|:---|:---|`)
#[cfg(test)]
fn content_has_table(content: &str) -> bool {
    let table_lines: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('|') && trimmed.ends_with('|')
        })
        .collect();

    if table_lines.len() < 2 {
        return false;
    }

    // Check for separator line: cells contain only `-`, `:`, or whitespace
    table_lines.iter().any(|line| {
        let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
        !inner.is_empty()
            && inner.split('|').all(|cell| {
                let trimmed = cell.trim();
                trimmed
                    .chars()
                    .all(|c| c == '-' || c == ':' || c.is_whitespace())
            })
    })
}

/// Count total visual lines after applying word-wrap to a slice of Lines.
///
/// Each Line is flattened to text and run through `wrap_line()` to determine
/// how many screen rows it will occupy at the given width. This accounts for
/// both space-based wrapping and hard-breaks on long words.
fn count_wrapped_lines(lines: &[Line], width: usize) -> usize {
    if width == 0 {
        return lines.len();
    }
    let mut total = 0usize;
    for line in lines {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let wrapped = wrap_line(&text, width);
        total += wrapped.len().max(1);
    }
    total
}

/// Hard-break a word that exceeds `width` visual columns.
///
/// Breaks at Unicode character boundaries, splitting the word into chunks
/// that each fit within `width` columns (accounting for CJK double-width).
fn hard_break_word(word: &str, width: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut chunk = String::new();
    let mut chunk_width = 0usize;

    for ch in word.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if chunk_width + ch_width > width && !chunk.is_empty() {
            result.push(chunk);
            chunk = String::new();
            chunk_width = 0;
        }
        chunk.push(ch);
        chunk_width += ch_width;
    }

    if !chunk.is_empty() {
        result.push(chunk);
    }

    result
}

/// Render the chat area component.
///
/// Displays all messages in a continuous flow with type-based styling.
/// Blank lines are inserted before Assistant and Thinking messages to
/// visually separate the user's prompt from the response.
///
/// # Scrolling
///
/// The `ScrollState` determines the viewport position:
/// - Auto-scroll: shows the newest messages at the bottom. The scroll
///   offset is computed from the wrapped line count (via `count_wrapped_lines()`)
///   which accounts for word-wrapping at the terminal width.
/// - Manual scroll: uses `Paragraph::scroll()` with offset from top
///
/// **Why compute wrapped lines manually?** `Paragraph::wrap()` expands each
/// source line into potentially multiple screen rows. We replicate the same
/// wrap logic in `count_wrapped_lines()` so the scroll offset matches the
/// actual rendered layout.
pub fn render(
    f: &mut Frame,
    area: Rect,
    messages: &[ChatMessage],
    scroll_state: &ScrollState,
    theme: MarkdownTheme,
) {
    let mut lines: Vec<Line> = Vec::new();
    let available_width = area.width as usize;

    for msg in messages {
        match msg.msg_type {
            MessageType::User => {
                // ">>> " prefix in bold cyan, content in bold cyan
                lines.push(Line::from(vec![
                    Span::styled(">>> ", styles::bold_cyan()),
                    Span::styled(&msg.content, styles::bold_cyan()),
                ]));
            }
            MessageType::Assistant => {
                // Blank line before response for visual separation
                if !lines.is_empty() {
                    lines.push(Line::raw(String::new()));
                }
                // No prefix — markdown rendered
                let rendered = render_markdown(&msg.content, theme, available_width);
                lines.extend(rendered.lines);
            }
            MessageType::AssistantStreaming => {
                // Blank line before response for visual separation
                if !lines.is_empty() {
                    lines.push(Line::raw(String::new()));
                }
                // No prefix — plain text during streaming
                for line in msg.content.lines() {
                    lines.push(Line::raw(line.to_string()));
                }
            }
            MessageType::Thinking => {
                // Blank line before thinking block for visual separation
                if !lines.is_empty() {
                    lines.push(Line::raw(String::new()));
                }
                // "[Thinking]" label in dim cyan
                lines.push(Line::from(Span::styled("[Thinking]", styles::dim_cyan())));
                // Content indented with responsive word-wrap
                let content_style = styles::thinking_content_style();
                let content_width = available_width.saturating_sub(THINKING_INDENT.len());
                for source_line in msg.content.lines() {
                    for wrapped_line in wrap_line(source_line, content_width) {
                        lines.push(Line::from(Span::styled(
                            format!("{THINKING_INDENT}{wrapped_line}"),
                            content_style,
                        )));
                    }
                }
            }
            MessageType::Tool => {
                // Dim text, no prefix — content already has 🔧 from debug_tools
                // Multi-line aware: split on \n so emojis and text render correctly
                for line in msg.content.lines() {
                    lines.push(Line::from(Span::styled(line.to_string(), styles::dim())));
                }
            }
            MessageType::System => {
                // Dim text, no prefix
                // Multi-line aware: split on \n so emojis and text render correctly
                for line in msg.content.lines() {
                    lines.push(Line::from(Span::styled(line.to_string(), styles::dim())));
                }
            }
            MessageType::Error => {
                // "✗" prefix in bold red, content in red
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("✗ ", styles::error_style()),
                        Span::styled(line, styles::error_style()),
                    ]));
                }
            }
            MessageType::Banner => {
                // Banner: responsive layout (braille art)
                let session_lines: Vec<String> = msg.content.lines().map(String::from).collect();
                let banner_lines = super::super::banner::build_banner_lines(area, &session_lines);
                lines.extend(banner_lines);
            }
        }
        // No automatic blank line between messages — visual separation
        // is handled per type above (blank before Assistant/Thinking only)
    }

    // Calculate scroll offset from the top of content.
    //
    // We compute the total rendered line count AFTER wrap using
    // `count_wrapped_lines()`, then use `effective_scroll_from_top()`
    // to get the precise offset that shows the bottom (newest) content.
    // This is accurate because it mirrors the same wrap logic that
    // ratatui's Paragraph::wrap() would apply at render time.
    let wrapped_total = count_wrapped_lines(&lines, area.width as usize);
    let visible_height = area.height as usize;
    let scroll_from_top = scroll_state.effective_scroll_from_top(wrapped_total, visible_height);

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((scroll_from_top, 0));

    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create Vec<String> for comparison with wrap_line output
    macro_rules! sv {
        ($($s:expr),* $(,)?) => { vec![$($s.to_string()),*] };
    }

    #[test]
    fn test_wrap_line_short() {
        // Line fits within width — returned as-is
        let result = wrap_line("hello", 80);
        assert_eq!(result, sv!["hello"]);
    }

    #[test]
    fn test_wrap_line_break_at_space() {
        // "hello world foo" with width=11:
        // "hello" (5) + " " (1) + "world" (5) = 11 → "hello world" fits
        // But our algorithm puts "hello" on line 1, then "world foo" on line 2
        // Wait — let's trace it: first word "hello" (5), fits. Then " world" (6) → 5+1+5=11 ≤ 11.
        // So "hello world" fits in 11 cols. Then " foo" → need new line: "foo" (3).
        // Result: ["hello world", "foo"]
        let result = wrap_line("hello world foo", 11);
        assert_eq!(result, sv!["hello world", "foo"]);
    }

    #[test]
    fn test_wrap_line_space_at_boundary() {
        // "one two three" with width=7:
        // "one" (3) → fits. " two" (4) → 3+1+3=7 ≤ 7 → "one two" fits.
        // " three" (6) → need new line: "three" (5).
        // Result: ["one two", "three"]
        let result = wrap_line("one two three", 7);
        assert_eq!(result, sv!["one two", "three"]);
    }

    #[test]
    fn test_wrap_line_no_space() {
        // No space — hard break using Unicode char boundaries
        let result = wrap_line("abcdefghij", 5);
        assert_eq!(result, sv!["abcde", "fghij"]);
    }

    #[test]
    fn test_wrap_line_multiple_breaks() {
        // "one two three four five" with width=8:
        // "one" (3) → fits. " two" (4) → 3+1+3=7 ≤ 8 → "one two" fits.
        // " three" (6) → need new line: "three" (5).
        // " four" (5) → 5+1+4=10 > 8 → new line: "four" (4).
        // " five" (5) → 4+1+4=9 > 8 → new line: "five" (4).
        let result = wrap_line("one two three four five", 8);
        assert_eq!(result, sv!["one two", "three", "four", "five"]);
    }

    #[test]
    fn test_wrap_line_empty() {
        let result = wrap_line("", 80);
        assert_eq!(result, sv![""]);
    }

    #[test]
    fn test_wrap_line_zero_width() {
        // Zero width should return as-is (cannot wrap)
        let result = wrap_line("hello", 0);
        assert_eq!(result, sv!["hello"]);
    }

    #[test]
    fn test_wrap_line_unicode() {
        // Unicode-aware: "olá mundo" — "olá" is 3 chars (4 bytes), "mundo" is 5 chars
        // Width 10: "olá" (3 cols) + " " + "mundo" (5 cols) = 9 cols → fits
        let result = wrap_line("olá mundo", 10);
        assert_eq!(result, sv!["olá mundo"]);

        // Width 5: "olá" (3 cols) + " " + "mundo" (5 cols) → "olá" then "mundo"
        let result = wrap_line("olá mundo", 5);
        assert_eq!(result, sv!["olá", "mundo"]);
    }

    #[test]
    fn test_wrap_line_cjk() {
        // CJK characters are 2 columns wide
        // "日本語" = 3 chars × 2 cols = 6 visual cols, "test" = 4 cols
        // Width 8: "日本語" (6) + " " (1) + "t" (1) = 8 → "日本語 test" doesn't fit
        // "日本語" (6) fits in 8, then "test" (4) fits in 8
        let result = wrap_line("日本語 test", 8);
        assert_eq!(result, sv!["日本語", "test"]);
    }

    #[test]
    fn test_content_has_table_valid() {
        let table = "| A | B |\n|---|---|\n| 1 | 2 |";
        assert!(content_has_table(table));
    }

    #[test]
    fn test_content_has_table_aligned() {
        let table = "| A | B |\n|:---:|:---|\n| 1 | 2 |";
        assert!(content_has_table(table));
    }

    #[test]
    fn test_content_has_table_with_content() {
        // Table embedded in prose
        let mixed = "Here is some data:\n\n| Name | Value |\n|------|-------|\n| Foo  | 42    |\n\nMore text after.";
        assert!(content_has_table(mixed));
    }

    #[test]
    fn test_content_has_table_false_no_separator() {
        // Missing separator line
        let text = "| A | B |\n| 1 | 2 |";
        assert!(!content_has_table(text));
    }

    #[test]
    fn test_content_has_table_false_single_line() {
        // Only one table-like line
        let text = "| A | B |";
        assert!(!content_has_table(text));
    }

    #[test]
    fn test_content_has_table_false_plain_text() {
        assert!(!content_has_table("hello world"));
        assert!(!content_has_table("use | grep | sort"));
    }

    #[test]
    fn test_message_type_variants() {
        assert_eq!(ChatMessage::user("hi".into()).msg_type, MessageType::User);
        assert_eq!(
            ChatMessage::assistant_markdown("resp".into()).msg_type,
            MessageType::Assistant
        );
        assert_eq!(
            ChatMessage::thinking("think".into()).msg_type,
            MessageType::Thinking
        );
        assert_eq!(ChatMessage::tool("tool".into()).msg_type, MessageType::Tool);
        assert_eq!(
            ChatMessage::system("info".into()).msg_type,
            MessageType::System
        );
        assert_eq!(
            ChatMessage::error("err".into()).msg_type,
            MessageType::Error
        );
        assert_eq!(
            ChatMessage::banner("banner".into()).msg_type,
            MessageType::Banner
        );
    }
}
