//! SQL schema for semantic search storage
//!
//! Includes:
//! - conversations table (with session metadata)
//! - content_items table (unified storage for messages, notes, documents)
//! - content_chunks, content_embeddings, content_fts
//! - session_todos table (for task tracking)
//! - facts table (factual memory system)
//! - fact_embeddings (vec0 vector index for facts)

/// Schema version for migrations
pub const SCHEMA_VERSION: i32 = 11;

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

-- Session todos table (for task tracking)
CREATE TABLE IF NOT EXISTS session_todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    task_id INTEGER NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT NOT NULL DEFAULT 'medium',
    tags TEXT NOT NULL DEFAULT '',
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
    project_id TEXT,

    -- Whether this fact has a vector embedding in fact_embeddings
    has_embedding INTEGER DEFAULT 0
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
CREATE INDEX IF NOT EXISTS idx_facts_embedding ON facts(has_embedding) WHERE has_embedding = 0 AND invalidated_at IS NULL;

-- Vector embeddings for facts (256-dim Matryoshka, v11)
CREATE VIRTUAL TABLE IF NOT EXISTS fact_embeddings USING vec0(
    fact_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +scope TEXT,
    +category TEXT,
    +project_id TEXT
);

-- Content items table (unified storage for messages, notes, documents, v8)
-- Stores messages, notes, and documents in a unified schema
CREATE TABLE IF NOT EXISTS content_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    
    -- Content type discriminator
    content_type TEXT NOT NULL CHECK(content_type IN ('message', 'note', 'document')),
    
    -- Message fields (nullable, only for content_type='message')
    conversation_id TEXT,
    role TEXT CHECK(role IN ('user', 'assistant', 'system', 'tool')),
    message_type TEXT DEFAULT 'normal',
    previous_item_id INTEGER REFERENCES content_items(id),
    prompt_tokens INTEGER,
    
    -- Note/Document fields (nullable, only for content_type in ('note', 'document'))
    scope TEXT CHECK(scope IN ('project', 'global')),
    source TEXT CHECK(source IN ('user', 'llm')),
    title TEXT,
    
    -- Document-specific fields (nullable, only for content_type='document', v8)
    filename TEXT,
    file_type TEXT CHECK(file_type IN ('txt', 'md', 'org', 'pdf', 'epub')),
    word_count INTEGER,
    
    -- Common fields (all content types)
    content TEXT NOT NULL,
    importance REAL DEFAULT 0.5,
    access_count INTEGER DEFAULT 0,
    decay_score REAL DEFAULT 1.0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_accessed INTEGER NOT NULL,
    has_embedding INTEGER DEFAULT 0,
    pruned INTEGER NOT NULL DEFAULT 0,
    
    -- Project association (NULL for global scope)
    project_id TEXT
);

-- Indexes for content_items
CREATE INDEX IF NOT EXISTS idx_content_items_type ON content_items(content_type);
CREATE INDEX IF NOT EXISTS idx_content_items_conversation ON content_items(conversation_id) WHERE conversation_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_content_items_project ON content_items(project_id) WHERE project_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_content_items_scope ON content_items(scope) WHERE scope IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_content_items_timestamp ON content_items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_content_items_previous ON content_items(previous_item_id) WHERE previous_item_id IS NOT NULL;

-- Content chunks for long content (v7)
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

-- Indexes for content_chunks
CREATE INDEX IF NOT EXISTS idx_content_chunks_item ON content_chunks(item_id);
CREATE INDEX IF NOT EXISTS idx_content_chunks_order ON content_chunks(item_id, chunk_index);

-- Vector embeddings for content items (256-dim Matryoshka, v7)
CREATE VIRTUAL TABLE IF NOT EXISTS content_embeddings USING vec0(
    item_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT,
    +timestamp INTEGER
);

-- Vector embeddings for content chunks (v7)
CREATE VIRTUAL TABLE IF NOT EXISTS chunk_embeddings_v2 USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT,
    +timestamp INTEGER
);

-- Full-text search for content items (v7)
CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(
    content,
    content='content_items',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- Triggers to keep content_fts in sync
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

-- Feedback signals table (v10)
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

-- Indexes for feedback_signals
CREATE INDEX IF NOT EXISTS idx_feedback_signals_item_id ON feedback_signals(item_id);
CREATE INDEX IF NOT EXISTS idx_feedback_signals_session_id ON feedback_signals(session_id);
CREATE INDEX IF NOT EXISTS idx_feedback_signals_created_at ON feedback_signals(created_at);
"#;

/// Version check query
pub const VERSION_SQL: &str = "PRAGMA user_version;";

/// Set version query (not parameterized)
pub fn set_version_sql(version: i32) -> String {
    format!("PRAGMA user_version = {};", version)
}
