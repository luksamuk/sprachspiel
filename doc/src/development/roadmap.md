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
- Tab completion for commands and models
- Mode indicators in prompt (`[t]`, `[T]`)
- Token metrics display
- Thinking output visible when enabled
- Error recovery for tool/network errors

**Tools (23 total):**

| Category | Count | Feature Flag | Default |
|----------|-------|--------------|---------|
| Pokémon | 9 | `pokemon-tools` | ✅ Enabled |
| Weather | 3 | `weather-tools` | ✅ Enabled |
| File Operations | 5 | `file-tools` | ✅ Enabled |
| Calculator | 1 | `calc-tools` | ✅ Enabled |
| Web Search (Serper) | 2 | `serper-tools` | ✅ Enabled |
| Web Search (DDG) | 3 | `search-tools` | ❌ Disabled |
| System | 2 | `system-tools` | ✅ Enabled |

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

### Vision Module for Image Processing

**Priority:** HIGH  
**Status:** Planning

**Problem:** Add a dedicated vision module for image description/analysis. Currently, we have OCR for text extraction, but no way to get general image understanding.

**Design Decisions:**
- **Separate from OCR** - OCR (glm-ocr) for text extraction, Vision (moondream) for image understanding
  - OCR: specialized for text/tables/formulas extraction
  - Vision: general image description and analysis
  - Two commands, two purposes, two optimized models
- **Default model:** moondream:1.8b (1.7GB, lightweight, edge-optimized, "runs anywhere")
- **Focus:** Standalone subcommand first, chat integration later

**Vision Models Comparison (available in Ollama, ≤32B params):**

| Model | Size | Context | Params | Multi-Img | Notes |
|-------|------|---------|--------|-----------|-------|
| moondream:1.8b | 1.7 GB | 2K | 1.8B | ❓ | Lightweight, edge devices (DEFAULT) |
| llava:7b | 4.7 GB | 32K | 7B | ⚠️ | Popular, good OCR, higher resolution |
| llava:13b | 8.0 GB | 4K | 13B | ⚠️ | More accurate than 7b |
| minicpm-v:8b | 5.5 GB | 32K | 8B | ✅ | Multi-image leader, strong OCR, beats GPT-4V |
| llama3.2-vision:11b | 7.8 GB | 128K | 11B | ❓ | Meta official, large context |
| qwen2.5vl:7b | 6.0 GB | 125K | 7B | ❓ | Qwen vision, large context |
| qwen2.5vl:32b | 21 GB | 125K | 32B | ❓ | Max size for 32GB RAM |

**Proposed Structure:**
```
src/vision/
├── mod.rs           # Exports
├── cli.rs           # VisionArgs (clap)
├── processor.rs     # VisionProcessor (similar to OcrProcessor)
└── error.rs         # VisionError
```

**Proposed CLI:**
```bash
ask vision image.png                           # Default description
ask vision --detailed image.png                # Detailed analysis
ask vision --ocr image.png                     # Light OCR
ask vision image.png "What objects are here?"  # Custom prompt
ask vision img1.png img2.png                   # Multiple images
ask vision --json image.png                    # JSON output
ask vision -m llava image.png                  # Specific model
```

**Multi-Image Support:**
- Ollama API natively supports `images` array
- Best model for multi-image: minicpm-v:8b (SOTA on benchmarks)
- Use cases: comparison, before/after, counting across images
- No automatic fallback: if model performs poorly, user switches model manually

**Research Completed:**
- [x] Available vision models in Ollama (moondream, llava, minicpm-v, etc.)
- [x] Model comparison and default selection (moondream:1.8b)
- [x] Multi-image support: Ollama API supports `images` array; minicpm-v:8b is the leader (SOTA on multi-image benchmarks)

**Research Needed:**
- [ ] API differences: `/api/generate` vs `/api/chat` for vision
- [ ] Image size limitations

**Modes and Use Cases:**

| Mode | Flag | Default Prompt |
|------|------|----------------|
| default | (none) | "Describe this image." |
| detailed | `--detailed` | "Describe this image in detail, including composition, colors, subjects, and any notable elements." |
| ocr | `--ocr` | "Extract and transcribe all text visible in this image." |

**Use Cases Covered:**
- General description (default)
- Detailed analysis (--detailed)
- Light OCR (--ocr)
- Custom prompts (user-provided prompt overrides modes)
- **Comparison/inventory** (multi-image with custom prompt)
- **Code/UI analysis** (via custom prompt)

**Tasks:**
- [ ] Design: Default prompt for description (brainstorming in progress)
- [ ] Design: Vision command interface
- [ ] Implement: Vision module (cli, processor, error)
- [ ] Implement: Vision command
- [ ] Test: Compare vision models (configs added to models.toml)
- [ ] Document: Vision capabilities

---

## Medium Priority

### Automatic Conversation Compaction

**Priority:** Medium  
**Status:** Research needed

**Problem:** Manual `/compact` is sufficient, but automatic compaction based on token count would be more convenient.

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

### Chat Module Integration

**Priority:** Medium  
**Status:** Planning needed

**Problem:** Allow calling other modules (OCR, Vision, Translate, Summarize) from within chat mode. Currently, users must exit chat to use these features.

**Research Needed:**
- How to expose module functionality as chat commands
- Model switching for specialized tasks (e.g., switch to glm-ocr for OCR, then back)
- State management during module calls
- User experience design (commands vs tools vs natural language)
- **Contextualization:** When integrating with chat, think about how to contextualize module outputs. For example, after running OCR, the chat model should understand the extracted text as part of the conversation context, not just raw output. Same for vision descriptions - the model should be able to reason about what it "saw".

**Proposed Features:**
- `/ocr <image>` - Run OCR from within chat
- `/translate <lang> <text>` - Translate text in chat
- `/summarize` - Summarize conversation or pasted text
- `/vision <image>` - Analyze image (when vision module ready)

**Tasks:**
- [ ] Research: Best approach for module integration (commands vs tools)
- [ ] Design: Command interface and UX
- [ ] Design: Model switching strategy
- [ ] Implement: OCR command in chat
- [ ] Implement: Translate command in chat
- [ ] Implement: Summarize command in chat
- [ ] Document: Chat module commands

---

### System Tools - run_command

**Priority:** Medium  
**Status:** Blocked by tool call reliability

- `run_command` - Execute commands with configurable whitelist
  - **Requires:** Robust error handling and validation
  - **Security:** Command whitelist, timeout, sandboxing

**Tasks:**
- [ ] Research: Secure command execution patterns
- [ ] Design: Whitelist configuration
- [ ] Implement: `run_command` with security constraints

---

### Automatic Conversation Compaction

**Priority:** Low  
**Status:** Research needed

**Problem:** Manual `/compact` is sufficient, but automatic compaction based on token count would be more convenient.

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

## Low Priority

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