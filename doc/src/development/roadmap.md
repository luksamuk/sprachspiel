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
- [ ] Skill invocation commands (`/skill-name`)
- [ ] Skill composition (multiple skills active)
- [ ] Skill dependencies (skill A requires skill B)
- [ ] Project-level skill discovery
- [ ] Skill hot-reload during development

**Phase 3: Integration** (future)
- [ ] Integration with Document Import Tool
- [ ] Integration with OCR/Vision Tools
- [ ] User skill sharing (community repository?)
- [ ] Skill versioning

**Research Complete:** See [CLI Tools Research](./cli-tools-research.md) and [Skills System Design](./skills-system-design.md) for full details.

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

**Priority:** MEDIUM
**Status:** BLOCKED (requires Skills System Phase 1)

**Goal:** Import documents for semantic search with external tool pipelines.

**Dependencies:**
- Skills System (for PDF pipeline definition)
- CLI Tools Phase 2 (pdftotext, pdftoppm, tesseract integration)

**Planned Features:**
- TEXT/MD: Builtin support (import_text_file)
- PDF: External tools (pdftotext) + skills
- Scanned PDF: tesseract + pdftoppm pipeline
- Chunking with overlap (512 tokens, 64 overlap)
- `/import-doc`, `/list-docs`, `/remove-doc` commands

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