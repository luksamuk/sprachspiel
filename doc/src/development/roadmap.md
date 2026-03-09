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
- `/undo` for removing last response (with database cleanup)
- `/search` (alias `/find`, `/f`) for semantic search
- `/context` (alias `/ctx`) for token metrics
- Tab completion for commands and models
- Mode indicators in prompt (`[t]`, `[T]`)
- Token metrics display
- Thinking output visible when enabled
- Error recovery for tool/network errors
- Context overflow protection during tool execution

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

## Recent Releases

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
**Status:** 🟢 ~95% COMPLETE (v0.27.x)

**Goal:** Migrate from dual storage (JSON + SQLite) to SQLite as the single source of truth.

### Completed Work

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1: Schema | ✅ Done | Schema v4 with session metadata columns |
| Phase 2: ChatSession | ✅ Done | `save_sqlite()` / `load_sqlite()` implemented |
| Phase 3: Restore | ✅ Done | `/restore` command + auto-migration on startup |
| Phase 4: Commands | ✅ Done | `/save`, `/load`, `/list` use SQLite |
| Phase 5: Testing | ✅ Done | Basic tests pass |
| Phase 6: Cleanup | 🔄 In Progress | `ConversationStorage` deprecated, `repl.rs` clean |

### Current State

| Storage | Status | Content |
|---------|--------|---------|
| SQLite | 🟢 Primary | Full session state (messages + metadata + todos) |
| JSON | 🟡 Legacy | Still used by `/export json` and internal migration |

### Recent Changes (v0.27.x)

- `ConversationStorage` now marked with `#[deprecated]`
- `repl.rs` no longer instantiates `ConversationStorage`
- `migrate_all_legacy_sessions()` creates storage internally
- `restore_session()` creates storage internally
- Removed `save()` and `load()` deprecated methods from `ChatSession`
- `legacy_check.rs` uses `storage.load_session()` directly

### Remaining Tasks

- [ ] Consider removing `history.rs` entirely in future version
- [ ] Update user documentation for storage model
- [ ] Consider removing `SessionInfo` struct (only used for legacy listing)

---

## High Priority

### Memory Enhancement Part 1 (Phases 1-3)

**Priority:** HIGH  
**Status:** Phase 1 Complete, Phase 2-3 NOT STARTED

#### Phase 1: Source Attribution ✅ (v0.26.1)

Completed:
- ✅ `SourceType` enum with `prefix()` and `from_prefix()` methods
- ✅ Context formatted with source labels: `[msg:N]`, `[doc:N]`, `[note:N]`
- ✅ `remember` tool updated to use source type prefixes

#### Phase 2: Query Routing ❌ NOT STARTED

**Goal:** Route queries to appropriate search targets (conversations, docs, notes).

**Current State:**
- No query routing implementation exists
- All searches go through `search_hybrid()` without target discrimination

**Tasks:**
- [ ] Collect real query patterns from usage
- [ ] Prototype regex routing (pt-BR + en)
- [ ] Benchmark embedding-based routing latency
- [ ] Test `whatlang` crate for language detection

#### Phase 3: Timestamp Filtering ❌ NOT STARTED

**Goal:** Filter results by time ("what did I say yesterday?").

**Current State:**
- `search_hybrid()` has no timestamp filtering capability

**Implementation Needed:**
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
    exclude_ids: Option<&[i64]>,
    // NEW: Add timestamp filter
    timestamp_range: Option<(i64, i64)>,  // <-- Add this
) -> Result<Vec<SearchResult>>
```

**Tasks:**
- [ ] Add `timestamp_range: Option<(i64, i64)>` parameter to `search_hybrid()`
- [ ] Implement temporal reference detection (pt-BR + en patterns)
- [ ] Add SQL WHERE clause for timestamp filtering
- [ ] Update `remember` tool to support time-based queries

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

#### Phase 4: Schema Preparation (1-2 days)

**Goal:** Prepare schema for multiple source types.

**Tasks:**
- [ ] Design `documents` table schema
- [ ] Create `document_embeddings` virtual table
- [ ] Update `SearchTarget` enum
- [ ] Add `source_type` filter to hybrid search

#### Phase 5: Document Ingestion

**Goal:** Ingest and index external documents.

**Tasks:**
- [ ] Research: PDF parsing crates (pdf-extract, lopdf)
- [ ] Implement: Document chunking with overlap
- [ ] Implement: Embedding generation for chunks
- [ ] Implement: `/ingest <path>` command

---

### SOUL.md Support (Personality System)

**Priority:** HIGH  
**Status:** Research Needed

**Goal:** Support for `SOUL.md` file to define LLM personality.

**Tasks:**
- [ ] Research: SOUL.md format in other agents
- [ ] Research: Best injection point in system prompt
- [ ] Design: Config option for custom SOUL.md path
- [ ] Implement: SOUL.md file loading
- [ ] Implement: Inject into system prompt

---

### Smart Model List (Installed Only)

**Priority:** MEDIUM  
**Status:** Proposed

**Goal:** Show only installed/available models.

**Tasks:**
- [ ] Implement: `is_model_installed(model_id)` function
- [ ] Modify: `/list` command to filter installed models
- [ ] Modify: Model switch to validate before loading
- [ ] Modify: Chat startup with fallback chain

---

## Medium Priority

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

### System Tools - run_command

**Priority:** Medium  
**Status:** Blocked by security concerns

**Tasks:**
- [ ] Research: Secure command execution
- [ ] Design: Whitelist configuration
- [ ] Implement: Security constraints

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

## Low Priority

### OCR Model Customization

**Priority:** Low  
**Status:** Resolved (Ollama v0.17.6)

GLM-OCR fix available in Ollama v0.17.6+.

### Plugin System

**Priority:** Low  
**Status:** Not started

User-defined tools via dynamic loading or compilation.

---

### OpenAPI Compatibility (Direct API Access)

**Priority:** LOW  
**Status:** Research Needed

**Goal:** Support direct interaction with OpenAI-compatible APIs.

**Tasks:**
- [ ] Research: Required API endpoints
- [ ] Design: Provider trait/interface
- [ ] Implement: OpenAI provider
- [ ] Implement: LM Studio provider

---

### TUI (Terminal User Interface)

**Priority:** Low  
**Status:** Research & Planning needed

**Goal:** Build a responsive TUI using Ratatui-rs.

**Tasks:**
- [ ] Research: Ratatui-rs best practices
- [ ] Research: Terminal resize handling patterns
- [ ] Design: UX wireframes for main views
- [ ] Prototype: Basic TUI skeleton

---

### Multilingual Injection Detection

**Priority:** Low  
**Status:** Not started

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