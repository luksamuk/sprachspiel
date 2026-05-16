//! Input abstraction layer for chat REPL
//!
//! This module provides the `InputBackend` trait for abstracting input handling,
//! enabling future migration from rustyline to alternative input methods (e.g., TUI).
//!
//! # Architecture
//!
//! ```text
//! repl.rs (coordinator)
//!     ↓ uses
//! InputBackend (trait)
//!     ↓ implemented by
//! CrosstermInput (current TUI) ─── RustylineInput (removed)
//! ```
//!
//! # TUI Migration
//!
//! The `RustylineInput` has been removed in PR2 (#146) because
//! rustyline and ratatui are technically incompatible (both require
//! raw mode and terminal control). `CrosstermInput` handles key events
//! via the crossterm event loop, which integrates naturally with ratatui.
//!
//! IMPORTANT: Review and remove any dead code after TUI is implemented.

use std::path::PathBuf;

mod crossterm_input;

pub use crossterm_input::CrosstermInput;

/// Result of reading a line from input
#[derive(Debug, Clone)]
pub enum InputResult {
    /// User entered a line of text
    Line(String),
    /// User pressed Ctrl+C (interrupt)
    Interrupted,
    /// User pressed Ctrl+D (EOF)
    Eof,
    /// An error occurred
    #[allow(dead_code)] // InputBackend trait contract
    Error(String),
}

/// Abstraction for input handling in the chat REPL
///
/// This trait enables the REPL to work with different input backends:
/// - `CrosstermInput`: TUI implementation using crossterm key events
///
/// # Example
///
/// ```ignore
/// use chat::input::{InputBackend, InputResult};
///
/// let mut input = CrosstermInput::new(model_names);
/// // In TUI mode, input is handled via key events, not read_line()
/// ```
pub trait InputBackend {
    /// Read a line from input with the given prompt
    ///
    /// This method is kept for trait compatibility but should not be
    /// called in TUI mode. In the TUI, input is handled via crossterm
    /// key events processed by `CrosstermInput::handle_key_event()`.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt string to display before input
    ///
    /// # Returns
    ///
    /// Returns `InputResult` indicating what happened.
    #[allow(dead_code)] // InputBackend trait contract
    fn read_line(&mut self, prompt: &str) -> InputResult;

    /// Add a line to the input history
    ///
    /// History is persisted across sessions when `save_history` is called.
    ///
    /// # Arguments
    ///
    /// * `line` - The line to add to history
    fn add_history(&mut self, line: &str);

    /// Save history to persistent storage
    ///
    /// Returns an error string if saving fails.
    fn save_history(&mut self) -> Result<(), String>;
}

/// Returns the default history file path for the chat REPL
///
/// The path is determined in this order:
/// 1. `$XDG_DATA_HOME/sprachspiel/chat_history.txt` (if XDG_DATA_HOME is set)
/// 2. `~/.local/share/sprachspiel/chat_history.txt` (fallback)
/// 3. `.chat_history.txt` (current directory, last resort)
pub fn default_history_path() -> PathBuf {
    use crate::consts::app;

    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        let path = PathBuf::from(data_home).join(app::APP_DATA_DIR);
        let _ = std::fs::create_dir_all(&path);
        path.join("chat_history.txt")
    } else if let Some(home_dir) = dirs::home_dir() {
        let path = home_dir
            .join(".local")
            .join("share")
            .join(app::APP_DATA_DIR);
        let _ = std::fs::create_dir_all(&path);
        path.join("chat_history.txt")
    } else {
        PathBuf::from(".chat_history.txt")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_history_path_returns_path() {
        let path = default_history_path();
        assert!(path.to_string_lossy().contains("chat_history"));
    }

    #[test]
    fn test_input_result_variants() {
        let line = InputResult::Line("test".to_string());
        let interrupted = InputResult::Interrupted;
        let eof = InputResult::Eof;
        let error = InputResult::Error("test error".to_string());

        match line {
            InputResult::Line(s) => assert_eq!(s, "test"),
            _ => panic!("Expected Line variant"),
        }

        match interrupted {
            InputResult::Interrupted => {}
            _ => panic!("Expected Interrupted variant"),
        }

        match eof {
            InputResult::Eof => {}
            _ => panic!("Expected Eof variant"),
        }

        match error {
            InputResult::Error(e) => assert_eq!(e, "test error"),
            _ => panic!("Expected Error variant"),
        }
    }
}
