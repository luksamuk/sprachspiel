//! Configuration module — built-in model presets and defaults.
//!
//! Provides [`ModelConfig`] structs with optimized inference parameters (context window,
//! temperature, top_p, etc.) for each supported model. These presets serve as defaults
//! when the user has no `models.toml` override.
//!
//! W2 #121: The model schema is now OpenAI-aligned. `top_k`, `repeat_penalty`, and
//! `think` (Ollama-native fields) are no longer exposed. `num_ctx` may be auto-detected
//! from the server. `build_provider_options()` produces a `ProviderOptions` for
//! `LlmProvider` instead of an `ollama_rs::ModelOptions`.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::provider::types::ProviderOptions;

static CONFIGS: LazyLock<HashMap<&'static str, ModelConfig>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    // Default model for general queries, code, and vision (multimodal)
    configs.insert(
        "qwen3.5:4b",
        ModelConfig {
            model_id: "qwen3.5:4b".to_string(),
            num_ctx: 131072,
            temperature: 1.0,
            top_p: Some(0.95),
            thinking: true,
        },
    );

    // Translation model (used by translate command)
    configs.insert(
        "translategemma",
        ModelConfig {
            model_id: "translategemma:4b".to_string(),
            num_ctx: 4096,
            temperature: 0.2,
            top_p: None,
            thinking: false,
        },
    );

    // OCR model (used by ocr command)
    configs.insert(
        "glm-ocr",
        ModelConfig {
            model_id: "glm-ocr:bf16".to_string(),
            num_ctx: 0,
            temperature: 0.1,
            top_p: None,
            thinking: false,
        },
    );

    configs
});

/// Built-in model configuration (W2 #121 OpenAI-aligned).
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub model_id: String,
    /// 0 means "auto-detect" (via /v1/models or /api/show).
    pub num_ctx: u32,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub thinking: bool,
}

impl ModelConfig {
    #[cfg(test)]
    pub fn get(name: &str) -> Option<ModelConfig> {
        CONFIGS.get(name).cloned()
    }

    pub fn get_builtin(name: &str) -> Option<&'static ModelConfig> {
        CONFIGS.get(name)
    }

    /// Look up a built-in model config by its model_id field.
    pub fn get_builtin_by_model_id(model_id: &str) -> Option<&'static ModelConfig> {
        CONFIGS.values().find(|mc| mc.model_id == model_id)
    }

    #[cfg(test)]
    pub fn get_default() -> ModelConfig {
        CONFIGS
            .values()
            .find(|mc| mc.model_id == "qwen3.5:4b")
            .cloned()
            .unwrap()
    }

    #[cfg(test)]
    pub fn list_names() -> Vec<&'static str> {
        Self::list_builtin_names()
    }

    pub fn list_builtin_names() -> Vec<&'static str> {
        vec!["qwen3.5:4b", "translategemma", "glm-ocr"]
    }

    pub fn is_builtin_valid(name: &str) -> bool {
        CONFIGS.contains_key(name)
    }

    /// Build provider-agnostic `ProviderOptions` from this config.
    ///
    /// W2 #121: This replaces `build_model_options()` (which returned
    /// `ollama_rs::models::ModelOptions`). The output `ProviderOptions`
    /// can be consumed by any `LlmProvider` implementation.
    pub fn build_provider_options(&self) -> ProviderOptions {
        ProviderOptions {
            temperature: Some(self.temperature),
            top_p: self.top_p,
            // W2 #121: top_k, repeat_penalty removed (not OpenAI-spec)
            top_k: None,
            repeat_penalty: None,
            num_predict: None,
            stop_sequences: None,
            think: Some(self.thinking),
            format: None,
            audio_format: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_is_qwen35_4b() {
        let default = ModelConfig::get_default();
        assert_eq!(default.model_id, "qwen3.5:4b");
    }

    #[test]
    fn test_all_models_exist() {
        let names = ModelConfig::list_names();
        assert_eq!(names.len(), 3);

        for name in names {
            assert!(
                ModelConfig::is_builtin_valid(name),
                "Model {name} should exist"
            );
            assert!(
                ModelConfig::get(name).is_some(),
                "Model {name} should be retrievable"
            );
        }
    }

    #[test]
    fn test_invalid_model() {
        assert!(!ModelConfig::is_builtin_valid("nonexistent"));
        assert!(ModelConfig::get("nonexistent").is_none());
    }

    #[test]
    fn test_qwen35_4b_parameters() {
        let qwen = ModelConfig::get("qwen3.5:4b").unwrap();
        assert_eq!(qwen.model_id, "qwen3.5:4b");
        assert_eq!(qwen.num_ctx, 131072);
        assert_eq!(qwen.temperature, 1.0);
        assert_eq!(qwen.top_p, Some(0.95));
        assert!(qwen.thinking);
    }

    #[test]
    fn test_translategemma_parameters() {
        let trans = ModelConfig::get("translategemma").unwrap();
        assert_eq!(trans.model_id, "translategemma:4b");
        assert_eq!(trans.num_ctx, 4096);
        assert_eq!(trans.temperature, 0.2);
        assert!(!trans.thinking);
    }

    #[test]
    fn test_glm_ocr_parameters() {
        let ocr = ModelConfig::get("glm-ocr").unwrap();
        assert_eq!(ocr.model_id, "glm-ocr:bf16");
        assert_eq!(ocr.num_ctx, 0); // auto-detect
        assert_eq!(ocr.temperature, 0.1);
        assert!(!ocr.thinking);
    }

    #[test]
    fn test_get_builtin_by_model_id() {
        let by_key = ModelConfig::get_builtin("glm-ocr");
        assert!(by_key.is_some());
        assert_eq!(by_key.unwrap().model_id, "glm-ocr:bf16");

        let by_id = ModelConfig::get_builtin_by_model_id("glm-ocr:bf16");
        assert!(by_id.is_some());
        assert_eq!(by_id.unwrap().model_id, "glm-ocr:bf16");
        assert_eq!(by_id.unwrap().temperature, 0.1);

        let by_wrong_key = ModelConfig::get_builtin("glm-ocr:bf16");
        assert!(by_wrong_key.is_none());

        let qwen_by_id = ModelConfig::get_builtin_by_model_id("qwen3.5:4b");
        assert!(qwen_by_id.is_some());
        assert_eq!(qwen_by_id.unwrap().temperature, 1.0);

        assert!(ModelConfig::get_builtin_by_model_id("nonexistent:7b").is_none());
    }

    #[test]
    fn test_build_provider_options_qwen() {
        let qwen = ModelConfig::get("qwen3.5:4b").unwrap();
        let opts = qwen.build_provider_options();
        assert_eq!(opts.temperature, Some(1.0));
        assert_eq!(opts.top_p, Some(0.95));
        assert_eq!(opts.think, Some(true));
        assert!(opts.top_k.is_none());
        assert!(opts.repeat_penalty.is_none());
    }

    #[test]
    fn test_build_provider_options_ocr() {
        let ocr = ModelConfig::get("glm-ocr").unwrap();
        let opts = ocr.build_provider_options();
        assert_eq!(opts.temperature, Some(0.1));
        assert_eq!(opts.think, Some(false));
        assert!(opts.top_p.is_none());
    }
}
