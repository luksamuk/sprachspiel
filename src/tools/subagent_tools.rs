//! spawn_subagent tool - LLM-initiated subagent invocation
//!
//! Allows the LLM to delegate specialized tasks (OCR, Vision, Translation,
//! Summarization, Document processing) to purpose-built subagents.
//!
//! This tool creates a `SubagentRunner` with the appropriate model and
//! system prompt for the requested task type, then executes it and
//! returns the result.

use crate::chat::subagent::{SubagentConfig, SubagentRunner, SubagentType};
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::ocr::mode::parse_ocr_mode;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::security::validate_subagent_paths;
use crate::tools::context::{get_ollama, get_settings};
use crate::utils::expand_tilde_path;
use std::path::PathBuf;

/// Valid subagent type strings for error messages
const VALID_SUBAGENT_TYPES: &[&str] = &["ocr", "vision", "translate", "summarize", "document"];


const VISION_SYSTEM_PROMPT: &str = "You are a vision model. Analyze the image as instructed. \
    Describe what you see thoroughly and accurately. Output only your analysis.";

const TRANSLATE_SYSTEM_PROMPT: &str = "You are a translator. Translate the text as directed. \
    Preserve meaning, tone, and formatting. Output only the translation, no explanations.";

const DOCUMENT_SYSTEM_PROMPT: &str = "You are a document processor. Use the run_command tool to \
    extract text from the file. Follow instructions precisely. Output structured results.";

/// Spawn a specialized subagent for OCR, Vision, Translation, Summarization, or Document extraction.
///
/// Delegates a task to a purpose-built subagent model optimized for that
/// specific task type. The subagent runs independently and returns its result.
///
/// # Arguments
/// * `subagent_type` - **Required.** The type of subagent to invoke.
///   - `"ocr"` — Extract text from images (requires `file_path`)
///   - `"vision"` — Analyze or describe images (requires `file_path`, supports multiple)
///   - `"translate"` — Translate text between languages
///   - `"summarize"` — Summarize long text
///   - `"document"` — Process structured documents
///
/// * `prompt` - **Required.** The task description or text to process.
///   - For OCR: "Extract all text from this image" (file_path required)
///   - For Vision: "Describe what you see in this image" (file_path required, comma-separated for multiple)
///   - For Translate: The text to translate (or instructions like "Translate to Portuguese")
///   - For Summarize: The text to summarize
///   - For Document: Instructions for processing the document
///
/// * `file_path` - **Required for OCR/Vision.** Path(s) to the image file(s).
///   - For OCR: Single image path (e.g., `"/tmp/screenshot.png"`)
///   - For Vision: Comma-separated paths for multi-image analysis (e.g., `"img1.png,img2.jpg"`)
///   - Supports `~` home directory expansion
///   - Not required for Translate, Summarize, or Document types
///
/// * `ocr_mode` - **Optional.** OCR extraction mode. Only used when `subagent_type` is "ocr".
///   - `"text"` — General text recognition (default)
///   - `"table"` — Table structure extraction
///   - `"figure"` — Figure/diagram recognition
///   - `"formula"` — Mathematical formula extraction (LaTeX)
///   - If not specified, defaults to "text"
///   - For OCR: Single image path (e.g., `"/tmp/screenshot.png"`)
///   - For Vision: Comma-separated paths for multi-image analysis (e.g., `"img1.png,img2.jpg"`)
///   - Supports `~` home directory expansion
///   - Not required for Translate, Summarize, or Document types
///
/// # Returns
/// The subagent result as plain text, or an error message if the subagent fails.
///
/// # Errors
/// - Unknown subagent type → list valid types
/// - Missing file_path for OCR/Vision → explain it's required
/// - Subagent execution failure → error with details
///
/// # Example
/// ```ignore
/// spawn_subagent("ocr".to_string(), "Extract all text from this image".to_string(), Some("/tmp/document.png".to_string()), None)
/// spawn_subagent("ocr".to_string(), "Extract table structure".to_string(), Some("/tmp/table.png".to_string()), Some("table".to_string()))
/// spawn_subagent("vision".to_string(), "Describe these images".to_string(), Some("img1.png,img2.jpg".to_string()), None)
/// spawn_subagent("summarize".to_string(), "Summarize this long text...".to_string(), None, None)
/// spawn_subagent("vision".to_string(), "Describe these images".to_string(), Some("img1.png,img2.jpg".to_string()))
/// spawn_subagent("summarize".to_string(), "Summarize this long text...".to_string(), None)
/// ```
#[ollama_rs::function]
pub async fn spawn_subagent(
    subagent_type: String,
    prompt: String,
    file_path: Option<String>,
    ocr_mode: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None
    let file_path = file_path.filter(|s| !s.is_empty());
    let ocr_mode = ocr_mode.filter(|s| !s.is_empty());

    log_tool_call(
        "spawn_subagent",
        &[
            ("subagent_type".to_string(), subagent_type.clone()),
            ("prompt".to_string(), {
                let truncated: String = prompt.chars().take(80).collect();
                if prompt.chars().count() > 80 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            }),
            (
                "file_path".to_string(),
                file_path.clone().unwrap_or_else(|| "(none)".to_string()),
            ),
            ("ocr_mode".to_string(), ocr_mode.clone().unwrap_or_else(|| "(default)".to_string())),
        ],
    );

    // Validate subagent_type
    let agent_type = match SubagentType::parse(&subagent_type) {
        Some(t) => t,
        None => {
            let err = format!(
                "Error: Unknown subagent type '{}'. Valid types: {}",
                subagent_type,
                VALID_SUBAGENT_TYPES.join(", ")
            );
            log_tool_result("spawn_subagent", &err);
            return Ok(err);
        }
    };

    // Parse file paths based on subagent type
    // Vision supports comma-separated paths for multi-image analysis
    // OCR and other types use a single path
    let file_paths: Vec<PathBuf> = if agent_type.uses_generate_api() {
        match &file_path {
            Some(p) => {
                if agent_type == SubagentType::Vision {
                    // Vision: parse comma-separated paths (multi-image support)
                    let paths: Vec<PathBuf> = p
                        .split(',')
                        .map(|s| expand_tilde_path(s.trim()))
                        .filter(|p| !p.as_os_str().is_empty())
                        .collect();
                    if paths.is_empty() {
                        let err = "Error: No valid image paths provided.".to_string();
                        log_tool_result("spawn_subagent", &err);
                        return Ok(err);
                    }
                    paths
                } else {
                    // OCR: single path
                    vec![expand_tilde_path(p)]
                }
            }
            None => {
                let err = format!(
                    "Error: file_path is required for {} subagent.                      Provide the path to an image file.\n\n                     Example: spawn_subagent(\"{}\", \"Extract text\", \"/path/to/image.png\")",
                    agent_type.label(),
                    subagent_type
                );
                log_tool_result("spawn_subagent", &err);
                return Ok(err);
            }
        }
    } else {
        // Non-image types: file_path is optional, use single path if provided
        match &file_path {
            Some(p) => vec![expand_tilde_path(p)],
            None => Vec::new(),
        }
    };

    // Validate file paths for security (sandbox + blocklist)
    let validated_paths = if file_paths.is_empty() {
        Vec::new()
    } else {
        match validate_subagent_paths(&file_paths) {
            Ok(paths) => paths,
            Err(e) => {
                let err = format!("Error: Invalid file path: {}", e);
                log_tool_result("spawn_subagent", &err);
                return Ok(err);
            }
        }
    };

    // Get Ollama client from task-local context
    let ollama = match get_ollama() {
        Some(o) => o,
        None => {
            let err = "Error: Ollama client not available in tool context.                        This tool requires an active Ollama connection."
                .to_string();
            log_tool_result("spawn_subagent", &err);
            return Ok(err);
        }
    };

    // Get Settings from task-local context to resolve model names
    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available in tool context.                        This tool requires application configuration."
                .to_string();
            log_tool_result("spawn_subagent", &err);
            return Ok(err);
        }
    };

    // Resolve model and build config based on subagent type
    // For OCR, also parse and apply ocr_mode if provided
    let config = match agent_type {
        SubagentType::Ocr => {
            let mut cfg = build_ocr_config(&settings);
            match parse_ocr_mode(ocr_mode) {
                Ok(mode) => cfg = cfg.with_ocr_mode(mode),
                Err(e) => {
                    log_tool_result("spawn_subagent", &e);
                    return Ok(e);
                }
            }
            cfg
        }
        SubagentType::Vision => build_vision_config(&settings),
        SubagentType::Translate => build_translate_config(&settings),
        SubagentType::Summarize => build_summarize_config(&settings),
        SubagentType::Document => build_document_config(&settings),
    };

    // Create runner and execute
    let runner = SubagentRunner::new(ollama, config, (*settings).clone());

    let result = match runner.run(agent_type, prompt, validated_paths).await {
        Ok(output) => output,
        Err(e) => {
            let err = format!(
                "Error: {} subagent execution failed: {}",
                agent_type.label(),
                e
            );
            log_tool_result("spawn_subagent", &err);
            return Ok(err);
        }
    };

    log_tool_result("spawn_subagent", &result);
    Ok(result)
}

/// Build SubagentConfig for OCR tasks.
fn build_ocr_config(settings: &crate::settings::Settings) -> SubagentConfig {
    let (model, _, _) = settings.get_subcommand_config("ocr");
    SubagentConfig::new(model, "OCR")
}

/// Build SubagentConfig for Vision tasks.
fn build_vision_config(settings: &crate::settings::Settings) -> SubagentConfig {
    let (model, _, _) = settings.get_subcommand_config("vision");
    SubagentConfig::new(model, VISION_SYSTEM_PROMPT)
}

/// Build SubagentConfig for Translation tasks.
fn build_translate_config(settings: &crate::settings::Settings) -> SubagentConfig {
    let (model, _, _) = settings.get_subcommand_config("translate");
    SubagentConfig::new(model, TRANSLATE_SYSTEM_PROMPT)
}

/// Build SubagentConfig for Summarization tasks.
fn build_summarize_config(settings: &crate::settings::Settings) -> SubagentConfig {
    let (model, _, _) = settings.get_subcommand_config("summarize");
    let system_prompt = build_system_prompt(
        PromptConfig::new(PromptType::Summarize)
            .with_model_id(Some(&model))
            .with_retrieval(false),
    );
    SubagentConfig::new(model, system_prompt)
}

/// Build SubagentConfig for Document processing tasks.
pub fn build_document_config(settings: &crate::settings::Settings) -> SubagentConfig {
    let (model, _, _) = settings.get_subcommand_config("document");
    SubagentConfig::new(model, DOCUMENT_SYSTEM_PROMPT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn test_invalid_subagent_type() {
        // Unknown type strings should fail SubagentType::parse
        assert!(SubagentType::parse("invalid").is_none());
        assert!(SubagentType::parse("unknown").is_none());
        assert!(SubagentType::parse("").is_none());
        assert!(SubagentType::parse("code").is_none());

        // Valid type strings should succeed
        assert!(SubagentType::parse("ocr").is_some());
        assert!(SubagentType::parse("vision").is_some());
        assert!(SubagentType::parse("translate").is_some());
        assert!(SubagentType::parse("summarize").is_some());
        assert!(SubagentType::parse("document").is_some());
    }

    #[test]
    fn test_missing_file_path_for_ocr() {
        // OCR and Vision require file_path (uses_generate_api() == true)
        let ocr = SubagentType::Ocr;
        let vision = SubagentType::Vision;
        assert!(ocr.uses_generate_api());
        assert!(vision.uses_generate_api());

        // Text-based types do not require file_path
        assert!(!SubagentType::Translate.uses_generate_api());
        assert!(!SubagentType::Summarize.uses_generate_api());
        assert!(!SubagentType::Document.uses_generate_api());
    }

    #[test]
    fn test_valid_subagent_types_constant() {
        // Ensure the VALID_SUBAGENT_TYPES constant matches the enum
        assert_eq!(VALID_SUBAGENT_TYPES.len(), 5);
        assert!(VALID_SUBAGENT_TYPES.contains(&"ocr"));
        assert!(VALID_SUBAGENT_TYPES.contains(&"vision"));
        assert!(VALID_SUBAGENT_TYPES.contains(&"translate"));
        assert!(VALID_SUBAGENT_TYPES.contains(&"summarize"));
        assert!(VALID_SUBAGENT_TYPES.contains(&"document"));
    }

    #[test]
    fn test_file_path_empty_string_normalization() {
        // Empty string should be treated as None (filter pattern)
        let file_path: Option<String> = Some("".to_string());
        let normalized = file_path.filter(|s| !s.is_empty());
        assert!(normalized.is_none());

        // Non-empty string should remain Some
        let file_path: Option<String> = Some("/tmp/image.png".to_string());
        let normalized = file_path.filter(|s| !s.is_empty());
        assert!(normalized.is_some());

        // None should stay None
        let file_path: Option<String> = None;
        let normalized = file_path.filter(|s| !s.is_empty());
        assert!(normalized.is_none());
    }

    #[test]
    fn test_error_message_for_invalid_type() {
        // Verify the error message format matches expectations
        let invalid_type = "foobar";
        let err = format!(
            "Error: Unknown subagent type '{}'. Valid types: {}",
            invalid_type,
            VALID_SUBAGENT_TYPES.join(", ")
        );
        assert!(err.contains("summarize, document"));
    }

    #[test]
    fn test_build_translate_config_uses_translategemma() {
        // Build Settings and call build_translate_config
        // get_subcommand_config returns config key "translategemma",
        // SubagentConfig::new() resolves it to model_id "translategemma:4b"
        let settings = Settings::default();
        let config = build_translate_config(&settings);
        assert_eq!(config.model, "translategemma:4b");
    }

    #[test]
    fn test_build_ocr_config_uses_glm_ocr() {
        // Build Settings and call build_ocr_config
        // get_subcommand_config returns config key "glm-ocr",
        // SubagentConfig::new() resolves it to model_id "glm-ocr:bf16"
        let settings = Settings::default();
        let config = build_ocr_config(&settings);
        assert_eq!(config.model, "glm-ocr:bf16");
    }

    #[test]
    fn test_build_summarize_config_model() {
        // Build Settings and call build_summarize_config
        let settings = Settings::default();
        let config = build_summarize_config(&settings);
        // Should use the default model from settings
        assert!(!config.model.is_empty());
    }

    #[test]
    fn test_build_vision_config_model() {
        // Build Settings and call build_vision_config
        let settings = Settings::default();
        let config = build_vision_config(&settings);
        // Should use the default model from settings
        assert!(!config.model.is_empty());
    }

    #[test]
    fn test_build_document_config_model() {
        // Build Settings and call build_document_config
        let settings = Settings::default();
        let config = build_document_config(&settings);
        // Should use the default model from settings
        assert!(!config.model.is_empty());
    }
}
