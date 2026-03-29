//! Document import tool for LLM
//!
//! Allows the LLM to import documents (TXT, MD, ORG, PDF, EPUB) for
//! semantic search and retrieval.
//!
//! # File Size Limit
//!
//! Maximum file size is 5MB. Larger files are rejected with a helpful error.
//!
//! # Feature Dependencies
//!
//! - TXT/MD/ORG: Builtin support, no dependencies
//! - PDF/EPUB: Requires `skills-tools` feature (uses document-processing skill)

use crate::content::document::{detect_file_type, Document, FileType, MAX_DOCUMENT_SIZE};
use crate::content::types::ContentScope;
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::project::get_project_id;
use crate::tools::context::get_db;
use std::fs;
use std::path::PathBuf;

/// Import a document file for semantic search and retrieval.
///
/// Documents can be TXT, MD, ORG (builtin), or PDF, EPUB (requires skills-tools).
/// They are stored in the content_items table and can be searched via the remember tool.
///
/// # Arguments
/// * `path` - Absolute or relative path to the file.
/// * `scope` - "project" (default) or "global". Optional.
///
/// # Returns
/// Document ID, title, word count, and confirmation message.
///
/// # Errors
/// - File not found
/// - File too large (> 5MB)
/// - Unsupported file type
/// - PDF/EPUB requires skills-tools feature
///
/// # Example
/// ```ignore
/// import_document("/path/to/report.pdf".to_string(), Some("project".to_string()))
/// import_document("notes.md".to_string(), Some("global".to_string()))
/// ```
#[ollama_rs::function]
pub async fn import_document(
    path: String,
    scope: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "import_document",
        &[
            ("path".to_string(), path.clone()),
            ("scope".to_string(), scope.clone().unwrap_or_else(|| "project".to_string())),
        ],
    );

    // Parse scope
    let content_scope = match scope.as_deref() {
        Some("global") => ContentScope::Global,
        Some("project") | None => ContentScope::Project,
        Some(s) => {
            let err = format!(
                "Error: Invalid scope '{}'. Use 'project' or 'global'.",
                s
            );
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    // Resolve path
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        let err = format!(
            "Error: File not found: '{}'. Please check the path and try again.",
            path
        );
        log_tool_result("import_document", &err);
        return Ok(err);
    }

    // Check file size
    let metadata = match fs::metadata(&file_path) {
        Ok(m) => m,
        Err(e) => {
            let err = format!("Error: Cannot read file metadata: {}", e);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    if metadata.len() > MAX_DOCUMENT_SIZE as u64 {
        let err = format!(
            "Error: File exceeds maximum size of {} bytes (got {} bytes).\n\
             Consider splitting the document into smaller files.\n\
             Maximum size: 5MB",
            MAX_DOCUMENT_SIZE,
            metadata.len()
        );
        log_tool_result("import_document", &err);
        return Ok(err);
    }

    // Detect file type
    let file_type = match detect_file_type(&file_path) {
        Ok(ft) => ft,
        Err(e) => {
            let err = format!("Error: {}", e);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    // Check if PDF/EPUB requires skills-tools
    #[cfg(not(feature = "skills-tools"))]
    {
        if file_type.requires_skills() {
            let err = format!(
                "Error: Importing '{}' files requires the 'skills-tools' feature.\n\
                 Recompile with: cargo build --features skills-tools\n\
                 Alternatively, convert to TXT/MD/ORG format first.",
                file_type.extension()
            );
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    }

    // Get database context
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Database not available. Documents require a database connection.\n\
                       Start ask-ai without --anonymous to use documents."
                .to_string();
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    let project_id = if content_scope == ContentScope::Project {
        get_project_id()
    } else {
        None
    };

    // Read file content
    let content = match file_type {
        FileType::Txt | FileType::Md | FileType::Org => {
            match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    let err = format!("Error: Cannot read file: {}", e);
                    log_tool_result("import_document", &err);
                    return Ok(err);
                }
            }
        }
        FileType::Pdf | FileType::Epub => {
            #[cfg(feature = "skills-tools")]
            {
                match extract_text_with_skill(&file_path, &file_type) {
                    Ok(c) => c,
                    Err(e) => {
                        log_tool_result("import_document", &e);
                        return Ok(e);
                    }
                }
            }
            #[cfg(not(feature = "skills-tools"))]
            {
                unreachable!("Already checked above");
            }
        }
    };

    // Extract title and create document
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();

    let title = Document::extract_title(&content, &filename);

    let document = match Document::new(
        content,
        title.clone(),
        filename.clone(),
        file_type,
        content_scope,
        project_id.clone(),
    ) {
        Ok(d) => d,
        Err(e) => {
            let err = format!("Error: {}", e);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    // Insert into database
    let doc_id = match db.insert_document(&document) {
        Ok(id) => id,
        Err(e) => {
            let err = format!("Error: Failed to save document to database: {}", e);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    let scope_str = match content_scope {
        ContentScope::Project => format!("project '{}'", project_id.as_deref().unwrap_or("default")),
        ContentScope::Global => "global".to_string(),
    };

    let result = format!(
        "Imported document {} ({})\n\
         **Title:** {}\n\
         **Type:** {}\n\
         **Words:** {}\n\
         **Scope:** {}\n\
         \n\
         Use remember(id=\"{}\") to retrieve the full content.\n\
         Use remember(query=\"...\") to search across all documents.",
        doc_id,
        file_type.extension(),
        title,
        filename,
        document.word_count,
        scope_str,
        doc_id,
    );

    log_tool_result("import_document", &result);
    Ok(result)
}

/// Extract text from PDF/EPUB using external tools
///
/// FIXME: Technical Debt - Direct Command Invocation
///
/// This function calls Command::new("pdftotext") directly, bypassing the skills system.
/// Project-level skill overrides for document-processing are not respected.
///
/// Future Solution (Priority 4: Specialized Agent Architecture):
/// - spawn_subagent(type="document", prompt, file_path)
/// - Sub-agent uses run_command within skill-defined constraints
/// - Output returns as tool result to main agent
/// - Skills can override document-processing behavior at project level
///
/// Related: Issue #12 (OCR/Vision), Issue #9 (Document Import)
/// Milestone: Priority 4 expansion (Specialized Agents)
///
/// For now: This implementation works correctly for extraction.
/// Users can manually invoke the document-processing skill for other workflows.
#[cfg(feature = "skills-tools")]
fn extract_text_with_skill(
    file_path: &std::path::Path,
    file_type: &FileType,
) -> Result<String, String> {
    use std::process::Command;

    // Build command based on file type
    let (program, args) = match file_type {
        FileType::Pdf => {
            ("pdftotext", vec![file_path.to_string_lossy().to_string(), "-".to_string()])
        }
        FileType::Epub => {
            ("epub2txt", vec![file_path.to_string_lossy().to_string(), "-".to_string()])
        }
        _ => {
            return Err(format!(
                "Error: Internal error - unexpected file type '{}'.",
                file_type.extension()
            ));
        }
    };

    // Run extraction command
    let output = Command::new(program)
        .args(&args)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .map_err(|e| format!("Error: Failed to parse output as UTF-8: {}", e))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!(
                    "Error: {} failed with exit code {:?}: {}",
                    program,
                    output.status.code(),
                    stderr.trim()
                ))
            }
        }
        Err(e) => {
            Err(format!(
                "Error: Could not run '{}' - {}. Install with your package manager.",
                program,
                e
            ))
        }
    }
}