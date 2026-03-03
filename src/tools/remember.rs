//! Remember tool for conversation history access
//!
//! Provides the LLM with explicit access to search and retrieve
//! messages from conversation history.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::tools::context::{get_db, get_embedding};

/// Recall messages from your conversation history.
///
/// Use this tool to:
/// 1. Get the full content of a specific message (by ID from retrieved context)
/// 2. Search for topics not in the current context (by query)
///
/// # Arguments
/// * `id` - ID of message to retrieve (from retrieved context). Optional.
///   - Example: "42" to get message with id="42"
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
/// remember(id="42")                           // Get message 42
/// remember(query="Wittgenstein")              // Search for Wittgenstein
/// remember(query="philosophy", limit="10")    // Search with limit
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
            ("limit".to_string(), limit.clone().unwrap_or_else(|| "5".to_string())),
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
        .min(10)
        .max(1);

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

/// Retrieve a specific message by its database ID
async fn remember_by_id(db: &std::sync::Arc<crate::db::Database>, id_str: &str) -> String {
    // Parse ID
    let id: i64 = match id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return format!(
                "Error: Invalid message ID '{}'. Message IDs must be numbers.\n\n\
                 Tip: Look for id=\"N\" attributes in <retrieved_context>.",
                id_str
            );
        }
    };

    // Get message from database
    match db.get_message_by_id(id) {
        Ok(Some(msg)) => {
            let role_label = match msg.role.as_str() {
                "user" => "👤 User",
                "assistant" => "🤖 Assistant",
                "system" => "⚙️ System",
                "tool" => "🔧 Tool",
                _ => &msg.role,
            };

            let timestamp = chrono::DateTime::from_timestamp(msg.timestamp, 0)
                .unwrap_or_else(chrono::Utc::now);

            format!(
                "**Message {}**\nRole: {}\nTimestamp: {}\n\n---\n{}\n---",
                msg.message_id,
                role_label,
                timestamp.format("%Y-%m-%d %H:%M"),
                msg.content
            )
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

    // Perform hybrid search
    let results = match db.search_hybrid(query, &embedding, None, limit, 0.4, 0.6) {
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

    // Format results
    let mut output = format!("**Found {} message(s)**\n\n", results.len());

    for msg in results {
        let role_label = match msg.role.as_str() {
            "user" => "👤 User",
            "assistant" => "🤖 Assistant",
            "system" => "⚙️ System",
            "tool" => "🔧 Tool",
            _ => &msg.role,
        };

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
    }

    output.push_str("Use message IDs to retrieve full content: remember(id=\"N\")");
    output
}