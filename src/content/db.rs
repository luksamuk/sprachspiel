//! Database operations for content items
//!
//! Provides CRUD operations and search for content_items table.

#![expect(clippy::print_stderr)] // Database content output
use chrono::{DateTime, Utc};
use rusqlite::{Result, params};
use std::collections::HashMap;
use std::str::FromStr;

use super::document::{Document, FileType};
use super::types::{
    ContentItem, ContentScope, ContentSearchResult, ContentSearchType, ContentSource, ContentType,
    Note,
};
use crate::consts::roles::{ROLE_ASSISTANT, ROLE_USER};
use crate::db::Database;
use crate::db::WhereBuilder;
use crate::db::blob_to_f32_vec;
use crate::db::fts5_escape;

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
           ci.project_id, ci.thinking_content, bm25(content_fts) as score
    FROM content_fts fts
    JOIN content_items ci ON fts.rowid = ci.id";

const SEMANTIC_SEARCH_ITEMS_SQL: &str = "
    SELECT ce.item_id, ce.distance, ce.norm_correction, ci.id, ci.content_type, ci.conversation_id,
           ci.role, ci.message_type, ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source,
           ci.title, ci.content, ci.importance, ci.access_count, ci.decay_score,
           ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding, ci.project_id,
           ci.thinking_content
    FROM content_embeddings ce
    JOIN content_items ci ON ce.item_id = ci.id
    WHERE ce.embedding MATCH ? AND ce.k = ?";

const SEMANTIC_SEARCH_CHUNKS_SQL: &str = "
    SELECT cc.id, ce.distance, ce.norm_correction, cc.item_id, cc.chunk_index, cc.content, 
           cc.start_offset, cc.end_offset, ci.id, ci.content_type, ci.conversation_id,
           ci.role, ci.message_type, ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source,
           ci.title, ci.content as full_content, ci.importance, ci.access_count, ci.decay_score,
           ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding, ci.project_id,
           ci.thinking_content
    FROM chunk_embeddings_v2 ce
    JOIN content_chunks cc ON ce.chunk_id = cc.id
    JOIN content_items ci ON cc.item_id = ci.id
    WHERE ce.embedding MATCH ? AND ce.k = ?";

/// Parameters for content hybrid search
#[derive(Debug, Clone)]
pub struct ContentSearchParams<'a> {
    /// Search query
    pub query: &'a str,
    /// Query embedding for semantic search
    pub embedding: &'a [f32],
    /// Norm correction factor for the query embedding.
    /// Applied as: corrected_similarity = (1 - distance) * sqrt(query_nc * result_nc)
    pub query_norm_correction: f32,
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
    /// Feedback settings for boost and access tracking. None = skip feedback features.
    pub feedback_settings: Option<&'a crate::settings::FeedbackSettings>,
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

    // ============================================================
    // Document CRUD Operations
    // ============================================================

    /// Insert a document into content_items
    pub fn insert_document(&self, document: &Document) -> Result<i64> {
        self.with_connection(|conn| {
            let content_type = ContentType::Document.to_string();
            let scope = document.scope.to_string();
            let source = document.source.to_string();
            let file_type = document.file_type.to_string();

            conn.execute(
                "INSERT INTO content_items (
                    content_type, scope, source, title, content, importance,
                    access_count, decay_score, created_at, updated_at,
                    last_accessed, has_embedding, project_id, filename, file_type, word_count
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?13, ?14, ?15)",
                params![
                    content_type,
                    scope,
                    source,
                    document.title,
                    document.content,
                    document.importance,
                    document.access_count as i32,
                    document.decay_score,
                    document.created_at.timestamp(),
                    document.updated_at.timestamp(),
                    document.last_accessed.timestamp(),
                    document.project_id,
                    document.filename,
                    file_type,
                    document.word_count as i32,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a document by ID
    pub fn get_document(&self, id: i64) -> Result<Option<Document>> {
        self.with_connection(|conn| {
            let sql = "SELECT id, scope, source, title, content, importance, access_count,
                              decay_score, created_at, updated_at, last_accessed, project_id,
                              filename, file_type, word_count
                       FROM content_items
                       WHERE id = ?1 AND content_type = 'document'";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query_map(params![id], row_to_document)?;
            rows.next().transpose()
        })
    }

    /// List documents with optional filtering
    pub fn list_documents(
        &self,
        scope: Option<ContentScope>,
        project_id: Option<&str>,
    ) -> Result<Vec<Document>> {
        self.with_connection(|conn| {
            let mut builder = WhereBuilder::new();
            builder
                .add("content_type = 'document'")
                .add_option("scope = ?", scope.map(|s| s.to_string()))
                .add_option_str("project_id = ?", project_id);

            let sql = format!(
                "SELECT id, scope, source, title, content, importance, access_count,
                        decay_score, created_at, updated_at, last_accessed, project_id,
                        filename, file_type, word_count
                 FROM content_items {} ORDER BY created_at DESC",
                builder.build_where()
            );
            let params = builder.into_params();

            let mut stmt = conn.prepare(&sql)?;
            let rows =
                stmt.query_map(rusqlite::params_from_iter(params.iter()), row_to_document)?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Delete a document by ID
    pub fn delete_document(&self, id: i64) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM content_items WHERE id = ?1 AND content_type = 'document'",
                params![id],
            )?;
            Ok(())
        })
    }

    // ============================================================
    // Content Chunk Operations
    // ============================================================

    /// Get all chunks for a content item
    ///
    /// Returns chunks ordered by chunk_index (ascending).
    pub fn get_content_chunks(&self, item_id: i64) -> Result<Vec<super::types::ContentChunk>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, item_id, chunk_index, content, start_offset, end_offset
                 FROM content_chunks
                 WHERE item_id = ?1
                 ORDER BY chunk_index ASC",
            )?;
            let rows = stmt.query_map(params![item_id], row_to_content_chunk)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get a specific chunk by index
    ///
    /// Returns the chunk at the given index for the item.
    pub fn get_content_chunk(
        &self,
        item_id: i64,
        chunk_index: i32,
    ) -> Result<Option<super::types::ContentChunk>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, item_id, chunk_index, content, start_offset, end_offset
                 FROM content_chunks
                 WHERE item_id = ?1 AND chunk_index = ?2",
            )?;
            let mut rows = stmt.query_map(params![item_id, chunk_index], row_to_content_chunk)?;
            rows.next().transpose()
        })
    }

    /// Count chunks for a content item
    ///
    /// Returns the number of chunks for the item.
    pub fn count_content_chunks(&self, item_id: i64) -> Result<i32> {
        self.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM content_chunks WHERE item_id = ?1",
                params![item_id],
                |row| row.get(0),
            )?;
            Ok(count)
        })
    }

    /// Get content items without embeddings for regeneration
    ///
    /// Returns (item_id, content_type, content) for items that need embedding generation.
    /// Used during migration to regenerate embeddings from content.
    ///
    /// Filters out items with content shorter than `MIN_EMBED_CONTENT_LEN`
    /// (10 bytes) or empty content — these produce embeddings with too
    /// little semantic signal and were previously stuck in an infinite
    /// recovery loop (found by `has_embedding = 0`, skipped, left at
    /// `has_embedding = 0` on every startup).
    pub fn get_content_items_for_reindex(&self) -> Result<Vec<(i64, String, String)>> {
        self.with_connection(|conn| {
            let min_len = crate::embeddings::MIN_EMBED_CONTENT_LEN as i64;
            let mut stmt = conn.prepare(
                "SELECT id, content_type, content FROM content_items 
                 WHERE has_embedding = 0 
                 AND length(content) >= ?1 
                 AND content != ''
                 ORDER BY created_at ASC",
            )?;

            let rows = stmt.query_map(params![min_len], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get all content chunks without embeddings for regeneration
    ///
    /// Returns (chunk_id, content) for chunks that need embedding generation.
    /// Used during migration to regenerate embeddings from chunk content.
    ///
    /// Filters out chunks with content shorter than `MIN_EMBED_CONTENT_LEN`
    /// (10 bytes) or empty content — same rationale as `get_content_items_for_reindex`.
    pub fn get_content_chunks_for_reindex(&self) -> Result<Vec<(i64, String)>> {
        self.with_connection(|conn| {
            let min_len = crate::embeddings::MIN_EMBED_CONTENT_LEN as i64;
            let mut stmt = conn.prepare(
                "SELECT id, content FROM content_chunks 
                 WHERE has_embedding = 0 
                 AND length(content) >= ?1 
                 AND content != ''
                 ORDER BY created_at ASC",
            )?;

            let rows = stmt.query_map(params![min_len], |row| Ok((row.get(0)?, row.get(1)?)))?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Garbage-collect database artifacts.
    ///
    /// Removes:
    /// - Empty assistant messages (artifacts from Ctrl+C cancellation)
    /// - Orphan chunks (chunks whose parent item no longer exists)
    /// - Orphan content embeddings (vec0 rows whose parent item was deleted)
    /// - Orphan chunk embeddings (vec0 rows whose parent chunk was deleted)
    /// - Orphan fact embeddings (vec0 rows whose parent fact was deleted)
    ///
    /// Returns counts of removed items for reporting.
    pub fn garbage_collect(&self) -> Result<GcStats> {
        self.with_connection(|conn| {
            // 1. Delete empty assistant messages.
            // These are artifacts from Ctrl+C cancellation where the stream
            // was interrupted before any tokens were generated. They have no
            // semantic value, confuse the LLM with empty turns, and can never
            // receive embeddings.
            let empty_messages: usize = conn.execute(
                "DELETE FROM content_items
                 WHERE role = ?1
                 AND content = ''
                 AND content_type = 'message'",
                params![ROLE_ASSISTANT],
            )?;

            // 2. Delete orphan chunks (no matching parent item).
            // These can appear if an item was deleted but its chunks weren't
            // cleaned up, or if chunk insertion succeeded but the parent item
            // insertion failed.
            let orphan_chunks: usize = conn.execute(
                "DELETE FROM content_chunks
                 WHERE item_id NOT IN (SELECT id FROM content_items)",
                [],
            )?;

            // 3. Delete orphan content embeddings (item deleted but vec0 row remains).
            // Can happen if a content_item was hard-deleted (e.g. via /content prune)
            // but the associated vec0 row wasn't cleaned up.
            let orphan_item_embeddings: usize = conn.execute(
                "DELETE FROM content_embeddings
                 WHERE item_id NOT IN (SELECT id FROM content_items)",
                [],
            )?;

            // 4. Delete orphan chunk embeddings (chunk deleted but vec0 row remains).
            let orphan_chunk_embeddings: usize = conn.execute(
                "DELETE FROM chunk_embeddings_v2
                 WHERE chunk_id NOT IN (SELECT id FROM content_chunks)",
                [],
            )?;

            // 5. Delete orphan fact embeddings (fact deleted but vec0 row remains).
            let orphan_fact_embeddings: usize = conn.execute(
                "DELETE FROM fact_embeddings
                 WHERE fact_id NOT IN (SELECT id FROM facts)",
                [],
            )?;

            Ok(GcStats {
                empty_messages_removed: empty_messages,
                orphan_chunks_removed: orphan_chunks,
                orphan_item_embeddings_removed: orphan_item_embeddings,
                orphan_chunk_embeddings_removed: orphan_chunk_embeddings,
                orphan_fact_embeddings_removed: orphan_fact_embeddings,
            })
        })
    }
}

/// Statistics returned by [`Database::garbage_collect`].
#[derive(Debug, Clone, Copy, Default)]
pub struct GcStats {
    /// Number of empty assistant messages removed
    pub empty_messages_removed: usize,
    /// Number of orphan chunks removed
    pub orphan_chunks_removed: usize,
    /// Number of orphan content embeddings removed (vec0 rows without parent item)
    pub orphan_item_embeddings_removed: usize,
    /// Number of orphan chunk embeddings removed (vec0 rows without parent chunk)
    pub orphan_chunk_embeddings_removed: usize,
    /// Number of orphan fact embeddings removed (vec0 rows without parent fact)
    pub orphan_fact_embeddings_removed: usize,
}

/// Statistics returned by [`Database::reset_all_embedding_flags`].
///
/// Contains the number of rows affected in each table so the caller can
/// report how many items will be re-indexed.
#[derive(Debug, Clone, Copy)]
pub struct ResetStats {
    /// Number of content_items rows reset (`has_embedding` changed from 1 to 0).
    pub items: usize,
    /// Number of content_chunks rows deleted (chunks are derived data
    /// and will be re-created by `regenerate_all_embeddings`).
    #[allow(dead_code)]
    // Available for logging/diagnostics; chunks are re-created during regeneration
    pub chunks_deleted: usize,
    /// Number of facts rows reset (active facts only, excludes invalidated).
    pub facts: usize,
}

/// Item counts for a session, used in delete preview.
///
/// Shows what will be deleted so the user can make an informed decision
/// before confirming destructive operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionItemCounts {
    pub message_count: i64,
    pub embedding_count: i64,
    pub todo_count: i64,
}

impl Database {
    ///
    /// This is the core operation for `/reindex --yes` — it clears every
    /// embedding from the vec0 tables (`content_embeddings`, `chunk_embeddings_v2`,
    /// `fact_embeddings`), deletes all `content_chunks` rows (chunks are derived
    /// data that will be re-created by `regenerate_all_embeddings`), and resets
    /// `has_embedding = 0` across remaining content tables so that everything
    /// will be re-processed from scratch.
    ///
    /// # Why delete chunks instead of resetting their flags?
    ///
    /// Content chunks are derived from `content_items` via `chunk_text_with_config()`.
    /// If we only reset `has_embedding = 0` on existing chunks, `regenerate_all_embeddings`
    /// would insert *new* chunks for long items (because `insert_content_chunk` doesn't
    /// check for duplicates), creating duplicate entries. Deleting all chunks ensures
    /// a clean slate where regeneration re-creates exactly the right chunks.
    ///
    /// # Execution order
    ///
    /// 1. `DELETE FROM content_embeddings` — remove item-level vec0 embeddings
    /// 2. `DELETE FROM chunk_embeddings_v2` — remove chunk-level vec0 embeddings
    /// 3. `DELETE FROM fact_embeddings` — remove fact-level vec0 embeddings
    /// 4. `DELETE FROM content_chunks` — remove derived chunk rows (will be re-created)
    /// 5. `UPDATE content_items SET has_embedding = 0` — mark items for re-indexing
    /// 6. `UPDATE facts SET has_embedding = 0 WHERE invalidated_at IS NULL` — mark active facts
    ///
    /// Deletes must happen before the flag updates so that concurrent readers
    /// never see `has_embedding = 1` with missing vec0 rows.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the SQL statements fail (e.g., locked DB).
    pub fn reset_all_embedding_flags(&self) -> Result<ResetStats> {
        self.with_connection(|conn| {
            // --- Delete all vec0 embeddings first (before resetting flags) ---
            conn.execute("DELETE FROM content_embeddings", [])?;
            conn.execute("DELETE FROM chunk_embeddings_v2", [])?;
            conn.execute("DELETE FROM fact_embeddings", [])?;

            // --- Delete all content_chunks (derived data, will be re-created) ---
            let chunks_deleted: usize = conn.execute("DELETE FROM content_chunks", [])?;

            // --- Reset has_embedding flags ---
            let items: usize = conn.execute("UPDATE content_items SET has_embedding = 0", [])?;
            let facts: usize = conn.execute(
                "UPDATE facts SET has_embedding = 0 WHERE invalidated_at IS NULL",
                [],
            )?;

            Ok(ResetStats {
                items,
                chunks_deleted,
                facts,
            })
        })
    }

    /// Update content item embedding after generation
    ///
    /// Inserts the embedding into content_embeddings and marks the item as having embedding.
    /// Uses DELETE + INSERT because vec0 virtual tables do not support INSERT OR REPLACE
    /// (UNIQUE constraint on item_id PRIMARY KEY). This makes re-embedding safe.
    ///
    /// `norm_correction` is stored as a FLOAT auxiliary column in the vec0 table.
    /// It represents `1/(norm²)` for the truncated embedding, used to correct
    /// cosine similarity at query time.
    #[expect(clippy::too_many_arguments)] // All parameters needed for vec0 auxiliary columns
    pub fn update_content_item_embedding(
        &self,
        item_id: i64,
        embedding: &[f32],
        content_type: &str,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        timestamp: chrono::DateTime<chrono::Utc>,
        norm_correction: f32,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let embedding_bytes = crate::db::embedding_to_le_bytes(embedding);
            let ts = timestamp.timestamp();
            let norm_correction_f64 = f64::from(norm_correction);

            // DELETE first: vec0 does not support INSERT OR REPLACE.
            // If the item already has an embedding, the old row must be removed
            // before inserting the new one, otherwise the UNIQUE constraint on
            // item_id (PRIMARY KEY) would cause the INSERT to fail.
            conn.execute(
                "DELETE FROM content_embeddings WHERE item_id = ?1",
                params![item_id],
            )?;

            conn.execute(
                "INSERT INTO content_embeddings (item_id, embedding, content_type, conversation_id, project_id, timestamp, norm_correction)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    item_id,
                    embedding_bytes.as_slice(),
                    content_type,
                    conversation_id,
                    project_id,
                    ts,
                    norm_correction_f64,
                ],
            )?;

            conn.execute(
                "UPDATE content_items SET has_embedding = 1 WHERE id = ?1",
                params![item_id],
            )?;

            Ok(())
        })
    }

    /// Update content chunk embedding after generation
    ///
    /// Inserts the embedding into chunk_embeddings_v2 and marks the chunk as having embedding.
    /// Uses DELETE + INSERT because vec0 virtual tables do not support INSERT OR REPLACE
    /// (UNIQUE constraint on chunk_id PRIMARY KEY). This makes re-embedding safe.
    ///
    /// `norm_correction` is stored as a FLOAT auxiliary column in the vec0 table.
    /// It represents `1/(norm²)` for the truncated embedding, used to correct
    /// cosine similarity at query time.
    #[expect(clippy::too_many_arguments)] // All parameters needed for vec0 auxiliary columns
    pub fn update_content_chunk_embedding(
        &self,
        chunk_id: i64,
        embedding: &[f32],
        content_type: &str,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        timestamp: chrono::DateTime<chrono::Utc>,
        norm_correction: f32,
    ) -> Result<()> {
        self.with_connection(|conn| {
            let embedding_bytes = crate::db::embedding_to_le_bytes(embedding);
            let ts = timestamp.timestamp();
            let norm_correction_f64 = f64::from(norm_correction);

            // DELETE first: vec0 does not support INSERT OR REPLACE.
            // If the chunk already has an embedding, the old row must be removed
            // before inserting the new one, otherwise the UNIQUE constraint on
            // chunk_id (PRIMARY KEY) would cause the INSERT to fail.
            conn.execute(
                "DELETE FROM chunk_embeddings_v2 WHERE chunk_id = ?1",
                params![chunk_id],
            )?;

            conn.execute(
                "INSERT INTO chunk_embeddings_v2 (chunk_id, embedding, content_type, conversation_id, project_id, timestamp, norm_correction)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    chunk_id,
                    embedding_bytes.as_slice(),
                    content_type,
                    conversation_id,
                    project_id,
                    ts,
                    norm_correction_f64,
                ],
            )?;

            conn.execute(
                "UPDATE content_chunks SET has_embedding = 1 WHERE id = ?1",
                params![chunk_id],
            )?;

            Ok(())
        })
    }

    /// Get all content embedding vectors from the vec0 table
    ///
    /// Returns (item_id, embedding) pairs for all content items that have
    /// embeddings. Embeddings are stored as FLOAT[<configured_dims>] BLOBs and are
    /// deserialized into Vec<f32>.
    pub fn get_all_content_embedding_vectors(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT item_id, embedding FROM content_embeddings")?;

            let rows = stmt.query_map([], |row| {
                let item_id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let embedding = blob_to_f32_vec(&blob);
                Ok((item_id, embedding))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get all chunk embedding vectors from the vec0 table
    ///
    /// Returns (chunk_id, embedding) pairs for all content chunks that have
    /// embeddings. Embeddings are stored as FLOAT[<configured_dims>] BLOBs and are
    /// deserialized into Vec<f32>.
    pub fn get_all_chunk_embedding_vectors(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT chunk_id, embedding FROM chunk_embeddings_v2")?;

            let rows = stmt.query_map([], |row| {
                let chunk_id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let embedding = blob_to_f32_vec(&blob);
                Ok((chunk_id, embedding))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
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
                    Ok((row_to_content_item(row)?, row.get::<_, f32>(20)?))
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
                    subsequent_items: Vec::new(),
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
                        ci.project_id, ci.thinking_content, bm25(content_fts) as score
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
                    Ok((row_to_content_item(row)?, row.get::<_, f32>(20)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            for (item, score) in rows {
                results.push(ContentSearchResult {
                    item,
                    score: normalize_bm25_score(score),
                    search_type: ContentSearchType::Keyword,
                    chunk_content: None,
                    chunk_offsets: None,
                    subsequent_items: Vec::new(),
                });
            }

            Ok(results)
        })
    }

    /// Search content items using vector similarity with norm correction.
    ///
    /// When embeddings are truncated from higher dimensions, the cosine similarity
    /// of the truncated vectors underestimates the true similarity. The `norm_correction`
    /// factor compensates: `corrected = (1 - distance) * sqrt(nc_query * nc_result)`.
    #[expect(clippy::too_many_arguments)]
    pub fn search_content_semantic(
        &self,
        embedding: &[f32],
        query_norm_correction: f32,
        content_type: Option<ContentType>,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        scope: Option<ContentScope>,
        limit: usize,
    ) -> Result<Vec<ContentSearchResult>> {
        self.with_connection(|conn| {
            let embedding_bytes = crate::db::embedding_to_le_bytes(embedding);

            let fetch_limit = if conversation_id.is_some() || project_id.is_some() {
                limit * 3
            } else {
                limit
            };

            let mut results: Vec<ContentSearchResult> = Vec::new();

            let mut stmt = conn.prepare(SEMANTIC_SEARCH_ITEMS_SQL.trim())?;
            let rows = stmt
                .query_map(
                    params![embedding_bytes.as_slice(), fetch_limit as i32],
                    |row| {
                        let item_id: i64 = row.get(0)?;
                        let distance: f32 = row.get(1)?;
                        let norm_correction: f32 = row.get::<_, f64>(2)? as f32;
                        let item = ContentItem {
                            id: row.get(3)?,
                            content_type: ContentType::from_str(&row.get::<_, String>(4)?)
                                .map_err(rusqlite::Error::InvalidParameterName)?,
                            conversation_id: row.get(5)?,
                            role: row.get(6)?,
                            message_type: row.get(7)?,
                            previous_item_id: row.get(8)?,
                            prompt_tokens: row.get(9)?,
                            scope: row
                                .get::<_, Option<String>>(10)?
                                .map(|s| ContentScope::from_str(&s))
                                .transpose()
                                .map_err(rusqlite::Error::InvalidParameterName)?,
                            source: row
                                .get::<_, Option<String>>(11)?
                                .map(|s| ContentSource::from_str(&s))
                                .transpose()
                                .map_err(rusqlite::Error::InvalidParameterName)?,
                            title: row.get(12)?,
                            content: row.get(13)?,
                            thinking_content: row.get(22)?,
                            importance: row.get(14)?,
                            access_count: row.get::<_, i32>(15)? as u32,
                            decay_score: row.get(16)?,
                            created_at: DateTime::from_timestamp(row.get::<_, i64>(17)?, 0)
                                .unwrap_or_else(Utc::now),
                            updated_at: DateTime::from_timestamp(row.get::<_, i64>(18)?, 0)
                                .unwrap_or_else(Utc::now),
                            last_accessed: DateTime::from_timestamp(row.get::<_, i64>(19)?, 0)
                                .unwrap_or_else(Utc::now),
                            has_embedding: row.get::<_, i32>(20)? != 0,
                            project_id: row.get(21)?,
                        };
                        Ok((item_id, item, distance, norm_correction))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;

            for (_item_id, item, distance, result_nc) in rows {
                // Convert cosine distance to cosine similarity,
                // then apply norm correction for truncated embeddings.
                // corrected = (1 - distance) * sqrt(nc_query * nc_result)
                let raw_similarity = 1.0 - distance;
                let corrected_similarity =
                    raw_similarity * (query_norm_correction * result_nc).sqrt();
                results.push(ContentSearchResult {
                    item,
                    score: corrected_similarity,
                    search_type: ContentSearchType::Semantic,
                    chunk_content: None,
                    chunk_offsets: None,
                    subsequent_items: Vec::new(),
                });
            }

            let mut stmt = conn.prepare(SEMANTIC_SEARCH_CHUNKS_SQL.trim())?;
            let rows = stmt
                .query_map(
                    params![embedding_bytes.as_slice(), fetch_limit as i32],
                    |row| {
                        let _chunk_id: i64 = row.get(0)?;
                        let distance: f32 = row.get(1)?;
                        let norm_correction: f32 = row.get::<_, f64>(2)? as f32;
                        let item_id: i64 = row.get(3)?;
                        let _chunk_index: i32 = row.get(4)?;
                        let chunk_content: String = row.get(5)?;
                        let start_offset: i32 = row.get(6)?;
                        let end_offset: i32 = row.get(7)?;

                        let item = ContentItem {
                            id: row.get(8)?,
                            content_type: ContentType::from_str(&row.get::<_, String>(9)?)
                                .map_err(rusqlite::Error::InvalidParameterName)?,
                            conversation_id: row.get(10)?,
                            role: row.get(11)?,
                            message_type: row.get(12)?,
                            previous_item_id: row.get(13)?,
                            prompt_tokens: row.get(14)?,
                            scope: row
                                .get::<_, Option<String>>(15)?
                                .map(|s| ContentScope::from_str(&s))
                                .transpose()
                                .map_err(rusqlite::Error::InvalidParameterName)?,
                            source: row
                                .get::<_, Option<String>>(16)?
                                .map(|s| ContentSource::from_str(&s))
                                .transpose()
                                .map_err(rusqlite::Error::InvalidParameterName)?,
                            title: row.get(17)?,
                            content: row.get(18)?,
                            thinking_content: row.get(27)?,
                            importance: row.get(19)?,
                            access_count: row.get::<_, i32>(20)? as u32,
                            decay_score: row.get(21)?,
                            created_at: DateTime::from_timestamp(row.get::<_, i64>(22)?, 0)
                                .unwrap_or_else(Utc::now),
                            updated_at: DateTime::from_timestamp(row.get::<_, i64>(23)?, 0)
                                .unwrap_or_else(Utc::now),
                            last_accessed: DateTime::from_timestamp(row.get::<_, i64>(24)?, 0)
                                .unwrap_or_else(Utc::now),
                            has_embedding: row.get::<_, i32>(25)? != 0,
                            project_id: row.get(26)?,
                        };

                        Ok((
                            item_id,
                            item,
                            distance,
                            norm_correction,
                            Some(chunk_content),
                            Some((start_offset, end_offset)),
                        ))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;

            for (_item_id, item, distance, result_nc, chunk_content, chunk_offsets) in rows {
                // Convert cosine distance to cosine similarity,
                // then apply norm correction for truncated embeddings.
                let raw_similarity = 1.0 - distance;
                let corrected_similarity =
                    raw_similarity * (query_norm_correction * result_nc).sqrt();
                results.push(ContentSearchResult {
                    item,
                    score: corrected_similarity,
                    search_type: ContentSearchType::Semantic,
                    chunk_content,
                    chunk_offsets,
                    subsequent_items: Vec::new(),
                });
            }

            let mut best_results: HashMap<i64, ContentSearchResult> = HashMap::new();
            for result in results {
                let entry = best_results.entry(result.item.id);
                match entry {
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if result.score > e.get().score {
                            e.insert(result);
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(result);
                    }
                }
            }

            let mut results: Vec<ContentSearchResult> = best_results.into_values().collect();
            // Sort descending by similarity score (most similar first).
            // This ensures RRF assigns rank 1 to the best match.
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
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
    ///
    /// Applies post-RRF feedback boost when settings are enabled,
    /// and records content access for reinforcement tracking.
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
            params.query_norm_correction,
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

        // Apply feedback boost if enabled
        if let Some(fs) = params.feedback_settings
            && fs.enabled
        {
            let item_ids: Vec<i64> = results.iter().map(|r| r.item.id).collect();
            let boosts = self.with_connection(|conn| {
                crate::db::feedback_ops::compute_feedback_boost(
                    conn,
                    &item_ids,
                    chrono::Utc::now().timestamp(),
                )
                .map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
                })
            })?;

            // Apply post-RRF multiplier: (1.0 + boost).clamp(0.1, 3.0)
            for result in &mut results {
                let boost = boosts.get(&result.item.id).copied().unwrap_or(0.0);
                result.score *= (1.0 + boost).clamp(0.1, 3.0);
            }

            // Re-sort by score descending since boost may change order
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        results.truncate(params.limit);

        // Record access for reinforcement if enabled
        if let Some(fs) = params.feedback_settings
            && fs.access_reinforcement
        {
            for result in &results {
                if let Err(e) = self.with_connection(|conn| {
                    crate::db::content_decay_ops::on_content_access(
                        conn,
                        result.item.id,
                        fs.access_reinforcement_boost,
                    )
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
                    })
                }) {
                    eprintln!(
                        "Warning: Failed to record content access for item {}: {}",
                        result.item.id, e
                    );
                }
            }
        }

        Ok(results)
    }

    // ============================================================
    // Content Item CRUD Operations (Phase 1 - unified storage)
    // ============================================================

    /// Insert a content item (message, note, or document)
    ///
    /// Returns the item ID.
    #[expect(clippy::too_many_arguments)]
    pub fn insert_content_item(
        &self,
        content_type: &str,
        conversation_id: Option<&str>,
        role: Option<&str>,
        message_type: Option<&str>,
        previous_item_id: Option<i64>,
        prompt_tokens: Option<i64>,
        scope: Option<&str>,
        source: Option<&str>,
        title: Option<&str>,
        content: &str,
        thinking_content: Option<&str>,
        importance: f32,
        project_id: Option<&str>,
        timestamp: DateTime<Utc>,
    ) -> Result<i64> {
        self.with_connection(|conn| {
            let now = timestamp.timestamp();
            conn.execute(
                "INSERT INTO content_items (
                    content_type, conversation_id, role, message_type, previous_item_id,
                    prompt_tokens, scope, source, title, content, thinking_content, importance,
                    access_count, decay_score, created_at, updated_at, last_accessed,
                    has_embedding, project_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 1.0, ?13, ?13, ?13, 0, ?14)",
                params![
                    content_type,
                    conversation_id,
                    role,
                    message_type,
                    previous_item_id,
                    prompt_tokens,
                    scope,
                    source,
                    title,
                    content,
                    thinking_content,
                    importance,
                    now,
                    project_id,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Insert a chunk for a content item
    ///
    /// Returns the chunk ID.
    pub fn insert_content_chunk(
        &self,
        item_id: i64,
        chunk_index: i32,
        content: &str,
        start_offset: i32,
        end_offset: i32,
        timestamp: DateTime<Utc>,
    ) -> Result<i64> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_chunks (item_id, chunk_index, content, start_offset, end_offset, created_at, has_embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
                params![
                    item_id,
                    chunk_index,
                    content,
                    start_offset,
                    end_offset,
                    timestamp.timestamp(),
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Check if a content item has chunks
    pub fn content_item_has_chunks(&self, item_id: i64) -> Result<bool> {
        self.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM content_chunks WHERE item_id = ?1",
                params![item_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Mark item as having embeddings only when all its chunks are complete.
    ///
    /// This prevents premature marking where `has_embedding=1` is set on an item
    /// even though some chunks failed to generate embeddings. On next startup,
    /// the item would be skipped even though chunks are missing.
    ///
    /// For items WITHOUT chunks (short content embedded directly), this sets
    /// `has_embedding=1` immediately since the embedding was just saved.
    ///
    /// For items WITH chunks, this verifies ALL chunks have `has_embedding=1`
    /// before marking the parent item.
    pub fn mark_item_embedding_if_complete(&self, item_id: i64) -> Result<bool> {
        self.with_connection(|conn| {
            // Count chunks without embeddings
            let incomplete_chunks: i64 = conn.query_row(
                "SELECT COUNT(*) FROM content_chunks WHERE item_id = ?1 AND has_embedding = 0",
                params![item_id],
                |row| row.get(0),
            )?;

            if incomplete_chunks == 0 {
                // All chunks have embeddings (or item has no chunks) — safe to mark complete
                conn.execute(
                    "UPDATE content_items SET has_embedding = 1 WHERE id = ?1",
                    params![item_id],
                )?;
                Ok(true)
            } else {
                // Some chunks still missing — don't mark yet
                Ok(false)
            }
        })
    }

    /// Get content items for a conversation
    ///
    /// Returns items ordered by creation time (oldest first).
    pub fn get_conversation_items(&self, conversation_id: &str) -> Result<Vec<ContentItem>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, content_type, conversation_id, role, message_type,
                        previous_item_id, prompt_tokens, scope, source, title,
                        content, importance, access_count, decay_score,
                        created_at, updated_at, last_accessed, has_embedding, project_id,
                        thinking_content
                 FROM content_items
                 WHERE conversation_id = ?1
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![conversation_id], row_to_content_item)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Count content items for a conversation
    pub fn count_conversation_items(&self, conversation_id: &str) -> Result<i64> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM content_items WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
        })
    }

    /// Count all content items in database (across all conversations)
    pub fn count_all_content_items(&self) -> Result<i64> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM content_items", [], |row| row.get(0))
        })
    }

    /// Count facts in the database
    pub fn count_facts(&self) -> Result<i64> {
        self.with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM facts", [], |row| row.get(0))
        })
    }

    /// Count notes in the database
    pub fn count_notes(&self) -> Result<i64> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM content_items WHERE content_type = 'note'",
                [],
                |row| row.get(0),
            )
        })
    }

    /// Count documents in the database
    pub fn count_documents(&self) -> Result<i64> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM content_items WHERE content_type = 'document'",
                [],
                |row| row.get(0),
            )
        })
    }

    /// Delete the last N content items from a conversation
    ///
    /// Returns the number of items actually deleted.
    pub fn delete_last_content_items(&self, conversation_id: &str, count: usize) -> Result<usize> {
        if count == 0 {
            return Ok(0);
        }

        self.with_connection(|conn| {
            // Get item IDs to delete
            let item_ids: Vec<i64> = {
                let mut stmt = conn.prepare(
                    "SELECT id FROM content_items 
                     WHERE conversation_id = ?1 
                     ORDER BY created_at DESC LIMIT ?2",
                )?;
                stmt.query_map(params![conversation_id, count as i64], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect()
            };

            // Delete each item (and its chunks/embeddings)
            for item_id in &item_ids {
                // Delete chunk embeddings
                let chunk_ids: Vec<i64> = {
                    let mut stmt = conn.prepare("SELECT id FROM content_chunks WHERE item_id = ?1")?;
                    stmt.query_map(params![item_id], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect()
                };

                for chunk_id in &chunk_ids {
                    let _ = conn.execute(
                        "DELETE FROM chunk_embeddings_v2 WHERE chunk_id = ?1",
                        params![chunk_id],
                    );
                }

                // Delete content embedding
                let _ = conn.execute(
                    "DELETE FROM content_embeddings WHERE item_id = ?1",
                    params![item_id],
                );

                // Delete chunks
                conn.execute("DELETE FROM content_chunks WHERE item_id = ?1", params![item_id])?;
            }

            // Delete items
            let deleted = conn.execute(
                "DELETE FROM content_items WHERE conversation_id = ?1 AND id IN (
                    SELECT id FROM content_items WHERE conversation_id = ?1 ORDER BY created_at DESC LIMIT ?2
                )",
                params![conversation_id, count as i64],
            )?;

            Ok(deleted)
        })
    }

    /// Delete a conversation and all its content items
    ///
    /// Used by /forget command to completely remove conversation history.
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<()> {
        self.with_connection(|conn| {
            // Delete chunk embeddings for all items in the conversation
            let item_ids: Vec<i64> = {
                let mut stmt =
                    conn.prepare("SELECT id FROM content_items WHERE conversation_id = ?1")?;
                stmt.query_map(params![conversation_id], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect()
            };

            for item_id in &item_ids {
                // Get chunk IDs
                let chunk_ids: Vec<i64> = {
                    let mut stmt =
                        conn.prepare("SELECT id FROM content_chunks WHERE item_id = ?1")?;
                    stmt.query_map(params![item_id], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect()
                };

                // Delete chunk embeddings
                for chunk_id in &chunk_ids {
                    let _ = conn.execute(
                        "DELETE FROM chunk_embeddings_v2 WHERE chunk_id = ?1",
                        params![chunk_id],
                    );
                }
            }

            // Delete content embeddings (vec0 requires explicit deletion by item_id)
            for item_id in &item_ids {
                let _ = conn.execute(
                    "DELETE FROM content_embeddings WHERE item_id = ?1",
                    params![item_id],
                );
            }

            // FTS entries are cleaned automatically by the content_items_ad
            // trigger (AFTER DELETE on content_items) when we delete from
            // content_items below. No explicit FTS deletion is needed here
            // — the content_fts table uses external content mode and does
            // not have a conversation_id column.

            // Delete content_items (chunks cascade)
            conn.execute(
                "DELETE FROM content_items WHERE conversation_id = ?1",
                params![conversation_id],
            )?;

            // Delete conversation metadata
            conn.execute(
                "DELETE FROM conversations WHERE id = ?1",
                params![conversation_id],
            )?;

            Ok(())
        })
    }

    /// Count session items for delete preview.
    ///
    /// Returns the number of messages, embeddings, and todos that will be
    /// deleted if the given conversation is removed. Notes and facts are NOT
    /// included because they belong to the project, not the session.
    pub fn count_session_items(&self, conversation_id: &str) -> Result<SessionItemCounts> {
        self.with_connection(|conn| {
            let message_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM content_items WHERE conversation_id = ?1 AND content_type = 'message'",
                rusqlite::params![conversation_id],
                |row| row.get(0),
            )?;

            let embedding_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM content_embeddings WHERE item_id IN (SELECT id FROM content_items WHERE conversation_id = ?1)",
                rusqlite::params![conversation_id],
                |row| row.get(0),
            )?;

            let todo_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM session_todos WHERE conversation_id = ?1",
                rusqlite::params![conversation_id],
                |row| row.get(0),
            )?;

            Ok(SessionItemCounts {
                message_count,
                embedding_count,
                todo_count,
            })
        })
    }

    /// Get a content item by ID
    pub fn get_content_item_by_id(&self, item_id: i64) -> Result<Option<ContentItem>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, content_type, conversation_id, role, message_type,
                        previous_item_id, prompt_tokens, scope, source, title,
                        content, importance, access_count, decay_score,
                        created_at, updated_at, last_accessed, has_embedding, project_id,
                        thinking_content
                 FROM content_items
                 WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map(params![item_id], row_to_content_item)?;
            rows.next().transpose()
        })
    }

    // ============================================================
    // Message Search Functions (for content_type='message')
    // ============================================================

    /// Hybrid search for messages using RRF (convenience wrapper for content_type=message)
    #[expect(clippy::too_many_arguments)]
    pub fn search_messages_hybrid(
        &self,
        query: &str,
        embedding: &[f32],
        query_norm_correction: f32,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        limit: usize,
        keyword_weight: f32,
        semantic_weight: f32,
    ) -> Result<Vec<ContentSearchResult>> {
        let params = ContentSearchParams {
            query,
            embedding,
            query_norm_correction,
            content_type: Some(ContentType::Message),
            conversation_id,
            project_id,
            scope: None,
            limit,
            keyword_weight,
            semantic_weight,
            feedback_settings: None,
        };
        self.search_content_hybrid(&params)
    }

    /// Get subsequent assistant messages for a content item (for context enrichment)
    ///
    /// For messages with message_type='normal', returns all subsequent assistant messages
    /// in the same conversation until the next user message.
    pub fn get_content_subsequent_assistant(
        &self,
        item_id: i64,
        conversation_id: &str,
    ) -> Result<Vec<ContentItem>> {
        self.with_connection(|conn| {
            let sql = "
                SELECT id, content_type, conversation_id, role, message_type,
                       previous_item_id, prompt_tokens, scope, source, title,
                       content, importance, access_count, decay_score,
                       created_at, updated_at, last_accessed, has_embedding, project_id,
                       thinking_content
                FROM content_items
                WHERE conversation_id = ?1
                  AND role = 'assistant'
                  AND id > ?2
                  AND message_type = 'normal'
                ORDER BY id ASC
                LIMIT 5";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![conversation_id, item_id], row_to_content_item)?;
            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Enrich search results with conversation context
    ///
    /// For user messages, attaches all subsequent assistant messages (up to 5).
    pub fn enrich_content_results_with_context(
        &self,
        results: Vec<ContentSearchResult>,
    ) -> Result<Vec<ContentSearchResult>> {
        let mut enriched = Vec::with_capacity(results.len());
        let mut seen_ids = std::collections::HashSet::new();

        for mut result in results {
            seen_ids.insert(result.item.id);

            // Only enrich user messages
            if result.item.role.as_deref() == Some(ROLE_USER)
                && let Some(conv_id) = &result.item.conversation_id
            {
                let subsequent = self.get_content_subsequent_assistant(result.item.id, conv_id)?;

                for msg in subsequent {
                    if !seen_ids.contains(&msg.id) {
                        seen_ids.insert(msg.id);
                        result.subsequent_items.push(super::types::SubsequentItem {
                            item: msg,
                            source_type: crate::db::SourceType::Conversation,
                        });
                    }
                }
            }

            enriched.push(result);
        }

        Ok(enriched)
    }

    /// Clear prompt_tokens for all content items in a conversation.
    ///
    /// Called after compaction to invalidate old cumulative token counts.
    /// The next message sent to the LLM will have fresh prompt_tokens.
    pub fn clear_conversation_prompt_tokens(&self, conversation_id: &str) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "UPDATE content_items SET prompt_tokens = NULL WHERE conversation_id = ?1",
                params![conversation_id],
            )?;
            Ok(())
        })
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
        thinking_content: row.get(19)?,
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

/// Helper to map a row to a Document
fn row_to_document(row: &rusqlite::Row) -> Result<Document> {
    Ok(Document {
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
        filename: row.get(12)?,
        file_type: FileType::from_str(&row.get::<_, String>(13)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        word_count: row.get::<_, i32>(14)? as usize,
    })
}

/// Helper to map a row to a ContentChunk
fn row_to_content_chunk(row: &rusqlite::Row) -> Result<super::types::ContentChunk> {
    Ok(super::types::ContentChunk {
        id: row.get(0)?,
        item_id: row.get(1)?,
        chunk_index: row.get(2)?,
        content: row.get(3)?,
        start_offset: row.get(4)?,
        end_offset: row.get(5)?,
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

    #[test]
    fn test_insert_and_get_document() {
        let db = Database::in_memory().expect("Failed to create database");

        let doc = Document::new(
            "This is a test document.".to_string(),
            "Test Document".to_string(),
            "test.md".to_string(),
            FileType::Md,
            ContentScope::Project,
            Some("test-project".to_string()),
        )
        .expect("Failed to create document");

        let id = db.insert_document(&doc).expect("Failed to insert document");
        assert!(id > 0);

        let retrieved = db.get_document(id).expect("Failed to get document");
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.content, "This is a test document.");
        assert_eq!(retrieved.title, "Test Document");
        assert_eq!(retrieved.filename, "test.md");
        assert_eq!(retrieved.file_type, FileType::Md);
        assert_eq!(retrieved.scope, ContentScope::Project);
        assert_eq!(retrieved.source, ContentSource::User);
    }

    #[test]
    fn test_list_documents() {
        let db = Database::in_memory().expect("Failed to create database");

        let doc1 = Document::new(
            "Document 1 content.".to_string(),
            "Doc 1".to_string(),
            "doc1.txt".to_string(),
            FileType::Txt,
            ContentScope::Project,
            Some("project-a".to_string()),
        )
        .expect("Failed to create document");

        let doc2 = Document::new(
            "Document 2 content.".to_string(),
            "Doc 2".to_string(),
            "doc2.md".to_string(),
            FileType::Md,
            ContentScope::Global,
            None,
        )
        .expect("Failed to create document");

        db.insert_document(&doc1).expect("Failed to insert doc1");
        db.insert_document(&doc2).expect("Failed to insert doc2");

        let all_docs = db
            .list_documents(None, None)
            .expect("Failed to list documents");
        assert_eq!(all_docs.len(), 2);

        let project_docs = db
            .list_documents(None, Some("project-a"))
            .expect("Failed to list project documents");
        assert_eq!(project_docs.len(), 1);

        let global_docs = db
            .list_documents(Some(ContentScope::Global), None)
            .expect("Failed to list global documents");
        assert_eq!(global_docs.len(), 1);
    }

    #[test]
    fn test_delete_document() {
        let db = Database::in_memory().expect("Failed to create database");

        let doc = Document::new(
            "To be deleted.".to_string(),
            "Delete Test".to_string(),
            "delete.org".to_string(),
            FileType::Org,
            ContentScope::Project,
            None,
        )
        .expect("Failed to create document");

        let id = db.insert_document(&doc).expect("Failed to insert document");
        assert!(
            db.get_document(id)
                .expect("Failed to get document")
                .is_some()
        );

        db.delete_document(id).expect("Failed to delete document");
        assert!(
            db.get_document(id)
                .expect("Failed to get document")
                .is_none()
        );
    }

    #[test]
    fn test_document_size_validation() {
        use crate::content::document::MAX_DOCUMENT_SIZE;

        let long_content = "x".repeat(MAX_DOCUMENT_SIZE + 1);
        let result = Document::new(
            long_content,
            "Large Doc".to_string(),
            "large.txt".to_string(),
            FileType::Txt,
            ContentScope::Project,
            None,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("too large") || err.contains("exceeds"),
            "Error should mention size limit: {}",
            err
        );

        let valid_content = "x".repeat(1000);
        let result = Document::new(
            valid_content,
            "Small Doc".to_string(),
            "small.md".to_string(),
            FileType::Md,
            ContentScope::Project,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_thinking_content_stored_and_retrieved() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert an assistant message with thinking content
        let item_id = db
            .insert_content_item(
                "message",
                None,
                Some(ROLE_ASSISTANT),
                Some("regular"),
                None,
                Some(10),
                None,
                None,
                None,
                "The answer is 42",
                Some("I reasoned about the meaning of life"),
                0.5,
                None,
                Utc::now(),
            )
            .expect("Failed to insert content item");

        // Retrieve it back
        let loaded = db
            .get_content_item_by_id(item_id)
            .expect("Failed to get content item")
            .expect("Content item not found");

        assert_eq!(loaded.content, "The answer is 42");
        assert_eq!(
            loaded.thinking_content,
            Some("I reasoned about the meaning of life".to_string())
        );
    }

    #[test]
    fn test_thinking_content_not_in_search_results() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a note with "Rust" in content (should be found by search)
        let note = Note::new(
            "Rust programming language".to_string(),
            ContentScope::Global,
            None,
            ContentSource::User,
            Some("Test Note".to_string()),
        )
        .expect("Failed to create note");
        db.insert_note(&note).expect("Failed to insert note");

        // Insert an assistant message with thinking that mentions "Rust"
        // (thinking_content is NOT indexed by FTS, so this should NOT be found)
        db.insert_content_item(
            "message",
            None,
            Some(ROLE_ASSISTANT),
            Some("regular"),
            None,
            Some(10),
            None,
            None,
            None,
            "Python is great",
            Some("I considered Rust but chose Python"),
            0.5,
            None,
            Utc::now(),
        )
        .expect("Failed to insert content item");

        // Search for "Rust" — should find the note but NOT the assistant message
        // (thinking_content is not indexed by FTS)
        let results = db
            .search_notes_keyword("Rust", None, None, 10)
            .expect("Failed to search");

        // The note has "Rust" in its content, so it should be found
        assert_eq!(
            results.len(),
            1,
            "Should find only the note, not the message with thinking about Rust"
        );
        assert_eq!(results[0].item.title, Some("Test Note".to_string()));
    }

    /// Regression test: FTS keyword search must include thinking_content in SELECT.
    /// Bug: after adding thinking_content column, the inline format!() SQL in
    /// search_content_keyword() was missing ci.thinking_content, causing column
    /// index mismatch — row_to_content_item reads index 19 expecting String but
    /// found the bm25 score (REAL). This broke /search and all hybrid search.
    #[test]
    fn test_search_content_keyword_with_thinking_content_column() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert an assistant message with thinking_content
        db.insert_content_item(
            "message",
            Some("test-conv"),
            Some(ROLE_ASSISTANT),
            Some("regular"),
            None,
            Some(10),
            None,
            None,
            None,
            "Wittgenstein language games",
            Some("I need to think about Wittgenstein's philosophy"),
            0.5,
            None,
            Utc::now(),
        )
        .expect("Failed to insert content item");

        // search_content_keyword must not crash with column type mismatch
        let results = db
            .search_content_keyword("Wittgenstein", None, Some("test-conv"), None, None, 10)
            .expect("keyword search must succeed — thinking_content column must be in SELECT");

        assert_eq!(results.len(), 1, "Should find the message by keyword");
        let found = &results[0];
        // Verify thinking_content was read correctly (index 19, not score)
        assert_eq!(
            found.item.thinking_content,
            Some("I need to think about Wittgenstein's philosophy".to_string()),
            "thinking_content should be populated, not overwritten by bm25 score"
        );
        // Verify content is clean
        assert_eq!(found.item.content, "Wittgenstein language games");
    }

    #[test]
    fn test_count_session_items_empty() {
        let db = Database::in_memory().expect("Failed to create database");

        // Count items for a non-existent conversation
        let counts = db
            .count_session_items("nonexistent-id")
            .expect("count_session_items failed");

        assert_eq!(
            counts,
            SessionItemCounts {
                message_count: 0,
                embedding_count: 0,
                todo_count: 0
            }
        );
    }

    #[test]
    fn test_count_session_items_with_messages() {
        let db = Database::in_memory().expect("Failed to create database");
        let convo_id = "session-test-count";

        // Insert a conversation
        db.insert_conversation(
            convo_id,
            Some("project-1"),
            Some("test-count-session"),
            "test-model",
            chrono::Utc::now(),
            chrono::Utc::now(),
        )
        .expect("Failed to insert conversation");

        // Insert 3 messages for this conversation
        for i in 0..3 {
            db.insert_content_item(
                "message",
                Some(convo_id),
                Some(if i % 2 == 0 {
                    ROLE_USER
                } else {
                    ROLE_ASSISTANT
                }),
                Some("regular"),
                None,
                Some(10),
                Some("project"),
                Some("user"), // source must be 'user' or 'llm'
                None,         // title
                &format!("Message {}", i),
                None, // thinking_content
                0.5,
                Some("project-1"),
                chrono::Utc::now(),
            )
            .expect("Failed to insert message");
        }

        let counts = db
            .count_session_items(convo_id)
            .expect("count_session_items failed");

        assert_eq!(
            counts,
            SessionItemCounts {
                message_count: 3,
                embedding_count: 0,
                todo_count: 0
            },
            "Should count 3 messages, no embeddings, no todos"
        );
    }

    #[test]
    fn test_pruned_items_excluded_from_search() {
        let db = Database::in_memory().expect("Failed to create database");

        let item_id = db
            .insert_content_item(
                "message",
                Some("test-conv"),
                Some(ROLE_USER),
                Some("regular"),
                None,
                Some(10),
                Some("project"),
                Some("user"),
                None,
                "Hello world unique prune marker",
                None,
                0.5,
                Some("project-1"),
                Utc::now(),
            )
            .expect("Failed to insert content item");

        // Sanity: item appears in keyword search while unpruned
        let results = db
            .search_content_keyword("Hello world", None, None, None, None, 10)
            .expect("keyword search must succeed");
        assert!(
            results.iter().any(|r| r.item.id == item_id),
            "unpruned item should appear in keyword search"
        );

        // Prune the item directly (soft-delete, as run_content_decay_cycle does)
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE content_items SET pruned = 1 WHERE id = ?1",
                params![item_id],
            )
            .map(|_| ())
        })
        .expect("Failed to prune item");

        // After pruning, item must NOT appear in keyword search
        let results = db
            .search_content_keyword("Hello world", None, None, None, None, 10)
            .expect("keyword search must succeed after prune");
        assert!(
            !results.iter().any(|r| r.item.id == item_id),
            "pruned item leaked into keyword search results"
        );
    }
}
