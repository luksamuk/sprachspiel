//! Provider-agnostic abstraction layer for LLM backends.
//!
//! This module defines the `LlmProvider` trait and the agnostic types
//! used across all LLM providers (OpenAI-compatible, etc.).

pub mod embedding_models;
pub mod factory;
pub mod openai_compat;
pub mod openai_types;
pub mod retry;
pub mod tool_accumulator;
pub mod types;

pub use openai_compat::OpenAICompatibleProvider;
#[allow(unused_imports)]
pub use types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamEvent, LlmToolCall, LocalModel,
    ProviderCapabilities, ProviderError, ProviderOptions, RetryCategory, ToolFunctionInfo,
    ToolInfo, ToolType, retry_delay,
};

#[allow(unused_imports)]
pub use factory::build_provider;

use async_trait::async_trait;
use std::pin::Pin;

/// Core trait for LLM providers.
///
/// Implementations: `OpenAICompatibleProvider` (default).
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

    /// Streaming chat completion — returns a stream of semantic events.
    ///
    /// The stream carries `LlmStreamEvent` (text/thinking/tool-call
    /// deltas, retry lifecycle, completion) instead of aggregated chunks.
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
        Pin<Box<dyn futures::Stream<Item = Result<LlmStreamEvent, ProviderError>> + Send>>,
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
}
