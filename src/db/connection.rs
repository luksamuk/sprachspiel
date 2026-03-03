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
            // Apply schema
            conn.execute_batch(SCHEMA_SQL)?;
            // Set version (not parameterized)
            conn.execute_batch(&set_version_sql(SCHEMA_VERSION))?;
        }

        Ok(())
    }

    /// Load sqlite-vec extension globally (must be called before any connection)
    /// This is a one-time initialization for the process.
    pub fn init_sqlite_vec() {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as usize,
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
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
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
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
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
    }
}
