# Implementation History

This document consolidates key decisions from completed implementation phases.

---

## Context Management (v0.21.0 - v0.22.5)

### Context Order (Research-Based)

Based on "Lost in the Middle" phenomenon:

```
1. SYSTEM PROMPT (~500-2000 tokens) - Position: BEGINNING
2. RETRIEVED MESSAGES (~1000-5000 tokens) - Position: AFTER SYSTEM
3. COMPACTED SUMMARY (~500-1000 tokens) - Position: BEFORE RECENT
4. RECENT MESSAGES (~2000-5000 tokens) - Position: BEFORE QUERY
5. CURRENT USER QUERY - Position: VERY END
```

**Key Rule:** Never place important information in the middle of context.

### Middle Compaction Strategy

```
Before: [First 5] [Middle 20] [Last 5] = 30 messages
After:  [First 5] [Summary]   [Last 5] = 10 messages + summary

Context sent: System → Retrieved → First N → Summary → Recent → Query
```

**Critical:** Messages are NEVER deleted from SQLite. Only LLM context changes.

### `/clear` vs `/forget` (v0.22.4)

| Command | Messages | Summary | SQLite | RAG Enabled |
|---------|----------|---------|--------|-------------|
| `/clear` | Clears | Preserves | Intact | Yes (forced) |
| `/forget` | Clears | Clears | Removes | No |

### Forced Retrieval After `/clear` (v0.22.5)

When session is empty but database has messages:
- `should_force_retrieve()` returns true
- `MIN_MESSAGES_FOR_RETRIEVAL` lowered from 20 to 5
- `MIN_RETRIEVAL_FORCE_COUNT` = 2 for post-clear scenarios

---

## Remember Tool (v0.23.0)

### Architecture

```rust
tokio::task_local! {
    pub static REMEMBER_DB: Arc<Database>;
    pub static REMEMBER_EMBEDDING: Arc<EmbeddingClient>;
}
```

### Tool Interface

```rust
remember(id="42")                    // Get specific message by ID
remember(query="Wittgenstein")       // Search history by topic
remember(query="phi", limit="10")    // Search with limit
```

### Retrieved Context Format

```xml
<retrieved_context>
MESSAGES FROM YOUR PAST CONVERSATION with this user.
Each message has an ID. Use remember(id="N") for full content.
Use remember(query="topic") to search for past discussions.

<message id="42">
<role>user</role>
<content>What is the capital of France?</content>
</message>
</retrieved_context>
```

### Task-Local vs Thread-Local

**Decision:** Use `tokio::task_local!` instead of `thread_local!`.

**Reason:** Async tasks can move between threads after `await`. Thread-local storage is unsafe in async context.

---

## Conversation-Aware Retrieval (v0.24.0)

### Problem Solved

User questions have high semantic similarity but contain no information. Assistant responses have information but lower similarity due to semantic dispersion.

### Solution: Post-Retrieval Enrichment

When a user message is retrieved, automatically include the next assistant message:

```rust
pub struct SearchResult {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    // NEW: Next message for user questions
    pub next_message: Option<Box<SearchResult>>,
}
```

**Implementation:** `enrich_with_context()` in `src/db/operations.rs`

---

## Project-Aware Query Mode (v0.25.0)

### Design

| Feature | Chat | Query (Before) | Query (After) |
|---------|------|----------------|----------------|
| AGENTS.md | Yes | Yes | Yes |
| Retrieval from DB | Yes | No | Yes (project-wide) |
| Persists messages | Yes | No | No |
| Remember tool | Yes | No | Yes |

### Implementation

```rust
// In src/query.rs
let (db, embedding_client) = if cli_code {
    (None, None)
} else {
    match Database::new() {
        Ok(db) => {
            let embedding = Arc::new(EmbeddingClient::new(ollama.clone()));
            (Some(Arc::new(db)), Some(embedding))
        }
        Err(_) => (None, None)
    }
};
```

---

## Key Constants

```rust
// Retrieval
pub const MIN_MESSAGES_FOR_RETRIEVAL: usize = 5;  // Lowered from 20
pub const MIN_RETRIEVAL_FORCE_COUNT: usize = 2;  // For post-clear
pub const DEFAULT_RETRIEVAL_LIMIT: usize = 5;    // RRF results
pub const MAX_REMEMBER_LIMIT: usize = 10;         // Tool limit

// Context Overflow (percentage-based thresholds)
pub const PRE_TOOL_THRESHOLD: f32 = 0.75;     // 75% — warning only
pub const COMPACTION_THRESHOLD: f32 = 0.88;   // 88% — auto-compact
pub const INTER_TOOL_THRESHOLD: f32 = 0.94;   // 94% — warning during tool execution
pub const EMERGENCY_THRESHOLD: f32 = 0.97;    // 97% — truncate tool results
pub const DEFAULT_KEEP_FIRST: usize = 5;     // Compaction
pub const DEFAULT_KEEP_LAST: usize = 5;       // Compaction

// Token Budgets
pub const SYSTEM_PROMPT_BUDGET: usize = 2000;
pub const RETRIEVED_MESSAGES_BUDGET: usize = 5000;
pub const COMPACTED_SUMMARY_BUDGET: usize = 1000;
pub const RECENT_MESSAGES_BUDGET: usize = 5000;

// Fact Memory
pub const SEMANTIC_SEARCH_THRESHOLD: f32 = 0.70;  // Insert-time semantic search
pub const CONFLICT_THRESHOLD: f32 = 0.75;          // FTS5 BM25 conflict detection
pub const SEMANTIC_DEDUP_THRESHOLD: f32 = 0.90;    // Startup O(n²) verification
pub const FACT_MAX_LENGTH: usize = 500;             // Max chars per fact
pub const FACT_PROMPT_BUDGET: usize = 2200;         // Max chars in prompt
pub const PREFERENCE_HALF_LIFE_DAYS: f32 = 180.0;   // Ebbinghaus decay half-life
pub const FACT_HALF_LIFE_DAYS: f32 = 30.0;           // Ebbinghaus decay half-life
```

---

## Database Schema v12

> Schema has evolved from v3 through v12. Key migrations: v4 (content_items), v7 (chunk_embeddings_v2 vec0), v10 (facts table), v11 (fact_embeddings vec0), v12 (distance_metric=cosine on all vec0 tables).

```sql
-- Conversations
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    model_id TEXT,
    created_at INTEGER NOT NULL,
    title TEXT
);

-- Messages (refactored from v3: content_items in v4)
CREATE TABLE content_items (
    id INTEGER PRIMARY KEY,
    conversation_id TEXT,
    content_type TEXT NOT NULL,  -- 'message', 'note', 'document'
    role TEXT,
    content TEXT NOT NULL,
    model_id TEXT,
    timestamp INTEGER NOT NULL,
    importance REAL DEFAULT 0.5,
    access_count INTEGER DEFAULT 0,
    decay_score REAL DEFAULT 1.0,
    last_accessed INTEGER,
    source TEXT DEFAULT 'user',
    project_id TEXT,
    has_embedding INTEGER DEFAULT 0,
    prompt_tokens INTEGER DEFAULT 0,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

-- Content chunks (for long content)
CREATE TABLE content_chunks (
    id INTEGER PRIMARY KEY,
    item_id INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    has_embedding INTEGER DEFAULT 0,
    FOREIGN KEY (item_id) REFERENCES content_items(id)
);

-- Facts (added v10)
CREATE TABLE facts (
    id INTEGER PRIMARY KEY,
    scope TEXT NOT NULL DEFAULT 'project',  -- 'global' or 'project'
    category TEXT NOT NULL DEFAULT 'fact',   -- 'preference' or 'fact'
    content TEXT NOT NULL,
    importance REAL DEFAULT 1.0,
    access_count INTEGER DEFAULT 0,
    decay_score REAL DEFAULT 1.0,
    created_at INTEGER NOT NULL,
    last_accessed INTEGER,
    source TEXT DEFAULT 'user',
    project_id TEXT,
    has_embedding INTEGER DEFAULT 0,
    invalidated_at INTEGER  -- NULL = active, timestamp = replaced
);

-- FTS5 full-text index for facts
CREATE VIRTUAL TABLE facts_fts USING fts5(content, scope, category, project_id, content=facts, content_rowid=id);

-- Feedback signals (added v10)
CREATE TABLE feedback_signals (
    id INTEGER PRIMARY KEY,
    item_id INTEGER NOT NULL,
    signal_type TEXT NOT NULL,  -- 'positive', 'negative'
    source TEXT NOT NULL DEFAULT 'user',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (item_id) REFERENCES content_items(id)
);

-- Session todos
CREATE TABLE session_todos (
    id INTEGER PRIMARY KEY,
    content TEXT NOT NULL,
    completed INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Vector embeddings (schema v12: distance_metric=cosine on all vec0 tables)

-- Fact embeddings (256-dim, cosine distance)
CREATE VIRTUAL TABLE fact_embeddings USING vec0(
    fact_id INTEGER PRIMARY KEY,
    embedding FLOAT[256] distance_metric=cosine,
    +scope TEXT,
    +category TEXT,
    +project_id TEXT
);

-- Content item embeddings (256-dim, cosine distance)
CREATE VIRTUAL TABLE content_embeddings USING vec0(
    item_id INTEGER PRIMARY KEY,
    embedding FLOAT[256] distance_metric=cosine,
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT,
    +timestamp INTEGER
);

-- Chunk embeddings (256-dim, cosine distance)
CREATE VIRTUAL TABLE chunk_embeddings_v2 USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[256] distance_metric=cosine,
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT,
    +timestamp INTEGER
);
```

### Key Migration History

| Version | Change |
|---------|--------|
| v3→v4 | `messages` → `content_items` (unified type), added `feedback_signals` |
| v7 | `chunk_embeddings_v2` vec0 table, `has_embedding` on chunks |
| v10 | `facts` table, `facts_fts` FTS5 virtual table |
| v11 | `fact_embeddings` vec0 table, `has_embedding` on facts |
| v12 | `distance_metric=cosine` on all 3 vec0 tables (Bug #3 fix), ascending sort bug fix in content search |

---

## Design Decisions Summary

### Context Composition
- System prompt at start, current query at end
- Retrieved messages after system (avoid "lost in middle")
- Compaction preserves first N + last N + summary
- Messages never deleted from SQLite

### Retrieval
- Hybrid search: BM25 (keyword) + Semantic (vector cosine) + RRF fusion
- Default weights: 0.4 keyword, 0.6 semantic
- Automatic after 5+ messages (configurable)
- Forced after `/clear`

### Fact Memory
- 6-layer dedup: Exact → Normalized → Semantic+Triple (≥0.70) → FTS5 BM25 (≥0.75) → Startup verification (≥0.90) → Global-wins-project
- Exclusive predicates (`prefers`, `name is`) always contradict; Accumulative predicates (`likes`, `loves`) contradict only with word overlap > 0.3
- Polarity flips (`likes` → `hates`) always contradict
- Ebbinghaus decay: preferences 180d half-life, facts 30d half-life
- Schema v12 with `distance_metric=cosine` on all vec0 tables

### Tools
- Remember tool uses task-local storage for DB access
- Context enrichment for user questions (includes answers)
- Project-wide search for query mode

### Error Handling
- Tools always return `Ok(String)` on errors
- No `?` or `Err()` in tool functions
- Graceful degradation when DB unavailable

---

## References

- "Lost in the Middle: How LanguageModels Use Long Contexts" (Liu et al., 2023)
- Anthropic: "Prompt Engineering for Long Context"
- OpenAI: GPT-4 Prompt Engineering Guide
- Cohere: RAG Best Practices