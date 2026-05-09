# Research: Context Management in LLM Agents

**Status:** Research Complete  
**Date:** 2026-02-24  
**Scope:** Comprehensive analysis of context/history management for Sprachspiel

---

## Executive Summary

Effective LLM context management requires balancing **recency** (recent messages), **relevance** (semantically similar messages), and **compression** (summarization/compaction). This research consolidates:

1. Industry best practices from OpenAI, Anthropic, LangChain
2. Academic research findings
3. Current Sprachspiel implementation state
4. Proposed roadmap features
5. Recommendations for implementation

---

## 1. Current Sprachspiel Implementation

### 1.1 How Context Works Today

**Chat Mode (`src/chat/`):**
- Full conversation history stored as JSON files (`~/.local/share/sprachspiel/conversations/`)
- All messages loaded into coordinator on `/load`
- No automatic pruning or compaction
- `/compact` command available for manual summarization
- History grows indefinitely until manual compact

**Query Mode (`src/query.rs`):**
- Stateless by design
- No history persistence between invocations
- Single-turn interactions only

**Vision/OCR Modes:**
- Stateless image processing
- No session continuity

### 1.2 Current Pain Points

From roadmap analysis:
- "Suspicion that LLMs are receiving excessive conversation history"
- "Too many messages in history consuming context window"
- "Redundant or low-value messages being included"
- Manual `/compact` requires user intervention
- No token counting or automatic threshold management

### 1.3 What We Have Implemented

✅ **Persistence:** JSON file storage with session management  
✅ **Manual Compaction:** `/compact` command for explicit summarization  
✅ **Model Detection:** Capability detection (context window size)  
✅ **Tool Integration:** Tool results stored in history  
❌ **Automatic Pruning:** Not implemented  
❌ **Token Counting:** Not implemented  
❌ **Smart Selection:** No relevance-based filtering  

---

## 2. Industry Best Practices

### 2.1 Core Strategies

**A. Sliding Window (Fixed-Length)**
```
Keep: System + Last N messages
Discard: Everything else
```
- **Pros:** Simple, predictable, fast
- **Cons:** Loses older context completely
- **Use case:** Short conversations, low latency requirements

**B. Token-Based Pruning**
```
If total_tokens > threshold:
  - Preserve: System message, recent user messages
  - Remove: Oldest messages until under threshold
```
- **Pros:** Respects actual context limits
- **Cons:** Variable message count
- **Use case:** Long conversations with varying message sizes

**C. Hierarchical Context (Recommended)**
```
Level 1: System prompt + Current query (always kept)
Level 2: Summary of recent conversation (compressed)
Level 3: Vector-retrieved relevant history (semantic)
Level 4: Full detailed history (rarely used)
```
- **Pros:** Balances recency, relevance, and compression
- **Cons:** Complex implementation
- **Use case:** Production agents with long sessions

**D. OpenAI's Compaction Pattern**
```python
# Automatic server-side compaction
context_management = [
    {"type": "compaction", "compact_threshold": 200000}
]
```
- **Pros:** Automatic, efficient
- **Cons:** API-specific, vendor lock-in
- **Use case:** OpenAI API users

### 2.2 Message Pruning Hierarchy

From most to least important:

1. **System Message** - Never remove
2. **User Instructions** - Critical context
3. **Recent User Messages** (last 2-3) - Current intent
4. **Tool Results** (unresolved) - Active tasks
5. **Assistant Responses** (recent) - Recent answers
6. **Tool Calls** (completed) - Can be summarized
7. **Old Conversation Turns** - Summarize or remove
8. **Reasoning Traces** - Compress after completion

### 2.3 Token Counting

**Key Formulas:**
- English text: ~0.75 words = 1 token
- Code: ~0.5 tokens per character
- Tool definitions: Count toward system tokens
- Overhead: ~4 tokens per message

**Implementation Pattern:**
```rust
fn count_tokens(messages: &[Message], model: &str) -> usize {
    // Use tiktoken or model-specific tokenizer
    // Include message overhead
    // Include tool definitions
}
```

### 2.4 Relevance-Based Selection

**RAG for Conversation Memory:**

1. Embed all conversation turns as vectors
2. Store in vector database
3. For each query:
   - Embed the query
   - Retrieve top-K semantically similar messages
   - Combine with recent messages

**Hybrid Approach (Recommended):**
```
Context = System + Recent(3) + Relevant(K) + Summary
```

---

## 3. Academic Research Findings

### 3.1 "Lost in the Middle" Phenomenon

**Finding:** LLMs struggle to access information in the middle of long contexts.

**Implication:** Place most critical information at **start** (system) or **end** (recent) of context, not buried in middle.

**Recommendation for Sprachspiel:**
- Keep system prompt concise and at beginning
- Recent user messages at end
- Historical context summarized or vector-retrieved

### 3.2 Mamba & Linear-Time Models

**Paper:** "Mamba: Linear-Time Sequence Modeling" (2312.00752)

**Key Insight:** Selective state spaces allow content-based reasoning with linear scaling to million-length sequences.

**Implication for Transformers:** Current models still have quadratic attention complexity. Need efficient context management.

### 3.3 Context Window vs. Effective Context

**Research shows:**
- Larger context windows ≠ better performance
- Models degrade with excessive context
- Selective attention is more important than raw size

**Recommendation:** Optimize for effective context usage, not just window size.

---

## 4. Framework Comparison

| Framework | Approach | Pros | Cons |
|------------|----------|------|------|
| **LangChain** | Multiple strategies (Buffer, Summary, Vector) | Flexible, well-documented | Can be overwhelming |
| **LlamaIndex** | RAG-based memory | Semantic relevance | Requires embeddings |
| **OpenAI Agents** | Built-in compaction | Automatic | Vendor lock-in |
| **Anthropic** | Summarization cookbooks | Practical examples | Manual implementation |

### 4.1 LangChain's Memory Types

1. **ConversationBufferMemory** - Raw history (simple, unlimited)
2. **ConversationBufferWindowMemory** - Sliding window (fixed N messages)
3. **ConversationSummaryMemory** - Dynamic summarization
4. **VectorStoreRetrieverMemory** - Semantic retrieval

**Best for Sprachspiel:** Combination of Window + Summary + Vector

---

## 5. Session State Management Patterns

### 5.1 Multi-Tier Storage

```
Tier 1 (Hot - In-Memory): Last 10 messages
  └── Fast access, current context
  
Tier 2 (Warm - Redis): Last 50 messages
  └── Distributed, survives restarts
  
Tier 3 (Cold - JSON files): Full history
  └── Persistent, queryable
  
Tier 4 (Archive - File): Old sessions
  └── Long-term storage
```

### 5.2 Sprachspiel Current vs. Proposed

**Current:**
```
JSON Files (Full History) → Load All → Coordinator
```

**Proposed:**
```
JSON Files (Full History) → Smart Filter → Coordinator
                ↓
         [Pruning/Summarization]
                ↓
         Context Window
```

---

## 6. Roadmap Integration

### 6.1 LLM Context History Redesign

**Current Status:** Research complete  
**Next Steps:**

1. **Audit Current Mechanism**
   - Measure actual token usage per session
   - Identify redundant messages
   - Benchmark with different history sizes

2. **Implement Token Counting**
   - Research tiktoken-equivalent for Rust
   - Or use model-agnostic estimation
   - Track tokens per message type

3. **Smart Pruning Strategy**
   ```rust
   enum PruningStrategy {
       SlidingWindow { max_messages: usize },
       TokenBased { max_tokens: usize },
       Hierarchical { recent: usize, summary: bool },
   }
   ```

4. **Integration Points**
   - `/compact` becomes automatic
   - New `/context` command for manual control
   - Configurable thresholds in `config.toml`

### 6.2 Automatic Conversation Compaction

**Trigger Conditions:**
- Token count > 80% of model context window
- Message count > threshold (configurable)
- Before tool calls if context is large
- User-configurable auto-compact on/off

**Implementation:**
```rust
impl ChatSession {
    async fn check_and_compact(&mut self) {
        let token_count = self.count_tokens();
        let threshold = self.model_context_window * 0.8;
        
        if token_count > threshold {
            self.compact().await;
        }
    }
}
```

### 6.3 To-Do List Tooling Integration

**Context Impact:**
- To-do lists reduce need for full conversation history
- Model can reference list instead of scanning history
- Compact representation of task progress

**Implementation Considerations:**
- Store list state separately from chat history
- Include list in context when relevant
- Summarize list items instead of full history

### 6.4 File Modification Tools Context

**Session Tracking:**
```rust
struct FileSessionState {
    read_files: HashSet<PathBuf>,
    edited_files: HashMap<PathBuf, FileEditLog>,
    created_files: HashSet<PathBuf>,
    removed_files: HashSet<PathBuf>,
}
```

**Context Integration:**
- Include file modification history in system prompt
- Reference edited files by ID, not full content
- Clear file records on deletion

---

## 7. Recommendations

### 7.1 Immediate Actions (v0.17.0)

1. **Implement Token Counting**
   - Add `count_tokens()` utility
   - Log token usage per session
   - Display tokens in `/info` command

2. **Add Sliding Window Option**
   - Configurable `max_messages` in config
   - Default: Keep last 20 messages
   - Preserve system + user instructions

3. **Enhance `/compact`**
   - Show token savings
   - Add `--auto` flag for automatic mode
   - Configurable threshold

### 7.2 Short-term (v0.18.0)

1. **Automatic Compaction**
   - Trigger at 80% of context window
   - Configurable threshold
   - Visual indicator when compacting

2. **Hierarchical Context**
   - Summarize older conversation
   - Keep recent N messages in full
   - Vector retrieval for relevant history

3. **Context Metrics**
   - Tokens used per message type
   - Context window utilization
   - Compression ratio after compact

### 7.3 Long-term (v0.19.0+)

1. **Relevance-Based Selection**
   - Embed conversation turns
   - Semantic similarity search
   - Hybrid: Recent + Relevant + Summary

2. **Multi-Tier Storage**
   - Hot cache in memory
   - Warm cache in Redis
   - Cold storage in JSON files

3. **Smart Context Injection**
   - Include AGENTS.md content intelligently
   - Only inject relevant sections
   - Update based on conversation topic

---

## 8. Configuration Recommendations

### 8.1 Proposed Config Options

```toml
[context]
# Strategy: "sliding_window", "token_based", "hierarchical"
strategy = "hierarchical"

# Sliding window settings
max_messages = 20

# Token-based settings  
max_tokens = 6000
reserve_tokens = 1000  # Leave room for response

# Hierarchical settings
recent_messages = 5
summary_messages = 10
vector_retrieval = true

# Automatic compaction
auto_compact = true
compact_threshold = 0.8  # 80% of context window

# Token counting
model_tokenizer = "auto"  # or "tiktoken", "estimate"
```

### 8.2 Per-Model Context Settings

```toml
[models.llama3.1]
model_id = "llama3.1:8b"
num_ctx = 4096
# Inherit global context settings or override
context_strategy = "sliding_window"
max_messages = 15

[models.qwen3]
model_id = "qwen3:8b"
num_ctx = 32768
# Large context, can keep more history
context_strategy = "token_based"
max_tokens = 24000
```

---

## 9. Future Architecture: Embeddings + Vector Search

### 9.1 Why Embeddings?

Embeddings enable **semantic retrieval** of conversation history—finding messages that are *meaningfully related* to the current query, not just temporally recent.

**Example:**
```
Current: "How did I implement the login?"
Recent: Discussion about UI design
Semantic match: "Authentication with bcrypt" (50 messages ago)
```

Embeddings capture meaning, enabling relevance-based context selection.

### 9.2 SQLite + Vector Extensions

SQLite supports vector storage and similarity search through extensions:

**Option A: sqlite-vss** (Vector Similarity Search)
- GitHub: https://github.com/asg017/sqlite-vss
- Uses Faiss for efficient vector indexing
- Supports L2, cosine, and inner product similarity
- Production-ready

**Option B: sqlite-vec** (Recommended - newer)
- GitHub: https://github.com/asg017/sqlite-vec
- Pure C, no external dependencies
- Smaller, faster than sqlite-vss
- Native SQLite extension

### 9.3 Proposed Schema with Embeddings

```sql
-- Messages table with embeddings
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,  -- "user", "assistant", "system", "tool"
    content TEXT NOT NULL,
    embedding BLOB,      -- 768 or 1536 dimensions (f32 array)
    tokens INTEGER,      -- Token count
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

-- Virtual table for vector similarity search
CREATE VIRTUAL TABLE vss_messages USING vec0(
    embedding FLOAT[768]  -- Match your embedding model
);

-- Index for fast retrieval
CREATE INDEX idx_messages_session ON messages(session_id, timestamp);

-- Hybrid query: Recent + Relevant
WITH recent AS (
    SELECT * FROM messages 
    WHERE session_id = ? 
    ORDER BY timestamp DESC 
    LIMIT 5
),
relevant AS (
    SELECT m.* FROM messages m
    JOIN vss_messages v ON m.id = v.rowid
    WHERE vss_search(v.embedding, ?, 10)
    AND m.session_id = ?
    AND m.id NOT IN (SELECT id FROM recent)
    ORDER BY vss_distance
    LIMIT 5
)
SELECT * FROM recent
UNION ALL
SELECT * FROM relevant
ORDER BY timestamp DESC;
```

### 9.4 Embedding Models

**Local Options (no API required):**

| Model | Dimensions | Size | Best For |
|-------|-----------|------|----------|
| **all-MiniLM-L6-v2** | 384 | 80MB | Fast, good quality |
| **all-mpnet-base-v2** | 768 | 420MB | Better quality |
| **sentence-t5-large** | 768 | 1GB | Multilingual |
| **nomic-embed-text-v1** | 768 | 520MB | Open source, good |

**Rust Implementation:**
- Use `ort` crate for ONNX Runtime
- Or `candle` crate for pure Rust
- Embed at message creation time
- Store binary blob in SQLite

### 9.5 Migration Path: JSON → SQLite

**Phase 1: Dual Storage (Backward Compatible)**
```rust
pub struct HybridStorage {
    json: ConversationStorage,  // Existing
    sqlite: SqliteStorage,      // New
}

impl HybridStorage {
    fn save(&self, session: &Session) {
        self.json.save_session(session);  // Keep working
        self.sqlite.save_session(session); // New path
    }
}
```

**Phase 2: SQLite Only**
- Remove JSON fallback
- Optimize SQLite queries
- Add vector search

**Phase 3: Embeddings**
- Add embedding generation
- Index existing messages
- Enable semantic retrieval

### 9.6 When to Migrate

**Current (JSON):** ✅ Sufficient for now
- Hundreds of messages per session
- Simple load/save operations
- No complex queries needed

**Future (SQLite + Vectors):** When you need:
- Thousands of messages per session
- Semantic search in history
- Complex filtering (by date, role, content)
- Multi-session queries
- Analytics on conversation patterns

### 9.7 Implementation Considerations

**Pros of SQLite + Embeddings:**
- SQL queries for complex retrieval
- ACID transactions
- Scalable to millions of messages
- Semantic relevance search
- Standard tooling (DB viewers, backups)

**Cons:**
- Additional dependency (sqlite-vec)
- Embedding generation overhead
- More complex setup
- Migration effort

**Recommendation:** Plan for SQLite migration in v0.20+, implement embeddings when you have a clear use case for semantic history retrieval.

### 9.8 References

**SQLite Vector Extensions:**
- sqlite-vss: https://github.com/asg017/sqlite-vss
- sqlite-vec: https://github.com/asg017/sqlite-vec (recommended)
- SQLite FTS5: https://www.sqlite.org/fts5.html (text search alternative)

**Embedding Models:**
- Sentence Transformers: https://www.sbert.net/
- MTEB Leaderboard: https://huggingface.co/spaces/mteb/leaderboard
- Nomic Embed: https://github.com/nomic-ai/nomic

**Rust Crates:**
- `rusqlite`: SQLite bindings
- `ort`: ONNX Runtime for embeddings
- `candle`: Hugging Face's Rust ML framework
- `rust-bert`: BERT in Rust

**Research:**
- "Lost in the Middle" - Context positioning matters
- "RAG for Conversational AI" - Retrieval-augmented generation
- "Embedding-based Conversation Memory" - Vector retrieval patterns

---

## 10. Implementation Checklist

### Phase 1: Foundation
- [ ] Research tiktoken Rust equivalent
- [ ] Implement token counting utility
- [ ] Add token metrics to chat sessions
- [ ] Create context management module

### Phase 2: Basic Strategies
- [ ] Implement sliding window pruning
- [ ] Implement token-based pruning
- [ ] Add context strategy configuration
- [ ] Update `/compact` command

### Phase 3: Advanced Features
- [ ] Implement conversation summarization
- [ ] Add automatic compaction
- [ ] Create hierarchical context mode
- [ ] Add context visualization commands

### Phase 4: Intelligence
- [ ] Implement semantic embedding for messages
- [ ] Add vector retrieval for relevant history
- [ ] Smart AGENTS.md section selection
- [ ] Context-based tool recommendation

---

## 10. Research Sources

### Documentation
- OpenAI Agents SDK Context Management
- Anthropic Claude Best Practices Cookbook
- LangChain Memory Documentation
- LlamaIndex Chat Memory Guide

### Academic Papers
- "Mamba: Linear-Time Sequence Modeling" (2312.00752)
- "Lost in the Middle" (Context positioning)
- "Megalodon" (2404.08801) - Unlimited context

### Frameworks Analyzed
- LangChain (Python)
- LangGraph
- LlamaIndex
- OpenAI Agents SDK
- Pinecone Context Management

### Best Practices
- Token counting strategies
- Message pruning hierarchies
- Session state patterns
- Multi-tier storage architectures

---

## Conclusion

Effective context management is crucial for production LLM agents. The key insights are:

1. **Never send full history** - Always filter/compress
2. **Preserve system instructions** - Critical for behavior
3. **Balance recency and relevance** - Recent + Semantic retrieval
4. **Monitor token usage** - Track and optimize
5. **Automatic > Manual** - Auto-compact before issues arise
6. **Hierarchical is best** - Multi-level context strategies

Sprachspiel should implement a **hierarchical context strategy** combining:
- Recent messages (full)
- Summarized older conversation
- Vector-retrieved relevant history
- Automatic compaction at thresholds

This approach balances performance, cost, and capability while remaining framework-agnostic.
