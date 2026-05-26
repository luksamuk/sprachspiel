//! Centralized fact deduplication and insertion pipeline
//!
//! This module provides the single source of truth for the layered dedup
//! pipeline used by all three fact insertion paths:
//!
//! 1. `/fact add` CLI command (`handle_fact_add`)
//! 2. LLM tool `fact_add`
//! 3. Auto-extraction (`extract_and_insert_facts` via `insert_fact_with_dedup`)
//!
//! # Pipeline Order
//!
//! 1. **Layer 1**: Exact content match (case-insensitive, trimmed)
//! 2. **Layer 2**: Normalized content match (strips pronouns/subjects)
//! 3. **Layer 3.5**: Semantic embedding similarity (requires embedding client
//!    + extractable triple, cosine ≥ `semantic_threshold` from config)
//!    - Triple-based disambiguation: same predicate + different object → contradiction
//!    - Same triple → duplicate
//!    - Different predicates → polarity opposition fallback (`is_contradiction`)
//! 4. **Layer 3**: FTS5 keyword search with BM25 scoring (≥ `CONFLICT_THRESHOLD`)
//! 5. **Insert**: No conflicts found → insert new fact + synchronous embedding
//!
//! # Behavioral Bugs Fixed by Unification
//!
//! Before this module, the three callers had diverged in behavior:
//!
//! - **Bug #1** (LLM tool): Layer 3.5 ran AFTER Layer 3 instead of before,
//!   causing FTS5 to catch contradictions and skip the preferred semantic path.
//! - **Bug #2** (LLM tool): Used `SEMANTIC_DEDUP_THRESHOLD` (0.90) instead of
//!   `DEFAULT_SEMANTIC_SEARCH_THRESHOLD` (0.70), making semantic search unreachable.
//! - **Bug #3** (LLM tool): No triple-based disambiguation in Layer 3.5 — only
//!   `is_contradiction()` was used, causing false positives.
//! - **Bug #4** (LLM tool): Fire-and-forget embedding (`tokio::spawn`) instead
//!   of synchronous (`await`), causing missing embeddings for subsequent facts.

use super::conflict::{
    CONFLICT_THRESHOLD, Conflict, ConflictType, ResolutionAction,
    detect_conflicts, extract_fact_triple, is_contradiction, resolve_conflict,
};
use super::lang;
use super::types::{Category, Fact, Scope, Source};
use crate::db::Database;
use crate::embeddings::EmbeddingClient;

use std::sync::Arc;

// === Result Types ===

/// The outcome of a dedup-and-insert operation.
///
/// Callers format this into user-facing messages (CLI colors, LLM tool text, etc.).
#[derive(Debug, Clone)]
pub enum DedupResult {
    /// Fact was inserted successfully as a new fact.
    Inserted {
        /// The database-assigned ID of the new fact.
        id: i64,
        /// The category of the inserted fact.
        category: Category,
        /// The scope of the inserted fact.
        scope: Scope,
    },
    /// Fact was skipped because an exact duplicate already exists.
    ExactDuplicate {
        /// The ID of the existing fact.
        existing_id: i64,
        /// The content of the existing fact (for display).
        existing_content: String,
    },
    /// Fact was skipped because a normalized duplicate exists.
    NormalizedDuplicate {
        /// The ID of the existing fact.
        existing_id: i64,
        /// The content of the existing fact (for display).
        existing_content: String,
    },
    /// Fact was skipped because a semantic duplicate exists (cosine ≥ 0.90).
    SemanticDuplicate {
        /// The ID of the existing fact.
        existing_id: i64,
        /// The content of the existing fact (for display).
        existing_content: String,
        /// Cosine similarity score.
        #[allow(dead_code)] // Score useful for debugging and display
        score: f32,
    },
    /// Fact replaced an existing one due to contradiction (preference override).
    Updated {
        /// The database-assigned ID of the replacement fact.
        id: i64,
        /// The content of the old fact that was deleted.
        old_content: String,
        /// The reason for the update (e.g., "preference override", "contradiction").
        reason: UpdateReason,
        /// The category of the inserted fact.
        category: Category,
        /// The scope of the inserted fact.
        scope: Scope,
    },
    /// Fact was skipped due to FTS5 conflict (BM25 ≥ threshold).
    ///
    /// `is_contradiction` indicates whether the conflict was a contradiction
    /// (opposing preference) vs a duplicate (same preference). Callers can
    /// use this for display customization.
    #[allow(dead_code)] // Field used for display in callers
    Fts5Conflict {
        /// The ID of the conflicting fact.
        existing_id: i64,
        /// The content of the conflicting fact (for display).
        existing_content: String,
        /// The conflict type (duplicate or contradiction) as determined by FTS5.
        is_contradiction: bool,
    },
    /// Fact insertion failed due to a validation or database error.
    Error(String),
}

/// Why an existing fact was replaced by a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateReason {
    /// Same predicate, different object (preference override) detected via triple extraction.
    PreferenceOverride,
    /// Polarity opposition detected (like vs hate, negation).
    PolarityContradiction,
    /// FTS5 detected a contradiction (temporal resolution: newer wins).
    Fts5Contradiction,
}

impl std::fmt::Display for UpdateReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateReason::PreferenceOverride => write!(f, "preference override"),
            UpdateReason::PolarityContradiction => write!(f, "contradiction"),
            UpdateReason::Fts5Contradiction => write!(f, "contradiction"),
        }
    }
}

/// Configuration for the dedup pipeline.
///
/// Used to customize behavior per caller (e.g., LLM tool passes `Source::Llm`,
/// CLI passes `Source::User`).
#[derive(Debug, Clone)]
pub struct DedupConfig {
    /// Who is adding this fact (User or Llm).
    pub source: Source,
    /// Whether to eagerly generate and store an embedding after insertion.
    /// Should be `true` for all callers. The embedding client must be provided
    /// for this to take effect.
    pub generate_embedding: bool,
}

impl DedupConfig {
    /// Default config for user-initiated fact additions (CLI /fact add).
    pub fn user() -> Self {
        DedupConfig {
            source: Source::User,
            generate_embedding: true,
        }
    }

    /// Default config for LLM-initiated fact additions (fact_add tool).
    pub fn llm() -> Self {
        DedupConfig {
            source: Source::Llm,
            generate_embedding: true,
        }
    }
}

impl Default for DedupConfig {
    fn default() -> Self {
        Self::user()
    }
}

// === Context struct to reduce parameter sprawl ===

/// Context passed through the dedup pipeline layers.
struct DedupContext<'a> {
    db: &'a Database,
    content: &'a str,
    content_trimmed: &'a str,
    normalized_query: String,
    category: Category,
    scope: Scope,
    project_id: Option<&'a str>,
    config: &'a DedupConfig,
    embedding_client: Option<&'a Arc<EmbeddingClient>>,
    /// Semantic similarity threshold for Layer 3.5 (cosine ≥ threshold triggers disambiguation).
    /// Defaults to `DEFAULT_SEMANTIC_SEARCH_THRESHOLD` (0.70).
    semantic_threshold: f32,
}

// === Layer functions ===

/// Layer 1: Exact content match (case-insensitive, trimmed).
///
/// Returns `Some(DedupResult)` if an exact duplicate is found, `None` to continue.
fn check_exact_match(ctx: &DedupContext<'_>) -> Option<DedupResult> {
    match ctx.db.find_exact_fact(ctx.content_trimmed) {
        Ok(Some(existing)) => {
            log::debug!(
                "dedup: Exact duplicate found (id={}): '{}'",
                existing.id,
                existing.content
            );
            Some(DedupResult::ExactDuplicate {
                existing_id: existing.id,
                existing_content: existing.content.clone(),
            })
        }
        Ok(None) => None,
        Err(e) => {
            log::debug!("dedup: Exact match query failed: {}", e);
            None
        }
    }
}

/// Layer 2: Normalized content match (strips pronouns/subjects).
///
/// Returns `Some(DedupResult)` if a normalized duplicate is found, `None` to continue.
fn check_normalized_match(ctx: &DedupContext<'_>) -> Option<DedupResult> {
    match ctx.db.find_normalized_fact(&ctx.normalized_query) {
        Ok(matches) if !matches.is_empty() => {
            if ctx.scope == Scope::Global {
                resolve_global_normalized(ctx, &matches)
            } else {
                let existing = &matches[0];
                log::debug!(
                    "dedup: Skipping duplicate fact (normalized match): '{}'",
                    ctx.content
                );
                Some(DedupResult::NormalizedDuplicate {
                    existing_id: existing.id,
                    existing_content: existing.content.clone(),
                })
            }
        }
        Ok(_) => None,
        Err(e) => {
            log::debug!("dedup: Normalized match query failed: {}", e);
            None
        }
    }
}

/// Resolve normalized matches for Global-scope facts (global-wins-project rule).
fn resolve_global_normalized(ctx: &DedupContext<'_>, matches: &[Fact]) -> Option<DedupResult> {
    let mut global_match: Option<Fact> = None;
    for fact in matches {
        if fact.scope == Scope::Project {
            log::debug!(
                "dedup: Global fact overrides Project fact (id={}): '{}'",
                fact.id,
                fact.content
            );
            if let Err(e) = ctx.db.delete_fact(fact.id) {
                log::debug!("dedup: Failed to delete Project fact: {}", e);
            }
        } else {
            global_match = Some(fact.clone());
        }
    }
    if let Some(existing) = global_match {
        log::debug!(
            "dedup: Skipping duplicate Global fact (id={}): '{}'",
            existing.id,
            existing.content
        );
        Some(DedupResult::NormalizedDuplicate {
            existing_id: existing.id,
            existing_content: existing.content.clone(),
        })
    } else {
        // All duplicates were Project-scope and removed — fall through to insert
        None
    }
}

/// Layer 3.5: Semantic embedding similarity (contradiction + duplicate).
///
/// Runs BEFORE Layer 3 (FTS5 BM25) because:
/// - Contradictions like "prefer dark mode" vs "prefer light mode" have
///   different normalized strings → Layer 2 skips them → FTS5 BM25 also
///   misses them (low keyword overlap) → only semantic catches them.
/// - Embedding cosine ~0.77 for antonym pairs, above the 0.70 threshold.
/// - Triple-based disambiguation separates contradictions from duplicates.
///
/// Returns `Some(DedupResult)` if a semantic match is found, `None` to continue.
async fn check_semantic_match(ctx: &DedupContext<'_>) -> Option<DedupResult> {
    let client = ctx.embedding_client?;
    let _triple = extract_fact_triple(ctx.content)?;

    match super::embedding::generate_fact_embedding(ctx.content, client).await {
        Ok(result) => {
            let candidate_embedding = result.vector;
            match ctx.db.search_facts_semantic(&candidate_embedding, None, 5) {
                Ok(semantic_results) => resolve_semantic_results(ctx, &semantic_results).await,
                Err(e) => {
                    log::debug!("dedup: Semantic search failed: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::debug!(
                "dedup: Failed to generate embedding for semantic dedup: {}",
                e
            );
            None
        }
    }
}

/// Resolve semantic search results: check each result for triple-based
/// contradictions, duplicates, or polarity opposition.
async fn resolve_semantic_results(
    ctx: &DedupContext<'_>,
    results: &[crate::facts::db::FactSearchResult],
) -> Option<DedupResult> {
    for result in results {
        if result.score < ctx.semantic_threshold {
            continue;
        }

        // Step 1: Triple-based disambiguation
        if let Some(candidate_triple) = extract_fact_triple(ctx.content)
            && let Some(existing_triple) = extract_fact_triple(&result.fact.content)
        {
            if candidate_triple.contradicts(&existing_triple) {
                // Same predicate, different object → contradiction
                log::debug!(
                    "dedup: Semantic contradiction (cosine={:.3}, predicate='{}'): '{}' vs '{}'",
                    result.score,
                    candidate_triple.predicate,
                    ctx.content,
                    result.fact.content
                );
                if let Err(e) = ctx.db.delete_fact(result.fact.id) {
                    log::debug!("dedup: Failed to delete contradicted fact: {}", e);
                    continue;
                }
                return Some(
                    insert_and_return(ctx, UpdateReason::PreferenceOverride, &result.fact.content)
                        .await,
                );
            }
            if candidate_triple.predicate == existing_triple.predicate
                && candidate_triple.object == existing_triple.object
            {
                // Same triple → semantic duplicate
                log::debug!(
                    "dedup: Semantic duplicate (cosine={:.3}): '{}' vs '{}'",
                    result.score,
                    ctx.content,
                    result.fact.content
                );
                return Some(DedupResult::SemanticDuplicate {
                    existing_id: result.fact.id,
                    existing_content: result.fact.content.clone(),
                    score: result.score,
                });
            }
            // Different predicate, different/same object → fall through to is_contradiction()
        }

        // Step 2: Polarity opposition fallback
        if is_contradiction(ctx.content, &result.fact.content) {
            log::debug!(
                "dedup: Polarity contradiction (cosine={:.3}): '{}' vs '{}'",
                result.score,
                ctx.content,
                result.fact.content
            );
            if let Err(e) = ctx.db.delete_fact(result.fact.id) {
                log::debug!("dedup: Failed to delete contradicted fact: {}", e);
                continue;
            }
            return Some(
                insert_and_return(
                    ctx,
                    UpdateReason::PolarityContradiction,
                    &result.fact.content,
                )
                .await,
            );
        }

        // Neither contradiction nor duplicate — related but not conflicting. Continue.
    }
    None
}

/// Layer 3: FTS5 keyword search with BM25 scoring (≥ `CONFLICT_THRESHOLD`).
///
/// Returns `Some(DedupResult)` if FTS5 search finds conflicts, `None` on search failure
/// (falls through to insert).
async fn check_fts5_conflicts(ctx: &DedupContext<'_>) -> DedupResult {
    let scope_for_search = if ctx.scope == Scope::Global {
        Some(Scope::Global)
    } else {
        Some(Scope::Project)
    };

    let search_results = match ctx
        .db
        .search_facts(&ctx.normalized_query, scope_for_search, 5)
    {
        Ok(results) => results,
        Err(e) => {
            log::debug!("dedup: FTS5 search failed: {}", e);
            return do_insert(ctx).await;
        }
    };

    let conflicts = detect_conflicts(ctx.content, &search_results, CONFLICT_THRESHOLD);

    if conflicts.is_empty() {
        return do_insert(ctx).await;
    }

    // Resolve conflicts based on scope
    if ctx.scope == Scope::Global {
        resolve_global_fts5_conflicts(ctx, conflicts).await
    } else {
        resolve_project_fts5_conflicts(ctx, conflicts).await
    }
}

/// Resolve FTS5 conflicts for Global-scope facts (global-wins-project rule).
#[allow(clippy::expect_used)] // remaining_conflicts guaranteed non-empty by is_empty guard in caller
async fn resolve_global_fts5_conflicts(
    ctx: &DedupContext<'_>,
    conflicts: Vec<Conflict>,
) -> DedupResult {
    // Remove all conflicting Project-scope facts first
    let mut remaining_conflicts: Vec<Conflict> = Vec::new();
    for conflict in &conflicts {
        if conflict.existing_fact.scope == Scope::Project {
            log::debug!(
                "dedup: Global fact overrides Project fact (id={}): '{}'",
                conflict.existing_fact.id,
                conflict.existing_fact.content
            );
            if let Err(e) = ctx.db.delete_fact(conflict.existing_fact.id) {
                log::debug!(
                    "dedup: Failed to delete Project fact (id={}): {}",
                    conflict.existing_fact.id,
                    e
                );
            }
        } else {
            remaining_conflicts.push(conflict.clone());
        }
    }

    if remaining_conflicts.is_empty() {
        // All conflicts were Project-scope and removed — proceed to insert
        return do_insert(ctx).await;
    }

    // Resolve remaining Global conflicts
    let conflict = remaining_conflicts
        .into_iter()
        .next()
        .expect("remaining_conflicts is non-empty");
    match resolve_conflict(conflict.clone()) {
        ResolutionAction::Skip => {
            log::debug!(
                "dedup: Skipping duplicate Global fact: {}",
                crate::logging::truncate_for_log(ctx.content, 80)
            );
            DedupResult::Fts5Conflict {
                existing_id: conflict.existing_fact.id,
                existing_content: conflict.existing_fact.content.clone(),
                is_contradiction: matches!(conflict.conflict_type, ConflictType::Contradiction),
            }
        }
        ResolutionAction::Update => {
            if let Err(e) = ctx.db.delete_fact(conflict.existing_fact.id) {
                log::debug!("dedup: Failed to invalidate old fact: {}", e);
                return DedupResult::Error(format!("Failed to delete old fact: {}", e));
            }
            log::debug!(
                "dedup: Updating contradictory Global fact (old: '{}', new: '{}')",
                conflict.existing_fact.content,
                ctx.content
            );
            insert_and_return(
                ctx,
                UpdateReason::Fts5Contradiction,
                &conflict.existing_fact.content,
            )
            .await
        }
        ResolutionAction::Add => do_insert(ctx).await,
    }
}

/// Resolve FTS5 conflicts for Project-scope facts.
async fn resolve_project_fts5_conflicts(
    ctx: &DedupContext<'_>,
    conflicts: Vec<Conflict>,
) -> DedupResult {
    #[allow(clippy::expect_used)] // conflicts non-empty guaranteed by is_empty guard above
    let conflict = conflicts
        .into_iter()
        .next()
        .expect("conflicts is non-empty (is_empty guard above ensures at least one element)");
    match resolve_conflict(conflict.clone()) {
        ResolutionAction::Skip => {
            log::debug!(
                "dedup: Skipping duplicate fact (similarity >= threshold): {}",
                ctx.content
            );
            DedupResult::Fts5Conflict {
                existing_id: conflict.existing_fact.id,
                existing_content: conflict.existing_fact.content.clone(),
                is_contradiction: matches!(conflict.conflict_type, ConflictType::Contradiction),
            }
        }
        ResolutionAction::Update => {
            if let Err(e) = ctx.db.delete_fact(conflict.existing_fact.id) {
                log::debug!("dedup: Failed to invalidate old fact: {}", e);
                return DedupResult::Error(format!("Failed to delete old fact: {}", e));
            }
            log::debug!(
                "dedup: Updating contradictory fact (old: '{}', new: '{}')",
                conflict.existing_fact.content,
                ctx.content
            );
            insert_and_return(
                ctx,
                UpdateReason::Fts5Contradiction,
                &conflict.existing_fact.content,
            )
            .await
        }
        ResolutionAction::Add => do_insert(ctx).await,
    }
}

// === Centralized Pipeline ===

/// Deduplicate a fact against existing facts and insert if no conflict.
///
/// This is the single source of truth for the layered dedup pipeline.
/// All three insertion paths should call this function and then format
/// the `DedupResult` for their specific UI.
///
/// # Arguments
///
/// * `db` — Database connection for queries and insertion
/// * `content` — The fact content (already normalized to third-person storage format)
/// * `category` — Auto-detected or overridden category
/// * `scope` — Global or Project scope
/// * `project_id` — Project ID (required if scope is Project, None for Global)
/// * `config` — Pipeline configuration (source, embedding options)
/// * `embedding_client` — Optional embedding client for Layer 3.5 semantic search
///
/// # Returns
///
/// A `DedupResult` indicating what happened (inserted, skipped, updated, error).
#[allow(clippy::too_many_arguments)] // Pipeline requires all parameters
pub async fn deduplicate_and_insert(
    db: &Database,
    content: &str,
    category: Category,
    scope: Scope,
    project_id: Option<&str>,
    config: &DedupConfig,
    embedding_client: Option<&Arc<EmbeddingClient>>,
    semantic_threshold: f32,
) -> DedupResult {
    let content_trimmed = content.trim().to_lowercase();
    let normalized_query = lang::normalize_for_comparison(content);

    let ctx = DedupContext {
        db,
        content,
        content_trimmed: &content_trimmed,
        normalized_query,
        category,
        scope,
        project_id,
        config,
        embedding_client,
        semantic_threshold,
    };

    // Layer 1: Exact content match (case-insensitive, trimmed)
    if let Some(result) = check_exact_match(&ctx) {
        return result;
    }

    // Layer 2: Normalized content match (strips pronouns/subjects)
    if let Some(result) = check_normalized_match(&ctx) {
        return result;
    }

    // Layer 3.5: Semantic embedding similarity (contradiction + duplicate)
    if let Some(result) = check_semantic_match(&ctx).await {
        return result;
    }

    // Layer 3: FTS5 keyword search with BM25 scoring
    check_fts5_conflicts(&ctx).await
}

/// Insert a fact and return `DedupResult::Updated` (for contradiction replacements).
///
/// When a contradiction is detected, the old fact has already been deleted.
/// This inserts the new fact and wraps the result as `DedupResult::Updated`.
async fn insert_and_return(
    ctx: &DedupContext<'_>,
    reason: UpdateReason,
    old_content: &str,
) -> DedupResult {
    match do_insert(ctx).await {
        DedupResult::Inserted {
            id,
            category,
            scope,
        } => DedupResult::Updated {
            id,
            old_content: old_content.to_string(),
            reason,
            category,
            scope,
        },
        other => other,
    }
}

/// Core insertion: create the `Fact`, insert into DB, and eagerly generate embedding.
///
/// This is the final step of the pipeline, called when no dedup conflicts remain.
async fn do_insert(ctx: &DedupContext<'_>) -> DedupResult {
    let fact = match Fact::new(
        ctx.content.to_string(),
        ctx.category,
        ctx.scope,
        ctx.project_id.map(|s| s.to_string()),
        ctx.config.source,
    ) {
        Ok(f) => f,
        Err(e) => {
            log::debug!("dedup: Fact validation failed: {}", e);
            return DedupResult::Error(format!("Fact validation failed: {}", e));
        }
    };

    let id = match ctx.db.insert_fact(&fact) {
        Ok(id) => id,
        Err(e) => {
            log::debug!("dedup: Failed to insert fact: {}", e);
            return DedupResult::Error(format!("Failed to insert fact: {}", e));
        }
    };

    log::debug!(
        "dedup: Inserted fact #{}: {}",
        id,
        crate::logging::truncate_for_log(ctx.content, 80)
    );

    // Eagerly generate embedding for the newly inserted fact.
    // This MUST be synchronous (await, not fire-and-forget) so that
    // when the next fact's Layer 3.5 search runs, this fact's
    // embedding is already stored in fact_embeddings.
    if ctx.config.generate_embedding
        && let Some(client) = ctx.embedding_client
    {
        match super::embedding::generate_fact_embedding(ctx.content, client).await {
            Ok(result) => {
                if let Err(e) = ctx.db.update_fact_embedding(
                    id,
                    &result.vector,
                    &ctx.scope.to_string(),
                    &ctx.category.to_string(),
                    ctx.project_id,
                    result.norm_correction,
                ) {
                    log::debug!("dedup: Failed to store embedding: {}", e);
                }
            }
            Err(e) => {
                log::debug!(
                    "dedup: Failed to generate embedding for fact #{}: {}",
                    id,
                    e
                );
                // has_embedding stays 0; recovery generates on next startup.
            }
        }
    }

    DedupResult::Inserted {
        id,
        category: ctx.category,
        scope: ctx.scope,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_result_debug_format() {
        let result = DedupResult::ExactDuplicate {
            existing_id: 42,
            existing_content: "User prefers dark mode".to_string(),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ExactDuplicate"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_update_reason_display() {
        assert_eq!(
            UpdateReason::PreferenceOverride.to_string(),
            "preference override"
        );
        assert_eq!(
            UpdateReason::PolarityContradiction.to_string(),
            "contradiction"
        );
        assert_eq!(UpdateReason::Fts5Contradiction.to_string(), "contradiction");
    }

    #[test]
    fn test_dedup_config_defaults() {
        let config = DedupConfig::user();
        assert!(matches!(config.source, Source::User));
        assert!(config.generate_embedding);

        let config = DedupConfig::llm();
        assert!(matches!(config.source, Source::Llm));
        assert!(config.generate_embedding);

        let config = DedupConfig::default();
        assert!(matches!(config.source, Source::User));
    }

    #[test]
    fn test_inserted_result_fields() {
        let result = DedupResult::Inserted {
            id: 1,
            category: Category::Preference,
            scope: Scope::Global,
        };
        if let DedupResult::Inserted {
            id,
            category,
            scope,
        } = result
        {
            assert_eq!(id, 1);
            assert_eq!(category, Category::Preference);
            assert_eq!(scope, Scope::Global);
        } else {
            panic!("Expected Inserted");
        }
    }

    #[test]
    fn test_semantic_duplicate_result_fields() {
        let result = DedupResult::SemanticDuplicate {
            existing_id: 5,
            existing_content: "User prefers dark mode".to_string(),
            score: 0.95,
        };
        if let DedupResult::SemanticDuplicate {
            existing_id,
            existing_content,
            score,
        } = result
        {
            assert_eq!(existing_id, 5);
            assert_eq!(existing_content, "User prefers dark mode");
            assert!((score - 0.95).abs() < f32::EPSILON);
        } else {
            panic!("Expected SemanticDuplicate");
        }
    }
}
