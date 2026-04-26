//! Startup verification and semantic dedup for facts
//!
//! Ensures fact consistency through embedding-based similarity checking.
//! Called on startup after fact embedding recovery.
//!
//! # Verification Pipeline
//!
//! 1. Ensure all facts have embeddings (delegate to recovery if needed)
//! 2. Load all facts with embeddings
//! 3. Generate embeddings for all facts (ensures fresh embeddings)
//! 4. Compare all fact pairs with cosine similarity >= threshold
//! 5. Resolve conflicts using same heuristics as FTS5 dedup:
//!    - Duplicate → keep newer, remove older
//!    - Contradiction → keep newer, remove older
//!    - Global-wins-project → remove project fact

use std::collections::HashSet;
use std::sync::Arc;

use super::conflict::is_contradiction;
use super::types::Scope;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::embeddings::cosine_similarity;

/// Threshold for semantic similarity (cosine).
/// 0.90 = very similar, catches paraphrases and translations.
/// Lower values would catch too many false positives;
/// higher values would miss legitimate duplicates.
const SEMANTIC_DEDUP_THRESHOLD: f32 = 0.90;

/// Statistics from verification pass
#[derive(Debug, Default)]
pub struct VerifyStats {
    /// Number of duplicate facts removed
    pub duplicates_removed: usize,
    /// Number of contradictory facts resolved
    pub contradictions_resolved: usize,
    /// Number of project facts removed because a global fact exists
    pub global_wins: usize,
    /// Total facts checked
    pub facts_checked: usize,
    /// Number of embeddings generated during this pass
    #[allow(dead_code)] // Used for logging in callers
    pub embeddings_generated: usize,
}

/// Verify and dedup all facts using embeddings.
///
/// Called on REPL startup after `recover_missing_fact_embeddings()`.
/// Compares all fact pairs with semantic similarity >= threshold,
/// resolving duplicates and contradictions.
///
/// # Arguments
/// * `db` - Database connection
/// * `client` - Embedding client for generating embeddings
///
/// # Returns
/// Statistics about what was found and removed
pub async fn verify_and_dedup_facts(
    db: &Arc<Database>,
    client: &Arc<EmbeddingClient>,
) -> VerifyStats {
    // Step 1: Ensure all facts have embeddings (recovery handles this,
    // but we do it here too in case verification is called independently)
    let recovered = super::recovery::recover_missing_fact_embeddings(db, client).await;

    let mut stats = VerifyStats {
        embeddings_generated: recovered,
        ..Default::default()
    };

    // Step 2: Load all valid facts
    let all_facts = match db.list_facts(None, None, None) {
        Ok(facts) => facts,
        Err(e) => {
            log::warn!("Failed to load facts for verification: {}", e);
            return stats;
        }
    };

    stats.facts_checked = all_facts.len();
    if all_facts.len() < 2 {
        return stats;
    }

    // Step 3: Generate embeddings for all facts
    let mut fact_embeddings: Vec<(i64, Vec<f32>, Scope)> = Vec::new();
    for fact in &all_facts {
        match super::embedding::generate_fact_embedding(&fact.content, client).await {
            Ok(emb) => {
                fact_embeddings.push((fact.id, emb, fact.scope));
            }
            Err(e) => {
                log::warn!("Could not generate embedding for fact {}: {}", fact.id, e);
                continue;
            }
        }
    }

    // Step 4: Pair-wise comparison (O(n²) but n is typically < 100)
    let mut to_remove: HashSet<i64> = HashSet::new();

    for i in 0..fact_embeddings.len() {
        if to_remove.contains(&fact_embeddings[i].0) {
            continue; // Already marked for removal
        }

        for j in (i + 1)..fact_embeddings.len() {
            if to_remove.contains(&fact_embeddings[j].0) {
                continue; // Already marked for removal
            }

            let sim = cosine_similarity(&fact_embeddings[i].1, &fact_embeddings[j].1);

            if sim >= SEMANTIC_DEDUP_THRESHOLD {
                // Find the full fact objects for conflict detection
                let fact_i = all_facts.iter().find(|f| f.id == fact_embeddings[i].0);
                let fact_j = all_facts.iter().find(|f| f.id == fact_embeddings[j].0);

                let (Some(fi), Some(fj)) = (fact_i, fact_j) else {
                    continue;
                };

                if is_contradiction(&fi.content, &fj.content) {
                    // Contradiction: newer wins (higher id = newer)
                    stats.contradictions_resolved += 1;
                    let loser_id = if fi.id < fj.id { fi.id } else { fj.id };
                    to_remove.insert(loser_id);
                } else {
                    // Duplicate: apply global-wins-project rule
                    if fi.scope == Scope::Global && fj.scope == Scope::Project {
                        to_remove.insert(fj.id);
                        stats.global_wins += 1;
                    } else if fj.scope == Scope::Global && fi.scope == Scope::Project {
                        to_remove.insert(fi.id);
                        stats.global_wins += 1;
                    } else {
                        // Same scope duplicate: keep newer
                        stats.duplicates_removed += 1;
                        let loser_id = if fi.id < fj.id { fi.id } else { fj.id };
                        to_remove.insert(loser_id);
                    }
                }
            }
        }
    }

    // Step 5: Remove marked facts
    let total_removed = to_remove.len();
    for id in &to_remove {
        if let Err(e) = db.delete_fact(*id) {
            log::warn!("Failed to delete duplicate fact {}: {}", id, e);
        }
    }

    if total_removed > 0 {
        log::info!(
            "Fact verification: removed {} duplicates ({} semantic, {} contradictions, {} global-wins-project)",
            total_removed,
            stats.duplicates_removed,
            stats.contradictions_resolved,
            stats.global_wins,
        );
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_dedup_threshold() {
        // Verify threshold is sensible
        assert!(SEMANTIC_DEDUP_THRESHOLD >= 0.8);
        assert!(SEMANTIC_DEDUP_THRESHOLD <= 0.95);
    }

    #[test]
    fn test_verify_stats_default() {
        let stats = VerifyStats::default();
        assert_eq!(stats.duplicates_removed, 0);
        assert_eq!(stats.contradictions_resolved, 0);
        assert_eq!(stats.global_wins, 0);
        assert_eq!(stats.facts_checked, 0);
        assert_eq!(stats.embeddings_generated, 0);
    }
}
