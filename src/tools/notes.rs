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

use crate::content::types::{ContentScope, ContentSource, MAX_NOTE_CONTENT_SIZE, Note};
use crate::debug_tools::{RESET, TOOL_DIM, log_tool_call, log_tool_result};
use crate::project::get_project_id;
use crate::spinner::suspend_for_print;
use crate::tools::context::get_db;
use crate::utils::truncate_chars;

/// Parse note ID from various formats ("42" or "note:42")
fn parse_note_id(id: &str) -> Result<i64, String> {
    let id_str = id.trim();
    let numeric_str = if id_str.starts_with("note:") {
        id_str.strip_prefix("note:").unwrap_or(id_str)
    } else {
        id_str
    };
    numeric_str.parse::<i64>().map_err(|_| {
        format!(
            "Invalid note ID: '{}'. Use format 'note:N' or just 'N'.",
            id
        )
    })
}

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
    // Normalize empty strings to None — LLMs may send "" instead of omitting
    let title = title.filter(|s| !s.is_empty());

    log_tool_call(
        "note_add",
        &[
            ("content".to_string(), content.clone()),
            (
                "title".to_string(),
                title.as_deref().unwrap_or("None").to_string(),
            ),
        ],
    );

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
                       Start sprach without --anonymous to use notes.";
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
    )
    .map_err(|e| format!("Failed to create note: {}", e))?;

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
    let preview = truncate_chars(&content, 200);

    suspend_for_print(|| {
        eprintln!(
            "{TOOL_DIM}📝 Created note #{}: \"{}\"{RESET}",
            note_id, title_str
        );
    });

    let result = format!(
        "Created note {} (project-scoped)\n\n**Title:** {}\n\n**Preview:**\n{}\n\n\
         Use remember(id=\"note:{}\") to retrieve full content, or remember(query=\"...\") to search notes.",
        note_id, title_str, preview, note_id
    );

    log_tool_result("note_add", &result);
    Ok(result)
}

/// Edit an existing note's title and/or content.
///
/// Use this to correct or update notes you previously created with note_add.
/// At least one of `title` or `content` must be provided.
///
/// # Arguments
/// * `id` - The note ID to edit. Required.
///   - Format: "42" or "note:42" (both accepted)
///   - Example: "42" or "note:42"
/// * `title` - New title for the note. Optional.
///   - Set to change the note's title
/// * `content` - New content for the note. Optional.
///   - Replaces the entire note content (max 10,000 characters)
///   - Does NOT append — provide the complete new content
///
/// # Returns
/// Confirmation message showing what was updated.
///
/// # Example
/// ```ignore
/// // Change title only
/// note_edit(id="42", title="Revised Architecture Decision")
///
/// // Change content only
/// note_edit(id="note:42", content="Updated content here...")
///
/// // Change both
/// note_edit(id="42", title="New Title", content="New content...")
/// ```
#[ollama_rs::function]
pub async fn note_edit(
    id: String,
    title: Option<String>,
    content: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None — LLMs may send "" instead of omitting
    let title = title.filter(|s| !s.is_empty());
    let content = content.filter(|s| !s.is_empty());

    log_tool_call(
        "note_edit",
        &[
            ("id".to_string(), id.clone()),
            (
                "title".to_string(),
                title.as_deref().unwrap_or("unchanged").to_string(),
            ),
            (
                "content".to_string(),
                content
                    .as_ref()
                    .map(|c| format!("{} chars", c.len()))
                    .unwrap_or_else(|| "unchanged".to_string()),
            ),
        ],
    );

    let parsed_id = match parse_note_id(&id) {
        Ok(n) => n,
        Err(e) => {
            log_tool_result("note_edit", &e);
            return Ok(e);
        }
    };

    if title.is_none() && content.is_none() {
        let err = "Error: Provide at least one of 'title' or 'content' to update.\n\n\
                   Examples:\n\
                   - note_edit(id=\"42\", title=\"New Title\")\n\
                   - note_edit(id=\"note:42\", content=\"New content\")";
        log_tool_result("note_edit", err);
        return Ok(err.to_string());
    }

    if let Some(ref c) = content {
        if c.is_empty() {
            let err = "Error: Note content cannot be empty. To delete the note, use note_delete.";
            log_tool_result("note_edit", err);
            return Ok(err.to_string());
        }
        if c.len() > MAX_NOTE_CONTENT_SIZE {
            let err = format!(
                "Error: Note content exceeds {} characters (got {}.\n\
                 Please shorten the content or split into multiple notes.",
                MAX_NOTE_CONTENT_SIZE,
                c.len()
            );
            log_tool_result("note_edit", &err);
            return Ok(err);
        }
    }

    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Database not available. Notes require a database connection.\n\
                       Start sprach without --anonymous to use notes.";
            log_tool_result("note_edit", err);
            return Ok(err.to_string());
        }
    };

    match db.get_note(parsed_id) {
        Ok(Some(_)) => match db.update_note(parsed_id, title.as_deref(), content.as_deref()) {
            Ok(()) => {
                suspend_for_print(|| {
                    eprintln!("{TOOL_DIM}📝 Updated note #{}{RESET}", parsed_id);
                });
                let mut result = format!("Updated note #{}", parsed_id);
                if let Some(t) = &title {
                    result.push_str(&format!("\n**Title:** {}", t));
                }
                if let Some(c) = &content {
                    let preview = truncate_chars(c, 200);
                    result.push_str(&format!("\n**Content preview:** {}", preview));
                }
                result.push_str(&format!(
                    "\n\nUse remember(id=\"note:{}\") to view full content.",
                    parsed_id
                ));
                log_tool_result("note_edit", &result);
                Ok(result)
            }
            Err(e) => {
                let err = format!("Error: Failed to update note #{}: {}", parsed_id, e);
                log_tool_result("note_edit", &err);
                Ok(err)
            }
        },
        Ok(None) => {
            let err = format!(
                "Error: Note #{} not found.\n\n\
                 Use remember(query=\"...\") to search for notes.",
                parsed_id
            );
            log_tool_result("note_edit", &err);
            Ok(err)
        }
        Err(e) => {
            let err = format!("Error: Failed to retrieve note #{}: {}", parsed_id, e);
            log_tool_result("note_edit", &err);
            Ok(err)
        }
    }
}

/// Delete a note by its ID.
///
/// Permanently removes a note from storage. Use remember(query=\"...\") first
/// to find the note ID if you don't know it.
///
/// # Arguments
/// * `id` - The note ID to delete. Required.
///   - Format: "42" or "note:42" (both accepted)
///   - Example: "42" or "note:42"
///
/// # Returns
/// Confirmation message with the deleted note's title and preview, or error if not found.
///
/// # Example
/// ```ignore
/// note_delete(id="42")
/// note_delete(id="note:42")
/// ```
#[ollama_rs::function]
pub async fn note_delete(id: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("note_delete", &[("id".to_string(), id.clone())]);

    let parsed_id = match parse_note_id(&id) {
        Ok(n) => n,
        Err(e) => {
            log_tool_result("note_delete", &e);
            return Ok(e);
        }
    };

    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Database not available. Notes require a database connection.\n\
                       Start sprach without --anonymous to use notes.";
            log_tool_result("note_delete", err);
            return Ok(err.to_string());
        }
    };

    match db.get_note(parsed_id) {
        Ok(Some(note)) => match db.delete_note(parsed_id) {
            Ok(()) => {
                let title_str = note.title.as_deref().unwrap_or("Untitled");
                suspend_for_print(|| {
                    eprintln!(
                        "{TOOL_DIM}🗑️ Deleted note #{}: \"{}\"{RESET}",
                        parsed_id, title_str
                    );
                });
                let preview = truncate_chars(&note.content, 200);
                let result = format!(
                    "Deleted note #{}\n\n**Title:** {}\n**Preview:** {}\n\n\
                     Use note_add() to create a new note.",
                    parsed_id, title_str, preview
                );
                log_tool_result("note_delete", &result);
                Ok(result)
            }
            Err(e) => {
                let err = format!("Error: Failed to delete note #{}: {}", parsed_id, e);
                log_tool_result("note_delete", &err);
                Ok(err)
            }
        },
        Ok(None) => {
            let err = format!(
                "Error: Note #{} not found.\n\n\
                 Use remember(query=\"...\") to search for notes.",
                parsed_id
            );
            log_tool_result("note_delete", &err);
            Ok(err)
        }
        Err(e) => {
            let err = format!("Error: Failed to retrieve note #{}: {}", parsed_id, e);
            log_tool_result("note_delete", &err);
            Ok(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_add_validation_long() {
        let long_content = "x".repeat(MAX_NOTE_CONTENT_SIZE + 1);
        assert!(
            Note::new(
                long_content,
                ContentScope::Project,
                None,
                ContentSource::Llm,
                None,
            )
            .is_err()
        );
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
        )
        .expect("Valid note should succeed");

        assert_eq!(note.content, content);
        assert_eq!(note.scope, ContentScope::Project);
        assert_eq!(note.source, ContentSource::Llm);
        assert_eq!(note.title, Some("Architecture Note".to_string()));
        assert_eq!(note.project_id, Some("test-project".to_string()));
    }
}
