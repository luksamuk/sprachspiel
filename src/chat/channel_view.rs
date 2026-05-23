//! Channel-based ChatView proxy for async LLM tasks
//!
//! When the LLM runs in a background task, it can't hold a mutable reference
//! to the real `RatatuiView`. Instead, it uses `ChannelView`, which implements
//! `ChatView` but sends all view calls as `ViewAction` messages through an
//! mpsc channel. The main event loop drains these and applies them to the
//! real view.
//!
//! # Architecture
//!
//! ```text
//! LLM task                         Event loop
//!     ChannelView (ChatView)          RatatuiView (ChatView)
//!         │                                ↑
//!         └── mpsc::Sender<ViewAction> ────┘
//!                   (drain + apply)
//! ```

use super::command_output::CommandOutput;
use super::llm_event::ViewAction;
use super::view::{ChatView, TokenMetrics};

/// A `ChatView` implementation that sends all rendering calls through
/// an mpsc channel instead of rendering directly.
///
/// Used by the LLM background task so it can "render" without holding
/// a mutable reference to the real view. The event loop drains the
/// channel and applies each `ViewAction` to the real `RatatuiView`.
pub struct ChannelView {
    sender: tokio::sync::mpsc::Sender<ViewAction>,
}

impl ChannelView {
    /// Create a new `ChannelView` that sends view actions through the given sender.
    pub fn new(sender: tokio::sync::mpsc::Sender<ViewAction>) -> Self {
        Self { sender }
    }

    /// Send a view action through the channel.
    ///
    /// If the receiver has been dropped (event loop ended), silently ignores the error.
    fn send(&self, action: ViewAction) {
        let _ = self.sender.try_send(action);
    }
}

impl ChatView for ChannelView {
    fn show_system(&mut self, message: &str) {
        self.send(ViewAction::ShowSystem(message.to_string()));
    }

    fn show_error(&mut self, error: &str) {
        self.send(ViewAction::ShowError(error.to_string()));
    }

    fn show_assistant_response(&mut self, content: &str, thinking: Option<&str>) {
        self.send(ViewAction::ShowAssistantResponse {
            content: content.to_string(),
            thinking: thinking.map(|s| s.to_string()),
        });
    }

    fn show_token_metrics(&mut self, metrics: &TokenMetrics) {
        self.send(ViewAction::ShowTokenMetrics(TokenMetrics {
            prompt_tokens: metrics.prompt_tokens,
            response_tokens: metrics.response_tokens,
            total_tokens: metrics.total_tokens,
        }));
    }

    fn show_context_warning(&mut self, percent: u8, message: &str) {
        self.send(ViewAction::ShowContextWarning {
            percent,
            message: message.to_string(),
        });
    }

    fn show_compact_progress(&mut self, message: &str) {
        self.send(ViewAction::ShowCompactProgress(message.to_string()));
    }

    fn show_compact_complete(
        &mut self,
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    ) {
        self.send(ViewAction::ShowCompactComplete {
            count,
            preserved_first,
            preserved_last,
        });
    }

    fn show_markdown(&mut self, content: &str) {
        self.send(ViewAction::ShowMarkdown(content.to_string()));
    }

    fn show_thinking(&mut self, thinking: &str) {
        self.send(ViewAction::ShowThinking(thinking.to_string()));
    }

    fn show_help_line(&mut self) {
        self.send(ViewAction::ShowSystem(
            "Type /help for commands, /quit to exit".to_string(),
        ));
    }

    fn clear_continuation_line(&mut self) {
        self.send(ViewAction::ClearContinuationLine);
    }

    fn show_command_output(&mut self, output: &CommandOutput) {
        self.send(ViewAction::ShowCommandOutput(output.clone()));
    }

    fn suppress_progress_spinner(&self) -> bool {
        // ChannelView is used in TUI mode — always suppress indicatif spinners
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_view_sends_show_system() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ViewAction>(16);
        let mut view = ChannelView::new(tx);

        view.show_system("Hello");

        let action = rx.try_recv().unwrap();
        match action {
            ViewAction::ShowSystem(msg) => assert_eq!(msg, "Hello"),
            _ => panic!("Expected ShowSystem, got {:?}", action),
        }
    }

    #[test]
    fn test_channel_view_sends_show_error() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ViewAction>(16);
        let mut view = ChannelView::new(tx);

        view.show_error("Something went wrong");

        let action = rx.try_recv().unwrap();
        match action {
            ViewAction::ShowError(msg) => assert_eq!(msg, "Something went wrong"),
            _ => panic!("Expected ShowError, got {:?}", action),
        }
    }

    #[test]
    fn test_channel_view_suppresses_spinner() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<ViewAction>(16);
        let view = ChannelView::new(tx);
        assert!(view.suppress_progress_spinner());
    }

    #[tokio::test]
    async fn test_channel_view_drops_silently_on_closed_channel() {
        let (tx, rx) = tokio::sync::mpsc::channel::<ViewAction>(16);
        let mut view = ChannelView::new(tx);

        // Drop the receiver
        drop(rx);

        // Should not panic when sending to closed channel
        view.show_system("This should be silently ignored");
    }
}
