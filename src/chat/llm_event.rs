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
//!     ←── LlmEvent::StreamToken ── Streaming token chunk
//!     ←── LlmEvent::StreamThinking ── Streaming thinking chunk
//!     ←── LlmEvent::StreamDone ── Streaming complete (final_data)
//!     ←── LlmEvent::ToolCallStarted ── Tool calls detected in stream
//!     ←── LlmEvent::Complete  ── Result of handle_user_message()
//!     ←── LlmEvent::Error     ── LLM error
//!     ──→ CancellationToken    ── Ctrl+C cancellation
//! ```
//!
//! # Tool Message Ordering
//!
//! Tool calls and results are lifecycle events (`ToolCallPreview`,
//! `ToolExecutionStarted`, `ToolExecutionFinished`). They are rendered inside
//! the volatile `LiveTurn` and committed to history together with the rest of
//! the assistant turn. No separate channel is used for tool indicators.

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

    /// A streaming token chunk arrived from the LLM.
    ///
    /// The event loop should append this text to the CURRENT
    /// `AssistantStreaming` message in the chat area.
    StreamToken(String),

    /// A streaming thinking chunk arrived from the LLM.
    ///
    /// The event loop should append this text to the CURRENT
    /// thinking block being streamed.
    StreamThinking(String),

    /// Streaming is complete — replace the FINAL streaming message with
    /// the final markdown-rendered assistant response.
    ///
    /// This is the last event of a streaming turn. It finalizes the
    /// LAST content block (post-tool, or the only block if no tools).
    /// For turns with tool calls, the pre-tool block is finalized by
    /// `ToolCallStarted` (which calls `finalize_streaming_zone_as_is`).
    StreamDone {
        /// Full accumulated response content (for markdown rendering)
        content: String,
        /// Thinking content (if any, already accumulated)
        thinking: Option<String>,
        /// Token metrics from the final streaming chunk
        metrics: Option<TokenMetrics>,
    },

    /// The LLM call completed successfully (non-streaming path).
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

    /// Tool calls were detected in the streaming response.
    ///
    /// The event loop should transition the LLM state to `ToolCall`
    /// so the status bar shows "Running tool..." with a fresh spinner.
    ToolCallStarted,

    /// Partial tool call known so far (name + parsed partial arguments).
    ///
    /// Emitted while the LLM is still streaming `tool_calls` deltas.
    /// The TUI should display this as a transient tool message in the
    /// current turn, updating it as new deltas arrive. Once the call is
    /// finalized, the preview is frozen into a normal tool message.
    ToolCallPreview {
        /// Tool-call id (may be empty if not yet provided by the provider).
        tool_call_id: String,
        /// Tool name.
        name: String,
        /// Parsed partial arguments (best-effort JSON object).
        args: serde_json::Value,
    },

    /// A tool execution has started.
    ///
    /// Currently only start/end are emitted. Future work will add
    /// `ToolExecutionOutput` for long-running tools.
    ToolExecutionStarted {
        /// Tool-call id this execution belongs to.
        tool_call_id: String,
        /// Tool name.
        name: String,
        /// Final parsed arguments.
        args: serde_json::Value,
    },

    /// A tool execution has finished.
    ToolExecutionFinished {
        /// Tool-call id this execution belongs to.
        tool_call_id: String,
        /// Tool result string (or error message).
        result: String,
        /// Whether the tool returned an error.
        is_error: bool,
    },

    /// Provider is about to retry a failed HTTP request.
    ///
    /// Rendered in the status bar (right-aligned, red) so the user sees
    /// transient failures such as "model warming up" instead of a frozen UI.
    ProviderRetryStarted {
        /// 1-based attempt number.
        attempt: u32,
        /// Maximum attempts configured.
        max_attempts: u32,
        /// Delay before the next attempt, in milliseconds.
        delay_ms: u64,
        /// Human-readable reason (e.g., "model warming up").
        reason: String,
    },

    /// Provider retry finished.
    ProviderRetryFinished {
        /// Whether the retry succeeded.
        success: bool,
        /// Last attempt number.
        attempt: u32,
    },

    /// Token metrics from a completed ReAct round (intermediate status bar update).
    ///
    /// Emitted after each `stream_turn` completes, carrying the real
    /// `prompt_tokens` from the provider's `Done` event. This lets the
    /// status bar update during multi-round ReAct loops instead of
    /// freezing until the final `Complete`.
    ///
    /// Only emitted when the provider actually sends `usage`. When absent,
    /// the status bar falls back to `estimate_status_bar()`.
    TurnMetrics {
        /// Prompt tokens from the provider's usage report.
        prompt_tokens: u32,
        /// Completion tokens from the provider's usage report.
        completion_tokens: u32,
    },

    /// A streaming token chunk from compaction.
    ///
    /// Display as `AssistantStreaming` in the chat area, just like
    /// `StreamToken` but from a compaction LLM call rather than a
    /// regular user message.
    CompactStreamToken(String),

    /// System-level information during compaction (chunk count, truncation warning).
    ///
    /// Displayed as a dim `System` message in the chat area, separate from the
    /// streaming summary content. Used for progress info like "⚙ Compacting in 3 chunks..."
    /// and warnings like "⚠ Truncation applied, dropped 150 oldest messages."
    CompactInfo {
        /// Informational message to display
        message: String,
    },

    /// Compaction streaming completed.
    ///
    /// Contains the full compacted summary and the range of messages
    /// that were summarized. The event loop should finalize the
    /// streaming zone, update the session, save to database, and
    /// show the compact result.
    CompactStreamDone {
        /// The full compacted summary text
        summary: String,
        /// Range of compacted messages: (first_preserved, last_preserved_start)
        range: Option<(usize, usize)>,
    },
}

impl std::fmt::Debug for LlmEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ViewAction(action) => f.debug_tuple("ViewAction").field(action).finish(),
            Self::StreamToken(token) => {
                // Truncate token display to avoid flooding debug logs
                let display = if token.len() > 20 {
                    format!("{}...", &token[..20])
                } else {
                    token.clone()
                };
                f.debug_tuple("StreamToken").field(&display).finish()
            }
            Self::StreamThinking(token) => {
                let display = if token.len() > 20 {
                    format!("{}...", &token[..20])
                } else {
                    token.clone()
                };
                f.debug_tuple("StreamThinking").field(&display).finish()
            }
            Self::StreamDone {
                content: _,
                thinking: _,
                metrics,
            } => f
                .debug_struct("StreamDone")
                .field("metrics", metrics)
                .finish_non_exhaustive(),
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
            Self::ToolCallStarted => write!(f, "ToolCallStarted"),
            Self::ToolCallPreview {
                tool_call_id,
                name,
                args,
            } => f
                .debug_struct("ToolCallPreview")
                .field("tool_call_id", tool_call_id)
                .field("name", name)
                .field("args", args)
                .finish_non_exhaustive(),
            Self::ToolExecutionStarted {
                tool_call_id,
                name,
                args,
            } => f
                .debug_struct("ToolExecutionStarted")
                .field("tool_call_id", tool_call_id)
                .field("name", name)
                .field("args", args)
                .finish_non_exhaustive(),
            Self::ToolExecutionFinished {
                tool_call_id,
                result,
                is_error,
            } => f
                .debug_struct("ToolExecutionFinished")
                .field("tool_call_id", tool_call_id)
                .field("result_len", &result.len())
                .field("is_error", is_error)
                .finish_non_exhaustive(),
            Self::ProviderRetryStarted {
                attempt,
                max_attempts,
                delay_ms,
                reason,
            } => f
                .debug_struct("ProviderRetryStarted")
                .field("attempt", attempt)
                .field("max_attempts", max_attempts)
                .field("delay_ms", delay_ms)
                .field("reason", reason)
                .finish_non_exhaustive(),
            Self::ProviderRetryFinished { success, attempt } => f
                .debug_struct("ProviderRetryFinished")
                .field("success", success)
                .field("attempt", attempt)
                .finish_non_exhaustive(),
            Self::TurnMetrics {
                prompt_tokens,
                completion_tokens,
            } => f
                .debug_struct("TurnMetrics")
                .field("prompt_tokens", prompt_tokens)
                .field("completion_tokens", completion_tokens)
                .finish(),
            Self::CompactStreamToken(token) => f
                .debug_tuple("CompactStreamToken")
                .field(&token.len())
                .finish(),
            Self::CompactInfo { message } => f.debug_tuple("CompactInfo").field(message).finish(),
            Self::CompactStreamDone { summary, range } => f
                .debug_struct("CompactStreamDone")
                .field("summary_len", &summary.len())
                .field("range", &range)
                .finish_non_exhaustive(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_stream_token_debug_format() {
        let event = LlmEvent::CompactStreamToken("hello world".to_string());
        let debug = format!("{:?}", event);
        assert!(debug.contains("CompactStreamToken"));
        assert!(debug.contains("11")); // length of "hello world"
    }

    #[test]
    fn test_compact_stream_done_debug_format() {
        let event = LlmEvent::CompactStreamDone {
            summary: "Test summary".to_string(),
            range: Some((5, 20)),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("CompactStreamDone"));
        assert!(debug.contains("summary_len"));
        assert!(debug.contains("range"));
    }

    #[test]
    fn test_compact_stream_done_without_range() {
        let event = LlmEvent::CompactStreamDone {
            summary: "Full summary".to_string(),
            range: None,
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("CompactStreamDone"));
        assert!(debug.contains("None")); // range is None
    }

    #[test]
    fn test_compact_stream_token_long_string() {
        let long_token = "x".repeat(500);
        let event = LlmEvent::CompactStreamToken(long_token);
        let debug = format!("{:?}", event);
        // Debug should show length, not the full token
        assert!(debug.contains("500"));
        assert!(debug.len() < 100); // Truncated in debug output
    }

    #[test]
    fn test_compact_info_debug_format() {
        let event = LlmEvent::CompactInfo {
            message: "⚙ Compacting in 3 chunks...".to_string(),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("CompactInfo"));
        assert!(debug.contains("Compacting in 3 chunks"));
    }
}
