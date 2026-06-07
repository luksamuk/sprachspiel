//! Remember tool for conversation history, notes, and documents access
//!
//! Provides the LLM with explicit access to search and retrieve
//! messages from conversation history, user-created notes, and imported documents.

use crate::consts::roles::{ROLE_USER, format_role_label};
use crate::db::SourceType;
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::settings::{DEFAULT_KEYWORD_WEIGHT, DEFAULT_SEMANTIC_WEIGHT};
use crate::tools::context::{get_db, get_embedding, get_settings};
use sprachspiel_tool_derive::tool;

/// Number of chunks to show in preview for large documents
const MAX_PREVIEW_CHUNKS: i32 = 3;

/// Maximum content size to return for documents without chunks (in bytes)
/// Documents larger than this need to have been chunked during import
const MAX_UNCHUNKED_CONTENT: usize = 50_000; // 50 KB = 50,000 bytes ≈ 10k tokens

/// Maximum characters to display for note/document content in search results
const REMEMBER_NOTE_PREVIEW_CHARS: usize = 150;

/// Maximum characters to display for message content in search results
const REMEMBER_MESSAGE_PREVIEW_CHARS: usize = 200;

/// Maximum characters to display for subsequent message content in search results
const REMEMBER_SUBMESSAGE_PREVIEW_CHARS: usize = 100;

/// Parse a source ID into (SourceType, numeric_id)
/// IDs must include source type prefix (e.g., "msg:42", "doc:13")
fn parse_source_id(id: &str) -> Result<(SourceType, i64), String> {
    if let Some(pos) = id.find(':') {
        let prefix = &id[..pos];
        let num_str = &id[pos + 1..];

        let source_type = SourceType::from_prefix(prefix).ok_or_else(|| {
            format!(
                "Unknown source type: '{}'. Valid types: {}, {}, {}, {}",
                prefix,
                SourceType::Conversation.prefix(),
                SourceType::Document.prefix(),
                SourceType::Note.prefix(),
                SourceType::Web.prefix()
            )
        })?;

        let num = num_str
            .parse::<i64>()
            .map_err(|e| format!("Invalid ID number: {}", e))?;

        Ok((source_type, num))
    } else {
        Err(format!(
            "Invalid ID format: '{}'. Must include source type prefix.\n\
             Use: remember(id=\"{}:42\") for conversations\n\
             Use: remember(id=\"{}:13\") for documents\n\
             Use: remember(id=\"{}:7\") for notes",
            id,
            SourceType::Conversation.prefix(),
            SourceType::Document.prefix(),
            SourceType::Note.prefix()
        ))
    }
}

/// Recall content from your stored history (messages, notes, documents).
///
/// Use this tool to:
/// 1. Get the full content of a specific item (by ID from retrieved context)
/// 2. Get a specific chunk of a large document (by ID and chunk index)
/// 3. Search for topics not in the current context (by query)
///
/// # Arguments
/// * `id` - ID of content to retrieve (MUST include prefix). Optional.
///   - Example: "msg:42" for conversation message
///   - Example: "note:7" for user-created note
///   - Example: "doc:13" for imported document
/// * `chunk` - Chunk index for large documents (0-based). Optional.
///   - Example: "0" for first chunk, "15" for 16th chunk
///   - Use when document has multiple chunks (shown in document metadata)
/// * `query` - Search query for semantic search. Optional.
///   - Example: "Wittgenstein" to find content about that topic
///   - Searches across messages, notes, AND documents
/// * `limit` - Max results for query (default: 5, max: 10). Optional.
///
/// # Returns
/// - For id: Full content with metadata
/// - For id + chunk: Specific chunk content
/// - For query: List of matching items with IDs and excerpts
///
/// # Examples
/// ```ignore
/// remember(id="msg:42")              // Get conversation message 42
/// remember(id="note:7")              // Get note 7
/// remember(id="doc:13")              // Get document 13 (or preview for large docs)
/// remember(id="doc:13", chunk="5")    // Get chunk 5 of document 13
/// remember(query="Wittgenstein")     // Search by topic
/// remember(query="philosophy", limit="10")
/// ```
#[tool]
pub async fn remember(
    id: Option<String>,
    chunk: Option<String>,
    query: Option<String>,
    limit: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "remember",
        &[
            ("id".to_string(), id.clone().unwrap_or_default()),
            ("chunk".to_string(), chunk.clone().unwrap_or_default()),
            ("query".to_string(), query.clone().unwrap_or_default()),
            (
                "limit".to_string(),
                limit.clone().unwrap_or_else(|| "5".to_string()),
            ),
        ],
    );

    // Validate parameters - treat empty strings as None
    let id_is_empty = id.as_ref().map(|s| s.is_empty()).unwrap_or(true);
    let query_is_empty = query.as_ref().map(|s| s.is_empty()).unwrap_or(true);
    let chunk_is_empty = chunk.as_ref().map(|s| s.is_empty()).unwrap_or(true);
    let limit_is_empty = limit.as_ref().map(|s| s.is_empty()).unwrap_or(true);

    // Parse limit (default: 5, range: 1-10)
    let limit_num = if !limit_is_empty {
        match limit.and_then(|l| l.parse::<usize>().ok()) {
            Some(n) => n.clamp(1, 10),
            None => {
                let err = "Error: Invalid 'limit' parameter. Must be a number between 1 and 10.\n\n\
                           Example: remember(query=\"philosophy\", limit=\"10\")";
                log_tool_result("remember", err);
                return Ok(err.to_string());
            }
        }
    } else {
        5
    };

    // Parse chunk (only valid with id)
    let chunk_num = if !chunk_is_empty {
        match chunk.and_then(|c| c.parse::<i32>().ok()) {
            Some(n) if n < 0 => {
                let err = "Error: Invalid 'chunk' parameter. Must be a non-negative number.\n\n\
                           Example: remember(id=\"doc:13\", chunk=\"0\") for first chunk";
                log_tool_result("remember", err);
                return Ok(err.to_string());
            }
            Some(n) => Some(n),
            None => {
                let err = "Error: Invalid 'chunk' parameter. Must be a number.\n\n\
                           Example: remember(id=\"doc:13\", chunk=\"5\") for chunk 5";
                log_tool_result("remember", err);
                return Ok(err.to_string());
            }
        }
    } else {
        None
    };

    // Filter empty strings from id and query
    let id_val = id.filter(|s| !s.is_empty());
    let query_val = query.filter(|s| !s.is_empty());

    // Check for missing required parameters
    if id_is_empty && query_is_empty {
        let err = "Error: Provide either 'id' or 'query' parameter.\n\n\
                   Examples:\n\
                   - remember(id=\"msg:42\") to get a specific message\n\
                   - remember(id=\"note:7\") to get a specific note\n\
                   - remember(id=\"doc:13\") to get document 13 (or preview)\n\
                   - remember(id=\"doc:13\", chunk=\"5\") to get chunk 5\n\
                   - remember(query=\"Wittgenstein\") to search by topic\n\n\
                   Note: Use source prefix for IDs (msg:, note:, doc:)";
        log_tool_result("remember", err);
        return Ok(err.to_string());
    }

    // Validate mutually exclusive parameters: id and query cannot both be specified
    if id_val.is_some() && query_val.is_some() {
        let err = "Error: Cannot use both 'id' and 'query' parameters at the same time.\n\n\
                   Use one or the other:\n\
                   - remember(id=\"msg:42\") to retrieve a specific item\n\
                   - remember(query=\"authentication\") to search by topic\n\n\
                   Parameters 'id' and 'query' are mutually exclusive.";
        log_tool_result("remember", err);
        return Ok(err.to_string());
    }

    // Validate conditional parameters: limit only valid with query
    if !limit_is_empty && id_val.is_some() {
        let err = "Error: The 'limit' parameter is only valid with 'query' searches.\n\n\
                   The 'limit' parameter controls how many search results to return.\n\
                   It has no effect when retrieving a specific item by ID.\n\n\
                   Examples:\n\
                   - remember(query=\"authentication\", limit=\"10\")\n\
                   - remember(id=\"msg:42\") (omits limit, retrieves single item)";
        log_tool_result("remember", err);
        return Ok(err.to_string());
    }

    // Get task-local context
    let result = match (get_db(), get_embedding()) {
        (Some(db), Some(embedding)) => {
            if let Some(id_str) = id_val {
                // chunk validation happens inside remember_by_id after source_type is known
                remember_by_id(&db, &id_str, chunk_num).await
            } else if let Some(q) = query_val {
                remember_by_query(&db, &embedding, &q, limit_num).await
            } else {
                unreachable!() // Already validated above
            }
        }
        _ => {
            let err = "Error: Conversation database not available.\n\n\
                       This can happen if:\n\
                       1. You're in an anonymous session (--anonymous flag)\n\
                       2. Database initialization failed at startup\n\
                       3. Database path is inaccessible\n\n\
                       Check the startup messages for database errors.\n\
                        Use 'sprach chat' without --anonymous, or check database permissions.";
            err.to_string()
        }
    };

    log_tool_result("remember", &result);
    Ok(result)
}

/// Retrieve a specific message by its ID
async fn remember_by_id(
    db: &std::sync::Arc<crate::db::Database>,
    id_str: &str,
    chunk: Option<i32>,
) -> String {
    // Parse ID (supports "42" and "msg:42" formats)
    let (source_type, numeric_id) = match parse_source_id(id_str) {
        Ok(result) => result,
        Err(e) => {
            let err = format!(
                "Error: {}\n\nTip: Look for id=\"N\" attributes in <retrieved_context>.",
                e
            );
            log_tool_result("remember", &err);
            return err;
        }
    };

    // Validate: chunk parameter only valid for documents
    if let Some(chunk_index) = chunk
        && source_type != SourceType::Document
    {
        let err = format!(
            "Error: The 'chunk' parameter is only valid for documents.\n\n\
             You used: remember(id=\"{}:{}\", chunk=\"{}\")\n\
             Correct: remember(id=\"doc:{}\", chunk=\"{}\")\n\n\
             {} are retrieved in full - they do not have chunks.\n\
             Only imported documents (doc:) support chunk retrieval.",
            source_type.prefix(),
            numeric_id,
            chunk_index,
            numeric_id,
            chunk_index,
            match source_type {
                SourceType::Conversation => "Messages",
                SourceType::Note => "Notes",
                SourceType::Document => unreachable!(),
                SourceType::Web => "Web content",
            }
        );
        log_tool_result("remember", &err);
        return err;
    }

    // Handle different source types
    match source_type {
        SourceType::Conversation => fetch_conversation_message(db, numeric_id).await,
        SourceType::Document => fetch_document(db, numeric_id, chunk).await,
        SourceType::Note => fetch_note(db, numeric_id).await,
        SourceType::Web => {
            // Future: Web source support
            let err = "Error: Web retrieval not yet implemented.\n\n\
                       Only conversation messages, notes, and documents are supported at this time.";
            log_tool_result("remember", err);
            err.to_string()
        }
    }
}

/// Fetch a conversation message by ID
async fn fetch_conversation_message(db: &std::sync::Arc<crate::db::Database>, id: i64) -> String {
    // Get message from database
    match db.get_content_item_by_id(id) {
        Ok(Some(item)) => {
            let role = item.role.as_deref().unwrap_or("unknown");
            let role_label = format_role_label(role);

            let timestamp = item.created_at;

            let mut output = format!(
                "**Message {}**\nRole: {}\nTimestamp: {}\n\n---\n{}\n---",
                item.id,
                role_label,
                timestamp.format("%Y-%m-%d %H:%M"),
                item.content
            );

            // If user message, also fetch subsequent assistant messages
            if role == ROLE_USER
                && let Some(conv_id) = &item.conversation_id
            {
                // Get subsequent messages
                match db.get_content_subsequent_assistant(item.id, conv_id) {
                    Ok(assistant_msgs) => {
                        for answer in assistant_msgs {
                            output.push_str(&format!(
                                "\n\n**Assistant Response (id={})**\nTimestamp: {}\n\n---\n{}\n---",
                                answer.id,
                                answer.created_at.format("%Y-%m-%d %H:%M"),
                                answer.content
                            ));
                        }
                    }
                    Err(e) => {
                        output
                            .push_str(&format!("\n\n*Failed to fetch assistant response: {}*", e));
                    }
                }
            }

            output
        }
        Ok(None) => format!(
            "Error: Message {} not found.\n\n\
             The message may have been deleted or does not exist.\n\
             Try using remember(query=\"...\") to search by topic instead.",
            id
        ),
        Err(e) => format!(
            "Error: Failed to retrieve message {}.\n\n\
             Details: {}",
            id, e
        ),
    }
}

/// Fetch a note by ID
async fn fetch_note(db: &std::sync::Arc<crate::db::Database>, id: i64) -> String {
    match db.get_note(id) {
        Ok(Some(note)) => {
            let scope_str = match note.scope {
                crate::content::ContentScope::Global => "global",
                crate::content::ContentScope::Project => "project",
            };

            let source_str = match note.source {
                crate::content::ContentSource::User => "user",
                crate::content::ContentSource::Llm => "llm",
            };

            let mut output = format!(
                "**Note {}**\nScope: {}\nSource: {}\nCreated: {}\n",
                note.id,
                scope_str,
                source_str,
                note.created_at.format("%Y-%m-%d %H:%M")
            );

            if let Some(ref title) = note.title {
                output.push_str(&format!("Title: {}\n", title));
            }

            if let Some(ref project_id) = note.project_id {
                output.push_str(&format!("Project: {}\n", project_id));
            }

            output.push_str("\n---\n");
            output.push_str(&note.content);
            output.push_str("\n---");

            output
        }
        Ok(None) => format!(
            "Error: Note {} not found.\n\n\
             The note may have been deleted or does not exist.\n\
             Try using remember(query=\"...\") to search for notes.",
            id
        ),
        Err(e) => format!(
            "Error: Failed to retrieve note {}.\n\n\
             Details: {}",
            id, e
        ),
    }
}

/// Fetch a document by ID, optionally with a specific chunk
async fn fetch_document(
    db: &std::sync::Arc<crate::db::Database>,
    id: i64,
    chunk: Option<i32>,
) -> String {
    match db.get_document(id) {
        Ok(Some(doc)) => {
            let scope_str = match doc.scope {
                crate::content::ContentScope::Global => "global",
                crate::content::ContentScope::Project => "project",
            };

            // Check if document has chunks
            let has_chunks = match db.content_item_has_chunks(id) {
                Ok(has) => has,
                Err(e) => {
                    return format!("Error: Failed to check document chunks.\n\nDetails: {}", e);
                }
            };

            // If chunk specified, fetch only that chunk
            if let Some(chunk_index) = chunk {
                return fetch_document_chunk(db, id, chunk_index, &doc);
            }

            // If document has chunks, show preview
            if has_chunks {
                return fetch_document_preview(db, id, &doc).await;
            }

            // NO CHUNKS - Check size before returning full content
            if doc.content.len() > MAX_UNCHUNKED_CONTENT {
                // Document too large and has no chunks - needs reimport
                let scope_flag = match doc.scope {
                    crate::content::ContentScope::Global => " --global",
                    crate::content::ContentScope::Project => "",
                };
                return format!(
                    "**Document {}**: \"{}\"\n\
                     Type: {} | Size: {:.1} KB ({:.0} bytes) | Words: {}\n\
                     \n\
                     ⚠️ **This document is too large to display** ({} characters).\n\
                     It was imported without proper indexing.\n\
                     \n\
                     **To fix:** Delete and re-import this document:\n\
                     \n\
                     1. Delete: `/doc delete {}{}`\n\
                     2. Re-import: `import_document(\"{}\", None, \"Descriptive Title\")`\n\
                     \n\
                     The re-imported document will be automatically chunked for navigation.",
                    doc.id,
                    doc.title.escape_default(),
                    doc.file_type.extension(),
                    doc.content.len() as f64 / 1024.0,
                    doc.content.len() as f64,
                    doc.word_count,
                    doc.content.len(),
                    doc.id,
                    scope_flag,
                    doc.filename
                );
            }

            // Small enough - return full content
            let mut output = format!(
                "**Document {}**\nType: {}\nScope: {}\n",
                doc.id,
                doc.file_type.extension(),
                scope_str,
            );

            output.push_str(&format!("File: {}\n", doc.filename));
            output.push_str(&format!("Title: {}\n", doc.title));
            output.push_str(&format!("Words: {}\n", doc.word_count));
            output.push_str(&format!(
                "Created: {}\n",
                doc.created_at.format("%Y-%m-%d %H:%M")
            ));

            if let Some(ref project_id) = doc.project_id {
                output.push_str(&format!("Project: {}\n", project_id));
            }

            output.push_str("\n---\n");
            output.push_str(&doc.content);
            output.push_str("\n---");

            output
        }
        Ok(None) => format!(
            "Error: Document {} not found.\n\n\
             The document may have been deleted or does not exist.\n\
             Try using remember(query=\"...\") to search for documents.",
            id
        ),
        Err(e) => format!(
            "Error: Failed to retrieve document {}.\n\n\
             Details: {}",
            id, e
        ),
    }
}

/// Fetch a specific chunk of a large document
fn fetch_document_chunk(
    db: &std::sync::Arc<crate::db::Database>,
    doc_id: i64,
    chunk_index: i32,
    doc: &crate::content::Document,
) -> String {
    let scope_str = match doc.scope {
        crate::content::ContentScope::Global => "global",
        crate::content::ContentScope::Project => "project",
    };

    // Get the specific chunk
    let chunk = match db.get_content_chunk(doc_id, chunk_index) {
        Ok(Some(c)) => c,
        Ok(None) => {
            // Chunk doesn't exist - get total count for helpful error
            let total = db.count_content_chunks(doc_id).unwrap_or_default();
            return format!(
                "Error: Invalid chunk index {}.\n\n\
                 Document has {} chunks (0-{}).\n\
                 Use remember(id=\"doc:{}\", chunk=\"N\") with N between 0 and {}.",
                chunk_index,
                total,
                total.saturating_sub(1),
                doc_id,
                total.saturating_sub(1)
            );
        }
        Err(e) => {
            return format!(
                "Error: Failed to retrieve chunk {}.\n\nDetails: {}",
                chunk_index, e
            );
        }
    };

    // Get total chunk count
    let total_chunks = db.count_content_chunks(doc_id).unwrap_or(1);

    let mut output = format!(
        "**Document {}** — Chunk {}/{}\n",
        doc_id,
        chunk_index + 1,
        total_chunks
    );
    output.push_str(&format!("Title: {}\n", doc.title));
    output.push_str(&format!(
        "Type: {} | Scope: {}\n",
        doc.file_type.extension(),
        scope_str
    ));
    output.push_str(&format!(
        "Position: characters {}-{}\n",
        chunk.start_offset, chunk.end_offset
    ));

    if let Some(ref project_id) = doc.project_id {
        output.push_str(&format!("Project: {}\n", project_id));
    }

    output.push_str("\n---\n");
    output.push_str(&chunk.content);
    output.push_str("\n---");

    // Add navigation hint
    if total_chunks > 1 {
        output.push_str(&format!(
            "\n\n*Chunk {} of {}. Use remember(id=\"doc:{}\", chunk=\"N\") to navigate.*",
            chunk_index + 1,
            total_chunks,
            doc_id
        ));
    }

    output
}

/// Fetch preview of a large document (first few chunks)
async fn fetch_document_preview(
    db: &std::sync::Arc<crate::db::Database>,
    doc_id: i64,
    doc: &crate::content::Document,
) -> String {
    let scope_str = match doc.scope {
        crate::content::ContentScope::Global => "global",
        crate::content::ContentScope::Project => "project",
    };

    // Get total chunk count
    let total_chunks = db.count_content_chunks(doc_id).unwrap_or_default();

    // Get first few chunks for preview
    let all_chunks = match db.get_content_chunks(doc_id) {
        Ok(chunks) => chunks,
        Err(e) => {
            return format!(
                "Error: Failed to retrieve document chunks.\n\nDetails: {}",
                e
            );
        }
    };

    let preview_count = std::cmp::min(MAX_PREVIEW_CHUNKS, total_chunks);
    let preview_chunks: Vec<_> = all_chunks.iter().take(preview_count as usize).collect();

    let mut output = format!("**Document {}**: {}\n", doc_id, doc.title);
    output.push_str(&format!(
        "Type: {} | Scope: {} | Words: {}\n",
        doc.file_type.extension(),
        scope_str,
        doc.word_count
    ));
    output.push_str(&format!("File: {}\n", doc.filename));
    output.push_str(&format!("Chunks: {} total\n", total_chunks));

    if let Some(ref project_id) = doc.project_id {
        output.push_str(&format!("Project: {}\n", project_id));
    }

    output.push_str(&format!(
        "\n⚠️ Large document ({} words). Showing chunks 1-{} of {}.\n",
        doc.word_count, preview_count, total_chunks
    ));
    output.push_str(&format!(
        "Use remember(id=\"doc:{}\", chunk=\"N\") to read specific chunks.\n\n",
        doc_id
    ));

    // Show preview chunks
    for (i, chunk) in preview_chunks.iter().enumerate() {
        output.push_str(&format!(
            "--- Chunk {}/{} (chars {}-{}) ---\n",
            i + 1,
            total_chunks,
            chunk.start_offset,
            chunk.end_offset
        ));
        output.push_str(&chunk.content);
        output.push_str("\n\n");
    }

    output.push_str("---\n");
    output.push_str(&format!(
        "*Use remember(id=\"doc:{}\", chunk=\"N\") for other chunks (0-{}).*",
        doc_id,
        total_chunks - 1
    ));

    output
}

/// Search for content (messages, notes, and documents) by semantic query
async fn remember_by_query(
    db: &std::sync::Arc<crate::db::Database>,
    embedding_client: &std::sync::Arc<crate::embeddings::EmbeddingClient>,
    query: &str,
    limit: usize,
) -> String {
    // Generate embedding for query
    let query_result = match embedding_client.embed(query).await {
        Ok(result) => result,
        Err(e) => {
            return format!(
                "Error: Failed to generate embedding for query.\n\n\
                 Details: {}\n\n\
                 This may be a temporary issue. Try again.",
                e
            );
        }
    };
    let embedding = query_result.vector;
    let query_norm_correction = query_result.norm_correction;

    // Get feedback settings for boost and access tracking
    let settings = get_settings();
    let feedback_settings = settings.as_ref().map(|s| &s.feedback);
    let (keyword_weight, semantic_weight) = settings
        .as_ref()
        .map(|s| (s.retrieval.keyword_weight, s.retrieval.semantic_weight))
        .unwrap_or((DEFAULT_KEYWORD_WEIGHT, DEFAULT_SEMANTIC_WEIGHT));
    // Search for notes using unified content search
    let note_params = crate::content::ContentSearchParams {
        query,
        embedding: &embedding,
        query_norm_correction,
        content_type: Some(crate::content::ContentType::Note),
        conversation_id: None,
        project_id: None,
        scope: None,
        limit,
        keyword_weight,
        semantic_weight,
        feedback_settings,
    };

    let note_results = match db.search_content_hybrid(&note_params) {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: Note search failed.\n\n\
                 Details: {}",
                e
            );
        }
    };

    // Search for documents using unified content search
    let doc_params = crate::content::ContentSearchParams {
        query,
        embedding: &embedding,
        query_norm_correction,
        content_type: Some(crate::content::ContentType::Document),
        conversation_id: None,
        project_id: None,
        scope: None,
        limit,
        keyword_weight,
        semantic_weight,
        feedback_settings,
    };

    let doc_results = match db.search_content_hybrid(&doc_params) {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: Document search failed.\n\n\
                 Details: {}",
                e
            );
        }
    };

    // Search for messages using V7 search
    let message_results = match db.search_messages_hybrid(
        query,
        &embedding,
        query_norm_correction,
        None, // conversation_id
        None, // project_id
        limit,
        keyword_weight,
        semantic_weight,
    ) {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: Message search failed.\n\n\
                 Details: {}",
                e
            );
        }
    };

    // Enrich message results with assistant responses
    let enriched_messages = db
        .enrich_content_results_with_context(message_results)
        .unwrap_or_default();

    // Check if we have any results
    if note_results.is_empty() && doc_results.is_empty() && enriched_messages.is_empty() {
        return "No content found matching your query.\n\n\
               Tips:\n\
               - Try different keywords\n\
               - Use broader search terms\n\
               - Content includes messages, notes, and documents"
            .to_string();
    }

    // Format results
    let note_count = note_results.len();
    let doc_count = doc_results.len();
    let message_count = enriched_messages.len();
    let mut output = format!(
        "**Found {} result(s)** ({} message(s), {} note(s), {} document(s))\n\n",
        note_count + doc_count + message_count,
        message_count,
        note_count,
        doc_count
    );

    // Format notes first (if any)
    if !note_results.is_empty() {
        output.push_str("**Notes:**\n\n");
        for result in &note_results {
            let title = result.item.title.as_deref().unwrap_or("Untitled");
            let total_chars = result.item.content.chars().count();
            let content = if total_chars > REMEMBER_NOTE_PREVIEW_CHARS {
                let truncated: String = result
                    .item
                    .content
                    .chars()
                    .take(REMEMBER_NOTE_PREVIEW_CHARS)
                    .collect();
                format!(
                    "{}...[TRUNCATED: {} of {} chars. Use remember(id=\"note:{}\") for full content.]",
                    truncated, REMEMBER_NOTE_PREVIEW_CHARS, total_chars, result.item.id
                )
            } else {
                result.item.content.clone()
            };

            output.push_str(&format!(
                "**[id=note:{}]** {} (score: {:.2})\n{}\n\n",
                result.item.id, title, result.score, content
            ));
        }
    }

    // Format documents (if any)
    if !doc_results.is_empty() {
        output.push_str("**Documents:**\n\n");
        for result in &doc_results {
            let title = result.item.title.as_deref().unwrap_or("Untitled");
            let total_chars = result.item.content.chars().count();
            let content = if total_chars > REMEMBER_NOTE_PREVIEW_CHARS {
                let truncated: String = result
                    .item
                    .content
                    .chars()
                    .take(REMEMBER_NOTE_PREVIEW_CHARS)
                    .collect();
                format!(
                    "{}...[TRUNCATED: {} of {} chars. Use remember(id=\"doc:{}\") for full content.]",
                    truncated, REMEMBER_NOTE_PREVIEW_CHARS, total_chars, result.item.id
                )
            } else {
                result.item.content.clone()
            };

            output.push_str(&format!(
                "**[id=doc:{}]** {} (score: {:.2})\n{}\n\n",
                result.item.id, title, result.score, content
            ));
        }
    }

    // Format messages (if any)
    if !enriched_messages.is_empty() {
        output.push_str("**Messages:**\n\n");
        for result in &enriched_messages {
            let item = &result.item;
            let role = item.role.as_deref().unwrap_or("unknown");
            let role_label = format_role_label(role);

            // Truncate content for display with [TRUNCATED] notice
            let total_chars = item.content.chars().count();
            let content = if total_chars > REMEMBER_MESSAGE_PREVIEW_CHARS {
                let truncated: String = item
                    .content
                    .chars()
                    .take(REMEMBER_MESSAGE_PREVIEW_CHARS)
                    .collect();
                format!(
                    "{}...[TRUNCATED: {} of {} chars. Use remember(id=\"msg:{}\") for full content.]",
                    truncated, REMEMBER_MESSAGE_PREVIEW_CHARS, total_chars, item.id
                )
            } else {
                item.content.clone()
            };

            output.push_str(&format!(
                "**[id={}]** {} (score: {:.2})\n{}\n\n",
                item.id, role_label, result.score, content
            ));

            // Show subsequent assistant messages (for user messages)
            for sub_item in &result.subsequent_items {
                let type_prefix = match sub_item.item.message_type.as_deref() {
                    Some("pre_tool_content") => "[Intermediate] ",
                    _ => "",
                };
                let sub_total_chars = sub_item.item.content.chars().count();
                let sub_content = if sub_total_chars > REMEMBER_SUBMESSAGE_PREVIEW_CHARS {
                    let truncated: String = sub_item
                        .item
                        .content
                        .chars()
                        .take(REMEMBER_SUBMESSAGE_PREVIEW_CHARS)
                        .collect();
                    format!(
                        "{}...[+{} chars]",
                        truncated,
                        sub_total_chars - REMEMBER_SUBMESSAGE_PREVIEW_CHARS
                    )
                } else {
                    sub_item.item.content.clone()
                };
                output.push_str(&format!(
                    "  └─ **[id={}]** {}{}\n",
                    sub_item.item.id,
                    type_prefix,
                    sub_content.trim()
                ));
            }
            if !result.subsequent_items.is_empty() {
                output.push('\n');
            }
        }
    }

    output.push_str("Use IDs to retrieve full content:\n");
    output.push_str("- remember(id=\"msg:N\") for messages\n");
    output.push_str("- remember(id=\"note:N\") for notes\n");
    output.push_str("- remember(id=\"doc:N\") for documents\n");
    output
}
