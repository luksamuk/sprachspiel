//! Terminal view implementation
//!
//! This module provides the `TerminalView` struct, which implements
//! the `ChatView` trait using standard terminal output (println!/eprintln!).

use crate::chat::strip_thinking_tags;
use crate::consts::roles::format_role_label;
use crate::markdown;

use super::super::session::{ChatSession, MessageRole};
use super::{truncate_str, ChatView, RecentContextInfo, RecentMessage, TokenMetrics, WelcomeInfo};

/// Terminal output backend using println!/eprintln!
///
/// This implementation provides:
/// - System message display (plain text)
/// - Error display (red colored)
/// - Assistant response display with markdown rendering
/// - Token metrics display
/// - Context warnings (yellow colored)
/// - Compaction progress/complete messages
pub struct TerminalView;

impl TerminalView {
    /// Create a new TerminalView instance
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
    pub fn show_welcome(
        &mut self,
        model_id: &str,
        tools_enabled: bool,
        think_enabled: bool,
        vision_enabled: bool,
        sandbox_status: &str,
        project: &str,
        session_name: &str,
        is_anonymous: bool,
        version: &str,
        server_url: &str,
        fact_count: i64,
        note_count: i64,
        doc_count: i64,
        skill_count: usize,
    ) {
        let info = WelcomeInfo {
            model_id: model_id.to_string(),
            tools_enabled,
            think_enabled,
            vision_enabled,
            sandbox_status: sandbox_status.to_string(),
            project: project.to_string(),
            session_name: session_name.to_string(),
            is_anonymous,
            version: version.to_string(),
            server_url: server_url.to_string(),
            fact_count,
            note_count,
            doc_count,
            skill_count,
        };
        println!("{}", info.to_boxed_string());
    }

    /// Display recent context summary for a resumed session.
    ///
    /// Shows the last 3 exchanges (user+assistant pairs) from the session,
    /// with role labels and truncated content. Only displayed when resuming
    /// a session with messages, not for new or anonymous sessions.
    pub fn show_recent_context(&mut self, session: &ChatSession) {
        const MAX_EXCHANGES: usize = 3;
        let exchanges = session.get_recent_exchanges(MAX_EXCHANGES);

        if exchanges.is_empty() {
            return;
        }

        let total_messages = session.messages.len();

        let recent_exchanges: Vec<(RecentMessage, Option<RecentMessage>)> = exchanges
            .into_iter()
            .map(|(user_msg, asst_msg)| {
                let role_str = match user_msg.role {
                    MessageRole::User => "user",
                    _ => "user",
                };
                let user = RecentMessage {
                    role_label: format_role_label(role_str),
                    content: truncate_str(
                        &strip_thinking_tags(&user_msg.content),
                        super::MAX_CONTEXT_LINE_LENGTH,
                    ),
                };
                let assistant = asst_msg.map(|a| {
                    let a_role_str = match a.role {
                        MessageRole::Assistant => "assistant",
                        _ => "assistant",
                    };
                    RecentMessage {
                        role_label: format_role_label(a_role_str),
                        content: truncate_str(
                            &strip_thinking_tags(&a.content),
                            super::MAX_CONTEXT_LINE_LENGTH,
                        ),
                    }
                });
                (user, assistant)
            })
            .collect();

        let info = RecentContextInfo {
            total_messages,
            exchanges: recent_exchanges,
        };

        let summary = info.format_context_summary();
        if !summary.is_empty() {
            println!("{}", summary);
        }
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
            "qwen3.5:4b",
            true,
            true,
            true,
            "enabled",
            "my-project",
            "default",
            false,
            "0.39.5",
            "127.0.0.1:11434",
            3,
            2,
            0,
            4,
        );
    }

    #[test]
    fn test_terminal_view_compact_complete() {
        let mut view = TerminalView::new();
        view.show_compact_complete(10, 3, 3);
        view.show_compact_complete(5, 0, 0);
    }
}
