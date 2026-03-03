//! Search command implementation
//!
//! Provides hybrid search functionality for conversation history.

use chrono::{DateTime, Utc};
use ollama_rs::Ollama;

use crate::db::{Database, SearchResult, SearchType};
use crate::embeddings::EmbeddingClient;

/// Search result with formatted output
pub struct FormattedResult {
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub search_type: SearchType,
}

impl From<SearchResult> for FormattedResult {
    fn from(result: SearchResult) -> Self {
        FormattedResult {
            conversation_id: result.conversation_id,
            role: result.role,
            content: result.content,
            timestamp: DateTime::from_timestamp(result.timestamp, 0)
                .unwrap_or_else(Utc::now),
            score: result.score,
            search_type: result.search_type,
        }
    }
}

/// Search conversations using hybrid search (BM25 + semantic + RRF)
///
/// # Arguments
/// * `db` - Database instance
/// * `ollama` - Ollama client for embeddings
/// * `query` - Search query
/// * `conversation_id` - Optional conversation ID to search within
/// * `limit` - Maximum number of results
///
/// # Returns
/// Formatted search results or error message
pub async fn search_conversations(
    db: &Database,
    ollama: &Ollama,
    query: &str,
    conversation_id: Option<&str>,
    limit: usize,
) -> Result<Vec<FormattedResult>, String> {
    // Generate embedding for query
    let embedding_client = EmbeddingClient::new(ollama.clone());
    
    let embedding = embedding_client
        .embed(query)
        .await
        .map_err(|e| format!("Failed to generate embedding: {}", e))?;

    // Perform hybrid search
    let results = db
        .search_hybrid(query, &embedding, conversation_id, limit, 0.4, 0.6)
        .map_err(|e| format!("Search failed: {}", e))?;

    // Convert to formatted results
    Ok(results.into_iter().map(FormattedResult::from).collect())
}

/// Display search results in a readable format
pub fn display_results(results: &[FormattedResult]) {
    if results.is_empty() {
        println!("No results found.");
        return;
    }

    println!("\n**Search Results** ({} found)\n", results.len());

    for (i, result) in results.iter().enumerate() {
        let type_icon = match result.search_type {
            SearchType::Keyword => "🔍",
            SearchType::Semantic => "🧠",
            SearchType::Hybrid => "🔗",
        };

        let role_icon = match result.role.as_str() {
            "user" => "👤",
            "assistant" => "🤖",
            "system" => "⚙️",
            "tool" => "🔧",
            _ => "📝",
        };

        println!(
            "{}. {} {} **{}** (score: {:.4})",
            i + 1,
            type_icon,
            role_icon,
            result.role,
            result.score
        );

        // Truncate content for display
        let content = if result.content.len() > 200 {
            format!("{}...", &result.content[..200])
        } else {
            result.content.clone()
        };

        println!("   {}", content.replace('\n', "\n   "));
        println!("   _{}_ _{}_\n", result.conversation_id, result.timestamp.format("%Y-%m-%d %H:%M"));
    }
}

/// Run an interactive search session
pub async fn run_search(
    db: &Database,
    ollama: &Ollama,
    query: &str,
    conversation_id: Option<&str>,
    limit: usize,
) {
    match search_conversations(db, ollama, query, conversation_id, limit).await {
        Ok(results) => {
            display_results(&results);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}