//! Database operations for conversation and session management
//!
//! Provides:
//! - Conversation CRUD operations
//! - Session metadata operations
//! - Todo list operations
//! - SourceType for content classification

use chrono::{DateTime, Utc};
use rusqlite::{Result, params};
use serde::{Deserialize, Serialize};

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

/// Source type for retrieved content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    #[default]
    Conversation,
    Document,
    Note,
    Web,
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

/// Parameters for updating conversation metadata
#[derive(Debug, Clone)]
pub struct ConversationMetadataParams<'a> {
    /// Conversation ID
    pub id: &'a str,
    /// New name for the conversation
    pub name: Option<&'a str>,
    /// Model name
    pub model: &'a str,
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
    pub priority: String,
    pub tags: String,
    pub created_at: DateTime<Utc>,
}

/// Session summary for listing
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub name: Option<String>,
    #[allow(dead_code)] // Available for TUI session list display
    pub model: String,
    pub message_count: usize,
    #[allow(dead_code)] // Available for TUI session list display
    pub created_at: DateTime<Utc>,
    #[allow(dead_code)] // Available for TUI session list display
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

    /// Get the most recent session ID for a project
    ///
    /// Returns the session ID with the highest `updated_at` timestamp.
    /// Returns None if no sessions exist for the project.
    pub fn get_last_session_id(&self, project_id: Option<&str>) -> Result<Option<String>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql = if project_id.is_some() {
                "SELECT id FROM conversations WHERE project_id = ?1 ORDER BY updated_at DESC LIMIT 1"
            } else {
                "SELECT id FROM conversations ORDER BY updated_at DESC LIMIT 1"
            };

            let mut stmt = conn.prepare(sql)?;

            if let Some(pid) = project_id {
                let result = stmt.query_row(params![pid], |row| row.get::<_, String>(0));
                match result {
                    Ok(id) => Ok(Some(id)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            } else {
                let result = stmt.query_row([], |row| row.get::<_, String>(0));
                match result {
                    Ok(id) => Ok(Some(id)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e),
                }
            }
        })
    }

    /// List sessions for a project
    ///
    /// Returns session info including name, model, message count, and timestamps.
    pub fn list_sessions(&self, project_id: Option<&str>) -> Result<Vec<SessionSummary>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            let sql = if project_id.is_some() {
                "SELECT id, title, model, created_at, updated_at,
                        (SELECT COUNT(*) FROM content_items WHERE conversation_id = c.id AND content_type = 'message') as message_count
                 FROM conversations c
                 WHERE project_id = ?1
                 ORDER BY updated_at DESC"
            } else {
                "SELECT id, title, model, created_at, updated_at,
                        (SELECT COUNT(*) FROM content_items WHERE conversation_id = c.id AND content_type = 'message') as message_count
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
    /// Updates session-specific fields: model, system_prompt, compacted_summary,
    /// compacted_range, think, tools, tool_output_level.
    pub fn update_conversation_metadata(
        &self,
        params: &ConversationMetadataParams<'_>,
    ) -> Result<()> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "UPDATE conversations SET 
                    title = COALESCE(?1, title),
                    model = ?2,
                    system_prompt = ?3,
                    compacted_summary = ?4,
                    compacted_range_start = ?5,
                    compacted_range_end = ?6,
                    think = ?7,
                    tools = ?8,
                    tool_output_level = ?9,
                    updated_at = ?10
                 WHERE id = ?11",
                params![
                    params.name,
                    params.model,
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

    /// Find conversation by ID or name (title).
    ///
    /// First tries exact ID match. If not found, tries name match.
    /// Returns the conversation ID if found, or None if not found.
    pub fn find_conversation(&self, id_or_name: &str) -> Result<Option<String>> {
        self.with_connection(|conn: &rusqlite::Connection| {
            // Try exact ID match first
            let id_exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM conversations WHERE id = ?1",
                params![id_or_name],
                |row| row.get(0),
            )?;

            if id_exists {
                return Ok(Some(id_or_name.to_string()));
            }

            // Try name (title) match
            let found_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM conversations WHERE title = ?1 LIMIT 1",
                    params![id_or_name],
                    |row| row.get(0),
                )
                .ok();

            Ok(found_id)
        })
    }

    /// Get conversation metadata by ID or name.
    ///
    /// First tries exact ID match. If not found, tries name match.
    pub fn get_conversation_by_id_or_name(&self, id_or_name: &str) -> Result<ConversationMetadata> {
        // Try exact ID first
        match self.get_conversation_metadata(id_or_name) {
            Ok(meta) => Ok(meta),
            Err(_) => {
                // Try name match
                let id = self
                    .find_conversation(id_or_name)?
                    .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
                self.get_conversation_metadata(&id)
            }
        }
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
                    "INSERT INTO session_todos (conversation_id, task_id, description, status, priority, tags, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        conversation_id,
                        todo.task_id,
                        todo.description,
                        todo.status,
                        todo.priority,
                        todo.tags,
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
            // Check if priority and tags columns exist (migration v9)
            let columns: Vec<String> = {
                let mut stmt = conn.prepare("PRAGMA table_info(session_todos)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<String>, _>>()?
            };

            let has_priority_tags =
                columns.contains(&"priority".to_string()) && columns.contains(&"tags".to_string());

            let rows: Vec<TodoRow> = if has_priority_tags {
                let mut stmt = conn.prepare(
                    "SELECT task_id, description, status, priority, tags, created_at 
                     FROM session_todos 
                     WHERE conversation_id = ?1 
                     ORDER BY task_id ASC",
                )?;

                let rows = stmt.query_map(params![conversation_id], |row| {
                    let timestamp: i64 = row.get(5)?;
                    let created_at =
                        chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or_else(Utc::now);
                    Ok(TodoRow {
                        task_id: row.get(0)?,
                        description: row.get(1)?,
                        status: row.get(2)?,
                        priority: row.get(3)?,
                        tags: row.get(4)?,
                        created_at,
                    })
                })?;

                rows.collect::<Result<Vec<_>, _>>()?
            } else {
                // Fallback for pre-v9 schema (no priority/tags columns)
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
                        priority: "medium".to_string(),
                        tags: String::new(),
                        created_at,
                    })
                })?;

                rows.collect::<Result<Vec<_>, _>>()?
            };

            Ok(rows)
        })
    }

    /// Adjust the importance of a content item by a delta, clamped to [0.0, 1.0].
    ///
    /// Used by the /feedback command to reflect user sentiment:
    /// - Good feedback: importance + 0.05 (capped at 1.0)
    /// - Bad feedback: importance - 0.1 (floored at 0.0)
    /// - Correction: no importance change
    pub fn adjust_importance(&self, item_id: i64, delta: f32) -> Result<(), String> {
        self.with_connection(|conn: &rusqlite::Connection| {
            conn.execute(
                "UPDATE content_items SET importance = MIN(1.0, MAX(0.0, importance + ?1)) WHERE id = ?2",
                params![delta, item_id],
            )?;
            Ok(())
        })
        .map_err(|e| format!("Error adjusting importance for item {}: {}", item_id, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fts5_escape() {
        assert_eq!(fts5_escape("hello"), "\"hello\"");
        assert_eq!(fts5_escape("hello world"), "\"hello world\"");
        assert_eq!(fts5_escape("test AND other"), "\"test AND other\"");
        assert_eq!(fts5_escape("test OR other"), "\"test OR other\"");
        assert_eq!(fts5_escape("test NOT other"), "\"test NOT other\"");
        assert_eq!(fts5_escape("test*"), "\"test*\"");

        // Quote escaping
        assert_eq!(fts5_escape("test\"quote"), "\"test\"\"quote\"");
        assert_eq!(fts5_escape("a\"b\"c"), "\"a\"\"b\"\"c\"");

        // Special chars
        assert_eq!(fts5_escape("test()"), "\"test()\"");

        // Injection attempt
        assert_eq!(
            fts5_escape("test); DROP TABLE users; --"),
            "\"test); DROP TABLE users; --\""
        );
    }

    #[test]
    fn test_source_type() {
        assert_eq!(SourceType::Conversation.prefix(), "msg");
        assert_eq!(SourceType::Document.prefix(), "doc");
        assert_eq!(SourceType::Note.prefix(), "note");
        assert_eq!(SourceType::Web.prefix(), "web");

        assert_eq!(
            SourceType::from_prefix("msg"),
            Some(SourceType::Conversation)
        );
        assert_eq!(
            SourceType::from_prefix("conversation"),
            Some(SourceType::Conversation)
        );
        assert_eq!(SourceType::from_prefix("doc"), Some(SourceType::Document));
        assert_eq!(SourceType::from_prefix("note"), Some(SourceType::Note));
        assert_eq!(SourceType::from_prefix("web"), Some(SourceType::Web));
        assert_eq!(SourceType::from_prefix("unknown"), None);
    }

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
}
