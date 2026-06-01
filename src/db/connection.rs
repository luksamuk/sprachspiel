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
    /// Create a new database at the default storage location.
    ///
    /// Also handles migration from the legacy `embeddings.db` filename.
    pub fn new() -> Result<Self> {
        let path = Self::get_storage_path();
        Self::with_path(&path)
    }

    /// Create a database at an explicit path.
    ///
    /// Creates parent directories if they don't exist.
    /// Does NOT perform legacy filename migration (caller should use
    /// `get_storage_path()` for default paths where migration is desired).
    pub fn with_path(path: &PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Self::open(path)
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

    /// Check if a column exists in a table.
    fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        let names: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(names.contains(&column.to_string()))
    }

    /// Check if a table exists in the database.
    fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get::<_, i32>(0),
        )? > 0)
    }

    /// Add a column to a table if it doesn't already exist.
    fn add_column_if_missing(
        conn: &Connection,
        table: &str,
        column: &str,
        col_type: &str,
    ) -> Result<bool> {
        if !Self::column_exists(conn, table, column)? {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}"),
                [],
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Add multiple columns to a table if they don't exist.
    fn add_columns_if_missing(
        conn: &Connection,
        table: &str,
        columns: &[(&str, &str)],
    ) -> Result<()> {
        for (col_name, col_type) in columns {
            Self::add_column_if_missing(conn, table, col_name, col_type)?;
        }
        Ok(())
    }

    /// Migration v2 -> v3: Add has_embedding to message_chunks
    fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
        if Self::table_exists(conn, "message_chunks")? {
            Self::add_column_if_missing(
                conn,
                "message_chunks",
                "has_embedding",
                "INTEGER DEFAULT 0",
            )?;

            // Create index for missing embeddings
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_chunks_missing_embedding 
                 ON message_chunks(has_embedding) WHERE has_embedding = 0",
                [],
            )?;
        }
        Ok(())
    }

    /// Migration v3 -> v4: Add session metadata and todos
    fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
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
        Self::add_columns_if_missing(conn, "conversations", &conversations_columns)?;

        // Add prompt_tokens column to messages table (only if table exists - V2 legacy)
        if Self::table_exists(conn, "messages")? {
            Self::add_column_if_missing(conn, "messages", "prompt_tokens", "INTEGER")?;
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
        Ok(())
    }

    /// Migration v4 -> v5: Add message_type and previous_message_id
    fn migrate_v4_to_v5(conn: &Connection) -> Result<()> {
        // Only apply migrations if messages table exists (V2 legacy)
        if Self::table_exists(conn, "messages")? {
            Self::add_column_if_missing(conn, "messages", "message_type", "TEXT DEFAULT 'normal'")?;
            Self::add_column_if_missing(
                conn,
                "messages",
                "previous_message_id",
                "INTEGER REFERENCES messages(id)",
            )?;

            // Create index for previous_message_id lookups
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_messages_previous 
                 ON messages(previous_message_id) WHERE previous_message_id IS NOT NULL",
                [],
            )?;
        }
        Ok(())
    }

    /// Migration v5 -> v6: Add facts table (factual memory system)
    fn migrate_v5_to_v6(conn: &Connection) -> Result<()> {
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
        Ok(())
    }

    /// Migration v6 -> v7: Add content_items unified table
    fn migrate_v6_to_v7(conn: &Connection) -> Result<()> {
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
        if Self::table_exists(conn, "messages")? {
            // Migrate existing messages to content_items with content_type='message'
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
            // The old migration had a broken JOIN that created orphan chunks.
            // Chunks are derived data - they will be regenerated during recovery.
            // Clear any chunks from previous migration attempts
            conn.execute("DELETE FROM content_chunks", [])?;

            // Populate content_fts from content_items
            conn.execute(
                "INSERT INTO content_fts(rowid, content) SELECT id, content FROM content_items",
                [],
            )?;

            // Clear embeddings tables (will be regenerated after migration)
            // Safer than trying to migrate embeddings which can cause UNIQUE constraint errors
            conn.execute("DELETE FROM content_embeddings", [])?;
            conn.execute("DELETE FROM chunk_embeddings_v2", [])?;

            // Mark all content_items as needing embedding regeneration
            conn.execute("UPDATE content_items SET has_embedding = 0", [])?;

            // Drop old V2 tables - no longer needed after migration
            // IMPORTANT: chunk_embeddings_v2 is a V7 table (new), not V2!
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
        Ok(())
    }

    /// Migration v7 -> v8: Add document-specific columns to content_items
    fn migrate_v7_to_v8(conn: &Connection) -> Result<()> {
        let document_columns: [(&str, &str); 3] = [
            ("filename", "TEXT"),
            (
                "file_type",
                "TEXT CHECK(file_type IN ('txt', 'md', 'org', 'pdf', 'epub'))",
            ),
            ("word_count", "INTEGER"),
        ];
        Self::add_columns_if_missing(conn, "content_items", &document_columns)?;
        Ok(())
    }

    /// Migration v8 -> v9: Add priority and tags columns to session_todos
    fn migrate_v8_to_v9(conn: &Connection) -> Result<()> {
        let todo_columns: [(&str, &str); 2] = [
            ("priority", "TEXT NOT NULL DEFAULT 'medium'"),
            ("tags", "TEXT NOT NULL DEFAULT ''"),
        ];
        Self::add_columns_if_missing(conn, "session_todos", &todo_columns)?;
        Ok(())
    }

    /// Migration v9 -> v10: Add feedback_signals table and pruned column
    fn migrate_v9_to_v10(conn: &Connection) -> Result<()> {
        // Create feedback_signals table if not exists
        if !Self::table_exists(conn, "feedback_signals")? {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS feedback_signals (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    item_id INTEGER NOT NULL,
                    session_id TEXT,
                    signal_type TEXT NOT NULL CHECK(signal_type IN ('good', 'bad', 'correction')),
                    base_value REAL NOT NULL,
                    correction_text TEXT,
                    source TEXT NOT NULL DEFAULT 'user' CHECK(source IN ('user', 'llm')),
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY (item_id) REFERENCES content_items(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_feedback_signals_item_id ON feedback_signals(item_id);
                CREATE INDEX IF NOT EXISTS idx_feedback_signals_session_id ON feedback_signals(session_id);
                CREATE INDEX IF NOT EXISTS idx_feedback_signals_created_at ON feedback_signals(created_at);
                "#,
            )?;
        }

        // Add pruned column to content_items if not exists
        Self::add_column_if_missing(
            conn,
            "content_items",
            "pruned",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Ok(())
    }

    /// Migration v10 -> v11: Add fact_embeddings vec0 table and has_embedding column
    fn migrate_v10_to_v11(conn: &Connection) -> Result<()> {
        // Create fact_embeddings table if not exists (idempotent)
        if !Self::table_exists(conn, "fact_embeddings")? {
            conn.execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS fact_embeddings USING vec0(
                    fact_id INTEGER PRIMARY KEY,
                    embedding FLOAT[256],
                    +scope TEXT,
                    +category TEXT,
                    +project_id TEXT
                );",
            )?;
        }

        // Add has_embedding column to facts if not exists
        Self::add_column_if_missing(conn, "facts", "has_embedding", "INTEGER DEFAULT 0")?;

        // Create index for finding facts without embeddings
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_facts_embedding ON facts(has_embedding) WHERE has_embedding = 0 AND invalidated_at IS NULL",
            [],
        )?;
        Ok(())
    }

    /// Migration v11 -> v12: Add distance_metric=cosine to vec0 tables.
    ///
    /// sqlite-vec defaults to L2 (Euclidean) distance. All 3 vec0 tables were
    /// created without `distance_metric=cosine`, causing Bug #3 (L2 vs cosine
    /// metric mismatch). sqlite-vec does not support ALTER TABLE on virtual
    /// tables, so we must DROP and re-CREATE. This loses all embeddings, but
    /// startup recovery regenerates them (has_embedding flags are reset below).
    fn migrate_v11_to_v12(conn: &Connection) -> Result<()> {
        conn.execute_batch("DROP TABLE IF EXISTS fact_embeddings;")?;
        conn.execute_batch("DROP TABLE IF EXISTS content_embeddings;")?;
        conn.execute_batch("DROP TABLE IF EXISTS chunk_embeddings_v2;")?;

        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS fact_embeddings USING vec0(
                fact_id INTEGER PRIMARY KEY,
                embedding FLOAT[256] distance_metric=cosine,
                +scope TEXT,
                +category TEXT,
                +project_id TEXT
            );",
        )?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS content_embeddings USING vec0(
                item_id INTEGER PRIMARY KEY,
                embedding FLOAT[256] distance_metric=cosine,
                +content_type TEXT,
                +conversation_id TEXT,
                +project_id TEXT,
                +timestamp INTEGER
            );",
        )?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunk_embeddings_v2 USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding FLOAT[256] distance_metric=cosine,
                +content_type TEXT,
                +conversation_id TEXT,
                +project_id TEXT,
                +timestamp INTEGER
            );",
        )?;

        // Reset embedding flags so startup recovery regenerates all embeddings
        conn.execute(
            "UPDATE facts SET has_embedding = 0 WHERE invalidated_at IS NULL",
            [],
        )?;
        conn.execute("UPDATE content_items SET has_embedding = 0", [])?;
        conn.execute("UPDATE content_chunks SET has_embedding = 0", [])?;
        Ok(())
    }

    /// Migration v12 -> v13: Add norm_correction auxiliary column to vec0 tables.
    ///
    /// Matryoshka truncation (768→256 dims) discards dimensions that contribute
    /// to the L2 norm. When sqlite-vec computes cosine distance on the truncated
    /// vector, the result is biased because the stored vector is not truly unit-length.
    /// The `norm_correction` column stores `1/(|truncated_vec|^2)` so that at query
    /// time, `true_cosine ≈ measured_cosine * sqrt(query_nc * result_nc)`.
    ///
    /// `norm_correction` is stored as FLOAT (f64 in SQLite, cast from f32 on insert).
    /// sqlite-vec supports FLOAT as an auxiliary column type (alongside INTEGER,
    /// TEXT, and BLOB). Note: REAL (the standard SQLite float type name) does NOT
    /// work — the sqlite-vec parser requires the exact type name FLOAT.
    ///
    /// Since sqlite-vec does not support ALTER TABLE on virtual tables, we must
    /// DROP and re-CREATE all three vec0 tables. This loses all embeddings, but
    /// startup recovery regenerates them (has_embedding flags are reset below).
    fn migrate_v12_to_v13(conn: &Connection) -> Result<()> {
        conn.execute_batch("DROP TABLE IF EXISTS fact_embeddings;")?;
        conn.execute_batch("DROP TABLE IF EXISTS content_embeddings;")?;
        conn.execute_batch("DROP TABLE IF EXISTS chunk_embeddings_v2;")?;

        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS fact_embeddings USING vec0(
                fact_id INTEGER PRIMARY KEY,
                embedding FLOAT[256] distance_metric=cosine,
                +scope TEXT,
                +category TEXT,
                +project_id TEXT,
                +norm_correction FLOAT
            );",
        )?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS content_embeddings USING vec0(
                item_id INTEGER PRIMARY KEY,
                embedding FLOAT[256] distance_metric=cosine,
                +content_type TEXT,
                +conversation_id TEXT,
                +project_id TEXT,
                +timestamp INTEGER,
                +norm_correction FLOAT
            );",
        )?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunk_embeddings_v2 USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding FLOAT[256] distance_metric=cosine,
                +content_type TEXT,
                +conversation_id TEXT,
                +project_id TEXT,
                +timestamp INTEGER,
                +norm_correction FLOAT
            );",
        )?;

        // Reset embedding flags so startup recovery regenerates all embeddings
        // with norm_correction values
        conn.execute(
            "UPDATE facts SET has_embedding = 0 WHERE invalidated_at IS NULL",
            [],
        )?;
        conn.execute("UPDATE content_items SET has_embedding = 0", [])?;
        conn.execute("UPDATE content_chunks SET has_embedding = 0", [])?;
        Ok(())
    }

    /// Migration v13 -> v14: Add thinking_content column to content_items.
    ///
    /// Preserves thinking traces from LLM responses in a dedicated column.
    /// Previously, thinking was either stripped before storage (normal messages)
    /// or concatenated inline in the content field (pre-tool messages).
    /// The `thinking_content` column stores the thinking separately, keeping
    /// the `content` field clean for display and retrieval.
    ///
    /// Reference: Arabzadeh et al. 2026, arXiv:2605.03344
    fn migrate_v13_to_v14(conn: &Connection) -> Result<()> {
        Self::add_column_if_missing(conn, "content_items", "thinking_content", "TEXT")?;
        log::info!("Migration v13→v14: Added thinking_content column to content_items");
        Ok(())
    }

    /// Apply incremental schema migrations (dispatcher)
    fn apply_migrations(conn: &Connection, from_version: i32) -> Result<()> {
        if from_version < 3 {
            Self::migrate_v2_to_v3(conn)?;
        }
        if from_version < 4 {
            Self::migrate_v3_to_v4(conn)?;
        }
        if from_version < 5 {
            Self::migrate_v4_to_v5(conn)?;
        }
        if from_version < 6 {
            Self::migrate_v5_to_v6(conn)?;
        }
        if from_version < 7 {
            Self::migrate_v6_to_v7(conn)?;
        }
        if from_version < 8 {
            Self::migrate_v7_to_v8(conn)?;
        }
        if from_version < 9 {
            Self::migrate_v8_to_v9(conn)?;
        }
        if from_version < 10 {
            Self::migrate_v9_to_v10(conn)?;
        }
        if from_version < 11 {
            Self::migrate_v10_to_v11(conn)?;
        }
        if from_version < 12 {
            Self::migrate_v11_to_v12(conn)?;
        }
        if from_version < 13 {
            Self::migrate_v12_to_v13(conn)?;
        }
        if from_version < 14 {
            Self::migrate_v13_to_v14(conn)?;
        }
        Ok(())
    }

    /// Load sqlite-vec extension globally (must be called before any connection)
    /// This is a one-time initialization for the process.
    #[expect(clippy::missing_transmute_annotations)]
    pub fn init_sqlite_vec() {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    }

    /// Get the default storage path (~/.local/share/sprachspiel/sprachspiel.db)
    ///
    /// Also handles migration from legacy database filenames:
    /// - `embeddings.db` → `sprachspiel.db` (v0.27 and earlier)
    /// - `ask-ai.db` → `sprachspiel.db` (v0.42 and earlier)
    ///
    /// Migration only happens if the old file exists and the new one doesn't.
    pub fn get_storage_path() -> PathBuf {
        let path = Self::resolve_storage_path();
        Self::migrate_legacy_db(&path);
        path
    }

    /// Resolve the storage path without performing migration.
    fn resolve_storage_path() -> PathBuf {
        use crate::consts::app;

        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home)
                .join(app::APP_DATA_DIR)
                .join(app::DB_FILENAME)
        } else if let Some(home_dir) = dirs::home_dir() {
            home_dir
                .join(".local")
                .join("share")
                .join(app::APP_DATA_DIR)
                .join(app::DB_FILENAME)
        } else {
            PathBuf::from(app::APP_PROJECT_DIR).join(app::DB_FILENAME)
        }
    }

    /// Rename legacy database files if they exist and the new one doesn't.
    ///
    /// Migration chain: `embeddings.db` → `sprachspiel.db` and `ask-ai.db` → `sprachspiel.db`
    fn migrate_legacy_db(new_path: &PathBuf) {
        use crate::consts::app;

        let new_filename = new_path.file_name().unwrap_or_default().to_string_lossy();

        // Try each legacy filename in reverse chronological order
        let legacy_names = [app::DB_FILENAME_LEGACY_V2, app::DB_FILENAME_LEGACY_V1];

        for old_filename in legacy_names {
            if new_filename == old_filename {
                continue; // Already using this name, skip
            }

            let old_path = new_path.with_file_name(old_filename);

            if old_path.exists() && !new_path.exists() {
                if let Err(e) = std::fs::rename(&old_path, new_path) {
                    log::warn!(
                        "Failed to migrate legacy database {} → {}: {}",
                        old_path.display(),
                        new_path.display(),
                        e
                    );
                } else {
                    log::warn!(
                        "Migrated legacy database: {} → {}",
                        old_path.display(),
                        new_path.display()
                    );
                }
                return; // Only migrate one file
            }
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

    /// Normalize inline thinking tags in existing content items.
    ///
    /// Prior to v14, pre-tool messages stored `<thinking>` content inline in the
    /// `content` field. After v14, thinking is stored in a separate `thinking_content`
    /// column. This method migrates existing rows by:
    /// 1. Selecting rows where `content LIKE '%<thinking>%'`
    /// 2. Calling the provided `split_fn` to separate thinking from content
    /// 3. Updating the row: `content` = clean text, `thinking_content` = thinking text
    ///
    /// The `split_fn` parameter avoids a circular dependency: the `db` module does
    /// not import `chat::thinking::process_thinking`. The caller (e.g., `repl.rs`)
    /// passes `process_thinking` as the closure.
    ///
    /// Returns the number of rows normalized.
    ///
    /// All writes are wrapped in an explicit transaction so the batch is
    /// atomic: either every row is normalized or none are. If the process
    /// is interrupted (Ctrl+C, panic, kill), SQLite rolls back automatically.
    pub fn normalize_inline_thinking<F>(&self, split_fn: F) -> Result<u64>
    where
        F: Fn(&str) -> (Option<String>, String),
    {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, content FROM content_items WHERE content LIKE '%<thinking>%'",
            )?;
            let rows: Vec<(i64, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;

            let count = rows.len();
            if count == 0 {
                log::debug!("No inline thinking rows to normalize");
                return Ok(0u64);
            }

            // Explicit transaction: all or nothing. If any statement fails or
            // the process is interrupted, SQLite auto-rollbacks the batch.
            conn.execute_batch("BEGIN")?;

            for (id, content) in &rows {
                let (thinking, clean_content) = split_fn(content);

                // Rewrite content (remove inline thinking) and store thinking separately.
                // Also reset has_embedding=0 and delete stale embeddings/chunks because
                // the content changed — old embeddings were computed from text containing
                // <thinking> tags and are now semantically stale. The background embedding
                // recovery pipeline (repl_tui.rs) will regenerate them from the cleaned text.
                conn.execute(
                    "UPDATE content_items SET content = ?, thinking_content = ?, has_embedding = 0 WHERE id = ?",
                    rusqlite::params![clean_content, thinking, id],
                )?;
                conn.execute(
                    "DELETE FROM content_embeddings WHERE item_id = ?",
                    rusqlite::params![id],
                )?;
                conn.execute(
                    "DELETE FROM content_chunks WHERE item_id = ?",
                    rusqlite::params![id],
                )?;
            }

            conn.execute_batch("COMMIT")?;

            log::info!(
                "Normalized {} content items with inline thinking tags (embeddings will be regenerated)",
                count
            );
            Ok(count as u64)
        })
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
        assert!(columns.contains(&"has_embedding".to_string()));

        // Check facts_fts virtual table exists
        let vtables: Vec<String> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('facts_fts', 'fact_embeddings')",
                )?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()
            })
            .expect("Failed to list virtual tables");

        assert!(vtables.contains(&"facts_fts".to_string()));
        assert!(vtables.contains(&"fact_embeddings".to_string()));
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

        // Thinking trace field (added in v14)
        assert!(columns.contains(&"thinking_content".to_string()));

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

    #[test]
    fn test_thinking_content_column_in_content_items() {
        let db = Database::in_memory().expect("Failed to create database");

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

        assert!(
            columns.contains(&"thinking_content".to_string()),
            "thinking_content column must exist in content_items"
        );
    }

    #[test]
    fn test_normalize_inline_thinking_basic() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a content item with inline thinking tags
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, role, content, conversation_id, message_type, created_at, updated_at, last_accessed)
                 VALUES ('message', 'assistant', '<thinking>Let me reason</thinking>The answer is 42', 'conv1', 'pre_tool', ?1, ?1, ?1)",
                rusqlite::params![now],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test row");

        // Run normalize_inline_thinking with a simple split function
        let count = db
            .normalize_inline_thinking(|content| {
                if content.contains("<thinking>") {
                    let start = content.find("<thinking>").unwrap();
                    let end = content.find("</thinking>").unwrap() + "</thinking>".len();
                    let thinking = content[start..end].to_string();
                    let clean = content[..start].trim().to_string() + &content[end..];
                    (Some(thinking), clean.trim().to_string())
                } else {
                    (None, content.to_string())
                }
            })
            .expect("normalize_inline_thinking failed");

        assert_eq!(count, 1, "Should normalize 1 row");

        // Verify the row was updated
        let (content, thinking): (String, Option<String>) = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT content, thinking_content FROM content_items WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
            })
            .expect("Failed to query updated row");

        assert_eq!(content, "The answer is 42");
        assert_eq!(
            thinking,
            Some("<thinking>Let me reason</thinking>".to_string())
        );

        // Fix A: verify has_embedding was reset to 0
        let has_embedding: i32 = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT has_embedding FROM content_items WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
            })
            .expect("Failed to query has_embedding");
        assert_eq!(
            has_embedding, 0,
            "has_embedding should be reset to 0 after normalization"
        );
    }

    #[test]
    fn test_normalize_inline_thinking_no_thinking_rows() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a content item without thinking tags
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, role, content, conversation_id, message_type, created_at, updated_at, last_accessed)
                 VALUES ('message', 'assistant', 'Just a regular message', 'conv1', 'regular', ?1, ?1, ?1)",
                rusqlite::params![now],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test row");

        let count = db
            .normalize_inline_thinking(|content| {
                if content.contains("<thinking>") {
                    (Some("thinking".to_string()), "clean".to_string())
                } else {
                    (None, content.to_string())
                }
            })
            .expect("normalize_inline_thinking failed");

        assert_eq!(
            count, 0,
            "Should normalize 0 rows when no thinking tags present"
        );
    }

    #[test]
    fn test_thinking_content_roundtrip() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a content item with thinking_content
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, role, content, thinking_content, conversation_id, message_type, created_at, updated_at, last_accessed)
                 VALUES ('message', 'assistant', 'The answer is 42', 'My reasoning process', 'conv1', 'regular', ?1, ?1, ?1)",
                rusqlite::params![now],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test row");

        // Read it back
        let (content, thinking): (String, Option<String>) = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT content, thinking_content FROM content_items WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
            })
            .expect("Failed to query row");

        assert_eq!(content, "The answer is 42");
        assert_eq!(thinking, Some("My reasoning process".to_string()));
    }

    #[test]
    fn test_thinking_content_null_by_default() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a content item without thinking_content (should default to NULL)
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, role, content, conversation_id, message_type, created_at, updated_at, last_accessed)
                 VALUES ('message', 'assistant', 'Hello world', 'conv1', 'regular', ?1, ?1, ?1)",
                rusqlite::params![now],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test row");

        let thinking: Option<String> = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT thinking_content FROM content_items WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
            })
            .expect("Failed to query row");

        assert_eq!(thinking, None, "thinking_content should be NULL by default");
    }

    #[test]
    fn test_normalize_inline_thinking_resets_embedding_flag() {
        let db = Database::in_memory().expect("Failed to create database");

        // Insert a content item with inline thinking AND has_embedding = 1
        // (simulating an item that had an embedding computed from the old
        // content that included <thinking> tags)
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, role, content, conversation_id, message_type, has_embedding, created_at, updated_at, last_accessed)
                 VALUES ('message', 'assistant', '<thinking>My reasoning</thinking>The answer', 'conv1', 'pre_tool', 1, ?1, ?1, ?1)",
                rusqlite::params![now],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test row");

        let count = db
            .normalize_inline_thinking(|content| {
                if content.contains("<thinking>") {
                    let start = content.find("<thinking>").unwrap();
                    let end = content.find("</thinking>").unwrap() + "</thinking>".len();
                    let thinking = content[start..end].to_string();
                    let clean = content[..start].trim().to_string() + &content[end..];
                    (Some(thinking), clean.trim().to_string())
                } else {
                    (None, content.to_string())
                }
            })
            .expect("normalize_inline_thinking failed");

        assert_eq!(count, 1);

        // has_embedding must be 0 (reset for re-indexing by background recovery)
        let has_embedding: i32 = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT has_embedding FROM content_items WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
            })
            .expect("Failed to query has_embedding");
        assert_eq!(
            has_embedding, 0,
            "has_embedding must be reset to 0 after content rewrite"
        );
    }
}
