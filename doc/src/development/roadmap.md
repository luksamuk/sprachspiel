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

### GLM-OCR Returns Empty Output

**Status:** Upstream bug (Ollama issue #14474)

GLM-OCR model returns empty markdown after Ollama v0.17.1. This is a bug in Ollama, not in ask-ai.

**Workaround:** Use `ask vision` for image analysis until fixed.

---

## High Priority

### Token Counting & Context Metrics

**Priority:** HIGH  
**Status:** Ready for implementation

**Rationale:** Foundation for all context management. Without token visibility, we can't measure or optimize.

**Problem:** No visibility into token usage per session. Users can't optimize context usage.

**Implementation:**
```rust
fn count_tokens(messages: &[Message], model: &str) -> usize {
    // Use tiktoken-rs or estimation (~0.75 words/token)
    // Include message overhead (~4 tokens/message)
    // Include tool definitions
}
```

**Tasks:**
- [ ] Implement: Token counting utility
- [ ] Add: Token metrics to chat sessions
- [ ] Create: `/context` command for session info
- [ ] Display: Tokens per message type

---

### To-Do List Tooling

**Priority:** HIGH  
**Status:** Ready for implementation

**Rationale:** State Management is the most impactful context reduction. Explicit task tracking eliminates the need to search through history.

**Problem:** LLMs waste context searching through conversation history to track progress on multi-step tasks. An explicit to-do list reduces this need.

**Proposed Features:**
- `create_list(name: String)` - Create a new task list
- `add_task(list: String, task: String)` - Add task to list
- `update_task(list: String, task_id: usize, status: String)` - Update status (pending/in_progress/done)
- `get_tasks(list: String)` - Retrieve current tasks (model can query)
- `clear_list(list: String)` - Clear completed tasks

**Session Types:**
- **Query mode:** Ephemeral, in-memory list for single-task tracking
- **Chat mode:** Persistent, stored with session for multi-step workflows

**Implementation Notes:**
- Store list state separately from chat history
- Include current list in system prompt context
- Model references list instead of scanning history

**Tasks:**
- [ ] Research: LLM task management patterns
- [ ] Design: Tool interface and state storage
- [ ] Implement: To-do list tools in `src/tools/todo.rs`
- [ ] Integrate: Include list state in system prompt
- [ ] Test: Multi-step task scenarios

---

### Context Management v2 - Embeddings Research

**Priority:** HIGH  
**Status:** Research in progress

**Rationale:** Required for semantic retrieval (Phase 4 of context management). Model research complete, remaining tasks pending.

**Goal:** Enable semantic retrieval of conversation history for intelligent context selection.

**Model Research:** ✅ Complete - See `context_v2_plan.md` for details

| Model | Size | Context | Languages | Use Case |
|-------|------|---------|-----------|----------|
| **nomic-embed-text-v2-moe** | 958MB | 512 | 100 | Multilingual (primary) |
| nomic-embed-text | 274MB | 2048 | English | Long English docs |

**Rust Integration:** ✅ Complete - `ollama-rs` provides native embedding support via `generate_embeddings`

**Remaining Research Tasks:**
- [ ] Evaluate SQLite vector extensions
  - [ ] sqlite-vec (recommended)
  - [ ] sqlite-vss (alternative)
- [ ] Design: Storage schema and query interface
- [ ] Design: Dimension selection (768 vs 256)
- [ ] Test: Embedding latency (Ollama API call)
- [ ] Test: Storage requirements per dimension
- [ ] Test: Query latency for similarity search

**Detailed Research:** `doc/src/development/context_v2_plan.md`

---

## Medium Priority

### Automatic Middle Compaction

**Priority:** Medium  
**Status:** Planning

**Goal:** Automatically compact middle messages when approaching context limit.

**Strategy:**
1. Preserve: System prompt + Working state + Recent messages (last 10)
2. Summarize: Middle messages (abstractive)
3. Trigger: At 80% of context window

**Tasks:**
- [ ] Implement: Sliding window foundation
- [ ] Implement: Middle summarization
- [ ] Add: Auto-compact trigger
- [ ] Configure: Threshold in config.toml

---

### Chat Module Integration

**Priority:** Medium  
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

**Tasks:**
- [ ] Design: Command interface
- [ ] Design: Model switching during commands
- [ ] Implement: `/ocr` command
- [ ] Implement: `/vision` command
- [ ] Implement: `/translate` command
- [ ] Document: Chat module commands

---

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

## Low Priority

### OCR Model Customization

**Priority:** Low  
**Status:** Blocked by Ollama bug #14474

See Known Issues.

**Tasks:**
- [ ] Wait: Ollama bug fix
- [ ] Research: Alternative OCR models
- [ ] Implement: `-m` flag for OCR

---

### Plugin System

**Priority:** Low  
**Status:** Not started

User-defined tools via dynamic loading or compilation.

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