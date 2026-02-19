# Roadmap

This document outlines planned features and the current state of Ask-AI.

## Current State

### Implemented Features

**Core CLI:**
- 5 subcommands (query, chat, translate, ocr, summarize)
- 7 built-in model presets + user-defined models via `models.toml`
- Markdown rendering via termimad
- Model capability detection (tools, vision, ocr)
- Pipe support for all commands
- Debug mode, Think mode, Code mode
- Configuration file support (`~/.config/ask-ai/config.toml`)
- Custom models file (`~/.config/ask-ai/models.toml`)
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

**Tools (26 total):**

| Category | Count | Feature Flag | Default |
|----------|-------|--------------|---------|
| Pokémon | 9 | `pokemon-tools` | ✅ Enabled |
| Weather | 3 | `weather-tools` | ✅ Enabled |
| File Operations | 5 | `file-tools` | ✅ Enabled |
| Calculator | 1 | `calc-tools` | ✅ Enabled |
| Web Search (Serper) | 2 | `serper-tools` | ✅ Enabled |
| Web Search (DDG) | 3 | `search-tools` | ❌ Disabled |
| Finance | 1 | `finance-tools` | ❌ Disabled |
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

---

## Known Issues

None currently.

---

## High Priority

### Termux Builds

**Priority:** HIGH  
**Status:** Research complete, ready to implement

**Problem:** Users want to run ask-ai on Android devices via Termux for daily use.

**Research Findings:**

**Target Architecture:**
- **Primary:** `aarch64-linux-android` (arm64-v8a) - for modern Android devices
- **Secondary:** `armv7-linux-androideabi` - for older Android devices (if needed)

**Cross-Compilation Tools:**

1. **`cross` (Recommended)** - Zero-setup cross compilation
   - Uses Docker/Podman containers with pre-built toolchains
   - Most reliable option for Rust projects
   - Command: `cross build --target aarch64-linux-android --release`
   - Repository: https://github.com/cross-rs/cross

2. **`cargo-ndk`** - Direct Android NDK integration
   - Requires Android NDK installation
   - Command: `cargo ndk -t arm64-v8a -o ./dist build --release`
   - Repository: https://github.com/bbqsrc/cargo-ndk

**Dependency Compatibility Analysis:**

| Dependency | Status | Notes |
|------------|--------|-------|
| `reqwest` | ⚠️ Needs attention | TLS via `ring` crate may need special handling in cross-compilation |
| `tokio` | ✅ OK | Supports Android natively |
| `serde/clap` | ✅ OK | Platform-independent |
| `ollama-rs` | ⚠️ Needs testing | HTTP client compatibility must be verified |
| `termimad` | ⚠️ Needs testing | Terminal features on Android |
| `futures` | ✅ OK | Async runtime compatible |

**Known Limitations:**

1. **Ollama Server Dependency:** ask-ai is a client; the Ollama server must run elsewhere (desktop or network-accessible). This is actually an advantage for Termux - no heavy server to run on Android.

2. **TLS/OpenSSL:** The `ring` crate (used by `rustls`) can have issues with cross-compilation. May need to:
   - Use `native-tls` feature instead of `rustls` for `reqwest`
   - Or configure `cross` with proper OpenSSL headers

3. **DNS Resolution:** Some networking crates may have issues on Android. Testing required on actual device.

4. **File Paths:** Termux uses `/data/data/com.termux/files/home/` as home directory. Configuration paths should work, but need verification.

**Recommended Implementation:**

**Phase 1: Local Testing with `cross`**
```bash
# Install cross
$ cargo install cross --git https://github.com/cross-rs/cross

# Add Android target
$ rustup target add aarch64-linux-android

# Build (requires Docker or Podman)
$ cross build --target aarch64-linux-android --release

# Build with features
$ cross build --target aarch64-linux-android --release --features "weather-tools,file-tools,calc-tools,system-tools"
```

**Phase 2: GitHub Actions CI**
```yaml
# .github/workflows/termux-build.yml
# Use cross in GitHub Actions to build Android binaries automatically
# Publish to GitHub Releases on tags
```

**Phase 3: Distribution**
- Pre-built binaries for `aarch64-linux-android` in GitHub Releases
- Installation script for Termux users
- Optional: Termux package submission

**Build Configuration:**

Create `Cross.toml` for custom configuration:
```toml
[target.aarch64-linux-android]
# Optional: specify custom image if needed
# image = "ghcr.io/cross-rs/aarch64-linux-android:main"

# Add environment variables if needed for TLS
[target.aarch64-linux-android.env]
# Example: SSL_CERT_FILE if custom certs needed
```

**Installation for Termux Users:**

```bash
# In Termux terminal:
$ pkg install wget
$ wget https://github.com/luksamuk/ask-ai-rs/releases/download/vX.Y.Z/ask-ai-aarch64-linux-android
$ chmod +x ask-ai-aarch64-linux-android
$ mv ask-ai-aarch64-linux-android $PREFIX/bin/ask-ai

# Verify
$ ask-ai --version
```

**Tasks:**
- [x] Research: Cross-compilation setup for Android (DONE)
- [x] Research: Compare `cross` vs `cargo-ndk` (DONE - `cross` recommended)
- [x] Research: Verify dependency compatibility (DONE - see table above)
- [x] Implement: Create `Cross.toml` configuration (DONE)
- [x] Implement: Add Makefile targets for Termux builds (DONE)
- [x] Implement: Use rustls instead of OpenSSL for cross-compilation (DONE)
- [x] Implement: Fix ollama_host URL handling for Termux (DONE)
- [x] Test: Build locally with `cross` (DONE)
- [x] Test: Run binary on actual Termux device (DONE)
- [x] Document: Installation instructions for Termux (DONE)

---

### Custom Model Support

**Priority:** HIGH  
**Status:** Research needed

**Problem:** Users want to use local models that aren't pre-configured in Ask-AI. Currently, only predefined model presets are supported.

**Questions to explore:**
1. How to detect model capabilities from Ollama API?
2. What's the minimum config users need for a new model?
3. How to handle models without tool support?
4. How to handle thinking models vs non-thinking?

**Proposed config:**
```toml
[model.custom."my-model"]
ollama_name = "my-finetune:latest"
context_window = 32768
tools = true
vision = false
temperature = 0.7
```

**Tasks:**
- [ ] Research: Ollama API model metadata endpoints
- [ ] Research: Test capability detection with various models
- [ ] Design: Config schema for custom models
- [ ] Implement: Custom model registration
- [ ] Implement: Capability detection/inference
- [ ] Document: How to add custom models

---



## Medium Priority

### Shell Completions

**Priority:** Medium  
**Status:** Not started

Generate shell completions for bash, zsh, fish:

```bash
ask-ai --generate-completion bash
```

**Tasks:**
- [ ] Implement: Completion generation using clap
- [ ] Document: How to enable completions

---

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

### System Tools

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

### Code Redundancy Refactoring

**Priority:** Medium  
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

## Testing

### Test Coverage
- [ ] Unit tests for all commands
- [ ] Integration tests with mock Ollama
- [ ] Tool testing framework
- [ ] OCR/Translation testing

### CI/CD
- [ ] GitHub Actions for testing
- [ ] Release automation
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