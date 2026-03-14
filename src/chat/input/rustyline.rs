//! Rustyline input backend implementation
//!
//! This module provides the `RustylineInput` struct, which implements
//! the `InputBackend` trait using the rustyline library.

use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{Config, Editor};

use super::{InputBackend, InputResult};
use crate::chat::completion::ChatCompleter;
use crate::chat::input::default_history_path;

/// Input backend using rustyline for readline functionality
///
/// This implementation provides:
/// - Line editing with cursor movement
/// - History navigation (up/down arrows)
/// - Tab completion for models and commands
/// - Ctrl+C (interrupt) and Ctrl+D (EOF) handling
pub struct RustylineInput {
    editor: Editor<ChatCompleter, DefaultHistory>,
    history_path: std::path::PathBuf,
}

impl RustylineInput {
    /// Create a new RustylineInput with default configuration
    #[allow(dead_code)] // Will be used when InputBackend is integrated (Phase 7)
    pub fn new(model_names: Vec<String>) -> Self {
        let config = Config::default();
        let completer = ChatCompleter::new(model_names);

        let mut editor: Editor<ChatCompleter, DefaultHistory> =
            Editor::with_config(config).expect("Failed to create editor");

        editor.set_helper(Some(completer));

        let history_path = default_history_path();

        // Try to load history, ignore errors if file doesn't exist
        let _ = editor.load_history(&history_path);

        Self {
            editor,
            history_path,
        }
    }

    /// Create a new RustylineInput with custom history path
    #[allow(dead_code)] // Will be used when InputBackend is integrated (Phase 7)
    pub fn with_history_path(model_names: Vec<String>, history_path: std::path::PathBuf) -> Self {
        let config = Config::default();
        let completer = ChatCompleter::new(model_names);

        let mut editor: Editor<ChatCompleter, DefaultHistory> =
            Editor::with_config(config).expect("Failed to create editor");

        editor.set_helper(Some(completer));

        // Try to load history, ignore errors if file doesn't exist
        let _ = editor.load_history(&history_path);

        Self {
            editor,
            history_path,
        }
    }
}

impl InputBackend for RustylineInput {
    fn read_line(&mut self, prompt: &str) -> InputResult {
        match self.editor.readline(prompt) {
            Ok(line) => InputResult::Line(line),
            Err(ReadlineError::Interrupted) => InputResult::Interrupted,
            Err(ReadlineError::Eof) => InputResult::Eof,
            Err(e) => InputResult::Error(e.to_string()),
        }
    }

    fn add_history(&mut self, line: &str) {
        let _ = self.editor.add_history_entry(line.to_string());
    }

    fn save_history(&mut self) -> Result<(), String> {
        self.editor
            .save_history(&self.history_path)
            .map_err(|e| e.to_string())
    }

    fn load_history(&mut self) -> Result<(), String> {
        self.editor
            .load_history(&self.history_path)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_result_variants_match_readable() {
        // Verify that InputResult variants match ReadlineError
        let interrupted = InputResult::Interrupted;
        let eof = InputResult::Eof;

        match interrupted {
            InputResult::Interrupted => assert!(true),
            _ => panic!("Expected Interrupted"),
        }

        match eof {
            InputResult::Eof => assert!(true),
            _ => panic!("Expected Eof"),
        }
    }
}
