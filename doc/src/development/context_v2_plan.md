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
**Status:** Research needed

### Tasks

- [ ] Evaluate Rust embedding crates
  - [ ] `ort` (ONNX Runtime) - production-ready, maintained
  - [ ] `candle` (Hugging Face) - pure Rust, flexible
  - [ ] `rust-bert` - BERT in Rust
  
- [ ] Evaluate local embedding models
  - [ ] all-MiniLM-L6-v2 (384d, 80MB) - fast, good quality
  - [ ] all-mpnet-base-v2 (768d, 420MB) - better quality
  - [ ] nomic-embed-text-v1 (768d, 520MB) - open source
  
- [ ] Evaluate SQLite vector extensions
  - [ ] sqlite-vec (recommended)
  - [ ] sqlite-vss (alternative)
  
- [ ] Architecture design
  - [ ] Embedding generation timing (on message creation vs batch)
  - [ ] Storage schema (messages + embeddings)
  - [ ] Query interface for semantic retrieval
  
- [ ] Performance testing
  - [ ] Embedding latency
  - [ ] Storage requirements
  - [ ] Query latency for similarity search

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