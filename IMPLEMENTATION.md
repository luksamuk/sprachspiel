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

**v0.39.0** - 2026-03-29 (Document Import Tool)

## Current Implementation Status

✅ **Completed:**

- Core CLI with 5 subcommands (query, chat, translate, ocr, summarize)
- Interactive chat mode with persistent sessions
- Custom models support via `~/.config/ask-ai/models.toml`
- Built-in models: llama3.1, translategemma, glm-ocr (user models in config)
- Thinking support for cloud models (configurable via `thinking = true`)
- Dynamic model selection with capability detection
- Tool integration with error recovery (50 tools in 14 categories)
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

**Key Insight:** Factual Memory and Feedback System (PRIORITY 5) are **orthogonal** and **complementary**:
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

### ✅ PRIORITY 1: Code Quality - run_chat_repl Complexity (COMPLETED)

**Status:** ✅ COMPLETED (v0.35.0)

**Goal:** Reduce cyclomatic complexity of `run_chat_repl` from 78/25 to <25/25.

**Context:** Phase 1 (Issue #7) completed the initial refactoring, extracting 600+ lines into separate modules. Issue #22 tracks follow-up improvements.

**Result:** Cognitive complexity reduced from 78/25 to **eliminated** (no Clippy warning for `run_chat_repl`).

**Implementation:**

| Phase | File | Task | Lines | Status |
|-------|------|------|-------|--------|
| 1 | `src/chat/continuation.rs` (NEW) | Create file with `ContinuationResult` struct | ~320 | ✅ |
| 2 | `src/chat/command_handlers.rs` | Add `handle_command_result()`, `handle_model_switch()`, `print_context_info()` | ~400 | ✅ |
| 3 | `src/chat/repl.rs` | Extract `create_session()`, `resolve_session_model()`, `resolve_thinking_mode()`, `init_database()`, `run_startup_tasks()`, `handle_user_message()` | ~300 | ✅ |
| 4 | Tests | `cargo test --all-features` | - | ✅ |
| 5 | Clippy | `cargo clippy --all-features -- -W clippy::cognitive_complexity` | - | ✅ |

**Files Modified:**
- `src/chat/repl.rs`: 1090 → 540 lines (~550 lines removed)
- `src/chat/command_handlers.rs`: Added dispatch functions
- `src/chat/continuation.rs`: NEW, continuation handling
- `src/chat/mod.rs`: Updated exports

**Commits:** Part of PR #28

**Related:** Issue #22, PR #28

---

### ✅ PRIORITY 4: Code Quality - query.rs Complexity (COMPLETED)

**Status:** ✅ COMPLETED (v0.39.6)

**Goal:** Reduce cognitive complexity of `run_query` from 32/25 to <25/25.

**Context:** Non-interactive mode function (CLI query mode).

**Implementation:**

| Phase | Task | Status |
|-------|------|--------|
| 1 | Create `src/db/init.rs` | ✅ init_database_core() |
| 2 | Refactor `src/chat/repl.rs` | ✅ init_chat_database() |
| 3 | Create `src/query/mod.rs` | ✅ Module structure |
| 4 | Create `src/query/context.rs` | ✅ QueryContext + builder |
| 5 | Create `src/query/executor.rs` | ✅ execute_query_with_retry() |
| 6 | Create `src/query/coordinator.rs` | ✅ build_query_coordinator() |
| 7 | Refactor `src/query.rs` | ✅ run_query ~100 lines |
| 8 | Tests & Clippy | ✅ Clean, complexity <25/25 |

**Files Created:**
- `src/db/init.rs` - Core DB initialization (44 lines)
- `src/query/mod.rs` - Module exports, run_query (335 lines)
- `src/query/context.rs` - QueryContext struct (219 lines)
- `src/query/coordinator.rs` - Coordinator builder (55 lines)
- `src/query/executor.rs` - Execution with retry (119 lines)

**Files Modified:**
- `src/db/mod.rs` - Export init module
- `src/chat/repl.rs` - Use init_chat_database()

**Complexity Reduction:**
- Original: 516 lines in query.rs, cognitive complexity 32/25
- Final: ~100 lines in run_query, complexity below threshold (no longer flagged)
- Duplicate retry loop removed (lines 410-489 → single execute_retry_loop function)

**Commits:**
- `768bfb6` refactor: reduce query.rs cognitive complexity (Issue #29)

**Related:** Issue #29, PR #58

---

### 🔄 PRIORITY 4: Code Quality - context_builder.rs Complexity

**Status:** 🔄 IN PROGRESS

**Goal:** Reduce cognitive complexity of `build_context` from 27/25 to <25/25.

**Context:** Retrieval context building function in `src/retrieval/context_builder.rs`.

**Analysis:**
- Function `build_context` (lines 180-378) has complexity 27/25
- Complexity sources:
  1. Nested `if let` in retrieval logic (4 levels deep)
  2. Repeated `match msg.role` blocks (same pattern twice)
  3. Multiple `if use_debug` scattered throughout

**Proposed Solution:**
- Extract `perform_retrieval()` - handles retrieval with embedding client
- Extract `push_messages_as_chat_messages()` - helper for MessageRole match pattern
- Extract `build_retrieved_context_section()` - format retrieved results

**Implementation:**

| Phase | Task | Status |
|-------|------|--------|
| 1 | Extract `push_messages_as_chat_messages()` helper | ⏳ Pending |
| 2 | Extract `perform_retrieval()` for retrieval logic | ⏳ Pending |
| 3 | Refactor `build_context` to use helpers | ⏳ Pending |
| 4 | Run tests and clippy | ⏳ Pending |

**Estimated effort:** 1 day

**Related:** Issue #30

---

### 🔵 PRIORITY 4: Code Quality - registry.rs Complexity

**Status:** ❌ NOT STARTED

**Goal:** Reduce cognitive complexity of `register_tools` from 52/25 to <25/25.

**Context:** Tool registration function - largest complexity in codebase.

**Proposed Solution:**
- Extract `register_weather_tools()`
- Extract `register_file_tools()`
- Extract `register_pokemon_tools()`
- Extract `register_calc_tools()`
- Extract `register_serper_tools()`
- Extract `register_system_tools()`
- Extract `register_search_tools()`

**Estimated effort:** 1-2 days

**Related:** Issue #31

---

### 🔵 PRIORITY 4: Code Quality - commands.rs Complexity (parse_command)

**Status:** ❌ NOT STARTED

**Goal:** Reduce cyclomatic complexity of `parse_command` from ~450 lines to manageable size.

**Context:** Command parsing function in `src/chat/commands.rs` (lines 218-671). Single monolithic function with giant `match` statement handling all commands and their aliases.

**Proposed Solution:**
- Extract individual parsers for command groups (model, todo, note, fact)
- Use derive-based pattern matching for structured commands
- Maintain identical public API

**Estimated effort:** 2-3 days

**Related:** Issue #35

---

### ✅ PRIORITY 4: Code Quality - Dead Code Cleanup (COMPLETED)

**Status:** ✅ COMPLETED (v0.37.0)

**Goal:** Remove explicitly marked dead code and document justifications for retained `#[allow(dead_code)]` annotations.

**Context:** Codebase had 80 `#[allow(dead_code)]` annotations. Some are legitimate (future use, enum completeness, serde fields), but others are clearly dead code marked "no longer used".

**Removed (4 items):**

| File | Line | Code | Reason |
|------|------|------|--------|
| `src/context_overflow.rs` | 35 | `estimate_messages_tokens()` | Replaced by `estimate_chat_messages_tokens()` |
| `src/context_overflow.rs` | 60 | `MAX_TOOL_RESULT_TOKENS` | No longer used |
| `src/context_overflow.rs` | 64 | `CHARS_PER_TOKEN` | No longer used |
| `src/context_overflow.rs` | 69 | `truncate_tool_result()` | No longer used |

**Retained with Justification (~76 items):**
- Future use: `normalize()`, `cosine_similarity()`, `estimate_tokens_code()`
- Enum completeness: `ContextStatus` variants, `ResolutionAction::Add`
- Serde/API fields: Weather, Serper, Vision, OCR response structs
- Test-only: `Database::in_memory()`, test helper methods
- Feature-gated: LED methods (used with `led-tools` feature)

**Related:** Issue #37

---

### ✅ PRIORITY 4: Status Bar Above Prompt (COMPLETED)

**Status:** ✅ COMPLETED (v0.37.2)

**Goal:** Add a dynamic status bar above the prompt input showing real-time context information.

**Implementation:**

| File | Changes |
|------|---------|
| `src/chat/view/mod.rs` | Added `StatusBarInfo` struct, `STATUS_BAR_LINES` constant, `format_status_bar()` method, visual truncation |
| `src/chat/repl_state.rs` | Added `get_status_bar_info()` method to ReplState |
| `src/chat/repl.rs` | Integrated status bar rendering before prompt, ANSI clear codes with terminal width detection, prompt `>>> ` |

**Features:**
- Model name, context usage (XX.XK/YYYK), progress bar with percentage
- Think/Tools indicators (🧠🔧) in status bar
- Colored progress bar: Green (< 50%), Yellow (50-75%), Red (> 75%)
- Fixed width (77 visual characters) to prevent overflow
- Clean prompt: `>>> ` (model and indicators moved to status bar)
- ANSI codes clear status bar and input lines based on terminal width
- Dynamic calculation using `calculate_context_metrics()`
- Unicode-aware width calculation using `unicode-width` crate

**Files Modified:**
- `src/chat/view/mod.rs` - `StatusBarInfo` struct with `format_status_bar()`, `truncate_visual()` helper
- `src/chat/repl_state.rs` - `get_status_bar_info()` method
- `src/chat/repl.rs` - `build_status_bar()`, `calculate_visual_lines()`, `build_clear_code()` helpers

**Technical Details:**
- Uses `termimad::terminal_size()` to detect terminal width
- Uses `unicode_width::UnicodeWidthStr` for proper character width (CJK, etc.)
- Calculates visual lines: `total_width.div_ceil(terminal_width).max(1)`
- Clears correct number of lines: 3 (status bar) + N (visual lines of input)
- Fallback to 1 line if terminal width unavailable

**Commits:**
- `8433736` docs: update CHANGELOG and IMPLEMENTATION for status bar feature
- `c20e2d1` feat: add status bar above prompt
- `a707f02` fix: correct spacing around separators in status bar
- `4bf6a78` fix: remove extra whitespace from status bar content line
- `fd7a28a` fix: use visual truncation for status bar content line
- `d288e50` fix: reduce status bar content width to 77 columns
- `3b51308` revert: remove status bar from spinner
- `5e03f46` feat: change prompt from '>' to '>>>'
- `921bd6f` docs: update CHANGELOG and IMPLEMENTATION with final status bar details
- `716fb50` feat: detect terminal width for ANSI clear codes

**Design Decision:**
Status bar during spinner ("Thinking...") was attempted but caused display issues with ANSI codes across different terminals. Reverted to simpler approach where status bar appears only above prompt.

**Known Limitations:**
- Emoji width may be imprecise (but user input typically doesn't contain emojis)
- Terminal width detection may fail in some environments (fallback to 1 line)
- Long input wrapping to many lines may still leave minor visual artifacts

**Related:** Issue #47

---

### 🔵 PRIORITY 4: Code Quality - Notes System (COMPLETED)

**Status:** ✅ COMPLETED

**Goal:** Persistent notes with semantic search.

**Features:**
- `/note add/list/show/edit/delete` commands
- Notes stored with embeddings for semantic search
- FTS5 full-text search for keyword matching
- Hybrid search (BM25 + vector) includes notes in results
- Project/global scope like facts

**Architecture Decision:** Unified `content_items` table (see below)

**Dependencies:** None

**Estimated effort:** 5 days

**Reference:** `doc/src/development/planning-session-cli-tools.md` lines 157-160, 303-311

**Related:** Issue #6, Issue #34

**Implementation Plan:**

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Fix TODO persistence bug (Issue #34) | ✅ Done |
| 1 | Schema v7 + migration (preserve data) | ✅ Done |
| 2 | Types + Operations (content module) | ✅ Done |
| 3 | Unified search operations | ✅ Done |
| 4 | Note commands | ✅ Done |
| 5 | Embeddings for notes | ✅ Done |
| 6 | Tests and documentation | ✅ Done |
| 7 | Embedding regeneration after migration | ✅ Done |

**Commits:**
- `be0b279` - docs: update roadmap priorities
- `9f2a50b` - docs: update CHANGELOG and IMPLEMENTATION.md
- `99c92e3` - fix(todo): sync TodoState to session after LLM interaction
- `34f3c12` - feat(db): add schema v7 with content_items unified table
- `0d66a05` - feat(content): add content module with Note CRUD operations
- `c88e324` - feat(content): add unified search operations for content_items
- `a416f42` - feat(notes): add /note commands for persistent notes
- `e5a8a57` - feat(content): add embedding support for notes
- `d2544bc` - test(content): add tests for note operations
- `9245699` - docs: update IMPLEMENTATION.md with completed phases
- `7cf2fbf` - docs: update IMPLEMENTATION.md - Notes System complete
- `b4b013b` - docs(chat): add /note commands documentation
- `5694cd9` - feat(remember): integrate notes into retrieval system
- `cf3abe1` - fix: fail fast on database initialization failure

**Migration Note (v6→v7):**

The schema migration from v6 to v7 includes a breaking change for embeddings:

1. **Removed broken embedding migration** - The attempt to migrate embeddings from `message_embeddings` to `content_embeddings` caused UNIQUE constraint errors when multiple messages had the same content.
2. **Embeddings are regenerated** - After migration completes, all embeddings are regenerated from source content with a progress bar.
3. **User data preserved** - Messages, notes, and facts are preserved. Only embeddings (derived data) are rebuilt.

**Critical Bugs Fixed During Unification:**

| Bug | Description | Fix |
|-----|-------------|-----|
| #12 | Migration dropped wrong table (`chunk_embeddings_v2` is V7, not V2) | Changed to `DROP TABLE IF EXISTS chunk_embeddings` |
| #13 | Items with chunks never marked `has_embedding=1` | Added marking logic after successful chunk embedding |
| #14 | `regenerate_all_embeddings()` deleted all chunks on startup | Removed chunk cleanup, only clean orphan chunks |
| #7 | Embedding context length exceeded (512 tokens vs 1024 chars) | Dynamic chunk sizing based on model context |
| #8 | Orphan chunks caused infinite recovery loops | Clean orphan chunks at startup |
| #42 | `note_add` panics with Unicode content | Use `truncate_chars()` for character-aware slicing |

**Dynamic Chunking Architecture:**

The embedding system now dynamically calculates chunk sizes based on the model's context length:

```rust
// src/embeddings/chunk_config.rs
pub struct DynamicChunkConfig {
    context_length: usize,      // From Ollama API (e.g., 512)
    chunk_percent: f32,         // 0.90 (90% of available context)
    overlap_percent: f32,       // 0.20 (20% overlap between chunks)
    prefix_margin: usize,       // 30 tokens for "search_document: "
    chars_per_token: f32,       // 3.0 (conservative for Portuguese/code)
}
```

**Key Parameters:**
- `chunk_percent`: 90% - Reserve 10% for tokenizer variance
- `overlap_percent`: 20% - RAG best practice for context preservation
- `prefix_margin`: 30 tokens - "search_document: " prefix (~20 tokens) + safety margin
- `chars_per_token`: 3.0 - Conservative for mixed Portuguese/code content

**Migration to v0.34.0:**

When upgrading from v6 to v7:
1. Backup your v6 database: `cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.v6`
2. Run the new version - migration happens automatically
3. All 283 messages will be migrated to `content_items`
4. Embeddings are regenerated (first startup takes ~2 minutes)
5. V2 tables (`messages`, `message_chunks`, etc.) are dropped

This ensures:
- No UNIQUE constraint failures during migration
- Clean embedding state after schema upgrade
- All search functionality works correctly after first startup
- Second startup is instant (0 items to regenerate)

**Bug #15: `/clear` Reloaded Old Messages from Database**

The `/clear` command was intended to "clear messages (preserves context for retrieval)" but:
- It only cleared `session.messages` in memory
- On session reload (`load_sqlite`), ALL messages from database were restored
- Sessions appeared to "undo" the clear after app restart

**Solution:**
- Renamed `/clear` to `/new` to better reflect behavior
- `/new` now generates a NEW `session.id` (e.g., `session-1712345678`)
- Old messages stay in database (searchable via `/search` and `remember()`)
- New session starts empty
- Added `count_all_content_items()` to check if database has searchable content

**Difference from `/forget`:**
| Command | Session ID | Database | Searchable |
|---------|-------------|----------|------------|
| `/new` | New | Preserved | Yes |
| `/forget` | New | Deleted | No |

---

### Architecture: Content Items (Schema v7)

**Unified table approach** for messages, notes, and future documents.

**Tables:**

```sql
-- Unified content storage
CREATE TABLE content_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type TEXT NOT NULL CHECK(content_type IN ('message', 'note', 'document')),
    
    -- Message fields (nullable)
    conversation_id TEXT,
    role TEXT CHECK(role IN ('user', 'assistant', 'system', 'tool')),
    message_type TEXT DEFAULT 'normal',
    previous_item_id INTEGER REFERENCES content_items(id),
    prompt_tokens INTEGER,
    
    -- Note/Document fields (nullable)
    scope TEXT CHECK(scope IN ('project', 'global')),
    source TEXT CHECK(source IN ('user', 'llm')),
    title TEXT,
    
    -- Common fields
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    project_id TEXT,
    has_embedding INTEGER DEFAULT 0
);

-- Unified chunks for long content
CREATE TABLE content_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES content_items(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    has_embedding INTEGER DEFAULT 0
);

-- Unified embeddings (vec0)
CREATE VIRTUAL TABLE content_embeddings USING vec0(
    item_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT
);

CREATE VIRTUAL TABLE chunk_embeddings USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT
);

-- Unified FTS5
CREATE VIRTUAL TABLE content_fts USING fts5(
    content,
    content='content_items',
    content_rowid='id',
    tokenize='porter unicode61'
);
```

**Migration Strategy:**
1. Create new tables
2. Copy data from `messages` → `content_items`
3. Copy data from `message_chunks` → `content_chunks`
4. Copy embeddings from `message_embeddings` → `content_embeddings`
5. Copy embeddings from `chunk_embeddings` → `chunk_embeddings` (new table)
6. Populate FTS5
7. Keep old tables renamed as backup

---

### ✅ PRIORITY 3: Skills System (COMPLETED)

**Status:** ✅ COMPLETED (v0.38.0)

**Goal:** Markdown-defined AI behaviors with progressive disclosure.

**Design Research (2026-03-24):**
- Analyzed Hermes Agent skills system at `~/.hermes/hermes-agent`
- Confirmed INDEX + on-demand loading pattern (not inject all skills)
- Confirmed `SKILL.md` format with YAML frontmatter
- Confirmed deduplication priority: project > user > builtin

**Architecture:**
```
System Prompt
├── SKILLS INDEX (names + descriptions)
│   └── <available_skills> section
└── Tools section

On-demand Loading:
├── LLM sees relevant skill in INDEX
├── LLM calls skill_view(name="document-processing")
└── System returns full SKILL.md content
```

**Features:**
- `skill_list()` tool - returns INDEX (names + descriptions)
- `skill_view(name)` tool - loads full skill content
- Builtin skills embedded in binary (`include_str!`)
- User skills at `~/.config/ask-ai/skills/<name>/SKILL.md`
- Project skills at `.ask-ai/skills/<name>/SKILL.md`

**Dependencies:** None (CLI Tools completed in v0.28.x)

**Implementation Phases:**

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | ✅ COMPLETED | Skills Module (types, loader, sanitize, mod) |
| 2 | ✅ COMPLETED | Builtin Skills (4 .md files) |
| 3 | ✅ COMPLETED | Skills Tools (skill_list, skill_view) |
| 4 | ✅ COMPLETED | Prompt Integration (INDEX section) |
| 5 | ✅ COMPLETED | Testing (clippy, tests pass) |
| 6 | ✅ COMPLETED | Skills Slash Commands (activate skills via /skill-name) |

### Phase 6: Skills Slash Commands

**Goal:** Allow users to activate skills via slash commands (`/document-processing`).

**Behavior:**
```
/document-processing                    → Loads skill, shows activation message
/document-processing extrair texto.pdf  → Loads skill + sends user message
/skill-list                             → Lists available skills
```

**Architecture:**
- Dynamic slash command detection based on available skills
- Skill content injected into session system prompt
- Skills activated for current session only

**Implementation:**

| File | Change |
|------|--------|
| `src/chat/commands.rs` | Add `ChatCommand::Skill { name }` and `CommandResult::Skill` |
| `src/chat/commands.rs` | Modify `parse_command()` to detect `/skill-name` dynamically |
| `src/chat/session.rs` | Add `active_skill: Option<Skill>` field |
| `src/prompts/builder.rs` | Inject active skill into system prompt |

**Estimated effort:** 2 hours

**Reference:** Hermes Agent `agent/skill_commands.py`

**Files Created:**
- `src/skills/mod.rs` - Public API
- `src/skills/types.rs` - Skill, SkillIndex, SkillSource, Frontmatter
- `src/skills/loader.rs` - YAML parsing, directory scanning, deduplication
- `src/skills/sanitize.rs` - Injection pattern detection, validation
- `src/skills/builtin/document-processing.md` - PDF and ePub extraction skill
- `src/skills/builtin/ocr-images.md` - OCR for images skill
- `src/skills/builtin/code-analysis.md` - Code analysis skill
- `src/skills/builtin/web-scraping.md` - Web scraping skill
- `src/tools/skill_tools.rs` - skill_list, skill_view tools

**Files Modified:**
- `src/prompts/builder.rs` - Added SKILLS INDEX section, active_skill field
- `src/main.rs` - Added skills module
- `src/tools/mod.rs` - Added skill_tools module
- `src/tools/registry.rs` - Registered skills tools
- `src/Cargo.toml` - Added serde_yaml, skills-tools feature
- `src/chat/commands.rs` - Added ChatCommand::Skill, CommandResult::Skill, parse detection
- `src/chat/session.rs` - Added ActiveSkill struct, active_skill field
- `src/chat/command_handlers.rs` - Added handle_skill_activated
- `src/chat/core.rs` - Wired active_skill into build_session_system_prompt

**Commits:**
- `74a25be` feat(skills): add skills module with types, loader, sanitize, and builtin skills
- `73ced3a` feat(skills): implement skill_list and skill_view tools with registry integration

**Reference:** `doc/src/development/skills-system-design.md`

**Related:** Issue #8

---

### ✅ PRIORITY 3: Document Import Tool (COMPLETED)

**Status:** ✅ COMPLETED (v0.39.0)

**Goal:** Import documents for semantic search and retrieval.

**Dependencies:** Skills System ✅ COMPLETED (v0.38.0)

**Features:**
- **File Formats:** TXT, MD, ORG (builtin), PDF, EPUB (requires `skills-tools` feature)
- **File Size Limit:** 5MB for uploaded files; larger files rejected with helpful error
- **Chunking:** Same system as notes/messages (~512 tokens)
- **Scope:** Project-scoped by default, optional global scope
- **Commands:** `/doc import`, `/doc list`, `/doc show`, `/doc delete` (shortcuts: `/di`, `/dl`, `/ds`, `/dd`)
- **LLM Tool:** `import_document(path, scope?)` for autonomous import
- **Storage:** content_items table with ContentType::Document
- **Retrieval:** Integrated with `remember()` tool via hybrid search

**Feature Flag Dependencies:**
- `document-tools` feature enabled by default
- PDF/EPUB import requires `skills-tools` feature (also default)
- TXT/MD/ORG import works standalone (no skills dependency)
- Included in `all-tools` feature

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Database & Types (document.rs, db/schema.rs, migration v8) | ✅ Done |
| 2 | LLM Tool (tools/documents.rs) | ✅ Done |
| 3 | Commands (commands.rs, command_handlers.rs) | ✅ Done |
| 4 | Embeddings integration | ✅ Done |
| 5 | Tests | ✅ Done |
| 6 | Documentation | ✅ Done |

**Files Created:**
- `src/content/document.rs` - Document struct, FileType enum, detect_file_type(), extract_title(), MAX_DOCUMENT_SIZE constant
- `src/tools/documents.rs` - import_document() LLM tool

**Files Modified:**
- `src/content/mod.rs` - Export document module
- `src/content/db.rs` - Document CRUD operations (insert_document, get_document, list_documents, delete_document)
- `src/db/schema.rs` - Migration v8: added filename, file_type, word_count columns
- `src/db/connection.rs` - Migration v7→v8 for document columns
- `src/tools/mod.rs` - Add documents module (feature-gated)
- `src/tools/registry.rs` - Register import_document tool (feature-gated)
- `src/chat/commands.rs` - Added CommandResult variants and parsing for /doc commands
- `src/chat/command_handlers.rs` - Added handlers for document commands (feature-gated)
- `Cargo.toml` - Added `document-tools` feature flag (default, included in all-tools)

**Commits:**
- PR #53 - Full implementation

**Reference:** `doc/src/development/planning-session-cli-tools.md` lines 151-156, 287-302

**Related:** Issue #9

---

### ✅ PRIORITY 3: Embedding Fallback for Oversized Content (COMPLETED)

**Status:** ✅ COMPLETED (v0.37.2)

**Goal:** Handle content that exceeds embedding model's context window.

**Original Problem (v0.37.1):** When embedding fails due to context overflow, the old `embed_with_fallback()` returned `Vec<Vec<f32>>` (multiple embeddings), but callers tried to insert all of them with the same `chunk_id`, causing PRIMARY KEY constraint violations.

**Bugs Discovered:**

1. **PRIMARY KEY Violation:** `chunk_embeddings.chunk_id` is PRIMARY KEY, so only ONE embedding per chunk. Old code tried to insert multiple.

2. **`has_embedding` Marked Incorrectly:** Even when embeddings failed, `has_embedding` was set to 1, preventing recovery on next startup.

3. **Dangling Chunks:** Chunks created in memory but never persisted to database.

**New Design (v0.37.2):**

```
embed_chunk_with_fallback(ctx, db, client, context_length, division_count)
    │
    ├─► Try client.embed(content)
    │       │
    │       ├─► Success → db.update_content_chunk_embedding() → return Ok
    │       │
    │       └─► Error: ContextExceeded → FALLBACK
    │
    └─► FALLBACK:
            │
            ├─► Check MAX_FALLBACK_DIVISIONS (4) - panic if exceeded
            ├─► Check MAX_CHUNKS_PER_ITEM (64) - panic if exceeded
            ├─► Check MIN_CHUNK_TOKENS (32) - panic if below
            │
            ├─► Divide content with halved config
            │
            ├─► db.transaction() - ATOMIC
            │       ├─► UPDATE chunk 0 content (first chunk)
            │       └─► INSERT chunks 1..N (new chunks)
            │
            └─► For each chunk: embed_chunk_with_fallback() recursively
```

**Key Changes:**

| Old (v0.37.1) | New (v0.37.2) |
|---------------|---------------|
| `embed_with_fallback() -> Vec<Vec<f32>>` | `embed_chunk_with_fallback(ctx) -> Result<EmbedResult, FallbackError>` |
| Multiple embeddings, same chunk_id | Creates new chunks atomically |
| Caller manages embeddings | Function manages chunks + embeddings |
| Silent failures with `let _ = ...` | Panics on limit exceeded (configuration error) |
| No transaction protection | Atomic transactions for chunk creation |

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Create `src/embeddings/fallback.rs` module | ✅ Done |
| 2 | Add `EmbedContext`, `EmbedItemContext` structs | ✅ Done |
| 3 | Add `embed_chunk_with_fallback()` | ✅ Done |
| 4 | Add `embed_item_with_fallback()` | ✅ Done |
| 5 | Add protection constants | ✅ Done |
| 6 | Simplify `client.rs` - remove old `embed_with_fallback()` | ✅ Done |
| 7 | Update `session.rs` callers | ✅ Done |
| 8 | Update `regenerate.rs` callers | ✅ Done |
| 9 | Update `recovery.rs` callers | ✅ Done |
| 10 | Update `command_handlers.rs` callers | ✅ Done |
| 11 | Add tests for fallback module | ✅ Done |
| 12 | Update documentation | ✅ Done |

**New Files:**
- `src/embeddings/fallback.rs` - Complete fallback logic with atomic transactions

**Modified Files:**
- `src/embeddings/client.rs` - Simplified, made `DEFAULT_CONTEXT_LENGTH` public
- `src/embeddings/mod.rs` - Export new module
- `src/chat/session.rs` - Use new fallback functions
- `src/embeddings/regenerate.rs` - Use new fallback functions
- `src/embeddings/recovery.rs` - Use new fallback functions
- `src/chat/command_handlers.rs` - Use new fallback functions

**Protection Constants:**
```rust
const MAX_FALLBACK_DIVISIONS: usize = 4;   // 512→256→128→64→32
const MAX_CHUNKS_PER_ITEM: usize = 64;      // Prevent DB explosion
const MIN_CHUNK_TOKENS: usize = 32;         // Minimum before aborting
```

**Related:** Issue #40, PR #46

---

### 🟠 PRIORITY 2: Context Overflow During Multi-Tool Execution (COMPLETED)

**Status:** ✅ COMPLETED (v0.37.0)

**Goal:** Prevent context overflow when LLM calls multiple tools in sequence AND fix infinite compaction loop caused by oversized summaries.

**Problems:**

1. **Multi-Tool Overflow:** Auto-compaction only happens BEFORE the first message. When tools execute sequentially, results accumulate in history without token checks. Large tool outputs (file reads, command outputs) can overflow context during multi-tool chains.

2. **Compaction Loop (Critical Bug):** Compaction summaries had no size limit. With 368 messages being summarized, the LLM generated ~18,000 token summaries, causing immediate re-compaction in an infinite loop.

**Root Cause Analysis:**
- Trigger was too late (95%+ context usage)
- No buffer reserved before overflow
- Summary had no token limit, generating massive summaries
- Template was generic, not structured for context preservation

**Solution:** Three-layer protection with percentage-based thresholds:

**Layer 1: Percentage-Based Compaction Triggers**
```rust
// Scales with context window size (32K, 128K, 200K)
MODERATE_USAGE_PERCENT = 0.75  // Warning at 75% (8K remaining for 32K)
CRITICAL_USAGE_PERCENT = 0.88  // Auto-compact at 88% (4K remaining for 32K)
INTER_TOOL_USAGE_PERCENT = 0.94 // Inter-tool warning at 94% (2K remaining)
EMERGENCY_USAGE_PERCENT = 0.97  // Emergency truncation at 97% (1K remaining)

// Absolute minimums for small contexts:
PRE_TOOL_MIN = 2_000 tokens
COMPACTION_MIN = 1_000 tokens
INTER_TOOL_MIN = 512 tokens
EMERGENCY_MIN = 256 tokens
```

**Layer 2: Structured Summary with Hard Limit**
```rust
MAX_SUMMARY_TOKENS = 3_000 tokens
Template: Goal, Instructions, Progress, Discoveries, Relevant Files
Auto-truncate if LLM ignores limit
```

**Layer 3: Inter-Tool Protection** (from Phase 1 implementation)
```rust
MODERATE_USAGE = 75%  → Warning before first tool
CRITICAL_USAGE = 88%   → Auto-compact threshold
INTER_TOOL_USAGE = 94% → Warning during tool execution
EMERGENCY_USAGE = 97%  → Truncate result as last resort
```

**Critical Token Calculation Bugs Fixed (v0.37.0):**

Three separate double-counting bugs were discovered and fixed:

1. **`calculate_context_metrics()` double-counted system + tools**
   - Comments said `real_history_tokens` was "history only"
   - But it's actually the TOTAL from Ollama's `prompt_eval_count`
   - Function was adding system + tools again, causing double-count
   - Fix: Use total directly, derive history by subtraction

2. **`needs_inter_tool_compaction()` and related functions**
   - Received `history_tokens + system_tokens` and summed them again
   - Fix: Accept single `total_tokens` parameter

3. **Pre-tool warning showed wrong remaining tokens**
   - Used `context_window - history_real_tokens()` 
   - Missing system + tools in remaining calculation
   - Fix: Use `total_tokens` from `ContextStatus`

4. **Pre-tool warning said "Auto-compacting..." but didn't compact**
   - Logic showed warning at 75%, called `auto_compact_if_needed()`
   - But `auto_compact_if_needed()` only compacts at 88%
   - Fix: Split logic - warning at 75%, compact at 88%

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add inter-tool context check (80% threshold) | ✅ Done |
| 2 | Add emergency truncation (90% threshold) | ✅ Done |
| 3 | Add `needs_inter_tool_compaction()` function | ✅ Done |
| 4 | Add `truncate_to_budget()` for emergency truncation | ✅ Done |
| 5 | Add `ContextNearLimit` and `ContextTruncated` events | ✅ Done |
| 6 | Add tests for new functions | ✅ Done |
| 7 | Add percentage-based thresholds | ✅ Done |
| 8 | Restructure `COMPACTION_PROMPT` with structured template | ✅ Done |
| 9 | Add summary truncation in `compact_conversation()` | ✅ Done |
| 10 | Update `auto_compact_if_needed()` to use percentage thresholds | ✅ Done |
| 11 | Add `needs_buffered_compaction()` function | ✅ Done |
| 12 | Fix `calculate_context_metrics()` double-counting | ✅ Done |
| 13 | Fix `needs_inter_tool_compaction()` signature | ✅ Done |
| 14 | Fix pre-tool warning remaining calculation | ✅ Done |
| 15 | Split warning vs compact logic in continuation.rs | ✅ Done |
| 16 | Remove duplicate warning in core.rs | ✅ Done |

**Files Modified:**
- `src/context_overflow.rs` - Percentage thresholds, `calculate_thresholds()`, fixed function signatures
- `src/tokens.rs` - Fixed `calculate_context_metrics()` to not double-count
- `src/chat/continuation.rs` - Split warning/compact logic, fixed remaining calculation
- `src/chat/core.rs` - Removed duplicate warning when tools enabled
- `src/chat/custom_coordinator.rs` - Updated function calls for new signatures
- `src/prompts/base.rs` - Restructured `COMPACTION_PROMPT` with structured template
- `src/utils.rs` - Added `truncate_to_budget()` for emergency truncation
- `tests/context_tool_overflow.rs` - Updated for percentage-based thresholds
- `tests/context_recovery_flow.rs` - Updated for percentage-based thresholds

**Constants:**
- `MODERATE_USAGE_PERCENT = 0.75` - Warning threshold (75%)
- `CRITICAL_USAGE_PERCENT = 0.88` - Auto-compact threshold (88%)
- `INTER_TOOL_USAGE_PERCENT = 0.94` - Inter-tool warning (94%)
- `EMERGENCY_USAGE_PERCENT = 0.97` - Emergency truncation (97%)
- `PRE_TOOL_MIN = 2_000` - Minimum buffer for warning
- `COMPACTION_MIN = 1_000` - Minimum buffer for compaction
- `INTER_TOOL_MIN = 512` - Minimum buffer for inter-tool
- `EMERGENCY_MIN = 256` - Minimum buffer for emergency
- `RESPONSE_MARGIN = 2_000` - Tokens reserved for model response
- `MAX_SUMMARY_TOKENS = 3_000` - Hard limit on summary size
- `DEFAULT_OVERFLOW_THRESHOLD = 0.75` - For display purposes

**New Compaction Template:**
```markdown
## Goal
[1-2 sentences: What is the user trying to accomplish?]

## Instructions
- [Important user constraints and preferences, max 3 items]

## Progress
**Completed:** [Work done, max 5 items]
**Pending:** [Work remaining, max 3 items]

## Discoveries
[Key insights learned, max 3 items]

## Relevant Files
- [Files read/edited/concerned, max 5 items]
- Root path: [Project root if relevant]
```

**Flow Implemented:**
1. Before first message: Check if context needs compaction (trigger at `context - COMPACTION_BUFFER`)
2. Between tools: Check if context > 80% → emit `ContextNearLimit` event
3. Emergency: If context > 90% → truncate result → emit `ContextTruncated` event
4. After compaction: Generate summary with template, truncate if > MAX_SUMMARY_TOKENS

**Research Sources:**
- OpenCode compaction.ts: `COMPACTION_BUFFER = 20,000`, structured template
- LangChain: Token-based triggers, summary best practices
- ask-ai-rs context: Zettelkasten and learning focus (smaller buffer than code agents)

**Note for Future:** When implementing parallel tool execution, the nudge mechanism via `continuation_prompt` should be reviewed to handle multiple concurrent tool completions.

**v0.37.0 Addition - Inter-Tool Compaction:**

Automatic context compaction during multi-tool execution (implemented in PR #45):

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `ChatEvent::ContextNeedsCompaction` | ✅ Done |
| 2 | Add `needs_compaction` flag to `ContextCheckResult` | ✅ Done |
| 3 | Modify `process_response()` to stop tool execution on compaction needed | ✅ Done |
| 4 | Add `OverflowHandleResult` enum for error classification | ✅ Done |
| 5 | Add automatic continuation loop in `handle_user_message()` | ✅ Done |
| 6 | Add MAX_COMPACTION_CYCLES limit (3) | ✅ Done |

**New Files/Functions:**
- `src/chat/custom_coordinator.rs`: Added `ChatEvent::ContextNeedsCompaction`, error string format with `CONTEXT_NEEDS_COMPACT:` prefix
- `src/chat/continuation.rs`: Added `OverflowHandleResult`, `is_inter_tool_compaction_error()`, `parse_inter_tool_compaction_error()`, `handle_inter_tool_compaction_error()`, `build_inter_tool_compaction_prompt()`
- `src/prompts/base.rs`: Added `CONTINUATION_PROMPT_INTER_TOOL` for continuation after compaction
- `src/chat/repl.rs`: Added `MAX_COMPACTION_CYCLES` constant (module level)

**Flow:**
1. During multi-tool execution, check if `remaining < COMPACTION_BUFFER` after each tool
2. If true, emit `ContextNeedsCompaction` event and return error string with `CONTEXT_NEEDS_COMPACT:` prefix
3. `handle_overflow_error()` detects the error, returns `OverflowHandleResult::InterToolCompaction`
4. `handle_user_message()` detects `InterToolCompaction`, compacts, sends continuation prompt
5. LLM continues automatically (max 3 compaction cycles per message)

**Refactoring (v0.37.0):**
- Removed unused `CoordinatorError` enum (never used)
- Removed unused `CompactionStats` struct and `compaction_stats()` method
- Removed unused `_threshold` parameter from `check_context_overflow()`
- Removed unused `_system_prompt` and `_use_debug` parameters from `auto_compact_if_needed()`
- Simplified `check_and_handle_context_overflow()` signature (removed `_tool_name`)
- Moved `MAX_COMPACTION_CYCLES` to module level in `repl.rs`

**Related:** Issue #43

---

### 🟣 PRIORITY 5: Feedback Infrastructure

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

### 🟡 PRIORITY 4: Specialized Agent Architecture

**Status:** ❌ NOT STARTED

**Goal:** Delegate specialized tasks (OCR, vision, document extraction, translation, summarization) to one-shot agents with optimized models.

**Problem:**
- OCR/Vision/Translate/Summarize are standalone CLI commands, not integrated with chat
- Document import calls `Command::new()` directly, bypassing skills system
- Skills can be overridden at project level, but tools don't respect overrides

**Architecture:**

```
┌──────────────────────────────────────────────────────────────────┐
│ Main Agent (conversational)                                     │
│ - Full context: history + memory + database                      │
│ - Tools: spawn_subagent, remember, fact_add, import_document...  │
│                                                                 │
│   LLM decides: "I need to extract text from PDF"                │
│   Tool call: spawn_subagent(type="document", prompt="...")       │
│         ↓                                                       │
│ ┌───────────────────────────────────────────────────────────┐  │
│ │ Specialized Agent (one-shot, no context/database)         │  │
│ │                                                           │  │
│ │ Type: "ocr"       → glm-ocr:bf16                          │  │
│ │ Type: "vision"    → moondream:1.8b                        │  │
│ │ Type: "translate" → translategemma:4b                     │  │
│ │ Type: "summarize" → (same model, specialized prompt)      │  │
│ │ Type: "document"  → (same model, uses run_command)        │  │
│ │                                                           │  │
│ │ Tools: Whitelisted per type (run_command, etc.)           │  │
│ │ Output: Tool result (no thinking)                         │  │
│ └───────────────────────────────────────────────────────────┘  │
│         ↓                                                       │
│ Result injected as tool output for main LLM                     │
└──────────────────────────────────────────────────────────────────┘
```

**Key Characteristics:**

| Aspect | Main Agent | Specialized Agent |
|--------|------------|-------------------|
| Context | Full history + memory + database | One-shot (no history) |
| Database | Yes (SQLite) | No |
| Thinking | Optional | Never (output only) |
| Output | Returns to user | Returns to Main Agent |
| Model | User's chat model | Configured per type |
| Skills | All available | Type-specific whitelist |

**Technical Debt Resolved:**
- Issue #9 (Document Import): `import_document` will use `spawn_subagent(type="document")`
- Issue #12 (OCR/Vision): Integrated via specialized agents

**Subagent Types:**

| Type | Model | Tools | Purpose |
|------|-------|-------|---------|
| `ocr` | glm-ocr:bf16 | run_command(tesseract) | Image text extraction |
| `vision` | moondream:1.8b | - | Image analysis |
| `translate` | translategemma:4b | - | Translation |
| `summarize` | (same model) | - | Summarization |
| `document` | (same model) | run_command(pdftotext) | PDF/EPUB extraction |

**Configuration:**

```toml
# ~/.config/ask-ai/models.toml

[subagents]
# Override default models for specialized agents
ocr = "glm-ocr:bf16"
vision = "moondream:1.8b"
translation = "translategemma:4b"
# summarization and document use main chat model
```

**Implementation Phases:**

| Phase | Description | Effort |
|-------|-------------|--------|
| 1 | Define `SubagentType` enum and `spawn_subagent` tool signature | 0.5 day |
| 2 | Create subagent coordinator (one-shot context, model routing) | 1 day |
| 3 | Implement OCR subagent (glm-ocr, tesseract tools) | 1 day |
| 4 | Implement Vision subagent (moondream, image processing) | 1 day |
| 5 | Implement Translate/Summarize subagents | 0.5 day |
| 6 | Implement Document subagent (PDF/EPUB, respects skill overrides) | 1 day |
| 7 | Configuration (models.toml) and user commands | 0.5 day |
| 8 | Testing and documentation | 1 day |
| **Total** | | **7 days** |

**User Commands:**

| Command | Description |
|---------|-------------|
| `/ocr <image>` | OCR via specialized agent |
| `/vision <image>` | Image analysis via specialized agent |
| `/translate <lang> <text>` | Translation via specialized agent |
| `/summarize <text>` | Summarization via specialized agent |

**Dependencies:** Skills System (completed v0.38.0)

**Related Issues:** #9, #12

**Estimated effort:** 5-7 days

---

### 🟡 PRIORITY 5: Parallel Tool Execution

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

**Related:** Issue #11

---

## 🔵 LOW PRIORITY: Extended Features

Features planned for future releases:

| Priority | Feature | Description | Dependencies | Issue |
|----------|---------|-------------|--------------|-------|
| P8 | File Session State | Explicit file tracking | None | #13 |
| P9 | Skills System Extended | Multilingual sanitization, security enhancements | Skills System, Specialized Agents | #14 |
| P10 | File Staleness | Detect outdated file content | None | #50 |
| P11 | Extended Personalities | Per-personality model config | None | #49 |
| P12 | Plugin System | User-defined tools | None | #15 |
| P13 | TUI | Ratatui-based terminal interface | None | #16 |
| P14 | Memory Enhancement 2-5 | Query routing, filtering | Doc Import | #17 |

**Note:** OCR/Vision Tools Integration was merged into Priority 4 (Specialized Agent Architecture).

---

### 🔵 PRIORITY 8: File Session State

**Status:** ❌ NOT STARTED

**Goal:** Explicit file tracking for session context.

**Related:** Issue #13

---

### 🔵 PRIORITY 10: Extended Personalities System

**Status:** ❌ NOT STARTED

**Goal:** Per-personality model configuration and separate memory context.

**Reason for Priority:** Didactic use case requires separate personalities soon.

**Current State (SOUL.md):**
- Multiple personality files supported via symlinks
- Symlink approach: `ln -sf ~/.config/ask-ai/SPRACH.md ~/.config/ask-ai/SOUL.md`

**What's MISSING:**
- Per-personality model configuration
- Separate memory context per personality
- Personality directory support (`personalities/`)

**Dependencies:** None

**Estimated effort:** 2-3 days

**Related:** Issue #49

---

### 🔵 PRIORITY 11: Multilingual Skill Sanitization

**Status:** ❌ NOT STARTED

**Goal:** Enhanced security for multilingual skill content.

**Background:**
- Skills System (P3) uses English-only sanitization
- Multilingual prompt injection can bypass English-based detection (documentated in research)
- Specialized Agent Architecture (P4) enables translate functionality within chat sessions

**Features:**

| Feature | Description | Dependency |
|---------|-------------|------------|
| **Language Detection** | Detect non-Latin characters, log warnings | None (can implement now) |
| **Translate-then-Detect** | Translate non-English content, then scan | P4 (Specialized Agents) |
| **ML Detection** | XLM-RoBERTa fine-tuned (optional) | ML infrastructure |
| **LLM-as-Critic** | Second LLM reviews before loading | Token costs |

**Implementation Phases:**

| Phase | Description | Dependency |
|-------|-------------|------------|
| 1 | Language detection + warning | None ✅ |
| 2 | Translate-then-detect | P4 |
| 3 | ML model (optional) | Future |

**Research:**
- HackerNoon: Multilingual prompt injection bypasses Azure Content Filter
- arXiv:2512.23684: Hidden prompt injection in 500 ICML papers
- arXiv:2410.21337v1: XLM-RoBERTa achieves 99% accuracy

**Dependencies:**
- Skills System (P3) ✅
- Phase 2 requires P4 (Specialized Agent Architecture)

**Estimated effort:** Phase 1: 2-3 hours | Phase 2: TBD

**Reference:** `doc/src/development/skills-system-design.md` → Future Considerations

**Related:** Issue #14

**Related:** Issue #14

---

### 🔵 PRIORITY 12: Skills Management Tool

**Status:** ❌ NOT STARTED

**Goal:** Allow LLM to create, modify, and delete skills automatically.

**Background:**
- Skills System (P3) provides read-only access via `skill_list()` and `skill_view()`
- LLMs often discover repeatable workflows that should be captured as skills
- Hermes Agent shows successful pattern with `skill_manage()` tool

**Scope (MVP):**

| Action | Parameters | Description |
|--------|------------|-------------|
| `create` | `name`, `content` | Create new skill with SKILL.md |
| `patch` | `name`, `old_string`, `new_string` | Find-and-replace in skill |
| `delete` | `name` | Remove skill directory |

**NOT in MVP:**
- `edit` (full rewrite) - use patch
- `write_file/remove_file` (supporting files) - references can wait
- Skills Hub integration (community skills) - users install manually
- Categories - can wait

**Architecture:**

```
src/skills/
├── manager.rs       # NOVO: create_skill, patch_skill, delete_skill
├── loader.rs        # ✅ load_skill_indexes, get_skill_content
├── sanitize.rs      # ✅ validate_skill_file, is_valid_skill_name
└── types.rs         # ✅ Skill, SkillIndex, Frontmatter

src/tools/skill_tools.rs
├── skill_list()     # ✅
├── skill_view()     # ✅
└── skill_manage()   # NOVO
```

**Security:**

1. Builtin skills are protected (cannot edit/delete)
2. `old_string` must be unique (or `replace_all=true`)
3. Name validation: `[a-z0-9_-]` only (no path traversal)
4. Atomic writes (tempfile + rename, never partial writes)
5. Frontmatter validation (name + description required)
6. Max size: 256KB (same as read)

**Directories:**

```
~/.config/ask-ai/skills/          ← User skills (writable)
PROJECT/.ask-ai/skills/           ← Project skills (writable)

Priority for writes: project > user (same as reads)
Priority for deletes: user only (cannot delete project from CLI)
```

**Estimated effort:** 3-4 hours

**Dependencies:** Requires P3 (Skills System) ✅ COMPLETED

**Related:** Issue #52

---

### 🔵 PRIORITY 13: File Staleness Detection

**Status:** ❌ NOT STARTED

**Goal:** Prevent file edits based on outdated content.

**Problem:**
When the LLM edits a file using `edit_file` or `write_file`, it may operate on outdated content if:
1. The file was modified externally (by another process, user, or git operations)
2. The LLM's context contains stale information about the file's structure

**Proposed Solution:**
- Track modification time (mtime) when a file is read
- Before edit operations, compare current mtime with stored mtime
- If different, return warning: "File has been modified since it was read."

**Dependencies:** None

**Estimated effort:** 1-2 days

**Related:** Issue #50

---

### 🔵 PRIORITY 14: TUI (Terminal User Interface)

**Status:** ❌ NOT STARTED

**Goal:** Build a responsive TUI using Ratatui-rs.

See `doc/src/development/roadmap.md` - TUI section for detailed implementation plan.

**Components:**
- Chat pane with markdown rendering
- Input pane with history
- Status bar showing model, context usage, tokens
- Sidebar for tools/messages (optional)

**Estimated effort:** 3-4 weeks

**Related:** Issue #16

---

### 🔵 PRIORITY 15: Plugin System

**Status:** ❌ NOT STARTED

**Goal:** Pluggable architecture for extending ask-ai functionality with external tools.

**Dependencies:** TBD

**Estimated effort:** TBD

**Related:** Issue #15

#### Background: Opt-in Tools

The current architecture supports **feature flags** for optional tools:

- `pokemon-tools` - Opt-in (not in default build)
- `led-tools` - Opt-in (not in default build)
- `finance-tools` - Opt-in (not in default build)
- `search-tools` - Opt-in (not in default build)

This is a precedent for the plugin system: tools that require:
- External APIs (PokéAPI, Google Finance)
- Specific hardware (LED control)
- Opt-in due to size/complexity

#### Architecture Direction

The Plugin System should support two paradigms:

**1. Native Plugins (Rust WASM/WebAssembly)**

```rust
// Future: ./plugins/my_plugin.wasm
pub fn register_tools(registry: &mut ToolRegistry) {
    registry.register(MyTool::new());
}
```

**2. MCP (Model Context Protocol) Support**

MCP is an open standard for connecting AI applications to external systems:

- **Standardized interface**: JSON Schema for tool definitions
- **Server-based**: External processes provide tools via MCP protocol
- **Dynamic discovery**: Tools are listed at runtime, not compile-time
- **Security**: Human-in-the-loop for sensitive operations

**Reference:** https://modelcontextprotocol.io

**Example MCP Tool Definition:**
```json
{
  "name": "get_weather",
  "description": "Get current weather for a location",
  "inputSchema": {
    "type": "object",
    "properties": {
      "location": { "type": "string" }
    },
    "required": ["location"]
  }
}
```

#### Research Summary

| System | Approach | Type Safety | Security |
|--------|----------|-------------|----------|
| MCP | JSON Schema + server | Runtime validation | Human approval |
| AI SDK (Vercel) | Zod Schema + execute | Compile-time | Needs approval |
| Hermes Agent | Skills (Markdown) + Tools (Rust) | Compile-time for tools | Sanitization |
| **ask-ai (current)** | Rust code + feature flags | Compile-time | Blacklist |

#### Implementation Phases

**Phase 1: MCP Client Integration**
- Implement MCP client to connect to external tool servers
- Support `tools/list` and `tools/call` operations
- Human confirmation UI for tool invocations

**Phase 2: Native Plugin System**
- WASM module loading with sandbox
- Plugin registry API
- Hot-reload support

**Phase 3: Plugin Distribution**
- Plugin discovery mechanism
- Version management
- Security scanning

#### Why Not Generic HTTP Tool

A generic `http_request` tool has been considered and **rejected** for these reasons:

1. **Security**: No input sanitization, can call ANY URL
2. **Type Safety**: LLM must infer JSON schemas from responses
3. **Error Handling**: Runtime errors only, no compile-time validation
4. **Complexity**: LLMs struggle with complex nested APIs without typed schemas

The industry standard (MCP, Claude Code, etc.) uses **typed tool schemas**, not raw HTTP.

#### References

- [Model Context Protocol](https://modelcontextprotocol.io)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [Vercel AI SDK Tools](https://sdk.vercel.ai/docs/ai-sdk-core/tools-and-tool-calling)
- [OWASP LLM Top 10](https://genai.owasp.org/llm-top-10/)

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

2026-03-26 - v0.38.0: Pokemon tools removed from default, Plugin System + MCP documentation