# query Command

The `query` command is the default mode of Ask-AI. It sends your question or prompt to an LLM and returns a formatted response.

## Synopsis

```bash
ask-ai [GLOBAL OPTIONS] query [QUERY]
ask-ai [GLOBAL OPTIONS] [QUERY]
ask-ai [GLOBAL OPTIONS] q [QUERY]
```

## Description

The query command is the most flexible way to interact with LLMs through Ask-AI. It supports:

- **Multiple models** - Switch between different LLMs
- **Tool integration** - Automatic use of Pokémon, Weather, and Web Search tools
- **Think mode** - Enable reasoning for complex questions
- **Code mode** - Optimize responses for code output
- **Context retrieval** - Access project history (v0.25.0+)
- **Markdown rendering** - Beautiful terminal output

## Project-Aware Retrieval (v0.25.0+)

Query mode can access your conversation history to provide context:

- Retrieves relevant messages from all sessions in the project
- Uses semantic search to find related discussions
- Enriches user questions with their assistant answers

This is **read-only** - query mode does not persist new messages.

## Arguments

| Argument | Description |
|----------|-------------|
| `QUERY` | The question or prompt. Reads from stdin if not provided. |

## Global Options

These options must be placed **before** the `query` subcommand:

|| `-v` | Verbose logging |
|| `-vv` | Trace logging |
|| `--help` | Show help |

## Prompt Modes

The `-p` flag selects different system prompts:

| Mode | Description |
|------|-------------|
| `default` | General purpose queries |
| `tool_user` | Enhanced for tool usage |
| `code` | Code-focused responses |
| `code_with_tools` | Code mode with web search |

## Examples

### Basic Usage

```bash
# Simple question
ask-ai "What is the capital of France?"

# Equivalent explicit forms
ask-ai query "What is the capital of France?"
ask-ai q "What is the capital of France?"
```

### Model Selection

```bash
# Use a different model
ask-ai -m qwen2.5-coder:7b query "Generate a Python function"

# Code-focused model
ask-ai -m qwen3-coder query "Write a Rust struct for users"

# Smaller model for quick answers
ask-ai -m nanbeige4.1:3b query "What is 2+2?"
```

### Think Mode

Think mode enables reasoning models to show their thought process:

```bash
# Enable thinking
ask-ai -t query "Explain step by step how to solve x^2 + 5x + 6 = 0"

# Combine with specific model
ask-ai -m lfm -t query "What are the ethical implications of AI?"
```

### Code Mode

Code mode optimizes responses for code output. Use the `-c` flag or `--code`:

```bash
# Basic code mode (uses [model.code] config)
ask-ai -c query "Write a Python function to sort a list"

# Code with tools (file operations)
ask-ai -c --tools query "Read my config.rs and suggest improvements"

# Explicit model selection
ask-ai -m qwen2.5-coder:7b -c query "Implement a thread pool in Rust"
```

#### Code Mode Configuration

Code mode uses settings from `[model.code]` in your config file:

```toml
# ~/.config/ask-ai/config.toml
[model.code]
model = "qwen2.5-coder:7b"  # Recommended: fast and accurate
thinking = false
tools = true  # Enable file operations for context-aware code
```

With tools enabled, the model can read your files before generating code:

```bash
# Model reads your project files first, then generates code
ask-ai -c "Add error handling to my API handlers"
```

#### Code Mode Examples

```bash
# Generate code only (minimal explanation)
ask-ai -c query "Rust function to parse JSON"

# With file context (if tools enabled in config)
ask-ai -c query "Convert this function to async"

# Debug with code mode
ask-ai -c query "Why does this code panic?"
```

### Logging

Logging shows what's happening under the hood:

```bash
# See model configuration
ask-ai -v query "Test query"
#
# Shows:
# - Model being used
# - Capabilities detected
# - Tools enabled
# - Temperature and options
# - Tool calls and results
```

### Pipe from Stdin

```bash
# Read from stdin
echo "What is this?" | ask-ai

# Process file content
cat code.rs | ask-ai "Review this code"

# Chain commands
cat error.log | ask-ai "Explain this error"
```

### Tool Usage

Tools are automatically enabled for capable models:

```bash
# Pokémon tool (auto-enabled)
ask-ai query "Tell me about Pikachu"

# Weather tool (auto-enabled)
ask-ai query "What's the weather in Tokyo?"

# Web search tool (auto-enabled)
ask-ai query "Latest news about Rust programming"

# Force tools on any model
ask-ai --tools query "Tell me about Charizard"
```

## Tool Integration

When tools are enabled, the model can access:

### Pokémon Tools (8 total)
- `fetch_pokemon` - Comprehensive Pokémon data
- `fetch_pokemon_basic` - Basic info
- `fetch_pokemon_stats` - Base stats
- `fetch_pokemon_moves` - Learnable moves
- `fetch_pokemon_evolution` - Evolution chain
- `fetch_ability_details` - Ability descriptions
- `fetch_type_effectiveness` - Type matchups
- `fetch_move_details` - Move information

### Weather Tools (3 total)
- `get_weather` - Current + 3-day forecast
- `get_current_weather` - Current conditions
- `get_weather_forecast` - 7-day forecast

### Web Search Tools (3 total)
- `web_search` - General web search
- `web_search_news` - News search
- `web_instant_answer` - Quick facts

## Output Formats

### Markdown (Default)

Responses are formatted with markdown:

```bash
ask-ai "Create a simple table"
# Output:
# | Column 1 | Column 2 |
# |----------|----------|
# | Data     | Data     |
```

### Plain Text

For piping to other programs:

```bash
ask-ai --plain query "List files" | wc -w
```

## Common Patterns
# Think + specific model
ask-ai -m glm-5 -t query "Complex reasoning task"
#
# Code + verbose
ask-ai -c -v query "Debug this function"
#
# Tools + plain
ask-ai --tools --plain query "Get weather" | grep temperature

### Error Handling

If a model doesn't support a feature:

```bash
# Warning if think mode not supported
ask-ai -m ministral-3:14b -t query "Question"
# Output: Warning: ministral-3:14b does not support think mode. Ignoring -t.
```

## AGENTS.md Context

If an `AGENTS.md` file exists in the current directory, it is automatically loaded and injected into the system prompt. This provides project-specific context to the model.

### How It Works

1. The tool looks for `AGENTS.md` in the current working directory
2. Content is sanitized against prompt injection attacks
3. Sanitized content is framed and injected into the system prompt
4. Use `--ignore-agents` to disable this behavior

### Security

Content is sanitized to remove:
- Instruction override patterns (e.g., "ignore all previous instructions")
- Fake system tags (e.g., `[SYSTEM]`, `<instruction>`)
- Executable code blocks (bash, python, javascript, etc.)
- Lines exceeding 1000 lines are truncated

### Example

```bash
# If AGENTS.md exists, context is automatically loaded
ask-ai "Explain the project structure"

# Disable AGENTS.md context
ask-ai --ignore-agents "General question"
```

## Best Practices

1. **Start simple** - Default model works for most queries
2. **Use think mode** - For complex reasoning or math problems
3. **Code mode** - When you want code without explanations
4. **Pipe when needed** - For processing or saving responses
5. **Debug for issues** - Use `-d` to troubleshoot problems

## See Also

- [translate](./translate.md) - Language translation
- [ocr](./ocr.md) - Image text extraction
- [summarize](./summarize.md) - Text summarization
- [Models](../models.md) - Available models
- [Tools](../tools.md) - Available tools
