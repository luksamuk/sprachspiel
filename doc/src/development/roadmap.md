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

None currently.

---

## High Priority

### LLM Context History Redesign

**Priority:** HIGH  
**Status:** Research and brainstorming needed

**Problem:** Suspicion that LLMs are receiving excessive conversation history. Need to investigate and redesign how context is passed to the model.

**Potential Issues:**
- Too many messages in history consuming context window
- Redundant or low-value messages being included
- Model performance degrading with long histories
- Token usage inefficiency

**Research Needed:**
- Analyze current history management in chat sessions
- Benchmark model performance with varying history sizes
- Investigate message pruning strategies
- Consider relevance-based history selection

**Tasks:**
- [ ] Research: Audit current context/history passing mechanism
- [ ] Research: Benchmark history size vs model performance
- [ ] Brainstorm: History pruning strategies
- [ ] Design: New context management approach
- [ ] Implement: Optimized history handling

---

### To-Do List Tooling

**Priority:** HIGH  
**Status:** Research and brainstorming needed

**Problem:** LLMs need better task management during long sessions. A to-do list tool would help models track progress and maintain focus on multi-step tasks.

**Proposed Features:**
- Create a new list with multiple items
- Query current progress on the list
- Mark items as "done", "in_progress", or "pending"
- Clear the entire list
- Add items at specific positions
- Remove specific items

**Session Types:**
- **Query mode:** Ephemeral session, list is in-memory only, used for single-task progress tracking
- **Chat mode:** Persistent session, list survives across messages, used for multi-step workflows

**Integration Points:**
- Adjust chat and query prompts to encourage list usage
- Model should use lists to maintain progress visibility
- Tools should check/update list when performing tasks

**Research Needed:**
- Best practices for LLM task management
- Existing patterns in LLM agents/frameworks
- Prompt engineering for self-tracking

**Tasks:**
- [ ] Research: LLM task management patterns and best practices
- [ ] Research: Similar implementations in other LLM frameworks
- [ ] Brainstorm: Tool API design and prompt integration
- [ ] Design: Tool interface (create_list, query_list, update_list, etc.)
- [ ] Design: Prompt modifications for chat and query
- [ ] Implement: To-do list tools
- [ ] Test: Multi-step task scenarios

---

## Medium Priority

### Skills System - File-based Skill Loading

**Priority:** Medium  
**Status:** Not started

**Problem:** Allow users to define reusable "skills" via Markdown files with YAML frontmatter. Skills would be loaded dynamically to provide context, prompts, or behavior presets for the LLM.

**Concept:**
- Skills are Markdown files (`.md`) with YAML frontmatter metadata
- Stored in a dedicated directory (e.g., `~/.config/ask-ai/skills/` or project-local `.ask-ai/skills/`)
- Frontmatter contains: name, description, tags, triggers, model preferences, etc.
- Content (below frontmatter) contains the skill instructions/context

**Example Skill File:**
```markdown
---
name: code-review
description: Review code for quality and best practices
tags: [code, review, quality]
triggers: [review, critique]
model_preference: null
---

When reviewing code, focus on:
- Code readability and maintainability
- Potential bugs or edge cases
- Performance considerations
- Security vulnerabilities
...
```

**Proposed Features:**
- `--skill <name>` flag to load a skill for a query/chat session
- List available skills: `ask skills list`
- Show skill details: `ask skills show <name>`
- Skill discovery via tags/triggers
- Project-level skills (`.ask-ai/skills/`) override user-level skills
- Skills can specify recommended models

**Technical Approach:**
- Parse frontmatter with `gray_matter` crate
- Index skills for fast lookup (potential use of `tantivy` for full-text search)
- Cache parsed skills in memory during session

**Research Needed:**
- Similar implementations in other LLM CLI tools
- How Hermes Agent and Claude Code handle skills
- Best practices for skill file format and organization

**Tasks:**
- [ ] Research: Skill systems in other LLM agents (Hermes, Claude Code, etc.)
- [ ] Research: `gray_matter` crate for frontmatter parsing
- [ ] Research: `tantivy` for skill indexing/search
- [ ] Design: Skill file format and frontmatter schema
- [ ] Design: Skill discovery and loading mechanism
- [ ] Design: CLI interface for skills
- [ ] Implement: Skill file parser
- [ ] Implement: Skill registry and indexing
- [ ] Implement: `--skill` flag integration
- [ ] Document: Skill creation guide

---

### Plugin System

**Priority:** Low  
**Status:** Not started

Support for custom tools via plugins:

```rust
// User-defined tool
#[ollama_rs::function]
pub async fn my_custom_tool(arg: String) -> Result<String> {
    // Implementation
}
```

**Tasks:**
- [ ] Research: Dynamic loading vs compile-time plugins
- [ ] Design: Plugin interface
- [ ] Implement: Plugin loading mechanism
- [ ] Document: Plugin development guide

---

### Shell Completions

**Priority:** Low  
**Status:** Not started

Generate shell completions for bash, zsh, fish:

```bash
ask-ai --generate-completion bash
```

**Tasks:**
- [ ] Implement: Completion generation using clap
- [ ] Document: How to enable completions

---

### Automatic Conversation Compaction

**Priority:** Low  
**Status:** Blocked by LLM Context History Redesign

**Problem:** Manual `/compact` is sufficient, but automatic compaction based on token count would be more convenient.

**Blocked By:** This requires the LLM Context History Redesign to be completed first, as we need proper token counting and context management infrastructure.

**Research Needed:**
- Token counting for conversation history
- Optimal threshold for compaction
- Integration with model's context window size

**Tasks:**
- [ ] Research: Token counting methods (tiktoken, ollama API)
- [ ] Design: Threshold configuration (messages vs tokens)
- [ ] Implement: Compact before context exhausted
- [ ] Test: Verify context maintained after auto-compact

---

### Streaming Output

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