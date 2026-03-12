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
use crate::consts::roles::ROLE_USER;

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
    /// Timestamp (Unix epoch)
    pub timestamp: i64,
    /// Source type (conversation, document, note, web)
    pub source_type: SourceType,
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
    /// Message type: "normal" or "pre_tool_content"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    /// Previous message ID (for navigation, only for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_message_id: Option<i64>,
    /// Subsequent assistant messages (for conversation context)
    /// Renamed from `next_message` to support multiple messages
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subsequent_messages: Vec<SearchResult>,
    /// Token count from Ollama's prompt_eval_count (cumulative - includes all prompt tokens)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i64>,
}

/// Source type for retrieved content
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

/// Source type for retrieved content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    #[default]
    Conversation,
    Document,
    Note,
    Web,
}

/// Parameters for hybrid search
#[derive(Debug, Clone)]
pub struct SearchParams<'a> {
    /// Search query
    pub query: &'a str,
    /// Query embedding for semantic search
    pub embedding: &'a [f32],
    /// Filter by conversation ID
    pub conversation_id: Option<&'a str>,
    /// Filter by project ID
    pub project_id: Option<&'a str>,
    /// Maximum results to return
    pub limit: usize,
    /// Weight for keyword search (BM25)
    pub keyword_weight: f32,
    /// Weight for semantic search (vector)
    pub semantic_weight: f32,
    /// Message IDs to exclude from results
    pub exclude_ids: Option<&'a [i64]>,
}

/// Parameters for updating conversation metadata
#[derive(Debug, Clone)]
pub struct ConversationMetadataParams<'a> {
    /// Conversation ID
    pub id: &'a str,
    /// New name for the conversation
    pub name: Option<&'a str>,
    /// System prompt
    pub system_prompt: Option<&'a str>,
    /// Compacted summary
    pub compacted_summary: Option<&'a str>,
    /// Range of compacted messages (start, end)
    pub compacted_range: Option<(usize, usize)>,
    /// Whether thinking is enabled
    pub think: bool,
    /// Whether tools are enabled
    pub tools: bool,
    /// Tool output verbosity level
    pub tool_output_level: &'a str,
    /// Update timestamp
    pub updated_at: DateTime<Utc>,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Conversation => write!(f, "conversation"),
            SourceType::Document => write!(f, "document"),
            SourceType::Note => write!(f, "note"),
            SourceType::Web => write!(f, "web"),
        }
    }
}

impl SourceType {
    /// Get the prefix for a source type (used in message IDs)
    pub fn prefix(&self) -> &'static str {
        match self {
            SourceType::Conversation => "msg",
            SourceType::Document => "doc",
            SourceType::Note => "note",
            SourceType::Web => "web",
        }
    }

    /// Parse a source type from a prefix
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "msg" | "conversation" => Some(SourceType::Conversation),
            "doc" => Some(SourceType::Document),
            "note" => Some(SourceType::Note),
            "web" => Some(SourceType::Web),
            _ => None,
        }
    }
}

/// Conversation metadata from the database
#[derive(Debug, Clone)]
pub struct ConversationMetadata {
    pub id: String,
    pub project_id: Option<String>,
    pub name: Option<String>,
    pub model: String,
    pub system_prompt: Option<String>,
    pub compacted_summary: Option<String>,
    pub compacted_range: Option<(usize, usize)>,
    pub think: bool,
    pub tools: bool,
    pub tool_output_level: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Todo row from the database
#[derive(Debug, Clone)]
pub struct TodoRow {
    pub task_id: usize,
    pub description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Session summary for listing
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub name: Option<String>,
    pub model: String,
    pub message_count: usize,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
        // Use insert_message_with_type with default type "normal"
        self.insert_message_with_type(conversation_id, role, content, timestamp, "normal")
    }

    /// Insert a message with a specific type
    ///
    /// # Arguments
    /// * `conversation_id` - The conversation ID
    /// * `role` - Message role (user, assistant, system, tool)
    /// * `content` - Message content
    /// * `timestamp` - Message timestamp
    /// * `message_type` - Message type ("normal" or "pre_tool_content")
    ///
    /// # Returns
    /// The message ID of the inserted row
    pub fn insert_message_with_type(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        timestamp: DateTime<Utc>,
        message_type: &str,
    ) -> Result<i64> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "INSERT INTO messages (conversation_id, role, content, timestamp, importance, has_embedding, message_type)
                 VALUES (?1, ?2, ?3, ?4, 0.5, 0, ?5)",
                params![
                    conversation_id,
                    role,
                    content,
                    timestamp.timestamp(),
                    message_type,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Delete a message and all its associated data (embeddings, chunks)
    /// This is used by /undo command to clean up properly
    /// Note: Currently unused, kept for future single-message operations
    #[allow(dead_code)]
    pub fn delete_message(&self, message_id: i64) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            // Delete from message_embeddings (vec0 requires special handling)
            conn.execute(
                "DELETE FROM message_embeddings WHERE message_id = ?1",
                params![message_id],
            )?;

            // Get chunk IDs for this message before deleting chunks
            let chunk_ids: Vec<i64> = {
                let mut stmt =
                    conn.prepare("SELECT id FROM message_chunks WHERE message_id = ?1")?;
                stmt.query_map(params![message_id], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect()
            };

            // Delete chunk embeddings
            for chunk_id in &chunk_ids {
                conn.execute(
                    "DELETE FROM chunk_embeddings WHERE chunk_id = ?1",
                    params![chunk_id],
                )?;
            }

            // Delete message chunks
            conn.execute(
                "DELETE FROM message_chunks WHERE message_id = ?1",
                params![message_id],
            )?;

            // Delete the message itself
            conn.execute("DELETE FROM messages WHERE id = ?1", params![message_id])?;

            Ok(())
        })
    }

    /// Delete the last N messages from a conversation (used by /undo)
    /// Returns the number of messages actually deleted
    pub fn delete_last_messages(&self, conversation_id: &str, count: usize) -> Result<usize> {
        if count == 0 {
            return Ok(0);
        }

        self.with_connection(|conn: &rusqlite::Connection| {
            // Get the IDs of the last N messages
            let message_ids: Vec<i64> = {
                let mut stmt = conn.prepare(
                    "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
                )?;
                stmt.query_map(params![conversation_id, count as i64], |row| row.get(0))?
                    .filter_map(|r| r.ok())
                    .collect()
            };
            
            // Delete embeddings and chunks for each message
            for msg_id in &message_ids {
                // Delete message embeddings
                conn.execute(
                    "DELETE FROM message_embeddings WHERE message_id = ?1",
                    params![msg_id],
                )?;
                
                // Get chunk IDs
                let chunk_ids: Vec<i64> = {
                    let mut stmt = conn.prepare(
                        "SELECT id FROM message_chunks WHERE message_id = ?1"
                    )?;
                    stmt.query_map(params![msg_id], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect()
                };
                
                // Delete chunk embeddings
                for chunk_id in &chunk_ids {
                    conn.execute(
                        "DELETE FROM chunk_embeddings WHERE chunk_id = ?1",
                        params![chunk_id],
                    )?;
                }
                
                // Delete chunks
                conn.execute(
                    "DELETE FROM message_chunks WHERE message_id = ?1",
                    params![msg_id],
                )?;
            }
            
            // Delete messages
            let deleted = conn.execute(
                "DELETE FROM messages WHERE conversation_id = ?1 AND id IN (
                    SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT ?2
                )",
                params![conversation_id, count as i64],
            )?;
            
            Ok(deleted)
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

    /// Update a message with its prompt token count
    ///
    /// This stores the cumulative token count from Ollama's prompt_eval_count.
    /// The value is cumulative (includes system prompt + tools + all history + current message).
    pub fn update_message_prompt_tokens(&self, message_id: i64, prompt_tokens: u64) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "UPDATE messages SET prompt_tokens = ?1 WHERE id = ?2",
                params![prompt_tokens as i64, message_id],
            )?;
            Ok(())
        })
    }

    /// Search messages using full-text search (BM25)
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `conversation_id` - Specific conversation to search (None = all conversations)
    /// * `project_id` - Project to search (None = all projects, only used if conversation_id is None)
    /// * `limit` - Maximum results to return
    pub fn search_keyword(
        &self,
        query: &str,
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // Escape the query for FTS5 to prevent syntax errors and injection
        let escaped_query = fts5_escape(query);

        self.with_connection(|conn: &rusqlite::Connection| {
            let mut results = Vec::new();

            // Build query based on filters
            // conversation_id takes priority over project_id
            if let Some(conv_id) = conversation_id {
                let sql = r#"SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, bm25(messages_fts) as score
                    FROM messages_fts fts
                    JOIN messages m ON fts.rowid = m.id
                    WHERE messages_fts MATCH ?1 AND m.conversation_id = ?2
                    ORDER BY score ASC
                    LIMIT ?3"#;
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(params![escaped_query, conv_id, limit as i32], |row| {
                    Ok(SearchResult {
                        message_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        score: row.get::<_, f32>(5)?,
                        search_type: SearchType::Keyword,
                        source_type: SourceType::Conversation,
                        chunk_content: None,
                        chunk_start: None,
                        chunk_end: None,
                        message_type: None,
                        previous_message_id: None,
                        subsequent_messages: vec![],
                        prompt_tokens: None,
                    })
                })?;
                for r in rows {
                    results.push(r?);
                }
                return Ok(results);
            }

            // No conversation_id specified, check project_id
            if let Some(proj_id) = project_id {
                let sql = r#"SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, bm25(messages_fts) as score
                    FROM messages_fts fts
                    JOIN messages m ON fts.rowid = m.id
                    JOIN conversations c ON m.conversation_id = c.id
                    WHERE messages_fts MATCH ?1 AND c.project_id = ?2
                    ORDER BY score ASC
                    LIMIT ?3"#;
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map(params![escaped_query, proj_id, limit as i32], |row| {
                    Ok(SearchResult {
                        message_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        score: row.get::<_, f32>(5)?,
                        search_type: SearchType::Keyword,
                        source_type: SourceType::Conversation,
                        chunk_content: None,
                        chunk_start: None,
                        chunk_end: None,
                        message_type: None,
                        previous_message_id: None,
                        subsequent_messages: vec![],
                        prompt_tokens: None,
                    })
                })?;
                for r in rows {
                    results.push(r?);
                }
                return Ok(results);
            }

            // No filters - search all messages
            let sql = r#"SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, bm25(messages_fts) as score
                FROM messages_fts fts
                JOIN messages m ON fts.rowid = m.id
                WHERE messages_fts MATCH ?1
                ORDER BY score ASC
                LIMIT ?2"#;
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![escaped_query, limit as i32], |row| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    score: row.get::<_, f32>(5)?,
                    search_type: SearchType::Keyword,
                    source_type: SourceType::Conversation,
                    chunk_content: None,
                    chunk_start: None,
                    chunk_end: None,
                    message_type: None,
                    previous_message_id: None,
                    subsequent_messages: vec![],
                    prompt_tokens: None,
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
    /// Additional filters (like conversation_id, project_id) must be applied after retrieval.
    /// See: https://github.com/asg017/sqlite-vec
    ///
    /// Searches both:
    /// - Short messages (stored in message_embeddings)
    /// - Chunks of long messages (stored in chunk_embeddings)
    pub fn search_semantic(
        &self,
        embedding: &[f32],
        conversation_id: Option<&str>,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let embedding_bytes = embedding.as_bytes();

            // sqlite-vec KNN: only embedding MATCH and k=? allowed in WHERE
            // We fetch more results and filter in application code if needed
            let fetch_limit = if conversation_id.is_some() || project_id.is_some() {
                // Fetch 3x more when filtering to ensure enough results
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
                    source_type: SourceType::Conversation,
                    score: row.get::<_, f32>(5)?,
                    search_type: SearchType::Semantic,
                    chunk_content: row.get(6)?,
                    chunk_start: row.get(7)?,
                    chunk_end: row.get(8)?,
                    message_type: None,
                    previous_message_id: None,
                    subsequent_messages: vec![],
                    prompt_tokens: None,
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
                    source_type: SourceType::Conversation,
                    score: row.get::<_, f32>(5)?,
                    search_type: SearchType::Semantic,
                    chunk_content: row.get(6)?,
                    chunk_start: row.get(7)?,
                    chunk_end: row.get(8)?,
                    message_type: None,
                    previous_message_id: None,
                    subsequent_messages: vec![],
                    prompt_tokens: None,
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

            // Filter by conversation_id or project_id in application code
            // conversation_id takes priority over project_id
            if let Some(conv_id) = conversation_id {
                results.retain(|r| r.conversation_id == conv_id);
                results.truncate(limit);
            } else if let Some(proj_id) = project_id {
                // Fetch project_id for each conversation and filter
                let conv_ids: Vec<&str> = results.iter().map(|r| r.conversation_id.as_str()).collect();
                if conv_ids.is_empty() {
                    return Ok(Vec::new());
                }
                
                let placeholders: Vec<String> = conv_ids.iter().map(|_| "?".to_string()).collect();
                let placeholders = placeholders.join(",");
                
                let sql_project = format!(
                    "SELECT id, project_id FROM conversations WHERE id IN ({})",
                    placeholders
                );
                let mut stmt = conn.prepare(&sql_project)?;
                let params: Vec<&str> = conv_ids.to_vec();
                let project_map: HashMap<String, Option<String>> = stmt
                    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                
                results.retain(|r| {
                    project_map
                        .get(&r.conversation_id)
                        .and_then(|opt| opt.as_ref())
                        .map(|p| p == proj_id)
                        .unwrap_or(false)
                });
                results.truncate(limit);
            } else {
                results.truncate(limit);
            }

            Ok(results)
        })
    }

    /// Hybrid search using Reciprocal Rank Fusion
    ///
    /// # Arguments
    /// * `params` - Search parameters
    pub fn search_hybrid(&self, params: &SearchParams<'_>) -> Result<Vec<SearchResult>> {
        // Get keyword results (more = better fusion)
        let keyword_results = self.search_keyword(
            params.query,
            params.conversation_id,
            params.project_id,
            params.limit * 2,
        )?;

        // Get semantic results (more = better fusion)
        let semantic_results = self.search_semantic(
            params.embedding,
            params.conversation_id,
            params.project_id,
            params.limit * 2,
        )?;

        // Combine with RRF
        let mut results = reciprocal_rank_fusion(
            keyword_results,
            semantic_results,
            params.keyword_weight,
            params.semantic_weight,
            params.limit * 2, // Get more results before filtering
        );

        // Filter out excluded IDs
        if let Some(exclude) = params.exclude_ids {
            results.retain(|r| !exclude.contains(&r.message_id));
        }

        // Truncate to final limit
        results.truncate(params.limit);

        Ok(results)
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
                    let sql = "SELECT id, conversation_id, role, content, timestamp, prompt_tokens FROM messages 
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
                                source_type: SourceType::Conversation,
                                score: 0.0,
                                search_type: SearchType::Hybrid,
                                chunk_content: None,
                                chunk_start: None,
                                chunk_end: None,
                                message_type: None,
                                previous_message_id: None,
                                subsequent_messages: vec![],
                                prompt_tokens: row.get(5)?,
                            })
                        },
                    )?;
                    for r in rows {
                        results.push(r?);
                    }
                }
                None => {
                    let sql = "SELECT id, conversation_id, role, content, timestamp, prompt_tokens FROM messages 
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
                                source_type: SourceType::Conversation,
                                score: 0.0,
                                search_type: SearchType::Hybrid,
                                chunk_content: None,
                                chunk_start: None,
                                chunk_end: None,
                                message_type: None,
                                previous_message_id: None,
                                subsequent_messages: vec![],
                                prompt_tokens: row.get(5)?,
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
    /// Used by /forget command to completely remove conversation history.
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
            let sql =
                "SELECT id, conversation_id, role, content, timestamp, prompt_tokens FROM messages 
                       WHERE id = ?1";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query_map(params![message_id], |row: &rusqlite::Row<'_>| {
                Ok(SearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                    source_type: SourceType::Conversation,
                    score: 1.0,
                    search_type: SearchType::Keyword,
                    chunk_content: None,
                    chunk_start: None,
                    chunk_end: None,
                    message_type: None,
                    previous_message_id: None,
                    subsequent_messages: vec![],
                    prompt_tokens: row.get(5)?,
                })
            })?;

            rows.next().transpose()
        })
    }

    /// Get the next message by role in a conversation
    ///
    /// Used for conversation-aware retrieval: find the assistant response
    /// that follows a user question in the same conversation.
    ///
    /// # Arguments
    /// * `after_message_id` - The message ID to search after
    /// * `conversation_id` - The conversation ID (ensures we don't cross sessions)
    /// * `role` - The role to find (typically "assistant")
    ///
    /// # Returns
    /// The next message with the specified role, or None if not found
    pub fn get_next_message_by_role(
        &self,
        after_message_id: i64,
        conversation_id: &str,
        role: &str,
    ) -> Result<Option<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql = "SELECT id, conversation_id, role, content, timestamp 
                       FROM messages 
                       WHERE conversation_id = ?1 
                         AND id > ?2 
                         AND role = ?3
                       ORDER BY id ASC 
                       LIMIT 1";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query_map(
                params![conversation_id, after_message_id, role],
                |row: &rusqlite::Row<'_>| {
                    Ok(SearchResult {
                        message_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        score: 1.0,
                        search_type: SearchType::Keyword,
                        source_type: SourceType::Conversation,
                        chunk_content: None,
                        chunk_start: None,
                        chunk_end: None,
                        message_type: None,
                        previous_message_id: None,
                        subsequent_messages: vec![],
                        prompt_tokens: None,
                    })
                },
            )?;

            rows.next().transpose()
        })
    }

    /// Get all subsequent assistant messages after a given message
    ///
    /// Used for conversation-aware retrieval: when a user message is found,
    /// retrieve all assistant messages that follow until:
    /// - Up to 5 messages (limit)
    /// - End of conversation
    ///
    /// # Arguments
    /// * `after_message_id` - The message ID to search after
    /// * `conversation_id` - The conversation ID
    ///
    /// # Returns
    /// Vector of subsequent assistant messages (may be empty)
    pub fn get_subsequent_assistant_messages(
        &self,
        after_message_id: i64,
        conversation_id: &str,
    ) -> Result<Vec<SearchResult>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql =
                "SELECT id, conversation_id, role, content, timestamp, message_type, prompt_tokens
                       FROM messages 
                       WHERE conversation_id = ?1 
                         AND id > ?2 
                         AND role = 'assistant'
                       ORDER BY id ASC
                       LIMIT 5";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(
                params![conversation_id, after_message_id],
                |row: &rusqlite::Row<'_>| {
                    Ok(SearchResult {
                        message_id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        source_type: SourceType::Conversation,
                        score: 1.0,
                        search_type: SearchType::Keyword,
                        chunk_content: None,
                        chunk_start: None,
                        chunk_end: None,
                        message_type: row.get(5)?,
                        previous_message_id: None,
                        subsequent_messages: vec![],
                        prompt_tokens: row.get(6)?,
                    })
                },
            )?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
    }

    /// Get the ID of the previous message in a conversation
    ///
    /// # Arguments
    /// * `message_id` - The current message ID
    /// * `conversation_id` - The conversation ID
    ///
    /// # Returns
    /// The ID of the previous message, or None if this is the first message
    #[allow(dead_code)]
    pub fn get_previous_message_id(
        &self,
        message_id: i64,
        conversation_id: &str,
    ) -> Result<Option<i64>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql = "SELECT id FROM messages 
                       WHERE conversation_id = ?1 
                         AND id < ?2 
                       ORDER BY id DESC 
                       LIMIT 1";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query_map(
                params![conversation_id, message_id],
                |row: &rusqlite::Row<'_>| row.get(0),
            )?;

            rows.next().transpose()
        })
    }

    /// Enrich search results with conversation context
    ///
    /// For user messages, attaches all subsequent assistant messages (up to 5).
    /// This ensures question-answer pairs are retrieved together, addressing the issue
    /// where short questions have high similarity but long answers have low similarity.
    ///
    /// For isolated assistant messages (found directly in search), no enrichment is done.
    ///
    /// # Arguments
    /// * `results` - Search results from hybrid search
    ///
    /// # Returns
    /// Results with subsequent_messages populated for user messages
    pub fn enrich_with_context(&self, results: Vec<SearchResult>) -> Result<Vec<SearchResult>> {
        let mut enriched = Vec::with_capacity(results.len());
        let mut seen_ids = std::collections::HashSet::new();

        for result in results {
            seen_ids.insert(result.message_id);

            let subsequent_messages = if result.role == ROLE_USER {
                // Get all subsequent assistant messages
                let messages = self.get_subsequent_assistant_messages(
                    result.message_id,
                    &result.conversation_id,
                )?;

                // Filter out duplicates
                messages
                    .into_iter()
                    .filter(|m| !seen_ids.contains(&m.message_id))
                    .take(5)
                    .collect()
            } else {
                // Isolated assistant message: no enrichment
                vec![]
            };

            // Mark as seen
            for msg in &subsequent_messages {
                seen_ids.insert(msg.message_id);
            }

            enriched.push(SearchResult {
                subsequent_messages,
                ..result
            });
        }

        Ok(enriched)
    }

    /// Check if a conversation exists
    ///
    /// Used by tests to verify conversation creation.
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

    /// List sessions for a project
    ///
    /// Returns session info including name, model, message count, and timestamps.
    pub fn list_sessions(&self, project_id: Option<&str>) -> Result<Vec<SessionSummary>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql = if project_id.is_some() {
                "SELECT id, title, model, created_at, updated_at,
                        (SELECT COUNT(*) FROM messages WHERE conversation_id = c.id) as message_count
                 FROM conversations c
                 WHERE project_id = ?1
                 ORDER BY updated_at DESC"
            } else {
                "SELECT id, title, model, created_at, updated_at,
                        (SELECT COUNT(*) FROM messages WHERE conversation_id = c.id) as message_count
                 FROM conversations c
                 ORDER BY updated_at DESC"
            };

            let mut stmt = conn.prepare(sql)?;

            if let Some(pid) = project_id {
                let rows = stmt.query_map(params![pid], |row| {
                    let created_at_ts: i64 = row.get(3)?;
                    let updated_at_ts: i64 = row.get(4)?;
                    let msg_count: i64 = row.get(5)?;

                    Ok(SessionSummary {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        model: row.get(2)?,
                        message_count: msg_count as usize,
                        created_at: chrono::DateTime::from_timestamp(created_at_ts, 0)
                            .unwrap_or_else(Utc::now),
                        updated_at: chrono::DateTime::from_timestamp(updated_at_ts, 0)
                            .unwrap_or_else(Utc::now),
                    })
                })?;

                rows.collect::<Result<Vec<_>>>()
            } else {
                let rows = stmt.query_map([], |row| {
                    let created_at_ts: i64 = row.get(3)?;
                    let updated_at_ts: i64 = row.get(4)?;
                    let msg_count: i64 = row.get(5)?;

                    Ok(SessionSummary {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        model: row.get(2)?,
                        message_count: msg_count as usize,
                        created_at: chrono::DateTime::from_timestamp(created_at_ts, 0)
                            .unwrap_or_else(Utc::now),
                        updated_at: chrono::DateTime::from_timestamp(updated_at_ts, 0)
                            .unwrap_or_else(Utc::now),
                    })
                })?;

                rows.collect::<Result<Vec<_>>>()
            }
        })
    }

    /// Update conversation metadata (for session persistence)
    ///
    /// Updates session-specific fields: system_prompt, compacted_summary,
    /// compacted_range, think, tools, tool_output_level.
    #[allow(dead_code)]
    pub fn update_conversation_metadata(
        &self,
        params: &ConversationMetadataParams<'_>,
    ) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "UPDATE conversations SET 
                    title = COALESCE(?1, title),
                    system_prompt = ?2,
                    compacted_summary = ?3,
                    compacted_range_start = ?4,
                    compacted_range_end = ?5,
                    think = ?6,
                    tools = ?7,
                    tool_output_level = ?8,
                    updated_at = ?9
                 WHERE id = ?10",
                params![
                    params.name,
                    params.system_prompt,
                    params.compacted_summary,
                    params.compacted_range.map(|(s, _)| s as i64),
                    params.compacted_range.map(|(_, e)| e as i64),
                    params.think as i64,
                    params.tools as i64,
                    params.tool_output_level,
                    params.updated_at.timestamp(),
                    params.id,
                ],
            )?;
            Ok(())
        })
    }

    /// Get conversation metadata
    ///
    /// Returns session metadata for loading a saved session.
    pub fn get_conversation_metadata(&self, id: &str) -> Result<ConversationMetadata> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.query_row(
                "SELECT id, project_id, title, model, system_prompt, 
                        compacted_summary, compacted_range_start, compacted_range_end,
                        think, tools, tool_output_level, created_at, updated_at
                 FROM conversations WHERE id = ?1",
                params![id],
                |row| {
                    let created_at_ts: i64 = row.get(11)?;
                    let updated_at_ts: i64 = row.get(12)?;

                    Ok(ConversationMetadata {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        name: row.get(2)?,
                        model: row.get(3)?,
                        system_prompt: row.get(4)?,
                        compacted_summary: row.get(5)?,
                        compacted_range: match (
                            row.get::<_, Option<i64>>(6)?,
                            row.get::<_, Option<i64>>(7)?,
                        ) {
                            (Some(start), Some(end)) => Some((start as usize, end as usize)),
                            _ => None,
                        },
                        think: row
                            .get::<_, Option<i64>>(8)?
                            .map(|v| v != 0)
                            .unwrap_or(false),
                        tools: row
                            .get::<_, Option<i64>>(9)?
                            .map(|v| v != 0)
                            .unwrap_or(true),
                        tool_output_level: row
                            .get::<_, Option<String>>(10)?
                            .unwrap_or_else(|| "compact".to_string()),
                        created_at: chrono::DateTime::from_timestamp(created_at_ts, 0)
                            .unwrap_or_else(Utc::now),
                        updated_at: chrono::DateTime::from_timestamp(updated_at_ts, 0)
                            .unwrap_or_else(Utc::now),
                    })
                },
            )
        })
    }

    /// Save todos for a conversation
    ///
    /// Replaces all existing todos for the conversation.
    pub fn save_todos(&self, conversation_id: &str, todos: &[TodoRow]) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            // Delete existing todos
            conn.execute(
                "DELETE FROM session_todos WHERE conversation_id = ?1",
                params![conversation_id],
            )?;

            // Insert new todos
            for todo in todos {
                conn.execute(
                    "INSERT INTO session_todos (conversation_id, task_id, description, status, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        conversation_id,
                        todo.task_id,
                        todo.description,
                        todo.status,
                        todo.created_at.timestamp(),
                    ],
                )?;
            }

            Ok(())
        })
    }

    /// Get todos for a conversation
    pub fn get_todos(&self, conversation_id: &str) -> Result<Vec<TodoRow>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let mut stmt = conn.prepare(
                "SELECT task_id, description, status, created_at 
                 FROM session_todos 
                 WHERE conversation_id = ?1 
                 ORDER BY task_id ASC",
            )?;

            let rows = stmt.query_map(params![conversation_id], |row| {
                let timestamp: i64 = row.get(3)?;
                let created_at =
                    chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or_else(Utc::now);
                Ok(TodoRow {
                    task_id: row.get(0)?,
                    description: row.get(1)?,
                    status: row.get(2)?,
                    created_at,
                })
            })?;

            rows.collect::<Result<Vec<_>>>()
        })
    }

    /// Get all messages without embeddings for reindexing
    ///
    /// Used by recovery manager to generate missing embeddings.
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
                    source_type: SourceType::Conversation,
                    score: 0.0,
                    search_type: SearchType::Hybrid,
                    chunk_content: None,
                    chunk_start: None,
                    chunk_end: None,
                    message_type: None,
                    previous_message_id: None,
                    subsequent_messages: vec![],
                    prompt_tokens: None,
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
            .insert_message("test-conv", ROLE_USER, "Hello world", Utc::now())
            .expect("Failed to insert message");

        assert!(msg_id > 0);
    }

    #[test]
    fn test_update_embedding() {
        let db = Database::in_memory().expect("Failed to create database");

        db.insert_conversation("test-conv", None, None, "llama3.1", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        let msg_id = db
            .insert_message("test-conv", ROLE_USER, "Hello world", Utc::now())
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
        use crate::consts::roles::{ROLE_ASSISTANT, ROLE_USER};

        let db = Database::in_memory().expect("Failed to create database");

        db.insert_conversation("test-conv", None, None, "llama3.1", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        db.insert_message("test-conv", ROLE_USER, "Hello world from Rust", Utc::now())
            .expect("Failed to insert message");
        db.insert_message(
            "test-conv",
            ROLE_ASSISTANT,
            "How can I help you?",
            Utc::now(),
        )
        .expect("Failed to insert message");

        let results = db
            .search_keyword("Rust", None, None, 10)
            .expect("Failed to search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Hello world from Rust");
    }

    #[test]
    fn test_reciprocal_rank_fusion() {
        let make_result = |id: i64, content: &str| SearchResult {
            message_id: id,
            conversation_id: "test".to_string(),
            role: ROLE_USER.to_string(),
            content: content.to_string(),
            timestamp: 0,
            source_type: SourceType::Conversation,
            score: 0.0,
            search_type: SearchType::Keyword,
            chunk_content: None,
            chunk_start: None,
            chunk_end: None,
            message_type: None,
            previous_message_id: None,
            subsequent_messages: vec![],
            prompt_tokens: None,
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
            db.insert_message(conv_id, ROLE_USER, &format!("Message {}", i), Utc::now())
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
        db.insert_message(conv_id, ROLE_USER, "Hello", Utc::now())
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

    #[test]
    fn test_list_sessions() {
        use crate::consts::roles::{ROLE_ASSISTANT, ROLE_USER};
        let db = Database::in_memory().expect("Failed to create database");

        // Insert multiple conversations
        db.insert_conversation(
            "conv1",
            Some("project1"),
            Some("Session One"),
            "llama3.1",
            Utc::now(),
            Utc::now(),
        )
        .expect("Failed to insert conversation");
        db.insert_conversation(
            "conv2",
            Some("project1"),
            Some("Session Two"),
            "llama3.2",
            Utc::now(),
            Utc::now(),
        )
        .expect("Failed to insert conversation");
        db.insert_conversation(
            "conv3",
            Some("project2"),
            Some("Session Three"),
            "llama3.1",
            Utc::now(),
            Utc::now(),
        )
        .expect("Failed to insert conversation");

        // Add messages to conv1
        db.insert_message("conv1", ROLE_USER, "Hello", Utc::now())
            .expect("Failed to insert message");
        db.insert_message("conv1", ROLE_ASSISTANT, "Hi there", Utc::now())
            .expect("Failed to insert message");

        // List all sessions
        let all_sessions = db.list_sessions(None).expect("Failed to list sessions");
        assert_eq!(all_sessions.len(), 3);

        // List sessions for project1
        let project1_sessions = db
            .list_sessions(Some("project1"))
            .expect("Failed to list sessions");
        assert_eq!(project1_sessions.len(), 2);

        // List sessions for project2
        let project2_sessions = db
            .list_sessions(Some("project2"))
            .expect("Failed to list sessions");
        assert_eq!(project2_sessions.len(), 1);
        assert_eq!(project2_sessions[0].name, Some("Session Three".to_string()));
        assert_eq!(project2_sessions[0].model, "llama3.1");

        // Check message count
        let conv1_session = all_sessions
            .iter()
            .find(|s| s.id == "conv1")
            .expect("conv1 not found");
        assert_eq!(conv1_session.message_count, 2);
    }

    #[test]
    fn test_get_conversation_metadata() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert conversation with metadata
        db.insert_conversation(
            "test-meta",
            Some("project1"),
            Some("Test Session"),
            "llama3.1",
            Utc::now(),
            Utc::now(),
        )
        .expect("Failed to insert conversation");

        // Update metadata
        db.update_conversation_metadata(&ConversationMetadataParams {
            id: "test-meta",
            name: Some("Renamed Session"),
            system_prompt: Some("You are helpful"),
            compacted_summary: Some("Summary of conversation"),
            compacted_range: Some((5, 10)),
            think: true,
            tools: false,
            tool_output_level: "full",
            updated_at: Utc::now(),
        })
        .expect("Failed to update metadata");

        // Get metadata
        let meta = db
            .get_conversation_metadata("test-meta")
            .expect("Failed to get metadata");
        assert_eq!(meta.id, "test-meta");
        assert_eq!(meta.project_id, Some("project1".to_string()));
        assert_eq!(meta.name, Some("Renamed Session".to_string()));
        assert_eq!(meta.model, "llama3.1");
        assert_eq!(meta.system_prompt, Some("You are helpful".to_string()));
        assert_eq!(
            meta.compacted_summary,
            Some("Summary of conversation".to_string())
        );
        assert_eq!(meta.compacted_range, Some((5, 10)));
        assert!(meta.think);
        assert!(!meta.tools);
        assert_eq!(meta.tool_output_level, "full");
    }

    #[test]
    fn test_save_and_get_todos() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert conversation
        db.insert_conversation("test-todos", None, None, "llama3.1", Utc::now(), Utc::now())
            .expect("Failed to insert conversation");

        // Save todos
        let todos = vec![
            TodoRow {
                task_id: 1,
                description: "Task 1".to_string(),
                status: "pending".to_string(),
                created_at: Utc::now(),
            },
            TodoRow {
                task_id: 2,
                description: "Task 2".to_string(),
                status: "completed".to_string(),
                created_at: Utc::now(),
            },
        ];

        db.save_todos("test-todos", &todos)
            .expect("Failed to save todos");

        // Get todos
        let retrieved = db.get_todos("test-todos").expect("Failed to get todos");
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].description, "Task 1");
        assert_eq!(retrieved[0].status, "pending");
        assert_eq!(retrieved[1].description, "Task 2");
        assert_eq!(retrieved[1].status, "completed");

        // Update todos (replace)
        let updated_todos = vec![TodoRow {
            task_id: 1,
            description: "Updated Task".to_string(),
            status: "in_progress".to_string(),
            created_at: Utc::now(),
        }];

        db.save_todos("test-todos", &updated_todos)
            .expect("Failed to update todos");

        let retrieved = db.get_todos("test-todos").expect("Failed to get todos");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].description, "Updated Task");
        assert_eq!(retrieved[0].status, "in_progress");
    }
}
