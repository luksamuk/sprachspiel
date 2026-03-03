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
- `/undo` for removing last response
- `/search` (alias `/find`, `/f`) for semantic search
- `/context` (alias `/ctx`) for token metrics
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

### Semantic Retrieval Context Framing (CRITICAL)

**Status:** Identified, fix planned for v0.22.9

The LLM doesn't understand what `<retrieved_context>` represents. It sees retrieved messages but doesn't recognize them as conversation history. Smaller models (ministral) are especially affected.

**Symptoms:**
- After `/clear`, LLM says "no memory of previous conversations"
- After model switch, same issue
- Retrieved context is ignored

**Root Cause:** The `<retrieved_context>` tag lacks explanation that these are past conversation messages.

**Fix:** See `doc/src/development/v0.22.9_plan.md`

---

## High Priority

### Context Framing for Semantic Retrieval (CRITICAL)

**Priority:** CRITICAL  
**Status:** Planned (v0.22.9)

**Problem:** After implementing forced retrieval (v0.22.7), semantic retrieval works correctly but the LLM doesn't understand the retrieved context.

The LLM receives:
```xml
<retrieved_context>
<message index="1">...</message>
</retrieved_context>
```

But doesn't know these are past conversation messages.

**Solution:**
1. Add framing text explaining `<retrieved_context>`
2. Add MEMORY section to system prompt (conditional)

**Decisions:**
- Accept ~80 tokens overhead for clarity
- MEMORY section conditional (only when retrieval happens)
- Keep 5 message limit (works for 32K+ context)
- Don't include conversation_id in messages
- Defer recall tool to future (low priority)

**Detailed Plan:** `doc/src/development/v0.22.9_plan.md`

**Tasks:**
- [ ] Add context explanation to retrieved messages
- [ ] Add MEMORY section to system prompt (conditional)
- [ ] Pass retrieval flag to prompt builder
- [ ] Test with glm-5:cloud and ministral
- [ ] Verify retrieval after `/clear`
- [ ] Verify retrieval after model switch

---

### Token Counting & Context Metrics ✅

**Priority:** HIGH  
**Status:** Completed (will be released in v0.19.0)

**Rationale:** Foundation for all context management. Without token visibility, we can't measure or optimize.

**Problem:** No visibility into token usage per session. Users can't optimize context usage.

**Implementation:**
```rust
fn count_messages_tokens(messages: &[ChatMessage]) -> usize {
    // Word-based estimation: ~0.75 words/token
    // Message overhead: ~4 tokens/message
    // System prompt + tools + history
}
```

**Tasks:**
- [x] Implement: Token counting utility (`src/tokens.rs`)
- [x] Add: Token metrics to chat sessions (`ContextMetrics` struct)
- [x] Create: `/context` command for session info
- [x] Display: Tokens per message type (system, tools, history)
- [x] Document: `/context` command in doc/src/commands/context.md

---

### To-Do List Tooling ✅

**Priority:** HIGH  
**Status:** Completed (will be released in v0.19.0)

**Rationale:** State Management is the most impactful context reduction. Explicit task tracking eliminates the need to search through history.

**Problem:** LLMs waste context searching through conversation history to track progress on multi-step tasks. An explicit to-do list reduces this need.

**Implemented Features:**
- `todo_add(description)` - Add a new task to the list
- `todo_update(task_id, status)` - Update status (pending/in_progress/done)
- `todo_list()` - List all tasks with current status
- `todo_clear_done()` - Remove completed tasks
- `todo_clear_all()` - Clear all tasks

**Session Types:**
- **Query mode:** Ephemeral, in-memory list via global state
- **Chat mode:** Persistent, stored with session in `todos` field

**Implementation:**
- `src/chat/todo_state.rs` - TodoState, Task, TaskStatus structs
- `src/tools/todo.rs` - 5 todo tools
- `src/prompts/tools.rs` - TODO TOOLS section in system prompt
- Enabled via `todo-tools` feature flag (default: enabled)

**Tasks:**
- [x] Research: LLM task management patterns
- [x] Design: Tool interface and state storage
- [x] Implement: To-do list tools in `src/tools/todo.rs`
- [x] Integrate: Include state in system prompt
- [x] Test: Unit tests for todo_state

---

### Context Management v2 - Semantic Retrieval ✅

**Priority:** HIGH  
**Status:** Completed (released in v0.20.0)

**Goal:** Enable semantic retrieval of conversation history for intelligent context selection.

**Architecture Decisions:**

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Vector Extension | **sqlite-vec** | sqlite-vss archived, sqlite-vec active |
| Dimensions | **256 (Matryoshka)** | 3x storage savings, ~2-3% quality loss |
| Retrieval | **Hybrid (BM25 + Semantic)** | RRF fusion, weights: 0.4 keyword / 0.6 semantic |
| Threshold | **0.7 cosine similarity** | Balance precision/recall |

**Model:** nomic-embed-text-v2-moe (958MB, 768 → 256d, multilingual)

**Storage Estimate:** ~20-30 MB for 10,000 messages

**Implemented:**
- [x] Create `src/db/` module with sqlite-vec integration
- [x] Create `src/embeddings/` module with Ollama API
- [x] Create `src/retrieval/` module with hybrid search (RRF)
- [x] Add `/search <query>` command
- [x] FTS5 query sanitization for SQL injection protection
- [x] Embedding dimension validation (768 → 256)

**Pending (v0.21.0):**
- [ ] Integrate with ChatSession (auto-save messages + embeddings)
- [ ] Add `/migrate` command (JSON → SQLite)
- [ ] Add `/reindex` command (rebuild embeddings)
- [ ] Context overflow handling
- [ ] Auto-retrieval (M recent + N relevant)

**Dependencies:**
```toml
rusqlite = { version = "0.32", features = ["bundled"] }
sqlite-vec = "0.1"
zerocopy = "0.8"
```

**Detailed Plan:** `doc/src/development/context_v2_plan.md`

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

### Recall Tool for Conversation History

**Priority:** Low  
**Status:** Deferred (future research)

**Concept:** Allow LLM to explicitly recall topics from conversation history via a tool call.

**Tool Interface:**
```rust
/// Search conversation history for a specific topic
/// Returns relevant messages with context
recall_topic(query: String, limit: Option<usize>) -> Result<String>
```

**Benefits:**
- LLM has explicit control over what to recall
- Can search specific topics, not just rely on automatic retrieval
- Can request more context when needed
- User can see what LLM is recalling

**Challenges:**
- Requires tool calling capability (not all models support)
- Need to define tool interface and parameters
- May conflict with automatic retrieval
- Tool calling overhead adds latency
- Research needed on optimal interaction pattern

**Relationship to Current Work:**
- Current automatic retrieval (v0.22.7+) works for most cases
- Recall tool would be LLM-initiated, not automatic
- Could complement or replace automatic retrieval
- Needs research on when LLM should call it

**Tasks:**
- [ ] Research: Tool calling patterns for memory
- [ ] Research: Auto vs. manual recall trade-offs
- [ ] Design: Tool interface and parameters
- [ ] Implement: `recall_topic` tool
- [ ] Evaluate: User experience comparison

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