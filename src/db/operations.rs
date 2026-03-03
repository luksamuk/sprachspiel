//! Database operations for message storage and retrieval
//!
//! Provides:
//! - Insert/update messages
//! - Insert embeddings
//! - Hybrid search (BM25 + semantic + RRF)

use chrono::{DateTime, Utc};
use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use zerocopy::IntoBytes;

use super::Database;

/// Escape a string for FTS5 MATCH queries.
///
/// FTS5 has special syntax for queries. To safely search for user-provided text:
/// 1. Wrap the entire query in double quotes (phrase query)
/// 2. Escape any embedded double quotes by doubling them
///
/// This prevents SQL injection and FTS5 syntax errors.
///
/// # Examples
/// ```ignore
/// let safe = fts5_escape("hello world");  // "\"hello world\""
/// let safe = fts5_escape("test\"quote");  // "\"test""quote\""
/// let safe = fts5_escape("a AND b");       // "\"a AND b\"" (literal search, not boolean)
/// ```
pub fn fts5_escape(query: &str) -> String {
    // Escape double quotes by doubling them
    let escaped = query.replace('"', "\"\"");
    // Wrap in double quotes for phrase search
    format!("\"{}\"", escaped)
}

/// Search result from hybrid search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Message ID
    pub message_id: i64,
    /// Conversation ID
    pub conversation_id: String,
    /// Message role
    pub role: String,
    /// Message content (full message)
    pub content: String,
    /// Timestamp
    pub timestamp: i64,
    /// Combined score (RRF)
    pub score: f32,
    /// Source of the result
    pub search_type: SearchType,
    /// Chunk content (if result matched a chunk)
    pub chunk_content: Option<String>,
    /// Chunk start offset in original message
    pub chunk_start: Option<i32>,
    /// Chunk end offset in original message
    pub chunk_end: Option<i32>,
}

/// Type of search that found the result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    /// Full-text search (BM25)
    Keyword,
    /// Vector similarity search
    Semantic,
    /// Combined result (appears in both)
    Hybrid,
}

impl Database {
    /// Insert a new conversation
    pub fn insert_conversation(
        &self,
        id: &str,
        project_id: Option<&str>,
        title: Option<&str>,
        model: &str,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "INSERT OR REPLACE INTO conversations (id, project_id, title, model, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    project_id,
                    title,
                    model,
                    created_at.timestamp(),
                    updated_at.timestamp(),
                ],
            )?;
            Ok(())
        })
    }

    /// Insert a new message
    pub fn insert_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<i64> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "INSERT INTO messages (conversation_id, role, content, timestamp, importance, has_embedding)
                 VALUES (?1, ?2, ?3, ?4, 0.5, 0)",
                params![
                    conversation_id,
                    role,
                    content,
                    timestamp.timestamp(),
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Update a message with its embedding
    pub fn update_message_embedding(
        &self,
        message_id: i64,
        embedding: &[f32],
        conversation_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            // Convert embedding to bytes
            let embedding_bytes = embedding.as_bytes();

            // Insert embedding
            conn.execute(
                "INSERT INTO message_embeddings (message_id, embedding, conversation_id, timestamp)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    message_id,
                    embedding_bytes,
                    conversation_id,
                    timestamp.timestamp(),
                ],
            )?;

            // Mark message as having embedding
            conn.execute(
                "UPDATE messages SET has_embedding = 1 WHERE id = ?1",
                params![message_id],
            )?;

            Ok(())
        })
    }

    /// Search messages using full-text search (BM25)
    pub fn search_keyword(
        &self,
        query: &str,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // Escape the query for FTS5 to prevent syntax errors and injection
        let escaped_query = fts5_escape(query);

        self.with_connection(|conn: &rusqlite::Connection| {
            let mut results = Vec::new();

            let sql = match conversation_id {
                Some(conv_id) => {
                    let sql = r#"SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, bm25(messages_fts) as score
                        FROM messages_fts fts
                        JOIN messages m ON fts.rowid = m.id
                        WHERE messages_fts MATCH ?1 AND m.conversation_id = ?2
                        ORDER BY score ASC
                        LIMIT ?3"#;
                    let mut stmt = conn.prepare(sql)?;
                    let rows = stmt.query_map(params![escaped_query, conv_id, limit as i32], |row: &rusqlite::Row<'_>| {
                        Ok(SearchResult {
                            message_id: row.get(0)?,
                            conversation_id: row.get(1)?,
                            role: row.get(2)?,
                            content: row.get(3)?,
                            timestamp: row.get(4)?,
                            score: row.get::<_, f32>(5)?,
                            search_type: SearchType::Keyword,
                            chunk_content: None,
                            chunk_start: None,
                            chunk_end: None,
                        })
                    })?;
                    for r in rows {
                        results.push(r?);
                    }
                    return Ok(results);
                }
                None => {
                    r#"SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, bm25(messages_fts) as score
                        FROM messages_fts fts
                        JOIN messages m ON fts.rowid = m.id
                        WHERE messages_fts MATCH ?1
                        ORDER BY score ASC
                        LIMIT ?2"#
                }
            };

            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![escaped_query, limit as i32], |row: &rusqlite::Row<'_>| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    score: row.get::<_, f32>(5)?,
                    search_type: SearchType::Keyword,
                    chunk_content: None,
                    chunk_start: None,
                    chunk_end: None,
                })
            })?;

            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Search messages using vector similarity
    ///
    /// Note: sqlite-vec KNN queries only support `embedding MATCH ? AND k = ?`.
    /// Additional filters (like conversation_id) must be applied after retrieval.
    /// See: https://github.com/asg017/sqlite-vec
    ///
    /// Searches both:
    /// - Short messages (stored in message_embeddings)
    /// - Chunks of long messages (stored in chunk_embeddings)
    pub fn search_semantic(
        &self,
        embedding: &[f32],
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let embedding_bytes = embedding.as_bytes();

            // sqlite-vec KNN: only embedding MATCH and k=? allowed in WHERE
            // We fetch more results and filter in application code if needed
            let fetch_limit = if conversation_id.is_some() {
                // Fetch 3x more when filtering by conversation to ensure enough results
                limit * 3
            } else {
                limit
            };

            // Query message_embeddings (short messages without chunks)
            let sql_messages = r#"SELECT me.message_id, me.conversation_id, m.role, m.content, m.timestamp, me.distance,
                NULL as chunk_content, NULL as chunk_start, NULL as chunk_end
                FROM message_embeddings me
                JOIN messages m ON me.message_id = m.id
                WHERE me.embedding MATCH ?1 AND me.k = ?2"#;

            let mut results: Vec<SearchResult> = Vec::new();

            // Search message embeddings (short messages)
            let mut stmt = conn.prepare(sql_messages)?;
            let rows = stmt.query_map(params![embedding_bytes, fetch_limit as i32], |row| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    score: row.get::<_, f32>(5)?,
                    search_type: SearchType::Semantic,
                    chunk_content: row.get(6)?,
                    chunk_start: row.get(7)?,
                    chunk_end: row.get(8)?,
                })
            })?;
            for r in rows {
                results.push(r?);
            }

            // Query chunk_embeddings (chunks of long messages)
            let sql_chunks = r#"SELECT c.message_id, ce.conversation_id, m.role, m.content, m.timestamp, ce.distance,
                c.content as chunk_content, c.start_offset, c.end_offset
                FROM chunk_embeddings ce
                JOIN message_chunks c ON ce.chunk_id = c.id
                JOIN messages m ON c.message_id = m.id
                WHERE ce.embedding MATCH ?1 AND ce.k = ?2"#;

            let mut stmt = conn.prepare(sql_chunks)?;
            let rows = stmt.query_map(params![embedding_bytes, fetch_limit as i32], |row| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    score: row.get::<_, f32>(5)?,
                    search_type: SearchType::Semantic,
                    chunk_content: row.get(6)?,
                    chunk_start: row.get(7)?,
                    chunk_end: row.get(8)?,
                })
            })?;
            for r in rows {
                results.push(r?);
            }

            // Deduplicate by message_id, keeping best score
            // (a message might appear in both results if it has chunks)
            use std::collections::HashMap;
            let mut best_results: HashMap<i64, SearchResult> = HashMap::new();
            for result in results {
                let entry = best_results.entry(result.message_id);
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

            let mut results: Vec<SearchResult> = best_results.into_values().collect();
            results.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));

            // Filter by conversation_id in application code
            if let Some(conv_id) = conversation_id {
                results.retain(|r| r.conversation_id == conv_id);
                results.truncate(limit);
            } else {
                results.truncate(limit);
            }

            Ok(results)
        })
    }

    /// Hybrid search using Reciprocal Rank Fusion
    pub fn search_hybrid(
        &self,
        query: &str,
        embedding: &[f32],
        conversation_id: Option<&str>,
        limit: usize,
        keyword_weight: f32,
        semantic_weight: f32,
    ) -> Result<Vec<SearchResult>> {
        // Get keyword results (more = better fusion)
        let keyword_results = self.search_keyword(query, conversation_id, limit * 2)?;

        // Get semantic results (more = better fusion)
        let semantic_results = self.search_semantic(embedding, conversation_id, limit * 2)?;

        // Combine with RRF
        Ok(reciprocal_rank_fusion(
            keyword_results,
            semantic_results,
            keyword_weight,
            semantic_weight,
            limit,
        ))
    }

    /// Get messages for a conversation (for context loading)
    pub fn get_conversation_messages(
        &self,
        conversation_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let mut results = Vec::new();

            match limit {
                Some(lim) => {
                    let sql = "SELECT id, conversation_id, role, content, timestamp FROM messages 
                        WHERE conversation_id = ?1 ORDER BY timestamp ASC LIMIT ?2";
                    let mut stmt = conn.prepare(sql)?;
                    let rows = stmt.query_map(
                        params![conversation_id, lim as i32],
                        |row: &rusqlite::Row<'_>| {
                            Ok(SearchResult {
                                message_id: row.get(0)?,
                                conversation_id: row.get(1)?,
                                role: row.get(2)?,
                                content: row.get(3)?,
                                timestamp: row.get(4)?,
                                score: 0.0,
                                search_type: SearchType::Hybrid,
                                chunk_content: None,
                                chunk_start: None,
                                chunk_end: None,
                            })
                        },
                    )?;
                    for r in rows {
                        results.push(r?);
                    }
                }
                None => {
                    let sql = "SELECT id, conversation_id, role, content, timestamp FROM messages 
                        WHERE conversation_id = ?1 ORDER BY timestamp ASC";
                    let mut stmt = conn.prepare(sql)?;
                    let rows =
                        stmt.query_map(params![conversation_id], |row: &rusqlite::Row<'_>| {
                            Ok(SearchResult {
                                message_id: row.get(0)?,
                                conversation_id: row.get(1)?,
                                role: row.get(2)?,
                                content: row.get(3)?,
                                timestamp: row.get(4)?,
                                score: 0.0,
                                search_type: SearchType::Hybrid,
                                chunk_content: None,
                                chunk_start: None,
                                chunk_end: None,
                            })
                        })?;
                    for r in rows {
                        results.push(r?);
                    }
                }
            }

            Ok(results)
        })
    }

    /// Delete a conversation and all its messages
    ///
    /// Future use: Conversation management commands (delete, archive).
    #[allow(dead_code)]
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            // Embeddings are deleted via CASCADE
            conn.execute(
                "DELETE FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
            )?;
            conn.execute(
                "DELETE FROM conversations WHERE id = ?1",
                params![conversation_id],
            )?;
            Ok(())
        })
    }

    /// Count messages in a conversation (for RAG decision after /clear)
    ///
    /// This is used by RAG to determine if retrieval should be performed
    /// even when session.messages is empty (after /clear).
    pub fn count_conversation_messages(&self, conversation_id: &str) -> Result<usize> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }

    /// Get a single message by ID (for remember tool)
    ///
    /// Used by the remember tool to retrieve full message content
    /// when the LLM sees a truncated message in the retrieved context.
    pub fn get_message_by_id(&self, message_id: i64) -> Result<Option<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql = "SELECT id, conversation_id, role, content, timestamp FROM messages 
                       WHERE id = ?1";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query_map(params![message_id], |row: &rusqlite::Row<'_>| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    score: 1.0,
                    search_type: SearchType::Keyword,
                    chunk_content: None,
                    chunk_start: None,
                    chunk_end: None,
                })
            })?;

            Ok(rows.next().transpose()?)
        })
    }

    /// Check if a conversation exists
    #[allow(dead_code)]
    pub fn conversation_exists(&self, conversation_id: &str) -> Result<bool> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Count messages with embeddings
    ///
    /// Future use: Database statistics and diagnostics.
    #[allow(dead_code)]
    pub fn count_embedded_messages(&self) -> Result<i64> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE has_embedding = 1",
                [],
                |row: &rusqlite::Row<'_>| row.get(0),
            )
        })
    }

    /// Get all conversation IDs
    ///
    /// Future use: `/reindex all` command to rebuild embeddings for all conversations.
    #[allow(dead_code)]
    pub fn list_conversations(&self) -> Result<Vec<String>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let mut stmt = conn.prepare("SELECT id FROM conversations ORDER BY updated_at DESC")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>>>()
        })
    }

    /// Get all messages without embeddings for reindexing
    ///
    /// Future use: `/reindex all` command to generate missing embeddings.
    #[allow(dead_code)]
    pub fn get_messages_for_reindex(&self) -> Result<Vec<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let mut results = Vec::new();

            let sql = "SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp 
                FROM messages m 
                WHERE m.has_embedding = 0 
                ORDER BY m.timestamp ASC";

            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row: &rusqlite::Row<'_>| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    score: 0.0,
                    search_type: SearchType::Hybrid,
                    chunk_content: None,
                    chunk_start: None,
                    chunk_end: None,
                })
            })?;

            for r in rows {
                results.push(r?);
            }

            Ok(results)
        })
    }

    /// Rebuild FTS5 index from existing messages
    ///
    /// Call this after migration to ensure all messages are searchable.
    /// This is needed because FTS5 with external content table doesn't
    /// automatically index existing messages - only new INSERTs trigger indexing.
    pub fn rebuild_fts5(&self) -> Result<usize> {
        self.with_connection(|conn: &rusqlite::Connection| {
            // First, clear the FTS index
            conn.execute("DELETE FROM messages_fts", [])?;

            // Then, rebuild from messages table
            // For FTS5 with content table, we need to insert each row
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;

            // Insert all messages into FTS5
            conn.execute(
                "INSERT INTO messages_fts(rowid, content) 
                 SELECT id, content FROM messages",
                [],
            )?;

            Ok(count as usize)
        })
    }

    /// Insert a chunk of a long message
    pub fn insert_chunk(
        &self,
        message_id: i64,
        chunk_index: i32,
        content: &str,
        start_offset: i32,
        end_offset: i32,
        timestamp: DateTime<Utc>,
    ) -> Result<i64> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "INSERT INTO message_chunks (message_id, chunk_index, content, start_offset, end_offset, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    message_id,
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

    /// Update a chunk with its embedding
    pub fn update_chunk_embedding(
        &self,
        chunk_id: i64,
        embedding: &[f32],
        conversation_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let embedding_bytes = embedding.as_bytes();

            conn.execute(
                "INSERT INTO chunk_embeddings (chunk_id, embedding, conversation_id, timestamp)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    chunk_id,
                    embedding_bytes,
                    conversation_id,
                    timestamp.timestamp(),
                ],
            )?;

            // Mark chunk as having embedding
            conn.execute(
                "UPDATE message_chunks SET has_embedding = 1 WHERE id = ?1",
                params![chunk_id],
            )?;

            Ok(())
        })
    }

    /// Get all chunks for a message
    #[allow(dead_code)]
    pub fn get_message_chunks(&self, message_id: i64) -> Result<Vec<ChunkRow>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let mut stmt = conn.prepare(
                "SELECT id, message_id, chunk_index, content, start_offset, end_offset, created_at
                 FROM message_chunks 
                 WHERE message_id = ?1 
                 ORDER BY chunk_index ASC",
            )?;

            let rows = stmt.query_map(params![message_id], |row| {
                Ok(ChunkRow {
                    id: row.get(0)?,
                    message_id: row.get(1)?,
                    chunk_index: row.get(2)?,
                    content: row.get(3)?,
                    start_offset: row.get(4)?,
                    end_offset: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?;

            rows.collect::<Result<Vec<_>>>()
        })
    }

    /// Get chunks without embeddings for recovery
    ///
    /// Returns (chunk_id, content) pairs for chunks that need embedding generation.
    /// Used by recovery manager on startup to resume interrupted embedding generation.
    pub fn get_chunks_without_embedding(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<(i64, String)>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let mut stmt = conn.prepare(
                "SELECT mc.id, mc.content 
                 FROM message_chunks mc
                 JOIN messages m ON mc.message_id = m.id
                 WHERE m.conversation_id = ?1 AND mc.has_embedding = 0
                 ORDER BY mc.id ASC",
            )?;

            let rows = stmt.query_map(params![conversation_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;

            rows.collect::<Result<Vec<_>>>()
        })
    }
}

/// Row from message_chunks table
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChunkRow {
    pub id: i64,
    pub message_id: i64,
    pub chunk_index: i32,
    pub content: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub created_at: i64,
}

/// Reciprocal Rank Fusion algorithm
///
/// Combines multiple ranked lists into a single ranked list.
/// RRF score = Σ (weight_i / (k + rank_i)) where k is typically 60
pub fn reciprocal_rank_fusion(
    keyword_results: Vec<SearchResult>,
    semantic_results: Vec<SearchResult>,
    keyword_weight: f32,
    semantic_weight: f32,
    limit: usize,
) -> Vec<SearchResult> {
    use std::collections::{HashMap, HashSet};

    let k = 60.0; // RRF constant
    let mut scores: HashMap<i64, (f32, SearchResult)> = HashMap::new();
    let mut seen_in_keyword: HashSet<i64> = HashSet::new();

    // Process keyword results
    for (rank, result) in keyword_results.into_iter().enumerate() {
        let rank_f = (rank + 1) as f32;
        let rrf_score = keyword_weight / (k + rank_f);
        scores.insert(result.message_id, (rrf_score, result.clone()));
        seen_in_keyword.insert(result.message_id);
    }

    // Process semantic results
    for (rank, result) in semantic_results.into_iter().enumerate() {
        let rank_f = (rank + 1) as f32;
        let rrf_score = semantic_weight / (k + rank_f);

        if let Some((existing_score, existing_result)) = scores.get_mut(&result.message_id) {
            // Combine scores
            *existing_score += rrf_score;
            // Mark as hybrid
            existing_result.search_type = SearchType::Hybrid;
        } else {
            scores.insert(result.message_id, (rrf_score, result));
        }
    }

    // Sort by combined score (descending)
    let mut results: Vec<_> = scores.into_values().collect();
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Update scores and limit
    results
        .into_iter()
        .map(|(score, mut result)| {
            result.score = score;
            result
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_conversation() {
        let db = Database::in_memory().expect("Failed to create database");

        db.insert_conversation(
            "test-conv",
            Some("project-1"),
            Some("Test Conversation"),
            "llama3.1",
            Utc::now(),
            Utc::now(),
        )
        .expect("Failed to insert conversation");

        let count: i64 = db
            .with_connection(|conn: &rusqlite::Connection| {
                conn.query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            })
            .expect("Failed to count conversations");

        assert_eq!(count, 1);
    }

    #[test]
    fn test_insert_message() {
        let db = Database::in_memory().expect("Failed to create database");

        db.insert_conversation("test-conv", None, None, "llama3.1", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        let msg_id = db
            .insert_message("test-conv", "user", "Hello world", Utc::now())
            .expect("Failed to insert message");

        assert!(msg_id > 0);
    }

    #[test]
    fn test_update_embedding() {
        let db = Database::in_memory().expect("Failed to create database");

        db.insert_conversation("test-conv", None, None, "llama3.1", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        let msg_id = db
            .insert_message("test-conv", "user", "Hello world", Utc::now())
            .expect("Failed to insert message");

        let embedding: Vec<f32> = (0..256).map(|i| i as f32 / 256.0).collect();
        db.update_message_embedding(msg_id, &embedding, "test-conv", Utc::now())
            .expect("Failed to update embedding");

        let has_embedding: i64 = db
            .with_connection(|conn: &rusqlite::Connection| {
                conn.query_row(
                    "SELECT has_embedding FROM messages WHERE id = ?1",
                    params![msg_id],
                    |row| row.get(0),
                )
            })
            .expect("Failed to check embedding");

        assert_eq!(has_embedding, 1);
    }

    #[test]
    fn test_search_keyword() {
        let db = Database::in_memory().expect("Failed to create database");

        db.insert_conversation("test-conv", None, None, "llama3.1", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        db.insert_message("test-conv", "user", "Hello world from Rust", Utc::now())
            .expect("Failed to insert message");
        db.insert_message("test-conv", "assistant", "How can I help you?", Utc::now())
            .expect("Failed to insert message");

        let results = db
            .search_keyword("Rust", None, 10)
            .expect("Failed to search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Hello world from Rust");
    }

    #[test]
    fn test_reciprocal_rank_fusion() {
        let make_result = |id: i64, content: &str| SearchResult {
            message_id: id,
            conversation_id: "test".to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: 0,
            score: 0.0,
            search_type: SearchType::Keyword,
            chunk_content: None,
            chunk_start: None,
            chunk_end: None,
        };

        let keyword_results = vec![
            make_result(1, "Result 1"),
            make_result(2, "Result 2"),
            make_result(3, "Result 3"),
        ];

        let semantic_results = vec![
            make_result(2, "Result 2"), // Duplicate - appears in both
            make_result(4, "Result 4"),
            make_result(5, "Result 5"),
        ];

        let fused = reciprocal_rank_fusion(keyword_results, semantic_results, 0.4, 0.6, 5);

        // Result 2 should be at the top (appears in both lists)
        assert_eq!(fused[0].message_id, 2);
        assert_eq!(fused[0].search_type, SearchType::Hybrid);
    }

    #[test]
    fn test_fts5_escape() {
        use super::fts5_escape;

        // Basic text
        assert_eq!(fts5_escape("hello"), "\"hello\"");

        // Text with spaces (phrase)
        assert_eq!(fts5_escape("hello world"), "\"hello world\"");

        // Text with special FTS characters (should be literal, not operators)
        assert_eq!(fts5_escape("test AND other"), "\"test AND other\"");
        assert_eq!(fts5_escape("test OR other"), "\"test OR other\"");
        assert_eq!(fts5_escape("test NOT other"), "\"test NOT other\"");
        assert_eq!(fts5_escape("test*"), "\"test*\"");

        // Text with double quotes (should be escaped)
        assert_eq!(fts5_escape("test\"quote"), "\"test\"\"quote\"");
        assert_eq!(fts5_escape("a\"b\"c"), "\"a\"\"b\"\"c\"");

        // Parentheses (should be literal)
        assert_eq!(fts5_escape("test()"), "\"test()\"");

        // Injection attempt
        assert_eq!(
            fts5_escape("test); DROP TABLE users; --"),
            "\"test); DROP TABLE users; --\""
        );
    }

    #[test]
    fn test_count_conversation_messages() {
        let db = Database::in_memory().expect("Failed to create database");
        let conv_id = "test-conv-count";

        // Insert conversation
        db.insert_conversation(conv_id, None, None, "test-model", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        // Insert messages
        for i in 0..10 {
            db.insert_message(conv_id, "user", &format!("Message {}", i), Utc::now())
                .expect("Failed to insert message");
        }

        // Count
        let count = db
            .count_conversation_messages(conv_id)
            .expect("Failed to count");
        assert_eq!(count, 10);

        // Non-existent conversation
        let count = db
            .count_conversation_messages("nonexistent")
            .expect("Failed to count");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_conversation_exists() {
        let db = Database::in_memory().expect("Failed to create database");
        let conv_id = "test-conv-exists";

        // Insert conversation
        db.insert_conversation(conv_id, None, None, "test-model", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        // Check exists
        assert!(db.conversation_exists(conv_id).expect("Failed to check"));
        assert!(!db
            .conversation_exists("nonexistent")
            .expect("Failed to check"));
    }

    #[test]
    fn test_delete_conversation() {
        let db = Database::in_memory().expect("Failed to create database");
        let conv_id = "test-conv-delete";

        // Insert conversation with messages
        db.insert_conversation(conv_id, None, None, "test-model", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");
        db.insert_message(conv_id, "user", "Hello", Utc::now())
            .expect("Failed to insert message");

        // Verify exists
        assert!(db.conversation_exists(conv_id).expect("Failed to check"));
        assert_eq!(
            db.count_conversation_messages(conv_id)
                .expect("Failed to count"),
            1
        );

        // Delete
        db.delete_conversation(conv_id).expect("Failed to delete");

        // Verify deleted
        assert!(!db.conversation_exists(conv_id).expect("Failed to check"));
        assert_eq!(
            db.count_conversation_messages(conv_id)
                .expect("Failed to count"),
            0
        );
    }
}
