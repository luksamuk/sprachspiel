//! Tools for loading and viewing skills on-demand.
//!
//! Skills are Markdown files with YAML frontmatter that define AI behaviors.
//! They are loaded on-demand when the LLM requests them.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::skills::{get_available_skill_names, get_skill_content, load_skill_indexes};
use crate::spinner::suspend_for_print;
use ollama_rs::function;

/// ANSI style: DIM (faint) + light gray text — matches tool call display
const SKILL_DIM: &str = "\x1B[2m\x1B[37m";
/// ANSI reset
const RESET: &str = "\x1B[0m";
///
/// Returns a list of skill names and descriptions from the SKILLS INDEX.
/// Use this to discover what skills are available before loading one with skill_view.
///
/// # Returns
/// A formatted list of available skills with:
/// - Skill name (use with skill_view)
/// - Brief description
/// - Source (builtin/user/project)
///
/// # Example
/// ```ignore
/// skill_list().await
/// // Returns:
/// // Available skills (4):
/// // - document-processing (builtin): Extract content from PDF and ePub files
/// // - ocr-images (builtin): Process images with OCR
/// // ...
/// ```
#[function]
pub async fn skill_list() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("skill_list", &[]);

    let indexes = load_skill_indexes();

    if indexes.is_empty() {
        let result = "No skills available.".to_string();
        log_tool_result("skill_list", &result);
        return Ok(result);
    }

    let mut lines = Vec::new();
    lines.push(format!("Available skills ({}):", indexes.len()));
    lines.push(String::new());

    for index in indexes {
        lines.push(format!(
            "- **{}** ({}): {}",
            index.name, index.source, index.description
        ));
    }

    lines.push(String::new());
    lines.push("Use `skill_view(name=\"skill-name\")` to load a specific skill.".to_string());

    let result = lines.join("\n");
    log_tool_result("skill_list", &result);
    Ok(result)
}

/// Load and view the full content of a specific skill.
///
/// Returns the complete skill content including all instructions.
/// Use this after `skill_list` shows relevant skills for your task.
///
/// # Arguments
/// * `name` - The skill name (e.g., "document-processing", "ocr-images")
///
/// # Returns
/// The full skill content:
/// - Skill name and description
/// - Complete instructions and guidelines
/// - Examples and common patterns
///
/// # Errors
/// Returns an error message if:
/// - Skill not found (use skill_list to see available skills)
/// - Skill name is invalid (must be alphanumeric + hyphen/underscore)
/// - Skill content failed validation (contains injection patterns)
///
/// # Example
/// ```ignore
/// skill_view("document-processing".to_string()).await
/// // Returns:
/// // # PDF Processing
/// // When asked to process PDF files:
/// // 1. Check tool availability...
/// ```
#[function]
pub async fn skill_view(name: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("skill_view", &[("name".to_string(), name.clone())]);

    // Validate name format
    let valid_names = get_available_skill_names();
    if !valid_names.contains(&name) {
        let available = valid_names.join(", ");
        let result = format!(
            "Error: Skill '{}' not found.\n\nAvailable skills: {}\n\nUse skill_list() to see all available skills.",
            name, available
        );
        log_tool_result("skill_view", &result);
        return Ok(result);
    }

    // Load skill content
    match get_skill_content(&name) {
        Some(skill) => {
            // Check if content is empty after sanitization
            if skill.content.trim().is_empty() {
                let result = format!(
                    "Error: Skill '{}' has no content after sanitization. It may contain injection patterns.",
                    name
                );
                log_tool_result("skill_view", &result);
                return Ok(result);
            }

            // Visual indicator that a skill was loaded (matches tool call styling)
            suspend_for_print(|| {
                eprintln!(
                    "{SKILL_DIM}📖 Loaded skill: {} ({}){RESET}",
                    skill.name, skill.source
                );
            });

            // Format skill content
            let mut lines = Vec::new();
            lines.push(format!("# Skill: {} ({})", skill.name, skill.source));
            lines.push(String::new());
            lines.push(format!("**Description:** {}", skill.description));
            lines.push(String::new());
            lines.push("---".to_string());
            lines.push(String::new());
            lines.push(skill.content);

            let result = lines.join("\n");
            log_tool_result("skill_view", &result);
            Ok(result)
        }
        None => {
            let result = format!(
                "Error: Skill '{}' could not be loaded. It may contain invalid content.",
                name
            );
            log_tool_result("skill_view", &result);
            Ok(result)
        }
    }
}
