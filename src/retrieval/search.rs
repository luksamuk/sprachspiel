//! Search command implementation
//!
//! Provides hybrid search functionality for conversation history.
//! Uses the unified content_items table (V7 architecture).
//!
//! # Architecture
//!
//! `run_search()` returns a `SearchOutcome` enum instead of printing directly.
//! Callers (like `handle_search()`) convert the outcome to `CommandOutput`
//! for rendering via `ChatView`. This separation keeps search logic independent of rendering.

use chrono::{DateTime, Utc};

use crate::consts::roles::format_role_label;
use crate::content::{ContentSearchResult, ContentSearchType};
use crate::db::Database;

use crate::embeddings::EmbeddingClient;

/// Search result with formatted output
pub struct FormattedResult {
    pub item_id: i64,
    pub conversation_id: Option<String>,
    pub content_type: String,
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
            content_type: result.item.content_type.to_string(),
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

/// Outcome of a search operation.
///
/// Returns data instead of printing, enabling callers to render
/// via `RatatuiView` (TUI chat) or the standalone renderer (non-chat subcommands).
pub enum SearchOutcome {
    /// Search completed successfully with results (may be empty)
    Results(Vec<FormattedResult>),
    /// Failed to generate embedding for the query
    EmbeddingError(String),
    /// Hybrid search query failed
    SearchError(String),
    /// Enrichment partially failed — partial results still available
    EnrichmentWarning {
        partial_results: Vec<FormattedResult>,
        error: String,
    },
}

/// Human label for a search result row, by content type (LUC-141).
///
/// Messages delegate to the centralized role labels (`consts/roles.rs`) —
/// do NOT hardcode role labels here again (AGENTS.md string-duplication
/// rule). Documents and notes get their own type labels. Unknown content
/// types are passed through as-is.
pub fn subject_label(content_type: &str, role: Option<&str>) -> String {
    match content_type {
        "message" => format_role_label(role.unwrap_or("unknown")),
        "document" => "📄 Document".to_string(),
        "note" => "📝 Note".to_string(),
        other => other.to_string(),
    }
}

/// Format search results as a markdown string.
///
/// Returns a formatted markdown string suitable for rendering via
/// `ChatView::show_command_output()` or `print_markdown()`.
/// Returns `None` if results are empty (caller decides what to display).
pub fn format_results(results: &[FormattedResult]) -> Option<String> {
    if results.is_empty() {
        return None;
    }

    let mut output = String::new();
    output.push_str(&format!("**Search Results** ({} found)\n\n", results.len()));

    for (i, result) in results.iter().enumerate() {
        let type_str = match result.search_type {
            ContentSearchType::Keyword => "🔍 Keyword",
            ContentSearchType::Semantic => "🧠 Semantic",
            ContentSearchType::Hybrid => "🔗 Hybrid",
        };

        let label = subject_label(&result.content_type, result.role.as_deref());

        let conv_id = result
            .conversation_id
            .as_deref()
            .unwrap_or("project-scoped");

        output.push_str(&format!(
            "{}. [id={}] {} — {} (score: {:.4})\n",
            i + 1,
            result.item_id,
            type_str,
            label,
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

    Some(output)
}

/// Run a search and return results as data.
///
/// Returns a `SearchOutcome` enum instead of printing directly.
/// Callers convert the outcome to `CommandOutput` for rendering via `ChatView`.
#[expect(clippy::too_many_arguments)] // 10 args — search pipeline context (db, provider,
// model config, query scoping). Existing callers pass them positionally; grouping
// into a struct is deferred until a second call site exists.
pub async fn run_search(
    db: &Database,
    provider: &crate::provider::OpenAICompatibleProvider,
    embedding_model_id: &str,
    embedding_dimensions: u32,
    embedding_prefix: &str,
    embedding_context_length: Option<u32>,
    query: &str,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    limit: usize,
) -> SearchOutcome {
    // Debug: Show search parameters
    log::debug!(
        "Search params:\n  query: \"{}\"\n  conversation_id: {:?}\n  limit: {}",
        query,
        conversation_id,
        limit
    );

    // Generate embedding for query
    let embedding_client = EmbeddingClient::with_model(
        provider.clone(),
        embedding_model_id.to_string(),
        embedding_dimensions,
        embedding_prefix.to_string(),
        embedding_context_length,
    );

    log::debug!("Generating embedding for query...");
    let query_result = match embedding_client.embed(query).await {
        Ok(result) => {
            log::debug!("Embedding generated ({} dimensions)", result.vector.len());
            result
        }
        Err(e) => {
            return SearchOutcome::EmbeddingError(format!("Failed to generate embedding: {}", e));
        }
    };
    let embedding = query_result.vector;
    let query_norm_correction = query_result.norm_correction;

    // Perform hybrid search using content_items (V7)
    log::debug!("Running hybrid search on content_items...");
    let settings = crate::settings::Settings::load();
    let keyword_weight = settings.indexing.keyword_weight;
    let semantic_weight = settings.indexing.semantic_weight;
    // LUC-141: search ALL content types (messages, notes, documents) —
    // not just messages. Scoping: session messages (conversation_id filter)
    // plus same-project project-scoped content (coupled filter in db layer).
    // feedback_settings stays None: no feedback boost and no on_content_access
    // from /search — keep None unless ADR-008/009 doc-side reinforcement is
    // designed.
    let params = crate::content::ContentSearchParams {
        query,
        embedding: &embedding,
        query_norm_correction,
        content_type: None,
        conversation_id,
        project_id,
        scope: None,
        limit: limit * 2,
        keyword_weight,
        semantic_weight,
        feedback_settings: None,
    };
    let results = match db.search_content_hybrid(&params) {
        Ok(r) => {
            log::debug!("Hybrid search found {} results", r.len());
            r
        }
        Err(e) => {
            return SearchOutcome::SearchError(format!("Hybrid search failed: {}", e));
        }
    };

    // Enrich results with assistant responses
    log::debug!("Enriching results with assistant responses...");
    let enriched_results = match db.enrich_content_results_with_context(results) {
        Ok(r) => {
            let enriched_count = r.iter().filter(|res| res.chunk_content.is_some()).count();
            log::debug!("Enriched {} results", enriched_count);
            r
        }
        Err(e) => {
            // Enrichment failed — return partial (unenriched) results with a warning
            log::warn!("Failed to enrich results: {}", e);
            return SearchOutcome::EnrichmentWarning {
                partial_results: vec![],
                error: e.to_string(),
            };
        }
    };

    // Convert to formatted results
    let formatted: Vec<FormattedResult> = enriched_results.into_iter().map(|r| r.into()).collect();

    SearchOutcome::Results(formatted)
}

#[cfg(test)]
mod tests {
    use super::subject_label;

    #[test]
    fn test_subject_label_by_content_type() {
        assert_eq!(subject_label("message", Some("user")), "👤 User");
        assert_eq!(subject_label("message", Some("assistant")), "🤖 Assistant");
        assert_eq!(subject_label("message", None), "unknown");
        assert_eq!(subject_label("document", None), "📄 Document");
        assert_eq!(subject_label("note", None), "📝 Note");
    }
}
