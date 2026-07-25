# Completed Features

This document lists features that have been implemented and are available in the current release. For planned features, see [Roadmap](./roadmap.md). For per-issue tracking, see [Feature Status](./feature-status.md).

## Implemented Features

**Core CLI:**
- 5 subcommands (query, chat, translate, ocr, summarize)
- 3 built-in model presets (llama3.1, translategemma, glm-ocr)
- User-defined models via `~/.config/sprachspiel/models.toml`
- Optional model parameters (top_p, temperature)
- Thinking support for cloud models (`thinking = true` in config)
- Standalone monochrome markdown renderer
- Model capability detection (tools, vision, ocr)
- Pipe support for all commands
- Debug mode, Think mode, Code mode
- Configuration file support (`~/.config/sprachspiel/config.toml`)
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
  - `/retry` for regenerating last response
  - `/undo` for removing last response (with database cleanup)
  - `/search` for semantic search
  - `/context` for token metrics
- Tab completion for commands and models
- Mode indicators in prompt (`[t]`, `[T]`)
- Token metrics display
- Thinking output visible when enabled
- Error recovery for tool/network errors
  - Typed error classification with `OllamaError` (not string heuristics)
  - `JsonError` (parsing failures) now recoverable - LLM can self-correct malformed tool calls
  - Network errors, unknown tools, invalid arguments remain recoverable
- Context overflow protection during tool execution
- **Context Continuity with Graceful Interruption** (v0.31.0)
  - LLM can pause reasoning when context fills up
  - Automatic continuation after compaction
  - Nested continuations (up to 3 levels)
  - Context status injected into prompts
- **Feedback-Driven Memory** (v0.40.0)
  - Explicit feedback signals (Good/Bad/Correction) with decay-weighted RRF fusion
  - Content decay activation (Ebbinghaus for content_items)
  - Access tracking (retrieval reinforces retention)
  - `/feedback` command and `/content prune` command

**Tools (50 total):**

| Category | Count | Feature Flag | Default |
|----------|-------|--------------|---------|
| Pokémon | 9 | `pokemon-tools` | ✅ Enabled |
| Weather | 3 | `weather-tools` | ✅ Enabled |
| File Read | 5 | `file-tools` | ✅ Enabled |
| File Write | 3 | `file-tools` | ✅ Enabled |
| Calculator | 1 | `calc-tools` | ✅ Enabled |
| Web Search (DuckDuckGo) | 3 | `search-tools` | ❌ Opt-in |
| System | 2 | `system-tools` | ✅ Enabled |
| Factual Memory | 3 | (always on) | ✅ Enabled |
| Memory Retrieval | 1 | (always on) | ✅ Enabled |
| Notes | 3 | (always on) | ✅ Enabled |
| Document Import | 1 | `document-tools` | ✅ Enabled |
| Run Command | 1 | (always on) | ✅ Enabled |
| Subagent | 4 | `subagent-tools` | ✅ Enabled |
| Skills | 2 | `skills-tools` | ✅ Enabled |
| Feedback | 3 | (always on) | ✅ Enabled |
| Finance | 1 | `finance-tools` | ❌ Disabled |
| LED Control | 5 | `led-tools` | ❌ Disabled* |

*LED tools require `[led]` configuration in config.toml.

**Factual Memory (v0.33.0):**
- Persistent fact storage across sessions
- User commands: `/fact add`, `/fact list`, `/fact search`, `/fact remove`, `/fact prune`
- LLM tools: `fact_add`, `fact_search`, `fact_remove` for autonomous fact management
- Heuristic classification: preferences vs facts
- Scope: project-specific vs global facts
- Conflict resolution: duplicate detection, contradiction handling
- Automatic decay: Ebbinghaus forgetting curve (180d preferences, 30d facts)
- FTS5 full-text search with BM25 scoring
- Prompt injection: Facts injected into system prompt (max 2200 chars)

**File Write Tools (✅ Completed):**

| Tool | Purpose | Status |
|------|---------|--------|
| `write_file` | Create or overwrite files | ✅ Completed |
| `edit_file` | Edit existing files (replace/insert/delete) | ✅ Completed |
| `append_file` | Add content to existing files | ✅ Completed |

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

## Recent Releases

### v0.44.0 (2026-05-25)

**Responsive Chat Rebuild (W6, Issues #145–#148, PR #155):**
- Ratatui-based responsive rendering at any terminal width (replaces println+ANSI)
- CrosstermInput with ratatui-textarea (replaces rustyline)
- App event loop with crossterm key events, tokio mpsc channels
- Streaming compaction, tool message ordering, inter-tool thinking
- Intelligent table reflow, Mermaid diagram rendering, catppuccin code blocks
- Mouse scroll, text selection, floating completion menu, bracketed paste
- Ctrl+C context-dependent copy/cancel (4 priority levels)
- `/toggle-style`, `/reindex --yes`, embedding progress indicator
- Provider-agnostic error strings, CompactionContext refactor
- Flaky test fix with `#[serial_test::serial]`

**Changed:**
- Removed TerminalView (println-based), RustylineInput, termimad dependency
- Removed ~40 command shortcuts/aliases
- Standalone monochrome markdown renderer replaces termimad
- Key binding overhaul with explicit mappings

### v0.43.0 (2026-05-11)

**Features:**
- Visual indicators for tool actions
- Proactive skill loading
- Tool call display decoupled from `log` crate
- `run_command` tilde expansion with blocklist
- SF4: Logging overhaul (MultiLogger, file logging, data sensitivity)
- SF5: Agent spawning tools (4 dedicated tools)
- Auto fact extraction, feedback infrastructure, semantic dedup
- Triple-based contradiction disambiguation
- Fact dedup pipeline centralized in `src/facts/dedup.rs`

### v0.42.0 (2026-05-01)

**Features:**
- OCR Prompt Strategy — model-aware prompt selection for vision models
- `/ocr` command accepts optional mode parameter (text, table, figure, formula)
- `spawn_ocr_agent` tool accepts `ocr_mode` parameter

**Bug Fixes:**
- Unicode panic on string truncation in chat resume
- Context overflow during multi-tool execution
- FTS schema mismatch fix (PR #87)

### v0.41.0 (2026-04-28)

**Features:**
- Specialized Agent Architecture — dedicated OCR/vision/translate/summarize spawning tools
- Removed generic `spawn_subagent` and `spawn_document_agent`
- Removed hardcoded PDF pipeline from Rust (LLM orchestrates via skills)

### v0.40.0 (2026-04-11)

**Features:**
- Document Import Tool — TXT, MD, ORG import with semantic search
- Query module refactoring — reduced cognitive complexity
- DB rename: `embeddings.db` → `sprachspiel.db` + `--db` CLI flag
- Logging overhaul — `MultiLogger` with file logging, data sensitivity policy
- Agent spawning tools — 4 dedicated tools replacing generic `spawn_subagent`

### v0.39.5 (2026-04-07)

**Features:**
- Enhanced TODO tools — CRUD gaps, priority, tags
- `/forget --yes` confirmation for destructive command
- `/skill <name>` subcommand (namespace collision prevention)
- Content staleness indicators in facts prompt
- Truncation warnings in tool outputs

### v0.39.0 (2026-04-05)

**Features:**
- Document Import Tool with `/doc import`, `/doc list`, `/doc show`, `/doc delete`

### v0.38.0 (2026-04-03)

**Features:**
- Skills System — `skill_list`, `skill_view` tools, `/skill <name>` command

### v0.37.0 (2026-03-29)

**Features:**
- Context overflow during multi-tool execution
- Inter-tool compaction (automatic context management during tool chains)
- Percentage-based context thresholds
- Embedding fallback for oversized content

## SQLite as Single Storage

**Priority:** HIGH  
**Status:** 🟢 COMPLETE (v0.28.0)

**Goal:** Migrate from dual storage (JSON + SQLite) to SQLite as the single source of truth.

### Completed Work

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1: Schema | ✅ Done | Schema v4 with session metadata columns |
| Phase 2: ChatSession | ✅ Done | `save_sqlite()` / `load_sqlite()` implemented |
| Phase 3: Restore | ✅ Done | `/restore` command + auto-migration on startup |
| Phase 4: Commands | ✅ Done | `/save`, `/load`, `/list` use SQLite |
| Phase 5: Testing | ✅ Done | Basic tests pass |
| Phase 6: Cleanup | ✅ Done | Project identification moved to `project.rs` |
| User Documentation | ✅ Done | Updated `chat.md` with SQLite storage model |

### Current State

| Storage | Status | Description |
|---------|--------|-------------|
| SQLite | 🟢 Primary | `~/.local/share/sprachspiel/sprachspiel.db` |
| JSON | 🟡 Backup | Only for `/export json` and `/restore` command |

**Note:** `/restore` command is kept indefinitely for disaster recovery from JSON backups.

## Specialized Agent Architecture

**Priority:** HIGH  
**Status:** ✅ COMPLETE

**Goal:** Delegate specialized tasks (OCR, vision, document extraction, translation, summarization) to one-shot agents with optimized models.

**Problem:**
- OCR/Vision/Translate/Summarize are standalone CLI commands, not integrated with chat
- Document import calls `Command::new()` directly, bypassing skills system
- Skills can be overridden at project level, but tools don't respect overrides

**Architecture:**

| Aspect | Main Agent | Specialized Agent |
|--------|------------|-------------------|
| Context | Full history + memory + database | One-shot (no history) |
| Database | Yes (SQLite) | No |
| Thinking | Optional | Never (output only) |
| Output | Returns to user | Returns to Main Agent |
| Model | User's chat model | Configured per type |

**Subagent Types:**

| Type | Model | API | Purpose |
|------|-------|-----|---------|
| `ocr` | glm-ocr:bf16 | /api/generate | Image text extraction |
| `vision` | moondream:1.8b | /api/generate | Image analysis |
| `translate` | translategemma:4b | /api/chat | Translation |
| `summarize` | (same model) | /api/chat | Summarization |
| `document` | (same model) | /api/chat | PDF/EPUB extraction |

**v0.42.0-dev Update — OCR Prompt Strategy:**
- OCR prompts now adapt to model type: GLM-OCR keeps rigid prefixes, vision models use descriptive restricted prompts
- `/ocr` chat command accepts optional mode parameter (text, table, figure, formula)
- `spawn_ocr_agent` tool accepts `ocr_mode` parameter for LLM-driven mode selection
- All 3 OCR entry points (CLI, `/ocr`, `spawn_ocr_agent`) use model-aware prompt selection

**Chat Commands:**

| Command | Description |
|---------|-------------|
| `/ocr <image>` | OCR via specialized agent |
| `/vision <image>` | Image analysis via specialized agent |
| `/translate <lang> <text>` | Translation via specialized agent |
| `/summarize <text>` | Summarization via specialized agent |

**LLM Tools:**

| Tool | Description |
|------|-------------|
| `spawn_ocr_agent(prompt, file_path, ocr_mode?)` | Extract text from images via OCR |
| `spawn_vision_agent(prompt, file_path)` | Analyze or describe images via vision model |
| `spawn_translate_agent(prompt)` | Translate text between languages |
| `spawn_summarize_agent(prompt)` | Summarize long text |

**Technical Debt Resolved:**
- Replaced generic `spawn_subagent` with dedicated per-type tools (clearer LLM docstrings, no irrelevant parameters)
- PDF/EPUB import: `import_document` no longer accepts PDF/EPUB directly; extract via `run_command("pdftotext")` first

**Key Files:**
- `src/chat/subagent.rs` - SubagentRunner and SubagentConfig
- `src/tools/subagent_tools.rs` - spawn_ocr_agent, spawn_vision_agent, spawn_translate_agent, spawn_summarize_agent
- `src/prompts/tools.rs` - Tool prompts for spawn tools

**Implementation:** See `IMPLEMENTATION.md` - Priority 4

## CLI Tools Infrastructure (Phase 1)

**Priority:** HIGH  
**Status:** ✅ COMPLETE (v0.29.0)

**Goal:** Secure external CLI tool integration with sandboxing.

**Implementation:**
- External module with types, config, platform detection
- `check_tool_availability()` - Check installed tools
- `run_command()` - Secure command execution with:
  - No shell features (pipes, redirects blocked)
  - Mandatory whitelist validation
  - head/tail parameters for LLM-controlled output
  - Landlock sandbox (Linux 5.13+, enabled by default)
  - Platform-specific handling (Termux, macOS documented)

**Security:**
- Pattern validation blocks: `|`, `;`, `&&`, `||`, `$()`, backticks, redirects
- Landlock filesystem isolation on Linux
- Graceful degradation on older kernels / non-Linux platforms
- User can disable sandbox via `enable_sandbox = false`

**Documentation:** See [run_command Redesign](./run-command-redesign.md)

## Document Import Tool

**Priority:** HIGH  
**Status:** ✅ COMPLETE (v0.39.0)

**Goal:** Import documents for semantic search and retrieval.

**Implemented:**
- TXT/MD/ORG: Builtin support
- PDF/EPUB: External tools (pdftotext, epub2txt) via skills-tools feature
- Chunking with overlap (~512 tokens)
- `/doc import`, `/doc list`, `/doc show`, `/doc delete` commands
- Integration with `remember()` tool for retrieval

**Technical Debt:** PDF/EPUB extraction calls `Command::new()` directly, bypassing skills system. Will be resolved in Specialized Agent Architecture (Priority 4).

---

## See Also

- [Roadmap](./roadmap.md) — Planned features
- [Feature Status](./feature-status.md) — Per-issue tracking
- [Implementation Status](./implementation-status.md) — Current snapshot
- [Changelog](../CHANGELOG.md) — Version history