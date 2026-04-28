//! Agent spawning tools — dedicated tools for each subagent type.
//!
//! Replaces the former `spawn_subagent` generic tool with four
//! purpose-specific tools, each with only the parameters relevant
//! to its subagent type. This eliminates unused optional parameters
//! (e.g., `ocr_mode` on a translate agent) and produces clearer
//! docstrings for the LLM.
//!
//! # Available Tools
//!
//! - `spawn_ocr_agent`       — Extract text from images via OCR
//! - `spawn_vision_agent`    — Analyze or describe images via vision model
//! - `spawn_translate_agent` — Translate text between languages
//! - `spawn_summarize_agent` — Summarize long text

use crate::chat::subagent::{SubagentConfig, SubagentRunner, SubagentType};
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::ocr::mode::parse_ocr_mode;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::security::validate_subagent_paths;
use crate::tools::context::{get_ollama, get_settings};
use crate::utils::expand_tilde_path;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// spawn_ocr_agent
// ---------------------------------------------------------------------------

/// Extract text from images using OCR (Optical Character Recognition).
///
/// Delegates to a dedicated OCR model (typically GLM-OCR or a vision model
/// with OCR capability). Best for: tables, formulas, scanned documents,
/// and structured text extraction.
///
/// # Arguments
/// * `prompt` - **Required.** What to extract or describe.
///   - "Extract all text from this image"
///   - "Extract the table structure"
///   - "Extract the mathematical formula"
///
/// * `file_path` - **Required.** Path to the image file.
///   - Supports PNG, JPG, JPEG, WebP, GIF
///   - Supports `~` home directory expansion
///
/// * `ocr_mode` - **Optional.** OCR extraction mode.
///   - `"text"` — General text recognition (default)
///   - `"table"` — Table structure extraction
///   - `"figure"` — Figure/diagram recognition
///   - `"formula"` — Mathematical formula extraction (LaTeX)
///
/// # Returns
/// Extracted text content on success, or an error message.
///
/// # Example
/// ```ignore
/// spawn_ocr_agent("Extract all text from this image", "/tmp/document.png", None)
/// spawn_ocr_agent("Extract table structure", "/tmp/table.png", Some("table"))
/// spawn_ocr_agent("Extract the formula", "/tmp/formula.png", Some("formula"))
/// ```
#[ollama_rs::function]
pub async fn spawn_ocr_agent(
    prompt: String,
    file_path: String,
    ocr_mode: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None
    let ocr_mode = ocr_mode.filter(|s| !s.is_empty());

    log_tool_call(
        "spawn_ocr_agent",
        &[
            ("prompt".to_string(), {
                let truncated: String = prompt.chars().take(80).collect();
                if prompt.chars().count() > 80 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            }),
            ("file_path".to_string(), file_path.clone()),
            (
                "ocr_mode".to_string(),
                ocr_mode.clone().unwrap_or_else(|| "(text)".to_string()),
            ),
        ],
    );

    let path = expand_tilde_path(&file_path);
    let validated_paths = match validate_subagent_paths(&[path]) {
        Ok(p) => p,
        Err(e) => {
            let err = format!("Error: Invalid file path: {}", e);
            log_tool_result("spawn_ocr_agent", &err);
            return Ok(err);
        }
    };

    let ollama = match get_ollama() {
        Some(o) => o,
        None => {
            let err = "Error: Ollama client not available in tool context.".to_string();
            log_tool_result("spawn_ocr_agent", &err);
            return Ok(err);
        }
    };

    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available in tool context.".to_string();
            log_tool_result("spawn_ocr_agent", &err);
            return Ok(err);
        }
    };

    let (model, _, _) = settings.get_subcommand_config("ocr");
    let mut config = SubagentConfig::new(model, "OCR");

    if let Some(mode_str) = ocr_mode {
        match parse_ocr_mode(Some(mode_str)) {
            Ok(mode) => config = config.with_ocr_mode(mode),
            Err(e) => {
                log_tool_result("spawn_ocr_agent", &e);
                return Ok(e);
            }
        }
    }

    let runner = SubagentRunner::new(ollama, config);
    let result = match runner.run(SubagentType::Ocr, prompt, validated_paths).await {
        Ok(output) => output,
        Err(e) => {
            let err = format!("Error: OCR agent execution failed: {}", e);
            log_tool_result("spawn_ocr_agent", &err);
            return Ok(err);
        }
    };

    log_tool_result("spawn_ocr_agent", &result);
    Ok(result)
}

// ---------------------------------------------------------------------------
// spawn_vision_agent
// ---------------------------------------------------------------------------

/// Analyze or describe images using a vision model.
///
/// Best for: charts, graphs, diagrams, figures, visual content
/// requiring interpretation. Use spawn_ocr_agent for text extraction
/// from scanned documents or tables.
///
/// # Arguments
/// * `prompt` - **Required.** What to analyze or describe.
///   - "Describe this image"
///   - "What charts are visible in this diagram?"
///   - "Compare these two screenshots"
///
/// * `file_path` - **Required.** Path(s) to image file(s).
///   - Single image: "/path/to/image.png"
///   - Multiple images: "/path/img1.png,/path/img2.png" (comma-separated)
///   - Supports `~` home directory expansion
///
/// # Returns
/// Vision model analysis on success, or an error message.
///
/// # Example
/// ```ignore
/// spawn_vision_agent("Describe this image", "/tmp/photo.png")
/// spawn_vision_agent("Compare these screenshots", "/tmp/a.png,/tmp/b.png")
/// ```
#[ollama_rs::function]
pub async fn spawn_vision_agent(
    prompt: String,
    file_path: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "spawn_vision_agent",
        &[
            ("prompt".to_string(), {
                let truncated: String = prompt.chars().take(80).collect();
                if prompt.chars().count() > 80 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            }),
            ("file_path".to_string(), file_path.clone()),
        ],
    );

    // Vision supports comma-separated paths for multi-image analysis
    let file_paths: Vec<PathBuf> = file_path
        .split(',')
        .map(|s| expand_tilde_path(s.trim()))
        .filter(|p| !p.as_os_str().is_empty())
        .collect();

    if file_paths.is_empty() {
        let err = "Error: No valid image paths provided.".to_string();
        log_tool_result("spawn_vision_agent", &err);
        return Ok(err);
    }

    let validated_paths = match validate_subagent_paths(&file_paths) {
        Ok(p) => p,
        Err(e) => {
            let err = format!("Error: Invalid file path: {}", e);
            log_tool_result("spawn_vision_agent", &err);
            return Ok(err);
        }
    };

    let ollama = match get_ollama() {
        Some(o) => o,
        None => {
            let err = "Error: Ollama client not available in tool context.".to_string();
            log_tool_result("spawn_vision_agent", &err);
            return Ok(err);
        }
    };

    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available in tool context.".to_string();
            log_tool_result("spawn_vision_agent", &err);
            return Ok(err);
        }
    };

    let (model, _, _) = settings.get_subcommand_config("vision");
    let system_prompt = "You are a vision model. Analyze the image as instructed. \
        Describe what you see thoroughly and accurately. Output only your analysis.";
    let config = SubagentConfig::new(model, system_prompt);

    let runner = SubagentRunner::new(ollama, config);
    let result = match runner
        .run(SubagentType::Vision, prompt, validated_paths)
        .await
    {
        Ok(output) => output,
        Err(e) => {
            let err = format!("Error: Vision agent execution failed: {}", e);
            log_tool_result("spawn_vision_agent", &err);
            return Ok(err);
        }
    };

    log_tool_result("spawn_vision_agent", &result);
    Ok(result)
}

// ---------------------------------------------------------------------------
// spawn_translate_agent
// ---------------------------------------------------------------------------

/// Translate text between languages.
///
/// Delegates to a dedicated translation model (typically translategemma).
/// Provide the text and target language in the prompt.
///
/// # Arguments
/// * `prompt` - **Required.** The text to translate, or translation instructions.
///   - "Translate to Portuguese: Hello, how are you?"
///   - "Traduza para inglês: Bom dia, como vai?"
///
/// # Returns
/// Translated text on success, or an error message.
///
/// # Example
/// ```ignore
/// spawn_translate_agent("Translate to Portuguese: Hello world")
/// ```
#[ollama_rs::function]
pub async fn spawn_translate_agent(
    prompt: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "spawn_translate_agent",
        &[("prompt".to_string(), {
            let truncated: String = prompt.chars().take(80).collect();
            if prompt.chars().count() > 80 {
                format!("{}...", truncated)
            } else {
                truncated
            }
        })],
    );

    let ollama = match get_ollama() {
        Some(o) => o,
        None => {
            let err = "Error: Ollama client not available in tool context.".to_string();
            log_tool_result("spawn_translate_agent", &err);
            return Ok(err);
        }
    };

    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available in tool context.".to_string();
            log_tool_result("spawn_translate_agent", &err);
            return Ok(err);
        }
    };

    let (model, _, _) = settings.get_subcommand_config("translate");
    let system_prompt = "You are a translator. Translate the text as directed. \
        Preserve meaning, tone, and formatting. Output only the translation, no explanations.";
    let config = SubagentConfig::new(model, system_prompt);

    let runner = SubagentRunner::new(ollama, config);
    let result = match runner
        .run(SubagentType::Translate, prompt, Vec::new())
        .await
    {
        Ok(output) => output,
        Err(e) => {
            let err = format!("Error: Translate agent execution failed: {}", e);
            log_tool_result("spawn_translate_agent", &err);
            return Ok(err);
        }
    };

    log_tool_result("spawn_translate_agent", &result);
    Ok(result)
}

// ---------------------------------------------------------------------------
// spawn_summarize_agent
// ---------------------------------------------------------------------------

/// Summarize long text into key points.
///
/// Delegates to a summarization-optimized model. Provide the full text
/// in the prompt along with any specific summarization instructions.
///
/// # Arguments
/// * `prompt` - **Required.** The text to summarize, plus instructions.
///   - "Summarize the following text in 3 bullet points: <long text>"
///   - "What are the main arguments in this passage? <text>"
///
/// # Returns
/// Summarized text on success, or an error message.
///
/// # Example
/// ```ignore
/// spawn_summarize_agent("Summarize this long text in bullet points: ...")
/// ```
#[ollama_rs::function]
pub async fn spawn_summarize_agent(
    prompt: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "spawn_summarize_agent",
        &[("prompt".to_string(), {
            let truncated: String = prompt.chars().take(80).collect();
            if prompt.chars().count() > 80 {
                format!("{}...", truncated)
            } else {
                truncated
            }
        })],
    );

    let ollama = match get_ollama() {
        Some(o) => o,
        None => {
            let err = "Error: Ollama client not available in tool context.".to_string();
            log_tool_result("spawn_summarize_agent", &err);
            return Ok(err);
        }
    };

    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available in tool context.".to_string();
            log_tool_result("spawn_summarize_agent", &err);
            return Ok(err);
        }
    };

    let (model, _, _) = settings.get_subcommand_config("summarize");
    let system_prompt = build_system_prompt(
        PromptConfig::new(PromptType::Summarize)
            .with_model_id(Some(&model))
            .with_retrieval(false),
    );
    let config = SubagentConfig::new(model, system_prompt);

    let runner = SubagentRunner::new(ollama, config);
    let result = match runner
        .run(SubagentType::Summarize, prompt, Vec::new())
        .await
    {
        Ok(output) => output,
        Err(e) => {
            let err = format!("Error: Summarize agent execution failed: {}", e);
            log_tool_result("spawn_summarize_agent", &err);
            return Ok(err);
        }
    };

    log_tool_result("spawn_summarize_agent", &result);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn test_ocr_config_uses_glm_ocr() {
        let settings = Settings::default();
        let (model, _, _) = settings.get_subcommand_config("ocr");
        let config = SubagentConfig::new(model, "OCR");
        assert_eq!(config.model, "glm-ocr:bf16");
    }

    #[test]
    fn test_vision_config_model() {
        let settings = Settings::default();
        let (model, _, _) = settings.get_subcommand_config("vision");
        let config = SubagentConfig::new(model, "Vision");
        assert!(!config.model.is_empty());
    }

    #[test]
    fn test_translate_config_uses_translategemma() {
        let settings = Settings::default();
        let (model, _, _) = settings.get_subcommand_config("translate");
        let config = SubagentConfig::new(model, "Translate");
        assert_eq!(config.model, "translategemma:4b");
    }

    #[test]
    fn test_summarize_config_model() {
        let settings = Settings::default();
        let (model, _, _) = settings.get_subcommand_config("summarize");
        let config = SubagentConfig::new(model, "Summarize");
        assert!(!config.model.is_empty());
    }

    #[test]
    fn test_file_path_empty_string_normalization() {
        let file_path: Option<String> = Some("".to_string());
        let normalized = file_path.filter(|s| !s.is_empty());
        assert!(normalized.is_none());

        let file_path: Option<String> = Some("/tmp/image.png".to_string());
        let normalized = file_path.filter(|s| !s.is_empty());
        assert!(normalized.is_some());

        let file_path: Option<String> = None;
        let normalized = file_path.filter(|s| !s.is_empty());
        assert!(normalized.is_none());
    }
}
