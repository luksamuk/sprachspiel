# Context Management v2 - Implementation Plan

**Status:** Planning  
**Date:** 2026-03-01  
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

### Phase 1: Foundation (v0.17.0)

**Goal:** Infrastructure for context management

1. **Token Counting**
   - Implement `count_tokens()` utility
   - Use `tiktoken-rs` or estimation (~0.75 words = 1 token for English)
   - Track tokens per message type
   - Display in `/info` and `/context` commands

2. **Session State Structure**
   ```rust
   struct SessionState {
       files_read: HashSet<PathBuf>,
       files_edited: HashMap<PathBuf, EditLog>,
       decisions: Vec<Decision>,
       active_tasks: Vec<Task>,
   }
   ```

3. **Context Metrics**
   - Total tokens used
   - Tokens per message type
   - Context window utilization percentage

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

#### SQLite Vector Extensions

**Status:** Research needed

**sqlite-vec:**
- Pure C extension, minimal dependencies
- Supports cosine similarity, L2 distance
- Binary blob storage for vectors
- Works with SQLite's BLOB type
- GitHub: https://github.com/asg017/sqlite-vec

**sqlite-vss:**
- Alternative extension
- More features, heavier dependencies
- May have compatibility issues

**Storage Estimation:**
```
Per message storage:
- 768 dimensions × 4 bytes (f32) = ~3 KB
- 256 dimensions × 4 bytes (f32) = ~1 KB

For 1000 messages:
- 768d: ~3 MB
- 256d: ~1 MB
```

#### Architecture Design

**Status:** Research needed

**Key Questions:**
1. **Embedding Generation Timing**
   - On message creation (real-time, higher latency)
   - Batch mode (background job, lower overhead)
   
2. **Storage Schema**
   - Session-based or global?
   - Embedding per message or per conversation turn?
   - Metadata to store (timestamp, role, topic?)
   
3. **Query Interface**
   - Similarity search API
   - Integration with context builder
   - Threshold configuration
   
4. **Dimension Selection**
   - 768d: Maximum quality
   - 256d: 3x storage savings, ~5-10% quality loss

#### Performance Testing

**Status:** Not started

**Metrics to measure:**
- Embedding latency (Ollama API call)
- Storage requirements (per dimension)
- Query latency for similarity search
- Memory usage during search

---

### Task Checklist

#### Model Selection ✅
- [x] Evaluate nomic-embed-text-v2-moe (958MB, multilingual, SoTA)
- [x] Evaluate nomic-embed-text (274MB, English, long context)
- [x] Compare benchmarks (BEIR, MIRACL)

#### Rust Integration ✅
- [x] Use ollama-rs for embeddings (no additional crates needed)
- [x] Document `generate_embeddings` API usage
- [x] Document batch embedding support

#### SQLite & Storage (Pending)
- [ ] Research sqlite-vec API and Rust bindings
- [ ] Research sqlite-vss as alternative
- [ ] Design storage schema for embeddings
- [ ] Prototype vector similarity queries

#### Architecture (Pending)
- [ ] Design embedding generation pipeline
- [ ] Design context retrieval API
- [ ] Decide: real-time vs batch embedding
- [ ] Decide: 768d vs 256d dimensions

#### Performance (Pending)
- [ ] Benchmark embedding latency
- [ ] Benchmark similarity search latency
- [ ] Measure storage overhead
- [ ] Test with 10/100/1000 message sessions

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