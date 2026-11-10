//! Spy provider for embedding-path tests.
//!
//! Capture provider implementing `LlmProvider`: it records every text
//! that reaches the `/v1/embeddings` call so tests can assert the exact
//! rendered string and that the steering prefix was applied exactly
//! once. Test-only, no network. The call log is shared through an
//! `Arc` handle so tests can inspect it after the provider is moved
//! into the client.

use async_trait::async_trait;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::provider::types::{
    LlmMessage, LlmResponse, ProviderCapabilities, ProviderError, ProviderOptions, ToolInfo,
};

#[derive(Clone, Default)]
pub struct SpyCalls(Arc<Mutex<Vec<String>>>);

impl SpyCalls {
    /// The exact texts sent to the provider, in call order.
    pub fn texts(&self) -> Vec<String> {
        self.0.lock().expect("spy poisoned").clone()
    }
}

/// In-memory spy implementing `LlmProvider`.
pub struct SpyProvider {
    calls: SpyCalls,
    dims: usize,
}

impl SpyProvider {
    pub fn new(dims: usize) -> Self {
        Self {
            calls: SpyCalls::default(),
            dims,
        }
    }

    /// Shared, cloneable handle to the captured call log.
    pub fn calls(&self) -> SpyCalls {
        self.calls.clone()
    }
}

#[async_trait]
impl crate::provider::LlmProvider for SpyProvider {
    async fn chat(
        &self,
        _model: &str,
        _messages: Vec<LlmMessage>,
        _tools: Vec<ToolInfo>,
        _options: ProviderOptions,
    ) -> Result<LlmResponse, ProviderError> {
        Err(ProviderError::Unsupported("spy: no chat".into()))
    }

    async fn generate(
        &self,
        _model: &str,
        _prompt: &str,
        _images: Vec<String>,
        _audio: Vec<String>,
        _options: ProviderOptions,
    ) -> Result<String, ProviderError> {
        Err(ProviderError::Unsupported("spy: no generate".into()))
    }

    async fn embed(
        &self,
        text: &str,
        _model: &str,
        _dimensions: Option<usize>,
    ) -> Result<Vec<f32>, ProviderError> {
        self.calls.0.lock().expect("spy poisoned").push(text.to_string());
        Ok(vec![0.5; self.dims])
    }

    async fn detect_capabilities(
        &self,
        _model: &str,
    ) -> Result<ProviderCapabilities, ProviderError> {
        Err(ProviderError::Unsupported("spy: no capabilities".into()))
    }
}
