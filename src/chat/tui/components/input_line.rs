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

    /// Delete the character at the cursor position (forward delete).
    ///
    /// This is the symmetrical counterpart to `backspace()`: while backspace
    /// deletes the character **before** the cursor (moving it left), this
    /// deletes the character **at** the cursor (keeping cursor position).
    ///
    /// Returns `true` if a character was deleted, `false` if the cursor is
    /// at the end of the buffer.
    pub fn delete_char_right(&mut self) -> bool {
        if self.cursor_pos < self.buffer.len() {
            // Find the next character boundary
            let next_pos = self.buffer[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.buffer.len());
            self.buffer.drain(self.cursor_pos..next_pos);
            // Cursor position stays the same (character at cursor is removed,
            // next character shifts left into its place)
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
/// cursor stays visible. Scroll is computed at render time from the
/// actual `area.width` — no state tracking needed.
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

        // Compute horizontal scroll so cursor stays within the visible area.
        let text_before_cursor = &state.buffer[..state.cursor_pos];
        let cursor_visual = PROMPT_WIDTH + text_before_cursor.width() as u16;
        let right_edge = area.width;
        let scroll_x = (cursor_visual + 1).saturating_sub(right_edge);

        let paragraph = Paragraph::new(line).scroll((0, scroll_x));
        f.render_widget(paragraph, area);

        // Set cursor position in the input area
        let cursor_x = area.x + cursor_visual.saturating_sub(scroll_x);
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

    #[test]
    fn test_delete_char_right() {
        // Delete at start: removes first character
        let mut state = InputState::new();
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        // cursor at end: "abc|"
        assert!(!state.delete_char_right()); // nothing to delete at end
        assert_eq!(state.buffer, "abc");

        // Move cursor to position 1: "a|bc"
        state.cursor_left();
        state.cursor_left();
        assert_eq!(state.cursor_pos, 1);
        assert!(state.delete_char_right()); // deletes 'b'
        assert_eq!(state.buffer, "ac");
        assert_eq!(state.cursor_pos, 1); // cursor stays at 1

        // Delete at cursor: "a|c" → "a"
        assert!(state.delete_char_right()); // deletes 'c'
        assert_eq!(state.buffer, "a");
        assert_eq!(state.cursor_pos, 1);

        // Empty buffer
        state.clear();
        assert!(!state.delete_char_right());
    }

    #[test]
    fn test_delete_char_right_unicode() {
        let mut state = InputState::new();
        state.insert_char('你');
        state.insert_char('好');
        state.insert_char('世');
        state.insert_char('界');
        // "你好世界", cursor at end (position 12 = 4 * 3 bytes)
        state.cursor_left(); // move back one CJK char: cursor at 9 (after 你好世)
        assert_eq!(state.cursor_pos, 9);
        assert!(state.delete_char_right()); // deletes 界
        assert_eq!(state.buffer, "你好世");
        assert_eq!(state.cursor_pos, 9); // cursor stays at same position
    }

    #[test]
    fn test_delete_char_right_at_middle() {
        // "abcde", cursor at position 2 (between b and c)
        let mut state = InputState::new();
        state.buffer = "abcde".to_string();
        state.cursor_pos = 2;
        assert!(state.delete_char_right()); // deletes 'c'
        assert_eq!(state.buffer, "abde");
        assert_eq!(state.cursor_pos, 2);
    }
}
