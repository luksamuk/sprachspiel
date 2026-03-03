//! SQL schema for semantic search storage
//!
//! Includes:
//! - conversations table
//! - messages table
//! - message_chunks table (for long messages)
//! - message_embeddings virtual table (vec0)
//! - messages_fts virtual table (FTS5)

/// Schema version for migrations
pub const SCHEMA_VERSION: i32 = 2;

/// Create all tables and indexes
pub const SCHEMA_SQL: &str = r#"
-- Conversations table
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    title TEXT,
    model TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system', 'tool')),
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    importance REAL DEFAULT 0.5,
    has_embedding INTEGER DEFAULT 0,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

-- Index for conversation lookup
CREATE INDEX IF NOT EXISTS idx_messages_conversation 
    ON messages(conversation_id);

-- Index for timestamp ordering
CREATE INDEX IF NOT EXISTS idx_messages_timestamp 
    ON messages(timestamp DESC);

-- Message chunks for long messages (>1024 chars)
CREATE TABLE IF NOT EXISTS message_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);

-- Index for chunk lookup by message
CREATE INDEX IF NOT EXISTS idx_chunks_message 
    ON message_chunks(message_id);

-- Index for chunk ordering
CREATE INDEX IF NOT EXISTS idx_chunks_order 
    ON message_chunks(message_id, chunk_index);

-- Vector embeddings for short messages (256-dim Matryoshka)
CREATE VIRTUAL TABLE IF NOT EXISTS message_embeddings USING vec0(
    message_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +conversation_id TEXT,
    +timestamp INTEGER
);

-- Vector embeddings for message chunks
CREATE VIRTUAL TABLE IF NOT EXISTS chunk_embeddings USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +conversation_id TEXT,
    +timestamp INTEGER
);

-- Full-text search for keyword matching
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content='messages',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) 
    VALUES('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) 
    VALUES('delete', old.id, old.content);
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
"#;

/// Version check query
pub const VERSION_SQL: &str = "PRAGMA user_version;";

/// Set version query (not parameterized)
pub fn set_version_sql(version: i32) -> String {
    format!("PRAGMA user_version = {};", version)
}
