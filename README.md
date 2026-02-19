# Ask-AI

A powerful Rust CLI tool for interacting with Ollama LLM models with support for translation, OCR, summarization, and tool-assisted queries.

## Overview

Ask-AI provides a comprehensive command-line interface to local and cloud-based LLMs through Ollama.

## Quick Start

```bash
# Install (default includes Pokémon, Weather, File tools)
make install

# Basic query
ask-ai "What is Rust?"

# Translate
ask-ai translate en:pt "Hello world"

# OCR
ask-ai ocr document.png

# Summarize
echo "Long text..." | ask-ai summarize

# List models
ask-ai --list
```

## Installation

```bash
# Clone and build
git clone <repository>
cd ask-ai
make install

# Or with custom prefix
make install PREFIX=/usr
```

## Documentation

**Complete documentation** is available at `doc/`:

- **User Guide**: `cd doc && mdbook serve`
- **Man Page**: `man ask-ai`
- **Online**: Build with `cd doc && mdbook build`

## Commands

- `ask-ai query [QUERY]` - General LLM queries
- `ask-ai translate [LANG] [TEXT]` - Translate (50+ languages)
- `ask-ai ocr [FILE...]` - Extract text from images
- `ask-ai summarize [TEXT]` - Summarize text

## Examples

```bash
# OCR → Summarize → Translate
ask-ai ocr document.png | ask-ai summarize | ask-ai translate :pt

# Translate a file
cat article.txt | ask-ai translate :es

# Code with specific model
ask-ai -m qwen3-coder "Write a Python function"
```

## Requirements

- [Ollama](https://ollama.ai) running locally
- Required models: `llama3.1:8b`, `translategemma:12b`, `glm-ocr:bf16`

## Installing Models

Ask-AI uses **modelfiles** that must be **built** with custom parameters. Simply pulling models directly won't work.

```bash
cd modelfiles

# Install essential models (builds with custom config)
make models-essential

# Install optional recommended models
make models-optional
```

**Important:** Models must be built via the Makefile to apply custom configurations (context window, temperature, etc.). Direct `ollama pull` won't work correctly.

See `modelfiles/README.md` for details.

## Project Context (AGENTS.md)

Ask-AI automatically loads `AGENTS.md` from the current directory to provide project-specific context:

```bash
# If AGENTS.md exists, context is automatically injected
ask-ai "Explain the project structure"

# Disable with --ignore-agents
ask-ai --ignore-agents "General question"
```

Content is sanitized for security (injection patterns, executable code blocks removed).

## Build Features

Tools are organized into compile-time features:

```bash
# Default build (includes: Pokémon, Weather, File, Calculator, Serper, System tools)
make build

# With all tools (adds DuckDuckGo search, Finance)
make build-all-tools

# Install locally with all tools
make install-local-all-tools
```

| Feature | Tools | Default | Notes |
|---------|-------|---------|-------|
| `pokemon-tools` | 9 Pokémon data tools | ✅ Yes | |
| `weather-tools` | 3 Weather lookup tools | ✅ Yes | |
| `file-tools` | 5 File operation tools | ✅ Yes | |
| `calc-tools` | 1 Calculator tool | ✅ Yes | |
| `serper-tools` | 2 Web search tools | ✅ Yes | Requires `SERPER_API_KEY` |
| `system-tools` | 2 System info tools | ✅ Yes | |
| `search-tools` | 3 DuckDuckGo tools | ❌ No | May fail due to CAPTCHA |
| `finance-tools` | 1 Stock quote tool | ❌ No | Planned |

## AI-Assisted Development

Developed with assistance from:
- **Kimi K2.5** (Moonshot AI) - Build and Research
- **GLM 5** (Z.ai) - Plan and Brainstorm

Human oversight for architecture decisions and quality assurance.

## License

MIT License - see [LICENSE.txt](LICENSE.txt)

Copyright (c) 2026 Lucas S. Vieira

---

For complete documentation, see the `doc/` directory or run `man ask-ai`.
