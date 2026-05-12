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
}

impl InputState {
    /// Create a new empty input state
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            disabled: false,
            disabled_reason: None,
        }
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
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
        }
    }

    /// Clear the buffer and reset cursor
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    /// Get the current line content for submission
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
/// the LLM is processing.
pub fn render(f: &mut Frame, area: Rect, state: &InputState) {
    let prompt_style = styles::prompt_style();
    let dim_style = Style::default().add_modifier(Modifier::DIM);

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
        let paragraph = Paragraph::new(line);
        f.render_widget(paragraph, area);

        // Set cursor position in the input area
        // prompt ">>> " = 4 characters (ASCII, width = 4)
        let text_before_cursor = &state.buffer[..state.cursor_pos];
        let cursor_x = area.x + 4 + text_before_cursor.width() as u16;
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
