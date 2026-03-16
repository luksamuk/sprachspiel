# Implementation Plan for ask-ai

**Note**: This document tracks implementation status. For strategic direction, see:

## Quick Links

### Canonical Documents

| Document | Purpose |
|----------|---------|
| **[Implementation Directive](./doc/src/development/implementation-directive.md)** | Definitive direction for continuous learning feature |
| [Architecture](./doc/src/development/architecture.md) | Design decisions and system architecture |
| [Roadmap](./doc/src/development/roadmap.md) | Current development status and future plans |

### Reference Documents

| Document | Description |
|----------|-------------|
| [Skills System Design](./doc/src/development/skills-system-design.md) | Skills architecture |
| [Research Synthesis](./doc/src/development/research/research-appendix.md) | Complete research synthesis |
| [Papers Reference](./doc/src/development/research/papers-reference.md) | arXiv links for MemOS, OpenClaw-RL, MemGPT |

### External

- [GitHub Project Board](https://github.com/luksamuk?tab=projects) - Kanban board for task tracking

## Current Version

**v0.33.0** - 2026-03-15

## Current Implementation Status

✅ **Completed:**

- Core CLI with 5 subcommands (query, chat, translate, ocr, summarize)
- Interactive chat mode with persistent sessions
- Custom models support via `~/.config/ask-ai/models.toml`
- Built-in models: llama3.1, translategemma, glm-ocr (user models in config)
- Thinking support for cloud models (configurable via `thinking = true`)
- Dynamic model selection with capability detection
- Tool integration with error recovery (31 tools in 9 categories)
- Translation (50+ languages)
- OCR with multiple modes
- Summarization with styles
- Vision analysis
- Markdown rendering
- Pipe support
- Debug mode, Think mode, Code mode
- Token metrics display (`/context`)
- Context management foundation
- Semantic search (`/search`) with hybrid retrieval (BM25 + vector + RRF)
- SQLite storage with sqlite-vec extension
- Embedding generation with Matryoshka truncation (768d → 256d)
- AGENTS.md context injection with security sanitization
- **SOUL.md personality system** - User-configurable agent personality
- **Context Continuity with Graceful Interruption** - LLM pauses/resumes during overflow
- **Factual Memory System** - Persistent fact storage with decay and conflict resolution
- Complete documentation with mdBook
- Man page
- Termux/Android builds
- Error recovery for tool/network errors

### v0.21.x - ChatSession Integration

- ChatSession integration (auto-save messages + embeddings)
- `/migrate` command (JSON → SQLite)
- `/reindex` command (rebuild embeddings)
- Context overflow handling (auto-compaction at 80%)
- Auto-retrieval (M relevant + N recent messages)
- Context composition based on "Lost in the Middle" research

### v0.22.x - Chunking & Compaction

- Message chunking for long messages (>1024 chars)
- UTF-8 safe chunking with char boundary detection
- Synchronous chunking (guaranteed persistence)
- Embedding recovery on startup
- Middle compaction (preserve first N + last N)
- Auto-compaction at 72% warning and 80% overflow
- Visual context utilization bar in /context
- Remember tool for conversation recall
- Conversation-aware retrieval (enrichment)
- Project-aware query mode

### v0.26.x - Memory & Storage

- Source attribution in memory system (`SourceType` enum)
- SQLite as primary storage (schema v4, `/restore`, auto-migration)
- `ConversationStorage` deprecated, removed from REPL

### v0.27.x - Quality Improvements

- Markdown in compaction summaries
- Web scraping content quality improvements
- Compaction visual indicator

### v0.28.x - CLI Tools & Timeout

- **CLI Tools Infrastructure (Phase 1)**
  - External module with types, config, platform detection
  - Per-tool TOML parsing for tools.toml
  - `check_tool_availability()` and `run_command()` tools
  - Simplified run_command API: single command_line string
  - Debug logging for tool failures
  - Fixed duplicate error messages in REPL

- **run_command Security Redesign**
  - No shell features (pipes, redirects, command chains blocked)
  - Mandatory whitelist (only configured tools can execute)
  - head/tail parameters for LLM-controlled output truncation
  - Landlock sandbox (enabled by default on Linux, kernel 5.13+)
  - Platform-specific sandbox handling (Termux, macOS documented)
  - Pattern validation with proper ordering (multi-char before single-char)

- **run_command Timeout & Parameter Types**
  - Fixed critical bug: processes not killed on timeout
  - Changed to tokio::process::Command with kill_on_drop(true)
  - Fixed parameter types from Option<usize> to Option<String> (LLM compatibility)
  - Removed dead code (executor.rs, registry.rs)
  - Added unit tests for timeout and string parameter handling

- **SQLite Cleanup**
  - Created `src/project.rs` with `get_project_id()` and `normalize_git_url()`
  - Updated `history.rs` to be purely migration module (deprecated)
  - Clear separation: project identification vs. legacy storage
  - `history.rs` kept for `/restore` command (disaster recovery)
  - Updated user documentation: `doc/src/commands/chat.md` now explains SQLite storage

---

## Priority Roadmap

### ✅ PRIORITY 0: Factual Memory System (COMPLETED)

**Status:** ✅ COMPLETED

**Goal:** Enable ask-ai to remember user preferences and project facts across sessions.

**Problem Statement:**
- Users must repeat contextual information every session (e.g., "my docs are in ~/docs")
- No persistent storage for facts about user/project
- AGENTS.md is static and project-level only
- LLM doesn't learn from interactions

**Solution:** Persistent fact storage with automatic decay, LLM-autonomous management, and intelligent conflict resolution.

**Documentation:** See [Factual Memory System Design](./doc/src/development/factual-memory-system.md) for complete design.

**Key Insight:** Factual Memory and Feedback System (PRIORITY 1) are **orthogonal** and **complementary**:
- Factual Memory → "What do I know about the user/project?"
- Feedback System → "How should I weight retrieved messages?"
- They operate at different layers and don't conflict.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│                    FACTUAL MEMORY SYSTEM                    │
│                    (SIMPLIFIED)                             │
├─────────────────────────────────────────────────────────────┤
│  Storage: SQLite (facts table + FTS5, same DB)             │
│  Scope: project (default) + global (override)               │
│  Categories: preference (180d), fact (30d)                  │
│  Classification: Heuristic only (no LLM)                   │
│  Search: FTS5 keyword search (no embeddings)                 │
│  Conflict Resolution: Heuristic → FTS5 → LLM fallback      │
│  Decay: Ebbinghaus curve with access reinforcement          │
│  Limits: 500 chars/fact, 2200 chars total in prompt         │
└─────────────────────────────────────────────────────────────┘
```

**Design Decisions:**
- **Only 2 categories:** `preference` (180d) and `fact` (30d). No `context` category (handled by RAG).
- **No embeddings:** FTS5 keyword search only, simpler and faster.
- **Heuristic classification:** No LLM for classification (pattern matching), LLM only for conflict resolution.
- **Hard limit:** 500 chars per fact (rejected at DB), 2200 chars total (truncated in prompt with Unicode-safe truncation).
- **Same DB:** Uses existing `embeddings.db`, no separate storage.

**Implementation Phases:**

| Phase | Description | Status | Effort |
|-------|-------------|--------|--------|
| 0.1 | Schema (facts table + FTS5, migration v5→v6) | ✅ DONE | 0.5 day |
| 0.2 | Core module (types, CRUD, decay) | ✅ DONE | 1 day |
| 0.3 | LLM tools (fact_add/search/remove) | ✅ DONE | 1 day |
| 0.4 | Prompt injection (## User Facts section) | ✅ DONE | 0.5 day |
| 0.5 | Decay startup + /fact prune command | ✅ DONE | 0.5 day |
| 0.6 | User commands (/fact add/list/remove/search) | ✅ DONE | 0.5 day |
| 0.7 | Conflict resolution (detect + resolve) | ✅ DONE | 0.5 day |
| 0.8 | Testing & documentation | ✅ DONE | 0.5 day |
| **Total** | | ✅ **COMPLETED** | **5 days** |

**Files to Create:**
- `src/facts/mod.rs` - Module exports
- `src/facts/types.rs` - Category, Scope, Source, Fact structs
- `src/facts/db.rs` - CRUD operations
- `src/facts/classify.rs` - Heuristic classification
- `src/facts/decay.rs` - Ebbinghaus decay calculations
- `src/facts/conflict.rs` - Conflict detection and resolution
- `src/facts/prompt.rs` - Build "## User Facts" section
- `src/tools/facts.rs` - LLM tools

**Files to Modify:**
- `src/db/schema.rs` - Add facts table (v6)
- `src/db/connection.rs` - Migration v5→v6
- `src/prompts/builder.rs` - Add `with_facts()`
- `src/chat/core.rs` - Load facts on session start
- `src/chat/repl.rs` - Add /fact command parsing
- `src/chat/command_handlers.rs` - Add /fact handlers
- `Cargo.toml` - Add `fact-tools` feature

**LLM Tools (autonomous):**

```rust
fact_add(content, scope?)   // LLM calls autonomously, auto-classified
fact_search(query, scope?)  // LLM searches facts (FTS5)
fact_remove(id)             // LLM removes incorrect facts
```

**User Commands:**

```
/fact add <text>            // Add project fact (auto-classified)
/fact add --global <text>   // Add global fact
/fact list                  // List all facts
/fact list --global         // List global facts only
/fact remove <id>           // Remove a fact
/fact search <query>        // Search facts
/fact prune                 // Manual decay run
```

**Related:** Issue #20

---

### ✅ PRIORITY 0: TODO System Activation (COMPLETED)

**Status:** ✅ COMPLETED (v0.34.0)

**Goal:** Activate the existing TODO system to enable task tracking for both LLM and users.

**Problem Statement:**
- TODO system (`src/chat/todo_state.rs` and `src/tools/todo.rs`) was implemented but not integrated
- LLM tools registered but no synchronization with session state
- No user commands to manage TODOs interactively
- Tasks not persisted across sessions

**Solution:** Activate the TODO system with full integration.

**Implementation:**

| Component | Description | Status |
|-----------|-------------|--------|
| Tools sync | `load_from_session()` / `save_to_session()` functions | ✅ |
| User commands | `/todo add/list/update/clear-done/clear-all` | ✅ |
| Command handlers | `handle_todo_*` functions | ✅ |
| Prompt integration | `format_todos_for_prompt()` in system prompt | ✅ |
| Session persistence | Load/save todos with session in `repl.rs` | ✅ |

**Files Modified:**
- `src/tools/todo.rs` - Added `load_from_session()`, `save_to_session()`, `format_todos_for_prompt()`
- `src/chat/commands.rs` - Added `ChatCommand::TodoAdd/TodoList/TodoUpdate/TodoClearDone/TodoClearAll`
- `src/chat/command_handlers.rs` - Added `handle_todo_*` functions
- `src/chat/repl.rs` - Added command handling and session sync
- `src/prompts/builder.rs` - Added `todos` field to `PromptConfig`
- `src/chat/core.rs` - Added `todos_section` parameter to `build_session_system_prompt()`

**LLM Tools (already registered):**

```
todo_add(description)       // Add a new task
todo_list()                 // List all tasks
todo_update(id, status)     // Update task status
todo_clear_done()            // Clear completed tasks
todo_clear_all()             // Clear all tasks
```

**User Commands:**

```
/todo add <description>            // Add a new task
/todo list                          // List all tasks
/todo update <id> <status>          // Update task status (pending|in_progress|done)
/todo clear-done                    // Clear completed tasks
/todo clear-all                      // Clear all tasks
/ta <description>                   // Shortcut: add task
/tl                                   // Shortcut: list tasks
/tu <id> <status>                    // Shortcut: update task
```

**Architecture:**

```
┌─────────────────────────────────────────┐
│           TODO SYSTEM FLOW              │
├─────────────────────────────────────────┤
│  Session Start                          │
│  └── load_from_session(session.todos)   │
│      └── Copies to global TODO_STATE    │
│                                         │
│  During Session                         │
│  ├── LLM calls todo_* tools            │
│  │   └── Operates on TODO_STATE        │
│  ├── User runs /todo commands          │
│  │   └── Operates on TODO_STATE        │
│  │   └── Syncs to session.todos       │
│  └── System prompt includes todos      │
│      └── format_todos_for_prompt()    │
│                                         │
│  Session End                            │
│  └── save_sqlite()                     │
│      └── session.todos.to_rows()      │
│          └── Database persistence      │
└─────────────────────────────────────────┘
```

**Estimated effort:** 0.5 day → **Actual:** 0.5 day

**Related:** Issue #25

---

### ✅ PRIORITY 1: Code Quality - Prompts Centralization (COMPLETED)

**Status:** ✅ COMPLETED (v0.33.0)

**Goal:** Centralize all prompts in `prompts/` module for maintainability.

**Problem:**
- Prompts for compaction and continuation were embedded in `core.rs`
- Difficult to find and modify prompts scattered across files
- Inconsistent prompt management

**Solution:** Move prompts to centralized location in `prompts/` module.

**Tasks Completed:**

| Task | File | Status |
|------|------|--------|
| Add `COMPACTION_PROMPT` constant | `prompts/base.rs` | ✅ |
| Add `CONTINUATION_PROMPT_TEMPLATE` constant | `prompts/base.rs` | ✅ |
| Create `build_compaction_prompt()` function | `prompts/builder.rs` | ✅ |
| Move `build_continuation_prompt()` | `prompts/builder.rs` | ✅ |
| Update exports in `prompts/mod.rs` | `prompts/mod.rs` | ✅ |
| Refactor `core.rs` to use centralized prompts | `chat/core.rs` | ✅ |

**Files Modified:**
- `src/prompts/base.rs` - Added `COMPACTION_PROMPT` and `CONTINUATION_PROMPT_TEMPLATE`
- `src/prompts/builder.rs` - Added `build_compaction_prompt()`, moved `build_continuation_prompt()`
- `src/prompts/mod.rs` - Updated exports
- `src/chat/core.rs` - Removed ~50 lines of prompt templates, now uses centralized functions

**Estimated effort:** 0.5 day → **Actual:** 0.5 day

**Related:** Issue #21

---

### 🔄 PRIORITY 1: Code Quality - REPL Complexity Reduction (Follow-up)

**Status:** 🔄 IN PROGRESS

**Goal:** Continue reducing cyclomatic complexity of `run_chat_repl` after Phase 1 refactoring.

**Context:** Phase 1 (Issue #7) completed the initial refactoring, extracting 600+ lines into separate modules. Issue #22 tracks follow-up improvements.

**Problem:**
- `run_chat_repl` still has some cyclomatic complexity
- May need further simplification of remaining logic
- Middleware pattern for hooks not yet evaluated

**Solution:** Continue refactoring with Command/Handler pattern and proper separation.

**Tasks:**

| Task | Description | Status |
|------|-------------|--------|
| Extract processing logic | Move input processing to separate function | ❌ |
| Simplify main loop | Reduce branches in REPL loop | ❌ |
| Consider middleware pattern | For logging, metrics, compaction hooks | ❌ |
| Evaluate Command pattern | For cleaner dispatch of handlers | ❌ |

**Estimated effort:** 2-3 days

**Related:** Issue #22

---

### 🔴 PRIORITY 2: Notes System

**Status:** ❌ NOT STARTED

**Goal:** Persistent notes with semantic search.

**Features:**
- `/note add/list/show/edit/delete` commands
- Note storage with embeddings
- Update context builder for note results
- Add `SourceType::Note` to retrieval system

**Dependencies:** None

**Estimated effort:** 2-3 days

**Reference:** `doc/src/development/planning-session-cli-tools.md` lines 157-160, 303-311

**Related:** Issue #6

---

### 🔴 PRIORITY 2: Feedback Infrastructure

**Status:** 📋 PLANNED (depends on: Factual Memory)

**Goal:** Capture explicit and implicit feedback signals.

**Documentation:** See [Implementation Directive](./doc/src/development/implementation-directive.md) for complete design.

**Related:** Issue #23

**Key Insight:** Feedback improves *how we retrieve* past messages. Factual Memory provides *what we know* about the user. Both layers work together:

```
Context Assembly:
├── System Prompt
│   └── [FACTUAL MEMORY] ← "User prefers Portuguese"
│       "Docs are in ~/docs"
├── Retrieved Context (messages)
│   └── [FEEDBACK WEIGHT] ← Message #42: +1.2 (good feedback)
│       Message #15: -0.3 (bad feedback)
└── Response
```

**Implementation Phases:**

| Phase | Description | Effort |
|-------|-------------|--------|
| 1.1 | `/feedback` command + schema | 2 days |
| 1.2 | Weight propagation | 1 day |
| 1.3 | `/context` enhancement | 0.5 day |
| 1.4 | Implicit signal capture | 1 day |
| 1.5 | Weighted retrieval | 3 days |
| 1.6 | Decay implementation | 1 day |
| **Total** | | **8.5 days** |

**Related:** Issue #23

---

### ✅ PRIORITY 2: Context Continuation (COMPLETED)

**Goal:** Enable LLM to gracefully pause reasoning when context fills up, then automatically continue after compaction.

**Problem Statement:**
- LLM can run out of context mid-task (complex multi-step operations)
- Current auto-compaction happens AFTER response completes
- No mechanism for LLM to signal "I need to pause and continue later"
- Lost work when context overflow occurs during tool execution

**Solution (Implemented):** Tag-based continuation protocol:
1. ✅ LLM receives context % in prompt (`with_context_status()`)
2. ✅ LLM instructed to emit `<continuation_needed>` tag when pausing
3. ✅ System detects tag, compacts, and injects continuation prompt via ephemeral
4. ✅ LLM continues without user intervention
5. ✅ Supports nested continuations (up to 3)

**Implementation Completed:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Context status in prompt | ✅ Done |
| 2 | Continuation tag parsing | ✅ Done |
| 3 | Ephemeral messages support | ✅ Done |
| 4 | Continuation loop in REPL | ✅ Done |
| 5 | Tests and documentation | ✅ Done |

**Key Components Implemented:**

1. **Context Status Section** (prompts/builder.rs)
   - `PromptConfig.context_status` field
   - Dynamic section showing usage % injected when >72%
   - Warning when critical: `⚠️ CRITICAL: Context window is nearly full`

2. **CONTEXT MANAGEMENT Instructions** (prompts/base.rs)
   - `CONTEXT_MANAGEMENT_INSTRUCTION` constant
   - Instructs LLM on pause protocol
   - Injected when context is overflow (>80%)

3. **ContinuationTag Parsing** (chat/custom_coordinator.rs)
   - `ContinuationTag` struct with `paused_at` and `next_step`
   - `parse_continuation_tag()` extracts and strips tag
   - Ignores tags inside code blocks

4. **Ephemeral Messages** (chat/custom_coordinator.rs)
   - `push_ephemeral()` for continuation prompts
   - Prepended to requests but never persisted

5. **Continuation Loop** (chat/repl.rs)
   - `build_continuation_prompt()` creates resume instructions
   - `send_message()` accepts optional continuation_tag
   - Automatic continuation after compaction
   - Token metrics accumulated across continuations

4. **Ephemeral Messages** (custom_coordinator.rs)
   - `ephemeral_messages: Vec<ChatMessage>` - not saved to history
   - `push_ephemeral()` - add temporary message
   - `take_ephemeral()` - retrieve and clear
   - Prepended to request before history

5. **Continuation Loop** (repl.rs)
   - Detect continuation tag in response
   - Compact context
   - Inject continuation prompt as ephemeral message
   - Continue generation loop
   - Auto-retry until no continuation needed

**Constants (Reused):**
- `DEFAULT_OVERFLOW_THRESHOLD: f32 = 0.8` - Critical (80%)
- `PRE_TOOL_THRESHOLD: f32 = 0.75` - Warning (75%)
- Warning at 72% (90% of 80%)

**Data Structures:**

```rust
pub struct ContinuationTag {
    pub paused_at: String,   // Where reasoning stopped
    pub next_step: String,   // What was about to be done
}

// Note: Continuation is detected via SendMessageResult.continuation_needed field,
// not via ChatEvent. The ChatEvent enum only has PreToolContent, ToolCall, ToolResult.
```

**Edge Cases:**
- Tag embedded in code block → Should NOT be parsed
- Multiple tags → Parse first one only
- Empty tag content → Treat as no continuation
- Tag in pre-tool content → Parse and handle

**Testing:**
1. Unit tests for `parse_continuation_tag()`
2. Integration test for continuation loop
3. Edge case tests (empty, multiple, in code block)
4. Manual testing with simulated context pressure

**Dependencies:** None (all infrastructure exists)

**Estimated effort:** 1-2 days

---

### ✅ PRIORITY 1: PreToolContent Persistence & Context Enrichment (COMPLETED)

**Status:** ✅ COMPLETED (All phases done)

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Schema v5 (message_type, previous_message_id) | ✅ Done |
| 1 | insert_message_with_type() | ✅ Done |
| 1 | get_subsequent_assistant_messages() | ✅ Done |
| 1 | get_previous_message_id() | ✅ Done |
| 1 | enrich_with_context() for multiple messages | ✅ Done |
| 1 | SearchResult struct updated | ✅ Done |
| 1 | All queries updated with message_type | ✅ Done |
| 2 | PreToolContent struct | ✅ Done |
| 2 | CustomCoordinator accumulators | ✅ Done |
| 2 | take_pre_tool_content() | ✅ Done |
| 2 | process_response() accumulation | ✅ Done |
| 2 | SendMessageResult updated | ✅ Done |
| 3 | SavedMessage.message_type | ✅ Done |
| 3 | add_pre_tool_message() | ✅ Done |
| 3 | add_user_message() returns message_id | ✅ Done |
| 3 | update_message_previous_id() | ✅ Done |
| 3 | get_conversation_messages includes message_type | ✅ Done |
| 4 | format_retrieved_context() | ✅ Done |
| 4 | Prompts MEMORY TOOLS navigation section | ✅ Done |
| 4 | remember.rs shows message_type | ✅ Done |

**Key Files Modified:**
- `src/db/schema.rs` - Schema v5 definition
- `src/db/connection.rs` - Migration v4→5
- `src/db/operations.rs` - New methods, updated SearchResult
- `src/chat/session.rs` - SavedMessage.message_type, add_pre_tool_message()
- `src/chat/custom_coordinator.rs` - PreToolContent accumulation
- `src/chat/repl.rs` - PreToolContent extraction and saving
- `src/prompts/builder.rs` - MEMORY TOOLS navigation instructions
- `src/tools/remember.rs` - Shows subsequent_messages with type

**Commits:**
- `0f9a6d2 feat(db): add message_type and previous_message_id columns (schema v5)`
- `7b91c47 feat(chat): accumulate PreToolContent in CustomCoordinator`

### ✅ PRIORITY 1: SOUL.md - AI Personality System (COMPLETED)

**Status:** ✅ COMPLETED (v0.29.0)

**Implementation:**
- `src/soul.rs` - Module for loading and processing SOUL.md
- `src/prompts/base.rs` - Added `PERSONALITY_DEFAULT` fallback
- `src/prompts/builder.rs` - Integrated SOUL layer into prompt assembly
- `src/prompts/personality.rs` - REMOVED (Pepe personality)
- CLI flags: `--soulless` for `chat` and `query` commands
- Documentation: `doc/src/soul.md`

**Breaking Change:** Pepe personality removed. Users should create their own `~/.config/ask-ai/SOUL.md` for custom personalities.

---

### 🔴 PRIORITY 2: File Write Tools (COMPLETED)

**Status:** ✅ COMPLETED (v0.32.0)

**Goal:** Enable LLM to create, edit, and append to files safely.

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | `write_file` tool | ✅ Done |
| 2 | `edit_file` tool | ✅ Done |
| 3 | `append_file` tool | ✅ Done |
| 4 | Blocklist module | ✅ Done |
| 5 | Integration into read tools | ✅ Done |
| 6 | Documentation | ✅ Done |

**Key Files Modified:**
- `src/tools/files_blocklist.rs` - Shared security module with blocked patterns
- `src/tools/files_write.rs` - Write operations module (write_file, edit_file, append_file)
- `src/tools/files.rs` - Added blocklist checks to read operations
- `src/tools/mod.rs` - Export new modules
- `src/tools/registry.rs` - Register new tools
- `src/external/types.rs` - Added `FileToolsConfig` struct
- `src/external/config.rs` - Added `FileToolsSection` for TOML parsing
- `src/external/mod.rs` - Export `FileToolsConfig`
- `Cargo.toml` - Added `tempfile = "3"` as dev dependency
- `doc/src/tools.md` - Documented all 8 file tools

**Commits:**
- `f0e9481 feat: add files_blocklist module with shared security logic`
- `e8dfabe feat: add write_file, edit_file, and append_file tools`
- `82fa9e5 feat: add file-tools config section for blocked patterns`
- `fed4a9e feat: integrate blocklist into read operations`
- `4a08cb0 fix: use strip_prefix instead of manual slicing in clippy`

**Security Model:**
- **Sandbox ALWAYS enforced** for writes (ignoring `sandbox=false`)
- **Blocked patterns** for sensitive files (`.env`, `secrets`, `.pem`, etc.)
- **5MB size limit** per operation
- **Atomic writes** (temp file + rename) to prevent corruption
- **UTF-8 validation** - reject binary content

**Configuration:**
```toml
[file-tools]
max_file_size = 5242880  # 5MB
blocked_patterns = [".env.*", "*secret*", "*.pem"]
block_read = true   # Block reading sensitive files
block_list = false  # Allow listing (filenames visible)
# block_write is always true, not configurable
```

**Reference:** `doc/src/development/file-write-tools.md` - Full implementation plan

---

### ✅ PRIORITY 3: Code Quality - run_chat_repl Refactoring (COMPLETED)

**Status:** ✅ COMPLETED (PR #19 merged)

**Goal:** Refactor the oversized `run_chat_repl` function (~1100 lines) into smaller, testable units with abstractions for future TUI migration.

**Problem:**
- `run_chat_repl` is 1100+ lines and hard to maintain
- Complex command handling with 20+ branches
- Difficult to test individual command behaviors
- High cognitive load for code reviewers
- Tight coupling to rustyline (blocks future TUI migration)

**Solution:** Extract into layered architecture with traits for input/output abstraction.

### Architecture

```
Layer 0 (Base): input.rs (trait), view.rs (trait) - NO dependencies
Layer 1 (Session): session.rs, cli.rs
Layer 2 (Implementations): input/rustyline.rs, view/terminal.rs
Layer 3 (State): repl_state.rs
Layer 4 (Core): core.rs, command_handlers.rs
Layer 5 (Entry): repl.rs (coordinator)
```

### New Modules

| File | Purpose | Status |
|------|---------|--------|
| `src/chat/input/mod.rs` | `InputBackend` trait, `InputResult` | ✅ Done |
| `src/chat/input/rustyline.rs` | `RustylineInput` implementation | ✅ Done |
| `src/chat/view/mod.rs` | `ChatView` trait, `TokenMetrics`, `WelcomeInfo` | ✅ Done |
| `src/chat/view/terminal.rs` | `TerminalView` implementation | ✅ Done |
| `src/chat/repl_state.rs` | `ReplState` struct, `ReplStateBuilder` | ✅ Done |
| `src/chat/core.rs` | `send_message`, `compact_conversation`, etc. | ✅ Done |
| `src/chat/command_handlers.rs` | Command handlers using ReplState | ✅ Done |

### Implementation Phases

| Phase | Module | Description | Status |
|-------|--------|-------------|--------|
| 1 | `input/mod.rs` | `InputBackend` trait (empty, for TUI) | ✅ Done |
| 2 | `view/mod.rs` | `ChatView` trait (empty, for TUI) | ✅ Done |
| 3 | `repl_state.rs` | Consolidate state variables | ✅ Done |
| 4 | `input/rustyline.rs` | Implement RustylineInput | ✅ Done |
| 5 | `view/terminal.rs` | Implement TerminalView | ✅ Done |
| 6 | `core.rs` | Extract send_message, compact_conversation, etc. | ✅ Done |
| 7 | `command_handlers.rs` | Extract command handlers | ✅ Done |
| 8 | `repl.rs` | Refactor to use ReplState + abstractions | ✅ Done |
| 9 | Tests | Unit tests for refactored modules | ✅ Done |

### Phase Order Rationale

**Why Phase 8 comes before Phase 7:**

The async command handlers in `repl.rs` need ~8 parameters each (session, ollama, model_config, db, embedding_client, etc.). Extracting them now would require:

```rust
// Before Phase 8 - messy with many parameters
pub async fn handle_compact(
    ollama: &Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
) -> Result<(), String>
```

**After Phase 8**, we'll have `ReplState` populated in the REPL loop:

```rust
// After Phase 8 - clean single parameter
pub async fn handle_compact(state: &mut ReplState) -> Result<(), String>
```

`ReplState` (from Phase 3) already contains all the necessary fields. Phase 8 populates it in the REPL loop, then Phase 7 extracts handlers with clean signatures.

### Checkpoint 1 (2026-03-13)

**Completed:** Phases 1-5
- Input abstraction layer (`InputBackend` trait)
- Output abstraction layer (`ChatView` trait)
- RustylineInput implementation with history/completion
- TerminalView implementation with all output methods
- ReplState struct with builder pattern

### Checkpoint 2 (2026-03-13)

**Completed:** Phase 6
- Created `src/chat/core.rs` with:
  - `TokenMetrics` struct (moved from repl.rs)
  - `SendMessageResult` struct (moved from repl.rs)
  - `send_message()` async function
  - `setup_coordinator()` function
  - `prepare_messages()` async function
  - `process_chat_response()` function
  - `build_session_system_prompt()` function
  - `build_continuation_prompt()` function
  - `auto_compact_if_needed()` async function
  - `compact_conversation()` async function
- Removed ~600 lines of duplicated code from `repl.rs`
- Updated `view/mod.rs` to re-export `TokenMetrics` from core

**Next:** Phase 7 - Extract command handlers using `ReplState`

### Checkpoint 3 (2026-03-13)

**Completed:** ReplState extended + Phase order finalized
- Added `Settings` to `ReplState` struct and `ReplStateBuilder`
- Documented why Phase 8 comes before Phase 7 (ReplState enables cleaner handler extraction)
- `repl.rs` reduced from ~1916 to ~1359 lines (557 lines moved to core.rs)

### Checkpoint 4 (2026-03-14) - Phase 8 COMPLETE

**Completed:** Variable migration to ReplState
- Created `ReplState` at start of `run_chat_repl` (line 318)
- Migrated all variables to `state.*` references:
  - [x] `use_debug` → `state.use_debug`
  - [x] `cli_code` → `state.cli_code`
  - [x] `cli_soulless` → `state.cli_soulless`
  - [x] `agents_md` → `state.agents_md`
  - [x] `tools_active` → `state.tools_active`
  - [x] `capabilities` → `state.capabilities`
  - [x] `model_config` → `state.model_config`
  - [x] `current_model_name` → `state.current_model_name`
  - [x] `session` → `state.session.*` (fields and methods)
  - [x] `ollama` → `state.ollama`
  - [x] `db` → `state.db`
  - [x] `embedding_client` → `state.embedding_client`
  - [x] `settings` → `state.settings`

**Commits:**
- `7a9e3a3` - Add command_handlers.rs placeholder
- `06b1f8a` - Migrate use_debug, cli_code, cli_soulless, agents_md
- `0d80f57` - Migrate settings
- `c3f9c2f` - Migrate ollama, db, embedding_client
- `038039a` - Migrate tools_active
- `12b4dcf` - Migrate current_model_name
- `19ea48c` - Migrate model_config and capabilities
- `08d6101` - Migrate session

**Phase Order Rationale:**
- Phase 8 populates `ReplState` in the REPL loop
- Phase 7 then extracts handlers with clean 1-parameter signatures: `fn handle_xxx(state: &mut ReplState)`
- Without ReplState, handlers would need 8+ parameters each

**Current State:**
- Phases 1-8 complete
- `repl.rs` reduced from ~1916 to ~1080 lines
- Phase 7 COMPLETE (all handlers extracted)

### Checkpoint 5 (2026-03-14) - Phase 7 COMPLETE ✅

**Completed:** Handler extraction from repl.rs to command_handlers.rs
- [x] Phase 0: Fixed variable references (`session`, `ollama`, `db`, `agents_md` → `state.*`)
- [x] Phase 1: Simple handlers (think, tools, retrieval, debug, tool-output)
- [x] Phase 2: Sync handlers (undo)
- [x] Phase 3: Async handlers (search, restore, reindex)
- [x] Phase 4: Complex async handler (compact)
- [x] Phase 5: Most complex handler (retry)

**Handlers Extracted (11/11 - ALL COMPLETE):**
| Handler | Type | Status |
|---------|------|--------|
| `handle_think_toggled` | sync | ✅ |
| `handle_tools_toggled` | sync | ✅ |
| `handle_retrieval_toggled` | sync | ✅ |
| `handle_tool_output_changed` | sync | ✅ |
| `handle_debug_toggled` | sync | ✅ |
| `handle_undo` | sync | ✅ |
| `handle_search` | async | ✅ |
| `handle_restore` | sync | ✅ |
| `handle_reindex` | async | ✅ |
| `handle_compact` | async | ✅ |
| `handle_retry` | async | ✅ |

**File Size Reduction:**
- `repl.rs`: 1380 → 1080 lines (300 lines reduced, 22% reduction)
- `command_handlers.rs`: 48 → 424 lines (new functionality)

**Commits in this session:**
- `e37a6c2` - Complete ReplState migration in repl.rs loop
- `bdc5d5b` - Extract simple command handlers to command_handlers.rs
- `3758238` - Extract handle_undo to command_handlers.rs
- `4eb2f48` - Extract async handlers (search, restore, reindex)
- `96eb775` - Progress update - 9 handlers extracted
- `66510ee` - Extract handle_compact to command_handlers.rs
- `aa076eb` - Extract handle_retry to command_handlers.rs

**Phase 7 Complete!** All command handlers have been extracted with clean signatures.

### Checkpoint 6 (2026-03-14) - Phase 9 COMPLETE ✅

**Completed:** Unit tests for command handlers
- Added 10 unit tests for `command_handlers.rs`
- Tests cover: think_toggle, tools_toggle, retrieval_toggle, debug_toggle, tool_output_changed, undo
- All tests pass with `--all-features`
- Clippy passes with `-D warnings`

**Tests Added:**
| Test | Coverage |
|------|----------|
| `test_handle_think_toggled_unsupported` | Model doesn't support thinking |
| `test_handle_think_toggled_enabled` | Model supports thinking |
| `test_handle_tools_toggled_unsupported` | Model doesn't support tools |
| `test_handle_tools_toggled_supported` | Model supports tools, enable |
| `test_handle_tools_toggled_disables_when_false` | Disable tools |
| `test_handle_retrieval_toggled_enabled` | Enable retrieval |
| `test_handle_retrieval_toggled_disabled` | Disable retrieval |
| `test_handle_debug_toggled` | Toggle debug mode |
| `test_handle_tool_output_changed` | Change output level |
| `test_handle_undo_empty_session` | Undo with empty session |

**Quality Checks:**
- [x] `cargo build --all-features` - compiles without errors
- [x] `cargo clippy --all-features -- -D warnings` - no warnings
- [x] `cargo test --all-features` - 362 tests pass
- [x] Functional behavior unchanged (handlers extracted, not modified)

**Final File Sizes:**
| File | Before | After | Change |
|------|--------|-------|--------|
| `repl.rs` | 1380 lines | 1080 lines | -300 (22%) |
| `command_handlers.rs` | 48 lines | 490 lines | +442 (new) |

### TUI Preparation

This refactoring prepares for future `ratatui.rs` TUI:

- `InputBackend` trait enables swapping rustyline for TUI input widget
- `ChatView` trait enables swapping println for TUI rendering
- `ReplState` separates state from I/O layer
- `ChatCore` makes business logic reusable across UIs

See `doc/src/development/roadmap.md` - TUI section for future work.

**Benefits:**
- Each function under 200 lines
- Individual behaviors testable in isolation
- Clearer separation of concerns
- Input/output abstraction for TUI migration
- Easier code review for changes

**Estimate:** 16-24 hours → **Actual: 24h**

**Branch:** `refactor/run-chat-repl-decoupling`

**PR:** [#19](https://github.com/luksamuk/ask-ai-rs/pull/19) (MERGED)

**Issues:** [#7](https://github.com/luksamuk/ask-ai-rs/issues/7) (CLOSED), [#22](https://github.com/luksamuk/ask-ai-rs/issues/22) (OPEN - follow-up)

---

### 🔴 PRIORITY 4: Skills System Phase 1

**Status:** ❌ NOT STARTED

**Goal:** Markdown-defined AI behaviors for tool pipelines.

**Features:**
- SkillsLoader for `.md` files
- Builtin skills (pdf-processing, ocr-images)
- User skills (`~/.config/ask-ai/skills/`)
- Project skills (`.ask-ai/skills/`)
- Prompt injection integration

**Dependencies:** None

**Estimated effort:** 3-5 days

**Reference:** `doc/src/development/skills-system-design.md`

---

### 🟡 PRIORITY 5: Document Import Tool

**Status:** ❌ BLOCKED (requires Skills System Phase 1)

**Goal:** Import documents for semantic search.

**Features:**
- TEXT/MD: Builtin support (import_text_file)
- PDF: External tools (pdftotext) + skills
- Scanned PDF: tesseract + pdftoppm pipeline
- Chunking with overlap (512 tokens, 64 overlap)
- `/import-doc`, `/list-docs`, `/remove-doc` commands
- Update `search_hybrid()` for document chunks

**Dependencies:** Skills System Phase 1 (for PDF pipeline definition)

**Estimated effort:** 5-7 days

---

### 🟡 PRIORITY 6: Chat Module Integration

**Status:** ❌ NOT STARTED

**Goal:** Use OCR/Vision/Translate/Summarize from chat.

**Features:**
- `/ocr`, `/vision`, `/translate`, `/summarize` commands in REPL
- Model switching during commands
- Design: temporary context or persistent?

**Dependencies:** None

**Estimated effort:** 3-5 days

---

### 🟡 PRIORITY 7: Parallel Tool Execution

**Status:** ❌ NOT STARTED

**Goal:** Execute independent tool calls in parallel for faster response times.

**Problem:**
- Current implementation executes tool calls sequentially
- LLM often requests multiple independent tools (e.g., `get_weather` + `get_current_datetime`)
- Sequential execution unnecessarily increases latency
- User waits longer for responses with multiple tools

**Solution:** Detect independent tool calls and execute concurrently using `tokio::join!` or `futures::join_all`.

**Architecture:**

```rust
// Current: Sequential execution
for tool_call in tool_calls {
    let result = execute_tool(&tool_call).await;
    results.push(result);
}

// Proposed: Parallel execution
let futures: Vec<_> = tool_calls.iter()
    .map(|tc| execute_tool(tc))
    .collect();
let results = futures::future::join_all(futures).await;
```

**Dependencies Analysis:**
- Tools are independent if they don't modify shared state
- Read-only tools (weather, calc, search) can run in parallel
- Tools that modify state (file writes, database) need sequential execution

**Implementation Phases:**

| Phase | Task | Duration |
|-------|------|----------|
| 1 | Identify which tools are safe for parallel execution | 0.5 day |
| 2 | Implement dependency analysis in CustomCoordinator | 1 day |
| 3 | Parallel execution with `join_all` | 1 day |
| 4 | Preserve sequential order for stateful tools | 0.5 day |
| 5 | Tests and benchmarks | 1 day |

**Safe for Parallel (read-only):**
- `get_weather`, `get_current_datetime`
- `read_file`, `read_file_segment`, `count_lines`, `list_directory`, `search_files`
- `web_search`, `search_duckduckgo`
- `calculate`
- `get_pokemon_*` (all Pokemon tools)
- `get_system_info`

**Requires Sequential (stateful/write):**
- `run_command` (may have side effects)
- `write_file`, `edit_file`, `append_file` (when implemented)
- Database operations
- File writes

**Dependencies:** None

**Estimated effort:** 3-4 days

---

### 🟢 LOW PRIORITY: Memory Enhancement

**Status:** ❌ BLOCKED (requires Document Import + Notes System)

**Phases:**
- **Phase 2:** Query routing (blocked by multiple source types)
- **Phase 3:** Timestamp filtering (blocked by Phase 2)
- **Phase 4-5:** Advanced memory features (blocked by Document Import + Notes)

**Reference:** `doc/src/development/effective-agents-analysis.md` lines 196-226, 446-545

---

### 🟢 LOW PRIORITY: Other Features

- **OCR/Vision Tools** - Image processing via CLI tools (tesseract, exiftool, imagemagick)
- **File Session State** - Explicit file tracking with security constraints
- **Skills System Extended** - YAML frontmatter, skill composition
- **Plugin System** - User-defined tools via dynamic loading
- **TUI (Terminal User Interface)** - Ratatui-rs based interface

---

## Streaming Architecture (Future)

The `ollama-rs` library (already included with `stream` feature) provides streaming capabilities:

```rust
// Streaming API
pub async fn send_chat_messages_stream(
    &self,
    request: ChatMessageRequest,
) -> Result<ChatMessageResponseStream>

// ChatMessage includes thinking content
pub struct ChatMessage {
    pub content: String,
    pub thinking: Option<String>,  // For DeepSeek R1, etc.
    // ...
}
```

**Current Status:** Non-streaming only (`send_chat_messages()`)
**Streaming Path:** `send_chat_messages_stream()` or `send_chat_messages_with_history_stream()`

**Implementation Considerations:**

1. **CLI Mode (current):** `termimad` is synchronous, requires block buffering
2. **TUI Mode (future):** Ratatui supports incremental rendering via `tui-markdown`
3. **Thinking Display:** Separate pane in TUI, inline dimmed text in CLI

See: `doc/src/development/roadmap.md` - TUI section for detailed streaming approach

---

## Documentation

Full documentation is available in the `doc/` directory:

```bash
# View user documentation
cd doc
mdbook serve

# Or build static site
mdbook build

# View man page
man ask-ai
```

## For Developers

See the development documentation:

1. [Architecture](./doc/src/development/architecture.md) - Technical architecture
2. [Roadmap](./doc/src/development/roadmap.md) - Future plans
3. [Contributing](./doc/src/development/contributing.md) - How to contribute
4. [Context Composition Design](./doc/src/development/context_composition_design.md) - v0.21.0 design decisions

## Legacy Content

The original detailed implementation notes have been moved to:

- `doc/src/development/architecture.md` - Architecture decisions
- `doc/src/development/roadmap.md` - Future plans
- `doc/src/CHANGELOG.md` - Version history

## Last Updated

2026-03-11 - v0.28.0: SQLite cleanup, run_command timeout fix, parameter type fix