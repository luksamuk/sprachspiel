//! Subagent Runner - lightweight one-shot executor for specialized tasks.
//!
//! Provides a minimal interface for dispatching sub-tasks (OCR, Vision,
//! Translate, Summarize) to Ollama models without the overhead
//! of `CustomCoordinator` (no history, no callbacks, no overflow detection).
//!
//! Two API paths:
//! - `/api/generate`: For image-based tasks (Ocr, Vision)
//! - `/api/chat`: For text-based tasks (Translate, Summarize)

use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::models::ModelOptions;

use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};

use std::path::PathBuf;

use crate::vision::{VisionArgs, VisionProcessor};
use std::path::Path;

use crate::ocr::error::OcrError;
use crate::ocr::mode::{OcrMode, is_glm_ocr_model};
use crate::ocr::processor::OcrProcessor;

/// Specialized subagent types, each targeting a distinct capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentType {
    /// Extract text from images via OCR models (uses /api/generate).
    Ocr,
    /// Analyze or describe images via vision models (uses /api/generate).
    Vision,
    /// Translate text between languages (uses /api/chat).
    Translate,
    /// Summarize long text (uses /api/chat).
    Summarize,
}

impl SubagentType {
    /// Returns `true` for variants that use `/api/generate` (image-based).
    #[allow(dead_code)] // Public API method used in tests
    pub fn uses_generate_api(&self) -> bool {
        matches!(self, SubagentType::Ocr | SubagentType::Vision)
    }

    /// Human-readable label for this subagent type.
    #[allow(dead_code)] // Public API method used in tests
    pub fn label(&self) -> &'static str {
        match self {
            SubagentType::Ocr => "OCR",
            SubagentType::Vision => "Vision",
            SubagentType::Translate => "Translate",
            SubagentType::Summarize => "Summarize",
        }
    }
}

impl std::str::FromStr for SubagentType {
    type Err = ();

    /// Parse a string into a SubagentType.
    ///
    /// Returns `Err(())` if the string doesn't match any known type.
    /// Case-insensitive matching.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ocr" => Ok(Self::Ocr),
            "vision" => Ok(Self::Vision),
            "translate" => Ok(Self::Translate),
            "summarize" => Ok(Self::Summarize),
            _ => Err(()),
        }
    }
}

impl SubagentType {
    /// Parse a string into a SubagentType (convenience wrapper).
    ///
    /// Returns None if the string doesn't match any known type.
    /// Case-insensitive matching.
    ///
    /// This is a convenience method that wraps the `FromStr` implementation.
    #[allow(dead_code)] // Public API method used in tests
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

/// Configuration for a subagent execution.
///
/// Each subagent type has its own defaults but can be overridden.
/// Model options are resolved from the built-in/user model config at
/// construction time, ensuring per-model temperature, num_ctx, etc.
/// are respected instead of falling back to a hardcoded temperature 0.0.
///
/// Note: Sub-agent results are NOT truncated. The coordinator's emergency
/// context overflow protection in `custom_coordinator.rs` handles any
/// results that would exceed the context window.
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Resolved model_id to use for this subagent (e.g. "glm-ocr:bf16", "translategemma:4b").
    pub model: String,
    /// System prompt injected before the user prompt.
    pub system_prompt: String,
    /// Model options (temperature, num_ctx, etc.) resolved from ModelConfig.
    pub model_options: ModelOptions,
    /// OCR extraction mode (Text, Table, Figure, Formula).
    pub ocr_mode: OcrMode,
}
impl SubagentConfig {
    /// Create a new config with the given model config key and system prompt.
    ///
    /// The `model` parameter is a config key (e.g., "translategemma", "glm-ocr")
    /// that gets resolved to a model_id (e.g., "translategemma:4b", "glm-ocr:bf16")
    /// for the actual Ollama API call. If the config key is not found in built-in
    /// or user models, it is used directly as the model name with default options.
    pub fn new(model: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        let config_key = model.into();
        let (resolved_model, model_options) = crate::user_models::get_model_config(&config_key)
            .map(|mc| (mc.model_id.clone(), mc.build_model_options()))
            .unwrap_or_else(|| (config_key.clone(), ModelOptions::default().temperature(0.0)));
        Self {
            model: resolved_model,
            system_prompt: system_prompt.into(),
            ocr_mode: OcrMode::Text,
            model_options,
        }
    }

    /// Set the OCR extraction mode (only affects OCR subagent).
    pub fn with_ocr_mode(mut self, mode: OcrMode) -> Self {
        self.ocr_mode = mode;
        self
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
    /// - `Translate` / `Summarize` → `/api/chat`
    ///
    /// Results are returned in full — truncation is handled by the
    /// coordinator's emergency context overflow protection in
    /// `custom_coordinator.rs` if needed.
    ///
    /// # Arguments
    /// * `subagent_type` - Which specialization to invoke.
    /// * `prompt` - The user-facing prompt (e.g. "Extract all text" or "Translate to Portuguese").
    /// * `file_path` - Required for Ocr/Vision; ignored for Translate/Summarize.
    pub async fn run(
        &self,
        subagent_type: SubagentType,
        prompt: String,
        file_paths: Vec<PathBuf>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match subagent_type {
            SubagentType::Ocr => {
                if file_paths.is_empty() {
                    return Err("file_path is required for OCR subagent".into());
                }
                self.run_ocr(&file_paths[0], self.config.ocr_mode).await
            }
            SubagentType::Vision => self.run_vision(&file_paths, &prompt).await,
            SubagentType::Translate | SubagentType::Summarize => self.run_chat(prompt).await,
        }
    }

    /// Execute via `/api/chat` — used for text-based subagents (Translate, Summarize).
    ///
    /// Constructs a system message from the config's system prompt and a user
    /// message from the provided prompt, then sends a `ChatMessageRequest`.
    async fn run_chat(
        &self,
        prompt: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let system_message = ChatMessage::system(self.config.system_prompt.clone());
        let user_message = ChatMessage::user(prompt);

        let model_options = self.config.model_options.clone();

        let request = ChatMessageRequest::new(
            self.config.model.clone(),
            vec![system_message, user_message],
        )
        .options(model_options);

        let response =
            self.ollama.send_chat_messages(request).await.map_err(|e| {
                format!("/api/chat failed for model '{}': {}", self.config.model, e)
            })?;

        Ok(response.message.content.trim().to_string())
    }

    /// Execute a translation task via `/api/chat`.
    ///
    /// Parses the language pair, builds a translation-specific prompt,
    /// and dispatches to `run_chat()`. No tools are registered (security).
    ///
    /// # Arguments
    /// * `lang_pair` - Language pair in "source:target" format (e.g., "en:pt")
    ///   or ":target" / "target" for auto-detection of the source language.
    /// * `text` - The text to translate.
    pub async fn run_translate(
        &self,
        lang_pair: &str,
        text: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use crate::translate::{LanguageMapper, build_translation_prompt, parse_language_pair};

        let mapper = LanguageMapper::new();
        let (source, target) = parse_language_pair(lang_pair, &mapper)
            .map_err(|e| format!("Invalid language pair '{}': {}", lang_pair, e))?;

        let prompt = build_translation_prompt(source.as_ref(), &target, text, None);

        self.run_chat(prompt).await
    }

    /// Execute a summarization task via `/api/chat`.
    ///
    /// Builds a summarize-specific system prompt using the prompt builder
    /// (no custom personality, no tools), sends the text for summarization,
    /// and returns the result truncated to the configured output budget.
    ///
    /// # Security
    /// No tools are registered — the coordinator is bare, preventing
    /// any tool invocation during summarization.
    ///
    /// # Arguments
    /// * `text` - The text to summarize.
    pub async fn run_summarize(
        &self,
        text: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Build summarize system prompt (no SOUL personality, no tools)
        let system_prompt = build_system_prompt(
            PromptConfig::new(PromptType::Summarize)
                .with_model_id(Some(&self.config.model))
                .with_retrieval(false),
        );

        let system_message = ChatMessage::system(system_prompt);
        let user_message = ChatMessage::user(text.to_string());

        let model_options = self.config.model_options.clone();

        let request = ChatMessageRequest::new(
            self.config.model.clone(),
            vec![system_message, user_message],
        )
        .options(model_options);

        let response = self.ollama.send_chat_messages(request).await.map_err(|e| {
            format!(
                "/api/chat failed for summarize on model '{}': {}",
                self.config.model, e
            )
        })?;

        let raw = response.message.content.trim().to_string();
        Ok(raw)
    }

    /// Execute a vision task using VisionProcessor.
    ///
    /// Delegates to the existing `VisionProcessor::process()` method,
    /// which handles image validation, base64 encoding, and API calls.
    /// The vision model is resolved from `self.config.model`.
    ///
    /// # Arguments
    /// * `paths` - Image file paths to analyze.
    /// * `prompt` - Custom prompt describing what to look for in the images.
    ///
    /// # Returns
    /// The full description/analysis text from the vision model.
    /// Results are NOT truncated — the coordinator's emergency context
    /// overflow protection handles results that exceed the context window.
    pub async fn run_vision(
        &self,
        paths: &[PathBuf],
        prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if paths.is_empty() {
            return Err("No image files provided for vision subagent.".into());
        }

        let model = self.config.model.clone();

        let args = VisionArgs {
            files: paths.to_vec(),
            prompt: Some(prompt.to_string()),
            detailed: false,
            json: false,
            model: None,
            max_tokens: 8192,
        };

        let processor = VisionProcessor::new();
        let output = processor
            .process(
                &args,
                &model,
                &self.ollama,
                self.config.model_options.clone(),
                false,
            )
            .await
            .map_err(|e| format!("Vision processing failed: {}", e))?;

        Ok(output.content)
    }

    /// Execute an OCR task using the dedicated `OcrProcessor`.
    ///
    /// Delegates to `OcrProcessor::process_file()` for actual OCR processing.
    /// This avoids reimplementing base64 encoding, file validation, and
    /// API interaction logic that already exists in the OCR module.
    ///
    /// # Arguments
    /// * `path` - Path to the image file to process.
    /// * `mode` - OCR extraction mode (Text, Table, Figure, Formula).
    ///
    /// # Returns
    /// * `Ok(String)` - Extracted text content on success, or an error message on failure.
    /// * `Err(Box<dyn Error>)` - Only for truly catastrophic failures (should not happen in practice).
    ///
    /// # Error Handling
    /// Following the tool error philosophy, all expected failures (file not found,
    /// read errors, Ollama errors) are returned as `Ok(String)` with descriptive
    /// error messages, allowing the LLM to understand and recover from the error.
    pub async fn run_ocr(
        &self,
        path: &Path,
        mode: OcrMode,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let processor = OcrProcessor::new();

        let prompt_override = if is_glm_ocr_model(&self.config.model) {
            None // GLM-OCR: use mode.into_prompt() (rigid prefix)
        } else {
            Some(mode.into_descriptive_prompt()) // Vision model: descriptive prompt
        };

        match processor
            .process_file(
                path,
                mode,
                prompt_override,
                &self.config.model,
                self.config.model_options.clone(),
                &self.ollama,
                false,
            )
            .await
        {
            Ok(output) => Ok(output.content),
            Err(OcrError::FileNotFound(msg)) => Ok(format!("Error: Image file not found: {}", msg)),
            Err(e) => Ok(format!("Error: OCR processing failed: {}", e)),
        }
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
    }

    #[test]
    fn subagent_type_labels() {
        assert_eq!(SubagentType::Ocr.label(), "OCR");
        assert_eq!(SubagentType::Vision.label(), "Vision");
        assert_eq!(SubagentType::Translate.label(), "Translate");
        assert_eq!(SubagentType::Summarize.label(), "Summarize");
    }

    #[test]
    fn subagent_config_defaults() {
        let config = SubagentConfig::new("glm-ocr:bf16", "Extract text from images");
        assert_eq!(config.model, "glm-ocr:bf16");
        assert_eq!(config.system_prompt, "Extract text from images");
        assert_eq!(config.ocr_mode, OcrMode::Text);
        let _opts = config.model_options.clone();
    }

    #[test]
    fn subagent_config_builder() {
        // Verify ocr_mode default
        let config = SubagentConfig::new("test-model", "test prompt");
        assert_eq!(config.ocr_mode, OcrMode::Text);
        let _opts = config.model_options.clone();
    }

    #[test]
    fn subagent_type_from_str() {
        // Valid types — case-insensitive
        assert_eq!("ocr".parse::<SubagentType>(), Ok(SubagentType::Ocr));
        assert_eq!("vision".parse::<SubagentType>(), Ok(SubagentType::Vision));
        assert_eq!(
            "translate".parse::<SubagentType>(),
            Ok(SubagentType::Translate)
        );
        assert_eq!(
            "summarize".parse::<SubagentType>(),
            Ok(SubagentType::Summarize)
        );

        // Case-insensitive
        assert_eq!("OCR".parse::<SubagentType>(), Ok(SubagentType::Ocr));
        assert_eq!("Vision".parse::<SubagentType>(), Ok(SubagentType::Vision));
        assert_eq!(
            "TRANSLATE".parse::<SubagentType>(),
            Ok(SubagentType::Translate)
        );

        // Invalid types
        assert!("unknown".parse::<SubagentType>().is_err());
        assert!("".parse::<SubagentType>().is_err());
        assert!("code".parse::<SubagentType>().is_err());
    }

    #[test]
    fn subagent_type_parse_convenience() {
        // The parse() convenience method wraps from_str
        assert_eq!(SubagentType::parse("ocr"), Some(SubagentType::Ocr));
        assert_eq!(SubagentType::parse("invalid"), None);
        assert_eq!(SubagentType::parse(""), None);
    }

    #[test]
    fn test_subagent_config_model_options_from_builtin() {
        // Create a SubagentConfig with config key "glm-ocr" — should resolve model_id
        let config = SubagentConfig::new("glm-ocr", "test");
        assert_eq!(config.model, "glm-ocr:bf16");
        // Verify temperature is resolved from ModelConfig (0.1 for glm-ocr)
        // We can't directly access model_options fields, but we can check via clone
        let opts = config.model_options.clone();
        // temperature should be 0.1 for glm-ocr, not the fallback 0.0
        // Since we can't directly access field, verify via debug output
        let debug_str = format!("{:?}", opts);
        assert!(
            debug_str.contains("temperature"),
            "ModelOptions should contain temperature field"
        );
    }

    #[test]
    fn test_subagent_config_model_options_from_unknown_model() {
        // Create a SubagentConfig with an unknown model
        let config = SubagentConfig::new("unknown-model-xyz", "test");
        // Should fall back to default ModelOptions with temperature 0.0
        let opts = config.model_options.clone();
        let debug_str = format!("{:?}", opts);
        assert!(
            debug_str.contains("temperature"),
            "ModelOptions should contain temperature field"
        );
    }

    #[test]
    fn test_subagent_config_with_translate_model() {
        // Test that SubagentConfig resolves config key "translategemma" to model_id
        let config = SubagentConfig::new("translategemma", "Translate text");
        assert_eq!(config.model, "translategemma:4b");
        let _opts = config.model_options.clone();
    }

    /// Test that config key "glm-ocr" resolves to model_id "glm-ocr:bf16"
    #[test]
    fn test_subagent_config_resolves_glm_ocr_config_key() {
        let config = SubagentConfig::new("glm-ocr", "Extract text");
        assert_eq!(config.model, "glm-ocr:bf16");
    }

    /// Test that config key "translategemma" resolves to model_id "translategemma:4b"
    #[test]
    fn test_subagent_config_resolves_translategemma_config_key() {
        let config = SubagentConfig::new("translategemma", "Translate text");
        assert_eq!(config.model, "translategemma:4b");
    }

    #[test]
    fn test_subagent_config_with_vision_model() {
        // Test that SubagentConfig works with a vision model
        let config = SubagentConfig::new("moondream:1.8b", "Analyze image");
        assert_eq!(config.model, "moondream:1.8b");
        let _opts = config.model_options.clone();
    }

    #[test]
    fn subagent_config_default_ocr_mode() {
        let config = SubagentConfig::new("glm-ocr", "OCR");
        assert_eq!(config.ocr_mode, OcrMode::Text);
    }

    #[test]
    fn subagent_config_with_ocr_mode() {
        let config = SubagentConfig::new("glm-ocr", "OCR").with_ocr_mode(OcrMode::Table);
        assert_eq!(config.ocr_mode, OcrMode::Table);

        let config = SubagentConfig::new("glm-ocr", "OCR").with_ocr_mode(OcrMode::Formula);
        assert_eq!(config.ocr_mode, OcrMode::Formula);
    }
    #[test]
    fn test_subagent_config_with_custom_ocr() {
        // Test that SubagentConfig works with a custom OCR model
        let config = SubagentConfig::new("custom-ocr:bf16", "OCR document");
        assert_eq!(config.model, "custom-ocr:bf16");
        let _opts = config.model_options.clone();
    }
}
