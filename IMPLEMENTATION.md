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

**v0.40.0** - 2026-03-29 (Document Import Tool)

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

### Milestones

| Milestone | Codename | Description | Priorities |
|-----------|----------|-------------|------------|
| **[M1]** | Core Evolution | All work before Sprach 2.0 | P0-P6, P8-P13 |
| **[M2]** | UX & TUI Design | TUI design, UX research, prototyping, private feedback | P14 (UX design phase) |
| **[M3]** | Sprach 2.0 | CAS research, cognitive extensions, TUI implementation | P7 (S2.1-S2.6), P14 (implementation), P15 |
| **[M4]** | Future | Deferred, no current priority | Cost tracking, team features, speculation, VCR |

**M2 rationale:** The TUI is the milestone that will likely coincide with a public release. It warrants dedicated UX research, private feedback rounds, and careful design before implementation. Separating design (M2) from implementation (M3) ensures the TUI gets the attention it deserves as a public-facing product, while Sprach 2.0 research and Plugin System (also complex) move to M3 alongside TUI coding.

### ✅ PRIORITY 0: Factual Memory System (COMPLETED) [M1]

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

### ✅ PRIORITY 0: TODO System Activation (COMPLETED) [M1]

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

### ✅ PRIORITY 1: Enhance Todo Tools — CRUD Gaps, Priority, and Tags (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Fix technical debt in todo tools by adding missing CRUD operations, priority levels, and tags/categories.

**Implementation Summary:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1.1 | Add `todo_get(id)` tool | ✅ Done |
| 1.2 | Add `todo_delete(id)` tool | ✅ Done |
| 1.3 | Add `todo_edit(id, description?, priority?, tags?)` tool | ✅ Done |
| 1.4 | Register new tools in registry | ✅ Done |
| 1.5 | Add tool descriptions to prompts | ✅ Done |
| 1.6 | Update slash commands and handlers | ✅ Done |
| 2.1 | Add `Priority` enum | ✅ Done |
| 2.2 | Add `tags: Vec<String>` to `Task` | ✅ Done |
| 2.3 | Extend `todo_add(description, priority?, tags?)` | ✅ Done |
| 2.4 | Extend `todo_edit(id, description?, priority?, tags?)` | ✅ Done |
| 2.5 | Extend `todo_list(filter?)` with filtering | ✅ Done |
| 2.6 | Extend `format_list_filtered()` for priority/tags | ✅ Done |
| 2.7 | DB migration v8→v9 for `priority` and `tags` columns | ✅ Done |
| 2.8 | Update `to_rows()`/`from_rows()` | ✅ Done |
| 2.9 | Update prompts and docs | ✅ Done |
| 2.10 | Manual tests | ✅ Done |
| 2.11 | Smoke test | ✅ Done (63/64 pass, 1 skipped) |
| 2.12 | Bug fix: error messages for /todo edit/get/delete without args | ✅ Done |
| 2.13 | Refactor: extract `parse_todo_subcommand`, remove YAGNI code | ✅ Done |

**Key files:** `src/chat/todo_state.rs`, `src/tools/todo.rs`, `src/db/connection.rs`, `src/db/operations.rs`, `src/db/schema.rs`, `src/tools/registry.rs`, `src/chat/commands.rs`, `src/chat/command_handlers.rs`, `src/prompts/tools.rs`

**Closes:** Issue #66 via PR #82

---

### ✅ PRIORITY 1: Code Quality - Prompts Centralization (COMPLETED) [M1]

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

### ✅ PRIORITY 1: Code Quality - run_chat_repl Complexity (COMPLETED) [M1]

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

### ✅ PRIORITY 4: Code Quality - query.rs Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.40.0)

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

### ✅ PRIORITY 4: Code Quality - context_builder.rs Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED

**Goal:** Reduce cognitive complexity of `build_context` from 27/25 to <25/25.

**Context:** Retrieval context building function in `src/retrieval/context_builder.rs`.

**Analysis:**
- Function `build_context` (lines 180-378) had complexity 27/25
- Complexity sources:
  1. Nested `if let` in retrieval logic (4 levels deep)
  2. Repeated `match msg.role` blocks (same pattern twice)
  3. Multiple `if use_debug` scattered throughout

**Implementation:**

| Phase | Task | Status |
|-------|------|--------|
| 1 | Extract `push_messages_as_chat_messages()` helper + tests | ✅ Done |
| 2 | Extract `RetrievalResult` struct + `perform_retrieval()` | ✅ Done |
| 3 | Add `log_if_debug!` macro + refactor both functions | ✅ Done |
| 4 | Run tests and clippy, verify complexity < 25/25 | ✅ Done |

**Files Modified:**
- `src/retrieval/context_builder.rs` - Added helper functions, macro, tests

**Complexity Reduction:**
- Before: 27/25 (flagged by clippy)
- After: No clippy warning (complexity below threshold)

**Commits:**
- `c46d12c` refactor(context_builder): extract push_messages_as_chat_messages helper (Phase 1)
- `ed83e21` refactor(context_builder): extract perform_retrieval helper (Phase 2)
- `0abb06b` refactor(context_builder): add log_if_debug macro (Phase 3)

**Related:** Issue #30

---

### ✅ PRIORITY 4: Code Quality - registry.rs Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED (Issue #31)

**Goal:** Reduce cognitive complexity of `register_tools` from 56/25 to <25/25.

**Context:** Tool registration function - largest complexity in codebase.

**Bugs Discovered During Analysis:**

| # | Bug | Description | Fix |
|---|-----|-------------|-----|
| B1 | `finance-tools` missing | `get_available_tool_names()` didn't include `get_stock_quote` | Added `finance-tools` block |
| B2 | `web_scrape` condition mismatch | Different `#[cfg]` conditions | Unified to `#[cfg(feature = "search-tools")]` |
| B3 | `test_tool` ignores blacklist | Always registered | Added blacklist check |

**Design Decision:** During review, we discovered that `todo-tools` was incorrectly feature-gated. Since `TodoState` is always part of `ChatSession`, todo tools should be built-in (like facts and notes).

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Create branch, update docs (with bugs) | ✅ Done |
| 2 | Fix bug B1: finance-tools in get_available_tool_names | ✅ Done |
| 3 | Fix bug B2: web_scrape condition | ✅ Done |
| 4 | Fix bug B3: test_tool blacklist check | ✅ Done |
| 5 | Extract 13 `register_*_tools()` helpers | ✅ Done |
| 6 | Extract 13 `get_*_tool_names()` helpers | ✅ Done |
| 7 | Refactor `register_tools()` | ✅ Done |
| 8 | Refactor `get_available_tool_names()` | ✅ Done |
| 9 | Run tests and clippy | ✅ Done |
| 10 | Make todo-tools built-in (remove feature gates) | ✅ Done |

**Complexity Reduction:**

| Function | Before | After |
|----------|--------|-------|
| `register_tools` | 56/25 | <25/25 (no warning) |
| `get_available_tool_names` | ~30/25 | <25/25 (no warning) |

**Files Modified:**
- `src/tools/registry.rs` - Extracted 26 helper functions, 2 macros, refactored main functions
- `src/tools/mod.rs` - Removed `todo-tools` feature gates
- `src/macros.rs` - Added `log_if_debug!` macro
- `src/retrieval/context_builder.rs` - Use shared macro
- `src/prompts/tools.rs` - Removed `todo-tools` feature gate
- `Cargo.toml` - Removed `todo-tools` from default and all-tools features

**Commits:**
- `f2884d7` docs: update CHANGELOG and IMPLEMENTATION with bug fixes for Issue #31
- `05c3639` refactor: reduce registry.rs cognitive complexity (Issue #31)
- `fcdcd9e` docs: mark Issue #31 as completed
- `7995956` docs: add Issue #63 to roadmap (notes tools missing)
- `4404bf9` fix: apply PR review feedback
- `3a86403` fix: make todo-tools built-in (remove feature gates)

**Related:** Issue #31, PR #62

---

### 🔵 PRIORITY 4: Code Quality - commands.rs Complexity (parse_command) [M1]

**Status:** ✅ COMPLETED (PR #84, ready for review)

**Goal:** Reduce cyclomatic complexity of `parse_command` from ~450 lines to manageable size, eliminate `CommandResult` enum duplication, and remove session subcommand duplication.

**Context:** `src/chat/commands.rs` (1919 lines). Five problems identified:

1. **Monolithic `parse_command`** — 44 match arms, ~645 lines of match code
2. **16 shortcut duplicates** — `/fa`, `/na`, `/di`, etc. copy 100% of parent subcommand logic (~135 lines)
3. **Two mirror enums** — `ChatCommand` and `CommandResult` with 23+ identical variants
4. **30 pass-through variants** in `execute_command` — no logic, just wrapping ChatCommand → CommandResult
5. **Session duplication** — `ChatCommand::Session` duplicates `New/Load/List/Save/Forget` (~151 lines)

**Implementation Phases:**

| Phase | Description | Lines Removed | Status |
|-------|-------------|---------------|--------|
| 1.1 | Extract `parse_fact_subcommand()` | ~70 (shortcut dedup) | ✅ Done |
| 1.2 | Extract `parse_note_subcommand()` | ~60 (shortcut dedup) | ✅ Done |
| 1.3 | Extract `parse_doc_subcommand()` | ~42 (shortcut dedup) | ✅ Done |
| 1.4 | Extract `parse_session_subcommand()` | ~13 (shortcut dedup) | ✅ Done |
| 1.5 | Consolidate 2-letter shortcuts as delegates | ~135 | ✅ Done |
| 1.6 | Add unit tests for extracted parsers | +490 (76 tests) | ✅ Done |
| 2 | Eliminate `CommandResult` enum, move execute logic to `command_handlers.rs` | ~321 | ✅ Done |
| 3 | Eliminate `SessionSubcommand` duplication | ~49 | ✅ Done |

**Estimated total reduction:** ~462 lines (1919 → ~1457)

**Files Modified:**
- `src/chat/commands.rs` — Extract parsers, delete `CommandResult`, delete `execute_command`, delete `SessionSubcommand`
- `src/chat/command_handlers.rs` — Absorb `execute_command` logic, create `handle_command()` using `ChatCommand`
- `src/chat/repl.rs` — Replace `execute_command + handle_command_result` with `handle_command`

**Branch:** `refactor/parse-command-complexity`
**PR:** #84 (ready for review)

**Commits:**
- `b5df9f0` docs: update CHANGELOG and IMPLEMENTATION.md for parse_command refactoring
- `e2b9e35` refactor: extract group parsers and consolidate 2-letter shortcuts
- `a5c2d80` refactor: eliminate CommandResult enum, add handle_command to command_handlers
- `bd8b927` refactor: eliminate SessionSubcommand enum and ChatCommand::Session variant
- `e226374` test: add unit tests for extracted subcommand parsers
- fix: remove /f shortcut from /forget, move to /search (collision causing data loss)
- fix: add missing /todo shortcuts (/tg, /te, /td, /tcd, /tca)

**Bugs found during manual testing (fixed):**
- `/f` was mapped to `/forget` instead of `/search` — collision causing accidental data loss
- Missing `/todo` shortcuts for get, edit, delete, clear-done, clear-all

**Pre-existing bugs (NOT from PR, separate issues):**
- Session save/load persistence (1.3, 1.5) — `/session save` reports success but data not found by `/session list`
- FTS schema mismatch (1.7) — `content_fts` table missing `conversation_id` column (FIXED in PR #87)
- FOREIGN KEY constraint on todos — session save FK warning on todo mutations (FIXED in PR #87)

**Estimated effort:** 2-3 days

**Related:** Issue #35

---

### ✅ PRIORITY 5: UX - `/forget --yes` Confirmation [M1] (COMPLETED)

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Require explicit confirmation for `/forget` command to prevent accidental data loss. ✅ **COMPLETED**

**Problem:**
- `/forget` is the most destructive command — it deletes the entire conversation from the database
- Previously executed immediately with no confirmation
- A typo (e.g., `/forget` instead of `/forgets`) could destroy hours of conversation
- The `/f` shortcut was previously mapped to `/forget`, causing accidental data loss (fixed in PR #84)

**Implementation:**
- ✅ `/forget` without `--yes` → warn: "This will permanently delete this conversation. Use /forget --yes to confirm."
- ✅ `/forget --yes` → execute the forget operation
- ✅ No shortcuts for `/forget` (already enforced in PR #84)
- ✅ `ChatCommand::Forget` became `ChatCommand::Forget { confirmed: bool }`
- ✅ Parser validates `--yes` flag, rejects invalid arguments
- ✅ FK constraint bug in `save_sqlite()` fixed — `ensure_conversation_exists()` added

**Related:** Issue #85 (CLOSED via PR #87)

---

### ✅ PRIORITY 5: UX - `/skill <name>` Subcommand [M1] (COMPLETED)

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Move skill activation from `/<skill-name>` to `/skill <name>` to prevent namespace collisions. ✅ **COMPLETED**

**Problem:**
- Skills were previously activated as top-level commands (e.g., `/document-processing`)
- Any skill name could collide with existing commands (e.g., a skill named "forget", "new", "help")
- No clear separation between built-in commands and user-defined skills
- The wildcard `_` match arm processed skill names last, making collision behavior unpredictable

**Implementation:**
- ✅ `/skill <name>` is now the explicit command to activate a skill
- ✅ `/skill` (no args) lists available skills (`ChatCommand::SkillList`)
- ✅ `/sk` is a shortcut for `/skill`
- ✅ `/<skill-name>` wildcard removed — unknown commands are now invalid (not skill activations)
- ✅ `/skill list` attempts to activate a skill named "list" — no reserved words
- ✅ Help text updated

**Related:** Issue #86 (CLOSED via PR #87)

---

### ✅ PRIORITY 5: Code Quality - Replace Debug Logs with `log` Crate + Verbosity System [M1]

**Status:** ✅ COMPLETED (v0.40.0)

**Goal:** Simplify verbosity system to 4 levels, remove debug mode, and integrate with REPL.

**Motivation:**
- **Simplified UX** - Most users only need 2 levels (normal and verbose)
- **Clearer semantics** - 4 levels are easier to understand than 5
- **Cleaner code** - Removed debug-specific logic and `debug_default` config

**Resolved Design Decisions:**

| Aspect | Old Design | New Design |
|--------|-----------|------------|
| Verbosity Levels | 5 (Quiet, Normal, Verbose, Debug, Trace) | 4 (Quiet, Normal, Verbose, Trace) |
| Normal Level | `warn` | `info` (shows tool calls) |
| Verbose Level | `info` | `debug` (shows tool calls + results) |
| Debug Level | `-vv` → `debug` | Removed (now verbose) |
| Trace Level | `-vvv` → `trace` | `-vv` → `trace` (replaced debug) |
| Debug Flag | `-d/--debug` (dry-run) | Removed |
| Verbose Flags | `-v`/`-vv`/`-q` | `-v` (verbose), `-vv` (trace) |
| Debug Toggle | `/debug` command | `/debug` command (Normal ↔ Trace) |
| `debug_default` | Config option | Removed |
| Rustyline Debug | Shown in normal mode | Always suppressed |
| Quiet Mode | Suppresses only warnings | Also suppresses spinners |
| `use_debug` Param | Passed to many functions | Removed from all functions |
| `Verbosity::Debug` | Exists | Removed |
| Future TUI | stderr logging | Logging to file instead |

| Verbosity | Flag | Log Level | Behavior |
|-----------|------|-----------|----------|
| Quiet | (none) | `error` | Only errors, no spinners |
| Normal | (default) | `info` | Tool calls visible + errors |
| Verbose | `-v` | `debug` | Tool calls + results + internal state |
| Trace | `-vv` | `trace` | Everything (including embedding distances, token budgets) |

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Simplify Verbosity enum (4 levels, remove Debug) | ✅ Completed |
| 2 | Update logging.rs - Verbosity struct with 4 variants | ✅ Completed |
| 3 | Update Rustyline input - Always suppress debug output | ✅ Completed |
| 4 | Update quiet mode - Spinners suppressed | ✅ Completed |
| 5 | Remove `debug_default` from config | ✅ Completed |
| 6 | Update `/debug` command - Toggle Normal ↔ Trace | ✅ Completed |
| 7 | Remove `use_debug` parameter from ALL functions | ✅ Completed |
| 8 | Update `/debug` DB error message (remove debug reference) | ✅ Completed |
| 9 | Remove `dbg!()` macro | ✅ Completed |
| 10 | Update tool call format - `🔧 name(args)` (no "Calling:") | ✅ Completed |
| 11 | Chat interactive mode ignores quiet flag | ✅ Completed |
| 12 | Tests & clippy & documentation | ✅ Completed |

**Files Created:**
- `src/logging.rs` — Logging initialization, Verbosity enum (4 levels), init(), set_verbosity(), 6 unit tests

**Files Modified:**
- `Cargo.toml` — Updated dependencies
- `src/main.rs` — Removed `-d/--debug` flag,简化 `-v`/`-vv` flags
- `src/lib.rs` — Added `pub mod logging`
- `src/chat/cli.rs` — Updated verbosity flags
- `src/chat/repl.rs` — Quiet mode handling, removed debug banners
- `src/chat/input/rustyline.rs` — Always suppress debug output
- `src/chat/command_handlers.rs` — `/debug` command syncs log level, not use_debug
- `src/db/connection.rs` — DB error message update (no debug reference)
- `src/settings.rs` — Removed `debug_default`, `debug_tools`, `verbosity` types updated

**Related Issues:**
- Issue #60 — Replace log_debug with log crate
- Issue #61 — Bug: `--debug` flag is dry-run mode, not debug logging
- Issue #87 — Simplify verbosity to 4 levels
- Issue #88 — Remove debug mode, update `/debug` command

---

### ✅ PRIORITY 4: Code Quality - Dead Code Cleanup (COMPLETED) [M1]

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

### ✅ PRIORITY 4: Status Bar Above Prompt (COMPLETED) [M1]

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

### 🔵 PRIORITY 4: Code Quality - Notes System (COMPLETED) [M1]

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

### ✅ PRIORITY 3: Bug - Notes LLM Tools Missing [M1]

**Status:** ✅ COMPLETED

**Issue:** #63

**PR:** #64

**Summary:** Only `note_add` exists as LLM tool. LLM cannot edit or delete notes it creates.

**Design Decision:** Only `note_edit` and `note_delete` are needed. Other operations are covered by existing `remember` tool:
- `note_list` → `remember(query)` discovers notes
- `note_show` → `remember(id="note:N")` returns full note content
- `note_search` → `remember(query)` searches across notes, docs, messages

**Implementation:**
- Added `note_edit(id, title?, content?)` and `note_delete(id)` to `src/tools/notes.rs`
- Added `parse_note_id()` helper (accepts "42" and "note:42" formats)
- Registered tools in `src/tools/registry.rs`
- Updated prompts in `src/prompts/tools.rs`
- Commits: `c809a76`, `e847288`, `f795e4e`, `b98adf9`, `80a6acf`

**Also included in PR #64:**
- Braille art welcome banner (replaced jp2a ASCII art)
  - 14-line colored braille art from extended-mind.png (width 39)
  - Reordered session info by importance: Model, Server, Tools, Think, Vision, Sandbox, Project, Session, Version
  - Added Skills count line (shown when tools enabled and skills > 0)
  - Expanded `WelcomeInfo` from 7 to 13 fields (added skill_count)
  - Separate Facts/Notes/Docs count lines
  - "Ollama" label renamed to "Server"
  - Removed embed_model from banner
  - Added `count_facts()`, `count_notes()`, `count_documents()` to Database
- Fix: config.toml model settings in summarize/vision subcommands (Issue #65)

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `parse_note_id()` helper (accepts "42" or "note:42") | ✅ Done |
| 2 | Add `note_edit(id, title?, content?)` tool | ✅ Done |
| 3 | Add `note_delete(id)` tool | ✅ Done |
| 4 | Register tools in registry.rs | ✅ Done |
| 5 | Update prompts/tools.rs | ✅ Done |
| 6 | Build, test, clippy | ✅ Done |
| 7 | Braille art banner | ✅ Done |
| 8 | Fix config.toml model in summarize/vision | ✅ Done |

**Also included in PR #64:**
- Braille art welcome banner (replaced jp2a ASCII art)
- Fix: config.toml model settings in summarize/vision subcommands (Issue #65, commit `aa0744b`)

**Estimated effort:** 0.5-1 day

---

### ✅ Bug: summarize/vision ignoring config.toml model settings (COMPLETED)

**Status:** ✅ COMPLETED

**Issue:** #65

**PR:** #64

**Summary:** `summarize` and `vision` subcommands were falling back to hardcoded `qwen3.5:4b` instead of respecting `config.toml` model settings.

**Root Cause:** Both subcommands called `ModelConfig::default()` instead of `resolve_model_config()`.

**Fix:** Changed to use `resolve_model_config()` which reads from CLI flag → config.toml → hardcoded fallback.

**Commit:** `aa0744b`

---

### ✅ PRIORITY 3: Skills System (COMPLETED) [M1]

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

### ✅ PRIORITY 3: Document Import Tool (COMPLETED) [M1]

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

### ✅ PRIORITY 3: Embedding Fallback for Oversized Content (COMPLETED) [M1]

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

### 🟣 PRIORITY 5: Feedback Infrastructure [M1]

**Status:** ✅ COMPLETED (merged PR #98)
**Related Issue:** #23
**Detailed Plan:** [`doc/src/development/feedback-architecture.md`](./doc/src/development/feedback-architecture.md) — feedback-driven memory with active forgetting (architecture, formulas, and data model)

**Goal:** Implement a complete feedback-driven memory system: capture explicit feedback signals (Good/Bad/Correction) with decay-weighted RRF fusion for retrieval ranking, activate content item decay (ghost fields become functional), and connect feedback to forgetting speed. Feedback is harness-only (no fine-tuning) — signals affect RRF fusion scoring AND content importance/decay, not model weights.

**Key Insight:** Feedback improves *how we retrieve* past messages. Factual Memory provides *what we know* about the user. Both layers work together:

```
Context Assembly:
├── System Prompt
│   └── [FACTUAL MEMORY] ← "User prefers Portuguese"
│       "Docs are in ~/docs"
├── Retrieved Context (messages)
│   └── [FEEDBACK WEIGHT] ← Message #42: +1.0 (good, decayed)
│       Message #15: -1.0 (bad, decayed)
│       RRF multiplier: clamp(0.1, 3.0)
│   └── [CONTENT DECAY] ← Message #42: importance=0.55 (good feedback +0.05)
│       Message #15: importance=0.30 (bad feedback -0.1 → pruned sooner)
│       access_count: 12 (retrieved 12 times → reinforced)
└── Response
```

#### Architecture Decision Records (ADRs)

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-001 | Feedback is harness-only (no fine-tuning) | No GPU, no training pipeline. RAG/ICL/BoN are valid inference-time methods (Krishnamurthy 2026). |
| ADR-002 | Decay formula: `2^(-t/half_life)` | Aligns with existing facts system (`src/facts/decay.rs`). `exp(-t/h)` is equivalent but confusing; `2^(-t/h)` matches Ebbinghaus curve already in code. |
| ADR-003 | Messages-only scope is Phase 1 (not permanent) | When Unified Knowledge Store ships, `feedback_signals.item_id` can reference `knowledge_items.id`. Migration: v10→messages, v11+→all sources. |
| ADR-004 | LLM self-feedback = 30% weight | Self-approval bias defense. Wu et al. (2025): self-verification consistently beaten by majority voting. Chan et al. (2025): ~3% decisions change per reflection step. Configurable via `config.toml [feedback].llm_feedback_weight`. |
| ADR-005 | Good=+1.0, Bad=-1.0, Correction=+1.0 | Binary-like symmetric signals (no partial credit). Drori et al. (2025): strict 0/1 verification. Granularity comes from temporal decay, not base_value. Correction value is in metadata text, not numerical weight. |
| ADR-006 | Score clamping: `.clamp(0.1, 3.0)` | Original `.max(-0.9).min(2.0)` allowed negative scores (bug: `1.0 + (-2.0) = -1.0 → max(-1.0, -0.9) = -0.9`). New clamp: min 0.1 (90% max suppression), max 3.0 (3× amplification cap). |
| ADR-008 | Content Decay Activation | `content_items` ghost fields activated: `decay_score`/`access_count`/`last_accessed` now functional with Ebbinghaus decay. Content-type half-lives: messages=90d, notes=60d, documents=120d. Feedback adjusts importance (good +0.05, bad -0.1), creating a forgetting loop. |
| ADR-009 | Retrieval Reinforces Retention | `on_content_access()` called on retrieval — increments `access_count`, updates `last_accessed`. Same pattern as facts system. RRF (immediate ranking) and access_count (future retention) are separate signals — not double-counting. |

#### Key Corrections from Original Plan

| Item | Original (implementation-directive.md) | Corrected (v2 plan) | ADR |
|------|---------------------------------------|---------------------|-----|
| Bad base_value | -0.5 | **-1.0** | ADR-005 |
| Correction base_value | 1.2 | **1.0** | ADR-005 |
| Decay formula | `exp(-t/h)` | **`2^(-t/h)`** | ADR-002 |
| LLM feedback weight | 1.0 (same as user) | **0.3 (30% discount)** | ADR-004 |
| RRF score clamping | `.max(-0.9).min(2.0)` | **`.clamp(0.1, 3.0)`** | ADR-006 |
| `/fc` shortcut | Present | **Removed** (correction always needs text) | — |

#### Key Corrections from V3

| Item | V3 | V4 | ADR |
|------|----|----|-----|
| "NO modification of content_items" | Explicit guardrail | REMOVED — feedback adjusts importance | ADR-008 |
| Content decay | Not addressed | Activated — all content_items decay | ADR-008 |
| access_count = 0 forever | Implicit limitation | Fixed — on_content_access() on retrieval | ADR-009 |
| Feedback → importance | Explicitly forbidden | Changed — good/bad adjusts importance | ADR-008 |

#### Implementation Phases

| Phase | Description | Effort | Key Correction | Status |
|-------|-------------|--------|----------------|--------|
| 1.1 | `/feedback` command + schema | 2 days | ADR-005 values; `/fc` removed | ✅ Done |
| 1.2 | Weight propagation | 1 day | — | ✅ Done |
| 1.3 | `/context` enhancement | 0.5 day | — | ✅ Done |
| 1.4 | Implicit signal capture | 1 day | — | ✅ Done |
| 1.5 | Weighted retrieval | 3 days | — | ✅ Done |
| 1.6 | Decay implementation | 1 day | `2^(-t/h)` + LLM 30% discount | ✅ Done |
| 1.7 | Content decay module | 2 days | ADR-008: Ebbinghaus for content_items | ✅ Done |
| 1.8 | Access tracking + importance adj. | 2 days | ADR-009: retrieval reinforces retention | ✅ Done |
| 1.9 | Decay cycle integration | 1 day | Startup trigger + /content prune | ✅ Done |
| **Total** | | **13.5 days** | | |

**Reserved Code (Phase 2):** The following functions in `src/feedback/prompt.rs` are implemented and tested but not yet wired into production. They are reserved for Phase 2 (Feedback-Aware Retrieval) and are documented with `#[allow(dead_code)] // Reserved for Phase 2`:

| Function | Purpose | Expected Use |
|----------|---------|-------------|
| `compute_feedback_boost_map()` | Struct-based version of boost computation using `Database` type directly | Phase 2 RRF fusion in `search_content_hybrid()` — will replace the direct `db::feedback_ops::compute_feedback_boost()` call |
| `build_feedback_section()` | Format feedback stats for `/context` display | Phase 2 `/context` enhancement — will replace inline formatting in `command_handlers.rs:1892-1916` |
| `build_decay_section()` | Format decay stats for `/context` display | Phase 2 `/context` enhancement — same as above |

**Boost Computation API Difference:** Two versions exist by design:
- `db::feedback_ops::compute_feedback_boost()` — DB-query-based, iterates rows directly. **Production (Phase 1).**
- `feedback::prompt::compute_feedback_boost_map()` → `feedback::decay::compute_total_boost()` → `decayed_weight()` — Struct-based, loads `FeedbackSignal` structs first. **Phase 2.** More composable when retrieval modules already have structs loaded.

Both use the same canonical decay formula via `feedback::decay::decayed_weight_raw()` (ADR-002).

Additionally, `src/feedback/decay.rs` provides the canonical decay computation:
- `decayed_weight_raw()` — Single point of calculation using unix timestamps with fractional-day precision
- `decayed_weight()` — Wrapper with `DateTime<Utc>` API (reserved for Phase 2)
- `compute_total_boost()` — Accumulates weights with first-stage clamping (reserved for Phase 2)

**Future Refactoring Note:** `facts/decay.rs` and `content/decay.rs` share an identical structural pattern (constants for half-lives, `compute_retention()`, `should_prune()`). A future refactoring could extract a shared `Decayable` trait or common `decay` module to eliminate this duplication.

**Sprach 2.0 Note:** The article's "Learned Personality" proposal (S2.5 — SOUL.md patching) overlaps with but extends P5. P5 captures *what happened* (feedback signals for retrieval weighting); S2.5 adjusts *who I am* (personality modification with human approval). Both are complementary.

---

### ✅ PRIORITY 2: Context Continuation (COMPLETED) [M1]

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

### ✅ PRIORITY 1: PreToolContent Persistence & Context Enrichment (COMPLETED) [M1]

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

### ✅ PRIORITY 1: SOUL.md - AI Personality System (COMPLETED) [M1]

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
- **Sandbox always enforced** for all file operations (cannot be disabled)
- **Blocked patterns** for sensitive files (`.env`, `secrets`, `.pem`, etc.)
- **5MB size limit** per operation
- **Atomic writes** (temp file + rename) to prevent corruption
- **UTF-8 validation** - reject binary content
- **`/tmp` and `/var/tmp`** allowed for tool interoperability

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

### ✅ PRIORITY 3: Code Quality - run_chat_repl Refactoring (COMPLETED) [M1]

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

### ✅ PRIORITY 4: Specialized Agent Architecture [M1]

**Status:** ✅ COMPLETED (v0.41.0)

**Implementation:**
- Created `src/chat/subagent.rs` - `SubagentRunner` for one-shot execution with dual API path support
- Added `spawn_subagent` tool in `src/tools/spawn_subagent.rs` - LLM-initiated subagent invocation with type-safe dispatch
- Implemented chat commands: `/ocr`, `/vision`, `/translate`, `/summarize` in `src/chat/commands/`
- Refactored document extraction to use subagent architecture (Issue #9)
- Added config support for `[model.ocr]` and `[model.document]` in `src/config/models.rs`
- Created feature flag `subagent-tools` in `Cargo.toml`
- Updated `doc/src/CHANGELOG.md` with P4 release notes

**Key Files Modified:**
- `src/chat/subagent.rs` (new) - Core subagent execution engine
- `src/tools/spawn_subagent.rs` (new) - Tool for LLM-initiated subagent spawning
- `src/chat/commands/mod.rs` - Added specialized command handlers
- `src/config/models.rs` - Added OCR and document model configuration
- `Cargo.toml` - Added `subagent-tools` feature flag

**Related Issues:** #9 (Document Import), #12 (OCR/Vision Integration)

**OCR Prompt Strategy (v0.42.0-dev):**

- Added `OcrMode::into_descriptive_prompt()` — restricted, mode-specific prompts for vision models
- Added `is_glm_ocr_model()` — model detection for prompt selection
- Added `parse_ocr_mode()` — convenience parser for LLM string parameters
- Added `prompt_override: Option<&str>` on `OcrProcessor::process_file()` and `process_batch()`
- Added `ocr_mode: OcrMode` field on `SubagentConfig` with builder method
- Added `ocr_mode: Option<String>` parameter on `spawn_subagent()` tool
- Updated `/ocr` chat command to accept optional mode parameter
- Updated all 3 OCR entry points (CLI, chat, subagent) with model-aware prompt selection
- Removed dead `OCR_SYSTEM_PROMPT` constant
- YAGNI dead code removal: uses_chat_api(), tool_whitelist, with_tool_whitelist(), with_max_output_chars(), with_model_options(), SubagentRunner::settings, run_generate()
- Module-level #![allow(dead_code)] removed from security.rs and subagent.rs

**Commits:**
- `c25be97` feat(ocr): add model-aware prompt selection with descriptive prompts for vision models
- `4c8a81a` feat(subagent): propagate ocr_mode through subagent pipeline
- `6864b04` feat(chat): add mode parameter to /ocr command with model-aware prompts
- `0574c18` chore(ocr): remove dead OCR_SYSTEM_PROMPT constant

---

### 🟡 PRIORITY 5: Parallel Tool Execution [M1]

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
| 0.1 | Add `busy_timeout` to DB connection | 0.5h |
| 0.2 | Evaluate WAL mode and implement if viable | 0.5 day |
| 0.3 | Build DB concurrency integration test binary | 1 day |
| 1 | Identify which tools are safe for parallel execution | 0.5 day |
| 2 | Implement dependency analysis in CustomCoordinator | 1 day |
| 3 | Parallel execution with `join_all` | 1 day |
| 4 | Preserve sequential order for stateful tools | 0.5 day |
| 5 | Tests and benchmarks | 1 day |

**Phase 0: Database Concurrency Prerequisites**

Before parallel tool execution can work reliably, the SQLite connection must
handle concurrent access properly. Today the DB uses `Arc<Mutex<Connection>>`
which serializes all access, but this creates contention when background
operations (embedding generation, recovery) hold the lock for extended periods.

**Phase 0.1: Add `busy_timeout`** (1 line change in `init_connection()`)

```rust
conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
```

This makes SQLite wait up to 5 seconds when the database is locked, instead
of failing immediately with SQLITE_BUSY. Prevents the "unable to open database
file" error observed in smoke tests when embedding generation competes with
tool calls for the DB lock.

**Phase 0.2: Evaluate WAL mode** (design decision)

```rust
conn.execute_batch("PRAGMA journal_mode=WAL;")?;
```

WAL (Write-Ahead Logging) enables concurrent reads during writes. Without WAL,
`Arc<Mutex<Connection>>` serializes all access — even read-only tools wait.
With WAL, multiple read connections can operate while a write is in progress.

**Risks to evaluate:**
- WAL changes file behavior (requires testing: backup, recovery, cross-platform)
- WAL creates `-wal` and `-shm` files alongside the database
- WAL may have different performance characteristics on network filesystems
- Must verify compatibility with existing backup/restore procedures

**Phase 0.3: DB Concurrency Integration Test** (`tests/db_concurrency.rs`)

A separate integration test binary that simulates concurrent DB access
patterns without requiring a running LLM or Ollama server. Uses its own
temporary database file to avoid affecting user data.

**Test scenarios:**

| Scenario | Description | Contention Level |
|----------|-------------|------------------|
| A | Two lightweight reads (`note_show` + `note_list`) | None |
| B | Two reads with embedding (`remember(query=x)` + `remember(query=y)`) | Low |
| C | Heavy write + read (`import_document` + `remember(query)`) | High |
| D | Background embedding + read (send message + `remember()`) | High — the smoke test case |
| E | Auto-compact + read (after long conversation + `remember()`) | Medium |

**Binary design:**

```bash
# Run with default temporary database
cargo test --test db_concurrency

# Run with specific database (for manual testing)
cargo test --test db_concurrency -- --db-path /tmp/test_concurrency.db

# Run specific scenario
cargo test --test db_concurrency -- --scenario heavy_write_read
```

**Architecture of the test binary:**

```rust
// tests/db_concurrency.rs
// Simulates concurrent DB access patterns that occur during LLM tool calls

use std::sync::Arc;
use std::time::{Duration, Instant};

/// Spawns N concurrent DB operations and measures:
/// - Total time (should be near-max of individual ops, not sum)
/// - Whether any operation fails with SQLITE_BUSY or lock errors
/// - Whether data integrity is preserved under concurrent access
async fn run_concurrent_scenario(
    db: Arc<Database>,
    scenario: Scenario,
) -> ConcurrencyResult {
    let futures: Vec<_> = scenario.operations()
        .map(|op| tokio::spawn(async move { op.execute(&db).await }))
        .collect();

    let start = Instant::now();
    let results = futures::future::join_all(futures).await;
    let elapsed = start.elapsed();

    ConcurrencyResult {
        scenario: scenario.name(),
        total_time: elapsed,
        individual_times: results.iter().map(|r| r.time).collect(),
        errors: results.iter().filter(|r| r.is_err()).collect(),
        integrity_ok: verify_data_integrity(&db),
    }
}
```

**Key metric:** With proper concurrency support (WAL + busy_timeout), total
time for parallel reads should approach the max of individual operation times,
not the sum. Without WAL, the Mutex serializes everything and total time
approaches the sum.

**Current DB concurrency sources (existing background operations):**

1. **Embedding generation** — `tokio::spawn` in `session.rs` (lines 374, 502, 613)
   holds DB lock while writing embeddings after each user message
2. **Embedding recovery** — `recovery.rs` runs on startup, may hold lock during
   orphan cleanup and embedding generation
3. **Auto-compact** — `auto_compact_if_needed` runs after each LLM response,
   performs multiple DB reads and writes

**Tools that access the DB (potential parallel readers):**

| Tool | Access Type | Estimated Duration |
|------|-------------|-------------------|
| `remember(query=...)` | Read (semantic search) | ~100ms (with embedding) |
| `remember(id=...)` | Read (SELECT by ID) | <5ms |
| `note_show` / `note_list` | Read (SELECT) | <10ms |
| `note_edit` / `note_add` | Write (UPDATE/INSERT) | <5ms |
| `fact_remember` / `fact_recall` | Write/Read | <5ms |
| `import_document` | Write (INSERT + chunking) | ~100ms-2s |
| `/fact list` / `/doc list` | Read | <5ms |

**Safe for Parallel (read-only, no DB access):**
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

**Implementation note:** The read-only vs write classification above should be formalized in code (e.g., `ToolCategory::ReadOnly` / `ToolCategory::Stateful` enum) to enable the runtime parallel execution decision.

**Dependencies:** Phase 0 must be completed before Phase 1-5

**Estimated effort:** 5-6 days (including Phase 0)

**Related:** Issue #11

---

### Session Context Resume

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Show a brief summary of recent messages when resuming a chat session, so the user can quickly remember what they were discussing.

**Problem Statement:**
When a user opens the chat and a previous session is loaded, they currently only see:
```
Resumed session: default (47 messages)
```
This provides no context about what was discussed. The user has to scroll up or issue `/search` to recall the conversation topic.

**Solution:** Display the last few user/assistant message exchanges automatically when a session is resumed. No LLM call needed — simply show the last N messages from the session's in-memory history.

**Design Decisions:**
- Show only User and Assistant messages (skip System and Tool messages)
- Show the last 3 exchanges (a "exchange" = one User message + its Assistant response)
- Truncate each message to ~80 characters for readability
- Use `format_role_label()` from `src/consts/roles.rs` for consistent role labels with emojis
- Only display when a session is resumed (not for new or anonymous sessions)
- Display after the welcome banner and resume message

**Example Output:**
```
Resumed session: default (47 messages)
Recent context (47 messages):
  👤 User: Can you check the auth middleware?
  🤖 Assistant: I found the issue - the token validation is checking expired tokens...
  👤 User: What about the refresh token logic?
  🤖 Assistant: The refresh logic looks fine, but the middleware needs to pass...
```

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `get_recent_exchanges()` to `ChatSession` | ✅ Done |
| 2 | Add `RecentContextInfo` struct and formatting to `view/mod.rs` | ✅ Done |
| 3 | Add `show_recent_context()` to `TerminalView` | ✅ Done |
| 4 | Integrate in `repl.rs` after resume message | ✅ Done |
| 5 | Make `truncate_str` pub(crate) in `view/mod.rs` | ✅ Done |
| 6 | Tests and verification | ✅ Done |

**Implementation:**
- Added `ChatSession::get_recent_exchanges()` method that walks messages forward, pairing User+Assistant into exchanges
- Added `RecentContextInfo` and `RecentMessage` structs in `view/mod.rs` with `format_context_summary()` method
- Made `truncate_str()` pub(crate) and added `MAX_CONTEXT_LINE_LENGTH` constant
- Added `TerminalView::show_recent_context()` method that extracts exchanges and formats them
- Wired up in `repl.rs` to call `show_recent_context()` after resume message
- Added 6 unit tests for `get_recent_exchanges()` and 3 tests for `RecentContextInfo`

**Related:** Issue #67

---

## 🟡 PRIORITY 2: Configurable Embedding Model + Server-Side Matryoshka [M1]

**Status:** 📋 READY  
**Depends on:** None  
**Estimated effort:** 1 week (4 phases)  
**Issue:** #106

**Goal:** Make the embedding model configurable in `models.toml` and use Ollama's `dimensions` parameter for server-side Matryoshka truncation instead of client-side truncation.

**Prerequisite for:** #107 (Embedding Provider Abstraction) → #72 (P6.0 Multi-Provider)

**Background:** Currently, the embedding model (`nomic-embed-text-v2-moe:latest`), dimensions (768→256), context length (512), and prefix (`"search_document: "`) are all hardcoded in `src/embeddings/client.rs` and `src/embeddings/truncate.rs`. Additionally, `truncate_and_normalize()` does client-side Matryoshka truncation, which is redundant since Ollama v0.11.11 (Sept 2025) supports the `dimensions` parameter on `/api/embed` for server-side truncation with L2 normalization.

### Current Hardcoded Constants

| Constant | Value | File |
|---|---|---|
| `DEFAULT_EMBEDDING_MODEL` | `nomic-embed-text-v2-moe:latest` | `client.rs:16` |
| `FULL_DIMENSIONS` | 768 | `truncate.rs:7` |
| `TRUNCATED_DIMENSIONS` | 256 | `truncate.rs:9` |
| `DEFAULT_CONTEXT_LENGTH` | 512 | `client.rs:21` |
| `"search_document: "` prefix | Hardcoded | `client.rs:214,266` |
| `EMBEDDING_PREFIX_TOKENS` | 30 | `client.rs:43` |
| DB vec0 tables | `FLOAT[256]` | `schema.rs:177,187`; `connection.rs:343,352` |

### Key Discovery: Ollama `dimensions` Parameter

Since Ollama v0.11.11 (Sept 2025), the `/api/embed` endpoint supports a `dimensions` parameter for server-side Matryoshka truncation. The parameter truncates the output embedding vector before L2 normalization. llama.cpp also supports this on its `/v1/embeddings` endpoint.

### Proposed Config (`models.toml`)

```toml
[embedding]
model = "nomic-embed-text-v2-moe:latest"
dimensions = 256        # Matryoshka truncated dims (via Ollama API "dimensions")
context_length = 8192   # Auto-detected from Ollama model info
prefix = "search_document: "  # Model-specific prefix, empty string if none
```

### Implementation Phases

| Phase | Description | Effort |
|-------|-------------|--------|
| 1. Config | Add `[embedding]` section to `Settings` / `config.toml`; replace hardcoded constants with config reads (defaults matching current behavior); auto-detect `context_length` from Ollama model info | 2-3 days |
| 2. Server-side truncation | Add `dimensions` field to Ollama embed API request; remove or bypass `truncate_and_normalize()` when `dimensions` is set; keep client-side truncation as fallback for older Ollama | 1-2 days |
| 3. DB migration | Migration that recreates `vec0` tables with dynamic `FLOAT[N]` from config; warn user and require reindex when dimensions change; `regenerate_all_embeddings()` already exists via `/reindex` | 2-3 days |
| 4. Validation | Test alternative models (nomic-embed-text v1.5, mxbai-embed-large, qwen3-embedding:0.6b); verify no regression with current model | 1-2 days |

### Matryoshka-Capable Embedding Models (Ollama)

| Model | Full Dims | Matryoshka → 256? | Context | Size | MTEB | Recommendation |
|---|---|---|---|---|---|---|
| nomic-embed-text-v2-moe | 768 | ✅ (64-768) | 8192 | 957MB | ~62 | Current default, multilingual |
| nomic-embed-text (v1.5) | 768 | ✅ (64-768) | 8192 | 274MB | 62.39 | English-only, lighter |
| mxbai-embed-large | 1024 | ✅ (64-1024) | 512 | 700MB | 64.68 | Best retrieval, short context |
| qwen3-embedding (0.6B) | 4096 | ✅ (32-4096) | 8192 | ~400MB | ~60 | Instruction-aware |
| qwen3-embedding (8B) | 4096 | ✅ (32-4096) | 8192 | ~5GB Q4 | 70.58 | SOTA quality |
| snowflake-arctic-embed2 | 1024 | ✅ (256) | 8192 | 1.2GB | 55.98 | Multilingual |
| embeddinggemma | 768 | ✅ (128-768) | 8192 | ~300MB | good/size | Google, no special prefix |

### Matryoshka-Capable Embedding Models (llama.cpp / OpenAI-compatible)

These models work with llama.cpp server's `/v1/embeddings` endpoint which also supports the `dimensions` parameter:

| Provider | Model | Full Dims | Matryoshka? | Context | Notes |
|---|---|---|---|---|---|
| OpenAI | text-embedding-3-small | 1536 | ✅ (512) | 8191 | $0.02/M tokens |
| OpenAI | text-embedding-3-large | 3072 | ✅ (256-3072) | 8191 | $0.13/M tokens |
| Any HF GGUF | nomic-embed-text-v1.5-GGUF | 768 | ✅ | 8192 | Can load custom fine-tunes |
| Any HF GGUF | bge-m3-GGUF | 1024 | ✅ | 8192 | Multilingual, dense+sparse+ColBERT |
| Any HF GGUF | snowflake-arctic-embed-m-GGUF | 768/1024 | ✅ | 8192 | Size variants 22M-335M |

### Validation Criteria

- [ ] `[embedding]` section in config.toml works
- [ ] Changing `model` triggers reindex prompt
- [ ] Changing `dimensions` triggers DB migration + reindex
- [ ] Server-side `dimensions` parameter used when available
- [ ] Client-side truncation still works as fallback
- [ ] No regression in search quality with nomic-embed-text-v2-moe (current model)
- [ ] At least one alternative model tested and validated

**Related:** Issue #106, Issue #107 (Embedding Provider Abstraction), Issue #72 (P6.0 Multi-Provider)

---

## 🔵 PRIORITY 6: Core Enhancements [M1]

Features that enhance core functionality before Sprach 2.0 work begins.

### P6.0: Multi-Provider Support (OpenAI-Compatible Backends)

**Status:** 📋 PLANNED  
**Depends on:** #106 (Configurable Embedding Model — required before embedding provider swap)  
**Estimated effort:** 4-7 weeks (5 phases)

**Goal:** Abstract provider differences to support both Ollama (local) and OpenAI-compatible APIs (llama.cpp, LM Studio, cloud providers) through a unified interface.

**Motivation:**
- **Performance:** llama.cpp server with OpenAI-compatible endpoints can be significantly faster than Ollama for local models
- **Flexibility:** Users without local GPU can use cloud models (OpenAI, Together, etc.)
- **Compatibility:** llama.cpp and LM Studio gain compatibility through the OpenAI-compatible adapter
- **Extensibility:** Future providers (Anthropic, Google) fit naturally into the abstraction

**Architecture:** Provider abstraction with `LlmProvider` trait:

```
┌─────────────────────────────────┐
│        ask-ai (business logic)   │
│   Uses agnostic types:          │
│   LlmMessage, LlmResponse,       │
│   ModelParams, ProviderError     │
├─────────────────────────────────┤
│         LlmProvider trait        │
│   chat_complete()                │
│   generate()                     │
│   embed()                        │
│   model_info()                   │
│   list_models()                  │
├──────────┬───────────────────────┤
│ Ollama   │  OpenAI-compatible    │
│ Provider │  Provider             │
│ (ollama  │  (reqwest +           │
│  -rs)    │   serde_json)         │
└──────────┴───────────────────────┘
```

**Impact analysis:** ~770 lines across ~27 files. Breaking changes but refactoring, not rewrite.

| Category | Files | Lines affected | Type |
|----------|-------|----------------|------|
| CustomCoordinator | 1 | ~200 | Breaking |
| ChatMessage/Message types | 12 | ~150 | Breaking |
| Error handling | 2 | ~105 | Breaking |
| ModelOptions/ModelInfo | 3 | ~90 | Breaking |
| Vision/OCR processors | 2 | ~95 | Breaking |
| Client/Provider creation | 5 | ~70 | Breaking |
| Tool macro/trait | 19 | ~39 | Aditivo (keep ollama-rs) |
| Config files | 0 | 0 | Aditivo |

**Key design decisions:**
- `#[ollama_rs::function]` macro **preserved** — ollama-rs stays as dependency
- Tool format conversion at provider boundary (Ollama vs OpenAI function calling)
- Config-based capabilities (no `show_model_info` equivalent in OpenAI)
- Per-model provider in `models.toml`

**Implementation phases:**

| Phase | Description | Effort |
|-------|-------------|--------|
| 1. Foundation | Create `src/provider/` with agnostic types + `LlmProvider` trait + `OllamaProvider` wrapper | 1-2 weeks |
| 2. Core migration | Migrate `CustomCoordinator`, error handling, chat/query modules | 2-3 weeks |
| 3. OpenAI-compatible | Implement `OpenAICompatibleProvider` with reqwest, tool calling, embeddings | 1-2 weeks |
| 4. Config & UX | Extend `models.toml`/`config.toml`, config-based capabilities, integration tests | 1 week |
| 5. Subcommands | Migrate Vision, OCR, Translate, Summarize | 1 week |

**Configuration example:**

```toml
# models.toml
[carnice-9b-local]
model_id = "carnice-9b"
provider = "openai-compatible"
base_url = "http://127.0.0.1:12434"  # llama.cpp / llama-swap
tools = true

[gpt-4o]
model_id = "gpt-4o"
provider = "openai-compatible"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
tools = true
vision = true
```

**Key risks:**

| Risk | Mitigation |
|------|-----------|
| Tool calling format incompatibility | Adapter pattern: `ToolInfo` → OpenAI format at boundary |
| `show_model_info()` absent in OpenAI | Config-based capability declaration in `models.toml` |
| `KeepAlive` is Ollama-only | Param ignored by OpenAI provider |
| Vision API completely different | Two paths: Ollama uses `/api/generate`, OpenAI uses chat + `image_url` |
| SSE vs JSON lines streaming | Not currently used — deferred |

**Reference:** See `doc/src/development/roadmap.md` - Multi-Provider Support section for architectural details and type definitions.

**Related:** Issue #72

---

### P6.1: Auto Fact Extraction (autoDream-lite)

**Status:** ✅ COMPLETED  
**Depends on:** P0 (Factual Memory System — completed)  
**Estimated effort:** 3-5 days (original) + 2 days (bug fixes)

**Implementation summary:**

Key files:
- `src/facts/extract.rs` — Heuristic extraction, dedup pipeline, validation, Layer 3.5 semantic dedup
- `src/facts/lang.rs` — Centralized EN/PT patterns, PT→EN translation, `normalize_to_storage_format()` (ADR-E4), `normalize_for_comparison()` (Lemma strip), `normalize_adverb_verb()` (adverb expansion), `lemmatize_verb()` (3rd person → base form)
- `src/facts/conflict.rs` — Conflict detection, preference override, lowered threshold
- `src/facts/db.rs` — FTS5 search, exact match, normalized match, BM25 scoring
- `src/facts/prompt.rs` — System prompt scope separation (Global/Project), defense-in-depth normalization
- `src/facts/types.rs` — Global scope forces project_id=None
- `src/tools/fact_tools.rs` — LLM tool with PT→EN translation, content validation, layered dedup, Layer 3.5
- `src/chat/repl.rs` — Async `try_auto_extract_facts()` passes embedding_client for Layer 3.5
- `src/chat/command_handlers.rs` — `/fact add` CLI with full 5-layer dedup, normalization, and embedding generation (async)
- `src/embeddings/client.rs` — Semaphore(1) for serialized embedding requests, 30s timeout

**Architecture: Five-layer dedup pipeline:**
1. **Layer 1: Exact content match** — case-insensitive, trimmed comparison via `find_exact_fact()`
2. **Layer 2: Normalized content match** — `normalize_for_comparison()` strips pronouns/subjects and lemmatizes verbs (3rd person → base form), catches "I prefer X" ≈ "User prefers X" ≈ "prefers X" → all normalize to "prefer X"
3. **Layer 3: FTS5 BM25 search** — keyword matching with threshold 0.75 (lowered from 0.85)
4. **Layer 3.5 (NEW): Semantic embedding** — cosine similarity ≥ 0.90 for preference facts, catches "prefer dark mode" ≈ "prefer light mode" via embeddings
5. **Layer 4 (startup): Semantic verification** — `verify_and_dedup_facts()` O(n²) cosine comparison on startup

**Bug fixes (from smoke test #1):**
- Bug #1: Dedup broken — Fixed with three-layer pipeline, exact match, normalized match, threshold 0.75
- Bug #2: PT→EN inconsistent — Fixed with expanded `translate_pt_to_en()` (3rd-person PT, hybrid LLM forms), `fact_add` English-only instruction
- Bug #3: `/fact list` scope — Fixed with `FactListScope::All/Global/Project`, separate sections
- Bug #4: Non-fact validation — Fixed with `is_extractable_sentence()` in `fact_add`
- Bug #5: PT commands — Fixed with `command_starters()` check in `fact_add`
- Bug #1/6: Global project_id — Fixed with `Fact::new()` forcing `project_id=None` for Global scope
- Scope separation — System prompt groups facts by scope (Global Preferences/Facts, then Project)
- Global-wins-project — New Global fact removes conflicting Project facts
- Preference override — "prefer dark mode" vs "prefer light mode" detected as contradiction

**Bug fixes (from smoke test #2):**
- Bug #1: Adverb modifier normalization — `normalize_adverb_verb()` in `lang.rs` handles EN patterns like "I really like X" → "User really likes X" and PT patterns like "Eu sempre prefiro X" → "User always prefers X" via regex expansion after static prefix lists fail. Covers 15 EN adverbs × 8 verbs + 13 PT adverbs × 6 verbs + negation ("I usually don't like" → "User usually doesn't like"). Falls through to no-change if pattern doesn't match.
- Bug #2: Layer 2 verb lemmatization — `normalize_for_comparison()` now lemmatizes third-person verbs after stripping the subject: "prefers dark mode" → "prefer dark mode" matches "prefer dark mode". Added `VERB_LEMMAS` constant and `lemmatize_verb()` function with explicit lemma map + generic trailing-'s' rule with 'ss' guard.
- Bug #3: `/fact add` CLI dedup parity — `handle_fact_add()` in `command_handlers.rs` now calls `normalize_to_storage_format()` (ADR-E4), performs Layer 1 (exact match) and Layer 2 (normalized match) dedup before FTS5, performs Layer 3.5 semantic contradiction detection when embedding client is available, and eagerly generates embeddings after insertion. Changed from synchronous `fn` to `async fn`. Previously, `/fact add` stored raw user input without normalization, used only FTS5 dedup, and never generated embeddings (`has_embedding=0` until startup recovery).
- Bug #4: Layer 3.5 testability documentation — Added SMOKE_TEST.md sections 21.14 (`/fact add` dedup parity test) and 21.15 (`/tools` toggle for auto-extraction-based Layer 3.5 testing). The `/tools` command disables LLM tool calls, forcing contradiction detection through the auto-extraction path, making Layer 3.5 independently testable.

**ADR References:**
- ADR-L1: All fact content stored in English (PT→EN via `lang::translate_pt_to_en()`)
- ADR-L2: Normalization output always English ("User prefers" not "User prefere")
- ADR-L3: EN+PT classification keywords in `lang::preference_keywords()`
- ADR-L4/L5: All string patterns centralized in `lang.rs`, no duplication
- ADR-E4 (revised): Third-person normalization applied at storage time (not just render time). All facts stored as "User prefers X". `normalize_to_third_person()` in prompt rendering remains as defense-in-depth.

**Phase 2 (P6.7, planned):** Embedding-based semantic dedup — ✅ COMPLETED (see P6.7 below)

---

### P6.2: Context Pinning

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** None  
**Estimated effort:** 2-4 days (after research)

**Goal:** Allow users to mark specific messages or decisions as high-priority, preserving them during compaction.

**Current state:** All messages are eligible for compaction. There is no mechanism to pin important context.

**Proposal:** Add `/pin <id>` command and message metadata to exempt specific messages from compaction.

**Open questions:**
- How many pins are reasonable? Unlimited pins could fill context.
- Should pins expire? Or require manual unpin?
- How does pinning interact with the compaction algorithm in `src/chat/core.rs`?
- UI: how does user see what's pinned?

**Related:** Issue #74

---

### P6.3: Dynamic Context Limits

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** None  
**Estimated effort:** 1-2 days (after research)

**Goal:** Calculate tool operation limits (max_lines, max_tokens for results) dynamically based on remaining context, instead of using fixed constants.

**Current state:** Some limits already adapt (pre-tool warning in `custom_coordinator.rs`), but `max_lines` in `read_file` and tool result truncation use fixed values.

**Open questions:**
- What's the right formula? Context_remaining - buffer = available_for_tool?
- Need more research to understand the full complexity.

**Related:** Issue #75

---

### P6.4: Secret Scanning (Content)

**Status:** 📋 PLANNED  
**Depends on:** Existing `files_blocklist.rs` (path-based scanning)  
**Estimated effort:** 1-2 days

**Goal:** Scan file CONTENT for credential patterns (AWS keys, GitHub tokens, OpenAI keys, SSH private keys) before write operations, extending the existing path-based blocklist.

**Current state:** `src/tools/files_blocklist.rs` blocks writing to sensitive FILE PATHS (`.env`, `id_rsa`, etc.). However, file CONTENT is not scanned — writing `AKIAIOSFODNN7EXAMPLE` to `notes.txt` would succeed. This is an evolution of the blocklist concept, not a new security layer.

**Proposal:** Add `scan_content_for_secrets(content: &str)` function that checks content against 25+ credential patterns before allowing write operations.

**Security rule:** Never log or display secret values — only show rule ID and label (e.g., "AWS Access Key detected").

**Related:** Issue #76

### P6.5: Config Upgrade Command

**Status:** 📋 PLANNED
**Depends on:** None
**Estimated effort:** 5 days

---

### P6.7: Fact Embedding & Semantic Dedup

**Status:** ✅ COMPLETED [M1]
**Depends on:** P6.1 (Auto Fact Extraction — completed)
**Estimated effort:** 5-7 days (completed)

**Goal:** Add embedding-based semantic dedup as Layer 4 on top of the existing three-layer dedup pipeline, enabling reliable detection of semantically equivalent facts regardless of phrasing, language, or subject form.

**Architecture: Five-layer dedup pipeline:**
1. **Layer 1: Exact content match** — case-insensitive, trimmed comparison via `find_exact_fact()`
2. **Layer 2: Normalized content match** — `normalize_for_comparison()` strips pronouns/subjects
3. **Layer 3: FTS5 BM25 search** — keyword matching with threshold 0.75
4. **Layer 3.5 (NEW): Semantic embedding** — cosine similarity ≥ 0.90 via `fact_embeddings` vec0 (insert-time, for preferences only)
5. **Layer 4 (startup): Semantic verification** — `verify_and_dedup_facts()` O(n²) pairwise cosine comparison at threshold 0.90

**Schema changes (v10 → v11):**
- Added `has_embedding INTEGER DEFAULT 0` column to `facts` table
- Added `fact_embeddings` vec0 virtual table (256d Matryoshka, same model as content embeddings)
- Added `idx_facts_embedding` partial index on `has_embedding WHERE has_embedding = 0 AND invalidated_at IS NULL`

**New modules:**
- `src/facts/embedding.rs` — `generate_fact_embedding()` wrapper around `EmbeddingClient::embed()`
- `src/facts/recovery.rs` — `recover_missing_fact_embeddings()` + `flush_pending_fact_embeddings()` for startup/shutdown
- `src/facts/verify.rs` — `verify_and_dedup_facts()` with O(n²) pair-wise cosine similarity comparison at threshold 0.90

**New DB methods:**
- `update_fact_embedding()` — Insert into `fact_embeddings` vec0, set `has_embedding = 1`
- `search_facts_semantic()` — KNN search via vec0, filter by scope
- `get_facts_for_reindex()` — Find facts with `has_embedding = 0`
- `delete_fact()` now also removes from `fact_embeddings`

**Embedding lifecycle:**
- **Eager (insert-time):** After `insert_fact()` in both auto-extraction and `fact_add`, `EmbeddingClient::embed()` generates embedding synchronously via `Semaphore(1)` (serialized, 30s timeout). If Ollama offline, `has_embedding = 0` and startup recovery catches up.
- **Startup recovery:** `recover_missing_fact_embeddings()` — generates embeddings for all facts with `has_embedding = 0`, then verifies no facts remain without embeddings (logs warning if any still missing).
- **Startup verification:** `verify_and_dedup_facts()` — pair-wise cosine comparison, resolves duplicates/contradictions/global-wins-project.
- **Shutdown:** `flush_pending_fact_embeddings()` — completes pending embedding generation before exit.

**Startup sequence:**
```
recover_missing_embeddings()           ← Content embeddings (existing)
recover_missing_fact_embeddings()      ← Fact embeddings (NEW)
verify_and_dedup_facts()               ← Semantic dedup (NEW)
```

**Conflict resolution (semantic):**
- Duplicate (cos ≥ 0.90, no contradiction) → Keep newer, remove older
- Contradiction (cos ≥ 0.90, with `is_contradiction()`) → Keep newer, remove older
- Global-wins-project → Global fact removes Project duplicate

**Silent by design:** All startup/shutdown operations use `log::info/debug` only; no visual output unless errors occur.

**Re-exports:** `EmbeddingError` and `cosine_similarity` now re-exported from `embeddings` module for use by fact modules.

**Related:** Issue #73

**Goal:** Add a `ask-ai config upgrade` subcommand that merges missing default fields into the user's existing `config.toml`, adding doc comments only for new fields. Users don't have to manually track which config fields are new after each update.

**Problem:**
- Every release adds new config fields (`[feedback]` in v0.40, `[facts]` in v0.42)
- `serde(default)` silently fills missing fields — no user-visible indication
- Users must read CHANGELOG to discover new fields and add them manually
- `--init-config` creates a full config, but doesn't merge with existing

**Solution:** Two-pass approach using `toml_edit` (comment preservation) + `toml` (value parsing):

```
ask-ai config upgrade [--dry-run] [--backup]
```

1. Read user's `config.toml` with `toml_edit::DocumentMut` (preserves comments and formatting)
2. Parse with `toml::from_str::<Settings>()` to detect which fields are present
3. Compare against `Settings::default()` to find missing fields
4. Insert missing fields with doc comments using `toml_edit`
5. Write back, preserving all existing content

**Design Decisions:**
- Insert-only: never modify existing fields or comments
- Cannot distinguish "explicitly set to default" from "missing" — acceptable limitation
- Comments come from a static const map keyed by field path
- Backup file created before upgrade (`config.toml.bak`)
- `--dry-run` flag shows what would be added without modifying

**New Files:**
- `src/commands/config_upgrade.rs` — `ConfigUpgrader` struct with upgrade algorithm

**New Dependency:**
- `toml_edit = "0.25"` — parse/write TOML with comment preservation

**Related:** Issue #105

---

### 🔵 PRIORITY 4: Code Quality — Memory Staleness Warnings [M1]

**Status:** ✅ COMPLETED (v0.39.5)  
**Estimated effort:** 0.5 day

**Goal:** Inject staleness warnings into the facts prompt when facts are old.

**Current state:** `src/facts/prompt.rs` formats facts without age indicators. Facts with `last_accessed` > 30 days may be outdated but are presented with the same confidence as fresh facts.

**Implementation:**

Added `get_staleness_label()` function in `src/facts/prompt.rs` with priority-based labels:
- `(stale)` — when `decay_score < 0.3` (badly decayed)
- `(N days ago)` — when `last_accessed` > 30 days (not recently used)
- `(unused)` — when `access_count == 0` and age > 7 days (never retrieved)
- No label for fresh facts (avoids noise)

Modified `build_facts_section()` to append staleness label after fact content:
```rust
for fact in preferences {
    let staleness = get_staleness_label(fact);
    section.push_str(&format!("- {}{}\n", fact.content, staleness));
}
```

**Complexity:** Very low — single file change in `src/facts/prompt.rs`.

**Related:** Issue #70

---

### 🔵 PRIORITY 4: Code Quality — Truncation Warnings [M1]

**Status:** ✅ COMPLETED (v0.39.5)  
**Estimated effort:** 0.5 day

**Goal:** Add explicit truncation metadata in tool outputs when file reads or search results are limited.

**Current state:** `read_file` with `max_lines` silently truncates. No `[TRUNCATED]` indicator in output.

**Implementation:**

Modified truncation handling across three files:

1. **`src/tools/files.rs`** — `read_file`:
   - Added `[TRUNCATED: Showing lines 1-N of M. Use read_file_segment to read more.]` when `max_lines` truncates output
   - Calculates `total_lines` before truncation to include total count
   - Only appends notice when actually truncated (skips if `max_lines >= total_lines`)

2. **`src/tools/files.rs`** — `search_files`:
   - Changed from `... (stopped after N matches)` to `[TRUNCATED: Showing N matches. Refine your search pattern for fewer results.]`

3. **`src/tools/remember.rs`**:
   - Added `REMEMBER_NOTE_PREVIEW_CHARS` (150), `REMEMBER_MESSAGE_PREVIEW_CHARS` (200), `REMEMBER_SUBMESSAGE_PREVIEW_CHARS` (100) constants
   - Notes/docs: `[TRUNCATED: 150 of N chars. Use remember(id="note:X") for full content.]`
   - Messages: `[TRUNCATED: 200 of N chars. Use remember(id="msg:X") for full content.]`
   - Sub-messages: `[+N chars]` (no retrievable ID, so simplified format)
   - All truncation uses Unicode-safe `.chars().take()` pattern

**Complexity:** Low — modify output formatting in `read_file`, `search_files`, and `remember`.

**Related:** Issue #71

---

### ✅ Bug: Embeddings Fail on Startup When Input Exceeds Context Window [M1]

**Status:** ✅ COMPLETED (Issue #40, PR #102, merged 2026-04-24)

**Complements:** PR #46 (Issue #40) — PR #46 fixed the fallback architecture; PR #102 fixed residual robustness issues.

**Goal:** Fix embedding generation failures when content exceeds the embedding model's context window during startup regeneration/recovery.

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Proactive context length check in `embed()` | ✅ Done |
| 2 | Cache `context_length` in `EmbeddingClient` via `OnceCell` | ✅ Done |
| 3 | Handle `ContextExceeded` variant in fallback match arms | ✅ Done |
| 4 | Replace `panic!` with graceful degradation in `regenerate.rs` | ✅ Done |
| 5 | Consistent empty content validation in `recovery.rs` | ✅ Done |
| 6 | Fix `has_embedding` marking logic | ✅ Done |
| 7 | Increased safety margins (CONTEXT_SAFETY_MARGIN 10%→20%, EMBEDDING_PREFIX_TOKENS 20→30, DEFAULT_CHUNK_PERCENT 90%→80%, DEFAULT_PREFIX_MARGIN 30→40) | ✅ Done |
| 8 | Documented token estimation limitations (ollama-rs v0.3.4 ignores `prompt_eval_count`) | ✅ Done |

**Files Modified:**
- `src/embeddings/client.rs` — Proactive context check, cached context length, API error → ContextExceeded conversion, increased safety margins, documented estimation limitations
- `src/embeddings/fallback.rs` — Handle `ContextExceeded` variant in both fallback paths
- `src/embeddings/regenerate.rs` — Replace panic with graceful degradation
- `src/embeddings/recovery.rs` — Add empty content validation, fix has_embedding marking
- `src/content/db.rs` — New `mark_item_embedding_if_complete()` method
- `src/embeddings/chunk_config.rs` — Reduced chunk percent (90%→80%), increased prefix margin (30→40)
- `SMOKE_TEST.md` — Section 4.3 for embedding startup resilience
- `.opencode/skills/` — Updated next-demand, pr-workflow, pr-testing, release-process with duplicate checks and card management

**Related:** Issue #40 (canonical), Issue #39 (duplicate, closed), PR #102, Issue #103 (future: exact token counts via reqwest)

**Related:** Issue #39

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

### 🔵 PRIORITY 8: File Session State [M1]

**Status:** ❌ NOT STARTED

**Goal:** Explicit file tracking for session context.

**Related:** Issue #13

---

### 🔵 PRIORITY 10: Extended Personalities System [M1]

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

### 🔵 PRIORITY 11: Multilingual Skill Sanitization [M1]

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

### 🔵 PRIORITY 12: Skills Management Tool [M1]

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

### 🔵 PRIORITY 13: File Staleness Detection [M1]

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

### 🔵 PRIORITY 14: TUI (Terminal User Interface) [M2 → M3]

**Status:** ❌ NOT STARTED

**Goal:** Build a responsive TUI using Ratatui-rs.

See `doc/src/development/roadmap.md` - TUI section for detailed implementation plan.

**Milestone split (2025-04-25):**
- **M2 (UX & TUI Design):** UX research, design mockups, prototyping, private feedback rounds. This is the design phase that will shape the public-facing product.
- **M3 (TUI Implementation):** Coding the TUI based on M2's design decisions. Happens alongside Sprach 2.0 research.

**Components:**
- Chat pane with markdown rendering
- Input pane with history
- Status bar showing model, context usage, tokens
- Sidebar for tools/messages (optional)

**Estimated effort:** 3-4 weeks

**Mascote idea:** An ASCII mascote (Sprach described itself as "Nó de Ideias" — Idea Knot) could serve as a visual indicator of system state. When reflection triggers fire (see S2.3), the mascote's expression could change to signal the user. This follows patterns from other agent frameworks where visual feedback helps users understand internal state. Note for P14 implementation.

**Related:** Issue #16

---

### 🔵 PRIORITY 15: Plugin System [M2 → M3]

**Status:** ❌ NOT STARTED

**Goal:** Pluggable architecture for extending ask-ai functionality with external tools.

**Dependencies:** TBD

**Estimated effort:** TBD

**Related:** Issue #15

**Sprach 2.0 Note:** The article adds architectural details to P15: (1) 4-layer architecture (Runtime WASM → Host Interface → Plugin Manifest → Plugin Code), (2) sandbox by capabilities (allowed/denied lists, not total isolation), (3) semantic versioning (DEC-005: major equal, minor ≥ required), (4) TOML manifest format. See S2.4 in PRIORITY 7 for details.

**Sub-items to address during P15 research:**

1. **MCP Client Integration:** Dynamic tool discovery via MCP protocol. Primary path for extending functionality without native code changes.
2. **Extensible Hooks:** Lifecycle hooks (PreToolCall, PostFileWrite, PreCompact) as a lightweight plugin alternative. May or may not be implemented depending on scope.
3. **Post-edit verification as EXTERNAL service:** NOT built into ask-ai. Code verification (syntax, typecheck, lint) should be a plugin or external service that the harness invokes. This keeps ask-ai focused on research, interaction, and cognitive evolution.
4. **Scope clarification:** ask-ai is NOT a code-specific harness. Features specific to software development workflows should be delegated to external tools/plugins. The core should remain focused on general-purpose cognitive interaction.

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

⚠️ **CRITICAL SECURITY ADVISORY (2026-04-19):** The Anthropic MCP SDK has a **by-design vulnerability** in `StdioServerParameters` that allows arbitrary command execution. The STDIO transport configuration passes commands directly to the OS without validation — even failed connections execute the command. This affects 7,000+ public MCP servers and 150M+ downloads (CVE-2025-65720, CVE-2026-30623, CVE-2026-30624, CVE-2026-30618, CVE-2026-33224, CVE-2026-30625, CVE-2026-30615, CVE-2026-26015, CVE-2026-40933, CVE-2025-49596, CVE-2026-22252, CVE-2026-22688, CVE-2025-54994, CVE-2025-54136). Anthropic has declined to fix this, calling it "expected behavior."

**ask-ai's MCP security requirements (ADR-007):**
1. `ask-ai` MUST NOT use the Anthropic MCP SDK's `StdioServerParameters` directly for untrusted input
2. MCP server configurations containing `command` fields MUST be treated as arbitrary code execution — equivalent to running a shell command
3. User confirmation MUST be required before installing or connecting to any MCP server (no zero-click auto-discovery)
4. An allowlist of approved MCP server commands MUST be maintained in `config.toml` (`[mcp].allowed_servers`)
5. MCP servers SHOULD prefer Streamable HTTP transport over STDIO when available (HTTP transport does not spawn arbitrary processes)
6. When STDIO transport is required, the server process MUST run with minimal privileges (seccomp/cgroups/namespace restrictions)
7. MCP marketplace/server registry URLs MUST be treated as untrusted input — URLs in server configurations can trigger hidden STDIO configurations (CVE category 4 from the OX Security research)

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
| MCP | JSON Schema + server | Runtime validation | Human approval ⚠️ RCE risk via STDIO (CVE-2025-65720 et al.) |
| AI SDK (Vercel) | Zod Schema + execute | Compile-time | Needs approval |
| Hermes Agent | Skills (Markdown) + Tools (Rust) | Compile-time for tools | Sanitization |
| **ask-ai (current)** | Rust code + feature flags | Compile-time | Blacklist |

#### Implementation Phases

**Phase 1: MCP Client Integration**
- Implement MCP client to connect to external tool servers
- Support `tools/list` and `tools/call` operations
- Human confirmation UI for tool invocations
- ⚠️ **ADR-007 constraints:** STDIO transport REQUIRES explicit user approval + command allowlist in `config.toml`. Prefer HTTP/SSE transport. Never use Anthropic SDK `StdioServerParameters` directly.

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

## 🟣 PRIORITY 7: Sprach 2.0 — CAS Research [M2 → M3]

**Status:** 🟡 RESEARCH NEEDED  
**Reference:** `~/git/biblio/sprach-2-0-auto-analise.org`  
**Comprehensive Design:** See [Sprach 2.0 Research](./doc/src/development/sprach-2-0-research.md) for open questions, code analysis, and implementation details.

Based on the Sprach 2.0 self-analysis article, which identifies ask-ai-rs as a Complex Adaptive System (CAS) with emergent properties but limited open-endedness. The proposals below aim to increase emergent connectivity and adaptive behavior.

**Prerequisite:** All P1-P5 current items must be completed before starting P7 work.

### S2.1: Visualize Connections Tool

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** None  
**Estimated effort:** 2-3 days (after research)

LLM tool that, given an item ID or query, finds top-N most similar items via embedding similarity and returns a Mermaid graph visualization.

**Existing infrastructure:**
- `search_content_semantic()` in `content/db.rs` — vector search works
- `content_embeddings` (vec0) — 256d embeddings already stored
- `ContentSearchResult.score` — similarity distance already computed
- `EmbeddingClient` — configurable embedding model

**Open questions:**
- How to handle items without embeddings?
- Mermaid rendering: terminal output vs. file vs. markdown block?
- Should connections be calculated on-the-fly or cached? (DEC-001: cache incrementally)
- What N is optimal for meaningful graphs without noise?

---

### S2.2: Content Relations Graph

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** S2.1  
**Estimated effort:** 5-8 days (after research)

Persistent `content_relations` table with a two-layer architecture:
1. **Layer 1 (Discovery):** Embedding-based, automatic, finds proximity (`find_similar(query_embedding, threshold=0.75)`)
2. **Layer 2 (Classification):** LLM-based, on-demand, classifies relation type (`classify_relation(source, target)`)

**Relation types** (inspired by Zettelkasten):

| Type | Definition | Example |
|------|-----------|---------|
| `extends` | B develops A | Carvalho extends Maturana |
| `contradicts` | B contests A | Lucas contests Estrada |
| `instantiates` | B is case of A | "Eu-difuso" instantiates "Strange Loop" |
| `cites` | B references A | Note cites Villalobos |
| `presupposes` | B assumes A as base | Enactivism presupposes autopoiesis |
| `resolves` | B dissolves tension in A | Synthesis resolves Ellis+Gödel |
| `questions` | B problematizes A | Critique questions Clark |

**Schema:**

```sql
CREATE TABLE content_relations (
    source_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    relation_type TEXT NOT NULL,      -- enum of 7 types
    strength REAL NOT NULL,           -- cosine similarity (0-1)
    confidence REAL NOT NULL,         -- LLM confidence (0-1)
    justification TEXT,               -- 1-sentence LLM explanation
    created_at INTEGER NOT NULL,
    PRIMARY KEY (source_id, target_id)
);
```

**Cache incremental approach (DEC-001):** Classification runs on-demand, results are cached. Graph grows organically by usage, not pre-computed.

**Existing infrastructure:**
- `content_items` unified table (schema v8) with migration system
- `EmbeddingClient` for similarity computation
- `ContentSearchResult` with distance scoring

**Open questions:**
- When to create relations? On-query (lazy) vs on-insert (eager) vs batch?
- Should unused relations decay (like facts)?
- Is persistent storage better than lazy computation (S2.1 only)?
- Scalability: 10K items × 10 relations = 100K rows — acceptable for SQLite?

---

### S2.3: Reflection on Triggers + Curation

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** S2.1, S2.2 (needs relation detection)  
**Estimated effort:** 4-7 days (after research)

Self-reflection triggered by specific events (not periodic). Reflection results are saved as drafts requiring human approval.

**Trigger types (DEC-002):**

| Trigger | Criterion | Example |
|---------|----------|---------|
| Error | Tool failure, insufficient context | `visualize_connections()` returns empty |
| Surprise | Embedding distant from expected | Query "enactivism" returns note about "Turing" |
| Conflict | Two notes contradict each other | Carvalho vs. Villalobos on closure |
| Pattern | Same query repeated N times | User asks about "open-endedness" 3× in 5 sessions |
| On-demand | User requests | "Sprach, reflect on X" → `/reflect` command |

**Curation pipeline (DEC-003):**

Reflections are saved as **drafts**, not published automatically:

1. **Novelty:** Cosine similarity < 0.85 with existing notes
2. **Actionability:** Must imply ≥1 concrete change (tool, note, behavior)
3. **Density:** Minimum 200 words, ≥1 Zettelkasten connection
4. **Human approval:** Draft → `/approve-patch` → published

**Existing infrastructure:**
- `note_add` tool (LLM can create notes)
- `ChatSession` with message counting
- `ContentSource::Llm` source attribution
- Fact decay system (model for reflection aging)

**Open questions:**
- How to detect "surprise" triggers? (embedding distance threshold tuning)
- How to detect "conflict" triggers? (contradictory notes identification)
- How to detect "pattern" triggers? (repeated query tracking)
- What prompt template produces useful reflections vs. noise?
- Where to store drafts? Database with `status=draft` flag?

---

### S2.4: Plugin System (WASM)

**NOTE:** This is already tracked as PRIORITY 15 in this document. The Sprach 2.0 article adds architectural details:

- **4-layer architecture:** Runtime WASM → Host Interface → Plugin Manifest → Plugin Code
- **Sandbox by capabilities** (DEC-004): allowed/denied lists, not total isolation
- **Semantic versioning** (DEC-005): Major equal, minor ≥ required
- **Example manifest:** TOML with `name`, `version`, `[capabilities]`
- **State of art:** WASM confirmed as emerging standard; alternatives (E2B, Daytona) need evaluation

These details should be incorporated into P15 when research begins.

---

### S2.5: SOUL.md Patching with Approval

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** S2.3 (curation pipeline feeds personality adjustment)  
**Estimated effort:** 3-5 days (after research)

Dynamic personality adaptation through LLM-generated patches to SOUL.md, with mandatory human approval.

**Flow (DEC-006):**

1. User gives feedback ("too verbose", "too technical")
2. Sprach generates a **suggestion patch** (not automatic)
3. Lucas reviews via `/apply-patch` command
4. If approved: patch applied + git commit automatic

**Key difference from P5 (Feedback Infrastructure):**
- P5 captures **what happened** (signal + weight for retrieval)
- S2.5 adjusts **who I am** (personality modification)

Both are complementary: P5 improves *retrieval quality*, S2.5 improves *behavior style*.

**Existing infrastructure:**
- `src/soul.rs` — loads SOUL.md statically (no dynamic updates yet)
- `src/facts/` — model for decay and scope
- `src/tools/notes.rs` — model for LLM-generated content with source attribution

**Open questions:**
- Should SOUL.md be in git? What about users without git?
- Patch format: search-replace? Section-level? Line-level?
- How to validate patches don't corrupt SOUL.md structure?
- Backup mechanism: timestamped copies before patching?

---

### S2.6: Skills Auto-Registration and Meta-Architecture

**Status:** 🕐 AWAITING MATURATION  
**Depends on:** S2.1-S2.5 operational  
**Estimated effort:** TBD

Meta-level architecture where skills can create and register other skills. Requires S2.1-S2.5 to be operational and well-tested before this becomes meaningful.

**Why wait:** Needs more experimentation with 6.1-6.5 before meta-level design makes sense.

---

### Sprach 2.0: Validated Decisions (DEC-001 to DEC-007)

The following architectural decisions from the Sprach 2.0 article have been validated by state-of-the-art research:

| Decision | Ruling | Validation |
|----------|--------|------------|
| **DEC-001** Cache incremental for `content_relations` | On-demand, not pre-computed | GraphSeek 2026, Graph RAG 2026 |
| **DEC-002** Reflection triggers over periodic | Specific triggers, not time-based | ICML 2025, MeCo arXiv 2025 |
| **DEC-003** Curation with human approval | Drafts, not auto-publish | Rewire.it, "Human-in-the-loop" |
| **DEC-004** WASM sandbox by capabilities | Allowed/denied, not total isolation. **CRITICAL (2026-04-19):** DEC-007 extends this — `process_spawn` deny is meaningless when MCP STDIO transport itself *is* process spawning. STDIO MCP servers require explicit allowlist + sandbox. | The New Stack 2026, MCP-SandboxScan, OX Security 2026 |
| **DEC-005** Semantic versioning for plugins | Major equal, minor ≥ | OpenFang, "Semver + manifest signing" |
| **DEC-006** SOUL.md patches with human approval | Suggestions, not automatic | MetaMind NeurIPS 2025, "Human oversight" |
| **DEC-007** MCP STDIO security: no untrusted command execution | Explicit approval + allowlist + sandbox for STDIO | OX Security 2026, CVE-2025-65720 et al., Anthropic MCP SDK vulnerability |

**Competitors identified:**
- Joplin GSoC 2026: Note graphs with AI (similar to S2.1 + S2.2)
- OpenClaw: WASM sandbox for community skills (similar to S2.4)

---

### ADR-007: MCP STDIO Transport Security

**Date:** 2026-04-19  
**Status:** Accepted  
**Severity:** CRITICAL

#### Context

The Anthropic MCP SDK has a by-design Remote Code Execution (RCE) vulnerability in its STDIO transport. `StdioServerParameters` executes arbitrary OS commands with the parent application's privileges **before any validation or connection attempt occurs**. This means that simply configuring an MCP server connection can execute malicious commands on the host system, even if the connection fails.

**Affected CVEs:** CVE-2025-65720, CVE-2026-30623, CVE-2026-30624, CVE-2026-30618, CVE-2026-33224, CVE-2026-30625, CVE-2026-30615, CVE-2026-26015, CVE-2026-40933, CVE-2025-49596, CVE-2026-22252, CVE-2026-22688, CVE-2025-54994, CVE-2025-54136

**Scope:** 7000+ MCP servers, 150M+ downloads affected. Anthropic declined to fix ("expected behavior").

**Impact on ask-ai:** Currently zero — ask-ai has no MCP code. However, P6 (Phase 1) includes MCP Client Integration (P15/Plugin System), making this a future-critical concern.

#### Decision

1. **Never use `StdioServerParameters` directly.** If STDIO transport is supported, it will be through a sandboxed wrapper that validates commands against an explicit allowlist before execution.
2. **Mandatory human confirmation for MCP server installation.** Users must explicitly approve each MCP server, with clear warning about the security implications.
3. **`config.toml` command allowlist.** STDIO MCP server configurations must declare an explicit `allowed_commands` list. Any command not on the list is rejected.
4. **HTTP transport preference.** Prefer HTTP/SSE transport over STDIO wherever possible. STDIO should require explicit opt-in with security acknowledgment.
5. **Extend DEC-004 WASM sandbox to MCP processes.** STDIO MCP servers run inside the same WASM sandbox that plugins use, with `process_spawn` capability denied by default.

#### Consequences

- **Positive:** ask-ai users are protected from the RCE vulnerability by design. The allowlist + sandbox approach means even a malicious MCP server config cannot execute arbitrary commands.
- **Negative:** STDIO MCP servers with complex startup commands may not work out-of-the-box. Users will need to review and approve each server's command list. This is intentional — security over convenience.
- **Relation to DEC-004:** `denied = ["process_spawn"]` is **meaningless** when MCP STDIO transport itself *is* process spawning. DEC-007 fixes this gap by requiring an explicit allowlist and sandbox for STDIO transport, making the DEC-004 capability model effective even with MCP.

#### References

- OX Security: "MCP Vulnerabilities Could Expose AI Apps to RCE, Data Theft and Other Attacks" (2026)
- CVE-2025-65720 et al.
- Anthropic MCP SDK `StdioServerParameters` source code
- DEC-004: WASM Sandbox by Capabilities

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

2026-04-11 - P6 Core Enhancements added, milestone tags [M1]/[M2]/[M3], P4 extras, P5 verbosity merge, P15 sub-items with scope clarification
2026-04-25 - Milestones restructured: M2→UX & TUI Design (design phase), M3→Sprach 2.0+CAS+TUI impl+Plugin System, M4→Future (was M3). P14 TUI split into M2(design) and M3(impl). P7,P14,P15 moved from M2 to M3.