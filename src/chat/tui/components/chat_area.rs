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
//!   from `count_ratatui_wrapped_lines()` which uses grapheme-level width
//!   (matching ratatui's `WordWrapper`), so the newest content is always
//!   visible regardless of terminal width — even with emoji ZWJ sequences,
//!   flag emojis, and CJK characters.
//! - **Manual scroll**: When the user presses PageUp/PageDown/Home/End,
//!   `auto_scroll` is disabled and `manual_offset` controls how far above
//!   the bottom the viewport is positioned, using Paragraph::scroll().
//! - Any user input (typing a message) resets to auto-scroll.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::super::markdown::{MarkdownTheme, render_markdown, render_markdown_streaming};
use super::super::styles;
use super::super::wrap::wrap_styled_line;
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

/// Count total visual lines after word-wrap, matching ratatui's `WordWrapper`.
///
/// This replicates the exact wrapping algorithm used by `Paragraph::wrap()`
/// internally (ratatui 0.29 `WordWrapper` with `trim: false`), so the scroll
/// offset always matches the actual rendered layout.
///
/// Key difference from the old `count_wrapped_lines()`: this function uses
/// grapheme-level width via `UnicodeWidthStr::width()` (same as ratatui),
/// not char-level width via `UnicodeWidthChar::width()`. This correctly
/// handles emoji ZWJ sequences (🇧🇷 width 2, not 0), flag emojis, and
/// combining characters.
///
/// Count the number of visual lines after ratatui word-wrapping.
///
/// Used only in tests — production code uses `wrap_visual_lines()` which
/// returns the actual wrapped strings plus a source-line map.
#[cfg(test)]
fn count_ratatui_wrapped_lines(lines: &[Line<'_>], max_width: u16) -> usize {
    if max_width < 1 {
        return lines.len();
    }
    let max_width = max_width as usize;
    let mut total = 0usize;

    for line in lines {
        let graphemes: Vec<&str> = line
            .spans
            .iter()
            .flat_map(|span| span.content.as_ref().graphemes(true))
            .collect();
        total += count_word_wrapped_graphemes(&graphemes, max_width, false);
    }

    total
}

/// Word-wrap a sequence of grapheme strings and return the number of visual
/// lines produced. Mirrors ratatui 0.29 `WordWrapper::process_input()`.
///
/// Used only in tests — production code uses `wrap_line_graphemes()`.
#[cfg(test)]
fn count_word_wrapped_graphemes(graphemes: &[&str], max_width: usize, trim: bool) -> usize {
    if max_width == 0 {
        return 1;
    }

    let mut line_width: usize = 0;
    let mut word_width: usize = 0;
    let mut whitespace_width: usize = 0;
    let mut non_whitespace_previous = false;
    let mut count = 0;

    for grapheme in graphemes {
        let is_whitespace = grapheme.chars().all(|c| c.is_whitespace());
        let symbol_width = grapheme.width();

        // Ignore graphemes wider than the line limit (ratatui drops them)
        if symbol_width > max_width {
            continue;
        }

        let word_found = non_whitespace_previous && is_whitespace;
        let trimmed_overflow = line_width == 0 && trim && word_width + symbol_width > max_width;
        let whitespace_overflow =
            line_width == 0 && trim && whitespace_width + symbol_width > max_width;
        let untrimmed_overflow =
            line_width == 0 && !trim && word_width + whitespace_width + symbol_width > max_width;

        if word_found || trimmed_overflow || whitespace_overflow || untrimmed_overflow {
            // Append finished word to current line
            if line_width > 0 || !trim {
                line_width += whitespace_width;
            }
            line_width += word_width;
            word_width = 0;
            whitespace_width = 0;
        }

        let line_full = line_width >= max_width;
        let pending_word_overflow =
            symbol_width > 0 && line_width + whitespace_width + word_width >= max_width;

        if line_full || pending_word_overflow {
            // Finish current wrapped line
            count += 1;
            line_width = 0;

            // With trim: drop whitespace up to end of line (ratatui pops from
            // front of pending_whitespace; we reset entirely).
            whitespace_width = 0;

            // Don't count first whitespace toward next word
            if is_whitespace && whitespace_width == 0 {
                non_whitespace_previous = false;
                continue;
            }
        }

        if is_whitespace {
            whitespace_width += symbol_width;
        } else {
            word_width += symbol_width;
        }

        non_whitespace_previous = !is_whitespace;
    }

    // Remaining content
    let has_pending = word_width > 0 || whitespace_width > 0 || line_width > 0;
    // Push remaining text as final line
    if line_width > 0 || word_width > 0 || (!trim && whitespace_width > 0) {
        count += 1;
    } else if !has_pending && count == 0 {
        // Empty line
        count = 1;
    }

    count.max(1)
}

/// Build wrapped visual lines and a source-line mapping.
///
/// For each source `Line`, word-wrap its text content at `max_width` and
/// produce one `String` per display row. Also produce a parallel `Vec<usize>`
/// mapping each display row back to its source line index.
///
/// This aligns `visual_lines` indices with `scroll_from_top` (both in
/// display-row space), fixing the mouse offset bug where wrapped lines
/// caused coordinate mismatch between mouse positions and content.
fn wrap_visual_lines(lines: &[Line<'_>], max_width: u16) -> (Vec<String>, Vec<usize>) {
    let max_width = max_width.max(1) as usize;
    let mut visual_lines = Vec::new();
    let mut source_line_map = Vec::new();

    for (source_idx, line) in lines.iter().enumerate() {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let trimmed = text.trim_end();

        if trimmed.is_empty() {
            // Empty line — still produces one display row
            visual_lines.push(String::new());
            source_line_map.push(source_idx);
        } else {
            // Word-wrap this line and produce one entry per display row
            let graphemes: Vec<&str> = trimmed.graphemes(true).collect();
            let wrapped = wrap_line_graphemes(&graphemes, max_width);
            for wrapped_line in wrapped {
                visual_lines.push(wrapped_line);
                source_line_map.push(source_idx);
            }
        }
    }

    (visual_lines, source_line_map)
}

/// Word-wrap a line (given as grapheme slices) at `max_width`.
///
/// Returns a `Vec<String>` where each entry is one display row.
/// Mirrors ratatui's `Wrap { trim: false }` wrapping behavior.
fn wrap_line_graphemes(graphemes: &[&str], max_width: usize) -> Vec<String> {
    if max_width == 0 || graphemes.is_empty() {
        return vec![String::new()];
    }

    let mut rows = Vec::new();
    let mut current_width = 0usize;
    let mut current_line = String::new();

    for grapheme in graphemes {
        let gw = grapheme.width();

        // Skip graphemes wider than max_width (ratatui drops them)
        if gw > max_width {
            continue;
        }

        if current_width + gw > max_width && !current_line.is_empty() {
            // Word wrap — start a new display row
            rows.push(current_line.trim_end().to_string());
            current_line.clear();
            current_width = 0;
        }

        current_line.push_str(grapheme);
        current_width += gw;
    }

    // Push remaining content as final display row
    if !current_line.is_empty() {
        rows.push(current_line.trim_end().to_string());
    } else if rows.is_empty() {
        // Edge case: all content was wider than max_width
        rows.push(String::new());
    }

    rows
}
///
/// Contains the visual line strings (after word-wrap) and the scroll
/// offset, which are needed for mapping mouse positions to content
/// and extracting selected text.
/// Metadata returned by `render()` for use in mouse mapping and text selection.
///
/// **Visual lines** are in display-row space (one entry per wrapped display row),
/// making them directly indexable by `local_row + scroll_from_top`.
///
/// **Source line map** maps each display row to the original source `Line` index,
/// enabling selection highlight to find the correct `Line` for a given display row.
pub struct RenderMetadata {
    /// Flat list of visual line strings (one per wrapped display row), in render order.
    /// Each entry corresponds to exactly one display row, so index alignment
    /// with scroll offset is correct for mouse mapping.
    pub visual_lines: Vec<String>,
    /// Scroll offset from the top of content (in display rows = visual_lines indices)
    pub scroll_from_top: u16,
    /// Maps each display row index to the source line index in `lines`.
    /// Used by selection highlight to find the correct `Line` for a given display row.
    pub source_line_map: Vec<usize>,
}

/// Build the styled `Line` vector from messages (shared between render and tests).
///
/// This is the core content pipeline: each message type adds its styled
/// lines to the vector, which is then wrapped, scrolled, and rendered.
fn build_lines(
    messages: &[ChatMessage],
    theme: MarkdownTheme,
    style_enabled: bool,
    available_width: usize,
) -> Vec<Line<'_>> {
    let mut lines: Vec<Line> = Vec::new();

    for msg in messages {
        match msg.msg_type {
            MessageType::User => {
                // ">>> " prefix on first line, "    " continuation on
                // subsequent lines. Multi-line input (Shift+Enter) preserves
                // \n in the content but ratatui Line does not render embedded
                // newlines — each visual line must be a separate Line.
                let content_lines = msg.content.split('\n').collect::<Vec<_>>();
                for (i, content_line) in content_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled(">>> ", styles::bold_cyan()),
                            Span::styled((*content_line).to_string(), styles::bold_cyan()),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("    ", styles::bold_cyan()),
                            Span::styled((*content_line).to_string(), styles::bold_cyan()),
                        ]));
                    }
                }
            }
            MessageType::Assistant => {
                // Blank line before response for visual separation
                if !lines.is_empty() {
                    lines.push(Line::raw(String::new()));
                }
                // No prefix — markdown rendered
                let rendered = render_markdown(&msg.content, theme, style_enabled, available_width);
                lines.extend(rendered.lines);
            }
            MessageType::AssistantStreaming => {
                // Blank line before response for visual separation
                if !lines.is_empty() {
                    lines.push(Line::raw(String::new()));
                }
                // Streaming mode: Mermaid blocks shown as code blocks (deferred rendering)
                let rendered =
                    render_markdown_streaming(&msg.content, theme, style_enabled, available_width);
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
                // Streaming mode: Mermaid blocks shown as code blocks (deferred rendering)
                let content_width = available_width.saturating_sub(THINKING_BORDER_WIDTH);
                let rendered =
                    render_markdown_streaming(&msg.content, theme, style_enabled, content_width);
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
                // Tool messages come in two flavors:
                // 1. Call indicators (🔧 name(args), ⚡ cmd, 📝 note, etc.) — these
                //    show WHAT the tool is doing and should be VISIBLE (not dimmed).
                // 2. Result lines (✓ Result:, 📤 name result:) — these are the
                //    tool's output and should be DIMMED to distinguish from the
                //    assistant's own content.
                //
                // Tool call indicators start with emoji prefixes like 🔧⚡📝💾📄⏭🗑📖.
                // Tool results start with ✓ or 📤. We render the entire message as
                // either visible or dim based on the first line's prefix.
                let is_call_indicator = msg.content.starts_with('🔧')
                    || msg.content.starts_with('⚡')
                    || msg.content.starts_with('📝')
                    || msg.content.starts_with('💾')
                    || msg.content.starts_with('📄')
                    || msg.content.starts_with('⏭')
                    || msg.content.starts_with('🗑')
                    || msg.content.starts_with('📖')
                    || msg.content.starts_with('⚡')
                    || msg.content.starts_with('👍')
                    || msg.content.starts_with('👎')
                    || msg.content.starts_with('✎');

                if is_call_indicator {
                    // Tool call indicator — render as normal (visible) markdown.
                    // No dim overlay so the user can clearly see what the tool is doing.
                    let rendered =
                        render_markdown(&msg.content, theme, style_enabled, available_width);
                    for render_line in rendered.lines {
                        let base_style = render_line.style;
                        let spans: Vec<Span<'_>> = render_line
                            .spans
                            .into_iter()
                            .map(|span| Span::styled(span.content, base_style.patch(span.style)))
                            .collect();
                        lines.push(Line::from(spans));
                    }
                } else {
                    // Tool result — render as dimmed markdown for visual distinction
                    // from assistant content. The result is informative but not the
                    // primary content the user is reading.
                    let rendered =
                        render_markdown(&msg.content, theme, style_enabled, available_width);
                    let dim_style = styles::dim();
                    for render_line in rendered.lines {
                        let base_style = render_line.style;
                        let dimmed_spans: Vec<Span<'_>> = render_line
                            .spans
                            .into_iter()
                            .map(|span| {
                                Span::styled(
                                    span.content,
                                    base_style.patch(span.style).patch(dim_style),
                                )
                            })
                            .collect();
                        lines.push(Line::from(dimmed_spans));
                    }
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
/// The selection is in display-row coordinates: (display_row, char_offset).
/// The `source_line_map` converts display row indices to source line indices
/// so the highlight is applied to the correct `Line` objects.
fn apply_selection_highlight(
    lines: &mut [Line<'_>],
    selection: &ChatSelection,
    source_line_map: &[usize],
) {
    if !selection.is_active() || source_line_map.is_empty() || lines.is_empty() {
        return;
    }

    let (start_display, start_col) = selection.selection_start();
    let (end_display, end_col) = selection.selection_end();

    // Clamp display row coordinates to valid range
    let start_display = start_display.min(source_line_map.len().saturating_sub(1));
    let end_display = end_display.min(source_line_map.len().saturating_sub(1));

    if start_display > end_display {
        return;
    }

    // Convert display rows to source line indices
    let start_source = source_line_map[start_display];
    let end_source = source_line_map[end_display];

    // For each source line in the selection range, compute highlight columns
    // based on which display rows of this source line are selected.
    for (source_idx, line) in lines.iter_mut().enumerate() {
        if source_idx < start_source || source_idx > end_source {
            continue;
        }

        // Determine the char range to highlight on this source line.
        // If this is the start source line, use start_col.
        // If this is the end source line, use end_col.
        // Otherwise, highlight the entire line.
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let line_len = line_text.chars().count();

        // For wrapped source lines, a display-row start/end column only applies
        // to the first/last display row of that source line. We approximate:
        // - start_col applies when source_idx == start_source
        // - end_col applies when source_idx == end_source
        // - full line highlighted for intermediate source lines
        let line_start_col = if source_idx == start_source {
            start_col.min(line_len)
        } else {
            0
        };
        let line_end_col = if source_idx == end_source {
            end_col.min(line_len)
        } else {
            line_len
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
///   offset is computed from the wrapped line count (via
///   `count_ratatui_wrapped_lines()`) which accounts for word-wrapping
///   at the terminal width, using the same grapheme-level width
///   measurement as ratatui's `Paragraph::wrap()`.
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
/// wrap logic in `count_ratatui_wrapped_lines()` using grapheme-level width
/// (`UnicodeWidthStr::width()`) so the scroll offset matches the actual
/// rendered layout — even for emoji ZWJ sequences, flag emojis, and CJK.
pub fn render(
    f: &mut Frame,
    area: Rect,
    messages: &[ChatMessage],
    scroll_state: &mut ScrollState,
    theme: MarkdownTheme,
    style_enabled: bool,
    selection: &ChatSelection,
) -> RenderMetadata {
    let available_width = area.width as usize;
    let mut lines = build_lines(messages, theme, style_enabled, available_width);

    // Build wrapped visual_lines in display-row space (one entry per wrapped line)
    // and a source_line_map that maps each display row to its source line index.
    // This aligns visual_lines indices with scroll_from_top (also in display-row space),
    // fixing the mouse offset bug where wrapped lines caused coordinate mismatch.
    let (visual_lines, source_line_map) = wrap_visual_lines(&lines, area.width);

    // Calculate scroll offset from the top of content.
    // Use grapheme-level wrapping (matching ratatui's WordWrapper) so the
    // scroll offset is accurate for emoji ZWJ sequences, flag emojis, and
    // CJK characters.
    let wrapped_total = visual_lines.len();
    let visible_height = area.height as usize;
    // Clamp manual_offset to valid range [0, max_scroll] before computing
    // effective offset. This prevents "overscroll" accumulation from rapid
    // mouse wheel scrolling — without clamping, scroll_up() can grow
    // manual_offset well beyond max_scroll, making scroll_down() feel
    // sluggish because each tick only subtracts a small number.
    scroll_state.clamp_offset(wrapped_total, visible_height);
    let scroll_from_top = scroll_state.effective_scroll_from_top(wrapped_total, visible_height);

    // Apply selection highlight AFTER computing scroll offset (modifies
    // span styles but not content — visual_lines and wrapped_total are
    // unaffected by styling changes). Use source_line_map to convert
    // display-row selection coordinates to source-line indices.
    apply_selection_highlight(&mut lines, selection, &source_line_map);

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .scroll((scroll_from_top, 0));

    f.render_widget(paragraph, area);

    RenderMetadata {
        visual_lines,
        scroll_from_top,
        source_line_map,
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

    // --- count_ratatui_wrapped_lines tests ---

    #[test]
    fn test_count_wrapped_short_line() {
        // Line fits in width — exact same as source lines
        let lines = vec![Line::from("hello")];
        assert_eq!(count_ratatui_wrapped_lines(&lines, 80), 1);
    }

    #[test]
    fn test_count_wrapped_long_line() {
        // Line wraps at word boundary
        let lines = vec![Line::from("hello world foo")];
        // width=11: "hello world" (11) fits, then "foo" (3)
        assert_eq!(count_ratatui_wrapped_lines(&lines, 11), 2);
    }

    #[test]
    fn test_count_wrapped_multiple_input_lines() {
        let lines = vec![Line::from("hello"), Line::from("world")];
        assert_eq!(count_ratatui_wrapped_lines(&lines, 80), 2);
    }

    #[test]
    fn test_count_wrapped_empty_line() {
        // Empty line still counts as 1 visual line
        let lines = vec![Line::from("")];
        assert_eq!(count_ratatui_wrapped_lines(&lines, 80), 1);
    }

    #[test]
    fn test_count_wrapped_cjk() {
        // CJK characters are 2 columns wide
        // "日本語 test" = 6 + 1 + 4 = 11 cols; width=8 → wraps
        let lines = vec![Line::from("日本語 test")];
        assert_eq!(count_ratatui_wrapped_lines(&lines, 8), 2);
    }

    #[test]
    fn test_count_wrapped_emoji_emoji_presentation() {
        // ✅ is width 2 (emoji presentation)
        // "✅ done" = 2 + 1 + 4 = 7 cols; width=5 → wraps to 2 lines
        let lines = vec![Line::from("✅ done")];
        assert_eq!(count_ratatui_wrapped_lines(&lines, 5), 2);
    }

    #[test]
    fn test_count_wrapped_flag_emoji() {
        // 🇧🇷 is width 2 (flag emoji = 2 regional indicators).
        // This is the KEY test — the old char-level count_wrapped_lines
        // treated each regional indicator as width 0, giving total width 1
        // (just the space) instead of 7 (2 + 1 + 4). That caused the
        // scroll offset to undercount and bottom lines to disappear.
        let lines = vec![Line::from("🇧🇷 flag")];
        // "🇧🇷 flag" = 2 + 1 + 4 = 7 cols; width=10 → fits (1 line)
        assert_eq!(count_ratatui_wrapped_lines(&lines, 10), 1);
        // width=4 → "🇧🇷" (2) fits, " flag" (5) → wraps (2 lines)
        assert_eq!(count_ratatui_wrapped_lines(&lines, 4), 2);
    }

    #[test]
    fn test_count_wrapped_zwj_emoji() {
        // 👨‍💻 is width 2 (ZWJ sequence)
        let lines = vec![Line::from("👨‍💻 code")];
        // "👨‍💻 code" = 2 + 1 + 4 = 7 cols; width=5 → wraps
        assert_eq!(count_ratatui_wrapped_lines(&lines, 5), 2);
    }

    #[test]
    fn test_count_wrapped_zero_width() {
        // Width 0 — should return number of input lines
        let lines = vec![Line::from("hello")];
        assert_eq!(count_ratatui_wrapped_lines(&lines, 0), 1);
    }

    #[test]
    fn test_count_wrapped_multi_span_line() {
        // Line with multiple spans — graphemes are flattened
        let lines = vec![Line::from(vec![
            Span::raw("hello "),
            Span::raw("world foo"),
        ])];
        // "hello world foo" = 15 cols; width=11 → "hello world" + "foo" (2 lines)
        assert_eq!(count_ratatui_wrapped_lines(&lines, 11), 2);
    }

    // --- wrap_visual_lines tests ---

    #[test]
    fn test_wrap_visual_lines_short_line() {
        // Line fits in width — one display row per source line
        let lines = vec![Line::from("hello")];
        let (visual, map) = wrap_visual_lines(&lines, 80);
        assert_eq!(visual, vec!["hello"]);
        assert_eq!(map, vec![0]);
    }

    #[test]
    fn test_wrap_visual_lines_long_line_wraps() {
        // Line that wraps produces multiple display rows mapping to same source
        let lines = vec![Line::from("hello world foo")];
        // width=11: "hello world" (11) + "foo" (3) = 2 display rows
        let (visual, map) = wrap_visual_lines(&lines, 11);
        assert_eq!(visual.len(), 2);
        assert_eq!(map, vec![0, 0]); // Both display rows map to source line 0
    }

    #[test]
    fn test_wrap_visual_lines_multiple_source_lines() {
        // Two short lines, no wrapping
        let lines = vec![Line::from("hello"), Line::from("world")];
        let (visual, map) = wrap_visual_lines(&lines, 80);
        assert_eq!(visual, vec!["hello", "world"]);
        assert_eq!(map, vec![0, 1]);
    }

    #[test]
    fn test_wrap_visual_lines_empty_line() {
        // Empty line produces one display row
        let lines = vec![Line::from("")];
        let (visual, map) = wrap_visual_lines(&lines, 80);
        assert_eq!(visual, vec![""]);
        assert_eq!(map, vec![0]);
    }

    #[test]
    fn test_wrap_visual_lines_mixed_wrap_and_no_wrap() {
        // First line wraps, second line doesn't
        let lines = vec![
            Line::from("hello world foo bar baz"), // wraps at width 12
            Line::from("short"),                   // doesn't wrap
        ];
        let (visual, map) = wrap_visual_lines(&lines, 12);
        // Source 0 wraps, source 1 doesn't
        assert!(
            visual.len() > 2,
            "Should have more display rows than source lines"
        );
        // All display rows for source 0 should map to 0, source 1 maps to 1
        let source0_count = map.iter().filter(|&&m| m == 0).count();
        let source1_count = map.iter().filter(|&&m| m == 1).count();
        assert!(
            source0_count > 1,
            "Source line 0 should span multiple display rows"
        );
        assert_eq!(
            source1_count, 1,
            "Source line 1 spans exactly one display row"
        );
    }

    #[test]
    fn test_wrap_visual_lines_cjk() {
        // CJK characters are 2 columns wide
        let lines = vec![Line::from("日本語 test")];
        let (visual, map) = wrap_visual_lines(&lines, 8);
        // "日本語 test" = 6 + 1 + 4 = 11 cols at width 8 → wraps
        assert!(visual.len() > 1, "CJK text should wrap at width 8");
        assert!(
            map.iter().all(|&m| m == 0),
            "All display rows map to source 0"
        );
    }

    #[test]
    fn test_wrap_visual_lines_multi_span() {
        // Line with multiple spans — text is flattened before wrapping
        let lines = vec![Line::from(vec![
            Span::raw("hello "),
            Span::raw("world foo"),
        ])];
        let (visual, map) = wrap_visual_lines(&lines, 11);
        // "hello world foo" = 15 cols at width 11 → "hello world" + "foo"
        assert_eq!(visual.len(), 2);
        assert_eq!(map, vec![0, 0]);
    }

    #[test]
    fn test_wrap_visual_lines_zero_width() {
        // Width 0 clamped to width 1 — each character gets its own display row
        let lines = vec![Line::from("hello")];
        let (visual, map) = wrap_visual_lines(&lines, 0);
        // With max_width=1, "hello" wraps to 5 rows (one char each)
        // This matches ratatui behavior which also doesn't support width 0
        assert_eq!(visual.len(), 5);
        assert!(map.iter().all(|&m| m == 0));
    }

    // --- apply_selection_highlight with source_line_map tests ---

    #[test]
    fn test_selection_highlight_single_line_no_wrap() {
        // Simple selection on a single line, no wrapping
        let mut lines = vec![Line::from("hello world")];
        let map = vec![0_usize]; // One display row, maps to source 0
        let mut selection = ChatSelection::new();
        // Select "world" (cols 6-11) on display row 0
        selection.begin(0, 6);
        selection.extend(0, 11);
        selection.finish(0, 11);
        apply_selection_highlight(&mut lines, &selection, &map);
        // Line should still have content — just styled differently
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn test_selection_highlight_wrapped_line_maps_correctly() {
        // A line that wraps: display rows 0 and 1 both map to source line 0
        // Selection across both display rows should highlight the whole source line
        let mut lines = vec![Line::from("hello world foo")];
        let map = vec![0_usize, 0_usize]; // Two display rows, both source 0
        let mut selection = ChatSelection::new();
        // Select from display row 0 col 0 to display row 1 col 3
        selection.begin(0, 0);
        selection.extend(1, 3);
        selection.finish(1, 3);
        apply_selection_highlight(&mut lines, &selection, &map);
        // Source line 0 is fully selected (start=0, end=entire line)
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "hello world foo");
    }

    #[test]
    fn test_selection_highlight_across_multiple_source_lines() {
        // Two source lines, selection spans both
        let mut lines = vec![Line::from("first line"), Line::from("second line")];
        let map = vec![0_usize, 1_usize]; // One display row per source
        let mut selection = ChatSelection::new();
        // Select from source 0 col 6 to source 1 col 6
        selection.begin(0, 6);
        selection.extend(1, 6);
        selection.finish(1, 6);
        apply_selection_highlight(&mut lines, &selection, &map);
        let text0: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let text1: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text0, "first line");
        assert_eq!(text1, "second line");
    }

    #[test]
    fn test_selection_highlight_empty_map_no_panic() {
        let mut lines = vec![Line::from("hello")];
        let map: Vec<usize> = vec![];
        let mut selection = ChatSelection::new();
        selection.begin(0, 0);
        selection.extend(0, 5);
        selection.finish(0, 5);
        // Should not panic
        apply_selection_highlight(&mut lines, &selection, &map);
    }

    #[test]
    fn test_selection_highlight_no_selection_no_change() {
        let mut lines = vec![Line::from("hello")];
        let map = vec![0_usize];
        let selection = ChatSelection::new();
        // No selection active
        apply_selection_highlight(&mut lines, &selection, &map);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "hello");
    }
}
