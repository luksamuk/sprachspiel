//! Incremental tool-call accumulator for OpenAI-compatible streaming.
//!
//! W2 #122: the provider emits fine-grained tool-call lifecycle events
//! (`ToolCallStart`, `ToolCallDelta`, `ToolCallEnd`) instead of forcing
//! the consumer to diff successive `LlmStreamChunk` snapshots. This module
//! keeps all partial-state bookkeeping inside the provider and exposes
//! only immutable events.

use std::collections::HashMap;

use super::types::{LlmStreamEvent, LlmToolCall};

/// Partial state for a single in-flight tool call.
#[derive(Debug, Clone, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
    /// Whether we have already emitted `ToolCallStart` for this index.
    started: bool,
}

/// Accumulates OpenAI-style incremental tool-call deltas and produces
/// lifecycle events.
///
/// OpenAI streams tool calls as multiple `delta.tool_calls` entries sharing
/// the same `index`. The first chunk carries `id` and `function.name`;
/// subsequent chunks extend `function.arguments`. This struct tracks that
/// state and emits `LlmStreamEvent` variants so the consumer can render a
/// live preview.
#[derive(Debug, Clone, Default)]
pub struct ToolCallAccumulator {
    by_index: HashMap<u32, PartialToolCall>,
}

impl ToolCallAccumulator {
    /// Create an empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest a single tool-call delta and return any events it produced.
    ///
    /// Events are returned in the order they should be emitted:
    /// 1. `ToolCallStart` when a new index is first seen.
    /// 2. `ToolCallDelta` when name or arguments change.
    pub fn ingest(
        &mut self,
        index: u32,
        id: Option<String>,
        name_delta: Option<String>,
        argument_delta: String,
    ) -> Vec<LlmStreamEvent> {
        let mut events = Vec::new();
        let accumulator = self.by_index.entry(index).or_default();

        let is_new = !accumulator.started;
        let mut name_changed = false;
        let mut arguments_changed = false;

        if let Some(id) = id
            && !id.is_empty()
        {
            accumulator.id = id;
        }

        if let Some(name) = name_delta
            && !name.is_empty()
        {
            accumulator.name = name;
            name_changed = true;
        }

        if !argument_delta.is_empty() {
            accumulator.arguments.push_str(&argument_delta);
            arguments_changed = true;
        }

        if is_new {
            accumulator.started = true;
            events.push(LlmStreamEvent::ToolCallStart {
                index,
                id: if accumulator.id.is_empty() {
                    None
                } else {
                    Some(accumulator.id.clone())
                },
                name: if accumulator.name.is_empty() {
                    None
                } else {
                    Some(accumulator.name.clone())
                },
            });
        }

        // Emit a delta whenever arguments (or name) changed on a chunk
        // after the one that created the tool call. The first chunk is fully
        // represented by ToolCallStart, so we avoid a redundant delta for the
        // same update.
        if !is_new && (name_changed || arguments_changed) {
            events.push(LlmStreamEvent::ToolCallDelta {
                index,
                id: if accumulator.id.is_empty() {
                    None
                } else {
                    Some(accumulator.id.clone())
                },
                name_delta: if name_changed {
                    Some(accumulator.name.clone())
                } else {
                    None
                },
                argument_delta,
            });
        }

        events
    }

    /// Finalize all in-flight tool calls and emit `ToolCallEnd` events.
    ///
    /// Called when the SSE stream ends or when the assistant's turn
    /// otherwise finishes. Only accumulators with a non-empty name become
    /// finalized calls; unnamed ones are dropped as incomplete.
    pub fn finalize_all(&mut self) -> Vec<LlmStreamEvent> {
        let mut events = Vec::new();
        // Sort by index so events are emitted in a stable order.
        let mut indices: Vec<u32> = self.by_index.keys().copied().collect();
        indices.sort_unstable();

        for index in indices {
            if let Some(acc) = self.by_index.remove(&index)
                && !acc.name.is_empty()
            {
                let arguments = serde_json::from_str(&acc.arguments).unwrap_or_else(|_| {
                    if acc.arguments.is_empty() {
                        serde_json::Value::Object(serde_json::Map::new())
                    } else {
                        serde_json::Value::String(acc.arguments.clone())
                    }
                });
                events.push(LlmStreamEvent::ToolCallEnd {
                    index,
                    call: LlmToolCall {
                        id: acc.id,
                        name: acc.name,
                        arguments,
                    },
                });
            }
        }

        events
    }

    /// Return a snapshot of all complete-enough tool calls seen so far.
    ///
    /// Test-only: compatibility view from the pre-event-stream era.
    #[cfg(test)]
    pub fn snapshot(&self) -> Vec<LlmToolCall> {
        let mut result: Vec<(u32, LlmToolCall)> = self
            .by_index
            .iter()
            .filter(|(_, acc)| !acc.name.is_empty())
            .map(|(index, acc)| {
                let arguments = serde_json::from_str(&acc.arguments).unwrap_or_else(|_| {
                    if acc.arguments.is_empty() {
                        serde_json::Value::Object(serde_json::Map::new())
                    } else {
                        serde_json::Value::String(acc.arguments.clone())
                    }
                });
                (
                    *index,
                    LlmToolCall {
                        id: acc.id.clone(),
                        name: acc.name.clone(),
                        arguments,
                    },
                )
            })
            .collect();
        result.sort_by_key(|(index, _)| *index);
        result.into_iter().map(|(_, call)| call).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accumulator_single_call() {
        let mut acc = ToolCallAccumulator::new();

        let events = acc.ingest(0, None, Some("search".to_string()), String::new());
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LlmStreamEvent::ToolCallStart {
                index: 0,
                name: Some(ref n),
                ..
            }
            if n == "search"
        ));

        let events = acc.ingest(0, None, None, "{\"q\":\"x".to_string());
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LlmStreamEvent::ToolCallDelta {
                index: 0,
                argument_delta: ref d,
                ..
            }
            if d == "{\"q\":\"x"
        ));

        let events = acc.ingest(0, None, None, "\"}".to_string());
        assert_eq!(events.len(), 1);

        let events = acc.finalize_all();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LlmStreamEvent::ToolCallEnd {
                index: 0,
                call: LlmToolCall {
                    name: ref n,
                    ..
                }
            }
            if n == "search"
        ));
    }

    #[test]
    fn test_accumulator_multiple_calls() {
        let mut acc = ToolCallAccumulator::new();

        // First call: name arrives.
        let _ = acc.ingest(0, None, Some("search".to_string()), String::new());
        // Second call: id and name arrive together.
        let events = acc.ingest(
            1,
            Some("call_2".to_string()),
            Some("calculator".to_string()),
            String::new(),
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LlmStreamEvent::ToolCallStart {
                index: 1,
                id: Some(ref id),
                name: Some(ref n),
                ..
            }
            if id == "call_2" && n == "calculator"
        ));

        let finalized = acc.finalize_all();
        assert_eq!(finalized.len(), 2);
    }

    #[test]
    fn test_accumulator_drops_incomplete() {
        let mut acc = ToolCallAccumulator::new();
        // Index 0 gets arguments but never a name -> incomplete.
        let events = acc.ingest(0, None, None, "{\"x\":1}".to_string());
        assert_eq!(events.len(), 1); // only ToolCallStart
        let finalized = acc.finalize_all();
        assert!(finalized.is_empty());
    }

    #[test]
    fn test_snapshot_returns_named_calls_only() {
        let mut acc = ToolCallAccumulator::new();
        let _ = acc.ingest(0, None, Some("named".to_string()), "{\"a\":1}".to_string());
        let _ = acc.ingest(1, None, None, "incomplete".to_string());
        let snapshot = acc.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].name, "named");
    }
}
