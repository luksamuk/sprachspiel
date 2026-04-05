# Prompt Modes

Prompt modes are system prompts that guide how the model responds. Different modes optimize the model for specific tasks.

## Available Modes

| Mode | Purpose | Best For |
|------|---------|----------|
| `default` | General queries | Everyday questions |
| `tool_user` | Tool usage | Queries with Pokémon/Weather/Web tools |
| `code` | Code output | Programming tasks |
| `code_with_tools` | Code + research | Programming with web search |
| `summarize` | Summarization | Document summaries |

## Personality System

All prompts include a personality layer from SOUL.md:

- **SOUL.md** - User-defined personality at `~/.config/ask-ai/SOUL.md`
- **PERSONALITY_DEFAULT** - Fallback when no SOUL.md exists
- **`--soulless` flag** - Skip personality entirely

See [SOUL.md](./soul.md) for details on creating custom personalities.

**Note:** Code modes (`code`, `code_with_tools`) and `summarize` mode do not use SOUL.md - they are purely operational.

## Mode Details

### default

The standard prompt for general purpose queries.

```bash
ask-ai -p default "Your question"
```

**Characteristics:**
- Sucinct responses
- Code when appropriate
- Markdown formatting
- No unnecessary conversation

**When to use:**
- General questions
- Quick answers
- When you don't need special behavior

### tool_user

Enhanced prompt for models with tool access.

```bash
ask-ai -p tool_user "What's the weather in Tokyo?"
```

**Characteristics:**
- Proactive tool usage guidance
- Clear tool selection rules:
  - Pokémon tools → ONLY for Pokémon content
  - Weather tools → ONLY for weather queries
  - Web Search → for general knowledge
- Structured thinking about tool selection

**When to use:**
- When using tool-capable models
- For complex queries that might need tools
- To encourage proactive web search

### code

Optimized for code generation with minimal explanations.

```bash
ask-ai -p code "Write a Rust function to parse JSON"
```

**Characteristics:**
- Returns only code
- No "Here's the code" introductions
- No "Hope this helps" conclusions
- Minimal comments (docstrings only if essential)
- Proper syntax highlighting

**When to use:**
- You want just the code
- Scripting and automation
- Code snippets for documentation

### code_with_tools

Code mode with web search tools enabled.

```bash
ask-ai -p code_with_tools "Latest Rust async patterns"
```

**Characteristics:**
- All code mode features
- Web search tools available
- Can research before coding
- Up-to-date code examples

**When to use:**
- Need current best practices
- Researching new APIs or libraries
- Want examples from latest documentation

### summarize

Professional summarization guidelines.

```bash
ask-ai summarize --style academic "Text..."
```

**Characteristics:**
- Objective tone
- Key points highlighted
- No personal opinions
- Structured output

**When to use:**
- Summarization command (automatic)
- Professional contexts
- Academic or business documents

## Usage Examples

### Basic Usage

```bash
# Default mode (no -p needed)
ask-ai "What is Rust?"

# Explicit mode selection
ask-ai -p tool_user "Complex query"

# Code mode
ask-ai -p code "Write a function"
```

### With Models

```bash
# Tool user with capable model
ask-ai -m qwen2.5-coder:7b -p tool_user "Tell me about Pikachu"

# Code mode with code model
ask-ai -m qwen2.5-coder:7b -p code "Implement sort"

# Default with any model
ask-ai -m qwen3.5:4b -p default "Question"
```

### Combining Options

```bash
# Code mode with think
ask-ai -p code -t "Complex algorithm"

# Tool user with debug
ask-ai -p tool_user -d "Debug this"

# Code with specific model and debug
ask-ai -m qwen3-coder -p code -d "Debug code generation"
```

## Mode Selection Guide

```mermaid
graph TD
    A[What do you need?] --> B{Code?}
    B -->|Yes> C{Research needed?}
    C -->|Yes> D[code_with_tools]
    C -->|No> E[code]
    B -->|No> F{Tools?}
    F -->|Yes> G[tool_user]
    F -->|No> J[default]
```

## Best Practices

1. **Start with default** - Works for most queries
2. **Use code for scripts** - Clean output for automation
3. **Tool user for capabilities** - When using Pokémon/Weather/Web
4. **Match to task** - Code mode for coding, etc.

## Listing Modes

See available modes:

```bash
ask-ai --list
```

Shows all prompt modes with descriptions.

## See Also

- [query](./commands/query.md) - Using prompt modes
- [Models](./models.md) - Available models
- [Tools](./tools.md) - Available tools
