# OpenAI-Compatible API Streaming Tool Calls

**Status:** Research finding from #120 development
**Date:** 2026-06-08
**Author:** Investigation via Ollama native API + llama-swap

## Context

While implementing the `OllamaProvider` (issue #120), we discovered a critical
difference between how **Ollama's native API** and **OpenAI-compatible APIs**
(llama.cpp, llama-swap, vLLM, OpenAI itself) stream **tool calls** in chat
completions.

This affects the design of the `OpenAICompatibleProvider` stub in #122.

## Test Setup

**Tested with the same prompt and tool definition:**

```json
{
  "model": "...",
  "messages": [{"role": "user", "content": "What is the weather in London?"}],
  "tools": [{
    "type": "function",
    "function": {
      "name": "get_weather",
      "description": "Get weather for a location",
      "parameters": {
        "type": "object",
        "properties": {"location": {"type": "string"}},
        "required": ["location"]
      }
    }
  }],
  "stream": true
}
```

## Ollama Native API (`/api/chat`)

**Endpoint:** `POST http://localhost:11434/api/chat`
**Model:** `qwen3.5:4b`

Streaming response (NDJSON, one JSON object per line):

```json
// Thinking tokens stream first
{"message":{"role":"assistant","content":"","thinking":"The user is asking..."}}
{"message":{"role":"assistant","content":"","thinking":" I have access..."}}

// Tool call arrives COMPLETE in ONE chunk
{"message":{"role":"assistant","content":"","tool_calls":[{
  "function":{"index":0,"name":"get_weather","arguments":{"location":"London"}}
}]},"done":false}

// Final
{"message":{"role":"assistant","content":""},"done":true,"done_reason":"stop"}
```

**Key observations:**

- `tool_calls` arrives as a **complete object** in one chunk
- `arguments` is a **JSON object** (not a string)
- `id` field is **NOT present** in the native API
- Each tool call has a `function.index` for ordering

## OpenAI-Compatible API (`/v1/chat/completions`)

**Endpoint:** `POST http://localhost:12434/v1/chat/completions` (llama-swap)
**Model:** `gemma4-e4b`

Streaming response (SSE, `data: {json}\n\n`):

```
data: {"choices":[{"delta":{"role":"assistant","content":null}}]}

data: {"choices":[{"delta":{"tool_calls":[{
  "index":0,
  "id":"BvIDM0pcFS1GNOtPjvBj6HFE8BoBIPmI",
  "type":"function",
  "function":{"name":"get_weather","arguments":"{"}
}]}}]}

data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"location"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\":"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\""}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"London"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\""}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"}"}}]}}]}

data: {"choices":[{"finish_reason":"tool_calls","delta":{}}]}

data: [DONE]
```

**Key observations:**

- `tool_calls` arguments arrive **incrementally** over multiple chunks
- `arguments` is a **string** that must be concatenated
- `id` field **IS present** (used for `tool_call_id` correlation in `tool` messages)
- Each tool call has a top-level `index` (used to merge partial chunks)
- `finish_reason="tool_calls"` signals the end of tool call generation

## Comparison Table

| Aspect | Ollama Native | OpenAI-Compatible |
|--------|---------------|-------------------|
| Protocol | NDJSON (newline-delimited) | SSE (Server-Sent Events) |
| Endpoint | `/api/chat` | `/v1/chat/completions` |
| Tool call delivery | Complete in 1 chunk | Incremental over N chunks |
| `arguments` type | JSON object | String (must be JSON-parsed) |
| `id` field | **Absent** | **Present** (e.g. `call_xyz123`) |
| Ordering field | `function.index` | Top-level `index` |
| End signal | `done:true` chunk | `finish_reason:"tool_calls"` |
| Thinking field | `message.thinking` | `delta.reasoning_content` (varies) |

## Implications for `OpenAICompatibleProvider` (#122)

The streaming parser for the OpenAI-compatible provider **must** accumulate
partial tool calls per `index` before emitting a complete `LlmStreamChunk` to
the consumer.

### Required parser behavior

1. **Buffer state per `index`**: maintain a `HashMap<u32, PartialToolCall>` keyed by
   `index` (0, 1, 2, ... for parallel tool calls).
2. **Accumulate `arguments` strings**: concatenate all incoming `arguments` fragments
   until `finish_reason="tool_calls"`.
3. **Parse accumulated JSON**: once complete, `serde_json::from_str()` the joined
   `arguments` string to produce the final `Map<String, Value>`.
4. **Preserve `id`**: store the `id` from the first chunk and emit it with the final
   tool call (this becomes `tool_call_id` in subsequent `tool` role messages).
5. **Stream content normally**: `delta.content` is still token-by-token; only
   `tool_calls` need the accumulation strategy.

### Mapping to `LlmStreamChunk`

```rust
// During streaming — emit only content
yield Ok(LlmStreamChunk {
    content: Some(delta.content),  // if present
    thinking: delta.reasoning_content,  // if present
    tool_calls: None,  // HOLD: not yet complete
    done: false,
    done_reason: None,
});

// On finish_reason="tool_calls" — emit accumulated tool calls
yield Ok(LlmStreamChunk {
    content: None,
    thinking: None,
    tool_calls: Some(accumulated_tool_calls),  // NOW we emit
    done: true,
    done_reason: Some("tool_calls".to_string()),
    eval_count: ...,
    prompt_eval_count: ...,
});
```

### Mapping to `LlmToolCall`

```rust
LlmToolCall {
    id: accumulated.id.unwrap_or_else(|| generate_id()),
    name: accumulated.function.name,
    arguments: serde_json::from_str(&accumulated.function.arguments)?,
}
```

## Architectural Recommendation

Given that the OpenAI-compatible API is the **de facto standard** for LLM
serving (llama.cpp, vLLM, llama-swap, LM Studio, OpenAI, Together, Groq, etc.),
**#122 should target 100% OpenAI compatibility first**, with Ollama as a
secondary target that may require its own native implementation (e.g. for
fields unique to Ollama like `thinking` in the message body rather than
`reasoning_content` in the delta).

### Phased approach

1. **Phase 1 (this PR cycle):** Build `OpenAICompatibleProvider` that:
   - Speaks the OpenAI API
   - Streams tool calls incrementally
   - Is tested ONLY against Ollama's OpenAI-compat endpoint
     (`http://localhost:11434/v1/chat/completions`)
2. **Phase 2 (next demand):** Test against llama-swap, vLLM, llama.cpp directly
3. **Phase 3 (future):** Test against OpenAI, Together, Groq, etc.

### Ollama's OpenAI-compat endpoint

Ollama also exposes `/v1/chat/completions` for OpenAI compatibility. This means
**we can develop and test the OpenAI-compatible provider against Ollama's
OpenAI endpoint** before broadening to other backends. This keeps the
test surface small while building the more universal implementation.

## Open Questions

- How do different backends handle `reasoning_content`?
  - OpenAI: uses `reasoning_content` field (in some models)
  - llama.cpp: not standardized
  - vLLM: varies by model
- How to handle `parallel_tool_calls`? Ollama native doesn't expose this.
- Should the Ollama native provider be deprecated in favor of OpenAI-compat for
  everything? (Probably yes, but keep it for unique features like `thinking`.)
