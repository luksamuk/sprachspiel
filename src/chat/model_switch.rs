//! Centralized model switching logic
//!
//! This module provides a single point for all model switching operations,
//! ensuring consistent state updates and capability handling.

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::settings::Settings;
use crate::user_models;

/// Result of a model switch operation
pub struct ModelSwitchResult {
    pub model_name: String,
    pub model_config: ModelConfig,
    pub capabilities: ModelCapabilities,
    pub tools_active: bool,
    pub think_active: bool,
    pub warnings: Vec<String>,
    /// O cliente LLM reconstruído para o provider do novo modelo.
    /// O caller DEVE atualizar `state.ollama` com este valor para que
    /// os próximos requests vão para o provider correto (declarado em
    /// `models.toml` para o novo modelo). Sem isto, o `/model` troca o
    /// `model_config` mas mantém o cliente HTTP do provider antigo,
    /// causando `no router for requested model` no próximo prompt.
    pub ollama: crate::provider::Ollama,
}

/// Switch to a new model with full state management.
///
/// This is the SINGLE point for all model switching logic.
/// It handles:
/// - Model validation
/// - Config resolution
/// - Capability detection
/// - Think/tools state adjustment
/// - Warning generation
///
/// # Arguments
/// * `model_name` - The model name to switch to (e.g., "llama3.1", "qwen3")
/// * `settings` - Settings used to build the LLM client for the new model's
///   provider (resolved from `models.toml`). This replaces the previous
///   `ollama: &Ollama` parameter — the caller no longer needs to rebuild
///   the client separately; `switch_model` does it internally and returns
///   the new client in `ModelSwitchResult.ollama`.
/// * `current_capabilities` - Current capabilities (fallback on detection failure)
/// * `current_think` - Current think mode state
/// * `current_tools` - Current tools state
///
/// # Returns
/// `Ok(ModelSwitchResult)` on success, or error message on failure.
pub async fn switch_model(
    model_name: &str,
    settings: &Settings,
    current_capabilities: &ModelCapabilities,
    current_think: bool,
    current_tools: bool,
) -> Result<ModelSwitchResult, String> {
    // 1. Validate model exists
    if !user_models::is_model_valid(model_name) {
        return Err(format!(
            "Unknown model: '{}'. Use --list to see available models.",
            model_name
        ));
    }

    // Reject embedding-only models. Models declared with
    // `embeddings = true` in models.toml are reserved for the
    // indexing pipeline and cannot be used for chat. The user must
    // use `[indexing].model` to reference them.
    if user_models::is_model_embedding_only(model_name) {
        return Err(format!(
            "'{model_name}' is an embedding-only model and cannot be used \
             for chat. Use `[indexing].model = \"{model_name}\"` in \
             config.toml to reference it for embedding generation, or \
             pick a chat model from --list."
        ));
    }

    // Bail-out: detect broken config before reaching resolve_model_config's
    // process::exit(1). If the user is mid-session and models.toml becomes
    // invalid, we want to surface the configuration error gracefully via
    // the TUI error channel rather than aborting the process.
    user_models::require_providers()?;

    // 2. Resolve model configuration
    let model_config = user_models::resolve_model_config(model_name);

    // 2b. Rebuild the LLM client for the NEW model's provider. The provider
    // is declared per-model in `models.toml` (`provider = "<name>"`).
    // `settings.ollama_client_for_model()` resolves the provider and
    // builds the `CompatOllama` shim pointing to its base_url. Without this,
    // the caller's `state.ollama` would stay bound to the initial model's
    // provider, causing `no router for requested model` errors when the new
    // model lives on a different provider (e.g., switching from llama-swap
    // to ollama). The same client is used for capability detection below,
    // so detection hits the right provider from the start.
    //
    // R5 recommendation: log the provider resolution for auditability. If
    // something goes wrong with provider switching in production, these
    // logs are the trail that shows which provider was resolved and whether
    // the client was rebuilt.
    let provider_name = user_models::get_provider_for_model(model_name);
    match &provider_name {
        Some(p) => log::info!(
            "Model switch: '{}' resolves to provider '{}' — rebuilding LLM client",
            model_name,
            p
        ),
        None => log::warn!(
            "Model switch: no provider declared for '{}' in models.toml — \
             falling back to first available OpenAI-compatible provider",
            model_name
        ),
    }
    let ollama = settings.ollama_client_for_model(model_name);
    log::debug!(
        "Model switch: built new LLM client for '{}' (provider: {})",
        model_name,
        provider_name.as_deref().unwrap_or("<fallback>")
    );

    // 3. Initialize warnings and detect capabilities (with fallback)
    let mut warnings = Vec::new();
    log::debug!(
        "Model switch: detecting capabilities for '{}' via the new provider",
        model_config.model_id
    );
    let capabilities = match ModelCapabilities::detect(&ollama, &model_config.model_id).await {
        Ok(c) => c,
        Err(e) => {
            warnings.push(format!(
                "Could not detect capabilities for '{}': {}. Using defaults.",
                model_config.model_id, e
            ));
            current_capabilities.clone()
        }
    };

    // 4. Calculate new states

    let think_active = if current_think && !capabilities.thinking {
        warnings.push(format!(
            "Note: '{}' does not support think mode. Think mode disabled.",
            model_name
        ));
        false
    } else {
        current_think
    };

    let tools_active = if current_tools && !capabilities.tools {
        warnings.push(format!(
            "Warning: Tools are enabled but '{}' does not support tool calling. Tools disabled.",
            model_name
        ));
        false
    } else {
        current_tools
    };

    Ok(ModelSwitchResult {
        model_name: model_name.to_string(),
        model_config,
        capabilities,
        tools_active,
        think_active,
        warnings,
        ollama,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `CompatOllama` shim exposes `base_url` via its `Debug` impl
    /// (`ollama_shim.rs:423`). Extract it so tests can assert which
    /// provider the client points to without accessing the private field.
    fn ollama_base_url(ollama: &crate::provider::Ollama) -> String {
        let dbg = format!("{ollama:?}");
        // The Debug output is: CompatOllama { base_url: "..." }
        let start = dbg.find("base_url: \"").map(|i| i + "base_url: \"".len());
        let end = dbg.rfind('"');
        match (start, end) {
            (Some(s), Some(e)) if s < e => dbg[s..e].to_string(),
            _ => String::new(),
        }
    }

    /// Verifies that `switch_model` builds a NEW `ollama` client pointing
    /// to the provider declared in `models.toml` for the requested model.
    ///
    /// This is a regression test for the provider-switching bug: previously
    /// `switch_model` received the existing `&ollama` and could not rebuild
    /// it, so `state.ollama` stayed bound to the initial model's provider
    /// even after `/model` switched to a model on a different provider.
    ///
    /// Environment-dependent: requires `qwen3.5-4b-abliterated` (provider:
    /// `llama-swap`) and `glm-5.2` (provider: `ollama`) to be declared in
    /// `~/.config/sprachspiel/models.toml`. Skips (returns early) if either
    /// model is missing so the test is a no-op in CI without that config.
    ///
    /// Does NOT require any provider to be online — capability detection
    /// falls back to `current_capabilities` on connection failure, but the
    /// `ollama` client is still constructed with the correct `base_url`.
    #[tokio::test]
    async fn switch_model_rebuilds_ollama_for_new_provider() {
        let local = "qwen3.5-4b-abliterated";
        let cloud = "glm-5.2";
        // Sanity: skip if the environment's models.toml doesn't declare
        // these models (keeps the test a no-op in CI without local config).
        if !user_models::is_model_valid(local) || !user_models::is_model_valid(cloud) {
            eprintln!("skip: test models not in models.toml");
            return;
        }
        let settings = Settings::default();
        let caps = ModelCapabilities::default();

        let result_local = switch_model(local, &settings, &caps, false, false)
            .await
            .expect("local switch should succeed");
        let result_cloud = switch_model(cloud, &settings, &caps, false, false)
            .await
            .expect("cloud switch should succeed");

        let local_url = ollama_base_url(&result_local.ollama);
        let cloud_url = ollama_base_url(&result_cloud.ollama);

        // The two providers have different base_urls (llama-swap vs ollama).
        assert!(
            local_url != cloud_url,
            "expected different base_urls for different providers, got \
             local={local_url:?} cloud={cloud_url:?}"
        );
        // Each client should point to its model's provider base_url.
        let local_provider =
            user_models::get_provider_for_model(local).expect("local model should have a provider");
        let cloud_provider =
            user_models::get_provider_for_model(cloud).expect("cloud model should have a provider");
        let providers = user_models::get_providers();
        let local_expected = &providers[&local_provider].base_url;
        let cloud_expected = &providers[&cloud_provider].base_url;
        assert_eq!(
            local_url, *local_expected,
            "local model client base_url mismatch"
        );
        assert_eq!(
            cloud_url, *cloud_expected,
            "cloud model client base_url mismatch"
        );
    }
}
