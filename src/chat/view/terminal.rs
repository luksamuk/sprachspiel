//! Terminal view implementation
//!
//! This module provides the `TerminalView` struct, which implements
//! the `ChatView` trait using standard terminal output (println!/eprintln!).

use crate::markdown;

use super::{ChatView, TokenMetrics, WelcomeInfo};

/// Terminal output backend using println!/eprintln!
///
/// This implementation provides:
/// - System message display (plain text)
/// - Error display (red colored)
/// - Assistant response display with markdown rendering
/// - Token metrics display
/// - Context warnings (yellow colored)
/// - Compaction progress/complete messages
#[allow(dead_code)] // Will be used when ChatView is integrated in repl.rs (Phase 7)
pub struct TerminalView;

impl TerminalView {
    /// Create a new TerminalView instance
    #[allow(dead_code)] // Will be used when ChatView is integrated (Phase 7)
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatView for TerminalView {
    fn show_system(&mut self, message: &str) {
        println!("{}", message);
    }

    fn show_error(&mut self, error: &str) {
        eprintln!("\x1B[31m{}\x1B[0m", error);
    }

    fn show_assistant_response(&mut self, content: &str, thinking: Option<&str>) {
        // Display thinking content first if present (dimmed)
        if let Some(thinking_content) = thinking {
            // Thinking is already formatted by the thinking module
            // We just display it before the main content
            let _ = thinking_content; // Thinking is handled separately by display_thinking
        }

        // Display the main response content as markdown
        markdown::print_markdown(content);
    }

    fn show_token_metrics(&mut self, metrics: &TokenMetrics) {
        if metrics.total_tokens > 0 {
            eprintln!(
                "\n\x1B[90m[Tokens: {} prompt + {} response = {} total]\x1B[0m",
                metrics.prompt_tokens, metrics.response_tokens, metrics.total_tokens
            );
        }
    }

    fn show_context_warning(&mut self, percent: u8, message: &str) {
        eprintln!("\x1B[33m⚠ Context {}% full. {}\x1B[0m", percent, message);
    }

    fn show_compact_progress(&mut self, message: &str) {
        eprintln!("\x1B[33m⏳ {}\x1B[0m", message);
    }

    fn show_compact_complete(
        &mut self,
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    ) {
        if preserved_first > 0 || preserved_last > 0 {
            // Middle compaction
            eprintln!(
                "\x1B[32m✓ Compacted {} messages\x1B[0m (preserved {} first, {} last).",
                count, preserved_first, preserved_last
            );
        } else {
            // Full compaction (backward compatible)
            eprintln!("\x1B[32m✓ Compacted all {} messages.\x1B[0m", count);
        }
    }
}

impl TerminalView {
    /// Display the welcome banner
    ///
    /// This is a convenience method that uses WelcomeInfo internally.
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)] // Will be used when ChatView is integrated (Phase 7)
    pub fn show_welcome(
        &mut self,
        model_id: &str,
        tools_enabled: bool,
        think_enabled: bool,
        sandbox_status: &str,
        project: &str,
        session_name: &str,
        is_anonymous: bool,
    ) {
        let info = WelcomeInfo {
            model_id: model_id.to_string(),
            tools_enabled,
            think_enabled,
            sandbox_status: sandbox_status.to_string(),
            project: project.to_string(),
            session_name: session_name.to_string(),
            is_anonymous,
        };
        println!("{}", info.to_boxed_string());
    }

    /// Display context utilization bar
    ///
    /// Shows a visual bar indicating context window usage.
    #[allow(dead_code)] // Will be used when ChatView is integrated (Phase 7)
    pub fn show_context_bar(&mut self, percent: u8, total_tokens: usize, context_window: usize) {
        let bar_width = 20;
        let filled = ((percent as usize).min(100) * bar_width) / 100;
        let empty = bar_width - filled;

        // Color code based on usage
        let (color_code, reset_code) = if percent < 72 {
            ("\x1B[32m", "\x1B[0m") // Green
        } else if percent < 80 {
            ("\x1B[33m", "\x1B[0m") // Yellow
        } else {
            ("\x1B[31m", "\x1B[0m") // Red
        };

        println!();
        println!("  Context Utilization:");
        println!(
            "    {}{}{}{} {}{}",
            color_code,
            "█".repeat(filled),
            "░".repeat(empty),
            reset_code,
            color_code,
            percent
        );
        println!(
            "    {}{} / {} tokens{}\x1B[0m",
            color_code, total_tokens, context_window, reset_code
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_view_show_system() {
        let mut view = TerminalView::new();
        // This would print to stdout, we just verify it compiles
        view.show_system("Test message");
    }

    #[test]
    fn test_terminal_view_show_error() {
        let mut view = TerminalView::new();
        // This would print to stderr in red, we just verify it compiles
        view.show_error("Test error");
    }

    #[test]
    fn test_terminal_view_show_welcome() {
        let mut view = TerminalView::new();
        view.show_welcome(
            "llama3.1",
            true,
            true,
            "enabled",
            "my-project",
            "default",
            false,
        );
    }

    #[test]
    fn test_terminal_view_compact_complete() {
        let mut view = TerminalView::new();
        view.show_compact_complete(10, 3, 3);
        view.show_compact_complete(5, 0, 0);
    }
}
