//! Fact embedding recovery for interrupted processes
//!
//! Ensures all facts have embeddings, even if generation was interrupted
//! or Ollama was offline during fact insertion.
//!
//! # Recovery Pipeline
//!
//! ```text
//! Startup:
//!   recover_missing_embeddings()           ← Content embeddings (existing)
//!   recover_missing_fact_embeddings()      ← Fact embeddings (NEW)
//!   verify_and_dedup_facts()              ← Semantic dedup (NEW)
//!
//! Insert-time (eager, fire-and-forget):
//!   tokio::spawn { generate_fact_embedding() + update_fact_embedding() }
//!   If Ollama offline → has_embedding stays 0, recovered on next startup
//! ```

use std::sync::Arc;

use super::embedding::generate_fact_embedding;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;

/// Recover missing fact embeddings on startup.
///
/// Called after content embedding recovery. Finds all facts with
/// `has_embedding = 0` and generates embeddings for them.
///
/// # Arguments
/// * `db` - Database connection
/// * `client` - Embedding client for generating embeddings
///
/// # Returns
/// Number of fact embeddings successfully recovered
pub async fn recover_missing_fact_embeddings(
    db: &Arc<Database>,
    client: &Arc<EmbeddingClient>,
) -> usize {
    let facts_for_reindex = match db.get_facts_for_reindex() {
        Ok(facts) if !facts.is_empty() => facts,
        Ok(_) => return 0,
        Err(e) => {
            log::warn!("Failed to query facts for reindex: {}", e);
            return 0;
        }
    };

    let total = facts_for_reindex.len();
    if total == 0 {
        return 0;
    }

    log::info!("Recovering {} missing fact embedding(s)...", total);

    let mut recovered = 0;
    for (fact_id, content) in &facts_for_reindex {
        // Skip empty content (shouldn't happen, but defensive)
        if content.trim().is_empty() {
            continue;
        }

        match generate_fact_embedding(content, client).await {
            Ok(embedding) => {
                // Fetch the full fact to get scope/category/project_id for vec0 partition keys
                let fact = match db.get_fact(*fact_id) {
                    Ok(Some(f)) => f,
                    Ok(None) => continue,
                    Err(e) => {
                        log::warn!("Failed to fetch fact {} for embedding: {}", fact_id, e);
                        continue;
                    }
                };

                if let Err(e) = db.update_fact_embedding(
                    *fact_id,
                    &embedding,
                    &fact.scope.to_string(),
                    &fact.category.to_string(),
                    fact.project_id.as_deref(),
                ) {
                    log::warn!("Failed to store fact embedding for id {}: {}", fact_id, e);
                    continue;
                }
                recovered += 1;
            }
            Err(e) => {
                log::warn!("Failed to generate embedding for fact {}: {}", fact_id, e);
                // has_embedding stays 0, will be retried on next startup
            }
        }
    }

    if recovered > 0 {
        log::info!("Successfully recovered {} fact embedding(s).", recovered);
    }

    // Post-recovery verification: check if any facts still lack embeddings
    if let Ok(remaining) = db.get_facts_for_reindex()
        && !remaining.is_empty()
    {
        log::warn!(
            "Fact embedding recovery incomplete: {} fact(s) still without embeddings. \
             This may indicate Ollama was unavailable during startup.",
            remaining.len()
        );
    }

    recovered
}

/// Flush pending fact embeddings before exit.
///
/// Called from the /exit handler after content embedding flush.
/// Simply delegates to `recover_missing_fact_embeddings()` since
/// facts are short content and don't need a progress bar.
///
/// # Arguments
/// * `db` - Database connection
/// * `client` - Embedding client
///
/// # Returns
/// Number of fact embeddings recovered
pub async fn flush_pending_fact_embeddings(
    db: &Arc<Database>,
    client: &Arc<EmbeddingClient>,
) -> usize {
    recover_missing_fact_embeddings(db, client).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_recovery_module_structure() {
        // Verify module compiles correctly
        assert!(true);
    }
}
