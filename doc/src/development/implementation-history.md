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

// Context Overflow
pub const OVERFLOW_THRESHOLD: f32 = 0.80;        // 80% of context window
pub const DEFAULT_KEEP_FIRST: usize = 5;         // Compaction
pub const DEFAULT_KEEP_LAST: usize = 5;           // Compaction

// Token Budgets
pub const SYSTEM_PROMPT_BUDGET: usize = 2000;
pub const RETRIEVED_MESSAGES_BUDGET: usize = 5000;
pub const COMPACTED_SUMMARY_BUDGET: usize = 1000;
pub const RECENT_MESSAGES_BUDGET: usize = 5000;
```

---

## Database Schema v3

```sql
-- Messages
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    importance REAL DEFAULT 0.5,
    has_embedding INTEGER DEFAULT 0
);

-- Message Chunks (for long messages)
CREATE TABLE message_chunks (
    id INTEGER PRIMARY KEY,
    message_id INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    has_embedding INTEGER DEFAULT 0,  -- v3: track embedding status
    FOREIGN KEY (message_id) REFERENCES messages(id)
);

-- Embeddings
CREATE TABLE message_embeddings (
    id INTEGER PRIMARY KEY,
    message_id INTEGER NOT NULL,
    embedding BLOB NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id)
);

-- Index for missing embeddings (for recovery on startup)
CREATE INDEX idx_chunks_missing_embedding 
    ON message_chunks(has_embedding) WHERE has_embedding = 0;
```

---

## Design Decisions Summary

### Context Composition
- System prompt at start, current query at end
- Retrieved messages after system (avoid "lost in middle")
- Compaction preserves first N + last N + summary
- Messages never deleted from SQLite

### Retrieval
- Hybrid search: BM25 (keyword) + Semantic (vector) + RRF fusion
- Default weights: 0.4 keyword, 0.6 semantic
- Automatic after 20+ messages (configurable)
- Forced after `/clear`

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