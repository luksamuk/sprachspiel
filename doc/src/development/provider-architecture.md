# Provider Architecture

This document describes the provider-agnostic abstraction layer that allows Sprachspiel to communicate with any OpenAI-compatible LLM backend.

## Overview

Sprachspiel uses a single `LlmProvider` trait to abstract all LLM communication. The default and only implementation is `OpenAICompatibleProvider`, which works with any backend that exposes OpenAI-compatible API endpoints:

- **Ollama** (local, `http://localhost:11434/v1`)
- **llama.cpp** (local, `http://localhost:8080/v1`)
- **LM Studio** (local)
- **llama-swap** (local proxy)
- **vLLM** (local/cloud)
- **OpenAI API** (cloud)
- **Any OpenAI-compatible endpoint**

## Design Philosophy

The provider layer was designed to be **provider-agnostic by default**. Business code (coordinator, tools, embeddings, chat) depends on the `LlmProvider` trait, never on a concrete provider implementation. This means:

- No provider-specific types leak into business logic
- No hardcoded provider names in the codebase
- Model names are opaque strings — their meaning depends on the configured backend
- Adding a new provider requires implementing only the `LlmProvider` trait

## The `LlmProvider` Trait

**File:** `src/provider/mod.rs`

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request with optional tools.
    async fn chat(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolInfo>,
        options: ProviderOptions,
    ) -> Result<LlmResponse, ProviderError>;

    /// Streaming chat completion — returns a stream of semantic events.
    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolInfo>,
        options: ProviderOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmStreamEvent, ProviderError>> + Send>>, ProviderError>;

    /// Generate a completion (non-chat, e.g., for vision/OCR).
    async fn generate(
        &self,
        model: &str,
        prompt: &str,
        images: Vec<String>,
        audio: Vec<String>,
        options: ProviderOptions,
    ) -> Result<String, ProviderError>;

    /// Generate embeddings for text.
    async fn embed(
        &self,
        text: &str,
        model: &str,
        dimensions: Option<usize>,
    ) -> Result<Vec<f32>, ProviderError>;

    /// Detect model capabilities (tools, vision, thinking, etc.).
    async fn detect_capabilities(&self, model: &str)
    -> Result<ProviderCapabilities, ProviderError>;
}
```

## Agnostic Types

All provider communication uses provider-agnostic types defined in `src/provider/types.rs`:

| Type | Purpose |
|------|---------|
| `LlmMessage` | Chat message (role + content) |
| `LlmResponse` | Non-streaming response |
| `LlmStreamEvent` | Streaming event (text/thinking/tool-call deltas) |
| `LlmToolCall` | Tool call from LLM |
| `LlmRole` | Message role (user/assistant/system/tool) |
| `ProviderOptions` | Model parameters (temperature, top_p, seed, etc.) |
| `ProviderCapabilities` | Detected capabilities (tools, vision, thinking) |
| `ProviderError` | Typed errors (retryable, unsupported, etc.) |
| `RetryCategory` | Error classification for retry logic |
| `ToolInfo` | Tool definition sent to LLM |
| `ToolType` | Tool type (function) |
| `LocalModel` | Local model metadata |

## `OpenAICompatibleProvider`

**File:** `src/provider/openai_compat.rs`

The default and only provider implementation. Uses `reqwest` to communicate with any OpenAI-compatible `/v1/chat/completions`, `/v1/completions`, and `/v1/embeddings` endpoints.

Key features:
- Streaming via SSE (Server-Sent Events) parsing
- Tool call accumulation from streamed chunks
- Retry with exponential backoff (`src/provider/retry.rs`)
- TTFB (Time to First Byte) watchdog for stream health monitoring
- Configurable base URL and API key

## Configuration

Providers are configured in `~/.config/sprachspiel/config.toml`:

```toml
[provider]
base_url = "http://localhost:11434/v1"  # Any OpenAI-compatible endpoint
api_key = ""                            # Optional, for cloud providers

[embedding]
model = "nomic-embed-text"
dimensions = 256                        # Matryoshka truncation
```

Model definitions in `~/.config/sprachspiel/models.toml`:

```toml
[models."my-model"]
provider = "openai"  # Currently the only option; reserved for future providers
tools = true
vision = false
thinking = false
```

> **Note:** The `provider` field in model configs is reserved for future multi-provider support. Currently, all models use the configured `base_url` endpoint.

## Module Structure

```
src/provider/
├── mod.rs              # LlmProvider trait definition
├── openai_compat.rs    # OpenAICompatibleProvider implementation
├── openai_types.rs     # OpenAI API request/response types
├── types.rs            # Agnostic types (LlmMessage, LlmResponse, etc.)
├── retry.rs            # Retry logic with exponential backoff
├── tool_accumulator.rs # Streamed tool call accumulation
└── embedding_models.rs # Embedding model configuration
```

## Error Handling

Provider errors use typed `ProviderError` with `RetryCategory` classification:

| Category | Behavior |
|----------|----------|
| `Retryable` | Retry with exponential backoff |
| `NonRetryable` | Fail immediately |
| `Unsupported` | Feature not supported by provider |
| `RateLimited` | Retry after delay |
| `Timeout` | Retry with backoff |

## Migration History

The provider layer replaced the former `ollama-rs` dependency (Wave 2, Milestone 1):

| Issue | Description | Status |
|-------|-------------|--------|
| #119 | Agnostic Provider Types | ✅ Complete |
| #120 | OllamaProvider (reqwest direct) | ✅ Complete |
| #121 | Consumer Migration | ✅ Complete |
| #122 | OpenAI-Compatible Provider | ✅ Complete |
| #123 | Remove ollama-rs from Cargo.toml | ✅ Complete |

All business code was migrated from `ollama-rs` types to the agnostic `LlmProvider` trait. The `ollama-rs` crate was removed from `Cargo.toml`.