//! Input line component — user input display with word-wrap
//!
//! Renders the textarea content at the bottom of the TUI, showing
//! the prompt prefix (">>> " / "... ") and the current input content.
//! Long lines are soft-wrapped at word boundaries (with glyph fallback
//! for words wider than the viewport), eliminating horizontal scroll.
//! Selection highlighting is applied by querying
//! `textarea.selection_range()` and styling the selected characters
//! with a blue background.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui_textarea::TextArea;

use super::super::styles;
use super::super::wrap::wrap_line;
use super::chat_selection::selection_style;

/// Width of the prompt prefix in characters: ">>> " or "... " = 4 chars.
const PROMPT_WIDTH: usize = 4;

/// A single wrapped sub-line within a logical line.
///
/// Tracks the byte offset range within the original logical line so
/// that selection highlighting can be mapped from logical positions
/// (row, col) to visual positions within wrapped sub-lines.
struct WrappedSubLine {
    /// The wrapped text content.
    text: String,
    /// Character offset within the original logical line where this
    /// sub-line starts. Used to map selection ranges onto wrapped lines.
    char_offset: usize,
}

/// Build display lines with word-wrap, prompt prefixes, and selection highlighting.
///
/// Each logical line is wrapped to `wrap_width` columns using `wrap_line()`.
/// The first sub-line of the first logical line gets ">>> " prefix; all
/// others get "... ". If a selection is active, characters within the range
/// are rendered with a blue background.
fn build_display_lines(textarea: &TextArea<'static>, wrap_width: usize) -> Vec<Line<'static>> {
    let lines = textarea.lines();
    let prompt_style = styles::prompt_style();
    let sel_style = selection_style();
    let selection = textarea
        .selection_range()
        .map(|((sr, sc), (er, ec))| SelectionRange {
            start_row: sr,
            start_col: sc,
            end_row: er,
            end_col: ec,
        });

    // wrap_width must account for the prompt prefix (4 chars).
    // The text area for content is (wrap_width - PROMPT_WIDTH) columns wide.
    let content_width = wrap_width.saturating_sub(PROMPT_WIDTH);
    let effective_width = if content_width > 0 { content_width } else { 20 };

    let mut display_lines: Vec<Line<'static>> = Vec::new();
    let mut first_line = true;

    for (logical_row, text_line) in lines.iter().enumerate() {
        // Wrap the logical line into sub-lines that fit the content width
        let wrapped_texts = wrap_line(text_line, effective_width);

        // Compute char offsets for each sub-line so we can map selection
        // ranges. wrap_line() splits on character boundaries, so we can
        // count chars in each sub-line to compute cumulative offsets.
        let mut sub_lines: Vec<WrappedSubLine> = Vec::new();
        let mut char_offset = 0usize;
        for sub_text in &wrapped_texts {
            sub_lines.push(WrappedSubLine {
                text: sub_text.clone(),
                char_offset,
            });
            char_offset += sub_text.chars().count();
        }

        // Build display lines for each sub-line
        for sub_line in &sub_lines {
            let prompt = if first_line {
                Span::styled(">>> ", prompt_style)
            } else {
                Span::styled("... ", prompt_style)
            };
            first_line = false;

            let text_spans = if let Some(ref sel) = selection {
                // Check if the selection intersects this sub-line's logical row
                if logical_row >= sel.start_row && logical_row <= sel.end_row {
                    apply_selection_to_wrapped_subline(
                        &sub_line.text,
                        logical_row,
                        sub_line.char_offset,
                        sel,
                        sel_style,
                    )
                } else {
                    vec![Span::raw(sub_line.text.clone())]
                }
            } else {
                vec![Span::raw(sub_line.text.clone())]
            };

            let mut spans = vec![prompt];
            spans.extend(text_spans);
            display_lines.push(Line::from(spans));
        }
    }

    // If textarea is completely empty, show at least the prompt line
    if display_lines.is_empty() {
        display_lines.push(Line::from(vec![
            Span::styled(">>> ", prompt_style),
            Span::raw(String::new()),
        ]));
    }

    display_lines
}

/// Selection range derived from `textarea.selection_range()`.
///
/// Stores the start and end positions as (row, col) pairs so that
/// selection parameters can be passed as a single struct instead of
/// 6 individual arguments.
struct SelectionRange {
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
}

/// Apply selection highlighting to a wrapped sub-line.
///
/// The `char_offset` is the character offset of this sub-line within
/// the original logical line. Selection positions (`start_col`, `end_col`)
/// are in character positions within the logical line, so we subtract
/// `char_offset` to get positions within the sub-line.
fn apply_selection_to_wrapped_subline(
    text: &str,
    logical_row: usize,
    char_offset: usize,
    sel: &SelectionRange,
    sel_style: Style,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let char_count = chars.len();

    // Convert selection columns from logical-line coords to sub-line coords
    let local_start = if logical_row == sel.start_row {
        sel.start_col.saturating_sub(char_offset)
    } else {
        0
    };
    let local_end = if logical_row == sel.end_row {
        sel.end_col
            .min(char_offset + char_count)
            .saturating_sub(char_offset)
    } else {
        char_count
    };

    if local_start >= char_count || local_end <= local_start {
        return vec![Span::raw(text.to_string())];
    }

    let mut spans = Vec::new();

    // Before selection
    if local_start > 0 {
        let before: String = chars[..local_start].iter().collect();
        spans.push(Span::raw(before));
    }

    // Selection
    let selected: String = chars[local_start..local_end.min(char_count)]
        .iter()
        .collect();
    spans.push(Span::styled(selected, sel_style));

    // After selection
    if local_end < char_count {
        let after: String = chars[local_end..].iter().collect();
        spans.push(Span::raw(after));
    }

    spans
}

/// Compute the cursor position on screen, accounting for word-wrap.
///
/// Returns `(visual_row, visual_col)` where `visual_row` is the 0-indexed
/// display line (counting wrapped sub-lines) and `visual_col` is the
/// character column within that sub-line (including prompt prefix offset).
fn cursor_visual_position(textarea: &TextArea<'static>, content_width: usize) -> (usize, usize) {
    let cursor = textarea.cursor();
    let cursor_row = cursor.0;
    let cursor_col = cursor.1;
    let lines = textarea.lines();
    let effective_width = if content_width > 0 { content_width } else { 20 };

    let mut visual_row = 0usize;

    for (i, line) in lines.iter().enumerate() {
        let wrapped = wrap_line(line, effective_width);
        if i == cursor_row {
            // Find which sub-line the cursor is on
            let mut char_offset = 0usize;
            for sub_line in &wrapped {
                let sub_len = sub_line.chars().count();
                if cursor_col < char_offset + sub_len
                    || sub_line == wrapped.last().unwrap_or(&String::new())
                {
                    // Cursor is on this sub-line
                    let col_in_sub = cursor_col.saturating_sub(char_offset);
                    return (visual_row, col_in_sub);
                }
                char_offset += sub_len;
                visual_row += 1;
            }
            // Fallback: cursor is at end of last sub-line
            return (visual_row, 0);
        } else {
            visual_row += wrapped.len();
        }
    }

    (0, 0)
}

/// Render the input line with word-wrap, prompt prefixes, and cursor positioning.
///
/// Returns the number of visual (wrapped) lines rendered, used by the
/// caller to calculate the input area height for the next render cycle.
///
/// When input is disabled (during LLM processing), shows a dim prompt
/// with the reason. When enabled, renders the textarea content with
/// prompt prefixes, selection highlighting, and word-wrap.
pub fn render(
    f: &mut Frame,
    area: Rect,
    textarea: &TextArea<'static>,
    disabled: bool,
    disabled_reason: Option<&str>,
) -> usize {
    let dim_style = Style::default().add_modifier(Modifier::DIM);

    if disabled {
        let reason = disabled_reason.unwrap_or("Processing...");
        let spans = vec![
            Span::styled(">>> ", dim_style),
            Span::styled(reason.to_string(), dim_style),
        ];
        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        f.render_widget(paragraph, area);
        return 1;
    }

    let block = Block::default();
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Wrap width = available inner width (includes prompt prefix space)
    let wrap_width = inner.width as usize;
    let content_width = wrap_width.saturating_sub(PROMPT_WIDTH);

    // Build lines with word-wrap, prompt prefixes, and selection highlighting
    let display_lines = build_display_lines(textarea, wrap_width);
    let total_visual_lines = display_lines.len();

    // Calculate vertical scroll offset to keep cursor visible.
    // Scroll so that the cursor's visual row is within the visible area.
    let (cursor_visual_row, cursor_visual_col) = cursor_visual_position(textarea, content_width);

    let visible_lines = inner.height as usize;
    let scroll_y = if cursor_visual_row >= visible_lines {
        (cursor_visual_row - visible_lines + 1) as u16
    } else {
        0
    };

    // Render the wrapped text with vertical scrolling
    let text = ratatui::text::Text::from(display_lines);
    let paragraph = Paragraph::new(text).scroll((scroll_y, 0));

    f.render_widget(paragraph, inner);

    // Position cursor: visual row adjusted by scroll, visual col offset by prompt
    let cursor_y = inner.y + cursor_visual_row as u16 - scroll_y;
    let cursor_x = inner.x + PROMPT_WIDTH as u16 + cursor_visual_col as u16;

    // Only show cursor if it's within the visible area
    if cursor_y >= inner.y
        && cursor_y < inner.y + inner.height
        && cursor_x >= inner.x
        && cursor_x < inner.x + inner.width
    {
        f.set_cursor_position((cursor_x, cursor_y));
    }

    total_visual_lines
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui_textarea::TextArea;

    use super::*;

    /// Helper to create a terminal with a test backend for rendering tests
    fn test_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        Terminal::new(backend).unwrap()
    }

    #[test]
    fn test_render_empty_textarea() {
        let mut terminal = test_terminal(80, 5);
        let textarea = TextArea::default();
        terminal
            .draw(|f| {
                render(f, f.area(), &textarea, false, None);
            })
            .unwrap();
    }

    #[test]
    fn test_render_disabled_input() {
        let mut terminal = test_terminal(80, 5);
        let textarea = TextArea::default();
        terminal
            .draw(|f| {
                render(f, f.area(), &textarea, true, Some("Thinking..."));
            })
            .unwrap();
    }

    #[test]
    fn test_render_multiline_textarea() {
        let mut terminal = test_terminal(80, 10);
        let mut textarea = TextArea::default();
        textarea.insert_str("hello\nworld");
        terminal
            .draw(|f| {
                render(f, f.area(), &textarea, false, None);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_prompt_narrow() {
        let mut terminal = test_terminal(20, 5);
        let textarea = TextArea::default();
        terminal
            .draw(|f| {
                render(f, f.area(), &textarea, false, None);
            })
            .unwrap();
    }

    #[test]
    fn test_render_with_selection() {
        let mut terminal = test_terminal(80, 5);
        let mut textarea = TextArea::default();
        textarea.insert_str("hello world");
        // Select "llo" (cols 2-5)
        textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        textarea.start_selection();
        textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        assert!(textarea.is_selecting());
        terminal
            .draw(|f| {
                render(f, f.area(), &textarea, false, None);
            })
            .unwrap();
    }

    #[test]
    fn test_apply_selection_to_wrapped_subline_full_line() {
        let style = selection_style();
        // Select entire line (row 0, start_col 0, end_col 5)
        let sel = SelectionRange {
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 5,
        };
        let spans = apply_selection_to_wrapped_subline("hello", 0, 0, &sel, style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn test_apply_selection_to_wrapped_subline_partial() {
        let style = selection_style();
        // Select "llo" from "hello" (cols 2-5) on row 0
        let sel = SelectionRange {
            start_row: 0,
            start_col: 2,
            end_row: 0,
            end_col: 5,
        };
        let spans = apply_selection_to_wrapped_subline("hello", 0, 0, &sel, style);
        // "he" (unselected) + "llo" (selected)
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "he");
        assert_eq!(spans[1].content, "llo");
    }

    #[test]
    fn test_apply_selection_to_wrapped_subline_with_offset() {
        let style = selection_style();
        // Sub-line "world" starts at char_offset=6 in "hello world"
        // Selection covers cols 6-11 (the word "world")
        let sel = SelectionRange {
            start_row: 0,
            start_col: 6,
            end_row: 0,
            end_col: 11,
        };
        let spans = apply_selection_to_wrapped_subline("world", 0, 6, &sel, style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "world");
    }

    #[test]
    fn test_apply_selection_to_wrapped_subline_no_intersection() {
        let style = selection_style();
        // Sub-line "world" at offset 6, but selection is on row 2
        let sel = SelectionRange {
            start_row: 2,
            start_col: 0,
            end_row: 3,
            end_col: 5,
        };
        let spans = apply_selection_to_wrapped_subline("world", 0, 6, &sel, style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "world");
    }

    #[test]
    fn test_cursor_visual_position_single_line() {
        let mut textarea = TextArea::default();
        textarea.insert_str("hello");
        // Cursor at end of "hello" (row 0, col 5)
        // With content_width=80, single line — no wrapping
        let (row, col) = cursor_visual_position(&textarea, 80);
        assert_eq!(row, 0);
        assert_eq!(col, 5);
    }

    #[test]
    fn test_cursor_visual_position_multiline() {
        let mut textarea = TextArea::default();
        textarea.insert_str("hello\nworld");
        // After insert_str, cursor is at end of "world": (row=1, col=5)
        // With content_width=80, neither line wraps, so visual_row=1
        let (row, col) = cursor_visual_position(&textarea, 80);
        assert_eq!(row, 1);
        assert_eq!(col, 5);
    }

    #[test]
    fn test_cursor_visual_position_wrapped() {
        let mut textarea = TextArea::default();
        textarea.insert_str("hello world foo bar");
        // With content_width=6, "hello" (5 chars) fits on one line,
        // "world" (5 chars) on the next, etc.
        let (row, _col) = cursor_visual_position(&textarea, 6);
        // Cursor is at end of line (col 19), which is on a wrapped sub-line
        // Just verify it returns without panic and visual_row > 0
        assert!(row > 0, "cursor should be on a wrapped sub-line");
    }
}
