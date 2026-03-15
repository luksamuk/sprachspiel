//! SQL schema for semantic search storage
//!
//! Includes:
//! - conversations table (with session metadata)
//! - messages table (with prompt_tokens)
//! - message_chunks table (for long messages)
//! - message_embeddings virtual table (vec0)
//! - messages_fts virtual table (FTS5)
//! - session_todos table (for task tracking)
//! - facts table (factual memory system, v6)

/// Schema version for migrations
pub const SCHEMA_VERSION: i32 = 6;

/// Create all tables and indexes
pub const SCHEMA_SQL: &str = r#"
-- Conversations table (with session metadata)
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    title TEXT,
    model TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    -- Session metadata columns (added in v4)
    system_prompt TEXT,
    compacted_summary TEXT,
    compacted_range_start INTEGER,
    compacted_range_end INTEGER,
    think INTEGER DEFAULT 0,
    tools INTEGER DEFAULT 1,
    tool_output_level TEXT DEFAULT 'compact'
);

-- Messages table (with prompt_tokens and message_type)
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system', 'tool')),
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    importance REAL DEFAULT 0.5,
    has_embedding INTEGER DEFAULT 0,
    -- Real token count from Ollama (added in v4)
    prompt_tokens INTEGER,
    -- Message type for distinguishing intermediate vs final (added in v5)
    message_type TEXT DEFAULT 'normal',
    -- Previous message ID for navigation (added in v5)
    previous_message_id INTEGER REFERENCES messages(id),
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

-- Index for conversation lookup
CREATE INDEX IF NOT EXISTS idx_messages_conversation 
    ON messages(conversation_id);

-- Index for timestamp ordering
CREATE INDEX IF NOT EXISTS idx_messages_timestamp 
    ON messages(timestamp DESC);

-- Index for previous message lookup (added in v5)
CREATE INDEX IF NOT EXISTS idx_messages_previous 
    ON messages(previous_message_id) WHERE previous_message_id IS NOT NULL;

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

-- Session todos table (for task tracking)
CREATE TABLE IF NOT EXISTS session_todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    task_id INTEGER NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

-- Index for todo lookup by conversation
CREATE INDEX IF NOT EXISTS idx_todos_conversation 
    ON session_todos(conversation_id);

-- Facts table (factual memory system, added in v6)
CREATE TABLE IF NOT EXISTS facts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    
    -- Classification
    scope TEXT NOT NULL CHECK(scope IN ('project', 'global')),
    category TEXT NOT NULL CHECK(category IN ('preference', 'fact')),
    
    -- Content (application validates <= 500 chars)
    content TEXT NOT NULL,
    
    -- Decay parameters
    importance REAL DEFAULT 0.5 CHECK(importance BETWEEN 0 AND 1),
    access_count INTEGER DEFAULT 0,
    decay_score REAL DEFAULT 1.0,
    
    -- Timestamps (INTEGER = Unix epoch seconds)
    created_at INTEGER NOT NULL,
    last_accessed INTEGER NOT NULL,
    
    -- Source tracking
    source TEXT DEFAULT 'user' CHECK(source IN ('user', 'llm')),
    
    -- Conflict resolution (soft delete)
    invalidated_at INTEGER,
    
    -- Project association (NULL for global facts)
    project_id TEXT
);

-- Full-text search for facts (keyword matching)
CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
    content,
    content='facts',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync
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

-- Indexes for facts
CREATE INDEX IF NOT EXISTS idx_facts_scope_category ON facts(scope, category);
CREATE INDEX IF NOT EXISTS idx_facts_decay ON facts(decay_score) WHERE invalidated_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_facts_project ON facts(project_id) WHERE scope = 'project';
CREATE INDEX IF NOT EXISTS idx_facts_access ON facts(last_accessed DESC);
"#;

/// Version check query
pub const VERSION_SQL: &str = "PRAGMA user_version;";

/// Set version query (not parameterized)
pub fn set_version_sql(version: i32) -> String {
    format!("PRAGMA user_version = {};", version)
}
