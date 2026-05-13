//! Input line component — user input display
//!
//! Renders the input line at the bottom of the TUI, showing
//! the prompt string and current input buffer.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::super::styles;
use unicode_width::UnicodeWidthStr;

/// Input line state for rendering
#[derive(Debug, Clone)]
pub struct InputState {
    /// Current input buffer content
    pub buffer: String,
    /// Cursor position within the buffer (byte offset)
    pub cursor_pos: usize,
    /// Whether input is disabled (e.g., during LLM processing)
    pub disabled: bool,
    /// Disabled reason (shown when input is disabled)
    pub disabled_reason: Option<String>,
    /// Horizontal scroll offset (visual columns). When the input text is
    /// wider than the viewport, this tracks how many columns are hidden
    /// to the left so the cursor stays visible.
    pub scroll_offset: u16,
}

impl InputState {
    /// Create a new empty input state
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            disabled: false,
            disabled_reason: None,
            scroll_offset: 0,
        }
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.update_scroll_offset();
    }

    /// Delete the character before the cursor (backspace)
    pub fn backspace(&mut self) -> bool {
        if self.cursor_pos > 0 {
            // Find the previous character boundary
            let prev_pos = self.buffer[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.buffer.drain(prev_pos..self.cursor_pos);
            self.cursor_pos = prev_pos;
            self.update_scroll_offset();
            true
        } else {
            false
        }
    }

    /// Move cursor left by one character
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev_pos = self.buffer[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor_pos = prev_pos;
            self.update_scroll_offset();
        }
    }

    /// Move cursor right by one character
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.buffer.len() {
            let next_pos = self.buffer[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.buffer.len());
            self.cursor_pos = next_pos;
            self.update_scroll_offset();
        }
    }

    /// Update horizontal scroll offset so the cursor stays visible.
    ///
    /// The visible text window starts at `scroll_offset` columns.
    /// If the cursor is before the window, scroll left. If the cursor
    /// is beyond the right edge, scroll right.
    fn update_scroll_offset(&mut self) {
        const PROMPT_WIDTH: u16 = 4; // ">>> "
        let text_before_cursor = &self.buffer[..self.cursor_pos];
        let cursor_visual = PROMPT_WIDTH + text_before_cursor.width() as u16;
        // Keep cursor within the last visible column (heuristic: 1 column margin)
        if cursor_visual > self.scroll_offset + self.visible_width().saturating_sub(1) {
            self.scroll_offset =
                cursor_visual.saturating_sub(self.visible_width().saturating_sub(1));
        } else if cursor_visual < PROMPT_WIDTH + self.scroll_offset {
            self.scroll_offset = cursor_visual.saturating_sub(PROMPT_WIDTH);
        }
    }

    /// Visible text width in columns (placeholder — set at render time).
    ///
    /// Since we don't know the terminal width here, we use a conservative
    /// default. The render function handles clamping via the area Rect.
    fn visible_width(&self) -> u16 {
        80 // Conservative default; actual clipping done by Paragraph
    }

    /// Clear the buffer and reset cursor
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    /// Get the current line content for submission
    #[allow(dead_code)] // PR3: Will be used for TUI input submission
    pub fn line(&self) -> &str {
        &self.buffer
    }

    /// Set the disabled state
    pub fn set_disabled(&mut self, disabled: bool, reason: Option<String>) {
        self.disabled = disabled;
        self.disabled_reason = reason;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the input line
///
/// Shows ">>> " prompt when enabled, or a disabled indicator when
/// the LLM is processing. Long input lines scroll horizontally so the
/// cursor stays visible. The horizontal scroll is tracked via
/// `state.scroll_offset` (visual columns from the left).
pub fn render(f: &mut Frame, area: Rect, state: &InputState) {
    let prompt_style = styles::prompt_style();
    let dim_style = Style::default().add_modifier(Modifier::DIM);
    const PROMPT_WIDTH: u16 = 4; // ">>> "

    if state.disabled {
        let reason = state.disabled_reason.as_deref().unwrap_or("Processing...");
        let spans = vec![
            Span::styled(">>> ", dim_style),
            Span::styled(reason.to_string(), dim_style),
        ];
        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        f.render_widget(paragraph, area);
    } else {
        let spans = vec![Span::styled(">>> ", prompt_style), Span::raw(&state.buffer)];
        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).scroll((0, state.scroll_offset));
        f.render_widget(paragraph, area);

        // Set cursor position in the input area
        // prompt ">>> " = 4 characters (ASCII, width = 4)
        let text_before_cursor = &state.buffer[..state.cursor_pos];
        let cursor_visual = PROMPT_WIDTH + text_before_cursor.width() as u16;
        let cursor_x = area.x + cursor_visual.saturating_sub(state.scroll_offset);
        // Clamp cursor to visible area
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(1));
        let cursor_y = area.y;
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_new() {
        let state = InputState::new();
        assert!(state.buffer.is_empty());
        assert_eq!(state.cursor_pos, 0);
        assert!(!state.disabled);
    }

    #[test]
    fn test_insert_char() {
        let mut state = InputState::new();
        state.insert_char('a');
        assert_eq!(state.buffer, "a");
        assert_eq!(state.cursor_pos, 1);
        state.insert_char('b');
        assert_eq!(state.buffer, "ab");
        assert_eq!(state.cursor_pos, 2);
    }

    #[test]
    fn test_backspace() {
        let mut state = InputState::new();
        state.insert_char('a');
        state.insert_char('b');
        state.backspace();
        assert_eq!(state.buffer, "a");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = InputState::new();
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        assert_eq!(state.cursor_pos, 3);

        state.cursor_left();
        assert_eq!(state.cursor_pos, 2);

        state.cursor_right();
        assert_eq!(state.cursor_pos, 3);
    }

    #[test]
    fn test_unicode_cursor() {
        let mut state = InputState::new();
        state.insert_char('你');
        state.insert_char('好');
        assert_eq!(state.buffer, "你好");
        assert_eq!(state.cursor_pos, 6); // 3 bytes per CJK char
        state.cursor_left();
        // Should move back by one char (3 bytes)
        assert_eq!(state.cursor_pos, 3);
    }

    #[test]
    fn test_disabled_state() {
        let mut state = InputState::new();
        state.set_disabled(true, Some("Thinking...".to_string()));
        assert!(state.disabled);
        assert_eq!(state.disabled_reason.as_deref(), Some("Thinking..."));
    }

    #[test]
    fn test_clear() {
        let mut state = InputState::new();
        state.insert_char('a');
        state.insert_char('b');
        state.clear();
        assert!(state.buffer.is_empty());
        assert_eq!(state.cursor_pos, 0);
    }
}
