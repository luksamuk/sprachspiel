# Context Composition Design (v0.21.0)

**Status:** Archived — See [Legacy Documentation](./legacy.md)

> This document has been archived. The context composition design is now documented in:
> - **[Memory Architecture](./memory-architecture.md)** — Unified overview of all memory systems
> - **[Context Anatomy](./context-anatomy.md)** — Current context composition details

---

**Original Version:** 0.21.0  
**Date:** 2026-03-03

---

## Overview

This document captures all design decisions for v0.21.0, which integrates ChatSession with SQLite storage, enables auto-retrieval, and implements context overflow handling.

---

## Research Foundation

### Lost in the Middle Phenomenon

Based on "Lost in the Middle: How Language Models Use Long Contexts" (Liu et al., 2023):

- **Critical finding:** Model performance degrades significantly when relevant information is in the **middle** of context windows
- **Optimal positions:** Beginning or end of context
- **U-shaped performance curve:** Affects even long-context models

### Provider Recommendations

| Provider | Key Recommendation |
|----------|-------------------|
| **Anthropic** | "Put longform data at the top, queries at the end" - up to 30% better performance |
| **OpenAI** | Pin to specific snapshots for consistency, use Markdown/XML tags |
| **Cohere** | Use Rerank endpoint to sort by relevance, ~400 word chunks |

---

## Architecture Decisions

### ChatSession Integration

```rust
pub struct ChatSession {
    // Existing fields...
    pub messages: Vec<SavedMessage>,
    pub compacted_summary: Option<String>,
    pub messages_sent_to_llm: usize,
    
    // NEW FIELDS (v0.21.0)
    #[serde(skip)]
    pub db: Option<Arc<Database>>,
    #[serde(skip)]
    pub embedding_client: Option<Arc<EmbeddingClient>>,
}
```

**Decision:** Use `Arc` for thread-safe sharing. Fields are `#[serde(skip)]` because they can't be serialized.

### Message Saving Flow

```
User sends message
    ↓
add_user_message_async(content)
    ↓
1. Add to memory (immediate)
    ↓
2. Save to SQLite (immediate)
    ↓
3. Generate embedding (async, fire-and-forget)
    ↓
Return to user (no blocking)
```

**Decision:** Embeddings are generated in background via `tokio::spawn`. Message is saved immediately, embedding comes later.

### Serialization Strategy

**Problem:** Database and EmbeddingClient can't be serialized.

**Solution:** 
- Fields marked with `#[serde(skip)]`
- Reconstructed when session is loaded from JSON
- Database path is deterministic (`~/.local/share/ask-ai/ask-ai.db`)

---

## Context Composition

### Optimal Ordering (Research-Based)

```
┌──────────────────────────────────────────────────────────────────┐
│ 1. SYSTEM PROMPT (~500-2000 tokens)                              │
│    Position: BEGINNING (Claude: "up to 30% better")              │
│    Contains: Identity, behavior, tools, examples, platform       │
├──────────────────────────────────────────────────────────────────┤
│ 2. RECOVERED MESSAGES (1000-5000 tokens)                         │
│    Position: AFTER SYSTEM (avoid "lost in middle")              │
│    Contains: M semantically relevant messages from history       │
│    Format: <retrieved index="1">...</retrieved>                  │
├──────────────────────────────────────────────────────────────────┤
│ 3. COMPACTED SUMMARY (~500-1000 tokens)                           │
│    Position: BEFORE RECENT                                       │
│    Contains: Summary of messages that were compacted             │
│    Preserves: Entities, decisions, facts                         │
├──────────────────────────────────────────────────────────────────┤
│ 4. RECENT MESSAGES (2000-5000 tokens)                            │
│    Position: BEFORE QUERY (avoid "lost in middle")              │
│    Contains: Last N message pairs in full fidelity               │
│    Default: 10 messages (5 pairs)                                 │
├──────────────────────────────────────────────────────────────────┤
│ 5. CURRENT USER QUERY                                            │
│    Position: VERY END (Anthropic: "critical for quality")        │
│    Contains: The actual question being asked                     │
└──────────────────────────────────────────────────────────────────┘
```

### Size Targets

| Component | Tokens | Priority | Notes |
|-----------|--------|----------|-------|
| System Prompt | 500-2000 | Critical | Always included |
| Recovered Messages | 1000-5000 | High | Only when retrieval active |
| Compacted Summary | 500-1000 | Medium | Only when compacted |
| Recent Messages | 2000-5000 | High | Last N messages |
| Current Query | Variable | Critical | Always at end |

**Total target:** 60-80% of model's context window

---

## Auto-Retrieval Design

### When to Activate Retrieval

| Context Usage | Behavior |
|--------------|----------|
| < 50% | No retrieval (normal history) |
| 50-80% | No retrieval (normal history) |
| ≥ 80% | Trigger auto-compaction |
| Post-compaction | Auto-retrieval ON for next queries |

**Alternative:** Configurable always-on retrieval with throttling:

```toml
[retrieval]
enabled = true
min_messages = 20          # Only activate after 20+ messages
min_query_interval = 5     # Skip if last query was < 5 seconds ago
```

### Retrieval Parameters

```toml
[retrieval]
relevant_count = 5        # M semantically relevant messages
recent_count = 10         # N recent messages (chronological)
keyword_weight = 0.4      # BM25 weight in hybrid search
semantic_weight = 0.6     # Vector weight in hybrid search
```

### Performance Considerations

- **Embedding latency:** ~50-100ms per query via Ollama API
- **Similarity search:** ~1-5ms for 10k vectors in SQLite
- **Throttling:** Skip retrieval if last query < 5 seconds ago

### Deduplication

**Decision:** Do NOT deduplicate retrieved messages against summary.

**Rationale:** Summary is condensed. User may want specific details that were omitted. Better to include potentially duplicate information than miss important details.

---

## Auto-Compaction Design

### Context Overflow Detection

```rust
pub enum ContextStatus {
    Ok {
        total_tokens: usize,
        max_tokens: usize,
    },
    Overflow {
        total_tokens: usize,
        max_tokens: usize,
        usage_percent: u8,
    },
}

// Trigger at 80% of context window
const OVERFLOW_THRESHOLD: f32 = 0.8;

pub fn check_context_overflow(
    session: &ChatSession,
    model_context_window: usize,
) -> ContextStatus {
    let messages = session.get_messages_for_llm("");
    let total_tokens: usize = messages.iter()
        .map(|m| count_tokens(&m.content))
        .sum();
    
    let usage = total_tokens as f32 / model_context_window as f32;
    
    if usage >= OVERFLOW_THRESHOLD {
        ContextStatus::Overflow { /* ... */ }
    } else {
        ContextStatus::Ok { /* ... */ }
    }
}
```

### Compaction Strategy

**Middle Compaction:**
1. Keep first N messages (usually system + initial context)
2. Keep last N messages (recent conversation)
3. Summarize messages in between

```rust
pub fn compact_middle_messages(
    session: &mut ChatSession,
    keep_first: usize,
    keep_last: usize,
) -> Option<String> {
    let total = session.messages.len();
    
    if total <= keep_first + keep_last {
        return None;  // Nothing to compact
    }
    
    // Extract middle for summary
    // Generate summary via LLM
    // Set session.compacted_summary
    
    // IMPORTANT: Messages remain in SQLite!
    // Only the LLM context is affected
}
```

**Critical Rule:** Messages are NEVER deleted from SQLite. Compaction only affects what's sent to the LLM.

---

## New Commands

### `/migrate` Command

Migrates existing JSON sessions to SQLite with embeddings.

**Usage:**
```
/migrate                 # Migrate all sessions for current project
/migrate <session_id>    # Migrate specific session
```

**Process:**
1. Load JSON session
2. Register conversation in SQLite
3. For each message:
   - Insert into SQLite
   - Generate embedding (batch for efficiency)
4. Report statistics

### `/reindex` Command

Rebuilds all embeddings (useful when embedding model changes).

**Usage:**
```
/reindex                      # Reindex all messages
/reindex <conversation_id>    # Reindex specific conversation
```

**Process:**
1. Fetch all messages from SQLite
2. Generate embeddings in batch
3. Update `message_embeddings` table

### `/retrieval` Command (Optional)

Toggle auto-retrieval for current session.

**Usage:**
```
/retrieval on      # Enable auto-retrieval
/retrieval off     # Disable auto-retrieval
/retrieval status  # Show current status
```

---

## Context Builder Implementation

```rust
/// Build context for LLM with optimal ordering
pub async fn build_context(
    session: &ChatSession,
    db: &Database,
    embedding_client: &EmbeddingClient,
    user_query: &str,
    config: &RetrievalConfig,
) -> Result<Vec<ChatMessage>, String> {
    let mut messages = Vec::new();
    
    // 1. System prompt (always first)
    let prompt = session.system_prompt.as_deref().unwrap_or("");
    messages.push(ChatMessage::system(prompt));
    
    // 2. Retrieved messages (if enabled, after system to avoid lost-in-middle)
    if config.enabled && session.messages.len() >= config.min_messages {
        let embedding = embedding_client.embed(user_query).await?;
        let retrieved = db.search_hybrid(
            user_query,
            &embedding,
            Some(&session.id),
            config.relevant_count,
            config.keyword_weight,
            config.semantic_weight,
        )?;
        
        // Format with XML tags
        if !retrieved.is_empty() {
            let mut retrieved_text = String::from("<retrieved_context>\n");
            for (i, msg) in retrieved.iter().enumerate() {
                retrieved_text.push_str(&format!(
                    "<message index=\"{}\" timestamp=\"{}\">\n<role>{}</role>\n<content>{}</content>\n</message>\n",
                    i + 1, msg.timestamp, msg.role, msg.content
                ));
            }
            retrieved_text.push_str("</retrieved_context>");
            messages.push(ChatMessage::system(retrieved_text));
        }
    }
    
    // 3. Compacted summary (if present)
    if let Some(ref summary) = session.compacted_summary {
        messages.push(ChatMessage::system(format!(
            "<summary_context>\n{}\n</summary_context>",
            summary
        )));
    }
    
    // 4. Recent messages (before query, avoid lost-in-middle)
    let recent: Vec<_> = session.messages.iter()
        .rev()
        .take(config.recent_count)
        .rev()
        .collect();
    
    for msg in recent {
        match msg.role {
            MessageRole::User => messages.push(ChatMessage::user(msg.content.clone())),
            MessageRole::Assistant => messages.push(ChatMessage::assistant(msg.content.clone())),
            MessageRole::System => {}  // Handled separately
            MessageRole::Tool => messages.push(ChatMessage::tool(msg.content.clone())),
        }
    }
    
    // 5. Current query (always last)
    // Note: User's actual query is sent as the last user message
    // by the calling code, not added here
    
    Ok(messages)
}
```

---

## Anti-Patterns to Avoid

| Anti-Pattern | Problem | Solution |
|--------------|---------|----------|
| Retrieval in middle | Lost in middle | Place AFTER system prompt |
| Query in middle | Confuses model | Query ALWAYS at very end |
| Context 100% full | Performance degradation | Target 60-80% utilization |
| No structure tags | Model confusion | Use XML tags for sections |
| Generic summary | Lost details | Preserve entities, facts, decisions |
| Delete from SQLite | Lost embeddings | Never delete, only compact LLM context |

---

## Configuration

### config.toml

```toml
[context]
# Trigger compaction at 80% of context window
overflow_threshold = 0.8

# Messages to preserve during compaction
keep_first = 5   # System + initial context
keep_last = 5    # Recent conversation

[retrieval]
# Enable semantic retrieval
enabled = true

# Minimum messages before activation
min_messages = 20

# Retrieve M semantically relevant messages
relevant_count = 5

# Include last N messages
recent_count = 10

# Skip if last query < X seconds ago (performance)
min_query_interval_secs = 5

# RRF weights for hybrid search
keyword_weight = 0.4
semantic_weight = 0.6
```

---

## Implementation Order

### Phase 1: ChatSession Integration
1. Add `db` and `embedding_client` fields to ChatSession
2. Modify `add_user_message` to save to SQLite
3. Add async variant for embedding generation
4. Initialize database in REPL

### Phase 2: `/migrate` Command
1. Create `src/db/migration.rs`
2. Implement batch embedding for efficiency
3. Add command to REPL
4. Report progress during migration

### Phase 3: `/reindex` Command
1. Add `get_messages_for_reindex` to Database
2. Add command to REPL
3. Test with embedding model changes

### Phase 4: Context Overflow
1. Create `src/context/` module
2. Implement overflow detection
3. Implement middle compaction (stub for LLM summary)
4. Test threshold triggers

### Phase 5: Auto-Retrieval
1. Create `src/retrieval/context.rs`
2. Implement `build_context` function
3. Integrate with ChatSession
4. Add configuration

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_session_saves_to_sqlite() { /* ... */ }

#[test]
fn test_embedding_generated_async() { /* ... */ }

#[test]
fn test_migrate_json_session() { /* ... */ }

#[test]
fn test_reindex_regenerates_embeddings() { /* ... */ }

#[test]
fn test_detects_overflow_80_percent() { /* ... */ }

#[test]
fn test_build_context_ordering() { /* ... */ }

#[test]
fn test_deduplication_logic() { /* ... */ }

#[test]
fn test_context_60_to_80_percent() { /* ... */ }
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_retrieval_flow() {
    // 1. Create session with 25 messages
    // 2. Trigger retrieval
    // 3. Verify 5 retrieved + 10 recent
    // 4. Verify correct ordering
}

#[tokio::test]
async fn test_compaction_preserves_sqlite() {
    // 1. Create session with 50 messages
    // 2. Trigger compaction
    // 3. Verify SQLite still has 50 messages
    // 4. Verify LLM context only has summary + recent
}
```

---

## Acceptance Criteria

- [ ] Messages are saved to SQLite automatically
- [ ] Embeddings are generated asynchronously (non-blocking)
- [ ] `/migrate` migrates JSON sessions with progress report
- [ ] `/reindex` regenerates embeddings for all messages
- [ ] Context stays below 80% threshold (auto-compaction)
- [ ] Retrieved messages positioned after system prompt
- [ ] Current query always at the end
- [ ] Context utilization target: 60-80%
- [ ] Configuration options for retrieval parameters

---

## References

### Academic Papers
- "Lost in the Middle: How Language Models Use Long Contexts" (Liu et al., 2023)

### Provider Documentation
- Anthropic: Prompt Engineering for Long Context
- OpenAI: GPT-4 Prompt Engineering Guide
- Cohere: RAG Best Practices

### Code References
- LangChain Memory Strategies
- LlamaIndex Context Management

---

## Changelog

- **2026-03-03:** Initial design document created