//! Crossterm input backend for the TUI chat REPL
//!
//! This module provides the `CrosstermInput` struct, which implements
//! the `InputBackend` trait using crossterm event handling instead of
//! rustyline. This is required for the ratatui TUI because rustyline
//! and ratatui both require raw mode and are incompatible.
//!
//! # Supported keys
//!
//! - Enter (submit line)
//! - Backspace (delete character before cursor)
//! - Ctrl+C (interrupt)
//! - Ctrl+D (EOF/exit)
//! - Left/Right arrows (cursor movement)
//! - Up/Down arrows (history navigation)
//! - Regular printable characters (typed input)
//!
//! Tab completion is deferred to PR3.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

use super::{InputBackend, InputResult, default_history_path};

/// Maximum history entries to keep
const MAX_HISTORY: usize = 1000;

/// Input backend using crossterm for key event handling
///
/// This implementation provides basic line editing with history
/// navigation, suitable for use inside a ratatui terminal application.
/// It does NOT handle raw mode or terminal setup — that's the
/// responsibility of the TUI module.
pub struct CrosstermInput {
    /// Current input buffer
    #[allow(dead_code)] // PR3: Will be used for TUI input buffer access
    pub(crate) buffer: String,
    /// Cursor position within the buffer (byte offset)
    #[allow(dead_code)] // PR3: Will be used for TUI cursor positioning
    pub(crate) cursor_pos: usize,
    /// Command history (most recent last)
    pub(crate) history: Vec<String>,
    /// Current position in history navigation (None = not navigating)
    pub(crate) history_pos: Option<usize>,
    /// Saved buffer before history navigation began
    pub(crate) saved_buffer: String,
    /// History file path for persistence
    pub(crate) history_path: PathBuf,
}

#[allow(dead_code)] // PR3: Will be used for TUI key event handling
impl CrosstermInput {
    /// Create a new CrosstermInput
    ///
    /// The `model_names` parameter is accepted for API compatibility
    /// with `InputBackend` but tab completion is not yet implemented.
    /// It will be used in PR3 for tab completion.
    ///
    /// History is automatically loaded from the default history file.
    pub fn new(_model_names: Vec<String>) -> Self {
        let mut input = Self {
            buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_pos: None,
            saved_buffer: String::new(),
            history_path: default_history_path(),
        };
        input.load_history();
        input
    }

    /// Create a CrosstermInput with a custom history path (for testing)
    ///
    /// Does NOT load history from the file, allowing tests to start
    /// with a clean state.
    #[cfg(test)]
    fn new_with_path(history_path: PathBuf) -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_pos: None,
            saved_buffer: String::new(),
            history_path,
        }
    }

    /// Load history from the history file
    pub(crate) fn load_history(&mut self) {
        if let Ok(contents) = std::fs::read_to_string(&self.history_path) {
            self.history = contents.lines().map(|l| l.to_string()).collect();
            // Keep only MAX_HISTORY entries
            if self.history.len() > MAX_HISTORY {
                let drain_count = self.history.len() - MAX_HISTORY;
                self.history.drain(..drain_count);
            }
        }
    }

    /// Handle a key event from crossterm
    ///
    /// This method processes a single key event and returns:
    /// - `Some(InputResult::Line(...))` when Enter is pressed
    /// - `Some(InputResult::Interrupted)` when Ctrl+C is pressed
    /// - `Some(InputResult::Eof)` when Ctrl+D is pressed on an empty line
    /// - `None` for other keys (buffer updated internally)
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<InputResult> {
        match key {
            // Enter — submit the line
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                let line = self.buffer.clone();
                self.buffer.clear();
                self.cursor_pos = 0;
                self.history_pos = None;
                self.saved_buffer.clear();
                Some(InputResult::Line(line))
            }

            // Ctrl+C — interrupt
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(InputResult::Interrupted),

            // Ctrl+D — EOF (exit if empty line, otherwise delete char)
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.buffer.is_empty() {
                    Some(InputResult::Eof)
                } else {
                    self.delete_char_right();
                    None
                }
            }

            // Backspace — delete char before cursor
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.backspace();
                None
            }

            // Delete — delete char at cursor
            KeyEvent {
                code: KeyCode::Delete,
                ..
            } => {
                self.delete_char_right();
                None
            }

            // Left arrow — move cursor left
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.cursor_left();
                None
            }

            // Right arrow — move cursor right
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.cursor_right();
                None
            }

            // Up arrow — history navigation (previous)
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.history_prev();
                None
            }

            // Down arrow — history navigation (next)
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.history_next();
                None
            }

            // Home — move cursor to start
            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.cursor_pos = 0;
                None
            }

            // End — move cursor to end
            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.cursor_pos = self.buffer.len();
                None
            }

            // Regular character — insert at cursor
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => {
                self.insert_char(c);
                None
            }

            // Ignore other key events
            _ => None,
        }
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete the character before the cursor
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev_pos = self.buffer[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.buffer.drain(prev_pos..self.cursor_pos);
            self.cursor_pos = prev_pos;
        }
    }

    /// Delete the character at the cursor (Delete key)
    fn delete_char_right(&mut self) {
        if self.cursor_pos < self.buffer.len() {
            let next_pos = self.buffer[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.buffer.len());
            self.buffer.drain(self.cursor_pos..next_pos);
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

    /// Navigate to previous history entry
    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        if self.history_pos.is_none() {
            // Start history navigation: save current buffer
            self.saved_buffer = self.buffer.clone();
            self.history_pos = Some(self.history.len().saturating_sub(1));
        } else if let Some(pos) = self.history_pos {
            if pos > 0 {
                self.history_pos = Some(pos - 1);
            } else {
                return; // Already at oldest entry
            }
        }

        if let Some(pos) = self.history_pos {
            self.buffer = self.history[pos].clone();
            self.cursor_pos = self.buffer.len();
        }
    }

    /// Navigate to next history entry
    fn history_next(&mut self) {
        match self.history_pos {
            None => {} // Not in history navigation
            Some(pos) => {
                if pos + 1 >= self.history.len() {
                    // Past the newest entry: restore saved buffer
                    self.history_pos = None;
                    self.buffer = self.saved_buffer.clone();
                    self.cursor_pos = self.buffer.len();
                } else {
                    self.history_pos = Some(pos + 1);
                    self.buffer = self.history[pos + 1].clone();
                    self.cursor_pos = self.buffer.len();
                }
            }
        }
    }

    /// Get the current buffer content
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Get the cursor position
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
        self.history_pos = None;
        self.saved_buffer.clear();
    }
}

impl InputBackend for CrosstermInput {
    fn read_line(&mut self, _prompt: &str) -> InputResult {
        // Note: This method is kept for InputBackend trait compatibility.
        // In the TUI, input is handled via crossterm events in the main
        // event loop, not via this blocking read_line method.
        // The prompt is displayed by the TUI, not by this method.
        //
        // This method should not be called in the TUI context.
        // It exists only so that CrosstermInput satisfies InputBackend.
        InputResult::Error("CrosstermInput::read_line should not be called in TUI mode".to_string())
    }

    fn add_history(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        // Don't add duplicates of the last entry
        if self.history.last().is_some_and(|last| last == line) {
            return;
        }
        self.history.push(line.to_string());
        // Keep only MAX_HISTORY entries
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
    }

    fn save_history(&mut self) -> Result<(), String> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.history_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = self.history.join("\n");
        std::fs::write(&self.history_path, content)
            .map_err(|e| format!("Failed to save history: {}", e))
    }
}

// Implement Default for CrosstermInput
impl Default for CrosstermInput {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() {
        let mut input =
            CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_insert.txt"));
        input.insert_char('a');
        input.insert_char('b');
        input.insert_char('c');
        assert_eq!(input.buffer(), "abc");
        assert_eq!(input.cursor_pos(), 3);
    }

    #[test]
    fn test_backspace() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_bs.txt"));
        input.insert_char('a');
        input.insert_char('b');
        input.backspace();
        assert_eq!(input.buffer(), "a");
        assert_eq!(input.cursor_pos(), 1);
    }

    #[test]
    fn test_cursor_movement() {
        let mut input =
            CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_cursor.txt"));
        input.insert_char('a');
        input.insert_char('b');
        input.insert_char('c');
        assert_eq!(input.cursor_pos(), 3);

        input.cursor_left();
        assert_eq!(input.cursor_pos(), 2);

        input.cursor_right();
        assert_eq!(input.cursor_pos(), 3);
    }

    #[test]
    fn test_unicode_handling() {
        let mut input =
            CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_unicode.txt"));
        input.insert_char('你');
        input.insert_char('好');
        assert_eq!(input.buffer(), "你好");
        assert_eq!(input.cursor_pos(), 6); // 3 bytes per character

        input.cursor_left();
        assert_eq!(input.cursor_pos(), 3); // Moved back one char (3 bytes)

        input.insert_char('世');
        assert_eq!(input.buffer(), "你世好");
    }

    #[test]
    fn test_history_navigation() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_nav.txt"));
        input.add_history("hello");
        input.add_history("world");
        assert_eq!(input.history.len(), 2);

        // Start history navigation
        input.history_prev();
        assert_eq!(input.buffer(), "world"); // Most recent
        assert_eq!(input.history_pos, Some(1));

        input.history_prev();
        assert_eq!(input.buffer(), "hello"); // Oldest
        assert_eq!(input.history_pos, Some(0));

        // Can't go further back
        input.history_prev();
        assert_eq!(input.history_pos, Some(0));

        // Go forward
        input.history_next();
        assert_eq!(input.buffer(), "world");
        assert_eq!(input.history_pos, Some(1));

        // Past end — restore saved buffer
        input.history_next();
        assert_eq!(input.history_pos, None);
    }

    #[test]
    fn test_add_history_dedup() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_dedup.txt"));
        input.add_history("hello");
        input.add_history("hello"); // Should not add duplicate
        assert_eq!(input.history.len(), 1);

        input.add_history("world");
        input.add_history("hello"); // Different from last, should add
        assert_eq!(input.history.len(), 3);
    }

    #[test]
    fn test_add_history_empty() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_empty.txt"));
        input.add_history("");
        assert!(input.history.is_empty());
    }

    #[test]
    fn test_handle_key_event_enter() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_enter.txt"));
        input.insert_char('h');
        input.insert_char('i');

        let result = input.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, Some(InputResult::Line(ref s)) if s == "hi"));
        assert!(input.buffer().is_empty());
    }

    #[test]
    fn test_handle_key_event_ctrl_c() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_ctrlc.txt"));
        let result =
            input.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(result, Some(InputResult::Interrupted)));
    }

    #[test]
    fn test_handle_key_event_ctrl_d_empty() {
        let mut input = CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_ctrld.txt"));
        let result =
            input.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(matches!(result, Some(InputResult::Eof)));
    }

    #[test]
    fn test_handle_key_event_ctrl_d_with_content() {
        let mut input =
            CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_ctrld_content.txt"));
        input.insert_char('a');
        input.cursor_pos = 0; // Move cursor to start
        let result =
            input.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(result.is_none()); // Doesn't exit, deletes char
    }

    #[test]
    fn test_read_line_returns_error() {
        // read_line should not be called in TUI mode
        let mut input =
            CrosstermInput::new_with_path(PathBuf::from("/tmp/test_history_readline.txt"));
        let result = input.read_line(">>> ");
        assert!(matches!(result, InputResult::Error(_)));
    }

    #[test]
    fn test_save_and_load_history() {
        let temp_dir = std::env::temp_dir();
        let history_path = temp_dir.join("sprachspiel_test_history_unit.txt");

        let mut input = CrosstermInput::new_with_path(history_path.clone());
        input.add_history("test line 1");
        input.add_history("test line 2");

        let result = input.save_history();
        assert!(result.is_ok());

        // Load history
        let mut input2 = CrosstermInput::new_with_path(history_path.clone());
        input2.load_history();
        assert_eq!(input2.history.len(), 2);
        assert_eq!(input2.history[0], "test line 1");
        assert_eq!(input2.history[1], "test line 2");

        // Cleanup
        let _ = std::fs::remove_file(&history_path);
    }
}
