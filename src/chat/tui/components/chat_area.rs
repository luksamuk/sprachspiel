//! Chat area component — scrollable message display
//!
//! Renders all chat messages in a continuous flow, differentiated by style
//! rather than bracketed labels:
//!
//! - **User**: `>>> ` prefix in bold cyan
//! - **Assistant**: no prefix, markdown rendered (blank line before)
//! - **Thinking**: `🧠 Thinking` header in dim cyan, markdown content with
//!   `│` left border in dim cyan, word-wrapped with style preservation
//! - **Tool**: dim markdown (already has 🔧 from debug_tools), responsive width
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

use super::super::markdown::{MarkdownTheme, render_markdown};
use super::super::styles;
use super::super::wrap::{wrap_line, wrap_styled_line};
use super::chat_selection::{ChatSelection, selection_style};
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
    /// Assistant response (streaming) — no prefix, markdown rendered incrementally.
    AssistantStreaming,
    /// Thinking block — `🧠 Thinking` header in dim cyan, content with `│` left border.
    Thinking,
    /// Tool call/result — dim markdown, no prefix (content already has 🔧 emoji).
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

    /// Create an assistant streaming message (markdown rendered incrementally, no prefix).
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

    /// Create a thinking block (`🧠 Thinking` header, content with `│` left border).
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

/// Left border prefix for thinking block content.
///
/// Each line of thinking content is prefixed with `│ ` (vertical bar
/// + space), creating a visual block enclosure. The `│` character
///   matches the box-drawing `BD_VLINE` used in table rendering.
const THINKING_BORDER_PREFIX: &str = "│ ";

/// Visual width of the thinking border prefix (2 columns: `│` + space).
const THINKING_BORDER_WIDTH: usize = 2;

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

/// Count total visual lines after applying word-wrap to a slice of line texts.
///
/// Each string is run through `wrap_line()` to determine how many screen
/// rows it will occupy at the given width. This accounts for both
/// space-based wrapping and hard-breaks on long words.
fn count_wrapped_lines(texts: &[&str], width: usize) -> usize {
    if width == 0 {
        return texts.len();
    }
    let mut total = 0usize;
    for text in texts {
        let wrapped = wrap_line(text, width);
        total += wrapped.len().max(1);
    }
    total
}

/// Metadata returned by `render()` for mouse/selection integration.
///
/// Contains the visual line strings (after word-wrap) and the scroll
/// offset, which are needed for mapping mouse positions to content
/// and extracting selected text.
pub struct RenderMetadata {
    /// Flat list of visual line strings (after word-wrap), in render order
    pub visual_lines: Vec<String>,
    /// Scroll offset from the top of content (in visual lines)
    pub scroll_from_top: u16,
}

/// Build the styled `Line` vector from messages (shared between render and tests).
///
/// This is the core content pipeline: each message type adds its styled
/// lines to the vector, which is then wrapped, scrolled, and rendered.
fn build_lines(
    messages: &[ChatMessage],
    theme: MarkdownTheme,
    available_width: usize,
) -> Vec<Line<'_>> {
    let mut lines: Vec<Line> = Vec::new();

    for msg in messages {
        match msg.msg_type {
            MessageType::User => {
                // ">>> " prefix in bold cyan, content in bold cyan
                lines.push(Line::from(vec![
                    Span::styled(">>> ", styles::bold_cyan()),
                    Span::styled(msg.content.clone(), styles::bold_cyan()),
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
                // Render markdown incrementally during streaming (same as Thinking blocks)
                let rendered = render_markdown(&msg.content, theme, available_width);
                lines.extend(rendered.lines);
            }
            MessageType::Thinking => {
                // Blank line before thinking block for visual separation
                if !lines.is_empty() {
                    lines.push(Line::raw(String::new()));
                }
                // Header: 🧠 Thinking
                lines.push(Line::from(Span::styled(
                    "🧠 Thinking",
                    styles::thinking_header_style(),
                )));
                // Render thinking content as markdown, with │ left border.
                // Content width is reduced by the border prefix width.
                let content_width = available_width.saturating_sub(THINKING_BORDER_WIDTH);
                let rendered = render_markdown(&msg.content, theme, content_width);
                let border_span =
                    Span::styled(THINKING_BORDER_PREFIX, styles::thinking_border_style());
                for render_line in rendered.lines {
                    // Wrap each styled line to content_width (preserving styles),
                    // then prepend the border span to each sub-line.
                    let wrapped = wrap_styled_line(render_line, content_width);
                    for sub_line in wrapped {
                        let mut spans = vec![border_span.clone()];
                        spans.append(&mut sub_line.spans.into_iter().collect());
                        lines.push(Line::from(spans));
                    }
                }
                // Blank line after thinking block for visual separation
                lines.push(Line::raw(String::new()));
            }
            MessageType::Tool => {
                // Render tool output as markdown with dim style overlay.
                // Tool output may contain code blocks, lists, tables — render
                // them properly while keeping visual distinction from assistant
                // content by applying dim modifier over all styles.
                let rendered = render_markdown(&msg.content, theme, available_width);
                let dim_style = styles::dim();
                for render_line in rendered.lines {
                    // Propagate Line.style to each Span before applying dim overlay.
                    // tui-markdown renders headings as Line { spans: [Span::raw(...)],
                    // style: heading_style } where heading_style carries color, bold,
                    // and underline. Line.style acts as fallback for Spans with
                    // Style::default(). We must merge it into each Span explicitly
                    // before discarding the Line (since Line::from() loses Line.style).
                    let base_style = render_line.style;
                    let dimmed_spans: Vec<Span<'_>> = render_line
                        .spans
                        .into_iter()
                        .map(|span| {
                            // Merge: Line.style (heading/formatting fallback)
                            //   → span.style (inline style override)
                            //   → dim_style (dim overlay)
                            Span::styled(
                                span.content,
                                base_style.patch(span.style).patch(dim_style),
                            )
                        })
                        .collect();
                    lines.push(Line::from(dimmed_spans));
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
                // Note: banners don't support selection (art layout is complex)
                // We need a Rect for banner but we only have width here.
                // Use a minimal Rect with the right width.
                let dummy_area = Rect::new(0, 0, available_width as u16, 20);
                let session_lines: Vec<String> = msg.content.lines().map(String::from).collect();
                let banner_lines =
                    super::super::banner::build_banner_lines(dummy_area, &session_lines);
                lines.extend(banner_lines);
            }
        }
        // No automatic blank line between messages — visual separation
        // is handled per type above (blank before Assistant/Thinking only)
    }

    lines
}

/// Apply selection highlight to a vector of `Line`s.
///
/// Modifies the style of spans that fall within the selection range.
/// The selection is in visual-line coordinates: (line_index, char_offset).
/// Each Line's text is flattened to compute character ranges.
fn apply_selection_highlight(lines: &mut Vec<Line>, selection: &ChatSelection) {
    if !selection.is_active() {
        return;
    }

    let (start_line, start_col) = selection.selection_start();
    let (end_line, end_col) = selection.selection_end();

    for (line_idx, line) in lines.iter_mut().enumerate() {
        if line_idx < start_line || line_idx > end_line {
            continue;
        }

        // Calculate the character range to highlight on this line
        let line_start_col = if line_idx == start_line { start_col } else { 0 };
        let line_end_col = if line_idx == end_line {
            end_col
        } else {
            // Highlight to end of line
            line.spans
                .iter()
                .map(|s| s.content.chars().count())
                .sum::<usize>()
        };

        if line_start_col >= line_end_col {
            continue;
        }

        // Rebuild the line with selection highlight applied
        let mut new_spans: Vec<Span> = Vec::new();
        let mut char_pos = 0;

        for span in line.spans.iter() {
            let span_len = span.content.chars().count();
            let span_start = char_pos;
            let span_end = char_pos + span_len;

            if span_end <= line_start_col || span_start >= line_end_col {
                // Outside selection — keep as-is
                new_spans.push(span.clone());
            } else {
                // This span overlaps with the selection
                let rel_start = line_start_col.saturating_sub(span_start);
                let rel_end = line_end_col.saturating_sub(span_start).min(span_len);

                // Part before selection
                if rel_start > 0 {
                    let before: String = span.content.chars().take(rel_start).collect();
                    new_spans.push(Span::styled(before, span.style));
                }

                // Selected part
                let selected: String = span
                    .content
                    .chars()
                    .skip(rel_start)
                    .take(rel_end - rel_start)
                    .collect();
                new_spans.push(Span::styled(selected, selection_style()));

                // Part after selection
                if rel_end < span_len {
                    let after: String = span.content.chars().skip(rel_end).collect();
                    new_spans.push(Span::styled(after, span.style));
                }
            }

            char_pos += span_len;
        }

        *line = Line::from(new_spans);
    }
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
/// # Selection
///
/// When `ChatSelection` is active, selected ranges are highlighted with
/// a distinct background color. The `visual_lines` in the returned
/// `RenderMetadata` are the plain-text strings for each visual line,
/// used for mouse mapping and text extraction.
///
/// **Why compute wrapped lines manually?** `Paragraph::wrap()` expands each
/// source line into potentially multiple screen rows. We replicate the same
/// wrap logic in `count_wrapped_lines()` so the scroll offset matches the
/// actual rendered layout.
pub fn render(
    f: &mut Frame,
    area: Rect,
    messages: &[ChatMessage],
    scroll_state: &mut ScrollState,
    theme: MarkdownTheme,
    selection: &ChatSelection,
) -> RenderMetadata {
    let available_width = area.width as usize;
    let mut lines = build_lines(messages, theme, available_width);

    // Build visual_lines BEFORE applying selection highlight (plain text for extraction).
    // Trim trailing whitespace from each line so that code block padding (styled
    // spaces that extend the background to the right edge) does not pollute
    // clipboard copies. Code content whitespace is preserved since only the
    // right edge is stripped.
    let visual_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            text.trim_end().to_string()
        })
        .collect();

    // Apply selection highlight (modifies span styles)
    apply_selection_highlight(&mut lines, selection);

    // Calculate scroll offset from the top of content.
    let wrapped_total = count_wrapped_lines(
        &visual_lines.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        area.width as usize,
    );
    let visible_height = area.height as usize;
    // Clamp manual_offset to valid range [0, max_scroll] before computing
    // effective offset. This prevents "overscroll" accumulation from rapid
    // mouse wheel scrolling — without clamping, scroll_up() can grow
    // manual_offset well beyond max_scroll, making scroll_down() feel
    // sluggish because each tick only subtracts a small number.
    scroll_state.clamp_offset(wrapped_total, visible_height);
    let scroll_from_top = scroll_state.effective_scroll_from_top(wrapped_total, visible_height);

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((scroll_from_top, 0));

    f.render_widget(paragraph, area);

    RenderMetadata {
        visual_lines,
        scroll_from_top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
