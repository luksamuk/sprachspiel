//! Remember tool for conversation history, notes, and documents access
//!
//! Provides the LLM with explicit access to search and retrieve
//! messages from conversation history, user-created notes, and imported documents.

use crate::consts::roles::{ROLE_USER, format_role_label};
use crate::db::SourceType;
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::tools::context::{get_db, get_embedding};

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
/// 2. Search for topics not in the current context (by query)
///
/// # Arguments
/// * `id` - ID of content to retrieve (MUST include prefix). Optional.
///   - Example: "msg:42" for conversation message
///   - Example: "note:7" for user-created note
///   - Example: "doc:13" for imported document
/// * `query` - Search query for semantic search. Optional.
///   - Example: "Wittgenstein" to find content about that topic
///   - Searches across messages, notes, AND documents
/// * `limit` - Max results for query (default: 5, max: 10). Optional.
///
/// # Returns
/// - For id: Full content with metadata
/// - For query: List of matching items with IDs and excerpts
///
/// # Examples
/// ```ignore
/// remember(id="msg:42")              // Get conversation message 42
/// remember(id="note:7")              // Get note 7
/// remember(id="doc:13")              // Get document 13
/// remember(query="Wittgenstein")     // Search by topic
/// remember(query="philosophy", limit="10")
/// ```
#[ollama_rs::function]
pub async fn remember(
    id: Option<String>,
    query: Option<String>,
    limit: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "remember",
        &[
            ("id".to_string(), id.clone().unwrap_or_default()),
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

    if id_is_empty && query_is_empty {
        let err = "Error: Provide either 'id' or 'query' parameter.\n\n\
                   Examples:\n\
                   - remember(id=\"msg:42\") to get a specific message\n\
                   - remember(id=\"note:7\") to get a specific note\n\
                   - remember(query=\"Wittgenstein\") to search by topic\n\n\
                   Note: Use source prefix for IDs (msg:, note:, doc:)";
        log_tool_result("remember", err);
        return Ok(err.to_string());
    }

    // Parse limit
    let limit_num = limit
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(5)
        .clamp(1, 10);

    // Filter empty strings from id and query
    let id_val = id.filter(|s| !s.is_empty());
    let query_val = query.filter(|s| !s.is_empty());

    // Get task-local context
    let result = match (get_db(), get_embedding()) {
        (Some(db), Some(embedding)) => {
            if let Some(id_str) = id_val {
                remember_by_id(&db, &id_str).await
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
                       Use 'ask-ai chat' without --anonymous, or check 'ask-ai.db' permissions.";
            err.to_string()
        }
    };

    log_tool_result("remember", &result);
    Ok(result)
}

/// Retrieve a specific message by its ID
async fn remember_by_id(db: &std::sync::Arc<crate::db::Database>, id_str: &str) -> String {
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

    // Handle different source types
    match source_type {
        SourceType::Conversation => fetch_conversation_message(db, numeric_id).await,
        SourceType::Document => fetch_document(db, numeric_id).await,
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
                        output.push_str(&format!("\n\n*Failed to fetch assistant response: {}*", e));
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

/// Fetch a document by ID
async fn fetch_document(db: &std::sync::Arc<crate::db::Database>, id: i64) -> String {
    match db.get_document(id) {
        Ok(Some(doc)) => {
            let scope_str = match doc.scope {
                crate::content::ContentScope::Global => "global",
                crate::content::ContentScope::Project => "project",
            };

            let mut output = format!(
                "**Document {}**\nType: {}\nScope: {}\n",
                doc.id,
                doc.file_type.extension(),
                scope_str,
            );

            output.push_str(&format!("File: {}\n", doc.filename));
            output.push_str(&format!("Title: {}\n", doc.title));
            output.push_str(&format!("Words: {}\n", doc.word_count));
            output.push_str(&format!("Created: {}\n", doc.created_at.format("%Y-%m-%d %H:%M")));

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

/// Search for content (messages, notes, and documents) by semantic query
async fn remember_by_query(
    db: &std::sync::Arc<crate::db::Database>,
    embedding_client: &std::sync::Arc<crate::embeddings::EmbeddingClient>,
    query: &str,
    limit: usize,
) -> String {
    // Generate embedding for query
    let embedding = match embedding_client.embed(query).await {
        Ok(emb) => emb,
        Err(e) => {
            return format!(
                "Error: Failed to generate embedding for query.\n\n\
                 Details: {}\n\n\
                 This may be a temporary issue. Try again.",
                e
            );
        }
    };

    // Search for notes using unified content search
    let note_params = crate::content::ContentSearchParams {
        query,
        embedding: &embedding,
        content_type: Some(crate::content::ContentType::Note),
        conversation_id: None,
        project_id: None,
        scope: None,
        limit,
        keyword_weight: 0.4,
        semantic_weight: 0.6,
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
        content_type: Some(crate::content::ContentType::Document),
        conversation_id: None,
        project_id: None,
        scope: None,
        limit,
        keyword_weight: 0.4,
        semantic_weight: 0.6,
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
        None,  // conversation_id
        None,  // project_id
        limit,
        0.4,   // keyword_weight
        0.6,   // semantic_weight
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
    let enriched_messages = db.enrich_content_results_with_context(message_results).unwrap_or_default();

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
            let content = if result.item.content.chars().count() > 150 {
                format!("{}...", result.item.content.chars().take(150).collect::<String>())
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
            let content = if result.item.content.chars().count() > 150 {
                format!("{}...", result.item.content.chars().take(150).collect::<String>())
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

            // Truncate content for display (respect UTF-8 boundaries)
            let content = if item.content.chars().count() > 200 {
                format!("{}...", item.content.chars().take(200).collect::<String>())
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
                let sub_content = if sub_item.item.content.chars().count() > 100 {
                    format!(
                        "{}...",
                        sub_item.item.content.chars().take(100).collect::<String>()
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
