//! View abstraction layer for chat REPL
//!
//! This module provides the `ChatView` trait for abstracting output rendering,
//! enabling future migration from terminal output to alternative rendering (e.g., TUI).
//!
//! # Architecture
//!
//! ```text
//! repl.rs (coordinator)
//!     ↓ uses
//! ChatView (trait)
//!     ↓ implemented by
//! TerminalView (current) ─── TuiView (future)
//! ```
//!
//! # TUI Migration
//!
//! When implementing ratatui.rs TUI:
//! - Add methods to trait for new rendering needs
//! - Implement `TuiView` struct in `src/chat/view/tui.rs`
//! - Update `repl.rs` to use the new implementation
//!
//! Methods are added incrementally as needed during refactoring.

mod terminal;

#[allow(unused_imports)] // Will be used when ChatView is implemented in repl.rs (Phase 7)
pub use terminal::TerminalView;

// Re-export TokenMetrics from core for consumers of this module
pub use crate::chat::core::TokenMetrics;

/// Abstraction for output rendering in the chat REPL
///
/// This trait enables the REPL to work with different output backends:
/// - `TerminalView`: Current implementation using println!/eprintln!
/// - `TuiView`: Future implementation for ratatui.rs TUI
///
/// # Example
///
/// ```ignore
/// use chat::view::ChatView;
///
/// let mut view = TerminalView::new();
/// view.show_welcome(&session, &model_config, &capabilities);
/// view.show_assistant_response(&content, thinking, &metrics);
/// view.show_error("Something went wrong");
/// ```
#[allow(dead_code)] // Will be used when ChatView is integrated in repl.rs (Phase 7)
pub trait ChatView {
    /// Display a system message (info, status, welcome)
    ///
    /// Used for:
    /// - Welcome banner on startup
    /// - Status messages (model switched, tools toggled)
    /// - Command results (compact complete, etc.)
    fn show_system(&mut self, message: &str);

    /// Display an error message
    ///
    /// Errors are typically shown in red/bold to catch user attention.
    fn show_error(&mut self, error: &str);

    /// Display an assistant response with optional thinking content
    ///
    /// For models with thinking support (e.g., DeepSeek R1), the thinking
    /// content is displayed separately (typically dimmed/italic) before
    /// the main response.
    ///
    /// # Arguments
    ///
    /// * `content` - The main response content (after thinking tags removed)
    /// * `thinking` - Optional thinking content to display first
    fn show_assistant_response(&mut self, content: &str, thinking: Option<&str>);

    /// Display token usage metrics
    ///
    /// Shows prompt tokens, response tokens, and total after a response.
    fn show_token_metrics(&mut self, metrics: &TokenMetrics);

    /// Display a context warning
    ///
    /// Used when context window is getting full (72%+ or 80%+ thresholds).
    /// Should be visually distinct (yellow/warning color).
    fn show_context_warning(&mut self, percent: u8, message: &str);

    /// Display compact progress indicator
    ///
    /// Shows when auto-compaction is in progress.
    fn show_compact_progress(&mut self, message: &str);

    /// Display a compact complete message
    ///
    /// Shows after compaction finishes, with count of messages compacted.
    fn show_compact_complete(
        &mut self,
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    );
}

/// Welcome information for display
///
/// Contains all the data needed to render a welcome banner.
/// This is deliberately a struct to allow different rendering strategies
/// (ASCII box for terminal, widgets for TUI).
#[allow(dead_code)] // Will be used when ChatView is integrated in repl.rs (Phase 7)
pub struct WelcomeInfo {
    pub model_id: String,
    pub tools_enabled: bool,
    pub think_enabled: bool,
    pub sandbox_status: String,
    pub project: String,
    pub session_name: String,
    pub is_anonymous: bool,
}

impl WelcomeInfo {
    /// Format the welcome banner as an ASCII box for terminal display
    #[allow(dead_code)] // Will be used when ChatView is integrated (Phase 7)
    pub fn to_boxed_string(&self) -> String {
        let mut output = String::new();
        output.push('\n');
        output.push_str("+==============================================================+\n");
        output.push_str("|  Ask-AI Chat                                                 |\n");
        output.push_str("+==============================================================+\n");

        output.push_str(&format!(
            "|  Model: {:52} |\n",
            truncate_str(&self.model_id, 52)
        ));

        if self.think_enabled {
            output.push_str(&format!("|  Think: {:52} |\n", "enabled"));
        }

        let tools_status = if self.tools_enabled {
            "enabled"
        } else {
            "disabled"
        };
        output.push_str(&format!("|  Tools: {:52} |\n", tools_status));

        output.push_str(&format!(
            "|  Sandbox: {:51} |\n",
            truncate_str(&self.sandbox_status, 51)
        ));
        output.push_str(&format!(
            "|  Project: {:50} |\n",
            truncate_str(&self.project, 50)
        ));

        let session_display = if self.is_anonymous {
            "anonymous (no persistence)".to_string()
        } else {
            self.session_name.clone()
        };
        output.push_str(&format!(
            "|  Session: {:50} |\n",
            truncate_str(&session_display, 49)
        ));

        output.push_str("+==============================================================+\n");
        output.push_str("|  Type /help for commands, /quit to exit                      |\n");
        output.push_str("+==============================================================+\n");

        output
    }
}

/// Truncate a string to a maximum length, adding ellipsis if truncated
#[allow(dead_code)] // Used by WelcomeInfo::to_boxed_string (Phase 7)
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_info_formatting() {
        let info = WelcomeInfo {
            model_id: "llama3.1".to_string(),
            tools_enabled: true,
            think_enabled: true,
            sandbox_status: "enabled (landlock)".to_string(),
            project: "my-project".to_string(),
            session_name: "default".to_string(),
            is_anonymous: false,
        };

        let output = info.to_boxed_string();
        assert!(output.contains("llama3.1"));
        assert!(output.contains("Project:"));
        assert!(output.contains("Session:"));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a very long string", 10), "this is...");
    }
}
