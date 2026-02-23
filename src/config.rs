use std::collections::HashMap;
use std::sync::LazyLock;

pub const DEFAULT_MODEL: &str = "llama3.1";

static CONFIGS: LazyLock<HashMap<&'static str, ModelConfig>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    // Default model for general queries
    configs.insert(
        "llama3.1",
        ModelConfig {
            model_id: "llama3.1:8b".to_string(),
            num_ctx: 4096,
            temperature: 0.8,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.1),
            thinking: false,
        },
    );

    // Translation model (used by translate command)
    configs.insert(
        "translategemma",
        ModelConfig {
            model_id: "translategemma:12b".to_string(),
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

    #[allow(dead_code)]
    pub fn get_default() -> ModelConfig {
        CONFIGS.get(DEFAULT_MODEL).cloned().unwrap()
    }

    #[allow(dead_code)]
    pub fn list_names() -> Vec<&'static str> {
        Self::list_builtin_names()
    }

    pub fn list_builtin_names() -> Vec<&'static str> {
        vec!["llama3.1", "translategemma", "glm-ocr"]
    }

    #[allow(dead_code)]
    pub fn is_valid(name: &str) -> bool {
        CONFIGS.contains_key(name)
    }

    pub fn is_builtin_valid(name: &str) -> bool {
        CONFIGS.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_is_llama31() {
        let default = ModelConfig::get_default();
        assert_eq!(default.model_id, "llama3.1:8b");
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
    fn test_llama31_parameters() {
        let llama = ModelConfig::get("llama3.1").unwrap();
        assert_eq!(llama.model_id, "llama3.1:8b");
        assert_eq!(llama.temperature, 0.8);
        assert_eq!(llama.top_k, None);
        assert_eq!(llama.top_p, None);
        assert_eq!(llama.repeat_penalty, Some(1.1));
    }
}
