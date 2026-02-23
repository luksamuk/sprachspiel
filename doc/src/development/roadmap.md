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
**Status:** Planning needed

**Problem:** Add a dedicated vision module for image processing beyond OCR. Currently, vision-capable models can process images, but there's no structured way to handle image inputs.

**Research Needed:**
- Image encoding for Ollama models (base64)
- Supported image formats (PNG, JPEG, WebP, GIF)
- Multi-image support
- Model capability detection for vision

**Proposed Features:**
- `ask-ai vision <image>` - Describe/analyze images
- Image input in chat mode for vision models
- Integration with vision-capable cloud models (kimi-k2.5, qwen3.5)

**Tasks:**
- [ ] Research: Ollama vision API and image encoding
- [ ] Research: Model vision capability detection
- [ ] Design: Vision command interface
- [ ] Implement: Image encoding and processing
- [ ] Implement: Vision command
- [ ] Document: Vision capabilities

---

### Code Redundancy Refactoring

**Priority:** HIGH  
**Status:** Planning needed

**Problem:** Code has redundancy issues that were identified during bugfix work. The Ollama client configuration was triplicated across three places before being consolidated into `Settings::ollama_client()`. Similar patterns may exist elsewhere.

**Example:**
- Before fix: `Ollama::default()` was called separately in main.rs, summarize/processor.rs, and ocr/processor.rs
- After fix: Single `Settings::ollama_client()` function

**Approach:**

This refactoring requires upfront planning before implementation:

**Phase 1: Audit and Documentation**
- Survey codebase for redundant code patterns
- Document all instances of:
  - Duplicated configuration logic
  - Repeated initialization patterns
  - Similar error handling blocks
  - Copy-pasted code across modules
- Create refactoring proposal with priorities

**Phase 2: Prioritized Refactoring**
- Start with high-impact, low-risk changes
- Ensure tests pass after each change
- Update documentation as needed

**Potential Areas to Investigate:**
- Model configuration loading
- Coordinator building patterns
- System prompt construction
- Error handling patterns in tools
- File operations across processors

**Tasks:**
- [ ] Audit: Survey codebase for redundancy patterns
- [ ] Audit: Document all instances with locations
- [ ] Design: Create refactoring plan with priorities
- [ ] Implement: Refactor high-priority areas
- [ ] Test: Verify all functionality after refactoring
- [ ] Document: Update code documentation

---

## Medium Priority

### Plugin System

**Priority:** Medium  
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