use std::collections::HashMap;
use std::sync::LazyLock;

static CONFIGS: LazyLock<HashMap<&'static str, ModelConfig>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert(
        "llama",
        ModelConfig {
            model_id: "llama3.2:3b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.2,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    configs.insert(
        "qwen",
        ModelConfig {
            model_id: "qwen3-coder:30b-64k".to_string(),
            num_ctx: 65536,
            temperature: 0.2,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    configs.insert(
        "mistral",
        ModelConfig {
            model_id: "mistral-small3.2:24b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.2,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    configs.insert(
        "lfm",
        ModelConfig {
            model_id: "lfm2.5-thinking:1.2b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.1,
            top_k: 50,
            top_p: 0.1,
            repeat_penalty: 1.05,
        },
    );

    configs
});

/// Model configuration for different model presets
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub model_id: String,
    pub num_ctx: u32,
    pub temperature: f32,
    pub top_k: u32,
    pub top_p: f32,
    pub repeat_penalty: f32,
}

impl ModelConfig {
    /// Get a specific model configuration by name
    pub fn get(name: &str) -> Option<ModelConfig> {
        CONFIGS.get(name).cloned()
    }

    /// List all available model names
    pub fn list_names() -> Vec<&'static str> {
        vec!["llama", "qwen", "mistral", "lfm"]
    }
}
