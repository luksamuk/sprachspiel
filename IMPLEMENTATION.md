# Implementation Plan for ask-ai

**Note**: This document has been reorganized. Detailed technical documentation is now in `doc/src/development/`.

## Quick Links

- [Architecture](./doc/src/development/architecture.md) - Design decisions and system architecture
- [Roadmap](./doc/src/development/roadmap.md) - Future features and planned improvements
- [Skills System Design](./doc/src/development/skills-system-design.md) - Skills architecture and implementation
- [CLI Tools Research](./doc/src/development/cli-tools-research.md) - External tools reference
- [Contributing](./doc/src/development/contributing.md) - How to contribute to the project

## Current Version

**v0.31.0** - 2026-03-12

## Current Implementation Status

✅ **Completed:**

- Core CLI with 5 subcommands (query, chat, translate, ocr, summarize)
- Interactive chat mode with persistent sessions
- Custom models support via `~/.config/ask-ai/models.toml`
- Built-in models: llama3.1, translategemma, glm-ocr (user models in config)
- Thinking support for cloud models (configurable via `thinking = true`)
- Dynamic model selection with capability detection
- Tool integration with error recovery (28 tools in 8 categories)
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

### 🔴 PRIORITY 0: Context Continuity with Graceful Interruption

**Status:** ✅ COMPLETED (v0.31.0)

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

pub enum ChatEvent {
    // ... existing variants ...
    ContinuationNeeded { tag: ContinuationTag },
}
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

---

### 🔴 PRIORITY 3: Skills System Phase 1

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

### 🟡 PRIORITY 4: Document Import Tool

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

### 🟡 PRIORITY 5: Chat Module Integration

**Status:** ❌ NOT STARTED

**Goal:** Use OCR/Vision/Translate/Summarize from chat.

**Features:**
- `/ocr`, `/vision`, `/translate`, `/summarize` commands in REPL
- Model switching during commands
- Design: temporary context or persistent?

**Dependencies:** None

**Estimated effort:** 3-5 days

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