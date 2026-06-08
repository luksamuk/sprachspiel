//! User-defined model configurations
//!
//! Allows users to define custom models or override built-in model parameters
//! via a TOML file at ~/.config/sprachspiel/models.toml
//!
//! New format (breaking change from #120):
//! ```toml
//! [provider."my-ollama"]
//! kind = "ollama"
//! base_url = "http://localhost:11434"
//! connect_timeout_secs = 5
//! read_timeout_secs = 300
//! stream_idle_timeout_secs = 60
//! max_retries = 3
//! retry_base_delay_ms = 2000
//! retry_max_delay_ms = 16000
//! retry_jitter_percent = 20
//!
//! [models.glm-5.1]
//! model_id = "glm-5.1:cloud"
//! num_ctx = 202757
//! thinking = true
//! tools = true
//! provider = "my-ollama"
//! ```

#![expect(clippy::print_stderr)] // CLI model management output
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::config::ModelConfig;

/// Provider kind enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Ollama,
    OpenAICompatible,
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::Ollama
    }
}

/// Provider configuration.
///
/// All timeouts are in seconds/milliseconds as indicated.
/// Defaults (used when field is omitted):
/// - connect_timeout_secs = 5
/// - read_timeout_secs = 300
/// - stream_idle_timeout_secs = 60
/// - max_retries = 3
/// - retry_base_delay_ms = 2000
/// - retry_max_delay_ms = 16000
/// - retry_jitter_percent = 20
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,

    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,

    #[serde(default = "default_read_timeout")]
    pub read_timeout_secs: u64,

    #[serde(default = "default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: u64,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,

    #[serde(default = "default_retry_max_delay_ms")]
    pub retry_max_delay_ms: u64,

    #[serde(default = "default_retry_jitter_percent")]
    pub retry_jitter_percent: u8,

    // Future: OpenAI-compatible specific
    pub api_key_env: Option<String>,
}

fn default_connect_timeout() -> u64 {
    5
}
fn default_read_timeout() -> u64 {
    300
}
fn default_stream_idle_timeout() -> u64 {
    60
}
fn default_max_retries() -> u32 {
    3
}
fn default_retry_base_delay_ms() -> u64 {
    2000
}
fn default_retry_max_delay_ms() -> u64 {
    16000
}
fn default_retry_jitter_percent() -> u8 {
    20
}

impl ProviderConfig {
    /// Normalize base_url to ensure it has a scheme (http:// or https://)
    pub fn normalize_base_url(&mut self) {
        let trimmed = self.base_url.trim();
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            self.base_url = format!("http://{}", trimmed);
        }
    }
}

/// User model configuration.
///
/// The `provider` field is required and must match a key in the `[provider]` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserModelConfig {
    pub model_id: String,
    pub num_ctx: Option<u32>,
    pub temperature: Option<f32>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub thinking: Option<bool>,
    pub tools: Option<bool>,
    pub provider: String,
}

/// Complete user models file structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserModelsFile {
    pub provider: HashMap<String, ProviderConfig>,
    pub models: HashMap<String, UserModelConfig>,
}

pub struct UserModelDefaults;

impl UserModelDefaults {
    pub const NUM_CTX: u32 = 32768;
    pub const TEMPERATURE: f32 = 0.8;
    pub const REPEAT_PENALTY: f32 = 1.1;
}

pub fn get_user_models_path() -> PathBuf {
    use crate::consts::app;

    if let Ok(data_home) = env::var("XDG_DATA_HOME") {
        PathBuf::from(data_home)
            .join(app::APP_CONFIG_DIR)
            .join("models.toml")
    } else if let Some(home_dir) = dirs::home_dir() {
        home_dir
            .join(".config")
            .join(app::APP_CONFIG_DIR)
            .join("models.toml")
    } else {
        PathBuf::from(app::APP_PROJECT_DIR).join("models.toml")
    }
}

fn load_user_models_internal() -> Result<UserModelsFile, String> {
    let path = get_user_models_path();

    if !path.exists() {
        return Err(format!("Models file not found at {}", path.display()));
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read models file '{}': {}", path.display(), e))?;

    let mut file: UserModelsFile = toml::from_str(&contents)
        .map_err(|e| format!("Failed to parse models file '{}': {}", path.display(), e))?;

    // Validate: provider section must exist
    if file.provider.is_empty() {
        return Err("Missing [provider] section in models.toml".to_string());
    }

    // Validate: at least one model must exist
    if file.models.is_empty() {
        return Err("No models defined in models.toml".to_string());
    }

    // Normalize base_url for all providers
    for (_, provider_config) in &mut file.provider {
        provider_config.normalize_base_url();
    }

    // Validate: all model providers exist
    for (model_name, model_config) in &file.models {
        if !file.provider.contains_key(&model_config.provider) {
            return Err(format!(
                "Model '{}' references unknown provider '{}'",
                model_name, model_config.provider
            ));
        }
    }

    // Model name uniqueness is guaranteed by HashMap keys

    Ok(file)
}

/// Cached loaded models file (panics on error - configuration errors should be caught at startup)
static USER_MODELS_FILE: LazyLock<UserModelsFile> = LazyLock::new(|| {
    load_user_models_internal().unwrap_or_else(|e| {
        eprintln!("Error loading models.toml: {}", e);
        std::process::exit(1);
    })
});

/// Get provider configs.
pub fn get_providers() -> &'static HashMap<String, ProviderConfig> {
    &USER_MODELS_FILE.provider
}

/// Get user-defined model configs.
pub fn get_user_models() -> &'static HashMap<String, UserModelConfig> {
    &USER_MODELS_FILE.models
}

/// Get the provider name for a given model name.
///
/// Returns `Some(provider_name)` if the model is in `models.toml` and has
/// a `provider` field set. Returns `None` for built-in models (which don't
/// have a `provider` field, since they were defined before the multi-provider
/// refactor) or for unknown model names.
///
/// Used by the chat banner to display "Provider: <name>" instead of the
/// server URL.
pub fn get_provider_for_model(model_name: &str) -> Option<String> {
    get_user_models()
        .get(model_name)
        .map(|cfg| cfg.provider.clone())
}

pub fn merge_configs(built_in: Option<&ModelConfig>, user: &UserModelConfig) -> ModelConfig {
    match built_in {
        Some(bi) => ModelConfig {
            model_id: user.model_id.clone(),
            num_ctx: user.num_ctx.unwrap_or(bi.num_ctx),
            temperature: user.temperature.unwrap_or(bi.temperature),
            top_k: user.top_k.or(bi.top_k),
            top_p: user.top_p.or(bi.top_p),
            repeat_penalty: user.repeat_penalty.or(bi.repeat_penalty),
            thinking: user.thinking.unwrap_or(bi.thinking),
        },
        None => ModelConfig {
            model_id: user.model_id.clone(),
            num_ctx: user.num_ctx.unwrap_or(UserModelDefaults::NUM_CTX),
            temperature: user.temperature.unwrap_or(UserModelDefaults::TEMPERATURE),
            top_k: user.top_k,
            top_p: user.top_p,
            repeat_penalty: user
                .repeat_penalty
                .or(Some(UserModelDefaults::REPEAT_PENALTY)),
            thinking: user.thinking.unwrap_or(false),
        },
    }
}

pub fn get_model_config(name: &str) -> Option<ModelConfig> {
    // Try config key lookup first (e.g., "glm-ocr", "translategemma")
    let built_in = ModelConfig::get_builtin(name);
    let user_models = get_user_models();
    let user_config = user_models.get(name);

    match (built_in, user_config) {
        (Some(bi), Some(uc)) => Some(merge_configs(Some(bi), uc)),
        (Some(bi), None) => Some(bi.clone()),
        (None, Some(uc)) => Some(merge_configs(None, uc)),
        (None, None) => {
            // Fall back to lookup by model_id (e.g., "translategemma:4b" matches builtin config)
            ModelConfig::get_builtin_by_model_id(name).cloned()
        }
    }
}

pub fn is_model_valid(name: &str) -> bool {
    ModelConfig::is_builtin_valid(name) || get_user_models().contains_key(name)
}

/// Resolve model configuration with error handling
///
/// Returns the model configuration or prints an error and exits.
/// Use this instead of the `is_model_valid` + `get_model_config().unwrap()` pattern.
pub fn resolve_model_config(name: &str) -> ModelConfig {
    match get_model_config(name) {
        Some(config) => config,
        None => {
            eprintln!(
                "Error: Unknown model '{}'. Use --list to see available models.",
                name
            );
            std::process::exit(1);
        }
    }
}

/// Determine if think mode should be enabled
///
/// Priority: CLI flag > model config > subcommand config
/// Returns the final think mode state and prints warning if model doesn't support it.
pub fn resolve_think_mode(
    cli_think: bool,
    subcommand_thinking: bool,
    model_config_thinking: bool,
    model_id: &str,
    capabilities_thinking: bool,
) -> bool {
    let model_supports_think = capabilities_thinking || model_config_thinking;

    if cli_think {
        if model_supports_think {
            true
        } else {
            eprintln!(
                "Warning: Model '{}' does not support think mode. Ignoring -t flag.",
                model_id
            );
            false
        }
    } else {
        (subcommand_thinking || model_config_thinking) && model_supports_think
    }
}

pub fn list_all_model_names() -> Vec<String> {
    let mut names: Vec<String> = ModelConfig::list_builtin_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    for name in get_user_models().keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }

    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_model_defaults() {
        assert_eq!(UserModelDefaults::NUM_CTX, 32768);
        assert_eq!(UserModelDefaults::TEMPERATURE, 0.8);
        assert_eq!(UserModelDefaults::REPEAT_PENALTY, 1.1);
    }

    #[test]
    fn test_merge_partial_override() {
        let built_in = ModelConfig {
            model_id: "test:1b".to_string(),
            num_ctx: 8192,
            temperature: 0.5,
            top_k: Some(50),
            top_p: Some(0.95),
            repeat_penalty: Some(1.1),
            thinking: false,
        };

        let user = UserModelConfig {
            model_id: "test:1b".to_string(),
            num_ctx: Some(16384),
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: None,
            thinking: None,
            tools: None,
            provider: "test".to_string(),
        };

        let merged = merge_configs(Some(&built_in), &user);

        assert_eq!(merged.model_id, "test:1b");
        assert_eq!(merged.num_ctx, 16384);
        assert_eq!(merged.temperature, 0.5);
        assert_eq!(merged.top_k, Some(50));
    }

    #[test]
    fn test_user_only_model_no_ctx() {
        let user = UserModelConfig {
            model_id: "custom-model:7b".to_string(),
            num_ctx: None,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.05),
            thinking: Some(true),
            tools: None,
            provider: "test".to_string(),
        };

        let merged = merge_configs(None, &user);

        assert_eq!(merged.model_id, "custom-model:7b");
        assert_eq!(merged.num_ctx, UserModelDefaults::NUM_CTX);
        assert_eq!(merged.temperature, UserModelDefaults::TEMPERATURE);
        assert_eq!(merged.repeat_penalty, Some(1.05));
        assert!(merged.thinking);
    }

    #[test]
    fn test_parse_user_models_file_new_format() {
        let toml_content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "localhost:11434"
connect_timeout_secs = 10
read_timeout_secs = 600

[models."glm-5.1"]
model_id = "glm-5.1:cloud"
num_ctx = 202757
thinking = true
tools = true
provider = "my-ollama"
"#;

        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();

        assert_eq!(parsed.provider.len(), 1);
        assert!(parsed.provider.contains_key("my-ollama"));

        let prov = parsed.provider.get("my-ollama").unwrap();
        assert_eq!(prov.kind, ProviderKind::Ollama);
        // Note: URL normalization happens in load_user_models_internal, not in
        // toml::from_str. The raw value preserves the user's input here.
        assert_eq!(prov.base_url, "localhost:11434");
        assert_eq!(prov.connect_timeout_secs, 10);
        assert_eq!(prov.read_timeout_secs, 600);

        assert_eq!(parsed.models.len(), 1);
        let model = parsed.models.get("glm-5.1").unwrap();
        assert_eq!(model.model_id, "glm-5.1:cloud");
        assert_eq!(model.provider, "my-ollama");
    }

    #[test]
    fn test_url_normalization_in_load() {
        // Verify that load_user_models_internal() normalizes base_url
        let toml_content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "localhost:11434"

[models."test-model"]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let mut parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        // Apply the same normalization as load_user_models_internal
        for (_, provider_config) in &mut parsed.provider {
            provider_config.normalize_base_url();
        }
        let prov = parsed.provider.get("my-ollama").unwrap();
        assert_eq!(prov.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_provider_defaults() {
        let toml_content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "http://localhost:11434"

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;

        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        let prov = parsed.provider.get("my-ollama").unwrap();

        // Check defaults are applied
        assert_eq!(prov.connect_timeout_secs, 5);
        assert_eq!(prov.read_timeout_secs, 300);
        assert_eq!(prov.stream_idle_timeout_secs, 60);
        assert_eq!(prov.max_retries, 3);
        assert_eq!(prov.retry_base_delay_ms, 2000);
        assert_eq!(prov.retry_max_delay_ms, 16000);
        assert_eq!(prov.retry_jitter_percent, 20);
    }

    #[test]
    fn test_get_model_config_by_model_id_translategemma() {
        let config = get_model_config("translategemma:4b");
        assert!(
            config.is_some(),
            "translategemma:4b should resolve via model_id"
        );
        let config = config.unwrap();
        assert_eq!(config.model_id, "translategemma:4b");
        assert_eq!(config.temperature, 0.2);
        assert!(!config.thinking);
    }

    #[test]
    fn test_get_model_config_by_model_id_glm_ocr() {
        let config = get_model_config("glm-ocr:bf16");
        assert!(config.is_some(), "glm-ocr:bf16 should resolve via model_id");
        let config = config.unwrap();
        assert_eq!(config.model_id, "glm-ocr:bf16");
        assert_eq!(config.temperature, 0.1);
        assert!(!config.thinking);
    }

    #[test]
    fn test_get_model_config_by_key_still_works() {
        let config = get_model_config("translategemma");
        assert!(config.is_some());
        assert_eq!(config.unwrap().model_id, "translategemma:4b");

        let config = get_model_config("glm-ocr");
        assert!(config.is_some());
        assert_eq!(config.unwrap().model_id, "glm-ocr:bf16");
    }

    #[test]
    fn test_get_model_config_unknown_returns_none() {
        let config = get_model_config("nonexistent:model");
        assert!(config.is_none());
    }
}
