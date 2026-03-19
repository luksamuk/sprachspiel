//! Search command implementation
//!
//! Provides hybrid search functionality for conversation history.
//! Uses the unified content_items table (V7 architecture).

use chrono::{DateTime, Utc};
use ollama_rs::Ollama;

use crate::content::{ContentSearchResult, ContentSearchType};
use crate::db::Database;
use crate::debug_tools::log_debug;
use crate::embeddings::EmbeddingClient;
use crate::markdown;

/// Search result with formatted output
pub struct FormattedResult {
    pub item_id: i64,
    pub conversation_id: Option<String>,
    pub role: Option<String>,
    pub content: String,
    pub chunk_content: Option<String>,
    pub chunk_start: Option<i32>,
    pub chunk_end: Option<i32>,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub search_type: ContentSearchType,
}

impl From<ContentSearchResult> for FormattedResult {
    fn from(result: ContentSearchResult) -> Self {
        FormattedResult {
            item_id: result.item.id,
            conversation_id: result.item.conversation_id,
            role: result.item.role,
            content: result.item.content,
            chunk_content: result.chunk_content,
            chunk_start: result.chunk_offsets.map(|(s, _)| s),
            chunk_end: result.chunk_offsets.map(|(_, e)| e),
            timestamp: result.item.created_at,
            score: result.score,
            search_type: result.search_type,
        }
    }
}

/// Display search results in a readable format with markdown
pub fn display_results(results: &[FormattedResult]) {
    if results.is_empty() {
        println!("No results found.");
        return;
    }

    let mut output = String::new();
    output.push_str(&format!("**Search Results** ({} found)\n\n", results.len()));

    for (i, result) in results.iter().enumerate() {
        let type_str = match result.search_type {
            ContentSearchType::Keyword => "🔍 Keyword",
            ContentSearchType::Semantic => "🧠 Semantic",
            ContentSearchType::Hybrid => "🔗 Hybrid",
        };

        let role_str = result.role.as_deref().unwrap_or("unknown");
        let role_label = match role_str {
            "user" => "👤 User",
            "assistant" => "🤖 Assistant",
            "system" => "⚙️ System",
            "tool" => "🔧 Tool",
            _ => role_str,
        };

        let conv_id = result.conversation_id.as_deref().unwrap_or("unknown");

        output.push_str(&format!(
            "{}. [id={}] {} — {} (score: {:.4})\n",
            i + 1,
            result.item_id,
            type_str,
            role_label,
            result.score
        ));

        // Check if we have chunk content (matched a chunk of a long message)
        let display_content = if let (Some(chunk), Some(start), Some(end)) =
            (&result.chunk_content, result.chunk_start, result.chunk_end)
        {
            // Chunk matched - show with ellipsis for context
            let prefix = if start > 0 { "..." } else { "" };
            let suffix = if end < result.content.len() as i32 {
                "..."
            } else {
                ""
            };

            // Truncate chunk if too long for display (respect UTF-8 boundaries)
            let chunk_display = if chunk.chars().count() > 400 {
                format!("{}...", chunk.chars().take(400).collect::<String>())
            } else {
                chunk.clone()
            };

            format!("{}{}{}", prefix, chunk_display, suffix)
        } else {
            // Full message matched - truncate for display (respect UTF-8 boundaries)
            if result.content.chars().count() > 300 {
                format!(
                    "{}...",
                    result.content.chars().take(300).collect::<String>()
                )
            } else {
                result.content.clone()
            }
        };

        // Format content with indentation
        output.push_str("```\n");
        for line in display_content.lines() {
            output.push_str(&format!("  {}\n", line));
        }
        output.push_str("```\n");
        output.push_str(&format!(
            "_{} — {}_\n\n",
            conv_id,
            result.timestamp.format("%Y-%m-%d %H:%M")
        ));
    }

    markdown::print_markdown(&output);
}

/// Run an interactive search session
pub async fn run_search(
    db: &Database,
    ollama: &Ollama,
    query: &str,
    conversation_id: Option<&str>,
    limit: usize,
) {
    // Debug: Show search parameters
    log_debug(&format!(
        "Search params:\n  query: \"{}\"\n  conversation_id: {:?}\n  limit: {}",
        query, conversation_id, limit
    ));

    // Generate embedding for query
    let embedding_client = EmbeddingClient::new(ollama.clone());

    log_debug("Generating embedding for query...");
    let embedding = match embedding_client.embed(query).await {
        Ok(emb) => {
            log_debug(&format!("Embedding generated ({} dimensions)", emb.len()));
            emb
        }
        Err(e) => {
            eprintln!("\x1B[31mError: Failed to generate embedding: {}\x1B[0m", e);
            return;
        }
    };

    // Perform hybrid search using content_items (V7)
    log_debug("Running hybrid search on content_items...");
    let results = match db.search_messages_hybrid(
        query,
        &embedding,
        conversation_id,
        None,
        limit * 2,
        0.4,
        0.6,
    ) {
        Ok(r) => {
            log_debug(&format!("Hybrid search found {} results", r.len()));
            r
        }
        Err(e) => {
            eprintln!("\x1B[31mError: Hybrid search failed: {}\x1B[0m", e);
            return;
        }
    };

    // Enrich results with assistant responses
    log_debug("Enriching results with assistant responses...");
    let enriched_results = match db.enrich_content_results_with_context(results) {
        Ok(r) => {
            let enriched_count = r.iter().filter(|res| !res.chunk_content.is_none()).count();
            log_debug(&format!("Enriched {} results", enriched_count));
            r
        }
        Err(e) => {
            eprintln!("\x1B[33mWarning: Failed to enrich results: {}\x1B[0m", e);
            return;
        }
    };

    // Convert to formatted results
    let formatted: Vec<FormattedResult> = enriched_results.into_iter().map(|r| r.into()).collect();

    display_results(&formatted);
}