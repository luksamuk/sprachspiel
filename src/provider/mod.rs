//! Provider-agnostic abstraction layer for LLM backends.
//!
//! This module defines the `LlmProvider` trait and the agnostic types
//! used across all LLM providers (Ollama, OpenAI-compatible, etc.).
//!
//! W2 Wave Context: Foundation of the Provider Chain (#119 → #123).

pub mod conversions;
pub mod factory;
pub mod ollama;
pub mod ollama_api;
pub mod types;

#[allow(unused_imports)] // Re-exported for #120/#121 consumers
pub use types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamChunk, LlmToolCall, LlmToolResult,
    ProviderCapabilities, ProviderError, ProviderOptions, RetryCategory, ToolFunctionInfo,
    ToolInfo, ToolType, retry_delay,
};

#[allow(unused_imports)] // Re-exported for #120 consumers
pub use factory::build_provider;

use async_trait::async_trait;
use std::pin::Pin;

/// Core trait for LLM providers.
///
/// Implementations: `OllamaProvider` (#120), `OpenAICompatibleProvider` (#122).
/// Business code should depend on this trait, not concrete providers.
#[allow(dead_code)] // Consumed by #120
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
    /// Providers that support streaming (Ollama, OpenAI-compatible) MUST override this.
    #[allow(unused_variables)] // Default impl does not consume parameters
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

    /// Provider identifier (e.g., "ollama", "openai-compatible").
    fn provider_name(&self) -> &str;

    /// Health check — is the provider reachable?
    async fn is_available(&self) -> bool;
}
