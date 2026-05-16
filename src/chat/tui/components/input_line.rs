//! Input line component — user input display using ratatui-textarea
//!
//! Renders the TextArea widget at the bottom of the TUI, showing
//! the prompt prefix and the current input content. The TextArea
//! handles all text editing internally (cursor, selection, kill-ring).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui_textarea::TextArea;

use super::super::styles;

/// Render the input line using the TextArea widget
///
/// When input is disabled (during LLM processing), shows a dim prompt
/// with the reason. When enabled, renders the TextArea widget with
/// a ">>> " prompt prefix and "... " continuation prefixes.
///
/// The `TextArea` widget handles all cursor positioning and scrolling
/// internally. We add prompt prefixes by prepending styled spans
/// to each line.
pub fn render(
    f: &mut Frame,
    area: Rect,
    textarea: &TextArea<'static>,
    disabled: bool,
    disabled_reason: Option<&str>,
) {
    let prompt_style = styles::prompt_style();
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

        // Build lines with prompt prefixes prepended
        let lines = textarea.lines();
        let mut display_lines: Vec<Line> = Vec::new();

        for (i, text_line) in lines.iter().enumerate() {
            let prompt = if i == 0 {
                Span::styled(">>> ", prompt_style)
            } else {
                Span::styled("... ", prompt_style)
            };
            display_lines.push(Line::from(vec![prompt, Span::raw(text_line.to_string())]));
        }

        // If textarea is completely empty, show the prompt line
        if display_lines.is_empty() {
            display_lines.push(Line::from(vec![
                Span::styled(">>> ", prompt_style),
                Span::raw(String::new()),
            ]));
        }

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
        // If we get here without panic, the render succeeded
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
}
