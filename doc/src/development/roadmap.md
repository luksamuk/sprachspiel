# Roadmap

This document outlines planned features and the current state of Ask-AI.

## Milestones

| Milestone | Codename | Description | Priorities |
|-----------|----------|-------------|------------|
| **[M1]** | Core Evolution | All work before Sprach 2.0 | P0-P6, P8-P13 |
| **[M2]** | Sprach 2.0 | CAS research, cognitive extensions | P7 (S2.1-S2.6), P14, P15 |
| **[M3]** | Future | Deferred, no current priority | Cost tracking, team features, speculation |

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
- `/undo` for removing last response (with database cleanup)
- `/search` (alias `/find`, `/f`) for semantic search
- `/context` (alias `/ctx`) for token metrics
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

**Tools (32 total):**

| Category | Count | Feature Flag | Default |
|----------|-------|--------------|---------|
| Pokémon | 9 | `pokemon-tools` | ✅ Enabled |
| Weather | 3 | `weather-tools` | ✅ Enabled |
| File Operations | 5 | `file-tools` | ✅ Enabled |
| Calculator | 1 | `calc-tools` | ✅ Enabled |
| Web Search (Serper) | 2 | `serper-tools` | ✅ Enabled |
| Web Search (DDG) | 3 | `search-tools` | ❌ Disabled |
| System | 2 | `system-tools` | ✅ Enabled |
| Factual Memory | 3 | (always on) | ✅ Enabled |
| Memory Retrieval | 1 | (always on) | ✅ Enabled |
| Run Command | 1 | (always on) | ✅ Enabled |
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

**Planned: File Write Tools (Priority 2):**

| Tool | Purpose | Status |
|------|---------|--------|
| `write_file` | Create or overwrite files | 📋 Planned |
| `edit_file` | Edit existing files (replace/insert/delete) | 📋 Planned |
| `append_file` | Add content to existing files | 📋 Planned |

See `doc/src/development/file-write-tools.md` for implementation plan.

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

## Upcoming Release

### v0.37.0 (In Development)

**Context Overflow Token Calculation Fixes:**
- Fixed three separate double-counting bugs in token calculation
- `calculate_context_metrics()` was double-counting system + tools
- `needs_inter_tool_compaction()` and related functions fixed
- Pre-tool warning showed wrong remaining tokens
- Pre-tool warning said "Auto-compacting" but only warned (now split logic)
- Duplicate warning removed when tools are enabled

**Percentage-Based Context Thresholds:**
- `MODERATE_USAGE_PERCENT = 0.75` - Warning at 75% usage
- `CRITICAL_USAGE_PERCENT = 0.88` - Auto-compact at 88% usage
- `INTER_TOOL_USAGE_PERCENT = 0.94` - Inter-tool warning at 94%
- `EMERGENCY_USAGE_PERCENT = 0.97` - Emergency truncation at 97%
- Absolute minimums for small contexts (2K, 1K, 512, 256 tokens)

**Context Overflow During Multi-Tool Execution:**
- Token budget verification before each tool execution
- Inter-tool context check with proper token counting
- Emergency truncation when approaching limit
- Per-tool token budgets defined in `TOOL_TOKEN_BUDGETS`

---

## Recent Releases

### v0.36.0 (2026-03-19)

**Features:**
- Welcome banner redesign with Extended Mind ASCII art
- Prompt emojis (`🧠🔧`) replacing `[t][T]` indicators
- `/new` command for new conversation session
- `/session` command group for unified session management
- Session auto-load on startup
- Database initialization failure diagnostics
- Schema migration v6→v7 fix for embedding duplicates

**Changes:**
- `/clear` renamed to `/new`
- `/load` auto-saves current session before switching

### v0.35.0 (TBD)

**Fixes:**
- Context display after compaction - correct token count after session reload

### v0.26.7 (2026-03-09)

**Dead Code Cleanup:**
- Removed unused constants (`MIN_PRESERVE_LAST`)
- Removed unused functions (`count_embedded_messages`, `get_message_chunks`, etc.)
- Removed legacy functions (`set_compacted_summary`, `clear_compacted_summary`, etc.)
- Converted test-only methods to `#[cfg(test)]`
- Fixed `#[allow(dead_code)]` annotations

### v0.26.6 (2026-03-08)

**Integration Tests for Context Overflow:**
- 22 integration tests for overflow protection
- Tests for threshold hierarchy, Unicode truncation, recovery cycles

**Bug Fix:**
- Context builder panic after `/compact` + `/clear`

### v0.26.5 (2026-03-08)

**Error Recovery During Tool Execution:**
- Detects "Context overflow during tool execution" error
- Removes failed messages, auto-compacts, prompts retry

**Pre-Tool Context Check:**
- Checks context at 75% threshold before tool execution
- Auto-compacts if needed to prevent overflow

**Bug Fixes:**
- `/undo` now deletes embeddings from database
- Code mode (-c flag) now works in chat
- Hybrid search supports `exclude_ids` parameter

### v0.26.4 (2026-03-08)

**Token Estimation in Coordinator:**
- Context overflow detection during tool execution (90% threshold)

**Unicode-Safe Tool Result Truncation:**
- `truncate_tool_result()` with charset-safe truncation
- `MAX_TOOL_RESULT_TOKENS = 4000` limit

### v0.26.3 (2026-03-08)

**Bug Fixes:**
- `/undo` deletes embeddings from database
- Fix crash after `/compact` + `/clear`
- Code mode (-c flag) works in chat
- Hybrid search `exclude_ids` parameter

### v0.26.2 (2026-03-05)

**Bug Fix:**
- Token count mismatch between `/context` and Ollama's `prompt_eval_count`

### v0.26.1 (2026-03-04)

**Feature:**
- Source attribution in memory system
- `SourceType` enum with `[msg:N]`, `[doc:N]`, `[note:N]` prefixes

---

## Known Issues

### GLM-OCR Returns Empty Output ✅

**Status:** Fixed in Ollama v0.17.6 (2026-03-04)

Users on rolling-release distros may need to wait for package updates. For immediate fix:
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### Semantic Retrieval Context Framing ✅

**Status:** Completed (v0.22.9)

Fixed with framing text, MEMORY section in system prompt, and explicit instructions.

---

## Critical Bugs (All Fixed)

All critical bugs have been resolved in v0.26.2 - v0.26.7:

| Bug | Status | Version |
|-----|--------|---------|
| Context token count mismatch | ✅ Fixed | v0.26.2 |
| `/undo` incomplete cleanup | ✅ Fixed | v0.26.3 |
| User prompt in hybrid search | ✅ Fixed | v0.26.3 |
| Code mode (-c) not working | ✅ Fixed | v0.26.3 |
| Context panic after `/compact` + `/clear` | ✅ Fixed | v0.26.6 |
| Context exhaustion during tools | ✅ Fixed | v0.26.4-v0.26.6 |
| Context utilization after `/compact` | ✅ Fixed | v0.26.8 |

### Context Utilization After Compaction

**Status:** ✅ FIXED (v0.26.8) - Needs Manual Testing

**Problem:** After running `/compact`, the `/context` command showed incorrect token counts:
- Displayed 100%+ utilization even after successful compaction
- Counted ALL messages including compacted ones
- Wrong message counts in breakdown

**Fix Applied:**
- `history_real_tokens()` now skips compacted messages and includes summary
- `check_context_overflow()` respects `messages_sent_to_llm`
- `/context` display shows active messages and summary tokens correctly

**Needs Manual Testing:**
- Test with long conversations that trigger auto-compact
- Verify `/context` shows reduced tokens after `/compact`
- Test with different compaction scenarios (manual vs auto)

---

## Pending Bugs

### Context Not Cleared After /compact

**Status:** ✅ FIXED (v0.27.2)

**Problem:** After `/compact`, context utilization remained high/overflow. User needed `/clear` to actually free space.

**Cause:** `prompt_tokens` values stored in messages still reflected the old (larger) context size after compaction.

**Fix:** `set_compacted_summary_with_range()` now clears `prompt_tokens` from all messages. The next LLM interaction will receive fresh token counts reflecting the reduced context.

### Markdown in Compaction Summary

**Status:** ✅ FIXED (v0.27.2)

**Problem:** Compaction summary was plain text, not formatted as markdown.

**Fix:**
- Updated compaction prompt to request structured markdown output
- Changed `println!` to `markdown::print_markdown()` for proper rendering
- Summary now includes sections: Key Topics, Decisions Made, Technical Details, Action Items

### Web Scraping Content Quality

**Status:** ✅ FIXED (v0.27.2)

**Problem:** Web fetch tool sometimes returned raw HTML/CSS instead of clean markdown.

**Fix:**
- Added `clean_html()` function to extract main content (`<main>`, `<article>`, etc.)
- Prioritizes semantic content over navigation, sidebars, ads
- Added `truncate_content()` with safe UTF-8 boundary handling
- Content limited to 50,000 characters to prevent memory issues
- Shows "(truncated)" indicator when content is limited

---

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
| SQLite | 🟢 Primary | `~/.local/share/ask-ai/embeddings.db` |
| JSON | 🟡 Backup | Only for `/export json` and `/restore` command |

**Note:** `/restore` command is kept indefinitely for disaster recovery from JSON backups.

---

### Specialized Agent Architecture

**Priority:** HIGH  
**Status:** Planned (Issue #12)

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

| Type | Model | Tools | Purpose |
|------|-------|-------|---------|
| `ocr` | glm-ocr:bf16 | run_command(tesseract) | Image text extraction |
| `vision` | moondream:1.8b | - | Image analysis |
| `translate` | translategemma:4b | - | Translation |
| `summarize` | (same model) | - | Summarization |
| `document` | (same model) | run_command(pdftotext) | PDF/EPUB extraction |

**Planned Commands:**

| Command | Description |
|---------|-------------|
| `/ocr <image>` | OCR via specialized agent |
| `/vision <image>` | Image analysis via specialized agent |
| `/translate <lang> <text>` | Translation via specialized agent |
| `/summarize <text>` | Summarization via specialized agent |

**Technical Debt Resolved:**
- `import_document` calling `Command::new()` directly → uses `spawn_subagent(type="document")`
- Skills can now override document-processing behavior at project level

**Implementation:** See `IMPLEMENTATION.md` - Priority 4

---

## High Priority

### CLI Tools Infrastructure (Phase 1)

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

---

### CLI Tools Infrastructure (Phase 2)

**Priority:** MEDIUM
**Status:** NOT STARTED

**Planned Features:**
- [ ] PDF pipeline skill (pdftotext + pdftoppm + tesseract)
- [ ] Image metadata skill (exiftool)
- [ ] Image conversion skill (imagemagick)
- [ ] Skills system integration

**Dependencies:** Phase 1 (complete)

---

---

### Skills System (Extended)

**Priority:** MEDIUM (Extended from HIGH Phase 2)  
**Status:** Research Complete

**Goal:** Refine skills system for advanced use cases.

**Extended Tasks:**

**Phase 1: Core Skills** (covered in HIGH priority section above)
- Basic Markdown loading
- Prompt injection
- Builtin skills

**Phase 2: Advanced Features** (future)
- [ ] YAML frontmatter parsing
- [x] Skill invocation via `/skill <name>` (completed PR #87)
- [ ] Skill composition (multiple skills active)
- [ ] Skill dependencies (skill A requires skill B)
- [ ] Project-level skill discovery
- [ ] Skill hot-reload during development

**Phase 3: Integration** (future)
- [ ] Integration with Document Import Tool
- [ ] User skill sharing (community repository?)
- [ ] Skill versioning

**Research Complete:** See [CLI Tools Research](./cli-tools-research.md) and [Skills System Design](./skills-system-design.md) for full details.

---

### Parallel Tool Execution

**Priority:** MEDIUM (PRIORITY 6 in implementation)  
**Status:** Research needed

**Goal:** Execute independent tool calls in parallel for faster response times.

**Problem:**
- Current implementation executes tool calls sequentially
- LLM often requests multiple independent tools (e.g., `get_weather` + `get_current_datetime`)
- Sequential execution unnecessarily increases latency

**Proposed Solution:**
- Detect independent tool calls using dependency analysis
- Execute read-only tools in parallel using `futures::join_all`
- Preserve sequential order for stateful tools (file writes, database ops)

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

**Estimated effort:** 3-4 days

**Implementation:** See `IMPLEMENTATION.md` - Priority 6

---

### File Session State

**Priority:** Medium  
**Status:** Research needed

**Goal:** Explicit tracking of file operations for context reduction and security.

**Tasks:**
- [ ] Research: File tracking patterns
- [ ] Design: Session state structure
- [ ] Implement: File tracking in session
- [ ] Implement: Security constraints

---

### Document Import Tool

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

### SQL ORM Evaluation

**Priority:** MEDIUM  
**Status:** Research Needed

**Tasks:**
- [ ] Research: `sqlx` vs `sea-orm` trade-offs
- [ ] Benchmark: Binary size impact
- [ ] Prototype: ORM for simple queries

---

### Skills System

**Priority:** Medium  
**Status:** Research needed

**Goal:** Load custom behaviors from `.ask-ai/skills/` or `~/.config/ask-ai/skills/`.

**Tasks:**
- [ ] Research: Skill systems in other agents
- [ ] Design: Skill file format
- [ ] Implement: Skill parser
- [ ] Implement: `--skill` flag

---

### Effective AI Coding Agents Analysis

**Priority:** Medium  
**Status:** Research Complete

**Goal:** Apply lessons learned from academic research on terminal-native AI agents.

Analysis of the paper "Building Effective AI Coding Agents for the Terminal" (OPENDEV, arXiv:2603.05344v2) comparing best practices with ask-ollama-rs architecture.

**Key Findings:**
- ask-ollama-rs implements ~60-70% of recommended patterns
- Strong alignment: Context Engineering (hybrid retrieval), Session Management, Tool System
- Gaps: Memory System (structured facts), System Reminders, Adaptive Compaction

**Recommendations:**
- Memory System for extracted facts (integrates with planned Notes + Document Import)
- System Reminders for instruction fade-out mitigation
- Per-workflow model selection for resource optimization

**Full Analysis:** See [Effective Agents Analysis](./effective-agents-analysis.md) for detailed comparison, code references, and implementation roadmap.

---

### P6: Core Enhancements [M1]

**Priority:** P6 (before Sprach 2.0, after current P1/P4/P5)  
**Status:** 📋 PLANNED / 🟡 RESEARCH

| ID | Feature | Status | Effort |
|----|---------|--------|--------|
| P6.0 | Multi-Provider Support (OpenAI-Compatible) | 📋 Planned | 4-7 weeks |
| P6.1 | Auto Fact Extraction (autoDream-lite) | 📋 Planned | 3-5 days |
| P6.2 | Context Pinning | 🟡 Research | 2-4 days |
| P6.3 | Dynamic Context Limits | 🟡 Research | 1-2 days |
| P6.4 | Secret Scanning (Content) | 📋 Planned | 1-2 days |

**Also in P4 (Code Quality extras) [M1]:**

| Feature | Status | Effort |
|---------|--------|--------|
| Memory Staleness Warnings | 📋 Planned | 0.5 day |
| Truncation Warnings | 📋 Planned | 0.5 day |

**P5 merge [M1]:** Verbosity Configuration merged with `log` crate item — single implementation covering both logging levels and configurable verbosity (quiet/normal/verbose/debug).

---

### Sprach 2.0: CAS Research [M2]

**Priority:** P7 (after all P1-P5 current items are resolved)  
**Status:** 🟡 RESEARCH NEEDED  
**Reference:** `~/git/biblio/sprach-2-0-auto-analise.org`  
**Full Design:** See [Sprach 2.0 Research](./sprach-2-0-research.md) for open questions, code analysis, and implementation details.

Self-analysis identifying ask-ai-rs as a Complex Adaptive System (CAS) with emergent properties but limited open-endedness. Proposals aim to increase emergent connectivity and adaptive behavior.

| ID | Proposal | Depends On | Status | Effort |
|----|----------|------------|--------|--------|
| S2.1 | Visualize Connections Tool | None | 🟡 Research | 2-3 days |
| S2.2 | Content Relations Graph (2-layer) | S2.1 | 🟡 Research | 5-8 days |
| S2.3 | Reflection on Triggers + Curation | S2.1, S2.2 | 🟡 Research | 4-7 days |
| S2.4 | Plugin System (WASM) | — | 📋 P15 (existing) | 3-4 weeks |
| S2.5 | SOUL.md Patching + `/apply-patch` | S2.3 | 🟡 Research | 3-5 days |
| S2.6 | Skills Auto-Registration (Meta) | S2.1-S2.5 | 🕐 Wait | TBD |

**Validated Decisions (DEC-001 to DEC-006):**

| Decision | Ruling | Validation |
|----------|--------|------------|
| DEC-001 | Cache incremental for relations (on-demand, not pre-computed) | GraphSeek 2026, Graph RAG 2026 |
| DEC-002 | Reflection triggers (not periodic) | ICML 2025, MeCo arXiv 2025 |
| DEC-003 | Curation with human approval (drafts, not auto-publish) | Rewire.it, "Human-in-the-loop" |
| DEC-004 | WASM sandbox by capabilities (allowed/denied, not total isolation) | The New Stack 2026, MCP-SandboxScan |
| DEC-005 | Semantic versioning for plugins (major equal, minor ≥) | OpenFang, "Semver + manifest signing" |
| DEC-006 | SOUL.md patches require human approval (suggestions, not automatic) | MetaMind NeurIPS 2025, "Human oversight" |

**Competitors:** Joplin GSoC 2026 (note graphs with AI), OpenClaw (WASM sandbox for community skills)

---

## Low Priority

### Plugin System

**Priority:** Low  
**Status:** Not started

User-defined tools via dynamic loading or compilation.

---

### TUI (Terminal User Interface)

**Priority:** Low  
**Status:** 🟡 IN PROGRESS (Architecture refactoring)

**Goal:** Build a responsive TUI using Ratatui-rs.

**Architecture Preparation (Current Phase):**
- ✅ `InputBackend` trait - abstracts input handling (Phase 1-5 complete)
- ✅ `ChatView` trait - abstracts output rendering
- ✅ `ReplState` struct - separates state from I/O
- ✅ `core.rs` - business logic isolated from I/O
- 📋 Phase 7-9: Command handlers extraction, refactoring, tests

**Future Tasks:**
- [ ] Research: Ratatui-rs best practices
- [ ] Research: Terminal resize handling patterns
- [ ] Design: UX wireframes for main views
- [ ] Prototype: `TuiInput` implementing `InputBackend`
- [ ] Prototype: `TuiView` implementing `ChatView`

**Reference:** See `IMPLEMENTATION.md` - Priority 3 for refactoring progress.

---

### Multilingual Injection Detection

**Priority:** Low  
**Status:** Not started

**Tasks:**
- [ ] Research: Injection patterns in non-English languages
- [ ] Implement: Multilingual pattern detection

---

## Low Priority

### Multi-Provider Support (OpenAI-Compatible Backends) → Moved to P6.0 [M1]

**Priority:** P6.0 (was LOW, upgraded)  
**Status:** 📋 PLANNED  
**Issue:** #72

> **NOTE:** This feature has been upgraded from LOW priority to P6.0 and is now tracked in IMPLEMENTATION.md under PRIORITY 6: Core Enhancements. The detailed implementation plan is in the P6.0 section of IMPLEMENTATION.md. This roadmap section is kept for architectural reference.

**Goal:** Abstract provider differences to support both Ollama (local) and OpenAI-compatible APIs (cloud/local) through a unified interface.

**Motivation:**
- **Performance:** llama.cpp server with OpenAI-compatible endpoints can be faster than Ollama for local models
- **Primary Target:** OpenAI API - enable cloud-based models for users without local GPU
- **Compatibility Targets:**
  - llama.cpp (via OpenAI-compatible `/v1/chat/completions` endpoint)
  - LM Studio (same pathway)
- **Deferred:** vLLM support postponed until testing resources available

**Current State:**
- `ollama-rs` is tightly coupled to Ollama's native API
- Tool calling uses `ollama-rs::generation::tools::Tool` trait
- Embeddings use `/api/embeddings` endpoint
- Model capabilities detected via `/api/show_model_info`
- Context overflow depends on `prompt_eval_count` from responses

**Architecture:**

```rust
/// Provider abstraction for LLM backends
trait LlmProvider {
    /// Send chat request with optional tools
    async fn chat(&self, messages: Vec<ChatMessage>, tools: Vec<Tool>) -> Result<ChatResponse>;
    
    /// Generate completion (for non-chat use cases)
    async fn generate(&self, prompt: &str, options: GenerateOptions) -> Result<GenerateResponse>;
    
    /// Get model capabilities (tools, vision, thinking)
    async fn get_model_capabilities(&self, model: &str) -> Result<ModelCapabilities>;
    
    /// Generate embeddings for text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    
    /// Check provider health/availability
    async fn is_available(&self) -> bool;
}

/// Ollama native provider (current implementation)
struct OllamaProvider { 
    client: ollama_rs::Ollama,
    base_url: String,
}

/// OpenAI-compatible provider (OpenAI API, llama.cpp, LM Studio)
struct OpenAICompatibleProvider { 
    client: reqwest::Client, 
    base_url: String,
    api_key: Option<String>,
}

/// Provider selector based on model configuration
enum Provider {
    Ollama(OllamaProvider),
    OpenAI(OpenAICompatibleProvider),
}
```

**Key Design Decisions:**

| Decision | Rationale |
|----------|-----------|
| Adapter pattern | Clean separation, existing Ollama code unchanged |
| Enabled by default | No feature flag complexity |
| Per-model provider | User can mix local (Ollama) + cloud (OpenAI) |
| Config-based capabilities | OpenAI-compatible servers don't have `/api/show_model_info` |

**Impact Analysis:**

| Component | Ollama API | OpenAI-Compatible | Effort | Notes |
|-----------|------------|-------------------|--------|-------|
| Chat/Query | `/api/chat` | `/v1/chat/completions` | LOW | Same message format |
| Tool Calling | Native tools | Function calling | HIGH | Format conversion required |
| Embeddings | `/api/embeddings` | `/v1/embeddings` | HIGH | Response format differs |
| Vision/OCR | `/api/generate` + images | Vision messages | MEDIUM | Image encoding differs |
| Capabilities | `/api/show_model_info` | Config-based | LOW | No API for this |
| Context Overflow | `prompt_eval_count` | `usage.prompt_tokens` | LOW | Minor parsing change |

**Configuration:**

```toml
# ~/.config/ask-ai/models.toml

# Default provider (ollama or openai)
default_provider = "ollama"

# Provider-specific settings
[providers.ollama]
base_url = "http://localhost:11434"

[providers.openai]
base_url = "https://api.openai.com/v1"  # or "http://localhost:8080/v1" for llama.cpp
api_key = "${OPENAI_API_KEY}"  # Environment variable

# Per-model provider override
[models."gpt-4o"]
provider = "openai"
tools = true
vision = true
thinking = false

[models."qwen3.5:4b"]
provider = "ollama"  # Explicit, but default anyway
tools = true
vision = true
thinking = false
```

**Implementation Phases:**

| Phase | Status | Description | Effort |
|-------|--------|-------------|--------|
| 1. Provider Trait | 📋 Planned | Define `LlmProvider` trait | 1 day |
| 2. Ollama Adapter | 📋 Planned | Wrap existing `ollama-rs` in adapter | 2 days |
| 3. OpenAI Adapter | 📋 Planned | Implement OpenAI-compatible client | 3 days |
| 4. Tool Conversion | 📋 Planned | Ollama ↔ OpenAI tool format conversion | 2 days |
| 5. Embeddings | 📋 Planned | Unified embedding interface | 2 days |
| 6. Vision/OCR | 📋 Planned | Image support through providers | 1 day |
| 7. Configuration | 📋 Planned | Provider per model in `models.toml` | 1 day |
| 8. Testing | 📋 Planned | Integration tests with real providers | 2 days |
| **Total** | | | **14 days** |

**Key Files to Modify:**

| File | Change |
|------|--------|
| `src/provider/mod.rs` | NEW - Provider trait and registry |
| `src/provider/ollama.rs` | NEW - OllamaProvider adapter |
| `src/provider/openai.rs` | NEW - OpenAICompatibleProvider |
| `src/settings.rs` | Provider configuration parsing |
| `src/capabilities.rs` | Config-based capability detection |
| `src/chat/custom_coordinator.rs` | Use `LlmProvider` trait instead of `ollama-rs` directly |
| `src/embeddings/client.rs` | Provider abstraction for embeddings |

**Dependencies:**
- None (adapter abstraction, enabled by default)

**Notes:**
- OpenAI API is the **primary implementation target** for cloud-based models
- llama.cpp and LM Studio gain compatibility **through** OpenAI-compatible adapter
- vLLM support **postponed** until testing resources available
- Ollama remains primary **local** provider; OpenAI for **cloud** models
- Users can mix providers: Ollama for local + OpenAI for specific cloud models
- Tool calling conversion is bidirectional (Ollama ↔ OpenAI format)
- No feature flag - abstraction is internal architecture detail

**References:**
- OpenAI API Reference: https://platform.openai.com/docs/api-reference/chat
- llama.cpp OpenAI-compatible server: https://github.com/ggerganov/llama.cpp/blob/master/examples/server/README.md
- LM Studio API: https://lmstudio.ai/docs/python
- Current `settings.rs` - configuration structure
- Current `chat/custom_coordinator.rs` - tool execution flow
- Current `embeddings/client.rs` - embedding client
- Current `capabilities.rs` - model capability detection

---

## [M3] Future — Deferred

Features explicitly deferred with no current priority:

| Feature | Reason |
|---------|--------|
| AutoDream full daemon (4-phase) | After Sprach 2.0 |
| Cost Tracking | After Sprach 2.0 |
| Multi-scope Memory (team/private) | Not applicable — harness is not code-focused |
| Context Collapse | Observe, don't implement |
| VCR Testing | When CI is robust |
| Speculation | Indefinite deferral |
| Remote Agent | P15+ |
| Team Memory Sync | Team use only |
| Remote Managed Settings | Enterprise |
| Worktree-aware sessions | Niche |
| Thinkback Marketplace | Too premature |
| Session Summary / Away Summary | Descarted — session continuation already works |

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

1. Check [GitHub Issues](https://github.com/luksamuk/ask-ai-rs/issues)
2. Comment on the issue
3. Submit a pull request

## See Also

- [Architecture](./architecture.md) - Technical architecture
- [Contributing](./contributing.md) - How to contribute
- [CHANGELOG](../CHANGELOG.md) - Version history