//! Database operations for facts
//!
//! Provides CRUD operations and search functionality for the Factual Memory System.

use chrono::{DateTime, Utc};
use rusqlite::{params, Result};
use std::str::FromStr;

use super::decay::should_prune;
use super::types::{Category, Fact, Scope, Source};
use crate::db::Database;

/// Escape a string for FTS5 MATCH queries.
fn fts5_escape(query: &str) -> String {
    let escaped = query.replace('"', "\"\"");
    format!("\"{}\"", escaped)
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
                        decay_score, created_at, last_accessed, source, invalidated_at, project_id
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
                })
            })?;
            rows.next().transpose()
        })
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
                            f.project_id, bm25(facts_fts) as score \
                     FROM facts_fts fts \
                     JOIN facts f ON fts.rowid = f.id \
                     WHERE facts_fts MATCH ?1 AND f.scope = ?2 AND f.invalidated_at IS NULL \
                     ORDER BY score ASC \
                     LIMIT ?3";

                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(params![escaped_query, s.to_string(), limit as i32], |row| {
                    let score: f32 = row.get(12)?;
                    let normalized_score = (-score).max(0.0);
                    Ok(FactSearchResult {
                        fact: row_to_fact(row)?,
                        score: normalized_score,
                    })
                })?;

                for r in rows {
                    results.push(r?);
                }
            } else {
                let sql = "SELECT f.id, f.scope, f.category, f.content, f.importance, f.access_count, \
                            f.decay_score, f.created_at, f.last_accessed, f.source, f.invalidated_at, \
                            f.project_id, bm25(facts_fts) as score \
                     FROM facts_fts fts \
                     JOIN facts f ON fts.rowid = f.id \
                     WHERE facts_fts MATCH ?1 AND f.invalidated_at IS NULL \
                     ORDER BY score ASC \
                     LIMIT ?2";

                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(params![escaped_query, limit as i32], |row| {
                    let score: f32 = row.get(12)?;
                    let normalized_score = (-score).max(0.0);
                    Ok(FactSearchResult {
                        fact: row_to_fact(row)?,
                        score: normalized_score,
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
            let mut results = Vec::new();
            
            let sql = match (&scope, &category, &project_id) {
                (Some(_), Some(_), Some(_)) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE scope = ?1 AND category = ?2 AND project_id = ?3 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (Some(_), Some(_), None) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE scope = ?1 AND category = ?2 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (Some(_), None, Some(_)) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE scope = ?1 AND project_id = ?2 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (None, Some(_), Some(_)) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE category = ?1 AND project_id = ?2 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (Some(_), None, None) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE scope = ?1 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (None, Some(_), None) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE category = ?1 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (None, None, Some(_)) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE project_id = ?1 \
                     AND invalidated_at IS NULL ORDER BY created_at DESC",
                (None, None, None) => 
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE invalidated_at IS NULL ORDER BY created_at DESC",
            };

            let mut stmt = conn.prepare(sql)?;
            
            let rows = match (&scope, &category, &project_id) {
                (Some(s), Some(c), Some(p)) => {
                    stmt.query_map(params![s.to_string(), c.to_string(), p], row_to_fact)?
                }
                (Some(s), Some(c), None) => {
                    stmt.query_map(params![s.to_string(), c.to_string()], row_to_fact)?
                }
                (Some(s), None, Some(p)) => {
                    stmt.query_map(params![s.to_string(), p], row_to_fact)?
                }
                (None, Some(c), Some(p)) => {
                    stmt.query_map(params![c.to_string(), p], row_to_fact)?
                }
                (Some(s), None, None) => {
                    stmt.query_map(params![s.to_string()], row_to_fact)?
                }
                (None, Some(c), None) => {
                    stmt.query_map(params![c.to_string()], row_to_fact)?
                }
                (None, None, Some(p)) => {
                    stmt.query_map(params![p], row_to_fact)?
                }
                (None, None, None) => {
                    stmt.query_map(params![], row_to_fact)?
                }
            };

            for r in rows {
                results.push(r?);
            }
            
            Ok(results)
        })
    }

    /// Delete a fact by ID
    pub fn delete_fact(&self, id: i64) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute("DELETE FROM facts WHERE id = ?1", params![id])?;
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
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE (scope = 'global' OR project_id = ?1) \
                     AND invalidated_at IS NULL ORDER BY \
                     CASE WHEN category = 'preference' THEN 0 ELSE 1 END, \
                     created_at DESC"
                }
                None => {
                    // Get only global facts
                    "SELECT id, scope, category, content, importance, access_count, \
                            decay_score, created_at, last_accessed, source, invalidated_at, project_id \
                     FROM facts WHERE scope = 'global' \
                     AND invalidated_at IS NULL ORDER BY \
                     CASE WHEN category = 'preference' THEN 0 ELSE 1 END, \
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
}

/// Helper function to map a row to a Fact
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
}
