//! Provider factory — creates `LlmProvider` implementations from configuration.
//!
//! # Tool call streaming
//!
//! The two provider kinds have **fundamentally different** tool call streaming
//! behaviors. See `doc/src/development/research/openai-streaming-tool-calls.md`
//! for the full investigation. Short version:
//!
//! - `OllamaProvider` (native `/api/chat`): tool calls arrive **complete** in one
//!   chunk. `arguments` is already a JSON object. No `id` field.
//! - `OpenAICompatibleProvider` (OpenAI `/v1/chat/completions`): tool calls arrive
//!   **incrementally** over multiple chunks. `arguments` is a string that must be
//!   accumulated and then JSON-parsed. `id` field is present and required for
//!   correlation with subsequent `tool` role messages.

use crate::user_models::{ProviderConfig as UserProviderConfig, ProviderKind};
use crate::provider::types::ProviderError;
use std::collections::HashMap;

use crate::provider::ollama::OllamaProvider;

/// Build an `LlmProvider` from a provider configuration.
///
/// # Arguments
/// * `provider_name` - Name of the provider (key in the `[provider]` section of models.toml)
/// * `all_providers` - All provider configurations from the parsed models.toml
///
/// # Returns
/// * `Ok(Box<dyn LlmProvider>)` on success
/// * `Err(ProviderError)` if provider not found or initialization fails
pub fn build_provider(
    provider_name: &str,
    all_providers: &HashMap<String, UserProviderConfig>,
) -> Result<Box<dyn crate::provider::LlmProvider + Send + Sync>, ProviderError> {
    let config = all_providers.get(provider_name)
        .ok_or_else(|| ProviderError::Config(format!("Provider '{}' not found in models.toml", provider_name)))?;

    match config.kind {
        ProviderKind::Ollama => {
            let ollama_config = crate::provider::ollama::OllamaProviderConfig {
                base_url: config.base_url.clone(),
                connect_timeout_secs: config.connect_timeout_secs,
                read_timeout_secs: config.read_timeout_secs,
                stream_idle_timeout_secs: config.stream_idle_timeout_secs,
                max_retries: config.max_retries,
                retry_base_delay_ms: config.retry_base_delay_ms,
                retry_max_delay_ms: config.retry_max_delay_ms,
                retry_jitter_percent: config.retry_jitter_percent,
            };
            OllamaProvider::new(ollama_config).map(|p| Box::new(p) as Box<dyn crate::provider::LlmProvider + Send + Sync>)
        }
        ProviderKind::OpenAICompatible => {
            Err(ProviderError::Unsupported(
                "OpenAICompatibleProvider not yet implemented (see #122)".to_string()
            ))
        }
    }
}