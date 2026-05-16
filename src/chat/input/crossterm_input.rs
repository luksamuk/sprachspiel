//! Crossterm input backend for the TUI chat REPL
//!
//! This module provides the `CrosstermInput` struct, which implements
//! the `InputBackend` trait for history management in the TUI.
//!
//! NOTE: Text editing (buffer, cursor, selection) is handled by
//! `ratatui_textarea::TextArea` in `App`. This struct only manages
//! command history persistence and navigation state.
//!
//! # Architecture
//!
//! ```text
//! App
//!     ├─ TextArea (text editing: buffer, cursor, selection)
//!     ├─ ChatCompleter (tab completion)
//!     └─ CrosstermInput (history: load, save, navigation state)
//! ```

use std::path::PathBuf;

use super::{InputBackend, InputResult, default_history_path};

/// Maximum history entries to keep
const MAX_HISTORY: usize = 1000;

/// Input backend using crossterm for history management.
///
/// Text editing is handled by `TextArea` in `App`. This struct only
/// manages command history (persistence, dedup, navigation state).
/// The `history` and `history_pos`/`saved_buffer` fields are accessed
/// by `App::history_prev()` and `App::history_next()` directly.
pub struct CrosstermInput {
    /// Command history (most recent last)
    pub(crate) history: Vec<String>,
    /// Current position in history navigation (None = not navigating)
    pub(crate) history_pos: Option<usize>,
    /// Saved buffer before history navigation began
    pub(crate) saved_buffer: String,
    /// History file path for persistence
    pub(crate) history_path: PathBuf,
}

impl CrosstermInput {
    /// Create a new CrosstermInput
    ///
    /// The `model_names` parameter is accepted for API compatibility
    /// but is not used — tab completion is handled by `ChatCompleter`.
    ///
    /// History is automatically loaded from the default history file.
    pub fn new(_model_names: Vec<String>) -> Self {
        let mut input = Self {
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

impl Default for CrosstermInput {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
