# Implementation Plan for ask-ai

**Note**: This document has been reorganized. Detailed technical documentation is now in `doc/src/development/`.

## Quick Links

- [Architecture](./doc/src/development/architecture.md) - Design decisions and system architecture
- [Roadmap](./doc/src/development/roadmap.md) - Future features and planned improvements
- [Skills System Design](./doc/src/development/skills-system-design.md) - Skills architecture and implementation
- [CLI Tools Research](./doc/src/development/cli-tools-research.md) - External tools reference
- [Contributing](./doc/src/development/contributing.md) - How to contribute to the project

## Current Version

**v0.28.0** - 2026-03-11

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

### 🚨 PRIORITY 1: SOUL.md - AI Personality System

**Status:** 🔄 IN PROGRESS

**Goal:** Define AI personality, behavior, and communication style via user-configurable file.

**Location:** `~/.config/ask-ai/SOUL.md`

**Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│                    Prompt Assembly                          │
├─────────────────────────────────────────────────────────────┤
│  1. SOUL LAYER                                              │
│     ├─ ~/.config/ask-ai/SOUL.md (if exists)                 │
│     └─ PERSONALITY_DEFAULT (fallback when no SOUL.md)      │
│     └─ EMPTY (when --soulless flag)                         │
├─────────────────────────────────────────────────────────────┤
│  2. OPERATION LAYER                                         │
│     └─ Role + Behavior + Tool Usage                         │
├─────────────────────────────────────────────────────────────┤
│  3. CONTEXT LAYER                                            │
│     ├─ Platform info                                        │
│     ├─ System context                                        │
│     └─ AGENTS.md                                             │
├─────────────────────────────────────────────────────────────┤
│  4. CAPABILITY LAYER                                        │
│     ├─ Tools (if enabled)                                   │
│     ├─ Memory (if enabled)                                  │
│     └─ Examples (if tools)                                  │
├─────────────────────────────────────────────────────────────┤
│  5. FINAL INSTRUCTION                                        │
└─────────────────────────────────────────────────────────────┘
```

**Implementation Tasks:**

| Task | Status | File |
|------|--------|------|
| Create `src/soul.rs` module | ⬜ TODO | `src/soul.rs` |
| Add `PERSONALITY_DEFAULT` constant | ⬜ TODO | `src/prompts/base.rs` |
| Remove Pepe personality | ⬜ TODO | DELETE `src/prompts/personality.rs` |
| Update prompt builder | ⬜ TODO | `src/prompts/builder.rs` |
| Add `--soulless` flag | ⬜ TODO | `src/cli/chat.rs`, `src/cli/query.rs` |
| Add module exports | ⬜ TODO | `src/main.rs`, `src/lib.rs`, `src/prompts/mod.rs` |
| Create documentation | ⬜ TODO | `doc/src/SOUL.md` |
| Add unit tests | ⬜ TODO | `src/soul.rs`, `src/prompts/builder.rs` |

**SOUL Processing:**

1. **Load:** Read `~/.config/ask-ai/SOUL.md` (or `XDG_CONFIG_HOME/ask-ai/SOUL.md`)
2. **Clean:** Remove HTML comments (`<!-- ... -->`) using regex
3. **Normalize:** Trim whitespace, collapse blank lines
4. **Validate:** Must have at least one `## ` section
5. **Fallback:** If file missing/invalid, use `PERSONALITY_DEFAULT`

**PromptType Integration:**

| PromptType | Uses SOUL? | Behavior |
|------------|-----------|----------|
| Default | ✅ Yes | SOUL + Role + Context |
| ToolUser | ✅ Yes | SOUL + Role + Context + Tools |
| Code | ❌ No | Role + File Tools only |
| CodeWithTools | ❌ No | Role + File Tools only |
| Summarize | ❌ No | Role (minimal) |

**CLI Flags:**

- `--soulless` - Skip SOUL.md loading, use empty personality layer
- Only applies to `chat` and `query` commands

**Removed:**

- Pepe personality (`PERSONALITY_PEPE` in `src/prompts/personality.rs`) - Users can define their own SOUL.md for custom personalities

**Dependencies:** None

**Estimated effort:** 5-8 hours

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