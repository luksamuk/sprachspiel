//! Live turn state for the TUI chat REPL
//!
//! A "live turn" is the volatile half of the chat buffer: everything the
//! model is currently producing (thinking, text, tool-call previews, tool
//! results) before it is committed to the permanent message history.
//!
//! # Why a separate buffer?
//!
//! The previous design stored streaming and stable messages in a single
//! `Vec<ChatMessage>`. Ordering tool messages, inter-round text, previews,
//! and final responses required fragile heuristics (`insert_before_streaming_zone`,
//! `insert_after_round_0`, `streaming_zone_start`, `finalize_streaming_zone_as_is`,
//! etc.). The two-buffer model separates concerns:
//!
//! - `App::messages` holds committed history (never modified during streaming).
//! - `App::live_turn` holds the in-flight turn, with explicit blocks and
//!   tool-call previews keyed by `tool_call_id`.
//!
//! This makes message ordering deterministic, preview matching exact, and
//! future features (partial tool-output streaming, collapsible previews,
//! per-tool-call expansion) straightforward to add.
//!
//! # Architecture
//!
//! ```text
//! App
//!   ├─ messages: Vec<ChatMessage>      (committed history)
//!   └─ live_turn: Option<LiveTurn>     (volatile turn in progress)
//!
//! LiveTurn
//!   ├─ round_index: usize              (0 = pre-tool, 1+ = post-tool)
//!   ├─ state: TurnState                (Thinking/Streaming/ToolCall/Finalizing/Done)
//!   ├─ blocks: Vec<TurnBlock>            (ordered content blocks)
//!   └─ tool_previews: BTreeMap<String, ToolPreview>   (keyed by tool_call_id)
//! ```

use std::collections::BTreeMap;

use crate::utils::truncate_chars;

use super::components::chat_area::ChatMessage;

/// Maximum characters of a tool result shown in the TUI chat area.
///
/// Tool outputs (e.g., `list_directory` recursive listings, `read_file`
/// of large files) can be tens of thousands of lines. Rendering the full
/// text blocks the main thread, word-wraps an unbounded number of lines, and
/// can corrupt the TUI. The full output is still kept in `ToolResult.content`
/// so it is sent back to the LLM; this limit applies only to on-screen
/// display.
///
/// Currently unused in the TUI display path because tool results are completely
/// suppressed from the chat area (only the compact `🔧 name(args) (id)` line is
/// shown). Kept for milestone 2, which will reintroduce optional result
/// display.
#[allow(dead_code)] // Will be used in milestone 2 for optional tool-result display
const TUI_TOOL_RESULT_MAX_CHARS: usize = 2_000;

/// Maximum number of lines from a tool result shown in the TUI.
///
/// Used together with `TUI_TOOL_RESULT_MAX_CHARS` so that even a short but
/// very tall output does not flood the chat area.
///
/// Currently unused in the TUI display path because tool results are completely
/// suppressed from the chat area. Kept for milestone 2.
#[allow(dead_code)] // Will be used in milestone 2 for optional tool-result display
const TUI_TOOL_RESULT_MAX_LINES: usize = 40;

/// Truncate a tool result for on-screen display.
///
/// Preserves the start of the output and appends a summary of how much
/// was elided. The original `content` in `ToolResult` is untouched.
///
/// Currently unused in the TUI display path because tool results are completely
/// suppressed from the chat area. Kept for milestone 2.
#[allow(dead_code)] // Will be used in milestone 2 for optional tool-result display
fn truncate_tool_result_for_display(content: &str) -> String {
    let line_count = content.lines().count();
    let mut truncated = truncate_chars(content, TUI_TOOL_RESULT_MAX_CHARS);
    if line_count > TUI_TOOL_RESULT_MAX_LINES {
        let keep: Vec<&str> = truncated.lines().take(TUI_TOOL_RESULT_MAX_LINES).collect();
        truncated = keep.join("\n");
    }
    let original_chars = content.chars().count();
    if original_chars > TUI_TOOL_RESULT_MAX_CHARS || line_count > TUI_TOOL_RESULT_MAX_LINES {
        truncated.push_str(&format!(
            "\n\n... [+{} chars, +{} lines hidden — output truncated for display]",
            original_chars.saturating_sub(truncated.chars().count()),
            line_count.saturating_sub(TUI_TOOL_RESULT_MAX_LINES.max(1)),
        ));
    }
    truncated
}

/// The lifecycle state of a live turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// The model is emitting thinking tokens.
    Thinking,
    /// The model is emitting content text tokens.
    Streaming,
    /// The turn is being finalized and committed.
    Finalizing,
    /// The turn has been committed and the slot is empty.
    Done,
}

/// A single block of content inside a live turn.
///
/// Blocks are kept in the order the model produced them. Each block
/// tracks whether it is still being streamed or has been finalized.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnBlock {
    /// Thinking content (may still be streaming).
    Thinking { content: String, is_streaming: bool },
    /// Text content (may still be streaming).
    Text { content: String, is_streaming: bool },
    /// A finalized tool call with its eventual result.
    ///
    /// The result starts as `None` and is filled in when the tool finishes
    /// execution. In the future this will also carry partial output while
    /// long-running tools stream their progress.
    ToolCall {
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
        result: Option<ToolResult>,
    },
}

/// Result of executing a tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    /// Tool output string or error message.
    pub content: String,
    /// Whether the tool returned an error.
    pub is_error: bool,
    /// Whether the result is still being streamed.
    ///
    /// Reserved for future partial-output streaming. Always `false` today.
    pub is_streaming: bool,
}

/// A transient preview of a tool call whose arguments are still streaming in.
///
/// Previews are keyed by `tool_call_id` in `LiveTurn::tool_previews`. When a
/// `ToolCallEnd` event arrives the preview is promoted to a `TurnBlock::ToolCall`
/// and removed from the preview map.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolPreview {
    /// Tool-call id (may be empty if the provider has not assigned one yet).
    pub tool_call_id: String,
    /// Tool name.
    pub name: String,
    /// Best-effort parsed partial arguments.
    pub args: serde_json::Value,
}

/// The volatile state of an in-flight LLM turn.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveTurn {
    /// Which round of a multi-round ReAct cycle this turn belongs to.
    pub round_index: usize,
    /// Current lifecycle state of the turn.
    pub state: TurnState,
    /// Ordered content blocks produced so far.
    pub blocks: Vec<TurnBlock>,
    /// Transient tool-call previews, keyed by tool_call_id.
    pub tool_previews: BTreeMap<String, ToolPreview>,
}

impl LiveTurn {
    /// Create a new live turn at the given round.
    pub fn new(round_index: usize) -> Self {
        Self {
            round_index,
            state: TurnState::Thinking,
            blocks: Vec::new(),
            tool_previews: BTreeMap::new(),
        }
    }

    /// Transition to `Streaming` state.
    pub fn start_streaming(&mut self) {
        self.state = TurnState::Streaming;
    }

    /// Transition to `Finalizing` state.
    pub fn start_finalizing(&mut self) {
        self.state = TurnState::Finalizing;
    }

    /// Append a thinking token to the current thinking block, or create one.
    pub fn append_thinking(&mut self, delta: &str) {
        if let Some(TurnBlock::Thinking {
            content,
            is_streaming: true,
        }) = self.blocks.last_mut()
        {
            content.push_str(delta);
        } else {
            self.blocks.push(TurnBlock::Thinking {
                content: delta.to_string(),
                is_streaming: true,
            });
        }
        self.state = TurnState::Thinking;
    }

    /// Append a text token to the current text block, or create one.
    pub fn append_text(&mut self, delta: &str) {
        if let Some(TurnBlock::Text {
            content,
            is_streaming: true,
        }) = self.blocks.last_mut()
        {
            content.push_str(delta);
        } else {
            self.blocks.push(TurnBlock::Text {
                content: delta.to_string(),
                is_streaming: true,
            });
        }
        self.state = TurnState::Streaming;
    }

    /// Mark the last text or thinking block as finalized.
    ///
    /// Called when a content block ends (e.g. `TextEnd`, `ThinkingEnd`, or
    /// `ToolCallStarted` interrupts the current block).
    pub fn finalize_last_block(&mut self) {
        if let Some(last) = self.blocks.last_mut() {
            match last {
                TurnBlock::Thinking { is_streaming, .. } => *is_streaming = false,
                TurnBlock::Text { is_streaming, .. } => *is_streaming = false,
                TurnBlock::ToolCall { .. } => {}
            }
        }
    }

    /// Update or insert a tool-call preview.
    pub fn upsert_tool_preview(
        &mut self,
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
    ) {
        self.tool_previews.insert(
            tool_call_id.clone(),
            ToolPreview {
                tool_call_id,
                name,
                args,
            },
        );
    }

    /// Promote a preview to a committed `TurnBlock::ToolCall`.
    ///
    /// If no preview exists for the id, creates an empty tool-call block.
    pub fn freeze_tool_preview(&mut self, tool_call_id: &str) {
        let preview = self.tool_previews.remove(tool_call_id);
        let block = match preview {
            Some(p) => TurnBlock::ToolCall {
                tool_call_id: p.tool_call_id,
                name: p.name,
                args: p.args,
                result: None,
            },
            None => TurnBlock::ToolCall {
                tool_call_id: tool_call_id.to_string(),
                name: String::new(),
                args: serde_json::Value::Object(serde_json::Map::new()),
                result: None,
            },
        };
        self.blocks.push(block);
    }

    /// Promote a preview to a committed `TurnBlock::ToolCall`, matching by
    /// tool name when the exact `tool_call_id` doesn't match any preview.
    ///
    /// This handles the common case where the streaming preview used the
    /// provider's call id (e.g., `"call_abc123"`) but the ReAct loop
    /// synthesizes `tool_call_id` from the tool name (e.g.,
    /// `"list_directory"`). Without name-based matching, `freeze_tool_preview`
    /// would create a new block with an empty name, producing
    /// `🔧 () (list_directory)` in the chat area.
    ///
    /// When a name match is found, the preview is updated with the correct
    /// `tool_call_id` (so future `set_tool_result` calls can find it) and
    /// the name/args from `ToolExecutionStarted` are used to fill in any
    /// gaps (e.g., empty name from a provider that didn't stream the name).
    ///
    /// If no match is found at all, creates a block with the provided
    /// `name` and `args` (not empty) so the user always sees the tool name.
    pub fn freeze_tool_preview_by_name(
        &mut self,
        tool_call_id: &str,
        name: &str,
        args: &serde_json::Value,
    ) {
        // Bug E fix: if a block with this tool_call_id already exists (e.g.,
        // it was frozen earlier by freeze_all_tool_previews), do nothing —
        // don't create a duplicate block.
        if self.blocks.iter().any(|b| {
            matches!(
                b,
                TurnBlock::ToolCall {
                    tool_call_id: id, ..
                } if id == tool_call_id
            )
        }) {
            return;
        }

        // First try exact id match. If the preview has empty args, fill them
        // from the ToolExecutionStarted args (provider didn't stream them).
        if let Some(mut preview) = self.tool_previews.remove(tool_call_id) {
            if preview.name.is_empty() && !name.is_empty() {
                preview.name = name.to_string();
            }
            let is_preview_args_empty = matches!(
                &preview.args,
                serde_json::Value::Object(o) if o.is_empty()
            );
            let final_args = if is_preview_args_empty && !args.is_null() {
                args.clone()
            } else {
                preview.args
            };
            self.blocks.push(TurnBlock::ToolCall {
                tool_call_id: preview.tool_call_id,
                name: preview.name,
                args: final_args,
                result: None,
            });
            return;
        }

        // Try name match: find a preview whose name matches, or a preview
        // with an empty name (placeholder from a provider that didn't stream
        // the name — it should be claimed by the first ToolExecutionStarted).
        if !name.is_empty() {
            if let Some(preview_id) = self.tool_previews.iter().find_map(|(id, p)| {
                if p.name == name || p.name.is_empty() {
                    Some(id.clone())
                } else {
                    None
                }
            }) {
                if let Some(mut preview) = self.tool_previews.remove(&preview_id) {
                    // Update the preview's tool_call_id to the execution id
                    preview.tool_call_id = tool_call_id.to_string();
                    // Fill in empty name (provider didn't stream it)
                    if preview.name.is_empty() {
                        preview.name = name.to_string();
                    }
                    // If the preview has empty args (provider didn't stream
                    // argument_delta — common with Ollama/cloud providers that
                    // only send args in the final ToolCallEnd), use the args
                    // from ToolExecutionStarted which carries the parsed args.
                    let is_preview_args_empty = matches!(
                        &preview.args,
                        serde_json::Value::Object(o) if o.is_empty()
                    );
                    let final_args = if is_preview_args_empty && !args.is_null() {
                        args.clone()
                    } else {
                        preview.args
                    };
                    self.blocks.push(TurnBlock::ToolCall {
                        tool_call_id: preview.tool_call_id,
                        name: preview.name,
                        args: final_args,
                        result: None,
                    });
                    return;
                }
            }
        }

        // No match — create block with the provided name/args (not empty)
        self.blocks.push(TurnBlock::ToolCall {
            tool_call_id: tool_call_id.to_string(),
            name: name.to_string(),
            args: args.clone(),
            result: None,
        });
    }

    /// Set the result for the most recent tool-call block matching `tool_call_id`.
    ///
    /// If the tool call has not been frozen yet, freezes the preview first.
    ///
    /// Bug F fix: only fills a block that does NOT already have a result.
    /// Previously this would overwrite an earlier round's result if the same
    /// `tool_call_id` appeared in multiple rounds. With Bug D fix (unique
    /// ids), collisions are rare, but this is defense in depth.
    pub fn set_tool_result(
        &mut self,
        tool_call_id: &str,
        content: String,
        is_error: bool,
        is_streaming: bool,
    ) {
        // If the preview is still in the preview map, freeze it first.
        if self.tool_previews.contains_key(tool_call_id) {
            self.freeze_tool_preview(tool_call_id);
        }

        // Bug F fix: prefer a block that has NO result yet. Only fall back
        // to overwriting an existing result if no empty block is found.
        for block in self.blocks.iter_mut().rev() {
            if let TurnBlock::ToolCall {
                tool_call_id: id,
                result,
                ..
            } = block
                && id == tool_call_id
                && result.is_none()
            {
                *result = Some(ToolResult {
                    content,
                    is_error,
                    is_streaming,
                });
                return;
            }
        }

        // No empty block found — fall back to the last block with this id
        // (even if it already has a result). This handles the edge case
        // where a provider sends duplicate ToolExecutionFinished events.
        for block in self.blocks.iter_mut().rev() {
            if let TurnBlock::ToolCall {
                tool_call_id: id,
                result,
                ..
            } = block
                && id == tool_call_id
            {
                *result = Some(ToolResult {
                    content,
                    is_error,
                    is_streaming,
                });
                return;
            }
        }

        // No matching block exists — create one on the fly.
        log::warn!(
            "set_tool_result called for unknown tool_call_id {}",
            tool_call_id
        );
        self.blocks.push(TurnBlock::ToolCall {
            tool_call_id: tool_call_id.to_string(),
            name: String::new(),
            args: serde_json::Value::Object(serde_json::Map::new()),
            result: Some(ToolResult {
                content,
                is_error,
                is_streaming,
            }),
        });
    }

    /// Promote all remaining previews into committed tool-call blocks.
    ///
    /// Used when the provider signals that tool-call collection is complete
    /// but we do not yet have results.
    pub fn freeze_all_tool_previews(&mut self) {
        let previews: Vec<ToolPreview> = self.tool_previews.values().cloned().collect();
        self.tool_previews.clear();
        for p in previews {
            self.blocks.push(TurnBlock::ToolCall {
                tool_call_id: p.tool_call_id,
                name: p.name,
                args: p.args,
                result: None,
            });
        }
    }

    /// Convert this live turn into committed `ChatMessage`s.
    ///
    /// After calling this, the turn is consumed and should be replaced by
    /// `TurnState::Done` or dropped.
    pub fn finalize(mut self) -> Vec<ChatMessage> {
        self.state = TurnState::Finalizing;
        let mut messages = Vec::with_capacity(self.blocks.len());
        for block in self.blocks {
            match block {
                TurnBlock::Thinking { content, .. } => {
                    if !content.is_empty() {
                        messages.push(
                            ChatMessage::thinking(content).with_round_index(self.round_index),
                        );
                    }
                }
                TurnBlock::Text { content, .. } => {
                    if !content.is_empty() {
                        messages.push(
                            ChatMessage::assistant_markdown(content)
                                .with_round_index(self.round_index),
                        );
                    }
                }
                TurnBlock::ToolCall {
                    tool_call_id,
                    name,
                    args,
                    result,
                } => {
                    let content = format_tool_message(&tool_call_id, &name, &args, result.as_ref());
                    let mut msg = ChatMessage::tool(content).with_round_index(self.round_index);
                    msg.tool_call_id = Some(tool_call_id);
                    messages.push(msg);
                }
            }
        }
        self.state = TurnState::Done;
        messages
    }

    /// Render the live turn as temporary `ChatMessage`s for display.
    ///
    /// Unlike `finalize`, this does not consume the turn. It is called every
    /// frame to produce the volatile portion of the chat area.
    pub fn render_blocks(&self) -> Vec<ChatMessage> {
        let mut messages = Vec::with_capacity(self.blocks.len() + self.tool_previews.len());

        for block in &self.blocks {
            match block {
                TurnBlock::Thinking {
                    content,
                    is_streaming,
                } => {
                    if !content.is_empty() {
                        let mut msg = ChatMessage::thinking(content.clone())
                            .with_round_index(self.round_index);
                        msg.is_streaming = *is_streaming;
                        messages.push(msg);
                    }
                }
                TurnBlock::Text {
                    content,
                    is_streaming,
                } => {
                    let mut msg = ChatMessage::assistant_streaming(content.clone())
                        .with_round_index(self.round_index);
                    msg.is_streaming = *is_streaming;
                    messages.push(msg);
                }
                TurnBlock::ToolCall {
                    tool_call_id,
                    name,
                    args,
                    result,
                } => {
                    let content = format_tool_message(tool_call_id, name, args, result.as_ref());
                    let mut msg = ChatMessage::tool(content).with_round_index(self.round_index);
                    msg.tool_call_id = Some(tool_call_id.clone());
                    msg.is_streaming = result.as_ref().is_some_and(|r| r.is_streaming);
                    messages.push(msg);
                }
            }
        }

        // Render active previews after committed blocks so the user sees the
        // tool call being built in real time.
        for preview in self.tool_previews.values() {
            let content = format_tool_preview(&preview.name, &preview.args, &preview.tool_call_id);
            let mut msg = ChatMessage::tool(content).with_round_index(self.round_index);
            msg.tool_call_id = Some(preview.tool_call_id.clone());
            msg.is_streaming = true;
            messages.push(msg);
        }

        messages
    }

    /// Return true if the turn has no content.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty() && self.tool_previews.is_empty()
    }
}

/// Format a finalized tool-call message.
///
/// Renders as a single compact line showing the tool name and its arguments.
/// Tool results are intentionally **not** displayed in the chat area — they are
/// kept in `ToolResult.content` and sent back to the LLM, but rendering them
/// causes TUI overflow/corruption and adds visual noise. Milestone 2 will
/// reintroduce optional result display.
fn format_tool_message(
    tool_call_id: &str,
    name: &str,
    args: &serde_json::Value,
    _result: Option<&ToolResult>,
) -> String {
    format_tool_line(name, args, tool_call_id)
}

/// Format a transient tool-call preview.
///
/// Uses the same compact single-line format as `format_tool_message` so the
/// preview updates in-place when the call is finalized, without changing size
/// or jumping in the chat buffer.
fn format_tool_preview(name: &str, args: &serde_json::Value, tool_call_id: &str) -> String {
    format_tool_line(name, args, tool_call_id)
}

/// Build the compact single-line representation of a tool call.
///
/// Normal mode: `🔧 name(k1=v1, k2=v2)`
/// Debug mode:  `🔧 name(k1=v1, k2=v2) (`id`)`
///
/// Empty-string and null argument values are omitted to keep the line short.
/// Very long combined lines are truncated to protect the chat area width.
fn format_tool_line(name: &str, args: &serde_json::Value, tool_call_id: &str) -> String {
    let compact_args = compact_args_for_display(args);
    let compact = format!("🔧 {name}({compact_args})");
    const MAX_LINE_WIDTH: usize = 120;

    // Show the tool-call id only in debug/trace mode — it's diagnostic info,
    // not useful for the everyday user.
    let show_id = log::max_level() == log::LevelFilter::Trace && !tool_call_id.is_empty();

    if show_id {
        let suffix = format!(" (`{tool_call_id}`)");
        let budget = MAX_LINE_WIDTH.saturating_sub(suffix.chars().count());
        let display_compact = crate::utils::truncate_chars(
            &compact,
            budget.max("🔧 ".chars().count() + name.chars().count() + "()".chars().count()),
        );
        format!("{display_compact}{suffix}")
    } else {
        crate::utils::truncate_chars(
            &compact,
            MAX_LINE_WIDTH.max("🔧 ".chars().count() + name.chars().count() + "()".chars().count()),
        )
    }
}

/// Argument keys that are "identifiers" — always shown first in the compact
/// display, regardless of the order the provider sent them. These are short,
/// high-value fields like `path`, `query`, `task_id` that the user needs to
/// see. Long-content fields like `content` are deferred to the end.
const PRIORITY_ARG_KEYS: &[&str] = &[
    "path",
    "file_path",
    "filename",
    "command_line",
    "command",
    "query",
    "search_term",
    "task_id",
    "id",
    "url",
    "model",
    "name",
    "language",
    "mode",
    "status",
    "action",
    "overwrite",
    "recursive",
    "recursive",
];

/// Maximum characters to display for a single argument value. Long values
/// (e.g., file `content`) are truncated to this length so they don't
/// monopolize the display line and hide shorter, more important args.
const MAX_ARG_VALUE_DISPLAY: usize = 30;

/// Build a compact, comma-separated `key=value` string from tool arguments.
///
/// Priority args (path, query, id, etc.) are shown first, regardless of JSON
/// order. Each value is truncated to `MAX_ARG_VALUE_DISPLAY` chars so a long
/// `content` field doesn't hide the `path` field. This is display-only; the
/// LLM still receives the full structured arguments.
fn compact_args_for_display(args: &serde_json::Value) -> String {
    let obj = match args {
        serde_json::Value::Object(o) if !o.is_empty() => o,
        _ => return String::new(),
    };

    let format_pair = |k: &str, v: &serde_json::Value| -> Option<String> {
        let value_str = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => return None,
            other => other.to_string(),
        };
        if value_str.is_empty() {
            return None;
        }
        let truncated = crate::utils::truncate_chars(&value_str, MAX_ARG_VALUE_DISPLAY);
        Some(format!("{k}={truncated}"))
    };

    // Partition into priority (identifiers) and others (content, body, etc.)
    let (priority, others): (Vec<_>, Vec<_>) = obj
        .iter()
        .partition(|(k, _)| PRIORITY_ARG_KEYS.contains(&k.as_str()));

    let mut pairs: Vec<String> = priority
        .iter()
        .filter_map(|(k, v)| format_pair(k, v))
        .collect();
    pairs.extend(others.iter().filter_map(|(k, v)| format_pair(k, v)));

    pairs.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::tui::components::chat_area::MessageType;

    #[test]
    fn new_turn_starts_in_thinking_state() {
        let turn = LiveTurn::new(0);
        assert_eq!(turn.state, TurnState::Thinking);
        assert!(turn.is_empty());
    }

    #[test]
    fn append_thinking_creates_and_extends_block() {
        let mut turn = LiveTurn::new(0);
        turn.append_thinking("I think");
        turn.append_thinking(", therefore");
        assert_eq!(turn.blocks.len(), 1);
        assert_eq!(
            turn.blocks[0],
            TurnBlock::Thinking {
                content: "I think, therefore".to_string(),
                is_streaming: true,
            }
        );
        assert_eq!(turn.state, TurnState::Thinking);
    }

    #[test]
    fn append_text_creates_and_extends_block() {
        let mut turn = LiveTurn::new(0);
        turn.append_text("Hello");
        turn.append_text(" world");
        assert_eq!(turn.blocks.len(), 1);
        assert_eq!(
            turn.blocks[0],
            TurnBlock::Text {
                content: "Hello world".to_string(),
                is_streaming: true,
            }
        );
        assert_eq!(turn.state, TurnState::Streaming);
    }

    #[test]
    fn finalize_last_block_marks_streaming_false() {
        let mut turn = LiveTurn::new(0);
        turn.append_text("Hello");
        turn.finalize_last_block();
        assert_eq!(
            turn.blocks[0],
            TurnBlock::Text {
                content: "Hello".to_string(),
                is_streaming: false,
            }
        );
    }

    #[test]
    fn interleaved_thinking_and_text_create_separate_blocks() {
        let mut turn = LiveTurn::new(0);
        turn.append_thinking("Hmm");
        turn.append_text("Answer");
        turn.append_thinking("Wait");
        assert_eq!(turn.blocks.len(), 3);
        assert!(matches!(turn.blocks[0], TurnBlock::Thinking { .. }));
        assert!(matches!(turn.blocks[1], TurnBlock::Text { .. }));
        assert!(matches!(turn.blocks[2], TurnBlock::Thinking { .. }));
    }

    #[test]
    fn upsert_tool_preview_updates_existing() {
        let mut turn = LiveTurn::new(0);
        turn.upsert_tool_preview(
            "call_1".to_string(),
            "weather".to_string(),
            serde_json::json!({"city": "São Paulo"}),
        );
        turn.upsert_tool_preview(
            "call_1".to_string(),
            "weather".to_string(),
            serde_json::json!({"city": "São Paulo", "unit": "C"}),
        );
        assert_eq!(turn.tool_previews.len(), 1);
        let preview = turn.tool_previews.get("call_1").unwrap();
        assert_eq!(
            preview.args,
            serde_json::json!({"city": "São Paulo", "unit": "C"})
        );
    }

    #[test]
    fn freeze_tool_preview_promotes_to_block() {
        let mut turn = LiveTurn::new(0);
        turn.upsert_tool_preview(
            "call_1".to_string(),
            "weather".to_string(),
            serde_json::json!({"city": "São Paulo"}),
        );
        turn.freeze_tool_preview("call_1");
        assert!(turn.tool_previews.is_empty());
        assert_eq!(turn.blocks.len(), 1);
        assert!(matches!(turn.blocks[0], TurnBlock::ToolCall { .. }));
    }

    #[test]
    fn freeze_tool_preview_by_name_matches_on_name() {
        // Simulate the common case: streaming preview used the provider's
        // call id ("call_abc123"), but ToolExecutionStarted synthesizes
        // tool_call_id from the tool name ("list_directory").
        let mut turn = LiveTurn::new(0);
        turn.upsert_tool_preview(
            "call_abc123".to_string(),
            "list_directory".to_string(),
            serde_json::json!({"path": "."}),
        );

        // Freeze with the execution id — should match by name
        turn.freeze_tool_preview_by_name(
            "list_directory",
            "list_directory",
            &serde_json::json!({"path": "."}),
        );

        assert!(turn.tool_previews.is_empty());
        assert_eq!(turn.blocks.len(), 1);
        match &turn.blocks[0] {
            TurnBlock::ToolCall {
                tool_call_id,
                name,
                args,
                ..
            } => {
                assert_eq!(tool_call_id, "list_directory");
                assert_eq!(name, "list_directory");
                assert_eq!(args, &serde_json::json!({"path": "."}));
            }
            _ => panic!("expected tool call block"),
        }
    }

    #[test]
    fn freeze_tool_preview_by_name_fills_empty_name() {
        // Provider streamed id but no name — name arrives in
        // ToolExecutionStarted. The block should get the correct name.
        let mut turn = LiveTurn::new(0);
        turn.upsert_tool_preview("call_xyz".to_string(), String::new(), serde_json::json!({}));

        turn.freeze_tool_preview_by_name(
            "list_directory",
            "list_directory",
            &serde_json::json!({"path": "/tmp"}),
        );

        assert!(turn.tool_previews.is_empty());
        assert_eq!(turn.blocks.len(), 1);
        match &turn.blocks[0] {
            TurnBlock::ToolCall {
                tool_call_id,
                name,
                args,
                ..
            } => {
                assert_eq!(tool_call_id, "list_directory");
                assert_eq!(name, "list_directory");
                // Args: preview had empty args, so the args from
                // ToolExecutionStarted are used as fallback.
                assert_eq!(args, &serde_json::json!({"path": "/tmp"}));
            }
            _ => panic!("expected tool call block"),
        }
    }

    #[test]
    fn freeze_tool_preview_by_name_no_match_uses_provided_name() {
        // No preview at all — block should still have the correct name
        // (not empty), preventing `🔧 () (id)` in the chat area.
        let mut turn = LiveTurn::new(0);
        turn.freeze_tool_preview_by_name(
            "list_directory",
            "list_directory",
            &serde_json::json!({"path": "."}),
        );

        assert_eq!(turn.blocks.len(), 1);
        match &turn.blocks[0] {
            TurnBlock::ToolCall {
                tool_call_id,
                name,
                args,
                ..
            } => {
                assert_eq!(tool_call_id, "list_directory");
                assert_eq!(name, "list_directory");
                assert_eq!(args, &serde_json::json!({"path": "."}));
            }
            _ => panic!("expected tool call block"),
        }
    }

    #[test]
    fn freeze_all_tool_previews_promotes_everything() {
        let mut turn = LiveTurn::new(0);
        turn.upsert_tool_preview("a".to_string(), "search".to_string(), serde_json::json!({}));
        turn.upsert_tool_preview("b".to_string(), "calc".to_string(), serde_json::json!({}));
        turn.freeze_all_tool_previews();
        assert!(turn.tool_previews.is_empty());
        assert_eq!(turn.blocks.len(), 2);
    }

    #[test]
    fn set_tool_result_finds_matching_block() {
        let mut turn = LiveTurn::new(1);
        turn.freeze_tool_preview("call_1");
        turn.set_tool_result("call_1", "sunny".to_string(), false, false);
        assert_eq!(turn.blocks.len(), 1);
        let result = match &turn.blocks[0] {
            TurnBlock::ToolCall { result, .. } => result.as_ref().unwrap(),
            _ => panic!("expected tool call block"),
        };
        assert_eq!(result.content, "sunny");
        assert!(!result.is_error);
    }

    #[test]
    fn set_tool_result_creates_block_if_missing() {
        let mut turn = LiveTurn::new(0);
        turn.set_tool_result("missing", "result".to_string(), true, false);
        assert_eq!(turn.blocks.len(), 1);
        let result = match &turn.blocks[0] {
            TurnBlock::ToolCall { result, .. } => result.as_ref().unwrap(),
            _ => panic!("expected tool call block"),
        };
        assert!(result.is_error);
    }

    #[test]
    fn finalize_converts_blocks_to_chat_messages() {
        let mut turn = LiveTurn::new(0);
        turn.append_thinking("I think");
        turn.finalize_last_block();
        turn.append_text("Hello");
        turn.finalize_last_block();
        turn.upsert_tool_preview("c".to_string(), "calc".to_string(), serde_json::json!({}));
        turn.freeze_tool_preview("c");
        turn.set_tool_result("c", "42".to_string(), false, false);

        let messages = turn.finalize();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].msg_type, MessageType::Thinking);
        assert_eq!(messages[1].msg_type, MessageType::Assistant);
        assert_eq!(messages[2].msg_type, MessageType::Tool);
        assert_eq!(messages[2].round_index, 0);
        assert!(
            messages[2]
                .tool_call_id
                .as_deref()
                .unwrap_or("")
                .contains("c")
        );
    }

    #[test]
    fn format_tool_message_truncates_large_result() {
        let long_result = "line\n".repeat(100);
        let block = TurnBlock::ToolCall {
            tool_call_id: "id".to_string(),
            name: "list_directory".to_string(),
            args: serde_json::json!({}),
            result: Some(ToolResult {
                content: long_result.clone(),
                is_error: false,
                is_streaming: false,
            }),
        };
        let rendered = format_tool_message(
            "id",
            "list_directory",
            &serde_json::json!({}),
            Some(&ToolResult {
                content: long_result,
                is_error: false,
                is_streaming: false,
            }),
        );
        assert!(rendered.contains("list_directory"));
        assert!(
            !rendered.contains("output truncated for display"),
            "Tool results should not be rendered in the TUI chat area"
        );
        // In normal (non-trace) mode, the tool_call_id is NOT shown.
        assert_eq!(
            rendered, "🔧 list_directory()",
            "Compact tool line should show name and args; id hidden in normal mode"
        );
        // Result stored in the block is still the full content
        if let TurnBlock::ToolCall {
            result: Some(r), ..
        } = block
        {
            assert_eq!(r.content.lines().count(), 100);
        } else {
            panic!("expected tool call block with result");
        }
    }

    #[test]
    fn render_blocks_includes_streaming_flags() {
        let mut turn = LiveTurn::new(0);
        turn.append_thinking("I think");
        turn.append_text("Hello");
        turn.upsert_tool_preview("p".to_string(), "search".to_string(), serde_json::json!({}));

        let rendered = turn.render_blocks();
        assert_eq!(rendered.len(), 3);
        assert_eq!(rendered[0].msg_type, MessageType::Thinking);
        assert!(rendered[0].is_streaming);
        assert_eq!(rendered[1].msg_type, MessageType::AssistantStreaming);
        assert!(rendered[1].is_streaming);
        assert_eq!(rendered[2].msg_type, MessageType::Tool);
        assert!(rendered[2].is_streaming);
        assert_eq!(rendered[2].tool_call_id.as_deref(), Some("p"));
    }
}
