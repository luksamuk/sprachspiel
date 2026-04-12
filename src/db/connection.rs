//! Database connection and initialization
//!
//! Handles SQLite connection with sqlite-vec extension loaded.

use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::schema::{SCHEMA_SQL, SCHEMA_VERSION, VERSION_SQL, set_version_sql};

/// Thread-safe database wrapper
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Create a new database at the default storage location
    pub fn new() -> Result<Self> {
        let path = Self::get_storage_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Self::open(&path)
    }

    /// Create an in-memory database (for testing)
    #[allow(dead_code)]
    pub fn in_memory() -> Result<Self> {
        // Initialize extension before opening connection
        Self::init_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        Self::init_connection(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open database at a specific path
    pub fn open(path: &PathBuf) -> Result<Self> {
        // Initialize extension before opening connection
        Self::init_sqlite_vec();
        let conn = Connection::open(path)?;
        Self::init_connection(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Initialize connection with extension and schema
    fn init_connection(conn: &Connection) -> Result<()> {
        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // Check and migrate schema
        let version: i32 = conn.query_row(VERSION_SQL, [], |row| row.get(0))?;
        if version < SCHEMA_VERSION {
            // Apply base schema
            conn.execute_batch(SCHEMA_SQL)?;

            // Apply incremental migrations
            Self::apply_migrations(conn, version)?;

            // Set version (not parameterized)
            conn.execute_batch(&set_version_sql(SCHEMA_VERSION))?;
        }

        Ok(())
    }

    /// Apply incremental schema migrations
    fn apply_migrations(conn: &Connection, from_version: i32) -> Result<()> {
        // Migration v2 -> v3: Add has_embedding to message_chunks
        if from_version < 3 {
            // Check if message_chunks table exists first
            let table_exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='message_chunks'",
                [],
                |row| row.get::<_, i32>(0),
            )? > 0;

            if table_exists {
                // Check if column already exists using pragma
                let column_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(message_chunks)")?;
                    let rows = stmt.query_map([], |row| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&"has_embedding".to_string())
                };

                if !column_exists {
                    conn.execute(
                        "ALTER TABLE message_chunks ADD COLUMN has_embedding INTEGER DEFAULT 0",
                        [],
                    )?;
                }

                // Create index for missing embeddings
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_chunks_missing_embedding 
                     ON message_chunks(has_embedding) WHERE has_embedding = 0",
                    [],
                )?;
            }
        }

        // Migration v3 -> v4: Add session metadata and todos
        if from_version < 4 {
            // Add columns to conversations table
            let conversations_columns = [
                ("system_prompt", "TEXT"),
                ("compacted_summary", "TEXT"),
                ("compacted_range_start", "INTEGER"),
                ("compacted_range_end", "INTEGER"),
                ("think", "INTEGER DEFAULT 0"),
                ("tools", "INTEGER DEFAULT 1"),
                ("tool_output_level", "TEXT DEFAULT 'compact'"),
            ];

            for (col_name, col_type) in conversations_columns {
                // Check if column exists
                let column_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(conversations)")?;
                    let rows = stmt.query_map([], |row| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&col_name.to_string())
                };

                if !column_exists {
                    conn.execute(
                        &format!(
                            "ALTER TABLE conversations ADD COLUMN {} {}",
                            col_name, col_type
                        ),
                        [],
                    )?;
                }
            }

            // Add prompt_tokens column to messages table (only if table exists - V2 legacy)
            let messages_exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |row| row.get::<_, i32>(0),
            )? > 0;

            if messages_exists {
                let prompt_tokens_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                    let rows = stmt.query_map([], |row| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&"prompt_tokens".to_string())
                };

                if !prompt_tokens_exists {
                    conn.execute("ALTER TABLE messages ADD COLUMN prompt_tokens INTEGER", [])?;
                }
            }

            // Create session_todos table if not exists
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS session_todos (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    conversation_id TEXT NOT NULL,
                    task_id INTEGER NOT NULL,
                    description TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                );
                
                CREATE INDEX IF NOT EXISTS idx_todos_conversation 
                    ON session_todos(conversation_id);
                "#,
            )?;
        }

        // Migration v4 -> v5: Add message_type and previous_message_id
        if from_version < 5 {
            // Only apply migrations if messages table exists (V2 legacy)
            let messages_exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |row| row.get::<_, i32>(0),
            )? > 0;

            if messages_exists {
                // Add message_type column to messages table
                let message_type_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                    let rows = stmt.query_map([], |row: &rusqlite::Row<'_>| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&"message_type".to_string())
                };

                if !message_type_exists {
                    conn.execute(
                        "ALTER TABLE messages ADD COLUMN message_type TEXT DEFAULT 'normal'",
                        [],
                    )?;
                }

                // Add previous_message_id column to messages table
                let previous_id_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                    let rows = stmt.query_map([], |row: &rusqlite::Row<'_>| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&"previous_message_id".to_string())
                };

                if !previous_id_exists {
                    conn.execute(
                        "ALTER TABLE messages ADD COLUMN previous_message_id INTEGER REFERENCES messages(id)",
                        [],
                    )?;
                }

                // Create index for previous_message_id lookups
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_messages_previous 
                     ON messages(previous_message_id) WHERE previous_message_id IS NOT NULL",
                    [],
                )?;
            }
        }

        // Migration v5 -> v6: Add facts table (factual memory system)
        if from_version < 6 {
            // Create facts table
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS facts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    scope TEXT NOT NULL CHECK(scope IN ('project', 'global')),
                    category TEXT NOT NULL CHECK(category IN ('preference', 'fact')),
                    content TEXT NOT NULL,
                    importance REAL DEFAULT 0.5 CHECK(importance BETWEEN 0 AND 1),
                    access_count INTEGER DEFAULT 0,
                    decay_score REAL DEFAULT 1.0,
                    created_at INTEGER NOT NULL,
                    last_accessed INTEGER NOT NULL,
                    source TEXT DEFAULT 'user' CHECK(source IN ('user', 'llm')),
                    invalidated_at INTEGER,
                    project_id TEXT
                );

                CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
                    content,
                    content='facts',
                    content_rowid='id',
                    tokenize='porter unicode61'
                );

                CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
                    INSERT INTO facts_fts(rowid, content) VALUES (new.id, new.content);
                END;

                CREATE TRIGGER IF NOT EXISTS facts_ad AFTER DELETE ON facts BEGIN
                    INSERT INTO facts_fts(facts_fts, rowid, content) 
                    VALUES('delete', old.id, old.content);
                END;

                CREATE TRIGGER IF NOT EXISTS facts_au AFTER UPDATE ON facts BEGIN
                    INSERT INTO facts_fts(facts_fts, rowid, content) 
                    VALUES('delete', old.id, old.content);
                    INSERT INTO facts_fts(rowid, content) VALUES (new.id, new.content);
                END;

                CREATE INDEX IF NOT EXISTS idx_facts_scope_category ON facts(scope, category);
                CREATE INDEX IF NOT EXISTS idx_facts_decay ON facts(decay_score) WHERE invalidated_at IS NULL;
                CREATE INDEX IF NOT EXISTS idx_facts_project ON facts(project_id) WHERE scope = 'project';
                CREATE INDEX IF NOT EXISTS idx_facts_access ON facts(last_accessed DESC);
                "#,
            )?;
        }

        // Migration v6 -> v7: Add content_items unified table
        if from_version < 7 {
            // Create content_items table
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS content_items (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    content_type TEXT NOT NULL CHECK(content_type IN ('message', 'note', 'document')),
                    conversation_id TEXT,
                    role TEXT CHECK(role IN ('user', 'assistant', 'system', 'tool')),
                    message_type TEXT DEFAULT 'normal',
                    previous_item_id INTEGER REFERENCES content_items(id),
                    prompt_tokens INTEGER,
                    scope TEXT CHECK(scope IN ('project', 'global')),
                    source TEXT CHECK(source IN ('user', 'llm')),
                    title TEXT,
                    content TEXT NOT NULL,
                    importance REAL DEFAULT 0.5,
                    access_count INTEGER DEFAULT 0,
                    decay_score REAL DEFAULT 1.0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    last_accessed INTEGER NOT NULL,
                    has_embedding INTEGER DEFAULT 0,
                    project_id TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_content_items_type ON content_items(content_type);
                CREATE INDEX IF NOT EXISTS idx_content_items_conversation ON content_items(conversation_id) WHERE conversation_id IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_content_items_project ON content_items(project_id) WHERE project_id IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_content_items_scope ON content_items(scope) WHERE scope IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_content_items_timestamp ON content_items(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_content_items_previous ON content_items(previous_item_id) WHERE previous_item_id IS NOT NULL;

                CREATE TABLE IF NOT EXISTS content_chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_id INTEGER NOT NULL REFERENCES content_items(id) ON DELETE CASCADE,
                    chunk_index INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    start_offset INTEGER NOT NULL,
                    end_offset INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    has_embedding INTEGER DEFAULT 0
                );

                CREATE INDEX IF NOT EXISTS idx_content_chunks_item ON content_chunks(item_id);
                CREATE INDEX IF NOT EXISTS idx_content_chunks_order ON content_chunks(item_id, chunk_index);

                CREATE VIRTUAL TABLE IF NOT EXISTS content_embeddings USING vec0(
                    item_id INTEGER PRIMARY KEY,
                    embedding FLOAT[256],
                    +content_type TEXT,
                    +conversation_id TEXT,
                    +project_id TEXT,
                    +timestamp INTEGER
                );

                CREATE VIRTUAL TABLE IF NOT EXISTS chunk_embeddings_v2 USING vec0(
                    chunk_id INTEGER PRIMARY KEY,
                    embedding FLOAT[256],
                    +content_type TEXT,
                    +conversation_id TEXT,
                    +project_id TEXT,
                    +timestamp INTEGER
                );

                CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(
                    content,
                    content='content_items',
                    content_rowid='id',
                    tokenize='porter unicode61'
                );

                CREATE TRIGGER IF NOT EXISTS content_items_ai AFTER INSERT ON content_items BEGIN
                    INSERT INTO content_fts(rowid, content) VALUES (new.id, new.content);
                END;

                CREATE TRIGGER IF NOT EXISTS content_items_ad AFTER DELETE ON content_items BEGIN
                    INSERT INTO content_fts(content_fts, rowid, content) 
                    VALUES('delete', old.id, old.content);
                END;

                CREATE TRIGGER IF NOT EXISTS content_items_au AFTER UPDATE ON content_items BEGIN
                    INSERT INTO content_fts(content_fts, rowid, content) 
                    VALUES('delete', old.id, old.content);
                    INSERT INTO content_fts(rowid, content) VALUES (new.id, new.content);
                END;
                "#,
            )?;

            // Only migrate data if V2 tables exist (upgrading from v6)
            let messages_exists: bool = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |row| row.get::<_, i32>(0),
            )? > 0;

            if messages_exists {
                // Migrate existing messages to content_items
                // Copy all messages with content_type='message'
                conn.execute(
                    r#"
                    INSERT INTO content_items (
                        content_type, conversation_id, role, message_type, previous_item_id,
                        prompt_tokens, content, importance, created_at, updated_at, 
                        last_accessed, has_embedding, project_id
                    )
                    SELECT 
                        'message' as content_type,
                        conversation_id,
                        role,
                        COALESCE(message_type, 'normal') as message_type,
                        previous_message_id as previous_item_id,
                        prompt_tokens,
                        content,
                        importance,
                        timestamp as created_at,
                        timestamp as updated_at,
                        timestamp as last_accessed,
                        has_embedding,
                        (SELECT project_id FROM conversations WHERE id = m.conversation_id) as project_id
                    FROM messages m
                    "#,
                    [],
                )?;

                // Do NOT migrate message_chunks to content_chunks.
                // The old migration had a broken JOIN that created orphan chunks
                // (item_id pointing to non-existent items).
                // Chunks are derived data - they will be regenerated during recovery
                // for any content > 1024 characters.
                //
                // Clear any chunks from previous migration attempts
                conn.execute("DELETE FROM content_chunks", [])?;

                // Populate content_fts from content_items
                conn.execute(
                    "INSERT INTO content_fts(rowid, content) SELECT id, content FROM content_items",
                    [],
                )?;

                // Clear embeddings tables (will be regenerated after migration)
                // This is safer than trying to migrate embeddings which can cause UNIQUE constraint errors
                // when multiple messages have the same content in the same conversation.
                // Embeddings are derived data and can be regenerated from content.
                conn.execute("DELETE FROM content_embeddings", [])?;
                conn.execute("DELETE FROM chunk_embeddings_v2", [])?;

                // Mark all content_items as needing embedding regeneration
                conn.execute("UPDATE content_items SET has_embedding = 0", [])?;

                // Drop old V2 tables - no longer needed after migration
                // Note: SQLite uses DROP TABLE for virtual tables too (FTS5, vec0)
                // IMPORTANT: chunk_embeddings_v2 is a V7 table (new), not V2!
                // The old V2 chunk table was "chunk_embeddings" (without _v2 suffix)
                conn.execute_batch(
                    r#"
                    DROP TABLE IF EXISTS messages;
                    DROP TABLE IF EXISTS message_chunks;
                    DROP TABLE IF EXISTS message_embeddings;
                    DROP TABLE IF EXISTS chunk_embeddings;
                    DROP TABLE IF EXISTS messages_fts;
                    "#,
                )?;
            }
        }

        // Migration v7 -> v8: Add document-specific columns to content_items
        if from_version < 8 {
            let document_columns = [
                ("filename", "TEXT"),
                (
                    "file_type",
                    "TEXT CHECK(file_type IN ('txt', 'md', 'org', 'pdf', 'epub'))",
                ),
                ("word_count", "INTEGER"),
            ];

            for (col_name, col_type) in document_columns {
                let column_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(content_items)")?;
                    let rows = stmt.query_map([], |row| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&col_name.to_string())
                };

                if !column_exists {
                    conn.execute(
                        &format!(
                            "ALTER TABLE content_items ADD COLUMN {} {}",
                            col_name, col_type
                        ),
                        [],
                    )?;
                }
            }
        }

        // Migration v8 -> v9: Add priority and tags columns to session_todos
        if from_version < 9 {
            let todo_columns = [
                ("priority", "TEXT NOT NULL DEFAULT 'medium'"),
                ("tags", "TEXT NOT NULL DEFAULT ''"),
            ];

            for (col_name, col_type) in todo_columns {
                let column_exists: bool = {
                    let mut stmt = conn.prepare("PRAGMA table_info(session_todos)")?;
                    let rows = stmt.query_map([], |row| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })?;
                    let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
                    names.contains(&col_name.to_string())
                };

                if !column_exists {
                    conn.execute(
                        &format!(
                            "ALTER TABLE session_todos ADD COLUMN {} {}",
                            col_name, col_type
                        ),
                        [],
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Load sqlite-vec extension globally (must be called before any connection)
    /// This is a one-time initialization for the process.
    #[allow(clippy::missing_transmute_annotations)]
    pub fn init_sqlite_vec() {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    }

    /// Get the default storage path (~/.local/share/ask-ai/embeddings.db)
    pub fn get_storage_path() -> PathBuf {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home)
                .join("ask-ai")
                .join("embeddings.db")
        } else if let Some(home_dir) = dirs::home_dir() {
            home_dir
                .join(".local")
                .join("share")
                .join("ask-ai")
                .join("embeddings.db")
        } else {
            PathBuf::from(".ask-ai").join("embeddings.db")
        }
    }

    /// Execute a query with a locked connection
    pub fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().map_err(|_| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                "Failed to acquire database lock",
            )))
        })?;
        f(&conn)
    }

    /// Execute a query with a mutable locked connection
    ///
    /// Future use: DDL operations, schema migrations, bulk inserts.
    #[allow(dead_code)]
    pub fn with_connection_mut<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        let mut conn = self.conn.lock().map_err(|_| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                "Failed to acquire database lock",
            )))
        })?;
        f(&mut conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database() {
        let db = Database::in_memory().expect("Failed to create in-memory database");

        // Verify we can execute queries
        let count: i64 = db
            .with_connection(|conn| conn.query_row("SELECT 1", [], |row| row.get(0)))
            .expect("Failed to execute query");

        assert_eq!(count, 1);
    }

    #[test]
    fn test_schema_creation() {
        let db = Database::in_memory().expect("Failed to create database");

        // Check tables exist
        let tables: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to list tables");

        assert!(tables.contains(&"conversations".to_string()));
        assert!(tables.contains(&"content_items".to_string()));
        assert!(tables.contains(&"content_embeddings".to_string()));
        assert!(tables.contains(&"content_fts".to_string()));
        assert!(tables.contains(&"session_todos".to_string()));
        assert!(tables.contains(&"facts".to_string()));
    }

    #[test]
    fn test_session_todos_table() {
        let db = Database::in_memory().expect("Failed to create database");

        // Verify session_todos table structure
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(session_todos)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to get table info");

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"conversation_id".to_string()));
        assert!(columns.contains(&"task_id".to_string()));
        assert!(columns.contains(&"description".to_string()));
        assert!(columns.contains(&"status".to_string()));
        assert!(columns.contains(&"priority".to_string()));
        assert!(columns.contains(&"tags".to_string()));
        assert!(columns.contains(&"created_at".to_string()));
    }

    #[test]
    fn test_conversations_metadata_columns() {
        let db = Database::in_memory().expect("Failed to create database");

        // Verify conversations table has metadata columns
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(conversations)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to get table info");

        assert!(columns.contains(&"system_prompt".to_string()));
        assert!(columns.contains(&"compacted_summary".to_string()));
        assert!(columns.contains(&"compacted_range_start".to_string()));
        assert!(columns.contains(&"compacted_range_end".to_string()));
        assert!(columns.contains(&"think".to_string()));
        assert!(columns.contains(&"tools".to_string()));
        assert!(columns.contains(&"tool_output_level".to_string()));
    }

    #[test]
    fn test_content_items_prompt_tokens_column() {
        let db = Database::in_memory().expect("Failed to create database");

        // Verify content_items table has prompt_tokens column
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(content_items)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to get table info");

        assert!(columns.contains(&"prompt_tokens".to_string()));
    }

    #[test]
    fn test_facts_table() {
        let db = Database::in_memory().expect("Failed to create database");

        // Check facts table exists
        let tables: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to list tables");

        assert!(tables.contains(&"facts".to_string()));

        // Verify facts table structure
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(facts)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to get table info");

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"scope".to_string()));
        assert!(columns.contains(&"category".to_string()));
        assert!(columns.contains(&"content".to_string()));
        assert!(columns.contains(&"importance".to_string()));
        assert!(columns.contains(&"access_count".to_string()));
        assert!(columns.contains(&"decay_score".to_string()));
        assert!(columns.contains(&"created_at".to_string()));
        assert!(columns.contains(&"last_accessed".to_string()));
        assert!(columns.contains(&"source".to_string()));
        assert!(columns.contains(&"invalidated_at".to_string()));
        assert!(columns.contains(&"project_id".to_string()));

        // Check facts_fts virtual table exists
        let vtables: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name='facts_fts'",
                )?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to list virtual tables");

        assert!(vtables.contains(&"facts_fts".to_string()));
    }

    #[test]
    fn test_content_items_table() {
        let db = Database::in_memory().expect("Failed to create database");

        // Check content_items table exists
        let tables: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to list tables");

        assert!(tables.contains(&"content_items".to_string()));
        assert!(tables.contains(&"content_chunks".to_string()));

        // Verify content_items table structure
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(content_items)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to get table info");

        // Common fields
        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"content_type".to_string()));
        assert!(columns.contains(&"content".to_string()));
        assert!(columns.contains(&"created_at".to_string()));
        assert!(columns.contains(&"updated_at".to_string()));
        assert!(columns.contains(&"has_embedding".to_string()));

        // Message fields
        assert!(columns.contains(&"conversation_id".to_string()));
        assert!(columns.contains(&"role".to_string()));
        assert!(columns.contains(&"message_type".to_string()));
        assert!(columns.contains(&"previous_item_id".to_string()));
        assert!(columns.contains(&"prompt_tokens".to_string()));

        // Note/Document fields
        assert!(columns.contains(&"scope".to_string()));
        assert!(columns.contains(&"source".to_string()));
        assert!(columns.contains(&"title".to_string()));

        // Check content_embeddings virtual table exists
        let vtables: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('content_embeddings', 'content_fts')",
                )?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to list virtual tables");

        assert!(vtables.contains(&"content_embeddings".to_string()));
        assert!(vtables.contains(&"content_fts".to_string()));
    }
}
