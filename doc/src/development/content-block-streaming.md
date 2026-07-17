# Content Block Streaming

This document describes the live turn architecture used by the TUI chat REPL to manage streaming content, tool calls, and message ordering.

## Problem

The chat REPL needs to display streaming text, thinking traces, and tool calls in the correct order — including text that appears *before* tool calls, text that appears *after* tool calls, and tool results that arrive asynchronously. Naively appending everything to a single message list leads to ordering bugs: pre-tool text gets overwritten by post-tool text, tool results end up in the wrong position, and re-rendering after tool completion is fragile.

## Solution: Two-Buffer Model

The `App` struct maintains two separate buffers:

```
App
  ├─ messages: Vec<ChatMessage>      (committed history — never modified during streaming)
  └─ live_turn: Option<LiveTurn>     (volatile turn in progress)
```

- **`messages`** holds the permanent conversation history. Once a message is committed here, it is never modified during streaming.
- **`live_turn`** holds everything the model is currently producing for the active turn. When the turn completes, its blocks are committed to `messages` and the live turn is cleared.

This separation makes message ordering deterministic, tool-call preview matching exact, and re-rendering after tool completion trivial.

## LiveTurn

**File:** `src/chat/tui/live_turn.rs`

A `LiveTurn` represents one assistant turn, which may include multiple rounds of text + tool calls:

```
LiveTurn
  ├─ round_index: usize              (0 = pre-tool, 1+ = post-tool)
  ├─ state: TurnState                (Thinking / Streaming / Finalizing / Done)
  ├─ blocks: Vec<TurnBlock>          (ordered content blocks)
  └─ tool_previews: BTreeMap<String, ToolPreview>  (keyed by tool_call_id)
```

### TurnState

| State | Meaning |
|-------|---------|
| `Thinking` | Model is emitting thinking tokens |
| `Streaming` | Model is emitting content text tokens |
| `Finalizing` | Turn is being committed |
| `Done` | Turn committed, slot is empty |

### TurnBlock

Each block is kept in the order the model produced it:

```rust
pub enum TurnBlock {
    Thinking { content: String, is_streaming: bool },
    Text { content: String, is_streaming: bool },
    ToolCall {
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
        result: Option<ToolResult>,
    },
}
```

- `Thinking` and `Text` blocks track `is_streaming` — when `true`, new tokens append to the block; when `false`, the block is finalized.
- `ToolCall` blocks start with `result: None` and are filled in when the tool finishes execution.

### ToolPreview

Tool call results are previewed live via `BTreeMap<String, ToolPreview>` keyed by `tool_call_id`. This allows partial results to be displayed while long-running tools are still executing.

## Event Flow

The event loop in `src/chat/event_loop.rs` receives `LlmEvent` messages via an mpsc channel:

```
LlmEvent::StreamToken(String)        → append token to current Text block
LlmEvent::StreamThinking(String)     → append token to current Thinking block
LlmEvent::ToolCallStarted            → finalize current streaming block, transition to ToolCall state
LlmEvent::ToolCallPreview { ... }    → upsert tool preview by tool_call_id
LlmEvent::StreamDone { content, ... } → finalize all blocks, commit LiveTurn to messages
LlmEvent::Complete { result }        → turn fully complete
LlmEvent::Error { error }           → display error, cancel live turn
```

### Multi-Round Turns

When the model calls tools, the coordinator executes them and sends the results back to the model for a follow-up response. This creates multiple "rounds" within a single turn:

```
Round 0: Text("Let me search...") → ToolCall(weather) → ToolResult("Sunny, 22°C")
Round 1: Text("It's 22°C and sunny.") → StreamDone
```

The `round_index` tracks which round is active. Round 0 blocks (pre-tool text, tool calls, tool results) are finalized and preserved when the model starts a new round of text generation.

## Key Design Decisions

1. **Two buffers, not one** — The previous single-buffer design required fragile heuristics like `insert_before_streaming_zone` and `streaming_zone_start`. The two-buffer model eliminates these.

2. **Blocks, not messages** — Within a live turn, content is organized as ordered blocks, not individual messages. This preserves the visual sequence (text → tool call → tool result → more text) without insertion-order tricks.

3. **Tool previews keyed by `tool_call_id`** — Exact matching, not positional. If a tool call ID changes between rounds, the preview system handles it cleanly.

4. **`is_streaming` flag on blocks** — A block can be finalized (no more tokens) while the turn continues with new blocks. This preserves pre-tool text when tool calls are detected.

## Key Files

| File | Purpose |
|------|---------|
| `src/chat/tui/live_turn.rs` | `LiveTurn`, `TurnBlock`, `ToolResult`, `ToolPreview` |
| `src/chat/app.rs` | `App` struct, `commit_live_turn()`, `cancel_live_turn()`, `render_messages()` |
| `src/chat/llm_event.rs` | `LlmEvent` enum (events sent via mpsc channel) |
| `src/chat/event_loop.rs` | Event loop dispatching `LlmEvent` to `App` methods |
| `src/chat/coordinator.rs` | Coordinator managing streaming + tool execution |