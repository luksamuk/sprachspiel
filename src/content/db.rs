//! Database operations for content items
//!
//! Provides CRUD operations and search for content_items table.

use chrono::{DateTime, Utc};
use rusqlite::{params, Result};
use std::collections::HashMap;
use std::str::FromStr;
use zerocopy::IntoBytes;

use super::types::{
    ContentItem, ContentScope, ContentSearchResult, ContentSearchType, ContentSource, ContentType,
    Note,
};
use crate::db::fts5_escape;
use crate::db::Database;
use crate::db::WhereBuilder;

// === SQL Constants ===
// Extracted from inline to improve maintainability and reduce duplication

const LIST_NOTES_SQL: &str = "
    SELECT id, scope, source, title, content, importance, access_count,
           decay_score, created_at, updated_at, last_accessed, project_id
    FROM content_items";

const SEARCH_NOTES_FTS_SQL: &str = "
    SELECT ci.id, ci.content_type, ci.conversation_id, ci.role, ci.message_type,
           ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source, ci.title,
           ci.content, ci.importance, ci.access_count, ci.decay_score,
           ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding,
           ci.project_id, bm25(content_fts) as score
    FROM content_fts fts
    JOIN content_items ci ON fts.rowid = ci.id";

const SEMANTIC_SEARCH_ITEMS_SQL: &str = "
    SELECT ce.item_id, ce.distance, ci.id, ci.content_type, ci.conversation_id,
           ci.role, ci.message_type, ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source,
           ci.title, ci.content, ci.importance, ci.access_count, ci.decay_score,
           ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding, ci.project_id
    FROM content_embeddings ce
    JOIN content_items ci ON ce.item_id = ci.id";

const SEMANTIC_SEARCH_CHUNKS_SQL: &str = "
    SELECT cc.id, ce.distance, cc.item_id, cc.chunk_index, cc.content, 
           cc.start_offset, cc.end_offset, ci.id, ci.content_type, ci.conversation_id,
           ci.role, ci.message_type, ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source,
           ci.title, ci.content as full_content, ci.importance, ci.access_count, ci.decay_score,
           ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding, ci.project_id
    FROM chunk_embeddings_v2 ce
    JOIN content_chunks cc ON ce.chunk_id = cc.id
    JOIN content_items ci ON cc.item_id = ci.id";

/// Parameters for content hybrid search
#[derive(Debug, Clone)]
pub struct ContentSearchParams<'a> {
    /// Search query
    pub query: &'a str,
    /// Query embedding for semantic search
    pub embedding: &'a [f32],
    /// Filter by content type (None = all types)
    pub content_type: Option<ContentType>,
    /// Filter by conversation ID (for messages)
    pub conversation_id: Option<&'a str>,
    /// Filter by project ID
    pub project_id: Option<&'a str>,
    /// Filter by scope (project or global)
    pub scope: Option<ContentScope>,
    /// Maximum results to return
    pub limit: usize,
    /// Weight for keyword search (BM25)
    pub keyword_weight: f32,
    /// Weight for semantic search (vector)
    pub semantic_weight: f32,
}

/// Normalize BM25 score to [0, 1) range
fn normalize_bm25_score(score: f32) -> f32 {
    if score >= 0.0 {
        0.0
    } else {
        (-score) / (1.0 - score)
    }
}

/// Reciprocal Rank Fusion for content search results
fn content_reciprocal_rank_fusion(
    keyword_results: Vec<ContentSearchResult>,
    semantic_results: Vec<ContentSearchResult>,
    keyword_weight: f32,
    semantic_weight: f32,
    limit: usize,
) -> Vec<ContentSearchResult> {
    use std::collections::HashSet;

    let k = 60.0;
    let mut scores: HashMap<i64, (f32, ContentSearchResult)> = HashMap::new();
    let mut seen_in_keyword: HashSet<i64> = HashSet::new();

    for (rank, result) in keyword_results.into_iter().enumerate() {
        let rank_f = (rank + 1) as f32;
        let rrf_score = keyword_weight / (k + rank_f);
        scores.insert(result.item.id, (rrf_score, result.clone()));
        seen_in_keyword.insert(result.item.id);
    }

    for (rank, result) in semantic_results.into_iter().enumerate() {
        let rank_f = (rank + 1) as f32;
        let rrf_score = semantic_weight / (k + rank_f);

        if let Some((existing_score, existing_result)) = scores.get_mut(&result.item.id) {
            *existing_score += rrf_score;
            existing_result.search_type = ContentSearchType::Hybrid;
        } else {
            scores.insert(result.item.id, (rrf_score, result));
        }
    }

    let mut results: Vec<_> = scores.into_values().collect();
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    results
        .into_iter()
        .map(|(score, mut result)| {
            result.score = score;
            result
        })
        .take(limit)
        .collect()
}

impl Database {
    /// Insert a note into content_items
    pub fn insert_note(&self, note: &Note) -> Result<i64> {
        self.with_connection(|conn| {
            let content_type = ContentType::Note.to_string();
            let scope = note.scope.to_string();
            let source = note.source.to_string();

            conn.execute(
                "INSERT INTO content_items (
                    content_type, scope, source, title, content, importance,
                    access_count, decay_score, created_at, updated_at,
                    last_accessed, has_embedding, project_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12)",
                params![
                    content_type,
                    scope,
                    source,
                    note.title,
                    note.content,
                    note.importance,
                    note.access_count as i32,
                    note.decay_score,
                    note.created_at.timestamp(),
                    note.updated_at.timestamp(),
                    note.last_accessed.timestamp(),
                    note.project_id,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a note by ID
    pub fn get_note(&self, id: i64) -> Result<Option<Note>> {
        self.with_connection(|conn| {
            let sql = "SELECT id, scope, source, title, content, importance, access_count,
                              decay_score, created_at, updated_at, last_accessed, project_id
                       FROM content_items
                       WHERE id = ?1 AND content_type = 'note'";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query_map(params![id], row_to_note)?;
            rows.next().transpose()
        })
    }

    /// List notes with optional filtering
    pub fn list_notes(
        &self,
        scope: Option<ContentScope>,
        project_id: Option<&str>,
    ) -> Result<Vec<Note>> {
        self.with_connection(|conn| {
            let mut builder = WhereBuilder::new();
            builder
                .add("content_type = 'note'")
                .add_option("scope = ?", scope.map(|s| s.to_string()))
                .add_option_str("project_id = ?", project_id);

            let sql = format!(
                "{} {} ORDER BY created_at DESC",
                LIST_NOTES_SQL.trim(),
                builder.build_where()
            );
            let params = builder.into_params();

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_note)?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Update a note
    pub fn update_note(&self, id: i64, title: Option<&str>, content: Option<&str>) -> Result<()> {
        self.with_connection(|conn| {
            let now = Utc::now().timestamp();
            
            match (title, content) {
                (Some(t), Some(c)) => {
                    conn.execute(
                        "UPDATE content_items SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
                        params![t, c, now, id],
                    )?;
                }
                (Some(t), None) => {
                    conn.execute(
                        "UPDATE content_items SET title = ?1, updated_at = ?2 WHERE id = ?3",
                        params![t, now, id],
                    )?;
                }
                (None, Some(c)) => {
                    conn.execute(
                        "UPDATE content_items SET content = ?1, updated_at = ?2 WHERE id = ?3",
                        params![c, now, id],
                    )?;
                }
                (None, None) => {}
            }
            Ok(())
        })
    }

    /// Delete a note by ID
    pub fn delete_note(&self, id: i64) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM content_items WHERE id = ?1 AND content_type = 'note'",
                params![id],
            )?;
            Ok(())
        })
    }

    /// Update a note's embedding in content_embeddings
    pub fn update_note_embedding(
        &self,
        note_id: i64,
        embedding: &[f32],
        project_id: Option<&str>,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let embedding_bytes = embedding.as_bytes();
            let content_type = ContentType::Note.to_string();
            let ts = timestamp.timestamp();

            conn.execute(
                "INSERT INTO content_embeddings (item_id, embedding, content_type, conversation_id, project_id, timestamp)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
                params![
                    note_id,
                    embedding_bytes,
                    content_type,
                    project_id,
                    ts,
                ],
            )?;

            conn.execute(
                "UPDATE content_items SET has_embedding = 1 WHERE id = ?1",
                params![note_id],
            )?;

            Ok(())
        })
    }

    /// Search notes using FTS5 keyword search
    pub fn search_notes_keyword(
        &self,
        query: &str,
        scope: Option<ContentScope>,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ContentSearchResult>> {
        let escaped_query = fts5_escape(query);

        self.with_connection(|conn| {
            let mut builder = WhereBuilder::new();
            builder
                .add("content_fts MATCH ?")
                .add("ci.content_type = 'note'")
                .add_option("ci.scope = ?", scope.map(|s| s.to_string()))
                .add_option_str("ci.project_id = ?", project_id);

            let sql = format!(
                "{} {} ORDER BY score ASC LIMIT ?",
                SEARCH_NOTES_FTS_SQL.trim(),
                builder.build_where()
            );

            let mut params = builder.into_params();
            params.insert(0, Box::new(escaped_query));
            params.push(Box::new(limit as i32));

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                    Ok((row_to_content_item(row)?, row.get::<_, f32>(19)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut results = Vec::new();
            for (item, score) in rows {
                results.push(ContentSearchResult {
                    item,
                    score: normalize_bm25_score(score),
                    search_type: ContentSearchType::Keyword,
                    chunk_content: None,
                    chunk_offsets: None,
                });
            }

            Ok(results)
        })
    }

    /// Search content items using FTS5 keyword search (all content types)
    pub fn search_content_keyword(
        &self,
        query: &str,
        content_type: Option<ContentType>,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        scope: Option<ContentScope>,
        limit: usize,
    ) -> Result<Vec<ContentSearchResult>> {
        let escaped_query = fts5_escape(query);

        self.with_connection(|conn| {
            let mut results = Vec::new();

            let mut conditions = Vec::new();
            conditions.push("content_fts MATCH ?".to_string());

            if let Some(ct) = &content_type {
                conditions.push(format!("ci.content_type = '{}'", ct));
            }
            if let Some(conv_id) = conversation_id {
                conditions.push(format!("ci.conversation_id = '{}'", conv_id));
            }
            if let Some(proj_id) = project_id {
                conditions.push(format!("ci.project_id = '{}'", proj_id));
            }
            if let Some(s) = &scope {
                conditions.push(format!("ci.scope = '{}'", s));
            }

            let where_clause = conditions.join(" AND ");

            let sql = format!(
                "SELECT ci.id, ci.content_type, ci.conversation_id, ci.role, ci.message_type,
                        ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source, ci.title,
                        ci.content, ci.importance, ci.access_count, ci.decay_score,
                        ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding,
                        ci.project_id, bm25(content_fts) as score
                 FROM content_fts fts
                 JOIN content_items ci ON fts.rowid = ci.id
                 WHERE {}
                 ORDER BY score ASC
                 LIMIT ?",
                where_clause
            );

            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![escaped_query, limit as i32], |row| {
                    Ok((row_to_content_item(row)?, row.get::<_, f32>(19)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            for (item, score) in rows {
                results.push(ContentSearchResult {
                    item,
                    score: normalize_bm25_score(score),
                    search_type: ContentSearchType::Keyword,
                    chunk_content: None,
                    chunk_offsets: None,
                });
            }

            Ok(results)
        })
    }

    /// Search content items using vector similarity
    pub fn search_content_semantic(
        &self,
        embedding: &[f32],
        content_type: Option<ContentType>,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        scope: Option<ContentScope>,
        limit: usize,
    ) -> Result<Vec<ContentSearchResult>> {
        self.with_connection(|conn| {
            let embedding_bytes = embedding.as_bytes();

            let fetch_limit = if conversation_id.is_some() || project_id.is_some() {
                limit * 3
            } else {
                limit
            };

            let mut results: Vec<ContentSearchResult> = Vec::new();

            let mut stmt = conn.prepare(SEMANTIC_SEARCH_ITEMS_SQL.trim())?;
            let rows = stmt
                .query_map(params![embedding_bytes, fetch_limit as i32], |row| {
                    let item_id: i64 = row.get(0)?;
                    let distance: f32 = row.get(1)?;
                    let item = ContentItem {
                        id: row.get(2)?,
                        content_type: ContentType::from_str(&row.get::<_, String>(3)?)
                            .map_err(rusqlite::Error::InvalidParameterName)?,
                        conversation_id: row.get(4)?,
                        role: row.get(5)?,
                        message_type: row.get(6)?,
                        previous_item_id: row.get(7)?,
                        prompt_tokens: row.get(8)?,
                        scope: row
                            .get::<_, Option<String>>(9)?
                            .map(|s| ContentScope::from_str(&s))
                            .transpose()
                            .map_err(rusqlite::Error::InvalidParameterName)?,
                        source: row
                            .get::<_, Option<String>>(10)?
                            .map(|s| ContentSource::from_str(&s))
                            .transpose()
                            .map_err(rusqlite::Error::InvalidParameterName)?,
                        title: row.get(11)?,
                        content: row.get(12)?,
                        importance: row.get(13)?,
                        access_count: row.get::<_, i32>(14)? as u32,
                        decay_score: row.get(15)?,
                        created_at: DateTime::from_timestamp(row.get::<_, i64>(16)?, 0)
                            .unwrap_or_else(Utc::now),
                        updated_at: DateTime::from_timestamp(row.get::<_, i64>(17)?, 0)
                            .unwrap_or_else(Utc::now),
                        last_accessed: DateTime::from_timestamp(row.get::<_, i64>(18)?, 0)
                            .unwrap_or_else(Utc::now),
                        has_embedding: row.get::<_, i32>(19)? != 0,
                        project_id: row.get(20)?,
                    };
                    Ok((item_id, item, distance))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            for (_item_id, item, distance) in rows {
                results.push(ContentSearchResult {
                    item,
                    score: distance,
                    search_type: ContentSearchType::Semantic,
                    chunk_content: None,
                    chunk_offsets: None,
                });
            }

            let mut stmt = conn.prepare(SEMANTIC_SEARCH_CHUNKS_SQL.trim())?;
            let rows = stmt
                .query_map(params![embedding_bytes, fetch_limit as i32], |row| {
                    let _chunk_id: i64 = row.get(0)?;
                    let distance: f32 = row.get(1)?;
                    let item_id: i64 = row.get(2)?;
                    let _chunk_index: i32 = row.get(3)?;
                    let chunk_content: String = row.get(4)?;
                    let start_offset: i32 = row.get(5)?;
                    let end_offset: i32 = row.get(6)?;

                    let item = ContentItem {
                        id: row.get(7)?,
                        content_type: ContentType::from_str(&row.get::<_, String>(8)?)
                            .map_err(rusqlite::Error::InvalidParameterName)?,
                        conversation_id: row.get(9)?,
                        role: row.get(10)?,
                        message_type: row.get(11)?,
                        previous_item_id: row.get(12)?,
                        prompt_tokens: row.get(13)?,
                        scope: row
                            .get::<_, Option<String>>(14)?
                            .map(|s| ContentScope::from_str(&s))
                            .transpose()
                            .map_err(rusqlite::Error::InvalidParameterName)?,
                        source: row
                            .get::<_, Option<String>>(15)?
                            .map(|s| ContentSource::from_str(&s))
                            .transpose()
                            .map_err(rusqlite::Error::InvalidParameterName)?,
                        title: row.get(16)?,
                        content: row.get(17)?,
                        importance: row.get(18)?,
                        access_count: row.get::<_, i32>(19)? as u32,
                        decay_score: row.get(20)?,
                        created_at: DateTime::from_timestamp(row.get::<_, i64>(21)?, 0)
                            .unwrap_or_else(Utc::now),
                        updated_at: DateTime::from_timestamp(row.get::<_, i64>(22)?, 0)
                            .unwrap_or_else(Utc::now),
                        last_accessed: DateTime::from_timestamp(row.get::<_, i64>(23)?, 0)
                            .unwrap_or_else(Utc::now),
                        has_embedding: row.get::<_, i32>(24)? != 0,
                        project_id: row.get(25)?,
                    };

                    Ok((
                        item_id,
                        item,
                        distance,
                        Some(chunk_content),
                        Some((start_offset, end_offset)),
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            for (_item_id, item, distance, chunk_content, chunk_offsets) in rows {
                results.push(ContentSearchResult {
                    item,
                    score: distance,
                    search_type: ContentSearchType::Semantic,
                    chunk_content,
                    chunk_offsets,
                });
            }

            let mut best_results: HashMap<i64, ContentSearchResult> = HashMap::new();
            for result in results {
                let entry = best_results.entry(result.item.id);
                match entry {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if result.score < e.get().score {
                            e.insert(result);
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(result);
                    }
                }
            }

            let mut results: Vec<ContentSearchResult> = best_results.into_values().collect();
            results.sort_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(ct) = &content_type {
                results.retain(|r| &r.item.content_type == ct);
            }
            if let Some(conv_id) = conversation_id {
                results.retain(|r| r.item.conversation_id.as_deref() == Some(conv_id));
            }
            if let Some(proj_id) = project_id {
                results.retain(|r| r.item.project_id.as_deref() == Some(proj_id));
            }
            if let Some(s) = &scope {
                results.retain(|r| r.item.scope.as_ref() == Some(s));
            }

            results.truncate(limit);
            Ok(results)
        })
    }

    /// Hybrid search using Reciprocal Rank Fusion
    pub fn search_content_hybrid(
        &self,
        params: &ContentSearchParams<'_>,
    ) -> Result<Vec<ContentSearchResult>> {
        let keyword_results = self.search_content_keyword(
            params.query,
            params.content_type,
            params.conversation_id,
            params.project_id,
            params.scope,
            params.limit * 2,
        )?;

        let semantic_results = self.search_content_semantic(
            params.embedding,
            params.content_type,
            params.conversation_id,
            params.project_id,
            params.scope,
            params.limit * 2,
        )?;

        let mut results = content_reciprocal_rank_fusion(
            keyword_results,
            semantic_results,
            params.keyword_weight,
            params.semantic_weight,
            params.limit * 2,
        );

        results.truncate(params.limit);
        Ok(results)
    }
}

/// Helper to map a row to a Note
fn row_to_note(row: &rusqlite::Row) -> Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        scope: ContentScope::from_str(&row.get::<_, String>(1)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        source: ContentSource::from_str(&row.get::<_, String>(2)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        title: row.get(3)?,
        content: row.get(4)?,
        importance: row.get(5)?,
        access_count: row.get::<_, i32>(6)? as u32,
        decay_score: row.get(7)?,
        created_at: DateTime::from_timestamp(row.get::<_, i64>(8)?, 0).unwrap_or_else(Utc::now),
        updated_at: DateTime::from_timestamp(row.get::<_, i64>(9)?, 0).unwrap_or_else(Utc::now),
        last_accessed: DateTime::from_timestamp(row.get::<_, i64>(10)?, 0).unwrap_or_else(Utc::now),
        project_id: row.get(11)?,
    })
}

/// Helper to map a row to a ContentItem
fn row_to_content_item(row: &rusqlite::Row) -> Result<ContentItem> {
    Ok(ContentItem {
        id: row.get(0)?,
        content_type: ContentType::from_str(&row.get::<_, String>(1)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        conversation_id: row.get(2)?,
        role: row.get(3)?,
        message_type: row.get(4)?,
        previous_item_id: row.get(5)?,
        prompt_tokens: row.get(6)?,
        scope: row
            .get::<_, Option<String>>(7)?
            .map(|s| ContentScope::from_str(&s))
            .transpose()
            .map_err(rusqlite::Error::InvalidParameterName)?,
        source: row
            .get::<_, Option<String>>(8)?
            .map(|s| ContentSource::from_str(&s))
            .transpose()
            .map_err(rusqlite::Error::InvalidParameterName)?,
        title: row.get(9)?,
        content: row.get(10)?,
        importance: row.get(11)?,
        access_count: row.get::<_, i32>(12)? as u32,
        decay_score: row.get(13)?,
        created_at: DateTime::from_timestamp(row.get::<_, i64>(14)?, 0).unwrap_or_else(Utc::now),
        updated_at: DateTime::from_timestamp(row.get::<_, i64>(15)?, 0).unwrap_or_else(Utc::now),
        last_accessed: DateTime::from_timestamp(row.get::<_, i64>(16)?, 0).unwrap_or_else(Utc::now),
        has_embedding: row.get::<_, i32>(17)? != 0,
        project_id: row.get(18)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const MAX_CONTENT: usize = 10000; // MAX_NOTE_CONTENT_SIZE from types.rs

    #[test]
    fn test_insert_and_get_note() {
        let db = Database::in_memory().expect("Failed to create database");

        let note = Note::new(
            "Test note content".to_string(),
            ContentScope::Project,
            Some("test-project".to_string()),
            ContentSource::User,
            Some("Test Title".to_string()),
        )
        .expect("Failed to create note");

        let id = db.insert_note(&note).expect("Failed to insert note");
        assert!(id > 0);

        let retrieved = db.get_note(id).expect("Failed to get note");
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.content, "Test note content");
        assert_eq!(retrieved.title, Some("Test Title".to_string()));
        assert_eq!(retrieved.scope, ContentScope::Project);
        assert_eq!(retrieved.source, ContentSource::User);
    }

    #[test]
    fn test_list_notes() {
        let db = Database::in_memory().expect("Failed to create database");

        let note1 = Note::new(
            "Note 1".to_string(),
            ContentScope::Project,
            Some("project-a".to_string()),
            ContentSource::User,
            None,
        )
        .expect("Failed to create note");

        let note2 = Note::new(
            "Note 2".to_string(),
            ContentScope::Global,
            None,
            ContentSource::User,
            None,
        )
        .expect("Failed to create note");

        db.insert_note(&note1).expect("Failed to insert note1");
        db.insert_note(&note2).expect("Failed to insert note2");

        let all_notes = db.list_notes(None, None).expect("Failed to list notes");
        assert_eq!(all_notes.len(), 2);

        let project_notes = db
            .list_notes(None, Some("project-a"))
            .expect("Failed to list project notes");
        assert_eq!(project_notes.len(), 1);

        let global_notes = db
            .list_notes(Some(ContentScope::Global), None)
            .expect("Failed to list global notes");
        assert_eq!(global_notes.len(), 1);
    }

    #[test]
    fn test_update_note() {
        let db = Database::in_memory().expect("Failed to create database");

        let note = Note::new(
            "Original content".to_string(),
            ContentScope::Project,
            Some("test-project".to_string()),
            ContentSource::User,
            Some("Original Title".to_string()),
        )
        .expect("Failed to create note");

        let id = db.insert_note(&note).expect("Failed to insert note");

        db.update_note(id, Some("New Title"), Some("New content"))
            .expect("Failed to update note");

        let updated = db.get_note(id).expect("Failed to get note").unwrap();
        assert_eq!(updated.title, Some("New Title".to_string()));
        assert_eq!(updated.content, "New content");
    }

    #[test]
    fn test_delete_note() {
        let db = Database::in_memory().expect("Failed to create database");

        let note = Note::new(
            "To be deleted".to_string(),
            ContentScope::Project,
            None,
            ContentSource::User,
            None,
        )
        .expect("Failed to create note");

        let id = db.insert_note(&note).expect("Failed to insert note");
        assert!(db.get_note(id).expect("Failed to get note").is_some());

        db.delete_note(id).expect("Failed to delete note");
        assert!(db.get_note(id).expect("Failed to get note").is_none());
    }

    #[test]
    fn test_search_notes_keyword() {
        let db = Database::in_memory().expect("Failed to create database");

        let note1 = Note::new(
            "Rust programming language".to_string(),
            ContentScope::Project,
            Some("proj-a".to_string()),
            ContentSource::User,
            Some("Rust Notes".to_string()),
        )
        .expect("Failed to create note");

        let note2 = Note::new(
            "Python machine learning".to_string(),
            ContentScope::Global,
            None,
            ContentSource::User,
            Some("Python Notes".to_string()),
        )
        .expect("Failed to create note");

        db.insert_note(&note1).expect("Failed to insert note1");
        db.insert_note(&note2).expect("Failed to insert note2");

        // Search for "Rust"
        let results = db
            .search_notes_keyword("Rust", None, None, 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.title, Some("Rust Notes".to_string()));
        assert!(results[0].item.content.contains("Rust"));

        // Search for "programming"
        let results = db
            .search_notes_keyword("programming", None, None, 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);

        // Search for "machine" (in content, not title)
        let results = db
            .search_notes_keyword("machine", None, None, 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);
        assert!(results[0].item.content.contains("machine"));

        // Search with project filter
        let results = db
            .search_notes_keyword("programming", None, Some("proj-a"), 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);

        // Search with scope filter
        let results = db
            .search_notes_keyword("Python", Some(ContentScope::Global), None, 10)
            .expect("Failed to search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.scope, Some(ContentScope::Global));
    }

    #[test]
    fn test_note_content_validation() {
        // Test max content size
        let long_content = "x".repeat(MAX_CONTENT + 1);
        let result = Note::new(
            long_content,
            ContentScope::Project,
            None,
            ContentSource::User,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds"));

        // Test valid content
        let valid_content = "x".repeat(MAX_CONTENT);
        let result = Note::new(
            valid_content,
            ContentScope::Project,
            None,
            ContentSource::User,
            None,
        );
        assert!(result.is_ok());
    }
}
