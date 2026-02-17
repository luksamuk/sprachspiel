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

1. **DuckDuckGo Web Search Blocked**
   - Status: CAPTCHA blocking automated requests
   - Impact: Web search tools non-functional
   - Priority: High
   - Solution: Alternative search provider needed

2. **GPT-OSS Tool Calling**
   - Status: Encoding errors with some tool calls
   - Impact: Tool calls fail with `invalid character '\u003c'`
   - Priority: Medium
   - Workaround: Use mistral-small or pepe models

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

#### System Tools

**Priority:** Medium

- `run_command` - Execute commands (configurable whitelist)
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
