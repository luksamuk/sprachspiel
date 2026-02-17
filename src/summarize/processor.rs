//! Summarize processor
//!
//! Handles text summarization using mistral-small model with tools disabled.
//! Ensures security and efficiency by not allowing tool calls during summarization.

use ollama_rs::Ollama;
use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;

use crate::config::ModelConfig;
use crate::prompts::{get_prompt, SYSTEM_PROMPT_SUMMARIZE};
use crate::spinner::create_spinner;

use super::cli::SummarizeArgs;

/// Summarization processor
pub struct SummarizeProcessor;

impl SummarizeProcessor {
    /// Create a new summarize processor
    pub fn new() -> Self {
        Self
    }

    /// Process summarization request
    pub async fn summarize(&self, args: &SummarizeArgs, text: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if text.is_empty() {
            return Err("No text provided for summarization".into());
        }

        // Get llama3.2 model config (default for summarization)
        let model_config = ModelConfig::get("llama3.2")
            .unwrap_or_else(|| ModelConfig::get("mistral-small").unwrap_or_else(|| ModelConfig::get("lfm").expect("Default model should exist")));

        // Initialize Ollama
        let ollama = Ollama::default();

        // Build model options
        let model_options = ModelOptions::default()
            .temperature(model_config.temperature)
            .top_p(model_config.top_p)
            .top_k(model_config.top_k)
            .num_ctx(model_config.num_ctx as u64)
            .repeat_penalty(model_config.repeat_penalty);

        // Build coordinator WITHOUT tools (security requirement)
        let mut coordinator = Coordinator::new(ollama, model_config.model_id.clone(), vec![])
            .options(model_options);
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
        spinner.finish_and_clear();

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
        let processor = SummarizeProcessor::new();
        // Just verify it creates successfully
    }
}
