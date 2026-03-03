//! Search command implementation
//!
//! Provides hybrid search functionality for conversation history.

use chrono::{DateTime, Utc};
use ollama_rs::Ollama;
use termimad::print_text;

use crate::db::{Database, SearchResult, SearchType, reciprocal_rank_fusion};
use crate::debug_tools::log_debug;
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
            SearchType::Keyword => "🔍 Keyword",
            SearchType::Semantic => "🧠 Semantic",
            SearchType::Hybrid => "🔗 Hybrid",
        };

        let role_str = match result.role.as_str() {
            "user" => "👤 **User**",
            "assistant" => "🤖 **Assistant**",
            "system" => "⚙️ **System**",
            "tool" => "🔧 **Tool**",
            _ => &format!("📝 **{}**", result.role),
        };

        output.push_str(&format!("{}. {} — {} (score: {:.4})\n", i + 1, type_str, role_str, result.score));

        // Truncate content for display
        let content = if result.content.len() > 300 {
            format!("{}...", &result.content[..300])
        } else {
            result.content.clone()
        };
        
        // Format content with indentation
        output.push_str("```\n");
        for line in content.lines() {
            output.push_str(&format!("  {}\n", line));
        }
        output.push_str("```\n");
        output.push_str(&format!("_{} — {}_\n\n", result.conversation_id, result.timestamp.format("%Y-%m-%d %H:%M")));
    }

    print_text(&output);
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
    log_debug(&format!("Search params:\n  query: \"{}\"\n  conversation_id: {:?}\n  limit: {}", 
        query, conversation_id, limit));
    
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

    // Perform keyword search (BM25)
    log_debug("Running keyword search (BM25)...");
    let keyword_results = match db.search_keyword(query, conversation_id, limit) {
        Ok(results) => {
            log_debug(&format!("Keyword search found {} results", results.len()));
            results
        }
        Err(e) => {
            eprintln!("\x1B[31mError: Keyword search failed: {}\x1B[0m", e);
            Vec::new()
        }
    };

    // Perform semantic search (vector similarity)
    log_debug("Running semantic search (vector)...");
    let semantic_results = match db.search_semantic(&embedding, conversation_id, limit) {
        Ok(results) => {
            log_debug(&format!("Semantic search found {} results", results.len()));
            results
        }
        Err(e) => {
            eprintln!("\x1B[31mError: Semantic search failed: {}\x1B[0m", e);
            Vec::new()
        }
    };

    // Combine with RRF
    log_debug("Combining results with RRF (keyword=0.4, semantic=0.6)...");
    let results = reciprocal_rank_fusion(keyword_results, semantic_results, 0.4, 0.6, limit);
    log_debug(&format!("Final combined results: {}", results.len()));

    // Convert to formatted results
    let formatted: Vec<FormattedResult> = results.into_iter().map(FormattedResult::from).collect();
    display_results(&formatted);
}