//! Provider factory — creates `LlmProvider` implementations from configuration.
//!
//! W2 #121: The default provider is now `OpenAICompatibleProvider`.
//! `ProviderKind::Ollama` is mapped to `OllamaLegacy` which returns a
//! runtime error prompting the user to run `sprach models upgrade`.

#![allow(dead_code)] // W2 #123: build_provider will be wired when ollama-rs is removed

use crate::provider::openai_compat::{OpenAICompatibleConfig, OpenAICompatibleProvider};
use crate::provider::types::ProviderError;
use crate::user_models::{ProviderConfig as UserProviderConfig, ProviderKind};
use std::collections::HashMap;

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
    let config = all_providers.get(provider_name).ok_or_else(|| {
        ProviderError::Config(format!(
            "Provider '{}' not found in models.toml",
            provider_name
        ))
    })?;

    match config.kind {
        ProviderKind::OpenAI => {
            let openai_config = OpenAICompatibleConfig::from(config);
            OpenAICompatibleProvider::new(openai_config)
                .map(|p| Box::new(p) as Box<dyn crate::provider::LlmProvider + Send + Sync>)
        }
        ProviderKind::OllamaLegacy => Err(ProviderError::Config(
            "ProviderKind 'ollama' is deprecated (W2 #121). \
             Run `sprach models upgrade` to migrate to kind = \"openai\". \
             The base_url should include the /v1 suffix (e.g., http://localhost:11434/v1)."
                .to_string(),
        )),
        ProviderKind::Anthropic => Err(ProviderError::Unsupported(
            "Anthropic provider is not yet implemented (M3 or later).".to_string(),
        )),
    }
}
