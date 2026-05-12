//! User-defined model configurations
//!
//! Allows users to define custom models or override built-in model parameters
//! via a TOML file at ~/.config/sprachspiel/models.toml

#![expect(clippy::print_stderr)] // CLI model management output
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::config::ModelConfig;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UserModelConfig {
    pub model_id: Option<String>,
    pub num_ctx: Option<u32>,
    pub temperature: Option<f32>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub thinking: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UserModelsFile {
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

fn load_user_models_internal() -> HashMap<String, UserModelConfig> {
    let path = get_user_models_path();

    if !path.exists() {
        return HashMap::new();
    }

    match fs::read_to_string(&path) {
        Ok(contents) => match toml::from_str::<UserModelsFile>(&contents) {
            Ok(file) => {
                let mut valid_models = HashMap::new();
                for (name, config) in file.models {
                    let is_builtin = ModelConfig::is_builtin_valid(&name);
                    if config.model_id.is_none() && !is_builtin {
                        eprintln!(
                            "Warning: User model '{}' is missing 'model_id' field. Skipping.",
                            name
                        );
                        continue;
                    }
                    valid_models.insert(name, config);
                }
                valid_models
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse user models file '{}': {}",
                    path.display(),
                    e
                );
                HashMap::new()
            }
        },
        Err(e) => {
            eprintln!(
                "Warning: Failed to read user models file '{}': {}",
                path.display(),
                e
            );
            HashMap::new()
        }
    }
}

static USER_MODELS: LazyLock<HashMap<String, UserModelConfig>> =
    LazyLock::new(load_user_models_internal);

pub fn get_user_models() -> &'static HashMap<String, UserModelConfig> {
    &USER_MODELS
}

pub fn merge_configs(built_in: Option<&ModelConfig>, user: &UserModelConfig) -> ModelConfig {
    match built_in {
        Some(bi) => ModelConfig {
            model_id: user.model_id.clone().unwrap_or_else(|| bi.model_id.clone()),
            num_ctx: user.num_ctx.unwrap_or(bi.num_ctx),
            temperature: user.temperature.unwrap_or(bi.temperature),
            top_k: user.top_k.or(bi.top_k),
            top_p: user.top_p.or(bi.top_p),
            repeat_penalty: user.repeat_penalty.or(bi.repeat_penalty),
            thinking: user.thinking.unwrap_or(bi.thinking),
        },
        None => ModelConfig {
            model_id: user.model_id.clone().unwrap_or_default(),
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
            model_id: None,
            num_ctx: Some(16384),
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: None,
            thinking: None,
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
            model_id: Some("custom-model:7b".to_string()),
            num_ctx: None,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.05),
            thinking: Some(true),
        };

        let merged = merge_configs(None, &user);

        assert_eq!(merged.model_id, "custom-model:7b");
        assert_eq!(merged.num_ctx, UserModelDefaults::NUM_CTX);
        assert_eq!(merged.temperature, UserModelDefaults::TEMPERATURE);
        assert_eq!(merged.repeat_penalty, Some(1.05));
        assert!(merged.thinking);
    }

    #[test]
    fn test_parse_user_models_file() {
        let toml_content = r#"
[models.my-custom]
model_id = "llama3:8b"
temperature = 0.7

[models.my-coder]
model_id = "phi3:mini"
num_ctx = 16384
"#;

        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        assert_eq!(parsed.models.len(), 2);
        assert!(parsed.models.contains_key("my-custom"));
        assert!(parsed.models.contains_key("my-coder"));

        let custom = parsed.models.get("my-custom").unwrap();
        assert_eq!(custom.model_id, Some("llama3:8b".to_string()));
        assert_eq!(custom.temperature, Some(0.7));
        assert_eq!(custom.num_ctx, None);
    }

    #[test]
    fn test_get_model_config_by_model_id_translategemma() {
        // "translategemma:4b" is the model_id in the builtin "translategemma" config.
        // It should be found via model_id lookup even though there's no config key
        // named "translategemma:4b".
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
        // "glm-ocr:bf16" is the model_id in the builtin "glm-ocr" config.
        let config = get_model_config("glm-ocr:bf16");
        assert!(config.is_some(), "glm-ocr:bf16 should resolve via model_id");
        let config = config.unwrap();
        assert_eq!(config.model_id, "glm-ocr:bf16");
        assert_eq!(config.temperature, 0.1);
        assert!(!config.thinking);
    }

    #[test]
    fn test_get_model_config_by_key_still_works() {
        // Exact config key lookups should still work normally
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
