//! Notes tools for LLM-created documents
//!
//! Notes are longer documents (up to 10,000 characters) that persist across sessions.
//! Unlike facts (which are short and always in the system prompt), notes are stored
//! in the database and only retrieved when explicitly requested via the remember tool.
//!
//! Use notes for:
//! - Architecture decisions and rationale
//! - Implementation summaries
//! - How-to guides and tutorials
//! - Extended reference material
//!
//! Use facts for:
//! - Short preferences (single sentence)
//! - Quick facts (project uses PostgreSQL, API key location)
//! - Settings and configuration snippets

use crate::content::types::{ContentScope, ContentSource, Note, MAX_NOTE_CONTENT_SIZE};
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::project::get_project_id;
use crate::tools::context::get_db;

/// Create a note for longer documents that should persist across sessions.
///
/// Notes are stored in the database and can be retrieved later with the remember tool.
/// They are NOT automatically injected into the system prompt (unlike facts).
///
/// # When to Use Notes vs Facts
///
/// **Use note_add for:**
/// - Architecture decisions and their rationale
/// - Implementation notes and summaries
/// - How-to guides and tutorials
/// - Extended code explanations
/// - Meeting notes and decisions
/// - Longer documents (up to 10,000 characters)
///
/// **Use fact_add for:**
/// - Short preferences ("I prefer dark mode", "Use snake_case")
/// - Quick facts ("Database is PostgreSQL 15", "API on port 8080")
/// - Settings and small configuration facts
/// - Single-sentence information (max 500 characters)
///
/// # Arguments
/// * `content` - The note content. Can be multi-paragraph and detailed (max 10,000 characters).
/// * `title` - Optional descriptive title for the note. Helps with searching.
///
/// # Returns
/// Confirmation with the note ID and a preview of the content.
///
/// # Example
/// ```ignore
/// note_add(
///     "Decision: We chose PostgreSQL over MySQL for the following reasons:\n\
///      1. Better JSON support with JSONB\n\
///      2. Native full-text search\n\
///      3. Better performance for complex queries\n\
///      4. Strong community support".to_string(),
///     Some("Architecture Decision: Database Choice".to_string())
/// )
/// // Returns: "Created note 42: Architecture Decision: Database Choice\n..."
/// ```
#[ollama_rs::function]
pub async fn note_add(
    content: String,
    title: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("note_add", &[("content".to_string(), content.clone()), ("title".to_string(), title.clone().unwrap_or_else(|| "None".to_string()))]);

    // Validate content length
    if content.is_empty() {
        let err = "Error: Note content cannot be empty. Use fact_add for short facts.";
        log_tool_result("note_add", err);
        return Ok(err.to_string());
    }

    if content.len() > MAX_NOTE_CONTENT_SIZE {
        let err = format!(
            "Error: Note content exceeds {} characters (got {}).\n\
             Please shorten the content or split into multiple notes.\n\
             Tip: Use fact_add for facts under 500 characters.",
            MAX_NOTE_CONTENT_SIZE,
            content.len()
        );
        log_tool_result("note_add", &err);
        return Ok(err);
    }

    // Get database context
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Database not available. Notes require a database connection.\n\
                       Start ask-ai without --anonymous to use notes.";
            log_tool_result("note_add", err);
            return Ok(err.to_string());
        }
    };

    let project_id = get_project_id();

    // Create note with LLM source
    let note = Note::new(
        content.clone(),
        ContentScope::Project, // Always project-scoped
        project_id,
        ContentSource::Llm, // Mark as LLM-created
        title.clone(),
    ).map_err(|e| format!("Failed to create note: {}", e))?;

    // Insert into database
    let note_id = match db.insert_note(&note) {
        Ok(id) => id,
        Err(e) => {
            let err = format!("Error: Failed to save note to database: {}", e);
            log_tool_result("note_add", &err);
            return Ok(err);
        }
    };

    // Build response
    let title_str = title.as_deref().unwrap_or("Untitled");
    let preview = if content.len() > 200 {
        format!("{}...", &content[..200])
    } else {
        content.clone()
    };

    let result = format!(
        "Created note {} (project-scoped)\n\n**Title:** {}\n\n**Preview:**\n{}\n\n\
         Use remember(id=\"note:{}\") to retrieve full content, or remember(query=\"...\") to search notes.",
        note_id, title_str, preview, note_id
    );

    log_tool_result("note_add", &result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_add_validation_long() {
        let long_content = "x".repeat(MAX_NOTE_CONTENT_SIZE + 1);
        assert!(Note::new(
            long_content,
            ContentScope::Project,
            None,
            ContentSource::Llm,
            None,
        ).is_err());
    }

    #[test]
    fn test_note_add_valid() {
        let content = "This is a valid note about architecture.".to_string();
        let note = Note::new(
            content.clone(),
            ContentScope::Project,
            Some("test-project".to_string()),
            ContentSource::Llm,
            Some("Architecture Note".to_string()),
        ).expect("Valid note should succeed");

        assert_eq!(note.content, content);
        assert_eq!(note.scope, ContentScope::Project);
        assert_eq!(note.source, ContentSource::Llm);
        assert_eq!(note.title, Some("Architecture Note".to_string()));
        assert_eq!(note.project_id, Some("test-project".to_string()));
    }
}