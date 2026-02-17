//! Model capability detection via Ollama API
//!
//! This module provides runtime detection of model capabilities (tools, vision, thinking)
//! by querying the Ollama API's show_model_info endpoint.

use ollama_rs::Ollama;
use ollama_rs::models::ModelInfo;

/// Detected capabilities for a specific model
#[derive(Debug, Clone)]
pub struct ModelCapabilities {
    pub tools: bool,
    pub vision: bool,
    pub completion: bool,
    pub thinking: bool,
}

impl ModelCapabilities {
    /// Detect model capabilities by querying Ollama API
    ///
    /// # Arguments
    /// * `ollama` - The Ollama client instance
    /// * `model_name` - The name of the model to check (e.g., "llama3.2:3b-32k")
    ///
    /// # Returns
    /// Detected capabilities for the model
    pub async fn detect(ollama: &Ollama, model_name: &str) -> crate::AppResult<Self> {
        let info: ModelInfo = ollama
            .show_model_info(model_name.to_string())
            .await
            .map_err(|e| format!("Failed to query model info: {}", e))?;

        Ok(Self {
            tools: info.capabilities.contains(&"tools".to_string()),
            vision: info.capabilities.contains(&"vision".to_string()),
            completion: info.capabilities.contains(&"completion".to_string()),
            thinking: info.capabilities.contains(&"thinking".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_detection() {
        let caps = ModelCapabilities {
            tools: true,
            vision: false,
            completion: true,
            thinking: false,
        };

        assert!(caps.tools);
        assert!(caps.completion);
        assert!(!caps.vision);
        assert!(!caps.thinking);
    }
}
