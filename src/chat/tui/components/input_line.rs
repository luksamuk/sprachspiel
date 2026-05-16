//! Input line component — user input display using ratatui-textarea
//!
//! Renders the TextArea widget at the bottom of the TUI, showing
//! the prompt prefix and the current input content. Selection highlighting
//! is applied by querying `textarea.selection_range()` and styling the
//! selected characters with a blue background.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui_textarea::TextArea;

use super::super::styles;
use super::chat_selection::selection_style;

/// Build display lines with prompt prefixes and selection highlighting.
///
/// Each line gets a ">>> " (first line) or "... " (continuation) prefix.
/// If the textarea has an active selection, characters within the selection
/// range are rendered with a blue background highlight.
fn build_display_lines(textarea: &TextArea<'static>) -> Vec<Line<'static>> {
    let lines = textarea.lines();
    let prompt_style = styles::prompt_style();
    let sel_style = selection_style();

    // Get selection range: Option<((start_row, start_col), (end_row, end_col))>
    let selection = textarea.selection_range();

    let mut display_lines: Vec<Line<'static>> = Vec::new();

    for (i, text_line) in lines.iter().enumerate() {
        let prompt = if i == 0 {
            Span::styled(">>> ", prompt_style)
        } else {
            Span::styled("... ", prompt_style)
        };

        let text_spans = if let Some(((start_row, start_col), (end_row, end_col))) = selection {
            // This line intersects the selection
            if i >= start_row && i <= end_row {
                apply_selection_to_line(
                    text_line, i, start_row, start_col, end_row, end_col, sel_style,
                )
            } else {
                vec![Span::raw(text_line.to_string())]
            }
        } else {
            vec![Span::raw(text_line.to_string())]
        };

        let mut spans = vec![prompt];
        spans.extend(text_spans);
        display_lines.push(Line::from(spans));
    }

    // If textarea is completely empty, show the prompt line
    if display_lines.is_empty() {
        display_lines.push(Line::from(vec![
            Span::styled(">>> ", prompt_style),
            Span::raw(String::new()),
        ]));
    }

    display_lines
}

/// Apply selection highlighting to a single line of text.
///
/// Returns a vector of `Span`s where characters within the selection
/// range are styled with the selection highlight, and characters outside
/// are plain.
fn apply_selection_to_line(
    line: &str,
    row: usize,
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
    sel_style: Style,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = line.chars().collect();
    let char_count = chars.len();

    // Determine the effective start and end columns for this line
    let eff_start = if row == start_row { start_col } else { 0 };
    let eff_end = if row == end_row {
        end_col.min(char_count)
    } else {
        char_count
    };

    if eff_start >= char_count || eff_end <= eff_start {
        // Selection doesn't intersect this line
        return vec![Span::raw(line.to_string())];
    }

    let mut spans = Vec::new();

    // Before selection
    if eff_start > 0 {
        let before: String = chars[..eff_start].iter().collect();
        spans.push(Span::raw(before));
    }

    // Selection
    let selected: String = chars[eff_start..eff_end.min(char_count)].iter().collect();
    spans.push(Span::styled(selected, sel_style));

    // After selection
    if eff_end < char_count {
        let after: String = chars[eff_end..].iter().collect();
        spans.push(Span::raw(after));
    }

    spans
}

/// Render the input line using the TextArea widget
///
/// When input is disabled (during LLM processing), shows a dim prompt
/// with the reason. When enabled, renders the TextArea content with
/// prompt prefixes and selection highlighting.
///
/// The `TextArea` widget handles all cursor positioning and scrolling
/// internally. We add prompt prefixes by prepending styled spans
/// to each line, and apply selection highlighting to active selections.
pub fn render(
    f: &mut Frame,
    area: Rect,
    textarea: &TextArea<'static>,
    disabled: bool,
    disabled_reason: Option<&str>,
) {
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
    } else {
        // Build the block with prompt prefixes rendered as text prefix
        // We use a Block with no borders for the prompt section
        let block = Block::default();
        let inner = block.inner(area);
        f.render_widget(block, area);

        // Build lines with prompt prefixes and selection highlighting
        let display_lines = build_display_lines(textarea);
        let text = ratatui::text::Text::from(display_lines);
        let paragraph = Paragraph::new(text);
        f.render_widget(paragraph, inner);

        // Position cursor based on textarea's internal cursor position
        // The prompt prefix is 4 chars wide (">>> " or "... ")
        const PROMPT_WIDTH: u16 = 4;
        let cursor = textarea.cursor();
        let cursor_row = cursor.0;
        let cursor_col = cursor.1;

        // Calculate vertical scroll: if cursor is below visible area, scroll down
        let visible_lines = inner.height as usize;
        let scroll_y = if cursor_row >= visible_lines {
            (cursor_row - visible_lines + 1) as u16
        } else {
            0
        };

        // Calculate horizontal scroll for the cursor's line
        let cursor_visual_x = PROMPT_WIDTH + cursor_col as u16;
        let right_edge = inner.width;

        // Compute cursor position within the visible area
        let cursor_y = area.y + cursor_row as u16 - scroll_y;
        let cursor_x = area.x + cursor_visual_x.min(area.x + right_edge.saturating_sub(1));

        // Only show cursor if it's within the visible area
        if cursor_y >= area.y && cursor_y < area.y + area.height {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }
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
    fn test_apply_selection_to_line_full_line() {
        let style = selection_style();
        // Select entire line (row 0, start_row 0, start_col 0, end_row 0, end_col 5)
        let spans = apply_selection_to_line("hello", 0, 0, 0, 0, 5, style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn test_apply_selection_to_line_partial() {
        let style = selection_style();
        // Select "llo" from "hello" (cols 2-5) on row 0 — selection goes to end of line
        let spans = apply_selection_to_line("hello", 0, 0, 2, 0, 5, style);
        // "he" (unselected) + "llo" (selected) — no trailing unselected span
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "he");
        assert_eq!(spans[1].content, "llo");
    }

    #[test]
    fn test_apply_selection_to_line_middle() {
        let style = selection_style();
        // Select "ll" from "hello" (cols 2-4) on row 0
        let spans = apply_selection_to_line("hello", 0, 0, 2, 0, 4, style);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "he");
        assert_eq!(spans[1].content, "ll");
        assert_eq!(spans[2].content, "o");
    }

    #[test]
    fn test_apply_selection_no_intersection() {
        let style = selection_style();
        // Line 0 not in selection on lines 2-3
        let spans = apply_selection_to_line("hello", 0, 2, 0, 3, 5, style);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
    }

    #[test]
    fn test_apply_selection_multiline_start() {
        let style = selection_style();
        // Line 0 is start of selection (from col 3 to end of line 0, continuing to line 2)
        let spans = apply_selection_to_line("hello", 0, 0, 3, 2, 5, style);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "hel");
        assert_eq!(spans[1].content, "lo");
    }
}
