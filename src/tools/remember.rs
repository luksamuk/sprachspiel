//! Remember tool for conversation history access
//!
//! Provides the LLM with explicit access to search and retrieve
//! messages from conversation history.

use crate::consts::roles::{ROLE_ASSISTANT, ROLE_USER, format_role_label};
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

/// Recall messages from your conversation history.
///
/// Use this tool to:
/// 1. Get the full content of a specific message (by ID from retrieved context)
/// 2. Search for topics not in the current context (by query)
///
/// # Arguments
/// * `id` - ID of message to retrieve (MUST include prefix). Optional.
///   - Example: "msg:42" for conversation message
///   - Example: "doc:13" for document (when implemented)
/// * `query` - Search query for semantic search. Optional.
///   - Example: "Wittgenstein" to find messages about that topic
/// * `limit` - Max results for query (default: 5, max: 10). Optional.
///
/// # Returns
/// - For id: Full message content with metadata
/// - For query: List of matching messages with IDs and excerpts
///
/// # Examples
/// ```ignore
/// remember(id="msg:42")              // Get conversation message 42
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

    // Validate parameters
    if id.is_none() && query.is_none() {
        let err = "Error: Provide either 'id' or 'query' parameter.\n\n\
                   Examples:\n\
                   - remember(id=\"42\") to get a specific message\n\
                   - remember(query=\"Wittgenstein\") to search by topic";
        log_tool_result("remember", err);
        return Ok(err.to_string());
    }

    // Parse limit
    let limit_num = limit
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(5)
        .clamp(1, 10);

    // Get task-local context
    let result = match (get_db(), get_embedding()) {
        (Some(db), Some(embedding)) => {
            if let Some(id_str) = id {
                remember_by_id(&db, &id_str).await
            } else if let Some(q) = query {
                remember_by_query(&db, &embedding, &q, limit_num).await
            } else {
                unreachable!() // Already validated above
            }
        }
        _ => {
            let err = "Error: Conversation database not available.\n\n\
                       This can happen if:\n\
                       1. You're in an anonymous session (--anonymous flag)\n\
                       2. The database is not initialized\n\n\
                       Start a regular chat session to access conversation history.";
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
        SourceType::Document => {
            // Phase 5: Document ingestion not yet implemented
            let err = "Error: Document retrieval not yet implemented.\n\n\
                       Only conversation messages are supported at this time. \
                       Use remember(id=\"N\") or remember(id=\"msg:N\") to retrieve messages.";
            log_tool_result("remember", err);
            err.to_string()
        }
        SourceType::Note => {
            // Future: Note support
            let err = "Error: Note retrieval not yet implemented.\n\n\
                       Only conversation messages are supported at this time.";
            log_tool_result("remember", err);
            err.to_string()
        }
        SourceType::Web => {
            // Future: Web source support
            let err = "Error: Web retrieval not yet implemented.\n\n\
                       Only conversation messages are supported at this time.";
            log_tool_result("remember", err);
            err.to_string()
        }
    }
}

/// Fetch a conversation message by ID
async fn fetch_conversation_message(db: &std::sync::Arc<crate::db::Database>, id: i64) -> String {
    // Get message from database
    match db.get_message_by_id(id) {
        Ok(Some(msg)) => {
            let role_label = format_role_label(&msg.role);

            let timestamp =
                chrono::DateTime::from_timestamp(msg.timestamp, 0).unwrap_or_else(chrono::Utc::now);

            let mut output = format!(
                "**Message {}**\nRole: {}\nTimestamp: {}\n\n---\n{}\n---",
                msg.message_id,
                role_label,
                timestamp.format("%Y-%m-%d %H:%M"),
                msg.content
            );

            // If user message, also fetch the assistant response
            if msg.role == ROLE_USER
                && let Ok(Some(answer)) = db.get_next_message_by_role(
                    msg.message_id,
                    &msg.conversation_id,
                    ROLE_ASSISTANT,
                )
            {
                let answer_timestamp = chrono::DateTime::from_timestamp(answer.timestamp, 0)
                    .unwrap_or_else(chrono::Utc::now);
                output.push_str(&format!(
                    "\n\n**Assistant Response (id={})**\nTimestamp: {}\n\n---\n{}\n---",
                    answer.message_id,
                    answer_timestamp.format("%Y-%m-%d %H:%M"),
                    answer.content
                ));
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

/// Search for messages by semantic query
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

    // Perform hybrid search (no ID exclusion for remember tool)
    let results = match db.search_hybrid(&crate::db::SearchParams {
        query,
        embedding: &embedding,
        conversation_id: None,
        project_id: None,
        limit,
        keyword_weight: 0.4,
        semantic_weight: 0.6,
        exclude_ids: None,
    }) {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: Search failed.\n\n\
                 Details: {}",
                e
            );
        }
    };

    if results.is_empty() {
        return "No messages found matching your query.\n\n\
               Tips:\n\
               - Try different keywords\n\
               - Use broader search terms\n\
               - Check if you've discussed this topic before"
            .to_string();
    }

    // Enrich results with assistant responses
    let enriched_results = match db.enrich_with_context(results) {
        Ok(r) => r,
        Err(e) => {
            // Continue with un-enriched results
            return format!(
                "Warning: Could not enrich results: {}\n\n\
                 Use message IDs to retrieve full content.",
                e
            );
        }
    };

    // Format results
    let mut output = format!("**Found {} message(s)**\n\n", enriched_results.len());

    for msg in enriched_results {
        let role_label = format_role_label(&msg.role);

        // Truncate content for display (respect UTF-8 boundaries)
        let content = if msg.content.chars().count() > 200 {
            format!("{}...", msg.content.chars().take(200).collect::<String>())
        } else {
            msg.content.clone()
        };

        output.push_str(&format!(
            "**[id={}]** {} (score: {:.2})\n{}\n\n",
            msg.message_id, role_label, msg.score, content
        ));

        // If user message has an assistant response, show it
        if let Some(ref answer) = msg.next_message {
            let answer_label = format_role_label(ROLE_ASSISTANT);
            let answer_content = if answer.content.chars().count() > 200 {
                format!(
                    "{}...",
                    answer.content.chars().take(200).collect::<String>()
                )
            } else {
                answer.content.clone()
            };
            output.push_str(&format!(
                "  └─ **[id={}]** {}\n     {}\n\n",
                answer.message_id, answer_label, answer_content
            ));
        }
    }

    output.push_str(&format!(
        "Use message IDs to retrieve full content: remember(id=\"{}:N\")",
        SourceType::Conversation.prefix()
    ));
    output
}
