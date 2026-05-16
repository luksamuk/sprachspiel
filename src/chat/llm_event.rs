//! LLM events for the async TUI event loop
//!
//! When the LLM runs in a background task, it communicates with the
//! main event loop through `LlmEvent` messages sent via `mpsc` channel.
//!
//! # Architecture
//!
//! ```text
//! Event loop (main task)          LLM task (spawned)
//!     ←── LlmEvent::ViewAction ── ChannelView (ChatView proxy)
//!     ←── LlmEvent::Complete  ── Result of handle_user_message()
//!     ←── LlmEvent::Error     ── LLM error
//!     ──→ CancellationToken    ── Ctrl+C cancellation
//! ```

use super::command_output::CommandOutput;
use super::session::ChatSession;
use super::view::TokenMetrics;

/// Events sent from the LLM background task to the main event loop.
pub enum LlmEvent {
    /// A view action was triggered during LLM processing.
    ///
    /// These are `ChatView` method calls made by the LLM handler
    /// (via `ChannelView`), forwarded to the event loop for rendering.
    ViewAction(ViewAction),

    /// The LLM call completed successfully.
    ///
    /// Contains the updated session (with new messages, tokens, etc.)
    /// and final token counts for the status bar update.
    Complete {
        /// Updated session after the LLM call (boxed to reduce enum size)
        session: Box<ChatSession>,
        /// Total tokens used after the response
        used_tokens: usize,
        /// Maximum context window tokens
        max_tokens: usize,
        /// Context usage percentage (0-100)
        percent: u8,
    },

    /// The LLM call failed with an error.
    #[allow(dead_code)] // Error enum variant — used by LLM task error handling
    Error(String),

    /// The LLM call was cancelled by the user (Ctrl+C).
    Cancelled,
}

impl std::fmt::Debug for LlmEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ViewAction(action) => f.debug_tuple("ViewAction").field(action).finish(),
            Self::Complete {
                session: _,
                used_tokens,
                max_tokens,
                percent,
            } => f
                .debug_struct("Complete")
                .field("used_tokens", used_tokens)
                .field("max_tokens", max_tokens)
                .field("percent", percent)
                .finish_non_exhaustive(),
            Self::Error(msg) => f.debug_tuple("Error").field(msg).finish(),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// View actions forwarded from the LLM background task.
///
/// Each variant corresponds to a `ChatView` method call.
/// The event loop applies these to the real `RatatuiView`.
#[derive(Debug)]
pub enum ViewAction {
    /// `ChatView::show_system(message)`
    ShowSystem(String),
    /// `ChatView::show_error(error)`
    ShowError(String),
    /// `ChatView::show_assistant_response(content, thinking)`
    ShowAssistantResponse {
        content: String,
        thinking: Option<String>,
    },
    /// `ChatView::show_token_metrics(metrics)`
    ShowTokenMetrics(TokenMetrics),
    /// `ChatView::show_context_warning(percent, message)`
    ShowContextWarning { percent: u8, message: String },
    /// `ChatView::show_compact_progress(message)`
    ShowCompactProgress(String),
    /// `ChatView::show_compact_complete(count, preserved_first, preserved_last)`
    ShowCompactComplete {
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    },
    /// `ChatView::show_markdown(content)`
    ShowMarkdown(String),
    /// `ChatView::show_thinking(thinking)`
    ShowThinking(String),
    /// `ChatView::clear_continuation_line()`
    ClearContinuationLine,
    /// `ChatView::show_command_output(output)`
    ShowCommandOutput(CommandOutput),
}
