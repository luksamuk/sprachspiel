//! Input line component — user input display
//!
//! Renders the input line at the bottom of the TUI, showing
//! the prompt string and current input buffer.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use super::super::styles;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

    /// Insert a newline at the cursor position (for multi-line input)
    pub fn insert_newline(&mut self) {
        self.buffer.insert(self.cursor_pos, '\n');
        self.cursor_pos += 1;
    }

    /// Number of lines in the buffer (including the initial line)
    ///
    /// An empty buffer has 1 line. Each `\n` adds one more line.
    /// "hello" → 1 line, "hello\n" → 2 lines, "hello\nworld" → 2 lines
    pub fn line_count(&self) -> u16 {
        if self.buffer.is_empty() {
            return 1;
        }
        // Count newlines + 1, since each \n starts a new line.
        // "abc" → 1, "abc\n" → 2, "abc\ndef" → 2, "abc\ndef\n" → 3
        (self.buffer.matches('\n').count() + 1) as u16
    }

    /// Get the line and visual column position of the cursor.
    ///
    /// Returns (line_index, visual_col) where line_index is 0-based
    /// and visual_col is the number of visual columns before the cursor
    /// within that line (Unicode-aware).
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let before = &self.buffer[..self.cursor_pos];
        // Count newlines before cursor to get line index
        let line_index = before.matches('\n').count();
        // Find start of current line
        let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let text_before_cursor = &self.buffer[line_start..self.cursor_pos];
        let visual_col = text_before_cursor.width();
        (line_index, visual_col)
    }

    /// Move cursor to the start of the current line
    pub fn cursor_home(&mut self) {
        let line_start = self.buffer[..self.cursor_pos]
            .rfind('\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        self.cursor_pos = line_start;
    }

    /// Move cursor to the end of the current line
    pub fn cursor_end(&mut self) {
        let next_newline = self.buffer[self.cursor_pos..]
            .find('\n')
            .map(|p| self.cursor_pos + p)
            .unwrap_or(self.buffer.len());
        self.cursor_pos = next_newline;
    }

    /// Whether the buffer contains multiple lines
    pub fn is_multiline(&self) -> bool {
        self.buffer.contains('\n')
    }

    /// Move cursor up one line (in multi-line input).
    ///
    /// Maintains the visual column position if possible. Returns `true` if the
    /// cursor actually moved up, `false` if already on the first line.
    pub fn cursor_up(&mut self) -> bool {
        let (current_line, visual_col) = self.cursor_line_col();
        if current_line == 0 {
            return false;
        }
        let target_line = current_line - 1;
        let target_line_byte_start = line_byte_start(&self.buffer, target_line);
        let target_line_content = line_content(&self.buffer, target_line);
        let target_byte_col = visual_col_to_byte_offset(target_line_content, visual_col);
        self.cursor_pos = target_line_byte_start + target_byte_col;
        true
    }

    /// Move cursor down one line (in multi-line input).
    ///
    /// Maintains the visual column position if possible. Returns `true` if the
    /// cursor actually moved down, `false` if already on the last line.
    pub fn cursor_down(&mut self) -> bool {
        let (current_line, visual_col) = self.cursor_line_col();
        let total_lines = self.buffer.matches('\n').count() + 1;
        if current_line >= total_lines.saturating_sub(1) {
            return false;
        }
        let target_line = current_line + 1;
        let target_line_byte_start = line_byte_start(&self.buffer, target_line);
        let target_line_content = line_content(&self.buffer, target_line);
        let target_byte_col = visual_col_to_byte_offset(target_line_content, visual_col);
        self.cursor_pos = target_line_byte_start + target_byte_col;
        true
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

/// Convert a visual column offset to a byte offset within a string.
///
/// This is used for cursor positioning across lines in multi-line input.
/// Returns the byte offset of the character at or just before the given
/// visual column position.
fn visual_col_to_byte_offset(line: &str, visual_col: usize) -> usize {
    let mut col = 0;
    for (byte_offset, ch) in line.char_indices() {
        let ch_width = ch.width().unwrap_or(1);
        if col + ch_width > visual_col {
            return byte_offset;
        }
        col += ch_width;
    }
    line.len()
}

/// Get the byte offset of the start of a line (0-based line index).
///
/// Handles trailing-newline edge cases that `str::lines()` doesn't.
fn line_byte_start(buffer: &str, line_index: usize) -> usize {
    let mut current_line = 0;
    let mut byte_pos = 0;
    for (i, ch) in buffer.char_indices() {
        if current_line == line_index {
            return i;
        }
        if ch == '\n' {
            current_line += 1;
        }
        byte_pos = i + ch.len_utf8();
    }
    // If we've exhausted the buffer and the target is the last line
    if current_line == line_index {
        byte_pos
    } else {
        buffer.len()
    }
}

/// Get the content of a line (0-based line index) without the trailing newline.
///
/// Returns the text of the line, stripped of its trailing `\n` if present.
fn line_content(buffer: &str, line_index: usize) -> &str {
    let start = line_byte_start(buffer, line_index);
    let rest = &buffer[start..];
    // Find the end of this line (next \n or end of buffer)
    match rest.find('\n') {
        Some(end) => &rest[..end],
        None => rest,
    }
}

/// Render the input line
///
/// Shows ">>> " prompt when enabled, or a disabled indicator when
/// the LLM is processing. For multi-line input (Shift+Enter), each line
/// of the buffer is displayed on its own row, with ">>> " on the first
/// line and "... " continuation prompts on subsequent lines.
///
/// Cursor positioning is computed from `InputState::cursor_line_col()`.
pub fn render(f: &mut Frame, area: Rect, state: &InputState) {
    let prompt_style = styles::prompt_style();
    let dim_style = Style::default().add_modifier(Modifier::DIM);
    const PROMPT_WIDTH: u16 = 4; // ">>> " or "... "

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
        // Build lines with prompt prefixes
        let mut display_lines: Vec<Line> = Vec::new();
        for (i, text_line) in state.buffer.lines().enumerate() {
            let prompt = if i == 0 {
                Span::styled(">>> ", prompt_style)
            } else {
                Span::styled("... ", prompt_style)
            };
            display_lines.push(Line::from(vec![prompt, Span::raw(text_line.to_string())]));
        }

        // Handle trailing newline: if buffer ends with \n, show an empty continuation line
        if state.buffer.ends_with('\n') {
            display_lines.push(Line::from(vec![
                Span::styled("... ", prompt_style),
                Span::raw(String::new()),
            ]));
        }

        // If buffer is completely empty, show the prompt line
        if display_lines.is_empty() {
            display_lines.push(Line::from(vec![
                Span::styled(">>> ", prompt_style),
                Span::raw(String::new()),
            ]));
        }

        let text = Text::from(display_lines);

        // Compute vertical scroll: if the cursor is below the visible area, scroll down
        let (cursor_line, cursor_col) = state.cursor_line_col();
        let visible_lines = area.height as usize;
        let scroll_y = if cursor_line >= visible_lines {
            (cursor_line - visible_lines + 1) as u16
        } else {
            0
        };

        // Compute horizontal scroll for the cursor's line
        let cursor_visual_x = PROMPT_WIDTH + cursor_col as u16;
        let right_edge = area.width;
        let scroll_x = (cursor_visual_x + 1).saturating_sub(right_edge);

        let paragraph = Paragraph::new(text).scroll((scroll_y, scroll_x));
        f.render_widget(paragraph, area);

        // Set cursor position in the input area
        let cursor_y = area.y + cursor_line as u16 - scroll_y;
        let cursor_x = area.x + cursor_visual_x.saturating_sub(scroll_x);
        // Clamp cursor to visible area
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(1));
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

    // ── Multi-line input tests ──────────────────────────────────────

    #[test]
    fn test_insert_newline() {
        let mut state = InputState::new();
        state.insert_char('a');
        state.insert_char('b');
        state.insert_newline();
        state.insert_char('c');
        assert_eq!(state.buffer, "ab\nc");
        assert_eq!(state.cursor_pos, 4);
    }

    #[test]
    fn test_line_count() {
        let mut state = InputState::new();
        assert_eq!(state.line_count(), 1);

        state.insert_newline();
        assert_eq!(state.line_count(), 2);

        state.insert_newline();
        assert_eq!(state.line_count(), 3);

        state.clear();
        assert_eq!(state.line_count(), 1);
    }

    #[test]
    fn test_is_multiline() {
        let mut state = InputState::new();
        assert!(!state.is_multiline());

        state.insert_newline();
        assert!(state.is_multiline());

        state.clear();
        assert!(!state.is_multiline());
    }

    #[test]
    fn test_cursor_line_col() {
        let mut state = InputState::new();
        state.buffer = "abc\ndef\nghi".to_string();

        // Cursor at start of first line
        state.cursor_pos = 0;
        assert_eq!(state.cursor_line_col(), (0, 0));

        // Cursor at end of first line
        state.cursor_pos = 3;
        assert_eq!(state.cursor_line_col(), (0, 3));

        // Cursor at start of second line
        state.cursor_pos = 4; // after the \n
        assert_eq!(state.cursor_line_col(), (1, 0));

        // Cursor in middle of second line
        state.cursor_pos = 6;
        assert_eq!(state.cursor_line_col(), (1, 2));

        // Cursor at start of third line
        state.cursor_pos = 8; // after "def\n"
        assert_eq!(state.cursor_line_col(), (2, 0));
    }

    #[test]
    fn test_cursor_line_col_unicode() {
        let mut state = InputState::new();
        // "你好\n世界" — 你=2 cols, 好=2 cols, 世=2 cols, 界=2 cols
        state.buffer = "你好\n世界".to_string();

        // Cursor after 你好 (6 bytes, 4 visual cols)
        state.cursor_pos = 6;
        assert_eq!(state.cursor_line_col(), (0, 4));

        // Cursor after \n (7 bytes = start of line 2)
        state.cursor_pos = 7;
        assert_eq!(state.cursor_line_col(), (1, 0));

        // Cursor after 世 (10 bytes = 2 visual cols into line 2)
        state.cursor_pos = 10;
        assert_eq!(state.cursor_line_col(), (1, 2));
    }

    #[test]
    fn test_cursor_home_end() {
        let mut state = InputState::new();
        state.buffer = "abc\ndef".to_string();

        // Cursor in middle of second line
        state.cursor_pos = 5; // "d|ef"
        state.cursor_home();
        assert_eq!(state.cursor_pos, 4); // "|def"

        state.cursor_end();
        assert_eq!(state.cursor_pos, 7); // "def|"
    }

    #[test]
    fn test_cursor_up_down() {
        let mut state = InputState::new();
        state.buffer = "abcde\nfgh".to_string();

        // Cursor at end of second line (visual col 3)
        state.cursor_pos = 9; // "fgh|"
        assert!(!state.cursor_down()); // already on last line
        assert!(state.cursor_up()); // move to first line

        // Column 3 on first line: "abc|de"
        assert_eq!(state.cursor_pos, 3);

        // Move back down: column 3 on second line ("fgh|")
        assert!(state.cursor_down());
        // "fgh" has only 3 chars, col 3 → at end
        assert_eq!(state.cursor_pos, 9);
    }

    #[test]
    fn test_cursor_up_down_column_preservation() {
        let mut state = InputState::new();
        state.buffer = "abc\nfghij\nkl".to_string();

        // Place cursor at column 3 on second line (after "fgh")
        state.cursor_pos = 7; // "fgh|ij"
        assert_eq!(state.cursor_line_col(), (1, 3));

        // Move up: column 3 on first line ("abc|")
        assert!(state.cursor_up());
        assert_eq!(state.cursor_pos, 3);
        assert_eq!(state.cursor_line_col(), (0, 3));

        // Move down twice to get to third line
        assert!(state.cursor_down()); // back to second line
        assert!(state.cursor_down()); // to third line
        // Column 3 clamped to line length 2 ("kl|")
        assert_eq!(state.cursor_line_col(), (2, 2));
    }

    #[test]
    fn test_cursor_up_on_first_line() {
        let mut state = InputState::new();
        state.buffer = "abc\nfgh".to_string();
        state.cursor_pos = 2; // "ab|c"
        assert!(!state.cursor_up()); // can't move up from first line
        assert_eq!(state.cursor_pos, 2); // unchanged
    }

    #[test]
    fn test_backspace_at_line_boundary() {
        let mut state = InputState::new();
        state.buffer = "abc\ndef".to_string();
        state.cursor_pos = 4; // start of second line, after \n

        // Backspace should delete the \n, joining the lines
        state.backspace();
        assert_eq!(state.buffer, "abcdef");
        assert_eq!(state.cursor_pos, 3); // cursor now between 'c' and 'd'
    }

    #[test]
    fn test_visual_col_to_byte_offset_ascii() {
        assert_eq!(visual_col_to_byte_offset("hello", 0), 0);
        assert_eq!(visual_col_to_byte_offset("hello", 3), 3);
        assert_eq!(visual_col_to_byte_offset("hello", 5), 5); // at end
        assert_eq!(visual_col_to_byte_offset("hello", 10), 5); // past end → len
    }

    #[test]
    fn test_visual_col_to_byte_offset_unicode() {
        // 你=3 bytes, 2 cols; 好=3 bytes, 2 cols
        assert_eq!(visual_col_to_byte_offset("你好", 0), 0);
        assert_eq!(visual_col_to_byte_offset("你好", 2), 3); // after 你
        assert_eq!(visual_col_to_byte_offset("你好", 4), 6); // at end
        assert_eq!(visual_col_to_byte_offset("你好", 1), 0); // mid-char → start of 你
    }
}
