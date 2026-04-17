#![allow(dead_code)]
//! Subagent Runner - lightweight one-shot executor for specialized tasks.
//!
//! Provides a minimal interface for dispatching sub-tasks (OCR, Vision,
//! Translate, Summarize, Document) to Ollama models without the overhead
//! of `CustomCoordinator` (no history, no callbacks, no overflow detection).
//!
//! Two API paths:
//! - `/api/generate`: For image-based tasks (Ocr, Vision)
//! - `/api/chat`: For text-based tasks (Translate, Summarize, Document)

use base64::Engine;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::images::Image;
use ollama_rs::models::ModelOptions;
use ollama_rs::Ollama;

use crate::utils::truncate_to_budget;

/// Default maximum output length in tokens for subagent results.
const DEFAULT_MAX_OUTPUT_TOKENS: usize = 10_000;

/// Specialized subagent types, each targeting a distinct capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentType {
    /// Extract text from images via OCR models (uses /api/generate).
    Ocr,
    /// Analyze or describe images via vision models (uses /api/generate).
    Vision,
    /// Translate text between languages (uses /api/chat).
    Translate,
    /// Summarize long text (uses /api/chat).
    Summarize,
    /// Process structured documents (uses /api/chat, may use tools).
    Document,
}

impl SubagentType {
    /// Returns `true` for variants that use `/api/generate` (image-based).
    pub fn uses_generate_api(&self) -> bool {
        matches!(self, SubagentType::Ocr | SubagentType::Vision)
    }

    /// Returns `true` for variants that use `/api/chat` (text-based).
    pub fn uses_chat_api(&self) -> bool {
        !self.uses_generate_api()
    }

    /// Human-readable label for this subagent type.
    pub fn label(&self) -> &'static str {
        match self {
            SubagentType::Ocr => "OCR",
            SubagentType::Vision => "Vision",
            SubagentType::Translate => "Translate",
            SubagentType::Summarize => "Summarize",
            SubagentType::Document => "Document",
        }
    }
}

/// Configuration for a subagent execution.
///
/// Each subagent type has its own defaults but can be overridden.
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Model name to use for this subagent (e.g. "glm-ocr:bf16").
    pub model: String,
    /// System prompt injected before the user prompt.
    pub system_prompt: String,
    /// Tool names allowed for this subagent (only relevant for Document).
    pub tool_whitelist: Vec<String>,
    /// Maximum output tokens; results are truncated beyond this.
    pub max_output_chars: usize,
}

impl SubagentConfig {
    /// Create a new config with the given model and system prompt.
    pub fn new(model: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            system_prompt: system_prompt.into(),
            tool_whitelist: Vec::new(),
            max_output_chars: DEFAULT_MAX_OUTPUT_TOKENS,
        }
    }

    /// Set the tool whitelist (only affects Document subagent).
    pub fn with_tool_whitelist(mut self, tools: Vec<String>) -> Self {
        self.tool_whitelist = tools;
        self
    }

    /// Override the default maximum output token budget.
    pub fn with_max_output_chars(mut self, max: usize) -> Self {
        self.max_output_chars = max;
        self
    }

    /// Build default `ModelOptions` for this config.
    pub fn default_model_options(&self) -> ModelOptions {
        ModelOptions::default().temperature(0.0)
    }
}

/// Lightweight one-shot executor for specialized sub-tasks.
///
/// Unlike `CustomCoordinator` (992 lines with history, callbacks, overflow
/// detection, continuation tags), `SubagentRunner` is intentionally minimal:
/// just an Ollama client, a config, and a `run()` method.
pub struct SubagentRunner {
    ollama: Ollama,
    config: SubagentConfig,
}

impl SubagentRunner {
    /// Create a new runner with the given Ollama client and config.
    pub fn new(ollama: Ollama, config: SubagentConfig) -> Self {
        Self { ollama, config }
    }

    /// Execute a subagent task.
    ///
    /// Dispatches to the correct Ollama API based on `subagent_type`:
    /// - `Ocr` / `Vision` → `/api/generate` (with image from `file_path`)
    /// - `Translate` / `Summarize` / `Document` → `/api/chat`
    ///
    /// Results are truncated at `config.max_output_chars` tokens via
    /// `truncate_to_budget()`.
    ///
    /// # Arguments
    /// * `subagent_type` - Which specialization to invoke.
    /// * `prompt` - The user-facing prompt (e.g. "Extract all text" or "Translate to Portuguese").
    /// * `file_path` - Required for Ocr/Vision; optional for Document; ignored for Translate/Summarize.
    pub async fn run(
        &self,
        subagent_type: SubagentType,
        prompt: String,
        file_path: Option<String>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let raw = match subagent_type {
            SubagentType::Ocr | SubagentType::Vision => {
                self.run_generate(prompt, file_path).await?
            }
            SubagentType::Translate | SubagentType::Summarize | SubagentType::Document => {
                self.run_chat(prompt).await?
            }
        };

        Ok(truncate_to_budget(&raw, self.config.max_output_chars))
    }

    /// Execute via `/api/generate` — used for image-based subagents (Ocr, Vision).
    ///
    /// Reads the file at `file_path`, base64-encodes it, attaches it as an
    /// image to a `GenerationRequest`, and returns the model's response text.
    async fn run_generate(
        &self,
        prompt: String,
        file_path: Option<String>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let path = file_path.ok_or_else(|| {
            format!(
                "Error: file_path is required for {} subagent",
                self.config.model
            )
        })?;

        // Read and encode the image file
        let image_bytes = tokio::fs::read(&path).await.map_err(|e| {
            format!("Error: Failed to read image file '{}': {}", path, e)
        })?;
        let base64_image =
            base64::engine::general_purpose::STANDARD.encode(&image_bytes);
        let image = Image::from_base64(base64_image);

        let model_options = self.config.default_model_options();

        let request = GenerationRequest::new(self.config.model.clone(), prompt)
            .options(model_options)
            .add_image(image);

        let response = self.ollama.generate(request).await.map_err(|e| {
            format!(
                "Error: /api/generate failed for model '{}': {}",
                self.config.model, e
            )
        })?;

        Ok(response.response.trim().to_string())
    }

    /// Execute via `/api/chat` — used for text-based subagents (Translate, Summarize, Document).
    ///
    /// Constructs a system message from the config's system prompt and a user
    /// message from the provided prompt, then sends a `ChatMessageRequest`.
    async fn run_chat(
        &self,
        prompt: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let system_message = ChatMessage::system(self.config.system_prompt.clone());
        let user_message = ChatMessage::user(prompt);

        let model_options = self.config.default_model_options();

        let request = ChatMessageRequest::new(
            self.config.model.clone(),
            vec![system_message, user_message],
        )
        .options(model_options);

        let response = self
            .ollama
            .send_chat_messages(request)
            .await
            .map_err(|e| {
                format!(
                    "Error: /api/chat failed for model '{}': {}",
                    self.config.model, e
                )
            })?;

        Ok(response.message.content.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagent_type_uses_generate_api() {
        assert!(SubagentType::Ocr.uses_generate_api());
        assert!(SubagentType::Vision.uses_generate_api());
        assert!(!SubagentType::Translate.uses_generate_api());
        assert!(!SubagentType::Summarize.uses_generate_api());
        assert!(!SubagentType::Document.uses_generate_api());
    }

    #[test]
    fn subagent_type_uses_chat_api() {
        assert!(!SubagentType::Ocr.uses_chat_api());
        assert!(!SubagentType::Vision.uses_chat_api());
        assert!(SubagentType::Translate.uses_chat_api());
        assert!(SubagentType::Summarize.uses_chat_api());
        assert!(SubagentType::Document.uses_chat_api());
    }

    #[test]
    fn subagent_type_labels() {
        assert_eq!(SubagentType::Ocr.label(), "OCR");
        assert_eq!(SubagentType::Vision.label(), "Vision");
        assert_eq!(SubagentType::Translate.label(), "Translate");
        assert_eq!(SubagentType::Summarize.label(), "Summarize");
        assert_eq!(SubagentType::Document.label(), "Document");
    }

    #[test]
    fn subagent_config_defaults() {
        let config =
            SubagentConfig::new("glm-ocr:bf16", "Extract text from images");
        assert_eq!(config.model, "glm-ocr:bf16");
        assert_eq!(config.system_prompt, "Extract text from images");
        assert!(config.tool_whitelist.is_empty());
        assert_eq!(config.max_output_chars, DEFAULT_MAX_OUTPUT_TOKENS);
    }

    #[test]
    fn subagent_config_builder() {
        let config = SubagentConfig::new("test-model", "test prompt")
            .with_tool_whitelist(vec!["run_command".to_string()])
            .with_max_output_chars(5000);
        assert_eq!(config.tool_whitelist, vec!["run_command"]);
        assert_eq!(config.max_output_chars, 5000);
    }
}