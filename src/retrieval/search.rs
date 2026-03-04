//! Search command implementation
//!
//! Provides hybrid search functionality for conversation history.

use chrono::{DateTime, Utc};
use ollama_rs::Ollama;

use crate::db::{Database, SearchResult, SearchType, reciprocal_rank_fusion};
use crate::debug_tools::log_debug;
use crate::embeddings::EmbeddingClient;
use crate::markdown;

/// Search result with formatted output
pub struct FormattedResult {
    pub message_id: i64,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub chunk_content: Option<String>,
    pub chunk_start: Option<i32>,
    pub chunk_end: Option<i32>,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub search_type: SearchType,
}

impl From<SearchResult> for FormattedResult {
    fn from(result: SearchResult) -> Self {
        FormattedResult {
            message_id: result.message_id,
            conversation_id: result.conversation_id,
            role: result.role,
            content: result.content,
            chunk_content: result.chunk_content,
            chunk_start: result.chunk_start,
            chunk_end: result.chunk_end,
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

        output.push_str(&format!("{}. [id={}] {} — {} (score: {:.4})\n", i + 1, result.message_id, type_str, role_str, result.score));

        // Check if we have chunk content (matched a chunk of a long message)
        let display_content = if let (Some(chunk), Some(start), Some(end)) = 
            (&result.chunk_content, result.chunk_start, result.chunk_end) {
            // Chunk matched - show with ellipsis for context
            let prefix = if start > 0 { "..." } else { "" };
            let suffix = if end < result.content.len() as i32 { "..." } else { "" };
            
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
                format!("{}...", result.content.chars().take(300).collect::<String>())
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
        output.push_str(&format!("_{} — {}_\n\n", result.conversation_id, result.timestamp.format("%Y-%m-%d %H:%M")));
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
    let keyword_results = match db.search_keyword(query, conversation_id, None, limit) {
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
    let semantic_results = match db.search_semantic(&embedding, conversation_id, None, limit) {
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

    // Enrich results with conversation context
    log_debug("Enriching results with assistant responses...");
    let enriched_results = match db.enrich_with_context(results) {
        Ok(r) => {
            let enriched_count = r.iter().filter(|msg| msg.next_message.is_some()).count();
            log_debug(&format!("Enriched {} results with assistant responses", enriched_count));
            r
        }
        Err(e) => {
            eprintln!("\x1B[33mWarning: Failed to enrich results: {}\x1B[0m", e);
            // Return early on error - can't use results after move
            return;
        }
    };

    // Convert to formatted results with context
    let formatted: Vec<FormattedResult> = enriched_results.into_iter().map(|msg| {
        // If this message has a context (assistant response), include it
        let content_with_context = if let Some(ref answer) = msg.next_message {
            format!("{}\n\n--- Assistant Response ---\n{}", msg.content, answer.content)
        } else {
            msg.content.clone()
        };

        FormattedResult {
            message_id: msg.message_id,
            conversation_id: msg.conversation_id,
            role: msg.role,
            content: content_with_context,
            timestamp: DateTime::from_timestamp(msg.timestamp, 0).unwrap_or_else(Utc::now),
            score: msg.score,
            search_type: msg.search_type,
            chunk_content: msg.chunk_content,
            chunk_start: msg.chunk_start,
            chunk_end: msg.chunk_end,
        }
    }).collect();

    display_results(&formatted);
}