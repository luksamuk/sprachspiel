# Available Tools

Ask-AI provides tools that enhance queries with real-time data from external sources. Tools are automatically enabled for capable models.

## Tool Overview

| Category | Count | Source | Status | Default |
|----------|-------|--------|--------|---------|
| Pokémon | 8 | PokéAPI | ✅ Working | ❌ Disabled* |
| Weather | 3 | Open-Meteo | ✅ Working | ✅ Enabled |
| Web Search | 3 | DuckDuckGo | ⚠️ Currently blocked | ✅ Enabled |
| File Operations | 5 | Local filesystem | ✅ Working | ✅ Enabled |

\* **Pokémon tools are disabled by default** to avoid polluting the context window with specialized tool descriptions when not needed. See [Compilation Features](#compilation-features) to enable them.

## Compilation Features

Tools are organized into feature flags that can be enabled or disabled at compile time. This allows you to build a leaner binary with only the tools you need.

### Default Features

The default build includes:
- `weather-tools` - Weather lookup tools
- `web-search-tools` - Web search tools (note: currently blocked by CAPTCHA)
- `file-tools` - File system operations

### Available Features

| Feature | Description | Tools Included |
|---------|-------------|----------------|
| `pokemon-tools` | Pokémon data from PokéAPI | fetch_pokemon*, fetch_ability_details, fetch_type_effectiveness, fetch_move_details |
| `weather-tools` | Weather data from Open-Meteo | get_weather, get_current_weather, get_weather_forecast |
| `web-search-tools` | Web search via DuckDuckGo | web_search, web_search_news, web_instant_answer |
| `file-tools` | Local file operations | read_file, read_file_segment, count_lines, list_directory, search_files |
| `all-tools` | Enable all tool categories | All of the above |

### Why Pokémon Tools Are Disabled by Default

Pokémon tools require 8 specialized tool definitions that consume significant context window space. For general-purpose usage, these tools often provide no value and can:
- **Pollute the context window** with unnecessary tool descriptions
- **Distract the model** from more relevant tools
- **Increase token usage** without benefit

Only enable Pokémon tools if you specifically need Pokémon data queries.

### Building with Custom Features

**Default build (no Pokémon tools):**
```bash
cargo build --release
```

**Enable Pokémon tools:**
```bash
cargo build --release --features pokemon-tools
```

**Enable all tools:**
```bash
cargo build --release --features all-tools
```

**Minimal build (only file tools):**
```bash
cargo build --release --no-default-features --features file-tools
```

**Build without web search (currently broken):**
```bash
cargo build --release --no-default-features --features "weather-tools,file-tools"
```

### Runtime Filtering

Even when tools are compiled in, you can disable specific tools at runtime using the [blacklist configuration](./configuration.md#tool-configuration):

```toml
# ~/.config/ask-ai/config.toml
[tools]
blacklist = ["fetch_pokemon", "web_search"]
```

When a tool is blacklisted, it won't be registered with the coordinator AND won't appear in the system prompt's tool descriptions. The model won't even know the tool exists.

### Feature Matrix

| Feature | Default | Binary Size Impact | Context Impact |
|---------|---------|-------------------|----------------|
| pokemon-tools | No | ~50KB | High (8 tools) |
| weather-tools | Yes | ~30KB | Low (3 tools) |
| web-search-tools | Yes | ~40KB | Medium (3 tools) |
| file-tools | Yes | ~35KB | Low (3 tools) |

## Pokémon Tools (8)

Powered by [PokéAPI](https://pokeapi.co/).

### fetch_pokemon

Get comprehensive Pokémon data including stats, abilities, moves, and evolution chain.

```
Function: fetch_pokemon
Args: name (string)
Example: fetch_pokemon(name: "pikachu")
```

### fetch_pokemon_basic

Get basic Pokémon information (types, height, weight, abilities).

```
Function: fetch_pokemon_basic
Args: name (string)
Example: fetch_pokemon_basic(name: "charizard")
```

### fetch_pokemon_stats

Get base stats (HP, Attack, Defense, etc.).

```
Function: fetch_pokemon_stats
Args: name (string)
Example: fetch_pokemon_stats(name: "mewtwo")
```

### fetch_pokemon_moves

Get learnable moves with optional limit.

```
Function: fetch_pokemon_moves
Args: name (string), limit (optional integer)
Example: fetch_pokemon_moves(name: "pikachu", limit: 10)
```

### fetch_pokemon_evolution

Get evolution chain information.

```
Function: fetch_pokemon_evolution
Args: name (string)
Example: fetch_pokemon_evolution(name: "eevee")
```

### fetch_ability_details

Get ability descriptions and which Pokémon have it.

```
Function: fetch_ability_details
Args: ability (string)
Example: fetch_ability_details(ability: "lightning-rod")
```

### fetch_type_effectiveness

Get type weaknesses, resistances, and immunities.

```
Function: fetch_type_effectiveness
Args: type_name (string)
Example: fetch_type_effectiveness(type_name: "electric")
```

### fetch_move_details

Get move information (power, accuracy, type, effect).

```
Function: fetch_move_details
Args: move (string)
Example: fetch_move_details(move: "thunderbolt")
```

## Weather Tools (3)

Powered by [Open-Meteo](https://open-meteo.com/) (free, no API key required).

### get_weather

Get current weather conditions and 3-day forecast.

```
Function: get_weather
Args: city (string), country (optional string)
Example: get_weather(city: "Tokyo", country: "Japan")
```

### get_current_weather

Get current conditions only (simpler response).

```
Function: get_current_weather
Args: city (string), country (optional string)
Example: get_current_weather(city: "London")
```

### get_weather_forecast

Get extended 7-day forecast.

```
Function: get_weather_forecast
Args: location (string), days (string, optional)
Example: get_weather_forecast(location: "Paris", days: "7")
```

**Note:** `days` accepts strings like "3", "5", "7".

## Web Search Tools (3)

⚠️ **Currently blocked by DuckDuckGo CAPTCHA**. Alternative needed.

Powered by DuckDuckGo Lite.

### web_search

General web search with results.

```
Function: web_search
Args: query (string), max_results (optional integer)
Example: web_search(query: "Rust programming language", max_results: 5)
```

### web_search_news

News-specific search.

```
Function: web_search_news
Args: query (string), max_results (optional integer)
Example: web_search_news(query: "technology", max_results: 5)
```

### web_instant_answer

Quick facts and definitions.

```
Function: web_instant_answer
Args: query (string)
Example: web_instant_answer(query: "What is photosynthesis?")
```

## File Operation Tools (5)

Perform local filesystem operations. **Sandboxed by default** to current working directory for security.

**Important:** For large files, use `count_lines` first to check size, then `read_file_segment` to read only what you need. Avoid polluting context with entire large files.

### read_file

Read contents of a file. Files larger than 1MB are rejected.

```
Function: read_file
Args: path (string), max_lines (string, optional), sandbox (string, optional)
Example: read_file(path: "README.md", max_lines: "50")
```

**Note:** All arguments are strings for robustness. `max_lines` accepts numbers like "50" or "100". `sandbox` accepts "true", "false", "1", "0".

**Features:**
- Limit output to N lines with `max_lines`
- Sandbox restricts access to current directory
- Supports relative and absolute paths
- Auto-resolves symlinks

### read_file_segment

Read a specific segment of a file (useful for large files).

```
Function: read_file_segment
Args: path (string), start_line (string), num_lines (string), sandbox (string, optional)
Example: read_file_segment(path: "src/main.rs", start_line: "100", num_lines: "50")
```

**Features:**
- Line numbers are 1-based
- Output includes line numbers for easy reference
- Useful for reading specific functions or sections without loading entire large files
- Helps keep context window small

**Output format:**
```
Lines 100-150 of 500:
----------------------------------------
   100 | fn my_function() {
   101 |     println!("Hello");
```

### count_lines

Count lines in a file. **Use this before reading large files** to avoid polluting context.

```
Function: count_lines
Args: path (string), sandbox (string, optional)
Example: count_lines(path: "src/main.rs")
```

**Output includes suggestion for large files:**
```
File: src/main.rs
Lines: 1500
Bytes: 45000

Tip: This file has 1500 lines. Use read_file_segment(path, start_line, num_lines) to read specific sections and avoid polluting the context window.
```

### list_directory

List files and directories. Shows file types and sizes.

```
Function: list_directory
Args: path (string), recursive (string, optional), sandbox (string, optional)
Example: list_directory(path: "src", recursive: "true")
```

**Note:** `recursive` accepts "true", "false", "1", "0".

**Features:**
- Non-recursive by default (current level only)
- Recursive mode with `recursive: true` (max depth 10)
- Shows file types: [file], [dir], [symlink]
- Displays file sizes for files

### search_files

Search file contents with regex pattern.

```
Function: search_files
Args: pattern (string), path (string), file_pattern (string, optional), sandbox (string, optional)
Example: search_files(pattern: "TODO|FIXME", path: "src", file_pattern: "*.rs")
```

**Note:** `sandbox` accepts "true", "false", "1", "0".

**Features:**
- Regex pattern matching (full Rust regex syntax)
- File pattern filtering with glob syntax (`*.rs`, `*.txt`)
- Returns matching lines with file path and line number
- Limited to 100 results and 1MB files
- Searches files within 5 directory levels

### File Tool Security

File tools are sandboxed by default:

```toml
# ~/.config/ask-ai/config.toml
[tools]
file_sandbox = true  # Only allow access to CWD and subdirectories
```

**Sandbox behavior:**
- ✅ Allowed: Files in current directory and subdirectories
- ❌ Blocked: Files outside working directory
- ❌ Blocked: System directories (`/etc`, `/usr`, etc.)
- ❌ Blocked: Symlinks pointing outside sandbox

**Disabling sandbox** (not recommended):

```toml
[tools]
file_sandbox = false
```

**Warning:** Disabling the sandbox allows the AI to access any file your user account can read. Only disable if you fully trust the AI and understand the security implications.

## Using Tools

### Automatic Tool Detection

Tools are automatically enabled for capable models:

```bash
# Tools auto-enabled for mistral-small
ask-ai -m mistral-small "Tell me about Pikachu"

# Tools auto-enabled for gpt-oss
ask-ai -m gpt-oss "What's the weather in Tokyo?"
```

### Force Enable Tools

Force tools on any model:

```bash
ask-ai --tools "Tell me about Pikachu"
```

### Tool User Prompt

Use enhanced prompt for better tool selection:

```bash
ask-ai -p tool_user "What's the weather?"
```

### Disable Specific Tools

Blacklist tools via configuration:

```toml
# ~/.config/ask-ai/config.toml
[tools]
blacklist = ["web_search", "web_instant_answer"]
```

## Tool Examples

### Pokémon Queries

```bash
# Comprehensive data
ask-ai "Tell me everything about Charizard"

# Specific information
ask-ai "What are Pikachu's stats?"
ask-ai "Show me Eevee's evolution chain"
ask-ai "What type is super effective against Water?"

# Compare Pokémon
ask-ai "Compare Blastoise and Charizard stats"

# Move information
ask-ai "Tell me about Thunderbolt"
ask-ai "What moves can Pikachu learn?"
```

### Weather Queries

```bash
# Current weather
ask-ai "What's the weather in Tokyo?"

# Forecast
ask-ai "Weather forecast for Paris"

# Specific queries
ask-ai "Is it raining in London?"
ask-ai "What's the temperature in New York?"

# With country
ask-ai "Weather in Sydney, Australia"
```

### Web Search (Currently Blocked)

```bash
# Note: Web search is currently blocked by DuckDuckGo CAPTCHA

# General search
ask-ai "Search for Rust async patterns"

# News
ask-ai "Latest technology news"

# Quick facts
ask-ai "What is quantum computing?"
```

### File Operations

```bash
# Read a file
ask-ai "Read the README.md file"

# List directory contents
ask-ai "Show me the files in the src directory"

# Search for code patterns
ask-ai "Find all TODO comments in the codebase"

# Analyze project structure
ask-ai "List all Rust files recursively and tell me what each module does"

# Search and analyze
ask-ai "Search for all functions named 'handle_' in the src directory"

# Multi-file analysis
ask-ai "Read Cargo.toml and tell me what dependencies this project has"
```

**Complex file operations:**

```bash
# Count lines of code
ask-ai "List all .rs files recursively, then count total lines of code"

# Find largest files
ask-ai "List the src directory recursively and identify the 5 largest files"

# Pattern analysis
ask-ai "Search for all 'async fn' declarations in src and summarize the async functions"
```

## Tool Selection

The model automatically selects appropriate tools based on your query:

```mermaid
graph TD
    A[User Query] --> B{File operation?}
    B -->|Yes> C[Use File tools]
    B -->|No> D{Contains Pokémon?}
    D -->|Yes> E[Use Pokémon tools]
    D -->|No> F{Contains Weather?}
    F -->|Yes> G[Use Weather tools]
    F -->|No> H{Needs Web Search?}
    H -->|Yes> I[Use Web Search tools]
    H -->|No> J[Answer directly]
```

## Known Issues

### DuckDuckGo Web Search Blocked

**Status**: ⚠️ Currently blocked

**Problem**: DuckDuckGo Lite endpoint blocks automated requests with CAPTCHA

**Error**: "Unfortunately, bots use DuckDuckGo too"

**Workaround**: None currently

**Solution**: Alternative search provider needed
- SerpAPI (paid)
- Bing API (paid)
- Searx (self-hosted)
- Local LLM web search

### GPT-OSS Tool Calling

**Status**: ⚠️ Under investigation

**Problem**: Models like `gpt-oss:120b` may fail with `invalid character '<'` error

**Cause**: Likely HTML entity encoding issue

**Workaround**: Use other models for tool calls:
- `mistral-small`
- `pepe`
- `lfm`

## Tool Capable Models

| Model | Tools | Notes |
|-------|-------|-------|
| mistral-small | ✅ | Best for tools |
| gpt-oss | ⚠️ | May have issues |
| qwen3-coder | ✅ | Code + tools |

## Debug Mode

See tool calls in debug mode:

```bash
ask-ai -d "Tell me about Pikachu"

# Output includes:
# - Tool calls with arguments (detailed format)
# - Tool results
# - Model configuration
```

### Tool Call Visibility

**Tool calls are always visible**, even without debug mode. This is intentional - users have the right to see what tools are being executed on their system.

**Without debug mode:**
```
🔧 Calling: read_file(path=README.md, max_lines=50)
```

**With debug mode (`-d`):**
```
═══════════════════════════════════════════════════════════════
🔧 TOOL CALL: read_file
───────────────────────────────────────────────────────────────
  path: README.md
  max_lines: 50
───────────────────────────────────────────────────────────────
📤 TOOL RESULT for read_file:
[content...]
═══════════════════════════════════════════════════════════════
```

## Best Practices

1. **Use capable models** - mistral-small works best
2. **Be specific** - "Pikachu stats" vs "Tell me about Pikachu"
3. **Use tool_user prompt** - For complex queries
4. **Check debug mode** - If tools aren't working
5. **Weather doesn't need API key** - Always available
6. **Keep file sandbox enabled** - For security
7. **Use relative paths** - When working with files
8. **Limit search scope** - Use file patterns to narrow searches
9. **Count before reading** - Use `count_lines` before reading large files
10. **Read in segments** - Use `read_file_segment` for large files

## Tool Error Handling

Tools are designed to handle errors gracefully and provide helpful feedback to the LLM:

### Error Philosophy

**Tools never crash the application.** Instead, they return informative error messages that help the LLM understand what went wrong and how to fix it.

Examples:

| Situation | Error Message |
|-----------|---------------|
| File not found | `Error: File not found: README.md. Please check if the file exists or try a different file name (e.g., README.org instead of README.md).` |
| Invalid line number | `Error: Invalid start_line 500. File has 100 lines. Line numbers start at 1.` |
| Search pattern error | `Error: Invalid regex pattern '[a-z'. Please check your regex syntax.` |
| API error | `Weather API error: 429. Please try again later.` |

### How the LLM Uses Errors

When a tool returns an error:

1. The error is returned as a string result (not an exception)
2. The LLM sees the error in the tool result
3. The LLM can try a different approach (e.g., try `README.org` if `README.md` not found)
4. The LLM can inform the user about the issue

### When Tools Fail

Only critical/unrecoverable errors cause failures:
- Network timeouts (after retries)
- System-level errors (out of memory)
- Invalid configuration

All other errors return helpful messages so the LLM can adapt.

## Security Considerations

### File Tool Security

File tools are sandboxed to prevent unauthorized access:

- ✅ Can read files in current project
- ❌ Cannot access `/etc/passwd`, `/home/otheruser`, etc.
- ❌ Cannot follow symlinks outside sandbox
- ❌ Cannot read files > 1MB (DoS protection)

### Tool Blacklisting

Disable potentially problematic tools:

```toml
# ~/.config/ask-ai/config.toml
[tools]
# Disable web search (currently broken)
blacklist = ["web_search", "web_search_news", "web_instant_answer"]
```

## Future Tools

Planned additions:

- **System Tools**: Execute commands (configurable whitelist)
- **Web Scraping**: Extract content from URLs
- **Database Tools**: Query local databases

## See Also

- [Configuration](./configuration.md) - Tool configuration and blacklisting
- [query](./commands/query.md) - Using tools with queries
- [Models](./models.md) - Tool-capable models
- [Prompts](./prompts.md) - Tool user prompt mode
