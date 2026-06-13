//! Centralized model switching logic
//!
//! This module provides a single point for all model switching operations,
//! ensuring consistent state updates and capability handling.

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::user_models;

/// Result of a model switch operation
pub struct ModelSwitchResult {
    pub model_name: String,
    pub model_config: ModelConfig,
    pub capabilities: ModelCapabilities,
    pub tools_active: bool,
    pub think_active: bool,
    pub warnings: Vec<String>,
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
/// * `ollama` - Ollama client for capability detection
/// * `current_capabilities` - Current capabilities (fallback on detection failure)
/// * `current_think` - Current think mode state
/// * `current_tools` - Current tools state
///
/// # Returns
/// `Ok(ModelSwitchResult)` on success, or error message on failure.
pub async fn switch_model(
    model_name: &str,
    ollama: &crate::provider::Ollama,
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

    // W2 #121 extension: reject embedding-only models. Models
    // declared with `embeddings = true` in models.toml are
    // reserved for the indexing pipeline and cannot be used
    // for chat. The user must use `[indexing].model` to
    // reference them.
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

    // 3. Initialize warnings and detect capabilities (with fallback)
    let mut warnings = Vec::new();
    let capabilities = match ModelCapabilities::detect(ollama, &model_config.model_id).await {
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
    })
}
