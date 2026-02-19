//! Summarize processor
//!
//! Handles text summarization using mistral-small model with tools disabled.
//! Ensures security and efficiency by not allowing tool calls during summarization.

use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;

use crate::prompts::{SYSTEM_PROMPT_SUMMARIZE, get_prompt};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};

use super::cli::SummarizeArgs;

fn build_model_options(config: &crate::config::ModelConfig) -> ModelOptions {
    let mut opts = ModelOptions::default()
        .temperature(config.temperature)
        .repeat_penalty(config.repeat_penalty.unwrap_or(1.1));

    if config.num_ctx > 0 {
        opts = opts.num_ctx(config.num_ctx as u64);
    }

    if let Some(top_k) = config.top_k {
        opts = opts.top_k(top_k);
    }

    if let Some(top_p) = config.top_p {
        opts = opts.top_p(top_p);
    }

    opts
}

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

        // Get model config (with fallback chain)
        let model_config = crate::user_models::get_model_config(model_id).unwrap_or_else(|| {
            crate::user_models::get_model_config("llama3.1")
                .expect("Default model should exist")
        });

        // Initialize Ollama with settings
        let ollama = settings.ollama_client();

        let model_options = build_model_options(&model_config);

        // Build coordinator WITHOUT tools (security requirement)
        let mut coordinator =
            Coordinator::new(ollama, model_config.model_id.clone(), vec![]).options(model_options);
        // Note: No .add_tool() calls - tools are disabled

        // Build system prompt (no Pepe personality for summarize - keep it professional)
        let base_prompt = get_prompt("summarize", Some(&model_config.model_id))
            .unwrap_or_else(|| SYSTEM_PROMPT_SUMMARIZE.to_string());
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
