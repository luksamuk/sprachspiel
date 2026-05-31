//! Database operations for facts
//!
//! Provides CRUD operations and search functionality for the Factual Memory System.

use chrono::{DateTime, Utc};
use rusqlite::{Result, params};
use std::str::FromStr;

use super::decay::should_prune;
use super::types::{Category, Fact, Scope, Source};
use crate::db::Database;
use crate::db::WhereBuilder;
use crate::db::blob_to_f32_vec;

/// Escape a string for FTS5 MATCH queries.
fn fts5_escape(query: &str) -> String {
    let escaped = query.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

/// Normalize a BM25 score to [0, 1) range.
///
/// BM25 scores from FTS5 are negative values where more negative = better match.
/// This function transforms them to [0, 1) where higher = better match.
///
/// Formula: (-score) / (1 - score)
/// - Score -10 (strong match) → 0.91
/// - Score -5 (good match) → 0.83
/// - Score -1 (weak match) → 0.50
/// - Score 0 (no match) → 0.00
fn normalize_bm25_score(score: f32) -> f32 {
    if score >= 0.0 {
        0.0
    } else {
        (-score) / (1.0 - score)
    }
}

/// Search result from facts search
#[derive(Debug, Clone)]
pub struct FactSearchResult {
    /// The fact
    pub fact: Fact,
    /// BM25 score from FTS5 (higher = more relevant)
    pub score: f32,
}

/// Decay statistics from running decay cycle
#[derive(Debug, Clone)]
pub struct DecayStats {
    /// Number of facts pruned
    pub pruned: usize,
    /// Number of facts remaining
    pub remaining: usize,
}

// === SQL Constants ===

const LIST_FACTS_SQL: &str = "
    SELECT id, scope, category, content, importance, access_count,
           decay_score, created_at, last_accessed, source, invalidated_at, project_id,
           has_embedding
    FROM facts";

impl Database {
    /// Insert a new fact
    pub fn insert_fact(&self, fact: &Fact) -> Result<i64> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO facts (scope, category, content, importance, access_count, 
                 decay_score, created_at, last_accessed, source, invalidated_at, project_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    fact.scope.to_string(),
                    fact.category.to_string(),
                    fact.content,
                    fact.importance,
                    fact.access_count as i32,
                    fact.decay_score,
                    fact.created_at.timestamp(),
                    fact.last_accessed.timestamp(),
                    fact.source.to_string(),
                    fact.invalidated_at.map(|t| t.timestamp()),
                    fact.project_id,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a fact by ID
    pub fn get_fact(&self, id: i64) -> Result<Option<Fact>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, scope, category, content, importance, access_count, 
                         decay_score, created_at, last_accessed, source, invalidated_at, project_id,
                         has_embedding
                  FROM facts WHERE id = ?1 AND invalidated_at IS NULL",
            )?;
            let mut rows = stmt.query_map(params![id], |row| {
                Ok(Fact {
                    id: row.get(0)?,
                    scope: Scope::from_str(&row.get::<_, String>(1)?)
                        .map_err(rusqlite::Error::InvalidParameterName)?,
                    category: Category::from_str(&row.get::<_, String>(2)?)
                        .map_err(rusqlite::Error::InvalidParameterName)?,
                    content: row.get(3)?,
                    importance: row.get(4)?,
                    access_count: row.get::<_, i32>(5)? as u32,
                    decay_score: row.get(6)?,
                    created_at: DateTime::from_timestamp(row.get::<_, i64>(7)?, 0)
                        .unwrap_or_else(Utc::now),
                    last_accessed: DateTime::from_timestamp(row.get::<_, i64>(8)?, 0)
                        .unwrap_or_else(Utc::now),
                    source: Source::from_str(&row.get::<_, String>(9)?)
                        .map_err(rusqlite::Error::InvalidParameterName)?,
                    invalidated_at: row
                        .get::<_, Option<i64>>(10)?
                        .map(|t| DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now)),
                    project_id: row.get(11)?,
                    has_embedding: row.get::<_, i32>(12)? != 0,
                })
            })?;
            rows.next().transpose()
        })
    }

    /// Find a fact by exact content match (case-insensitive, trimmed).
    ///
    /// Searches across all scopes for a fact whose content matches exactly
    /// after lowercasing and trimming. Used for deduplication before FTS5 search.
    ///
    /// # Arguments
    /// * `content` - The content to search for (will be compared lowercased and trimmed)
    ///
    /// # Returns
    /// The first matching fact, or None if no exact match is found.
    pub fn find_exact_fact(&self, content: &str) -> Result<Option<Fact>> {
        let normalized = content.trim().to_lowercase();
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, scope, category, content, importance, access_count,
                         decay_score, created_at, last_accessed, source, invalidated_at, project_id,
                         has_embedding
                 FROM facts
                 WHERE LOWER(TRIM(content)) = ?1 AND invalidated_at IS NULL
                 LIMIT 1",
            )?;
            let mut rows = stmt.query_map(params![normalized], row_to_fact)?;
            rows.next().transpose()
        })
    }

    /// Find a fact by normalized content match.
    ///
    /// Compares the `normalize_for_comparison()` output of the candidate
    /// against all existing facts. Detects duplicates like
    /// "I prefer dark mode" ≈ "User prefers dark mode" that exact match misses.
    ///
    /// # Arguments
    /// * `normalized_content` - Output of `normalize_for_comparison()` for the candidate
    ///
    /// # Returns
    /// All facts whose normalized content matches, or empty vec if none found.
    pub fn find_normalized_fact(&self, normalized_content: &str) -> Result<Vec<Fact>> {
        let candidate = normalized_content.trim().to_lowercase();
        let all_facts = self.list_facts(None, None, None)?;
        let matches: Vec<Fact> = all_facts
            .into_iter()
            .filter(|f| {
                let fact_normalized = crate::facts::lang::normalize_for_comparison(&f.content);
                fact_normalized == candidate
            })
            .collect();
        Ok(matches)
    }

    /// Search facts using FTS5 keyword search
    pub fn search_facts(
        &self,
        query: &str,
        scope: Option<Scope>,
        limit: usize,
    ) -> Result<Vec<FactSearchResult>> {
        let escaped_query = fts5_escape(query);

        self.with_connection(|conn| {
            let mut results = Vec::new();

            if let Some(s) = scope {
                let sql = "SELECT f.id, f.scope, f.category, f.content, f.importance, f.access_count, \
                            f.decay_score, f.created_at, f.last_accessed, f.source, f.invalidated_at, \
                            f.project_id, f.has_embedding, bm25(facts_fts) as score \
                     FROM facts_fts fts \
                     JOIN facts f ON fts.rowid = f.id \
                     WHERE facts_fts MATCH ?1 AND f.scope = ?2 AND f.invalidated_at IS NULL \
                     ORDER BY score ASC \
                     LIMIT ?3";

                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(params![escaped_query, s.to_string(), limit as i32], |row| {
                    let score: f32 = row.get(13)?;
                    Ok(FactSearchResult {
                        fact: row_to_fact(row)?,
                        score: normalize_bm25_score(score),
                    })
                })?;

                for r in rows {
                    results.push(r?);
                }
            } else {
                let sql = "SELECT f.id, f.scope, f.category, f.content, f.importance, f.access_count, \
                            f.decay_score, f.created_at, f.last_accessed, f.source, f.invalidated_at, \
                            f.project_id, f.has_embedding, bm25(facts_fts) as score \
                     FROM facts_fts fts \
                     JOIN facts f ON fts.rowid = f.id \
                     WHERE facts_fts MATCH ?1 AND f.invalidated_at IS NULL \
                     ORDER BY score ASC \
                     LIMIT ?2";

                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(params![escaped_query, limit as i32], |row| {
                    let score: f32 = row.get(13)?;
                    Ok(FactSearchResult {
                        fact: row_to_fact(row)?,
                        score: normalize_bm25_score(score),
                    })
                })?;

                for r in rows {
                    results.push(r?);
                }
            }

            Ok(results)
        })
    }

    /// List facts with optional filtering
    pub fn list_facts(
        &self,
        scope: Option<Scope>,
        category: Option<Category>,
        project_id: Option<&str>,
    ) -> Result<Vec<Fact>> {
        self.with_connection(|conn| {
            let mut builder = WhereBuilder::new();
            builder
                .add("invalidated_at IS NULL")
                .add_option("scope = ?", scope.map(|s| s.to_string()))
                .add_option("category = ?", category.map(|c| c.to_string()))
                .add_option_str("project_id = ?", project_id);

            let sql = format!(
                "{} {} ORDER BY created_at DESC",
                LIST_FACTS_SQL.trim(),
                builder.build_where()
            );
            let params = builder.into_params();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_fact)?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }

            Ok(results)
        })
    }

    /// Delete a fact by ID (also removes associated embedding)
    pub fn delete_fact(&self, id: i64) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM facts WHERE id = ?1", params![id])?;
            // Also remove the fact embedding from vec0
            conn.execute(
                "DELETE FROM fact_embeddings WHERE fact_id = ?1",
                params![id],
            )?;
            Ok(())
        })
    }

    /// Run decay cycle and prune old facts
    pub fn run_decay_cycle(&self) -> Result<DecayStats> {
        let now = Utc::now();

        // Get all facts
        let facts = self.list_facts(None, None, None)?;

        // Find facts to prune
        let facts_to_prune: Vec<i64> = facts
            .iter()
            .filter(|f| should_prune(f, now))
            .map(|f| f.id)
            .collect();

        let pruned = facts_to_prune.len();

        // Delete pruned facts
        self.with_connection(|conn| {
            for id in &facts_to_prune {
                conn.execute("DELETE FROM facts WHERE id = ?1", params![id])?;
            }
            Ok(())
        })?;

        // Update decay scores for remaining facts
        let remaining = facts.len() - pruned;

        Ok(DecayStats { pruned, remaining })
    }

    /// Get facts for the system prompt.
    ///
    /// Returns facts that should be injected into the system prompt:
    /// - Global facts (scope = global)
    /// - Project facts (scope = project AND project_id matches)
    ///
    /// Ordered by: preferences first, then facts, by creation date.
    pub fn get_facts_for_prompt(&self, project_id: Option<&str>) -> Result<Vec<Fact>> {
        self.with_connection(|conn| {
            let mut results = Vec::new();

            // Get all non-invalidated facts
            let sql = match project_id {
                Some(_pid) => {
                    // Get global facts + project facts
                    "SELECT id, scope, category, content, importance, access_count, 
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id,
                            has_embedding
                     FROM facts WHERE (scope = 'global' OR project_id = ?1) 
                     AND invalidated_at IS NULL ORDER BY 
                     CASE WHEN category = 'preference' THEN 0 ELSE 1 END, 
                     created_at DESC"
                }
                None => {
                    // Get only global facts
                    "SELECT id, scope, category, content, importance, access_count, 
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id,
                            has_embedding
                     FROM facts WHERE scope = 'global' 
                     AND invalidated_at IS NULL ORDER BY 
                     CASE WHEN category = 'preference' THEN 0 ELSE 1 END, 
                     created_at DESC"
                }
            };

            let mut stmt = conn.prepare(sql)?;

            let rows = match project_id {
                Some(pid) => stmt.query_map(params![pid], row_to_fact)?,
                None => stmt.query_map(params![], row_to_fact)?,
            };

            for r in rows {
                results.push(r?);
            }

            Ok(results)
        })
    }

    /// Store a fact embedding and mark has_embedding = 1.
    ///
    /// Inserts the embedding into the fact_embeddings vec0 table and updates
    /// the fact's has_embedding flag. Uses DELETE + INSERT because vec0 virtual
    /// tables do not support INSERT OR REPLACE (UNIQUE constraint violation).
    /// This makes re-embedding safe: if a fact already has an embedding, the
    /// old row is deleted before the new one is inserted.
    ///
    /// `norm_correction` is stored as a FLOAT auxiliary column in the vec0 table.
    /// It represents `1/(norm²)` for the truncated embedding, used to correct
    /// cosine similarity at query time.
    pub fn update_fact_embedding(
        &self,
        fact_id: i64,
        embedding: &[f32],
        scope: &str,
        category: &str,
        project_id: Option<&str>,
        norm_correction: f32,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let embedding_bytes = crate::db::embedding_to_le_bytes(embedding);
            let norm_correction_f64 = f64::from(norm_correction);

            // DELETE first: vec0 does not support INSERT OR REPLACE.
            // If the fact already has an embedding, the old row must be removed
            // before inserting the new one, otherwise the UNIQUE constraint on
            // fact_id (PRIMARY KEY) would cause the INSERT to fail.
            conn.execute(
                "DELETE FROM fact_embeddings WHERE fact_id = ?1",
                params![fact_id],
            )?;

            conn.execute(
                "INSERT INTO fact_embeddings (fact_id, embedding, scope, category, project_id, norm_correction)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    fact_id,
                    embedding_bytes.as_slice(),
                    scope,
                    category,
                    project_id,
                    norm_correction_f64,
                ],
            )?;

            conn.execute(
                "UPDATE facts SET has_embedding = 1 WHERE id = ?1",
                params![fact_id],
            )?;

            Ok(())
        })
    }

    /// Search facts by embedding similarity (semantic search) with norm correction.
    ///
    /// Uses vec0 KNN search to find facts with the most similar embeddings.
    /// Returns results sorted by cosine similarity (highest first).
    /// The `scope` parameter filters by scope using a WHERE clause.
    ///
    /// When embeddings are truncated from higher dimensions, cosine similarity
    /// underestimates true similarity. `query_norm_correction` compensates:
    /// `corrected = (1 - distance) * sqrt(query_nc * result_nc)`.
    #[allow(dead_code)] // Used by future semantic search features
    pub fn search_facts_semantic(
        &self,
        embedding: &[f32],
        query_norm_correction: f32,
        scope: Option<Scope>,
        limit: usize,
    ) -> Result<Vec<FactSearchResult>> {
        self.with_connection(|conn| {
            let embedding_bytes = crate::db::embedding_to_le_bytes(embedding);
            let mut results = Vec::new();

            let sql = match scope {
                Some(_) => {
                    "SELECT fe.fact_id, fe.distance, fe.norm_correction, f.id, f.scope, f.category, f.content, f.importance,
                            f.access_count, f.decay_score, f.created_at, f.last_accessed, f.source,
                            f.invalidated_at, f.project_id, f.has_embedding
                     FROM fact_embeddings fe
                     JOIN facts f ON fe.fact_id = f.id
                     WHERE fe.embedding MATCH ? AND fe.k = ?
                     AND f.invalidated_at IS NULL"
                }
                None => {
                    "SELECT fe.fact_id, fe.distance, fe.norm_correction, f.id, f.scope, f.category, f.content, f.importance,
                            f.access_count, f.decay_score, f.created_at, f.last_accessed, f.source,
                            f.invalidated_at, f.project_id, f.has_embedding
                     FROM fact_embeddings fe
                     JOIN facts f ON fe.fact_id = f.id
                     WHERE fe.embedding MATCH ? AND fe.k = ?
                     AND f.invalidated_at IS NULL"
                }
            };

            let mut stmt = conn.prepare(sql)?;

            let rows = stmt.query_map(params![embedding_bytes.as_slice(), limit as i32], |row| {
                let _fact_id: i64 = row.get(0)?;
                let distance: f32 = row.get(1)?;
                let result_nc: f32 = row.get::<_, f64>(2)? as f32;

                // Convert cosine distance to corrected cosine similarity.
                // Apply norm correction for truncated embeddings:
                // corrected = (1 - distance) * sqrt(query_nc * result_nc)
                let raw_similarity = 1.0 - distance;
                let corrected_similarity = raw_similarity * (query_norm_correction * result_nc).sqrt();

                // Read the fact columns starting from column 3 (shifted by norm_correction)
                let fact = Fact {
                    id: row.get(3)?,
                    scope: Scope::from_str(&row.get::<_, String>(4)?)
                        .map_err(rusqlite::Error::InvalidParameterName)?,
                    category: Category::from_str(&row.get::<_, String>(5)?)
                        .map_err(rusqlite::Error::InvalidParameterName)?,
                    content: row.get(6)?,
                    importance: row.get(7)?,
                    access_count: row.get::<_, i32>(8)? as u32,
                    decay_score: row.get(9)?,
                    created_at: DateTime::from_timestamp(row.get::<_, i64>(10)?, 0)
                        .unwrap_or_else(Utc::now),
                    last_accessed: DateTime::from_timestamp(row.get::<_, i64>(11)?, 0)
                        .unwrap_or_else(Utc::now),
                    source: Source::from_str(&row.get::<_, String>(12)?)
                        .map_err(rusqlite::Error::InvalidParameterName)?,
                    invalidated_at: row
                        .get::<_, Option<i64>>(13)?
                        .map(|t| DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now)),
                    project_id: row.get(14)?,
                    has_embedding: row.get::<_, i32>(15)? != 0,
                };

                Ok(FactSearchResult {
                    fact,
                    score: corrected_similarity,
                })
            })?;

            // Filter by scope if specified (can't filter in vec0, so do it in Rust)
            for r in rows {
                let result = r?;
                if let Some(s) = scope {
                    if result.fact.scope == s {
                        results.push(result);
                    }
                } else {
                    results.push(result);
                }
            }

            Ok(results)
        })
    }

    /// Get facts that need embedding generation (has_embedding = 0).
    ///
    /// Returns (id, content) pairs for all active facts without embeddings.
    /// Called by the recovery pipeline on startup to fill in missing embeddings.
    pub fn get_facts_for_reindex(&self) -> Result<Vec<(i64, String)>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, content FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL",
            )?;
            let rows = stmt.query_map([], |row| {
                let id: i64 = row.get(0)?;
                let content: String = row.get(1)?;
                Ok((id, content))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get all fact embedding vectors from the vec0 table
    ///
    /// Returns (fact_id, embedding) pairs for all facts that have
    /// embeddings. Embeddings are stored as FLOAT[256] BLOBs and are
    /// deserialized into Vec<f32>.
    pub fn get_all_fact_embedding_vectors(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT fact_id, embedding FROM fact_embeddings")?;

            let rows = stmt.query_map([], |row| {
                let fact_id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let embedding = blob_to_f32_vec(&blob);
                Ok((fact_id, embedding))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}

/// Helper function to map a row to a Fact
///
/// Expects columns in this order:
/// id, scope, category, content, importance, access_count, decay_score,
/// created_at, last_accessed, source, invalidated_at, project_id, has_embedding
fn row_to_fact(row: &rusqlite::Row) -> Result<Fact> {
    Ok(Fact {
        id: row.get(0)?,
        scope: Scope::from_str(&row.get::<_, String>(1)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        category: Category::from_str(&row.get::<_, String>(2)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        content: row.get(3)?,
        importance: row.get(4)?,
        access_count: row.get::<_, i32>(5)? as u32,
        decay_score: row.get(6)?,
        created_at: DateTime::from_timestamp(row.get::<_, i64>(7)?, 0).unwrap_or_else(Utc::now),
        last_accessed: DateTime::from_timestamp(row.get::<_, i64>(8)?, 0).unwrap_or_else(Utc::now),
        source: Source::from_str(&row.get::<_, String>(9)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        invalidated_at: row
            .get::<_, Option<i64>>(10)?
            .map(|t| DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now)),
        project_id: row.get(11)?,
        has_embedding: row.get::<_, i32>(12)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::types::Category;

    #[test]
    fn test_database_facts_table() {
        let db = Database::in_memory().expect("Failed to create database");

        // Create a test fact
        let fact = Fact::new(
            "Test fact content".to_string(),
            Category::Fact,
            Scope::Project,
            Some("test-project".to_string()),
            Source::User,
        )
        .expect("Failed to create fact");

        // Insert the fact
        let id = db.insert_fact(&fact).expect("Failed to insert fact");
        assert!(id > 0);

        // Get the fact back
        let retrieved = db.get_fact(id).expect("Failed to get fact");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.content, "Test fact content");
        assert!(matches!(retrieved.category, Category::Fact));
    }

    #[test]
    fn test_search_facts() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert test facts
        let fact1 = Fact::new(
            "The project uses SQLite".to_string(),
            Category::Fact,
            Scope::Project,
            Some("test".to_string()),
            Source::User,
        )
        .expect("Failed to create fact");

        let fact2 = Fact::new(
            "I prefer dark mode".to_string(),
            Category::Preference,
            Scope::Global,
            None,
            Source::User,
        )
        .expect("Failed to create fact");

        db.insert_fact(&fact1).expect("Failed to insert fact1");
        db.insert_fact(&fact2).expect("Failed to insert fact2");

        // Search for "SQLite"
        let results = db
            .search_facts("SQLite", None, 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);
        assert!(results[0].fact.content.contains("SQLite"));

        // Search for "prefer"
        let results = db
            .search_facts("prefer", None, 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);
        assert!(results[0].fact.content.contains("prefer"));
    }

    #[test]
    fn test_delete_fact() {
        let db = Database::in_memory().expect("Failed to create database");

        let fact = Fact::new(
            "To be deleted".to_string(),
            Category::Fact,
            Scope::Project,
            None,
            Source::User,
        )
        .expect("Failed to create fact");

        let id = db.insert_fact(&fact).expect("Failed to insert fact");

        // Verify it exists
        assert!(db.get_fact(id).expect("Failed to get fact").is_some());

        // Delete it
        db.delete_fact(id).expect("Failed to delete fact");

        // Verify it's gone
        assert!(db.get_fact(id).expect("Failed to get fact").is_none());
    }

    #[test]
    fn test_list_facts_with_scope() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert global fact
        let global_fact = Fact::new(
            "Global preference".to_string(),
            Category::Preference,
            Scope::Global,
            None,
            Source::User,
        )
        .expect("Failed to create fact");

        // Insert project fact
        let project_fact = Fact::new(
            "Project fact".to_string(),
            Category::Fact,
            Scope::Project,
            Some("my-project".to_string()),
            Source::User,
        )
        .expect("Failed to create fact");

        db.insert_fact(&global_fact)
            .expect("Failed to insert global");
        db.insert_fact(&project_fact)
            .expect("Failed to insert project");

        // List global facts
        let global_facts = db
            .list_facts(Some(Scope::Global), None, None)
            .expect("Failed to list global");
        assert_eq!(global_facts.len(), 1);
        assert!(matches!(global_facts[0].scope, Scope::Global));

        // List project facts
        let project_facts = db
            .list_facts(Some(Scope::Project), None, Some("my-project"))
            .expect("Failed to list project");
        assert_eq!(project_facts.len(), 1);
        assert!(matches!(project_facts[0].scope, Scope::Project));

        // List all facts
        let all_facts = db.list_facts(None, None, None).expect("Failed to list all");
        assert_eq!(all_facts.len(), 2);
    }

    #[test]
    fn test_run_decay_cycle() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a fact that will be pruned (very old with low importance)
        let old_fact = Fact {
            id: 0,
            scope: Scope::Global,
            category: Category::Fact,
            content: "Old fact to prune".to_string(),
            importance: 0.1,
            access_count: 0,
            decay_score: 1.0,
            created_at: chrono::Utc::now() - chrono::Duration::days(365),
            last_accessed: chrono::Utc::now() - chrono::Duration::days(365),
            source: Source::User,
            invalidated_at: None,
            project_id: None,
            has_embedding: false,
        };

        // Insert a fact that will be kept (recent)
        let new_fact = Fact::new(
            "New fact to keep".to_string(),
            Category::Fact,
            Scope::Global,
            None,
            Source::User,
        )
        .expect("Failed to create fact");

        db.insert_fact(&old_fact)
            .expect("Failed to insert old fact");
        db.insert_fact(&new_fact)
            .expect("Failed to insert new fact");

        // Run decay cycle
        let stats = db.run_decay_cycle().expect("Failed to run decay cycle");

        // Old fact should be pruned, new fact should remain
        assert!(stats.pruned >= 1, "At least one fact should be pruned");
        assert!(stats.remaining >= 1, "At least one fact should remain");

        // Verify the new fact still exists
        let remaining = db.list_facts(None, None, None).expect("Failed to list");
        assert!(remaining.iter().any(|f| f.content == "New fact to keep"));
    }

    #[test]
    fn test_get_facts_for_prompt() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert preference
        let pref = Fact::new(
            "I prefer concise responses".to_string(),
            Category::Preference,
            Scope::Global,
            None,
            Source::User,
        )
        .expect("Failed to create preference");

        // Insert fact
        let fact = Fact::new(
            "Project uses Rust".to_string(),
            Category::Fact,
            Scope::Project,
            Some("test-project".to_string()),
            Source::User,
        )
        .expect("Failed to create fact");

        db.insert_fact(&pref).expect("Failed to insert preference");
        db.insert_fact(&fact).expect("Failed to insert fact");

        // Get facts for prompt (with project_id)
        let facts = db
            .get_facts_for_prompt(Some("test-project"))
            .expect("Failed to get facts for prompt");

        // Both global and project facts should be returned
        assert_eq!(facts.len(), 2);

        // Preferences should come first
        assert!(matches!(facts[0].category, Category::Preference));
        assert!(matches!(facts[1].category, Category::Fact));

        // Get facts without project_id (global only)
        let global_only = db.get_facts_for_prompt(None).expect("Failed to get global");
        assert_eq!(global_only.len(), 1);
        assert!(matches!(global_only[0].category, Category::Preference));
    }

    #[test]
    fn test_bm25_normalization() {
        // Test the normalize_bm25_score helper function
        // BM25 scores are negative; more negative = better match
        // Formula: (-score) / (1 - score) maps (-inf, 0] to [0, 1)

        // Strong match (score -10) → ~0.91
        let normalized = super::normalize_bm25_score(-10.0);
        assert!(
            (normalized - 0.909).abs() < 0.01,
            "Expected ~0.91 for score -10, got {}",
            normalized
        );

        // Good match (score -5) → ~0.83
        let normalized = super::normalize_bm25_score(-5.0);
        assert!(
            (normalized - 0.833).abs() < 0.01,
            "Expected ~0.83 for score -5, got {}",
            normalized
        );

        // Weak match (score -1) → 0.50
        let normalized = super::normalize_bm25_score(-1.0);
        assert!(
            (normalized - 0.5).abs() < 0.01,
            "Expected 0.5 for score -1, got {}",
            normalized
        );

        // Very weak match (score -0.5) → ~0.33
        let normalized = super::normalize_bm25_score(-0.5);
        assert!(
            (normalized - 0.333).abs() < 0.01,
            "Expected ~0.33 for score -0.5, got {}",
            normalized
        );

        // No match (score 0) → 0.00
        let normalized = super::normalize_bm25_score(0.0);
        assert!(
            normalized == 0.0,
            "Expected 0.0 for score 0, got {}",
            normalized
        );

        // Edge case: positive score (shouldn't happen with FTS5) → 0.00
        let normalized = super::normalize_bm25_score(1.0);
        assert!(
            normalized == 0.0,
            "Expected 0.0 for positive score, got {}",
            normalized
        );
    }
}
