//! Document import tool for LLM
//!
//! Allows the LLM to import documents (TXT, MD, ORG) for
//! semantic search and retrieval.
//!
//! # File Size Limit
//!
//! Maximum file size is 2.5 MB (2,500,000 bytes). Larger files are rejected
//! with a helpful error message.
//!
//! # Supported Formats
//!
//! Only plain-text formats are supported: TXT, MD, ORG.
//! For PDF/EPUB files, extract the text first using run_command
//! (pdftotext, epub2txt), then import the resulting text file.

use crate::content::document::{Document, MAX_DOCUMENT_SIZE, detect_file_type};
use crate::content::types::ContentScope;
use crate::debug_tools::{log_tool_call, log_tool_result, tui_aware_print};
use crate::embeddings::{DEFAULT_CONTEXT_LENGTH, EmbedItemContext, embed_item_with_fallback};
use crate::project::get_project_id;
use crate::tools::context::{get_db, get_embedding};
use crate::utils::expand_tilde_path;
use std::fs;

/// Import a document file for semantic search and retrieval.
///
/// Documents are imported with **synchronous indexing** - they are immediately
/// searchable after this tool returns. Large documents are automatically chunked.
///
/// **IMPORTANT:** For .txt files without obvious titles, provide a descriptive
/// title to improve search quality and avoid duplicate imports.
///
/// # Arguments
/// * `path` - **Required.** Absolute or relative path to the file.
///   - Supports `~` home directory expansion
///   - Example: `"~/documents/notes.txt"` or `"/tmp/report.md"`
///   - **Only TXT, MD, and ORG files are supported.**
///
/// * `scope` - **Optional.** Search visibility.
///   - `"project"` (default): Only searchable in current project
///   - `"global"`: Searchable across all conversations
///
/// * `title` - **Recommended for .txt files.** A descriptive title for the document.
///   - Required when importing .txt files (they have no internal structure)
///   - Optional for .md/.org files (title is extracted automatically)
///   - Good titles: `"Meeting Notes 2026-03-29"`, `"GEB Chapter 1"`, `"Q3 Report"`
///   - Bad titles: `"notes"`, `"file"`, `"document"`
///
/// # File Size Limit
/// **Maximum: 2.5 MB**
///
/// Files larger than 2.5MB must be split before importing.
///
/// # Supported Formats
///
/// | Format | Title Extraction | Notes |
/// |--------|------------------|-------|
/// | .txt   | None (provide via `title`) | Plain text |
/// | .md    | First `# Heading` | Markdown |
/// | .org   | `#+TITLE:` or first `* Heading` | Org-mode |
///
/// # PDF/EPUB Files
///
/// Direct import of PDF and EPUB files is NOT supported.
/// To import content from PDF/EPUB files:
/// 1. Extract text using run_command: `run_command("pdftotext", ["file.pdf", "-"])`
/// 2. Save the extracted text to a .txt or .md file
/// 3. Import the text file with import_document
///
/// # Returns
/// Returns confirmation with document ID, word count, and chunk count.
/// Use `remember(id="doc:N")` to retrieve specific content.
///
/// # Errors
///
/// * `"File not found"` - Check the path is correct
/// * `"Document too large"` - Split the file first (max 2.5MB)
/// * `"Unsupported file type"` - PDF/EPUB not supported; extract text first
///
/// # After Importing
///
/// The document is immediately searchable:
/// ```ignore
/// remember(query="specific topic from document")
/// remember(id="doc:N", chunk="0")  // First chunk
/// remember(id="doc:N", chunk="1")  // Second chunk
/// ```
///
/// If indexing fails, the document is still stored but not searchable.
/// Run `/reindex` to generate embeddings later.
///
/// # Example
/// ```ignore
/// // Plain text with custom title (RECOMMENDED)
/// import_document(
///     "/path/to/notes.txt".to_string(),
///     None,
///     Some("Meeting Notes with Team 2026-03-29".to_string())
/// )
///
/// // Markdown with automatic title extraction
/// import_document(
///     "/path/to/report.md".to_string(),
///     None,
///     None
/// )
///
/// // Global scope for reference material
/// import_document(
///     "~/git/biblio/references.org".to_string(),
///     Some("global".to_string()),
///     None
/// )
/// ```
#[ollama_rs::function]
pub async fn import_document(
    path: String,
    scope: Option<String>,
    title: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None — LLMs may send "" instead of omitting
    let title = title.filter(|s| !s.is_empty());

    log_tool_call(
        "import_document",
        &[
            ("path".to_string(), path.clone()),
            (
                "scope".to_string(),
                scope.clone().unwrap_or_else(|| "project".to_string()),
            ),
            (
                "title".to_string(),
                title.as_deref().unwrap_or("(auto)").to_string(),
            ),
        ],
    );

    // Parse scope
    let content_scope = match scope.as_deref() {
        Some("global") => ContentScope::Global,
        Some("project") | None => ContentScope::Project,
        Some(s) => {
            let err = format!("Error: Invalid scope '{}'. Use 'project' or 'global'.", s);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    // Resolve path (expand ~ to home directory)
    let file_path = expand_tilde_path(&path);
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
        let size_mb = metadata.len() as f64 / 1_000_000.0;
        let limit_mb = MAX_DOCUMENT_SIZE as f64 / 1_000_000.0;
        let err = format!(
            "Error: File too large ({:.1} MB = {:.0} bytes). Maximum is {:.1} MB ({:.0} bytes).\n\
             \n\
             File: {}\n\
             \n\
             To import large documents, ask the user to split the file externally,\n\
             or import a smaller file. The LLM cannot split files automatically.",
            size_mb,
            metadata.len(),
            limit_mb,
            MAX_DOCUMENT_SIZE,
            path
        );
        log_tool_result("import_document", &err);
        return Ok(err);
    }

    // Detect file type (rejects PDF/EPUB with helpful message)
    let file_type = match detect_file_type(&file_path) {
        Ok(ft) => ft,
        Err(e) => {
            let err = format!("Error: {}", e);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    // Get database context
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Database not available. Documents require a database connection.\n\
                        Start sprach without --anonymous to use documents."
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

    // Read file content (only TXT/MD/ORG supported after detect_file_type)
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            let err = format!("Error: Cannot read file: {}", e);
            log_tool_result("import_document", &err);
            return Ok(err);
        }
    };

    // Extract title and create document
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();

    // Use provided title, or extract from content, or fall back to filename
    let final_title = title.unwrap_or_else(|| Document::extract_title(&content, &filename));

    let document = match Document::new(
        content.clone(),
        final_title.clone(),
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
        ContentScope::Project => {
            format!("project '{}'", project_id.as_deref().unwrap_or("default"))
        }
        ContentScope::Global => "global".to_string(),
    };

    // Generate embeddings synchronously (documents need to be searchable immediately)
    let result = if let Some(embedding_client) = get_embedding() {
        let db_clone = db.clone();
        let ctx = EmbedItemContext::new(
            &document.content,
            doc_id,
            "document",
            None,
            project_id.as_deref(),
        );

        // Use block_in_place for synchronous embedding in async context
        let embed_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                embed_item_with_fallback(ctx, &db_clone, &embedding_client, DEFAULT_CONTEXT_LENGTH)
                    .await
            })
        });

        match embed_result {
            Ok(embed_result) => {
                let chunks = embed_result.chunks_created.max(1);
                tui_aware_print(&format!(
                    "📄 Imported doc #{}: \"{}\" ({} chunks, {})",
                    doc_id, final_title, chunks, scope_str
                ));
                format!(
                    "Imported document {} ({})\n\
                     **Title:** {}\n\
                     **Type:** {}\n\
                     **Words:** {}\n\
                     **Chunks:** {}\n\
                     **Scope:** {}\n\
                     \n\
                     Document is indexed and ready for search.\n\
                     Use remember(id=\"doc:{}\") to retrieve.\n\
                     Use remember(query=\"...\") to search by topic.",
                    doc_id,
                    file_type.extension(),
                    final_title,
                    filename,
                    document.word_count,
                    chunks,
                    scope_str,
                    doc_id
                )
            }
            Err(e) => {
                tui_aware_print(&format!(
                    "📄 Imported doc #{}: \"{}\" (indexing failed, {})",
                    doc_id, final_title, scope_str
                ));
                format!(
                    "Imported document {} ({}) BUT indexing failed: {}\n\
                     **Title:** {}\n\
                     **Words:** {}\n\
                     \n\
                     The document is stored but NOT searchable.\n\
                     Run '/reindex' to generate embeddings.\n\
                     Use remember(id=\"doc:{}\") to retrieve manually.",
                    doc_id,
                    file_type.extension(),
                    e,
                    final_title,
                    document.word_count,
                    doc_id
                )
            }
        }
    } else {
        tui_aware_print(&format!(
            "📄 Imported doc #{}: \"{}\" (no embedding, {} words, {})",
            doc_id, final_title, document.word_count, scope_str
        ));
        format!(
            "Imported document {} ({})\n\
             **Title:** {}\n\
             **Words:** {}\n\
             **Scope:** {}\n\
             \n\
             ⚠️ No embedding model available. Document stored but NOT searchable.\n\
             Run '/reindex' after starting with an embedding model.\n\
             Use remember(id=\"doc:{}\") to retrieve.",
            doc_id,
            file_type.extension(),
            final_title,
            document.word_count,
            scope_str,
            doc_id
        )
    };

    log_tool_result("import_document", &result);
    Ok(result)
}
