# Roadmap

This document outlines planned features and the current state of Ask-AI.

## Current State

✅ **Implemented:**

- Core CLI with 4 subcommands (query, translate, ocr, summarize)
- 14 tools (8 Pokémon, 3 Weather, 3 Web Search)
- 14+ model presets
- Markdown rendering via termimad
- Model capability detection
- Tool integration with auto-detection
- Translation (50+ languages)
- OCR (text, tables, formulas, figures)
- Summarization (multiple styles)
- Pipe support for all commands
- Debug mode
- Think mode
- Code mode
- Man page
- mdBook documentation

## Known Issues

⚠️ **Active Issues:**

1. **DuckDuckGo Web Search Blocked** ⚠️
   - Status: CAPTCHA blocking automated requests
   - Impact: Web search tools non-functional
   - **Action Taken:** Web search tools are now blacklisted by default (Feb 2026)
   - **Note:** Tools can be enabled via config, but expect failures
   - Priority: High
   - Solution: Alternative search provider needed
   - **TODO:** When fixed, remove from default blacklist

2. **GPT-OSS Tool Calling**
   - Status: Encoding errors with some tool calls
   - Impact: Tool calls fail with `invalid character '\u003c'`
   - Priority: Medium
   - Workaround: Use mistral-small or pepe models

---

## Pre-Phase Research & Configuration Improvements

### 1. Enhanced Configuration System

**Priority:** High

**Problem:** Current configuration only supports a single default model. Users need per-subcommand model configuration.

**Research needed:**
- Design config structure for per-subcommand defaults
- Model settings per subcommand (query, summarize, ocr, translate)
- Per-subcommand thinking mode toggle
- Per-subcommand tool enable/disable

**Proposed config structure:**
```toml
[model]
# Global default (used by query subcommand)
default = "lfm"

[model.query]
default = "lfm"
thinking = true

[model.summarize]
default = "llama3.2"
thinking = false

[model.ocr]
# Fixed to glm-ocr - not configurable
# (ocr uses vision model with specific requirements)

[model.translate]
# Fixed to translategemma - not configurable
# (translate uses specialized translation model)

[model.code]
default = "devstral-small"
thinking = false
tools = false  # Code mode usually doesn't need tools

[ollama]
host = "127.0.0.1"
port = 11434
```

**Status:** ✅ COMPLETED (Feb 2026)

**Tasks:**
- [x] Design enhanced config schema
- [x] Update Settings struct to support per-subcommand configs
- [x] Update query handler to use respective config
- [x] Document new configuration options
- [x] Test backward compatibility
- [ ] Extend to summarize handler (Phase 1.1)

### 2. Code Mode Improvements

**Priority:** High

**Problem:** Developers need quick, code-focused responses for command-line tasks.

**Current state:**
- `-c` / `--code` flag exists
- Uses "code" or "code_with_tools" prompt
- No dedicated code-optimized model default

**Research needed:**
- Evaluate `devstral-small` as default code model
- Analyze current code prompt effectiveness
- Compare with `qwen3-coder` model
- Test code mode with/without tools

**Reference usage (from ask-ai.py):**
```bash
ask-ai -m devstral-small-2 "Como eu faço para mostrar a saída do comando 'ollama list', porém ignorando a primeira linha e pegando todos os dados da primeira coluna, e depois repassar cada um dos nomes de modelos recuperados para 'ollama show'?"
```

Expected output: Just the code, minimal explanation.

**Tasks:**
- [x] Research: List available models for code tasks
- [x] Research: Run `llm-checker recommend` for code model suggestions
- [x] Research: Test `devstral-small` for code queries
- [x] Research: Test `qwen3-coder` for code queries
- [x] Research: Test `deepseek-coder-v2` for code queries (WINNER!)
- [x] Design: Optimized code mode prompt
- [x] Implement: Code mode with per-subcommand config
- [x] Implement: Per-subcommand model selection for code
- [ ] Document: Code mode usage and best practices

### 3. Model Research for Code Tasks

**Models to evaluate:**

| Model | Purpose | Notes |
|-------|---------|-------|
| `devstral-small` | Code generation | Primary candidate for code mode default |
| `devstral-small-2` | Code generation | Newer version, user's favorite |
| `qwen3-coder` | Code generation | Alternative code model |

**Status:** ✅ COMPLETED (Feb 2026)

**Results:**
- **Winner:** deepseek-coder-v2:16b-32k (7.5x faster than devstral-small-2)
- **Runner-up:** devstral-small-2:24b-64k (more robust but 19.6s vs 2.6s)
- **Decision:** Use deepseek-coder-v2 as default for code mode

**Research tasks:**
- [x] Run `llm-checker recommend` for code model suggestions
- [x] Run `ollama list` to see installed models
- [x] Test each model with code queries
- [x] Measure response quality and speed
- [x] Determine best default for code mode
- [x] Add recommended model to modelfiles/ (✅ Done)

### 4. Implementation Order

```
1. Research Phase
   ├── List available models (ollama list, llm-checker recommend)
   ├── Test code models (devstral-small, qwen3-coder)
   ├── Evaluate current code prompt
   └── Design config schema

2. Configuration Phase
   ├── Implement enhanced Settings struct
   ├── Add per-subcommand model config
   ├── Update subcommand handlers
   └── Test configuration loading

3. Code Mode Phase
   ├── Implement code subcommand (or enhance flag)
   ├── Set devstral-small as default (if research confirms)
   ├── Optimize code prompt
   └── Test code-only workflows

4. Documentation Phase
   ├── Update configuration.md
   ├── Update tools.md (if code mode behavior changes)
   ├── Add code mode examples
   └── Update man page
```

---

## Phase 0: Code Mode Enhancement ✅ RESEARCH COMPLETE

**Status:** Research completed, ready for implementation  
**Priority:** High  
**Document:** [code_mode_research.md](./code_mode_research.md)

### Summary

Completed comprehensive research on code mode improvements, including model evaluation, system prompt optimization, and tool integration strategy.

### Key Findings

**Winner:** deepseek-coder-v2:16b-32k
- **7.5x faster** than devstral-small-2:24b-64k (2.6s vs 19.6s)
- MoE architecture (16B total, 2.4B active params)
- Superior code generation quality
- Recommended as default for code mode

### Implementation Tasks

**Phase 0.1: Configuration System Enhancement** ✅ COMPLETED
- [x] Extend Settings struct for per-subcommand model config
- [x] Add `[model.code]` section to config
- [x] Set deepseek-coder-v2:16b-32k as default for code mode
- [x] Support tool enable/disable per subcommand

**Phase 0.2: Model Integration** ✅ COMPLETED
- [x] Add deepseek-coder-v2:16b-32k to src/config.rs
- [x] Create optimized modelfile (✅ Already done)
- [x] Add to modelfiles/Makefile (✅ Already done)
- [x] Test with and without tools

**Phase 0.3: Tool-Enhanced Code Mode** ✅ COMPLETED
- [x] Implement SYSTEM_PROMPT_CODE_WITH_TOOLS
- [x] Dynamic prompt injection based on blacklist
- [x] File operations integration (list_directory, read_file, search_files)
- [x] Automatic fallback when tools are blacklisted

**Phase 0.4: Documentation** ✅ COMPLETED
- [x] Update doc/src/configuration.md
- [x] Update doc/src/models.md
- [x] Add code mode examples
- [x] Update man page

**Phase 1.1: Tool Call Robustness** ✅ COMPLETED
- [x] Tools return friendly error messages instead of crashing
- [x] File not found errors suggest alternatives (e.g., README.org vs README.md)
- [x] API errors return friendly messages with suggestions
- [x] Invalid regex patterns return helpful error messages

### Notes

- System prompts must respect blacklist - blacklisted tools don't appear in prompts
- Code mode with tools uses file operations for context-aware code generation
- Configuration schema designed for backward compatibility
- See full research document for detailed findings

---

## Planned Features

### Phase 1: Core Stability

#### Configuration File Support

**Priority:** High

Add support for user configuration files:

```toml
# ~/.config/ask-ai/config.toml
[model]
default = "lfm"

[tools]
blacklist = ["web_search"]

[output]
plain_default = false

[display]
skin = "dark"
```

#### Shell Completions

**Priority:** Medium

Generate shell completions for bash, zsh, fish:

```bash
ask-ai --generate-completion bash
```

### Phase 2: Enhanced Tools

#### File Operations Tools

**Priority:** High

Add tools for file system operations:

- `read_file` - Read file contents
- `list_directory` - List files
- `search_files` - Search file contents

#### Tool Call Robustness

**Priority:** High

**Problem:** LLMs frequently fail when calling tools, even with capable models. Errors include:
- Invalid JSON formatting in tool calls
- Missing required parameters
- Hallucinated parameter values
- Encoding errors (e.g., `invalid character '<'`)

**Research needed:**
1. Investigate `ollama-rs` crate for tool call handling mechanisms
2. Implement retry logic with detailed error feedback to LLM
3. Add validation layer for tool call parameters before execution
4. Improve error messages sent back to model for retry attempts

**Proposed Solution:**
```rust
// Enhanced tool execution with retry logic
async fn execute_tool_with_retry(
    coordinator: &mut Coordinator,
    tool_call: ToolCall,
    max_retries: u32,
) -> Result<String, ToolError> {
    // Validate parameters first
    if let Err(e) = validate_tool_call(&tool_call) {
        return Err(ToolError::InvalidParameters {
            error: e.to_string(),
            suggestion: format!("Please fix: {}", e),
        });
    }
    
    // Execute with error feedback
    match execute_tool(coordinator, tool_call).await {
        Ok(result) => Ok(result),
        Err(e) if max_retries > 0 => {
            // Send detailed error back to model with explicit retry request
            let error_feedback = format!(
                "Tool execution failed: {}\n\n\
                Please correct the tool call and try again. \
                Ensure all parameters are valid and properly formatted.",
                e
            );
            retry_with_feedback(coordinator, tool_call, error_feedback, max_retries - 1).await
        }
        Err(e) => Err(e.into()),
    }
}
```

**Success Criteria:**
- Reduce tool call failure rate from ~30% to <5%
- Provide actionable error feedback to model
- Support automatic retry with corrections
- Log detailed error patterns for analysis

**Note:** This is a prerequisite for `run_command` tool, which requires higher reliability due to security implications.

#### System Tools

**Priority:** Medium

**⚠️ BLOCKED:** Waiting for Tool Call Robustness implementation

- `run_command` - Execute commands (configurable whitelist)
  - **Requires:** Robust tool call validation and error handling
  - **Security:** Command whitelist, timeout, sandboxing
- `get_system_info` - System information

#### Web Scraping Tools

**Priority:** Medium

- `fetch_page` - Extract text from URLs
- `extract_articles` - Extract article content

### Phase 3: Advanced Features

#### Streaming Output

**Priority:** Low

Research streaming markdown rendering:

**Challenges:**
- Markdown context dependency
- Tables require full content
- Cross-line formatting

**Potential Solutions:**
1. Line-buffered rendering
2. Block-buffered rendering
3. Plain text streaming

#### Multi-Line Chat Mode

**Priority:** Low

Interactive chat mode:

```bash
ask-ai --chat
> First message
> Second message
> /quit
```

#### Configuration Management

**Priority:** Medium

- Model presets from config file
- Custom tool configurations
- User-defined shortcuts

### Phase 4: Tool Ecosystem

#### Compilation Features

**Priority:** Medium

Conditional tool compilation:

```toml
[features]
default = ["pokemon-tools"]
pokemon-tools = []
weather-tools = []
web-search-tools = []
all-tools = ["pokemon-tools", "weather-tools", "web-search-tools"]
```

Usage:

```bash
cargo build --no-default-features --features weather-tools
```

#### Plugin System

**Priority:** Low

Support for custom tools via plugins:

```rust
// Custom tool defined by user
#[ollama_rs::function]
pub async fn my_custom_tool(arg: String) -> Result<String> {
    // Implementation
}
```

## Future Tools

### Document Processing

- PDF text extraction
- Document format conversion
- Batch processing

### Data Analysis

- CSV/JSON analysis
- Statistical tools
- Visualization (ASCII charts)

### Code Tools

- Repository analysis
- Code quality checks
- Documentation generation

## Web Search Alternatives

### Option 1: SerpAPI (Paid)

Pros:
- Reliable
- Structured results

Cons:
- Requires API key
- Paid service

### Option 2: Bing API (Paid)

Pros:
- Microsoft integration
- Good results

Cons:
- Requires API key
- Rate limits

### Option 3: Searx (Self-Hosted)

Pros:
- Free
- Privacy-focused
- Self-hosted

Cons:
- Requires server
- Setup complexity

### Option 4: Local LLM Web Search

Pros:
- Privacy
- No API keys

Cons:
- Slower
- Less accurate

**Decision:** TBD - Community feedback needed

## Testing Improvements

### Test Coverage

- [ ] Unit tests for all commands
- [ ] Integration tests with mock Ollama
- [ ] Tool testing
- [ ] OCR testing
- [ ] Translation testing

### CI/CD

- [ ] GitHub Actions
- [ ] Automated testing
- [ ] Release automation
- [ ] Documentation deployment

## Documentation

### Current Status

✅ Complete:
- User documentation (mdBook)
- Man page
- Development documentation

### Planned

- [ ] API documentation (if we add library interface)
- [ ] Plugin development guide
- [ ] Video tutorials
- [ ] Example scripts

## Release Schedule

| Version | Focus | ETA |
|---------|-------|-----|
| 0.1.0 | Initial release | ✅ Done |
| 0.2.0 | Configuration files | Q1 2026 |
| 0.3.0 | New tools | Q2 2026 |
| 0.4.0 | Streaming | Q3 2026 |
| 1.0.0 | Stable release | Q4 2026 |

## Contributing

See something you want to work on?

1. Check [GitHub Issues](https://github.com/luksamuk/ask-ollama-rs/issues)
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
- GitHub Issues - Current issues and requests
