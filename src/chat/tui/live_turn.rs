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

use super::components::chat_area::ChatMessage;

/// The lifecycle state of a live turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// The model is emitting thinking tokens.
    Thinking,
    /// The model is emitting content text tokens.
    Streaming,
    /// Tool calls have been finalized and tools are executing.
    ToolCall,
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
    Thinking {
        content: String,
        is_streaming: bool,
    },
    /// Text content (may still be streaming).
    Text {
        content: String,
        is_streaming: bool,
    },
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

    /// Transition to `ToolCall` state.
    pub fn start_tool_call(&mut self) {
        self.state = TurnState::ToolCall;
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

    /// Set the result for the most recent tool-call block matching `tool_call_id`.
    ///
    /// If the tool call has not been frozen yet, freezes the preview first.
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
        let previews: Vec<ToolPreview> = self
            .tool_previews
            .values()
            .cloned()
            .collect();
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
                            ChatMessage::thinking(content)
                                .with_round_index(self.round_index),
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
                    let mut msg = ChatMessage::tool(content)
                        .with_round_index(self.round_index);
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
                TurnBlock::Thinking { content, is_streaming } => {
                    if !content.is_empty() {
                        let mut msg = ChatMessage::thinking(content.clone())
                            .with_round_index(self.round_index);
                        msg.is_streaming = *is_streaming;
                        messages.push(msg);
                    }
                }
                TurnBlock::Text { content, is_streaming } => {
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
                    let mut msg = ChatMessage::tool(content)
                        .with_round_index(self.round_index);
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
            let mut msg = ChatMessage::tool_preview(content)
                .with_round_index(self.round_index);
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
fn format_tool_message(
    tool_call_id: &str,
    name: &str,
    args: &serde_json::Value,
    result: Option<&ToolResult>,
) -> String {
    let mut lines = vec![format!("🔧 {name} (`{tool_call_id}`)")];
    if !args.is_null() && !matches!(args, serde_json::Value::Object(m) if m.is_empty()) {
        let pretty = serde_json::to_string_pretty(args).unwrap_or_else(|_| args.to_string());
        lines.push(format!("```json\n{pretty}\n```"));
    }
    if let Some(r) = result {
        let prefix = if r.is_error { "⛔ Error" } else { "📝 Result" };
        lines.push(format!("{prefix}:\n```\n{}\n```", r.content));
    }
    lines.join("\n\n")
}

/// Format a transient tool-call preview.
fn format_tool_preview(name: &str, args: &serde_json::Value, tool_call_id: &str) -> String {
    let compact = format!("🔧 {name}({args})");
    if compact.len() <= 80 && !matches!(args, serde_json::Value::Object(_)) {
        return format!("{compact} (`{tool_call_id}`)");
    }
    let pretty = serde_json::to_string_pretty(args).unwrap_or_else(|_| args.to_string());
    format!("🔧 {name} (`{tool_call_id}`)\n```json\n{pretty}\n```")
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
        assert_eq!(preview.args, serde_json::json!({"city": "São Paulo", "unit": "C"}));
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
        assert!(messages[2].tool_call_id.as_deref().unwrap_or("").contains("c"));
    }

    #[test]
    fn finalize_drops_empty_blocks() {
        let mut turn = LiveTurn::new(0);
        turn.append_text("");
        turn.finalize_last_block();
        turn.append_thinking("");
        turn.finalize_last_block();
        let messages = turn.finalize();
        assert!(messages.is_empty());
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
        assert!(rendered[2].is_tool_preview);
        assert!(rendered[2].is_streaming);
    }
}
