//! Provider-agnostic abstraction layer for LLM backends.
//!
//! This module defines the `LlmProvider` trait and the agnostic types
//! used across all LLM providers (OpenAI-compatible, Ollama, etc.).
//!
//! W2 Provider Chain (#119 → #123). The default provider in #121 is
//! `OpenAICompatibleProvider`, which talks to Ollama via `/v1/chat/completions`
//! and also handles OpenAI, llama.cpp, vLLM, LM Studio, llama-swap, etc.

pub mod factory;
pub mod ollama_shim;
pub mod openai_compat;
pub mod openai_types;
pub mod types;

/// Re-export of `crate::provider::Ollama`-compatible shim. Production code
/// that needs a HTTP provider uses `Ollama` (the shim) or `LlmProvider`
/// (the trait) diretamente. W2 #121: prefer `LlmProvider` for new code.
pub use ollama_shim::CompatOllama as Ollama;

#[allow(unused_imports)]
pub use types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamChunk, LlmToolCall, LlmToolResult,
    ProviderCapabilities, ProviderError, ProviderOptions, RetryCategory, ToolFunctionInfo,
    ToolInfo, ToolType, retry_delay,
};

#[allow(unused_imports)]
pub use factory::build_provider;

use async_trait::async_trait;
use std::pin::Pin;

/// Core trait for LLM providers.
///
/// Implementations: `OpenAICompatibleProvider` (#121, default).
/// Business code should depend on this trait, not concrete providers.
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

    /// Streaming chat completion — returns a stream of response chunks.
    ///
    /// Default implementation returns `Err(ProviderError::Unsupported)`.
    /// Providers that support streaming MUST override this.
    #[allow(unused_variables)]
    async fn chat_stream(
        &self,
        _model: &str,
        _messages: Vec<LlmMessage>,
        _tools: Vec<ToolInfo>,
        _options: ProviderOptions,
    ) -> Result<
        Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        Err(ProviderError::Unsupported(
            "streaming not implemented".into(),
        ))
    }

    /// Generate a completion (non-chat, e.g., for vision/OCR).
    async fn generate(
        &self,
        model: &str,
        prompt: &str,
        images: Vec<String>, // base64-encoded
        audio: Vec<String>,  // base64-encoded (mp3, wav, ogg)
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

    /// Provider identifier (e.g., "openai-compatible").
    fn provider_name(&self) -> &str;

    /// Health check — is the provider reachable?
    async fn is_available(&self) -> bool;
}
