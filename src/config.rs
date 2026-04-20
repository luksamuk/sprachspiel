use std::collections::HashMap;
use std::sync::LazyLock;

use ollama_rs::models::ModelOptions;

pub const DEFAULT_MODEL: &str = "qwen3.5:4b";

static CONFIGS: LazyLock<HashMap<&'static str, ModelConfig>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    // Default model for general queries, code, and vision (multimodal)
    configs.insert(
        "qwen3.5:4b",
        ModelConfig {
            model_id: "qwen3.5:4b".to_string(),
            num_ctx: 131072,
            temperature: 1.0,
            top_k: Some(20),
            top_p: Some(0.95),
            repeat_penalty: Some(1.0),
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
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.1),
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
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.0),
            thinking: false,
        },
    );

    configs
});

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub model_id: String,
    pub num_ctx: u32,
    pub temperature: f32,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub thinking: bool,
}

impl ModelConfig {
    #[allow(dead_code)]
    pub fn get(name: &str) -> Option<ModelConfig> {
        CONFIGS.get(name).cloned()
    }

    pub fn get_builtin(name: &str) -> Option<&'static ModelConfig> {
        CONFIGS.get(name)
    }

    /// Look up a built-in model config by its model_id field.
    ///
    /// This searches all built-in configs' `model_id` values, unlike `get_builtin()`
    /// which looks up by the config key name.
    /// Use this when you have a model ID like "glm-ocr:bf16" instead of
    /// the config key "glm-ocr".
    pub fn get_builtin_by_model_id(model_id: &str) -> Option<&'static ModelConfig> {
        CONFIGS.values().find(|mc| mc.model_id == model_id)
    }

    #[allow(dead_code)]
    pub fn get_default() -> ModelConfig {
        CONFIGS.get(DEFAULT_MODEL).cloned().unwrap()
    }

    #[allow(dead_code)]
    pub fn list_names() -> Vec<&'static str> {
        Self::list_builtin_names()
    }

    pub fn list_builtin_names() -> Vec<&'static str> {
        vec!["qwen3.5:4b", "translategemma", "glm-ocr"]
    }

    #[allow(dead_code)]
    pub fn is_valid(name: &str) -> bool {
        CONFIGS.contains_key(name)
    }

    pub fn is_builtin_valid(name: &str) -> bool {
        CONFIGS.contains_key(name)
    }
}

impl ModelConfig {
    pub fn build_model_options(&self) -> ModelOptions {
        let mut opts = ModelOptions::default()
            .temperature(self.temperature)
            .repeat_penalty(self.repeat_penalty.unwrap_or(1.1));

        if self.num_ctx > 0 {
            opts = opts.num_ctx(self.num_ctx as u64);
        }

        if let Some(top_k) = self.top_k {
            opts = opts.top_k(top_k);
        }

        if let Some(top_p) = self.top_p {
            opts = opts.top_p(top_p);
        }

        opts
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
            assert!(ModelConfig::is_valid(name), "Model {} should exist", name);
            assert!(
                ModelConfig::get(name).is_some(),
                "Model {} should be retrievable",
                name
            );
        }
    }

    #[test]
    fn test_invalid_model() {
        assert!(!ModelConfig::is_valid("nonexistent"));
        assert!(ModelConfig::get("nonexistent").is_none());
    }

    #[test]
    fn test_qwen35_4b_parameters() {
        let qwen = ModelConfig::get("qwen3.5:4b").unwrap();
        assert_eq!(qwen.model_id, "qwen3.5:4b");
        assert_eq!(qwen.num_ctx, 131072);
        assert_eq!(qwen.temperature, 1.0);
        assert_eq!(qwen.top_k, Some(20));
        assert_eq!(qwen.top_p, Some(0.95));
        assert_eq!(qwen.repeat_penalty, Some(1.0));
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
        // Lookup by config key should work
        let by_key = ModelConfig::get_builtin("glm-ocr");
        assert!(by_key.is_some());
        assert_eq!(by_key.unwrap().model_id, "glm-ocr:bf16");

        // Lookup by model_id should also work (different string than config key)
        let by_id = ModelConfig::get_builtin_by_model_id("glm-ocr:bf16");
        assert!(by_id.is_some());
        assert_eq!(by_id.unwrap().model_id, "glm-ocr:bf16");
        assert_eq!(by_id.unwrap().temperature, 0.1);

        // Config key lookup won't find model_id
        let by_wrong_key = ModelConfig::get_builtin("glm-ocr:bf16");
        assert!(by_wrong_key.is_none());

        // model_id lookup for default model (same as config key)
        let qwen_by_id = ModelConfig::get_builtin_by_model_id("qwen3.5:4b");
        assert!(qwen_by_id.is_some());
        assert_eq!(qwen_by_id.unwrap().temperature, 1.0);

        // Nonexistent model_id
        assert!(ModelConfig::get_builtin_by_model_id("nonexistent:7b").is_none());
    }
}
