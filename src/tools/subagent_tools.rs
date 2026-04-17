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
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::tools::context::{get_ollama, get_settings};
use crate::utils::expand_tilde_path;

/// Valid subagent type strings for error messages
const VALID_SUBAGENT_TYPES: &[&str] = &["ocr", "vision", "translate", "summarize", "document"];

/// Default system prompt for OCR subagent
const OCR_SYSTEM_PROMPT: &str = "You are an OCR assistant. Extract all text from the provided image. \
    Return only the extracted text, preserving the original layout and structure as much as possible. \
    Do not add commentary or explanations.";

/// Default system prompt for Vision subagent
const VISION_SYSTEM_PROMPT: &str = "You are a vision assistant. Analyze and describe the provided image \
    according to the user's instructions. Be thorough and accurate in your description.";

/// Default system prompt for Translate subagent
const TRANSLATE_SYSTEM_PROMPT: &str = "You are a professional translator. Translate the provided text \
    according to the user's instructions. Preserve the original meaning, tone, and formatting. \
    Output only the translated text without explanations.";

/// Default system prompt for Document subagent
const DOCUMENT_SYSTEM_PROMPT: &str = "You are a document processing assistant. Process the provided \
    document content according to the user's instructions. Follow the instructions precisely and \
    provide accurate, structured output.";

/// Spawn a specialized subagent for OCR, Vision, Translation, Summarization, or Document extraction.
///
/// Delegates a task to a purpose-built subagent model optimized for that
/// specific task type. The subagent runs independently and returns its result.
///
/// # Arguments
/// * `subagent_type` - **Required.** The type of subagent to invoke.
///   - `"ocr"` — Extract text from images (requires `file_path`)
///   - `"vision"` — Analyze or describe images (requires `file_path`)
///   - `"translate"` — Translate text between languages
///   - `"summarize"` — Summarize long text
///   - `"document"` — Process structured documents
///
/// * `prompt` - **Required.** The task description or text to process.
///   - For OCR: "Extract all text from this image" (file_path required)
///   - For Vision: "Describe what you see in this image" (file_path required)
///   - For Translate: The text to translate (or instructions like "Translate to Portuguese")
///   - For Summarize: The text to summarize
///   - For Document: Instructions for processing the document
///
/// * `file_path` - **Required for OCR/Vision.** Path to the image file.
///   - Example: `"/tmp/screenshot.png"` or `"~/documents/scan.jpg"`
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
/// spawn_subagent("ocr".to_string(), "Extract all text from this image".to_string(), Some("/tmp/document.png".to_string()))
/// spawn_subagent("summarize".to_string(), "Summarize this long text...".to_string(), None)
/// ```
#[ollama_rs::function]
pub async fn spawn_subagent(
    subagent_type: String,
    prompt: String,
    file_path: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None
    let file_path = file_path.filter(|s| !s.is_empty());

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

    // Validate file_path requirement for OCR/Vision
    if agent_type.uses_generate_api() && file_path.is_none() {
        let err = format!(
            "Error: file_path is required for {} subagent. \
             Provide the path to an image file.\n\n\
             Example: spawn_subagent(\"{}\", \"Extract text\", \"/path/to/image.png\")",
            agent_type.label(),
            subagent_type
        );
        log_tool_result("spawn_subagent", &err);
        return Ok(err);
    }

    // Expand tilde in file_path if present
    let resolved_path = file_path.map(|p| expand_tilde_path(&p).to_string_lossy().to_string());

    // Get Ollama client from task-local context
    let ollama = match get_ollama() {
        Some(o) => o,
        None => {
            let err = "Error: Ollama client not available in tool context. \
                       This tool requires an active Ollama connection."
                .to_string();
            log_tool_result("spawn_subagent", &err);
            return Ok(err);
        }
    };

    // Get Settings from task-local context to resolve model names
    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available in tool context. \
                       This tool requires application configuration."
                .to_string();
            log_tool_result("spawn_subagent", &err);
            return Ok(err);
        }
    };

    // Resolve model and build config based on subagent type
    let config = match agent_type {
        SubagentType::Ocr => build_ocr_config(&settings),
        SubagentType::Vision => build_vision_config(&settings),
        SubagentType::Translate => build_translate_config(&settings),
        SubagentType::Summarize => build_summarize_config(&settings),
        SubagentType::Document => build_document_config(&settings),
    };

    // Create runner and execute
    let runner = SubagentRunner::new(ollama, config, (*settings).clone());

    let result = match runner.run(agent_type.clone(), prompt, resolved_path).await {
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
    SubagentConfig::new(model, OCR_SYSTEM_PROMPT)
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
}