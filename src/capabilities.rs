//! Model capability detection via the LLM server API
//!
//! Capabilities (tools, vision, thinking) are detected at runtime
//! by querying the server's model info endpoint.

#![expect(clippy::print_stderr)] // Model capability detection output
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

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            tools: false,
            vision: false,
            completion: true,
            thinking: false,
        }
    }
}

impl ModelCapabilities {
    /// Detect model capabilities by querying the LLM server
    ///
    /// # Arguments
    /// * `ollama` - The LLM server client instance
    /// * `model_name` - The name of the model to check (e.g., "qwen3.5:4b")
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

    /// Detect model capabilities or return defaults on error
    ///
    /// Prints a warning on detection failure and returns default capabilities
    /// with completion enabled (safe fallback for most operations).
    pub async fn detect_or_default(ollama: &Ollama, model_name: &str) -> Self {
        match Self::detect(ollama, model_name).await {
            Ok(caps) => caps,
            Err(e) => {
                eprintln!("Warning: Could not detect model capabilities: {}", e);
                eprintln!("Continuing without capability detection...");
                Self::default()
            }
        }
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
