# Roadmap

This document outlines planned features and the current state of Ask-AI.

## Current State

### Implemented Features

**Core CLI:**
- 5 subcommands (query, chat, translate, ocr, summarize)
- 3 built-in model presets (llama3.1, translategemma, glm-ocr)
- User-defined models via `~/.config/ask-ai/models.toml`
- Optional model parameters (top_k, top_p, repeat_penalty)
- Thinking support for cloud models (`thinking = true` in config)
- Markdown rendering via termimad
- Model capability detection (tools, vision, ocr)
- Pipe support for all commands
- Debug mode, Think mode, Code mode
- Configuration file support (`~/.config/ask-ai/config.toml`)
- Per-subcommand model configuration
- AGENTS.md context injection with security sanitization
- Shell argument handling

**Interactive Chat:**
- Persistent conversation history per project
- Anonymous sessions (no persistence)
- Session management (`/save`, `/load`, `/list`)
- Model switching mid-conversation
- Export to Markdown/JSON
- Auto-save after each message
- `/think` and `/tools` toggle commands
- `/tools-output` for controlling tool verbosity
- `/compact` for conversation summarization
- `/retry` (alias `/r`) for regenerating last response
- `/undo` for removing last response
- `/search` (alias `/find`, `/f`) for semantic search
- `/context` (alias `/ctx`) for token metrics
- Tab completion for commands and models
- Mode indicators in prompt (`[t]`, `[T]`)
- Token metrics display
- Thinking output visible when enabled
- Error recovery for tool/network errors

**Tools (28 total):**

| Category | Count | Feature Flag | Default |
|----------|-------|--------------|---------|
| Pokémon | 9 | `pokemon-tools` | ✅ Enabled |
| Weather | 3 | `weather-tools` | ✅ Enabled |
| File Operations | 5 | `file-tools` | ✅ Enabled |
| Calculator | 1 | `calc-tools` | ✅ Enabled |
| Web Search (Serper) | 2 | `serper-tools` | ✅ Enabled |
| Web Search (DDG) | 3 | `search-tools` | ❌ Disabled |
| System | 2 | `system-tools` | ✅ Enabled |
| LED Control | 5 | `led-tools` | ❌ Disabled* |

*LED tools require `[led]` configuration in config.toml.

**System Tools:**
- `get_current_datetime` - Date, time, timezone, ISO 8601, Unix timestamp
- `get_project_context` - Languages, git info, stack detection, key files

**Translation:**
- 50+ languages via translategemma model

**OCR:**
- Text, tables, formulas, figures via glm-ocr model
- Specialized for text extraction (separate from vision module)

**Documentation:**
- Man page
- mdBook documentation
- AGENTS.md integration

**Build & Distribution:**
- Linux x86_64 builds (default and all-tools)
- Termux/Android builds (aarch64-linux-android)
- GitHub Releases automation

---

## Known Issues

### GLM-OCR Returns Empty Output ✅

**Status:** Fixed in Ollama v0.17.6 (2026-03-04)

GLM-OCR model returned empty markdown after Ollama v0.17.1 due to incorrect prompt rendering.

**Resolved:** Ollama v0.17.6 includes the fix: "Fixed issue where GLM-OCR would not work due to incorrect prompt rendering"

**Note:** Users on rolling-release distros (e.g., Arch Linux) may need to wait for package updates. For immediate fix, use the official install script:
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### Semantic Retrieval Context Framing ✅

**Status:** Completed (released in v0.22.9)

~The LLM doesn't understand what `<retrieved_context>` represents.~ Fixed with:
1. Framing text explaining messages are from conversation history
2. MEMORY section in system prompt (conditional)
3. Explicit instructions for smaller models

**See:** `doc/src/development/v0.22.9_plan.md`

---

## Critical (Bugs & Hotfixes)

**Status:** Under Analysis

These items represent critical bugs that must be fixed before any new features.

### Context Token Count Mismatch

**Status:** ✅ FIXED (v0.26.2)

**Problem:** The token count shown after each Ollama response (`prompt_eval_count`) differs significantly from `/context` command output.

**Root Cause:**
1. Retrieval context not included in `/context` calculation (~200 tokens)
2. Tool definitions estimated at 20 tokens (should be ~50)
3. Using estimated tokens instead of real Ollama counts

**Fix Applied (v0.26.2):**
- Added `prompt_tokens: Option<u64>` field to `SavedMessage`
- Store `prompt_eval_count` from Ollama after each response
- Use real tokens in `/context` when available, fallback to estimate
- Fixed retrieval inclusion in context calculation
- Increased tools_tokens estimate to 50 per tool

**Files Changed:**
- `src/chat/session.rs` - Added prompt_tokens field
- `src/chat/repl.rs` - Pass real tokens to context calculation
- `src/tokens.rs` - Added real_history_tokens parameter

**Verification Needed:** Test in production to confirm alignment.

---

### Context Builder Panic After /compact

**Status:** ⚠️ Needs Reproduction

**Problem:**
```
thread 'main' panicked at src/retrieval/context_builder.rs:318:51:
range start index 2 out of range for slice of length 1
```

**Trigger:** Occurs after running `/compact` command manually.

**Code Analysis (`src/context_overflow.rs`):**
- Edge case handling appears CORRECT (returns None when ≤10 messages)
- Range calculation verified correct for all edge cases

**Possible Causes:**
1. Race condition during /compact execution
2. Session modified between CompactionSuggestion creation and use
3. Message count changed mid-execution

**See:** `src/context_overflow.rs:get_compaction_range_default()`

---

### /undo Incomplete Cleanup

**Status:** ❌ BUG CONFIRMED

**Problem:** `/undo` removes messages from memory but does NOT delete embeddings from database.

**Code Location:** `src/chat/repl.rs:572-588`

**Current Behavior:**
```rust
let removed = session.remove_last_assistant_messages();
// Only removes from Vec<SavedMessage>, NO database cleanup
```

**Impact:** Orphaned embeddings in SQLite database.

**Fix Required:**
1. Add function to delete embeddings for specific messages
2. Call delete before removing from memory

**See:** `src/chat/session.rs:remove_last_assistant_messages()`

---

### User Prompt Included in Hybrid Search

**Status:** ❌ BUG CONFIRMED

**Problem:** The most recent user message is always included in hybrid search queries.

**Code Location:** `src/db/operations.rs:488-513`

**Root Cause:** `search_hybrid()` has no parameter to exclude specific message IDs.

```rust
pub fn search_hybrid(
    &self,
    query: &str,
    embedding: &[f32],
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    limit: usize,
    keyword_weight: f32,
    semantic_weight: f32,
) -> Result<Vec<SearchResult>> {
    // No exclude_ids parameter!
}
```

**Impact:** 
- Wasted search tokens (current prompt shouldn't be searched)
- Skewed relevance rankings (current prompt biases results)

**Fix Required:**
1. Add `exclude_message_ids: Option<Vec<i64>>` parameter to `search_hybrid()`
2. Modify SQL queries to exclude these IDs
3. Pass last message ID when calling search

**See:** `src/retrieval/context_builder.rs:222` (caller)

---

### Code Mode (-c Flag) Not Working in Chat

**Status:** ❌ BUG CONFIRMED

**Problem:** The `-c` (code mode) parameter works in `ask query` but NOT in `ask chat`.

**Root Cause:** `cli.code` is NOT passed to `run_chat_repl()`.

**Code Location:** `src/main.rs:468`

```rust
// Current (broken):
chat::run_chat_repl(settings, &args, cli.model.as_deref(), cli.think, cli.tools, cli.ignore_agents).await

// Missing: cli.code parameter
```

**Fix Required:**
1. Add `cli_code: bool` parameter to `run_chat_repl()` signature
2. Pass `cli.code` from `handle_chat()`
3. Use code configuration

**See:** `src/chat/repl.rs:38` (function signature)

---

### Context Builder Panic After /compact + /clear

**Status:** ✅ FIXED (this commit)

**Problem:** After `/compact` followed by `/clear`, the session crashes with:
```
range end index 5 out of range for slice of length 2
```

**Root Cause:** `clear_messages()` preserved `compacted_range` which contains indices into `messages`. After clear, `messages` is empty but `compacted_range` still references old indices.

**Fix Applied:**
1. Reset `compacted_range` in `clear_messages()`
2. Add bounds checking with `.min(session.messages.len())` in `context_builder.rs`

**Files Changed:**
- `src/chat/session.rs` - Reset `compacted_range` on clear
- `src/retrieval/context_builder.rs` - Clamp indices to message count

---

### Premature Message Saving

**Status:** ✅ ALREADY FIXED

**Problem:** When user sends a message, it's only saved after the LLM responds. If the process is interrupted (Ctrl+C), the user message is lost.

**Current Behavior (verified in code):**
```rust
// src/chat/repl.rs:721-727
// Save user message immediately before sending
session.add_user_message(line.to_string());
if !session.anonymous
    && let Err(e) = session.save(&storage)
{
    // handle error
}
```

**Verdict:** User messages are already saved immediately before sending to LLM. This bug was likely based on an older version.

---

### Legacy "conversations" Folder

**Status:** ⚠️ Needs Investigation

**Problem:** The `conversations/` folder still contains JSON files even though SQLite is now the primary storage.

**Questions:**
1. Is there still code writing to conversations folder?
2. Is it just reading for legacy migration?
3. Should we remove the folder entirely?

**Investigation Required:**
- [ ] Check for any `conversations/` write operations
- [ ] Determine if folder should be deleted or kept for backward compatibility

---

## Context Overflow & Compaction Issues (bugs2.md)

**Status:** Under Analysis

These issues relate to context management during tool calls and compaction.

### Context Exhaustion During Tool Calls

**Status:** 🟡 IN PROGRESS (Phase 1 Complete)

**Problem:** Context can be exhausted during a chain of tool calls, leaving no room for the final response.

**Root Cause:**
- Messages accumulate in coordinator history during tool execution without size checks
- `auto_compact_if_needed()` only runs AFTER complete response
- Large tool results have no size limits
- No context check in `process_next()` before sending to Ollama

**Implementation Plan:**

#### Phase 1: Token Estimation in Coordinator ✅ (v0.26.4)

**Completed:**
- ✅ Added `estimate_messages_tokens()` for SavedMessage
- ✅ Added `estimate_chat_messages_tokens()` for ChatMessage
- ✅ Added `context_window` and `system_prompt` fields to `CustomCoordinator`
- ✅ Added `context_window()` and `system_prompt()` builder methods
- ✅ Added context check in `process_next()` (90% threshold)
- ✅ Unit tests for token estimation
- ✅ Error returned when overflow detected during tool execution

**Files Changed:**
- `src/context_overflow.rs` - New estimation functions
- `src/chat/custom_coordinator.rs` - Context fields and check
- `src/query.rs` - Pass context to coordinator
- `src/chat/repl.rs` - Pass context to coordinator

#### Phase 2: Unicode-Safe Tool Result Truncation ✅ (v0.26.4)

**Completed:**
- ✅ Added `truncate_tool_result()` function (Unicode-safe using `.chars()`)
- ✅ Added `MAX_TOOL_RESULT_TOKENS` constant (4000 tokens)
- ✅ Added `CHARS_PER_TOKEN` constant (4 chars/token conservative)
- ✅ Applied truncation in `custom_coordinator.rs` before pushing to history
- ✅ Debug logging when truncation occurs
- ✅ Unit tests for truncation (empty, short, long, Unicode, at-limit)

**Files Changed:**
- `src/context_overflow.rs` - `truncate_tool_result()` function
- `src/chat/custom_coordinator.rs` - Apply truncation to tool results

#### Phase 3: Pre-Tool Context Check ✅ (v0.26.4)

**Completed:**
- ✅ Added `PRE_TOOL_THRESHOLD` constant (75%)
- ✅ Added `needs_pre_tool_compaction()` function
- ✅ Added `MIN_PRESERVE_LAST` constant for turn preservation
- ✅ Check context before creating coordinator in `run_chat_repl()`
- ✅ Auto-compact at 75% threshold before tool execution
- ✅ User message preserved during compaction (already saved before check)

**Files Changed:**
- `src/context_overflow.rs` - New function and threshold
- `src/chat/repl.rs` - Pre-tool check before send_message()

### Compaction Threshold Behavior

**Status:** Planned

- Check context BEFORE creating coordinator
- Auto-compact at 75% threshold if needed
- Preserve current turn during compaction

#### Phase 4: Turn Preservation in Compaction

**Status:** Planned

- Modify `get_compaction_range()` to preserve last N messages
- Never compact user message + assistant + tool chain

#### Phase 5: During-Tool Context Check

**Status:** Planned

- Already implemented in Phase 1 (context check in `process_next()`)
- Returns error when overflow detected

#### Phase 6: Error Recovery

**Status:** Planned

- Detect overflow error from coordinator
- Auto-compact immediately
- Retry once with compacted context

#### Phase 7: Testing

**Status:** Planned

- Test large tool result truncation
- Test context overflow during tool execution
- Test turn preservation
- Test Unicode-safe truncation
- Test multi-tool chain near context limit

**See:** `src/context_overflow.rs`, `src/chat/custom_coordinator.rs`

### Compaction Threshold Behavior

**Status:** Under Analysis

**Problem:** When context threshold is reached:
- User message should be saved temporarily
- Compaction should run
- Then normal flow continues with saved message
- No visual indication of compaction happening

**Proposed:**
- Add visual indicator ("Compacting context...")
- Save user message before compaction
- Restore after compaction

### Context Not Cleared After /compact

**Status:** Under Analysis

**Problem:** After `/compact`, context remains in overflow state. `/clear` is needed.

**Possible Causes:**
1. Compaction not reducing context enough
2. Summary itself is too large
3. Recent messages preserved are still too many

### Markdown in Compaction Summary

**Status:** Under Analysis

**Problem:** Context compaction summary does not produce markdown output.

**Fix Required:** Ensure LLM generates markdown for summaries.

### Web Scraping Content Quality

**Status:** Under Analysis

**Problem:** Web fetch tool sometimes returns raw HTML/CSS instead of clean markdown, polluting context.

**Proposed:** Review and improve HTML-to-markdown conversion in web tools.

---

## High Priority

### Memory Enhancement Part 1 (Phases 1-3)

**Priority:** HIGH  
**Status:** Phase 1 Complete, Phase 2 Research Needed

**Goal:** Improve memory/RAG system for better context retrieval and source attribution.

This is a multi-phase enhancement to our RAG capabilities, broken into small deliverables that can be implemented incrementally.

#### Phase 1: Source Attribution ✅

**Status:** Completed (released in v0.26.1)

**Goal:** LLM should cite sources in responses.

**Implemented:**
- ✅ `SourceType` enum in `src/db/operations.rs` (Conversation, Document, Note, Web)
- ✅ `source_type` field in `SearchResult` struct
- ✅ `SourceType::prefix()` method for ID prefixes (msg, doc, note, web)
- ✅ Context formatted with source labels: `[msg:N]`, `[doc:N]`, `[note:N]`
- ✅ Example citations in system prompt: "As we discussed [msg:42]"
- ✅ `remember` tool updated to use source type prefixes

**Implementation Details:**
- `src/db/operations.rs` - `SourceType` enum with `prefix()` and `from_prefix()` methods
- `src/retrieval/context_builder.rs` - `format_retrieved_context()` with source attribution
- `src/tools/remember.rs` - Source ID parsing with `parse_source_id()`

#### Phase 2: Query Routing Research (Research)

**Goal:** Determine best approach for routing queries to appropriate search targets.

**Problem:** Different queries should search different sources:
- "lembra o que eu falei?" → Memory (conversations)
- "o que está no PDF?" → Documents
- "como está o tempo?" → None (skip search)

**Research Questions:**
- Regex-based routing (fast, language-specific patterns)
- Embedding-based intent matching (language-agnostic, ~30ms overhead)
- Hybrid approach (regex fallback to embedding)

**Multilingual Concerns:**
- Regex requires patterns per language (pt-BR, en, es, fr, de)
- Embedding approach is language-agnostic but has latency
- May integrate with chat modules (OCR/Translation) in future

**Tasks:**
- [ ] Collect real query patterns from usage (analyze logs)
- [ ] Prototype regex routing (pt-BR + en)
- [ ] Benchmark embedding-based routing latency
- [ ] Test `whatlang` crate for language detection
- [ ] Compare accuracy vs latency trade-offs

**See:** `doc/src/development/research-query-routing.md`

#### Phase 3: Timestamp Filtering (1 day)

**Goal:** Filter results by time ("what did I say yesterday?").

**Implementation:**
- Extract temporal references from query
- Convert to timestamp range ("ontem" → yesterday's range)
- Add timestamp filter to `search_hybrid()`

**Tasks:**
- [ ] Add `timestamp_range: Option<(i64, i64)>` to `search_hybrid()`
- [ ] Implement temporal reference detection (pt-BR + en)
- [ ] Add SQL WHERE clause for timestamp filtering
- [ ] Test: "o que eu falei ontem" returns only yesterday's messages

**See:** `src/db/operations.rs`

---

### Chat Module Integration

**Priority:** HIGH  
**Status:** Planning needed

**Problem:** Users must exit chat to use OCR, Vision, Translate, Summarize features.

**Proposed Features:**
- `/ocr <image>` - Run OCR from chat
- `/vision <image>` - Analyze image
- `/translate <lang> <text>` - Translate
- `/summarize [text]` - Summarize

**Context Integration:**
- Module outputs should be contextualized
- Model should understand extracted text as conversation context

**Why High Priority:**
- Required dependency for Memory Enhancement Part 2 (document ingestion)
- OCR/Vision needed to process scanned documents and images
- Significantly improves user workflow (no need to exit chat)

**Streaming Consideration:**

Currently, chat uses non-streaming `send_chat_messages()`. To add streaming:

```rust
// Current approach (non-streaming)
let response = coordinator.chat(messages).await?;

// Streaming approach (future)
let mut stream = ollama.send_chat_messages_stream(request).await?;
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    
    // Real-time content
    print!("{}", chunk.message.content);
    
    // Thinking for reasoning models (DeepSeek R1, etc.)
    if let Some(thinking) = &chunk.message.thinking {
        // Display thinking separately
    }
}
```

**Tasks:**
- [ ] Design: Command interface
- [ ] Design: Model switching during commands
- [ ] Implement: `/ocr` command
- [ ] Implement: `/vision` command
- [ ] Implement: `/translate` command
- [ ] Document: Chat module commands

---

### Memory Enhancement Part 2 (Phases 4-5)

**Priority:** HIGH  
**Status:** Blocked by Chat Module Integration

**Goal:** Extend memory system to support documents and notes.

**Dependencies:**
- Chat Module Integration (for OCR/Vision to process scanned documents)
- Memory Enhancement Part 1 (source attribution foundation)

#### Phase 4: Schema Preparation (1-2 days)

**Goal:** Prepare schema for multiple source types (conversations, documents, notes).

**Implementation:**
- Create separate `documents` table (not in messages)
- Add `source_type` to retrieval logic
- Update embedding tables for multi-source search

**Schema Design:**
```sql
CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    source_type TEXT,  -- 'pdf', 'markdown', 'text', 'web'
    title TEXT,
    content TEXT,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE VIRTUAL TABLE document_embeddings USING vec0(
    embedding FLOAT[256],
    +document_id TEXT,
    +source_type TEXT,
    +created_at INTEGER
);
```

**Tasks:**
- [ ] Design `documents` table schema
- [ ] Create `document_embeddings` virtual table
- [ ] Update `SearchTarget` enum
- [ ] Add `source_type` filter to hybrid search
- [ ] Test: Search returns results from correct source

**See:** `src/db/schema.rs`

#### Phase 5: Document Ingestion (Future)

**Goal:** Ingest and index external documents (PDFs, markdown, text files).

**Dependencies:**
- Phase 4 (schema preparation)
- Chat module integration (for OCR/Vision)

**Tasks:**
- [ ] Research: PDF parsing crates (pdf-extract, lopdf)
- [ ] Implement: Document chunking with overlap
- [ ] Implement: Embedding generation for chunks
- [ ] Implement: `/ingest <path>` command
- [ ] Integrate: OCR from chat for scanned documents

---

### Conversation-Aware Retrieval ✅

**Priority:** HIGH  
**Status:** Completed (released in v0.24.0)

**Problem:** When searching conversation history, short user questions are retrieved with high
similarity but contain no information. Long assistant responses with the actual information
have lower similarity due to semantic dispersion.

**Symptoms:**
- remember(query) returns only questions, not answers
- Context shows "What about Wittgenstein?" but not the response about Wittgenstein
- LLM cannot access the information it previously provided

**Root Cause:** Short messages have concentrated similarity (high score), long messages
have dispersed similarity (low score). RRF doesn't account for message role.

**Solution:** Post-retrieval enrichment - attach assistant responses to user questions.

**Released:** v0.24.0 (2026-03-03)

**See:** `doc/src/development/v0.24.0_plan.md`

---

**Priority:** HIGH  
**Status:** Completed (released in v0.23.0)

**Problem:** Even with v0.22.9 context framing, GLM-5:cloud still responds "I have no memory of previous conversations." The LLM:
1. Doesn't know HOW to retrieve MORE context (only receives 5 messages)
2. Doesn't understand HOW to reference retrieved messages
3. Can't search for topics NOT in the last query

**Solution:**
1. `remember(id)` tool to retrieve full message by ID
2. `remember(query)` tool to search by topic
3. Include message IDs in retrieved context
4. Enable retrieval by default
5. Clear MEMORY TOOLS section in prompt

**Released:** v0.23.0 (2026-03-03)

**See:** `doc/src/development/v0.23.0_plan.md`

---

### Token Counting & Context Metrics ✅

**Priority:** HIGH  
**Status:** Completed (will be released in v0.19.0)

**Rationale:** Foundation for all context management. Without token visibility, we can't measure or optimize.

**Problem:** No visibility into token usage per session. Users can't optimize context usage.

**Implementation:**
```rust
fn count_messages_tokens(messages: &[ChatMessage]) -> usize {
    // Word-based estimation: ~0.75 words/token
    // Message overhead: ~4 tokens/message
    // System prompt + tools + history
}
```

**Tasks:**
- [x] Implement: Token counting utility (`src/tokens.rs`)
- [x] Add: Token metrics to chat sessions (`ContextMetrics` struct)
- [x] Create: `/context` command for session info
- [x] Display: Tokens per message type (system, tools, history)
- [x] Document: `/context` command in doc/src/commands/context.md

---

### To-Do List Tooling ✅

**Priority:** HIGH  
**Status:** Completed (will be released in v0.19.0)

**Rationale:** State Management is the most impactful context reduction. Explicit task tracking eliminates the need to search through history.

**Problem:** LLMs waste context searching through conversation history to track progress on multi-step tasks. An explicit to-do list reduces this need.

**Implemented Features:**
- `todo_add(description)` - Add a new task to the list
- `todo_update(task_id, status)` - Update status (pending/in_progress/done)
- `todo_list()` - List all tasks with current status
- `todo_clear_done()` - Remove completed tasks
- `todo_clear_all()` - Clear all tasks

**Session Types:**
- **Query mode:** Ephemeral, in-memory list via global state
- **Chat mode:** Persistent, stored with session in `todos` field

**Implementation:**
- `src/chat/todo_state.rs` - TodoState, Task, TaskStatus structs
- `src/tools/todo.rs` - 5 todo tools
- `src/prompts/tools.rs` - TODO TOOLS section in system prompt
- Enabled via `todo-tools` feature flag (default: enabled)

**Tasks:**
- [x] Research: LLM task management patterns
- [x] Design: Tool interface and state storage
- [x] Implement: To-do list tools in `src/tools/todo.rs`
- [x] Integrate: Include state in system prompt
- [x] Test: Unit tests for todo_state

---

### Context Management v2 - Semantic Retrieval ✅

**Priority:** HIGH  
**Status:** Completed (released in v0.20.0)

**Goal:** Enable semantic retrieval of conversation history for intelligent context selection.

**Architecture Decisions:**

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Vector Extension | **sqlite-vec** | sqlite-vss archived, sqlite-vec active |
| Dimensions | **256 (Matryoshka)** | 3x storage savings, ~2-3% quality loss |
| Retrieval | **Hybrid (BM25 + Semantic)** | RRF fusion, weights: 0.4 keyword / 0.6 semantic |
| Threshold | **0.7 cosine similarity** | Balance precision/recall |

**Model:** nomic-embed-text-v2-moe (958MB, 768 → 256d, multilingual)

**Storage Estimate:** ~20-30 MB for 10,000 messages

**Implemented:**
- [x] Create `src/db/` module with sqlite-vec integration
- [x] Create `src/embeddings/` module with Ollama API
- [x] Create `src/retrieval/` module with hybrid search (RRF)
- [x] Add `/search <query>` command
- [x] FTS5 query sanitization for SQL injection protection
- [x] Embedding dimension validation (768 → 256)

**Released:** v0.20.0 (2026-03-02)

**Dependencies:**
```toml
rusqlite = { version = "0.32", features = ["bundled"] }
sqlite-vec = "0.1"
zerocopy = "0.8"
```

**Detailed Plan:** `doc/src/development/context_v2_plan.md`

---

### Project-Aware Query Mode ✅

**Priority:** HIGH  
**Status:** Completed (released in v0.25.0)

**Problem:** Query mode had no access to conversation history. When running
`ask query "What did we discuss?"`, it responded without context from previous
chats in that project.

**Solution:** Enabled retrieval from project's conversation history, using same
RAG system as chat mode, but without persisting new messages.

**Implementation:**
- `build_query_context()` for ephemeral context (`src/retrieval/context_builder.rs`)
- `project_id` parameter in `search_hybrid()`
- DB + EmbeddingClient initialization in query mode (except `--code`)
- Task-local context for `remember()` tool support
- Graceful degradation if DB unavailable

**Released:** v0.25.0 (2026-03-04)

**See:** `doc/src/development/implementation-history.md`

---

### Automatic Middle Compaction ✅

**Priority:** Medium  
**Status:** Completed (released in v0.22.3)

**Goal:** Automatically compact middle messages when approaching context limit.

**Implementation:**
- `CompactionSuggestion` struct with `keep_first`, `keep_last`, `middle_indices`
- `get_compaction_range_default()` - calculates middle range to compact
- Auto-compact trigger at 72% (warning) and 80% (overflow)
- Preserves first N + last N messages, summarizes middle
- Visual context utilization bar in `/context`

**Released:** v0.22.2 (middle compaction), v0.22.3 (auto-trigger)

**See:** `src/context_overflow.rs`, `src/chat/repl.rs:1182-1234`

---

### SOUL.md Support (Personality System)

**Priority:** HIGH  
**Status:** Research Needed

**Goal:** Support for `SOUL.md` file to define LLM personality and modus operandi.

**Research Required:**
1. What is SOUL.md? Structure and format used by other agents
2. How to handle comments in the file (likely should be ignored)
3. Where is it injected in the system prompt?
4. Impact on context usage
5. Can it replace parts of existing chat/query prompts?

**Proposed Implementation:**
1. Create template in config folder: `~/.config/ask-ai/SOUL.md`
2. Add config option: `soul_file = "path/to/SOUL.md"` (default: config folder)
3. Inject content after personality prefix
4. Deprecate Pepe personality (move to `SOUL-pepe.md`)

**Tasks:**
- [ ] Research: SOUL.md format in other agents
- [ ] Research: Best injection point in system prompt
- [ ] Design: Config option for custom SOUL.md path
- [ ] Implement: SOUL.md file loading
- [ ] Implement: Inject into system prompt
- [ ] Deprecate: Pepe personality (optional, move to SOUL-pepe.md)

---

### Smart Model List (Installed Only)

**Priority:** MEDIUM  
**Status:** Proposed

**Goal:** Show only installed/available models in listings and validate model availability on startup.

**Current Problem:**
- `ask-ai -l` shows all configured models, not just installed ones
- No validation when switching models
- May fail with network errors during chat

**Proposed Behavior:**
1. List command: Filter to only show models found in `ollama list`
2. Model switch: Validate model exists before switching
3. Chat startup (no model specified): 
   - If default not available, try fallback chain
   - If no models available, show error but allow commands only
4. Mode indicators show "[none]" when no model loaded

**Example:**
```
$ ask chat -m hermes4:14b
Warning: hermes4:14b not installed. Falling back to qwen3.5:9b.
Warning: qwen3.5:9b not installed and is default. Commands only - no model.

none> /model qwen3.5:27b
Model loaded.

qwen3.5:27b[t][T]>
```

**Tasks:**
- [ ] Implement: `is_model_installed(model_id)` function
- [ ] Modify: `/list` command to filter installed models
- [ ] Modify: Model switch to validate before loading
- [ ] Modify: Chat startup with fallback chain
- [ ] Add: "[none]" mode indicator

---

## Medium Priority

### File Session State

**Priority:** Medium  
**Status:** Research needed

**Goal:** Explicit tracking of file operations for context reduction and security.

```rust
struct FileSessionState {
    read_files: HashSet<PathBuf>,
    edited_files: HashMap<PathBuf, FileEditLog>,
    created_files: HashSet<PathBuf>,
    removed_files: HashSet<PathBuf>,
}
```

**Security Constraints:**
- Create: Only files that don't exist
- Edit: Only files read in session
- Remove: Only files read in full
- Detect: External modifications

**Tasks:**
- [ ] Research: File tracking patterns
- [ ] Design: Session state structure
- [ ] Implement: File tracking in session
- [ ] Implement: Security constraints

---

### System Tools - run_command

**Priority:** Medium  
**Status:** Blocked by security concerns

- `run_command` - Execute commands with configurable whitelist
- Requires: Robust error handling, security assessment

**Tasks:**
- [ ] Research: Secure command execution
- [ ] Design: Whitelist configuration
- [ ] Implement: Security constraints

---

### SQL ORM Evaluation

**Priority:** MEDIUM  
**Status:** Research Needed

**Goal:** Evaluate migration from raw SQL to ORM for type safety and maintainability.

**Concerns:**
- Current raw SQL may have dialect issues across SQLite versions
- ORM would provide type safety for queries
- But: May increase binary size
- But: May complicate FTS5 search (need raw SQL for performance)

**Proposed Approach:**
- Keep raw SQL for FTS5 and complex queries (performance critical)
- Use ORM (e.g., `sqlx` or `sea-orm`) for:
  - Conversation retrieval
  - Message CRUD
  - Session management

**Tasks:**
- [ ] Research: `sqlx` vs `sea-orm` trade-offs
- [ ] Benchmark: Binary size impact
- [ ] Prototype: ORM for simple queries
- [ ] Migrate: Non-critical SQL to ORM

---

### Skills System

**Priority:** Medium  
**Status:** Research needed

Load custom behaviors from `.ask-ai/skills/` or `~/.config/ask-ai/skills/`.

**Example:**
```markdown
---
name: code-review
description: Review code for quality
tags: [code, review]
---

When reviewing code, focus on:
- Readability and maintainability
- Potential bugs
- Security vulnerabilities
```

**Tasks:**
- [ ] Research: Skill systems in other agents
- [ ] Design: Skill file format
- [ ] Implement: Skill parser
- [ ] Implement: `--skill` flag

---

### Remember Tool for Conversation History ✅

**Priority:** Low  
**Status:** Completed (released in v0.23.0)

Allow LLM to explicitly recall topics from conversation history via tool call.

**Implemented Tools:**
- `remember(id="42")` - Get specific message by ID
- `remember(query="topic")` - Search history by topic
- `remember(query="topic", limit="10")` - Search with limit

**See:** `doc/src/development/implementation-history.md` (Remember Tool section)

---

## Low Priority

### OCR Model Customization

**Priority:** Low  
**Status:** Blocked by Arch Linux package availability

Fixed in Ollama v0.17.6, but Arch Linux repositories may not have the update yet.
Users can install via official script for immediate fix:
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

**Goal:** Allow users to specify custom OCR model via `-m` flag.

**Tasks:**
- [ ] Wait: Arch Linux package update to v0.17.6+
- [ ] Implement: `-m` flag for `ask ocr` command
- [ ] Research: Alternative OCR models (glm-ocr alternatives)
- [ ] Test: GLM-OCR with various image types
- [ ] Document: Custom OCR model configuration

---

### Plugin System

**Priority:** Low  
**Status:** Not started

User-defined tools via dynamic loading or compilation.

---

### OpenAPI Compatibility (Direct API Access)

**Priority:** LOW  
**Status:** Research Needed

**Goal:** Support direct interaction with OpenAI-compatible APIs (not just Ollama).

**Rationale:**
- Ollama is convenient but limited in flexibility
- LM Studio offers more parameter control
- OpenAI API provides cloud alternatives
- Remove dependency on Ollama-specific features

**Research Required:**
- Identify OpenAI-compatible endpoints needed
- Evaluate `openai` crate vs custom HTTP client
- Determine config changes needed
- Assess impact on tool integration

**Proposed Implementation:**
1. Add config option: `provider = "ollama" | "openai" | "lm-studio"`
2. Create trait for LLM providers
3. Implement Ollama as default (current behavior)
4. Add OpenAI/LM Studio provider implementations

**Tasks:**
- [ ] Research: Required API endpoints
- [ ] Design: Provider trait/interface
- [ ] Implement: OpenAI provider
- [ ] Implement: LM Studio provider
- [ ] Config: Add provider option

---

### TUI (Terminal User Interface)

**Priority:** Low  
**Status:** Research & Planning needed

**Goal:** Build a responsive TUI using Ratatui-rs that works across:
- Desktop terminals (resizable)
- Termux on Android
- Various screen sizes

**Challenges:**
- Responsive layout for different terminal sizes
- Markdown rendering in TUI (tables, code blocks)
- Streaming output without flickering
- Keyboard navigation and shortcuts
- Accessibility (screen readers)
- Touch support (Termux on-screen keyboard)

**Research Topics:**
- [ ] Ratatui-rs capabilities and limitations
- [ ] Terminal resize handling patterns
- [ ] Markdown-to-TUI rendering approaches
- [ ] Existing TUI chat interfaces for inspiration
- [ ] UX patterns for CLI-to-TUI transition

**Design Questions:**
- Single-pane vs multi-pane layout?
- How to handle tools output in TUI?
- Split view for thinking mode?
- Theme/skin system?

**Dependencies:**
- Chat module integration (for inline OCR/vision)
- Memory enhancement (for better context display)

**Tasks:**
- [ ] Research: Ratatui-rs best practices
- [ ] Research: Terminal capability detection
- [ ] Design: UX wireframes for main views
- [ ] Design: Responsive layout system
- [ ] Prototype: Basic TUI skeleton
- [ ] Test: Termux compatibility
- [ ] Document: TUI user guide

**Streaming Considerations:**

The `ollama-rs` library (already included with `stream` feature) supports streaming chat completions:

```rust
// Streaming API from ollama-rs
pub async fn send_chat_messages_stream(
    &self,
    request: ChatMessageRequest,
) -> Result<ChatMessageResponseStream>

// ChatMessage includes thinking field for reasoning models
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,  // DeepSeek R1, etc.
    pub tool_calls: Vec<ToolCall>,
    pub images: Option<Vec<Image>>,
}

// Consuming the stream
while let Some(chunk_result) = stream.next().await {
    let chunk = chunk_result?;
    // Incremental content
    print!("{}", chunk.message.content);
    // Thinking (if available)
    if let Some(thinking) = &chunk.message.thinking {
        // Display thinking in real-time
    }
}
```

**Implementation Approach for Streaming:**

Option A: Accumulate and render blocks
- Buffer incoming content
- Render when block delimiter received (double newline)
- Latency between blocks visible to user

Option B: Plain text during stream + markdown at end
- Stream raw text for responsiveness
- Render full markdown when stream completes
- Less visually appealing during stream

Option C: TUI with incremental rendering (recommended for TUI mode)
- Use Ratatui's frame-based rendering
- Update buffer on each chunk
- Render complete frame efficiently
- Thinking pane separated from response pane

**Markdown Rendering in TUI:**

The `tui-markdown` crate solves markdown rendering for Ratatui:

```rust
use tui_markdown::from_str;

let markdown = r#"
# Heading
- List item 1
- List item 2

```rust
fn main() {
    println!("Hello, world!");
}
```
"#;

let text = from_str(markdown);  // Returns ratatui::text::Text
frame.render_widget(text, frame.area());
```

Features:
- Uses `pulldown-cmark` for markdown parsing
- Converts to `ratatui::text::Text` with styles
- Syntax highlighting via `highlight-code` feature (uses syntect)
- Works with incremental updates

**TUI Layout for Streaming:**

```
┌─────────────────────────────────────────────────┐
│ 💭 Thinking                                     │
│ [thinking content streams here in real-time]     │
│ [dimmed style for thinking]                     │
├─────────────────────────────────────────────────┤
│ 🤖 Assistant                                    │
│ [response content streams here]                  │
│ **bold**, `code`, lists rendered via            │
│ tui-markdown                                     │
│                                                  │
│ [scrolls as content grows]                      │
└─────────────────────────────────────────────────┘
```

**Key Advantage of TUI for Streaming:**
- Thinking pane separated from response pane
- Incremental updates per frame (no flickering)
- User can scroll back through history
- Interrupt with Ctrl+C or 'q' key
- State persists between messages

---

## Completed

### Context Management Research ✅

**Completed:** 2026-02-24

Full research document in `doc/src/development/context_management_research.md`.

### Prompt Refactoring ✅

**Completed:** 2026-02-27

- Modular structure (65% token reduction)
- ReAct-style examples
- Platform detection
- Documented in `doc/src/development/prompt-refactor.md`

### Tool Calling Improvements ✅

**Completed:** 2026-02-27

- CustomCoordinator with event-driven callbacks
- Pre-tool content forwarding
- Error recovery system
- Smaller model parameter tuning
- Documented in `TOOL_CALLING_RESEARCH.md`

### Model Switching Fix ✅

**Completed:** 2026-02-25

- Centralized model switching in `src/chat/model_switch.rs`
- CLI model override now has precedence
- Fixed state inconsistencies

---

## Completed (Historical)

### Vision Module ✅

**Completed:** 2026-02-23

- New `ask vision` command for image analysis
- Multi-image support
- Documentation in `doc/src/commands/vision.md`

### Custom Model Support ✅

**Completed:** 2026-02-19

- User-defined models in `~/.config/ask-ai/models.toml`
- Optional parameters: `top_k`, `top_p`, `repeat_penalty`
- Cloud model thinking support

### Termux Builds ✅

**Completed:** 2026-02-19

- Cross-compilation with `cross`
- aarch64-linux-android builds

**Priority:** Low  
**Status:** Research needed

**Challenges:**
- Markdown context dependency
- Tables require full content
- Cross-line formatting

**Potential solutions:**
1. Line-buffered rendering
2. Block-buffered rendering
3. Plain text streaming

**Tasks:**
- [ ] Research: Streaming markdown rendering approaches
- [ ] Prototype: Basic streaming with termimad

---

### Multilingual Injection Detection

**Priority:** Low  
**Status:** Not started

**Problem:** AGENTS.md sanitization only detects English injection patterns.

**Tasks:**
- [ ] Research: Injection patterns in non-English languages
- [ ] Implement: Multilingual pattern detection

---

## Future Tools

### Document Processing
- PDF text extraction
- Document format conversion
- Batch processing

### Data Analysis
- CSV/JSON analysis
- Statistical tools
- ASCII visualization

### Code Tools
- Repository analysis
- Code quality checks
- Documentation generation

---

## Completed

### Prompt Refactoring ✅

**Completed:** 2026-02-28 (v0.17.0)

- Created modular `src/prompts/` module with hierarchical structure
- Dynamic platform detection via `src/platform.rs` (Linux distros, Termux, macOS, Windows)
- 13 ReAct-style few-shot examples (replaced arrow notation)
- Removed all negative instructions (DO NOT, NEVER, etc.)
- Token count reduced from ~1700 to ~890 tokens (47% reduction)
- Benchmark tests in `tests/prompt_benchmark.rs` (10 passing)
- Created `src/lib.rs` for library module exports

### Vision Module ✅

**Completed:** 2026-02-23

- New `ask vision` command for image description and analysis
- Default model: moondream:1.8b (lightweight, 1.7GB)
- Multi-image support via Ollama API `images` array
- Modes: default (brief), --detailed (comprehensive), custom prompt
- JSON output for programmatic use
- Configuration via `[model.vision]` in config.toml
- Shared image validation utilities in `src/utils.rs`
- Documentation in `doc/src/commands/vision.md`

### Chat Improvements ✅

**Completed:** 2026-02-28 (v0.17.0)

- Fixed anonymous mode to truly not load/save history
- User messages saved immediately after sending (before LLM response)
- Added `/retry` command (alias `/r`) for regenerating last response
- Added `remove_last_assistant_messages()` and `get_last_user_message()` to ChatSession

### Termux Builds ✅

**Completed:** 2026-02-19

- Cross-compilation with `cross` working
- Termux builds for aarch64-linux-android available
- Installation documented in README-TERMUX.txt
- Makefile targets for building Termux binaries

### Custom Model Support ✅

**Completed:** 2026-02-19 (v0.14.0)

- Users can define models in `~/.config/ask-ai/models.toml`
- Override parameters for built-in models
- Optional parameters: `top_k`, `top_p`, `repeat_penalty`
- Cloud models can have `thinking = true` for thinking support
- Context size defaults to 32K for user-defined models

### Thinking Output ✅

**Completed:** 2026-02-20 (v0.14.1)

- Uses API-provided `thinking` field from Ollama
- Falls back to regex extraction if API doesn't provide
- Works with cloud models via `thinking = true` config

### Error Recovery ✅

**Completed:** 2026-02-22

- Tools return `Ok(String)` with error message (not `Err`)
- Model sees tool errors and can react/retry
- Error classification helpers in `coordinator.rs`
- Maximum 3 retry attempts for recoverable errors

### Code Redundancy Refactoring ✅

**Completed:** 2026-02-23

- Created `src/query.rs` module with shared query execution logic
- Unified `handle_query()` and `handle_legacy_query()` into single `run_query()` function
- Created `ChatContext` builder for coordinator with event callbacks
- Created `OutputFlags` helper for debug/plain flag resolution
- Centralized event handling with `handle_chat_event()`
- Reduced `main.rs` from 1175 to 572 lines (51% reduction)
- Fixed chat mode CLI flags (`-m`, `-t`, `--tools`, `--ignore-agents`)
- Total reduction: ~600 lines of duplicated code eliminated

### Vision Module ✅

**Completed:** 2026-02-23

- New `ask vision` command for image description and analysis
- Default model: moondream:1.8b (lightweight, 1.7GB)
- Multi-image support via Ollama API `images` array
- Modes: default (brief), --detailed (comprehensive), custom prompt
- JSON output for programmatic use
- Configuration via `[model.vision]` in config.toml
- Shared image validation utilities in `src/utils.rs`
- Documentation in `doc/src/commands/vision.md`

---

## Testing

### Test Coverage
- [ ] Unit tests for all commands
- [ ] Integration tests with mock Ollama
- [ ] Tool testing framework
- [ ] OCR/Translation testing

### CI/CD
- [x] GitHub Actions for testing
- [x] Release automation
- [ ] Documentation deployment

---

## Contributing

See something you want to work on?

1. Check [GitHub Issues](https://github.com/luksamuk/ask-ai-rs/issues)
2. Comment on the issue
3. Submit a pull request

## Feedback

Your feedback shapes the roadmap!

- Open an issue for feature requests
- Vote on existing issues
- Join discussions

## See Also

- [Architecture](./architecture.md) - Technical architecture
- [Contributing](./contributing.md) - How to contribute
- [CHANGELOG](../CHANGELOG.md) - Version history