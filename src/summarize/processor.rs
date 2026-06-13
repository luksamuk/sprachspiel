//! Summarize processor
//!
//! Handles text summarization respecting config.toml model settings with tools disabled.
//! Ensures security and efficiency by not allowing tool calls during summarization.

use ollama_rs::generation::chat::ChatMessage;

use crate::chat::CustomCoordinator;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};

use super::cli::SummarizeArgs;

/// Summarization processor
pub struct SummarizeProcessor;

impl SummarizeProcessor {
    /// Create a new summarize processor
    pub fn new() -> Self {
        Self
    }

    /// Process summarization request
    pub async fn summarize(
        &self,
        args: &SummarizeArgs,
        text: &str,
        model_id: &str,
        settings: &Settings,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if text.is_empty() {
            return Err("No text provided for summarization".into());
        }

        // Bail-out: detect broken config before reaching resolve_model_config's
        // process::exit(1). Per PR #206 review: failing silently with "default"
        // or generic "Unknown model" masks user configuration errors.
        crate::user_models::require_providers()?;

        let model_config = crate::user_models::resolve_model_config(model_id);

        // Initialize Ollama with settings
        #[allow(deprecated)] // ollama_client() removed in #121 (Consumer Migration)
        let ollama = settings.ollama_client_for_model(model_id);

        let provider_options = model_config.build_provider_options();
        // W2 #121: bridge to legacy ModelOptions for CustomCoordinator.
        let model_options = crate::chat::core::convert_provider_to_model(&provider_options);

        // Build coordinator WITHOUT tools (security requirement)
        let mut coordinator = CustomCoordinator::new(ollama, model_config.model_id.clone(), vec![])
            .options(model_options);
        // Note: No .add_tool() calls - tools are disabled

        // Build system prompt (no Pepe personality for summarize - keep it professional)
        let base_prompt = build_system_prompt(
            PromptConfig::new(PromptType::Summarize)
                .with_model_id(Some(&model_config.model_id))
                .with_retrieval(false),
        );
        let system_prompt = args.build_prompt(&base_prompt);

        // Create messages
        let system_message = ChatMessage::system(system_prompt);
        let user_message = ChatMessage::user(text.to_string());

        // Show spinner
        let spinner = create_spinner("Summarizing...");

        // Send request
        let response = coordinator
            .chat(vec![system_message, user_message])
            .await
            .map_err(|e| format!("Failed to summarize: {}", e))?;

        // Clear spinner
        finish_spinner(spinner);

        let content = response.message.content.trim().to_string();

        Ok(content)
    }
}

impl Default for SummarizeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let _processor = SummarizeProcessor::new();
    }
}
