# Context Management v2 - Implementation Plan

**Status:** Phase 4 Completed (v0.20.0), Phase 5 Pending  
**Date:** 2026-03-03  
**Based on:** Context Management Research + User Discussion

---

## Summary of Discussion

### Problem Analysis

The "Lost in the Middle" phenomenon affects LLM performance when critical information is buried in the middle of long contexts. Several mitigation strategies were analyzed:

### Technique Comparison

| Technique | Complexity | Information Loss | Best For |
|-----------|------------|------------------|----------|
| **Extractive Summarization** | Low | Low (preserves original) | Evidence preservation |
| **Abstractive Summarization** | Medium | Medium (rewrites) | Long conversations |
| **Hierarchical Context** | High | Low (drill-down) | Technical sessions |
| **Contextual Compression** | High | Medium | RAG systems |

### Non-Compression Alternatives

1. **Reordering** - Move important information to start/end
2. **Retrieval-Augmented Context (RAC)** - Load only relevant messages
3. **State Management** - Explicit state separate from history

---

## Recommended Strategy: Hybrid + State Management

### Core Insight

**State Management (To-Do List, File Session) reduces the need for long history.**

Instead of searching through 50 messages to find "what files did I read?", the state explicitly tracks:
- Files read/edited
- Decisions made
- Active tasks

This makes **Token-Based Pruning + Middle Compaction** viable without losing critical context.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     CONTEXT WINDOW                           │
├─────────────────────────────────────────────────────────────┤
│ 1. System Prompt (always)                                  │
│    - Role, behavior, tools, examples, platform              │
├─────────────────────────────────────────────────────────────┤
│ 2. Working State (always)                                  │
│    - Active tasks (to-do list)                              │
│    - Files read/edited (session state)                       │
│    - Key decisions (decision log)                            │
├─────────────────────────────────────────────────────────────┤
│ 3. Recent Messages (full fidelity)                          │
│    - Last N messages (configurable, default 10)             │
├─────────────────────────────────────────────────────────────┤
│ 4. Middle Summary (abstractive)                             │
│    - Compressed summary of messages 11 to N-10              │
│    - Generated on-demand or cached                           │
├─────────────────────────────────────────────────────────────┤
│ 5. Relevant History (semantic retrieval - future)          │
│    - Top-K messages matching current query                  │
│    - Requires embeddings (Phase 4)                           │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Foundation (v0.19.0) ✅ COMPLETED

- [x] Token counting utility (`src/tokens.rs`)
- [x] Context metrics (`ContextMetrics` struct)
- [x] `/context` command for session info
- [x] Tokens per message type display

### Phase 2: State Management (v0.19.0) ✅ COMPLETED

- [x] To-do list tools (`src/tools/todo.rs`)
- [x] TodoState persistence in session
- [x] Tools for task management (add, update, list, clear)

### Phase 3: Semantic Retrieval (v0.20.0) ✅ COMPLETED

**Goal:** Enable semantic search across conversation history

- [x] Database module (`src/db/`)
  - SQLite with sqlite-vec extension
  - FTS5 for keyword search
  - Schema: conversations, messages, message_embeddings
- [x] Embeddings module (`src/embeddings/`)
  - Ollama embedding client
  - Matryoshka truncation (768d → 256d)
- [x] Retrieval module (`src/retrieval/`)
  - Hybrid search (BM25 + semantic)
  - Reciprocal Rank Fusion (RRF)
- [x] `/search` command in REPL
- [x] FTS5 query sanitization

### Phase 4: Integration (v0.21.0) 🚧 PENDING

**Goal:** Auto-index messages on save, enable auto-retrieval

- [ ] Integrate with ChatSession
  - Auto-save messages to SQLite
  - Auto-generate embeddings on message
- [ ] `/migrate` command
  - Migrate JSON sessions to SQLite
  - Generate embeddings for existing messages
- [ ] `/reindex` command
  - Rebuild all embeddings
- [ ] Context overflow handling
  - Auto-compact at 80% context window
  - Middle summarization
- [ ] Auto-retrieval
  - M relevant messages (semantic)
  - N recent messages (chronological)
  - Combine for LLM context

### Phase 5: Future (v0.22+)

- [ ] Chat module integration (`/ocr`, `/vision` from chat)
- [ ] File session state tracking
- [ ] Hierarchical context compression

### Phase 2: To-Do List Tooling (v0.18.0)

**Goal:** State Management as primary context reduction

1. **To-Do List Tools**
   - `create_list(name: String)` - Create a new task list
   - `add_task(list: String, task: String)` - Add task to list
   - `update_task(list: String, id: usize, status: String)` - Update task status
   - `get_tasks(list: String)` - Retrieve current tasks
   - `clear_list(list: String)` - Clear completed tasks

2. **Session State Integration**
   - Track files read in session
   - Track decisions made
   - Expose via `/info` command

3. **Prompt Integration**
   - Include current to-do list state in system prompt
   - Include files read summary

### Phase 3: Token-Based Pruning (v0.19.0)

**Goal:** Automatic context management

1. **Sliding Window**
   - Configurable `max_messages` (default: 20)
   - Preserve system + working state + recent N

2. **Token-Based Trigger**
   - Auto-compact at 80% of context window
   - Visual indicator when compacting

3. **Middle Compaction**
   - Summarize messages in the middle
   - Keep first N and last N full
   - Use abstractive summarization

### Phase 4: Semantic Retrieval (v0.20.0+)

**Goal:** Intelligent context selection

1. **Embeddings Infrastructure**
   - Research Rust crates (`ort`, `candle`)
   - Local embedding model (all-MiniLM-L6-v2 or similar)
   - SQLite + sqlite-vec for storage

2. **Vector Storage**
   - Embed all conversation turns
   - Store in SQLite with session metadata

3. **Hybrid Retrieval**
   - Recent (5) + Relevant (K) + Summary
   - Semantic similarity search

---

## Embeddings Research (New High Priority Task)

**Priority:** HIGH (after To-Do List)  
**Status:** Model research complete, implementation research pending

### Model Research ✅

#### Primary: nomic-embed-text-v2-moe (Multilingual)

| Aspect | Value |
|--------|-------|
| **Size** | 958 MB |
| **Parameters** | 475M total, 305M active (MoE) |
| **Context Window** | 512 tokens |
| **Dimensions** | 768 (flexible: 768 → 256 via Matryoshka) |
| **Languages** | ~100 languages |
| **Training** | 1.6B multilingual pairs |

**Best for:** Multilingual conversations, Portuguese/English mixed context

**Key Features:**
- Mixture of Experts (8 experts, top-2 routing)
- Matryoshka embeddings: use 256-dim for 3x storage savings
- SoTA multilingual performance (MIRACL: 65.80)
- Fully open-source (weights, code, training data)

**Benchmark Comparison:**
```
Model                | Params | Dim  | BEIR  | MIRACL
---------------------|--------|------|-------|--------
nomic-embed-text-v2  | 305M   | 768  | 52.86 | 65.80 ✅
mE5 Base             | 278M   | 768  | 48.88 | 62.30
mGTE Base            | 305M   | 768  | 51.10 | 63.40
Arctic Embed v2 Base | 305M   | 768  | 55.40 | 59.90
BGE M3               | 568M   | 1024 | 48.80 | 69.20
```

#### Alternative: nomic-embed-text (English-only)

| Aspect | Value |
|--------|-------|
| **Size** | 274 MB |
| **Context Window** | 2048 tokens |
| **Dimensions** | 768 |
| **Languages** | English only |

**Best for:** English-only context, longer documents

**Key Features:**
- 4x longer context window (2048 vs 512 tokens)
- Surpasses OpenAI text-embedding-ada-002
- Surpasses text-embedding-3-small
- Smaller footprint (274MB vs 958MB)

**Comparison:**
```
Model                  | Size  | Context | Languages | Use Case
-----------------------|-------|---------|-----------|------------------
nomic-embed-text-v2-moe| 958MB | 512     | 100       | Multilingual
nomic-embed-text       | 274MB | 2048    | English   | Long English docs
```

### Usage Patterns

#### Prefix Conventions (nomic-embed-text-v2-moe only)

```rust
// REQUIRED for v2-moe - add appropriate prefixes
let query = "search_query: Como fazer X em Rust?";
let doc = "search_document: Guia completo de X em Rust...";

// NOT required for nomic-embed-text (v1)
```

#### Dimension Optimization

```rust
// Full precision (768d) - maximum quality
let embedding_dim = 768;  // ~3KB per message

// Matryoshka reduction (256d) - 3x storage savings
let embedding_dim = 256;  // ~1KB per message
// Performance degradation: ~5-10% on most benchmarks
```

### Integration with ollama-rs

The `ollama-rs` library (v0.3.4+) has native embedding support:

```rust
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

// Single embedding
let request = GenerateEmbeddingsRequest::new(
    "nomic-embed-text-v2-moe:latest".to_string(),
    "search_query: your text here".into()
);
let res = ollama.generate_embeddings(request).await?;

// Batch embeddings (multiple texts at once)
let request = GenerateEmbeddingsRequest::new(
    "nomic-embed-text-v2-moe:latest".to_string(),
    vec!["text 1", "text 2", "text 3"].into()
);
let res = ollama.generate_embeddings(request).await?;
```

### Recommended Configuration

```rust
// src/config.rs - add embedding model config
configs.insert(
    "nomic-embed",
    ModelConfig {
        model_id: "nomic-embed-text-v2-moe:latest".to_string(),
        num_ctx: 512,     // Max for v2-moe
        embedding_dim: 256, // Optimized for storage
        // ... temperature not applicable for embedding models
    },
);
```

---

### Implementation Research (Pending)

#### Rust Integration ✅

**Already solved via `ollama-rs`:**

The project already depends on `ollama-rs` (v0.3.4+) which has native embedding support:

```rust
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

// Single embedding
let request = GenerateEmbeddingsRequest::new(
    "nomic-embed-text-v2-moe:latest".to_string(),
    "search_query: your text here".into()
);
let res = ollama.generate_embeddings(request).await?;

// Batch embeddings
let request = GenerateEmbeddingsRequest::new(
    "nomic-embed-text-v2-moe:latest".to_string(),
    vec!["text 1", "text 2", "text 3"].into()
);
let res = ollama.generate_embeddings(request).await?;
```

**No additional crates needed** - `ollama-rs` handles all embedding operations via Ollama API.

#### SQLite & Storage ✅

**Decision:** sqlite-vec (sqlite-vss archived)

| sqlite-vec | sqlite-vss |
|------------|------------|
| Active development | **Archived** |
| Pure C, zero deps | Requires FAISS |
| Easy installation | Complex setup |
| Termux confirmed | Not tested |

**Storage Schema:**
```sql
-- Conversations
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT, model TEXT,
    created_at INTEGER DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- Messages
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    importance REAL DEFAULT 0.5,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

-- Vector embeddings (256-dim)
CREATE VIRTUAL TABLE message_embeddings USING vec0(
    message_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +conversation_id TEXT,
    +timestamp INTEGER
);

-- FTS5 for keyword search
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content, content='messages', content_rowid='id',
    tokenize='porter unicode61'
);
```

**Hybrid Search (BM25 + Semantic + RRF):**
- Keyword weight: 0.4
- Semantic weight: 0.6
- Similarity threshold: 0.7 cosine

**Storage Estimates:**
- 10,000 messages: ~20-30 MB
- 50,000 messages: ~85-125 MB

#### Architecture ✅

**Decisions:**
1. **Embedding timing:** On message creation (real-time)
2. **Dimension:** 256 (Matryoshka truncation)
3. **Storage:** SQLite + sqlite-vec + FTS5
4. **Retrieval:** Hybrid (keyword + semantic) with RRF

**Dependencies:**
```toml
rusqlite = { version = "0.32", features = ["bundled"] }
sqlite-vec = "0.1"
zerocopy = "0.8"
```

#### Performance ✅

**Estimates:**
- Embedding latency: ~50-100ms per message (Ollama API)
- Similarity search: ~1-5ms for 10k vectors
- Storage overhead: ~2-3 KB per message

#### Implementation Tasks

##### SQLite & Storage ✅
- [x] Select sqlite-vec (sqlite-vss archived)
- [x] Design storage schema (4 tables + triggers)
- [x] FTS5 integration for hybrid search
- [x] Test Termux compatibility (confirmed)

##### Architecture ✅
- [x] Hybrid retrieval (BM25 + semantic + RRF)
- [x] Dimension selection (256d Matryoshka)
- [x] Similarity threshold (0.7)
- [x] Module structure design

##### Implementation (Pending)
- [ ] Create `src/db/` module
- [ ] Create `src/embeddings/` module
- [ ] Create `src/retrieval/` module
- [ ] Integrate into chat session
- [ ] Add `/search` command for retrieval
- [ ] Test incremental updates
- [ ] Performance benchmarks

---

## File Session State (Complementary Feature)

**Goal:** Explicit tracking of file operations

```rust
struct FileSessionState {
    read_files: HashSet<PathBuf>,
    edited_files: HashMap<PathBuf, FileEditLog>,
    created_files: HashSet<PathBuf>,
    removed_files: HashSet<PathBuf>,
}

struct FileEditLog {
    path: PathBuf,
    edits: Vec<Edit>,
    last_read: DateTime,
    hash: Option<String>,
}
```

**Security Constraints:**
- Create: Only create files that don't exist
- Edit: Only edit files read in current session
- Remove: Only remove files read in full during session
- External modification detection via file hash/timestamp

---

## Recent Codebase Improvements

### Prompt Refactoring

**Before:** Monolithic `src/prompts.rs` (1700 tokens)

**After:** Modular structure (890 tokens, ~65% reduction)
```
src/prompts/
├── mod.rs         # Exports
├── base.rs        # Core prompts
├── builder.rs     # Build orchestrator
├── tools.rs       # Tool sections
├── examples.rs    # ReAct examples
├── personality.rs # Pepe overlay
└── platform.rs    # OS detection
```

**Key improvements:**
- Positive instructions (removed "DO NOT" / "NEVER")
- Hierarchical structure (`### ROLE`, `### BEHAVIOR`, etc.)
- Dynamic platform detection
- Few-shot ReAct examples

### Tool Calling Improvements

**CustomCoordinator:**
- Event-driven callbacks (`PreToolContent`, `ToolCall`, `ToolResult`)
- Preserves thinking/introductory text before tool calls
- Recursive tool processing

**Error Recovery:**
- Two-layer handling (tool level + coordinator level)
- Up to 3 retry attempts
- Model receives error as tool message

**Smaller Model Support:**
| Parameter | Default | Tool-Optimized |
|-----------|---------|-----------------|
| temperature | 0.7-1.0 | 0.1-0.3 |
| top_p | 0.9-1.0 | 0.80-0.95 |
| top_k | 40-100 | 20-50 |
| repeat_penalty | 1.0-1.2 | 1.0-1.05 |

---

## References

### Research Documents
- `doc/src/development/context_management_research.md` - Full research
- `TOOL_CALLING_RESEARCH.md` - Parameter optimization
- `doc/src/development/prompt-refactor.md` - Prompt structure

### Academic Papers
- "Lost in the Middle" - Context positioning matters
- "Mamba: Linear-Time Sequence Modeling" - Alternative architectures
- "RAG for Conversational AI" - Retrieval patterns

### Code References
- `src/chat/custom_coordinator.rs` - Tool execution
- `src/prompts/` - Prompt system
- `src/tools/` - Tool implementations