//! Database connection and initialization
//!
//! Handles SQLite connection with sqlite-vec extension loaded.

use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::schema::{set_version_sql, SCHEMA_SQL, SCHEMA_VERSION, VERSION_SQL};

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

            // Add prompt_tokens column to messages table
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
    fn get_storage_path() -> PathBuf {
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
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"message_embeddings".to_string()));
        assert!(tables.contains(&"messages_fts".to_string()));
        assert!(tables.contains(&"session_todos".to_string()));
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
    fn test_messages_prompt_tokens_column() {
        let db = Database::in_memory().expect("Failed to create database");

        // Verify messages table has prompt_tokens column
        let columns: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to get table info");

        assert!(columns.contains(&"prompt_tokens".to_string()));
    }
}
