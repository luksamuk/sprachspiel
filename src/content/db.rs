//! Database operations for content items
//!
//! Provides CRUD operations and search for content_items table.

use chrono::{DateTime, Utc};
use rusqlite::{params, Result};
use std::str::FromStr;

use super::types::{
    ContentItem, ContentScope, ContentSearchResult, ContentSearchType, ContentSource, ContentType,
    Note,
};
use crate::db::fts5_escape;
use crate::db::Database;

/// Normalize BM25 score to [0, 1) range
fn normalize_bm25_score(score: f32) -> f32 {
    if score >= 0.0 {
        0.0
    } else {
        (-score) / (1.0 - score)
    }
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
            let mut results = Vec::new();

            let sql = match (&scope, &project_id) {
                (Some(_), Some(_)) => {
                    "SELECT id, scope, source, title, content, importance, access_count,
                            decay_score, created_at, updated_at, last_accessed, project_id
                     FROM content_items WHERE content_type = 'note' 
                     AND scope = ?1 AND project_id = ?2
                     ORDER BY created_at DESC"
                }
                (Some(_), None) => {
                    "SELECT id, scope, source, title, content, importance, access_count,
                            decay_score, created_at, updated_at, last_accessed, project_id
                     FROM content_items WHERE content_type = 'note' 
                     AND scope = ?1
                     ORDER BY created_at DESC"
                }
                (None, Some(_)) => {
                    "SELECT id, scope, source, title, content, importance, access_count,
                            decay_score, created_at, updated_at, last_accessed, project_id
                     FROM content_items WHERE content_type = 'note' 
                     AND project_id = ?1
                     ORDER BY created_at DESC"
                }
                (None, None) => {
                    "SELECT id, scope, source, title, content, importance, access_count,
                            decay_score, created_at, updated_at, last_accessed, project_id
                     FROM content_items WHERE content_type = 'note'
                     ORDER BY created_at DESC"
                }
            };

            let mut stmt = conn.prepare(sql)?;

            let rows = match (&scope, &project_id) {
                (Some(s), Some(p)) => stmt.query_map(params![s.to_string(), p], row_to_note)?,
                (Some(s), None) => stmt.query_map(params![s.to_string()], row_to_note)?,
                (None, Some(p)) => stmt.query_map(params![p], row_to_note)?,
                (None, None) => stmt.query_map(params![], row_to_note)?,
            };

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
            let mut results = Vec::new();

            let sql = match (&scope, &project_id) {
                (Some(_), Some(_)) => {
                    "SELECT ci.id, ci.content_type, ci.conversation_id, ci.role, ci.message_type,
                            ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source, ci.title,
                            ci.content, ci.importance, ci.access_count, ci.decay_score,
                            ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding,
                            ci.project_id, bm25(content_fts) as score
                     FROM content_fts fts
                     JOIN content_items ci ON fts.rowid = ci.id
                     WHERE content_fts MATCH ?1 AND ci.content_type = 'note'
                     AND ci.scope = ?2 AND ci.project_id = ?3
                     ORDER BY score ASC
                     LIMIT ?4"
                }
                (Some(_), None) => {
                    "SELECT ci.id, ci.content_type, ci.conversation_id, ci.role, ci.message_type,
                            ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source, ci.title,
                            ci.content, ci.importance, ci.access_count, ci.decay_score,
                            ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding,
                            ci.project_id, bm25(content_fts) as score
                     FROM content_fts fts
                     JOIN content_items ci ON fts.rowid = ci.id
                     WHERE content_fts MATCH ?1 AND ci.content_type = 'note'
                     AND ci.scope = ?2
                     ORDER BY score ASC
                     LIMIT ?3"
                }
                (None, Some(_)) => {
                    "SELECT ci.id, ci.content_type, ci.conversation_id, ci.role, ci.message_type,
                            ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source, ci.title,
                            ci.content, ci.importance, ci.access_count, ci.decay_score,
                            ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding,
                            ci.project_id, bm25(content_fts) as score
                     FROM content_fts fts
                     JOIN content_items ci ON fts.rowid = ci.id
                     WHERE content_fts MATCH ?1 AND ci.content_type = 'note'
                     AND ci.project_id = ?2
                     ORDER BY score ASC
                     LIMIT ?3"
                }
                (None, None) => {
                    "SELECT ci.id, ci.content_type, ci.conversation_id, ci.role, ci.message_type,
                            ci.previous_item_id, ci.prompt_tokens, ci.scope, ci.source, ci.title,
                            ci.content, ci.importance, ci.access_count, ci.decay_score,
                            ci.created_at, ci.updated_at, ci.last_accessed, ci.has_embedding,
                            ci.project_id, bm25(content_fts) as score
                     FROM content_fts fts
                     JOIN content_items ci ON fts.rowid = ci.id
                     WHERE content_fts MATCH ?1 AND ci.content_type = 'note'
                     ORDER BY score ASC
                     LIMIT ?2"
                }
            };

            let mut stmt = conn.prepare(sql)?;
            let rows = match (&scope, &project_id) {
                (Some(s), Some(p)) => stmt
                    .query_map(
                        params![escaped_query, s.to_string(), p, limit as i32],
                        |row| Ok((row_to_content_item(row)?, row.get::<_, f32>(19)?)),
                    )?
                    .collect::<Result<Vec<_>, _>>()?,
                (Some(s), None) => stmt
                    .query_map(params![escaped_query, s.to_string(), limit as i32], |row| {
                        Ok((row_to_content_item(row)?, row.get::<_, f32>(19)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?,
                (None, Some(p)) => stmt
                    .query_map(params![escaped_query, p, limit as i32], |row| {
                        Ok((row_to_content_item(row)?, row.get::<_, f32>(19)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?,
                (None, None) => stmt
                    .query_map(params![escaped_query, limit as i32], |row| {
                        Ok((row_to_content_item(row)?, row.get::<_, f32>(19)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?,
            };

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
}
