# Sprachspiel

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE.txt)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

<img src="assets/sprachspiel-banner.png" alt="sprachspiel banner" width="100%">

A Rust CLI harness for research, interaction, and cognitive evolution with local and cloud LLMs.

## Overview

Sprachspiel is a cognitive interaction harness — not a code-specific tool — built for local and cloud LLMs via OpenAI-compatible backends. It provides persistent memory (factual + semantic), adaptive personality (SOUL.md), 50+ extensible tools, and conversational agent capabilities. Designed for research, knowledge management, and open-ended cognitive interaction rather than narrowly scoped development workflows.

Key capabilities:
- **Persistent memory** — facts, notes, documents with semantic search and Ebbinghaus decay
- **Adaptive personality** — SOUL.md defines agent identity and behavior style
- **50+ tools** — file operations, web search, calculations, task management, and more
- **Context intelligence** — hybrid RAG (BM25 + vector + RRF), auto-compaction, context overflow protection
- **Translation, OCR, summarization** — specialized modes for different cognitive tasks

## Quick Start

```bash
# Install (one-liner)
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash

# Basic query
sprach "What is Rust?"

# Interactive chat with semantic search
sprach chat

# Translate
sprach translate en:pt "Hello world"

# OCR
sprach ocr document.png

# Summarize
echo "Long text..." | sprach summarize

# List models
sprach --list
```

## Installation

### Option 1: One-Liner (Recommended)

Install directly from GitHub releases:

```bash
# Latest version
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash

# Specific version
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash -s -- --version 0.26.0

# With all tools
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash -s -- --tools all

# System-wide (requires sudo)
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash -s -- --prefix /usr
```

Installs to `~/.local/bin` by default. The manpage is installed to `~/.local/share/man/man1`.

### Option 2: Download Tarball

Download from [GitHub Releases](https://github.com/luksamuk/sprachspiel/releases):

```bash
# Download and extract
tar -xzf sprachspiel-0.26.0-linux-x86_64.tar.gz
cd sprachspiel-0.26.0-linux-x86_64

# Install
./install.sh

# Or install to custom location
./install.sh --prefix /usr    # System-wide (requires sudo)
./install.sh --bin ~/bin      # Custom binary location
./install.sh --man ~/man      # Custom manpage location

# Uninstall
./uninstall.sh
```

### Option 3: Build from Source

```bash
# Clone
git clone https://github.com/luksamuk/sprachspiel.git
cd sprachspiel

# Build and install
make install

# Or install to ~/.local (recommended for development)
make install-local
```

### Termux (Android)

Sprachspiel works on Termux! Download the Termux tarball from releases:

```bash
# In Termux
pkg install wget

# Download and install
wget https://github.com/luksamuk/sprachspiel/releases/download/v0.26.0/sprachspiel-0.26.0-termux-aarch64.tar.gz
tar -xzf sprachspiel-0.26.0-termux-aarch64.tar.gz
cd sprachspiel-0.26.0-termux-aarch64
./install.sh
```

**Note:** An LLM server (e.g. Ollama, llama.cpp, llama-swap, vLLM) must run on a separate machine. Configure the provider in `~/.config/sprachspiel/models.toml`:

```toml
[provider."remote"]
kind = "openai"
base_url = "http://192.168.1.100:11434/v1"
```

### Post-Installation

Add `~/.local/bin` to PATH if not already:

```bash
# Add to shell config (~/.bashrc, ~/.zshrc, etc.)
export PATH="$HOME/.local/bin:$PATH"

# For manpage access
export MANPATH="$HOME/.local/share/man:$MANPATH"

# Source the config
source ~/.bashrc  # or ~/.zshrc
```

## Documentation

**Complete documentation** is available at `doc/`:

- **User Guide**: `cd doc && mdbook serve`
- **Man Page**: `man sprach`
- **Online**: Build with `cd doc && mdbook build`

## Commands

### Query Mode (Default)

```bash
sprach "What is Rust?"           # Basic query
sprach -m qwen3.5:4b "Explain async"  # Specific model
sprach -c "Write a Python function"  # Code mode
sprach -t "Think step by step"   # Think mode
```

### Chat Mode

Interactive chat with persistent history and semantic search:

```bash
sprach chat                      # Start chat session
sprach chat -m qwen3.5-4b       # Specific model
sprach chat -t                   # Chat with thinking
sprach chat --anonymous          # Anonymous session (no history)
```

**Chat Commands:**
- `/search <query>` - Search conversation history semantically
- `/context` - Show context usage and token count
- `/compact` - Compact old messages to free context
- `/model <name>` - Switch model mid-session
- `/clear` - Clear current session
- `/save [name]` / `/load <name>` - Save/load sessions
- `/ocr <file> [mode]` - Extract text from image
- `/vision <path> [prompt]` - Analyze image with vision model
- `/translate <src:dst> <text>` - Translate text
- `/summarize <text>` - Summarize text
- `/fact add|list|search|remove|prune` - Manage facts
- `/note add|list|show|edit|delete|search` - Manage notes
- `/doc import|list|show|delete` - Manage documents
- `/todo add|list|done|remove|update` - Manage tasks
- `/think` - Toggle think mode
- `/tools` - Toggle tools
- `/skill [name]` - List or activate skills

### Translate

```bash
sprach translate en:pt "Hello world"    # English to Portuguese
sprach translate :es "Bonjour"          # Auto-detect to Spanish
cat file.txt | sprach translate :pt     # Pipe input
```

### OCR

```bash
sprach ocr document.png                 # Extract text
sprach ocr --detailed image.jpg         # Detailed extraction
sprach ocr page1.png page2.png           # Multiple files
```

### Summarize

```bash
sprach summarize "Long text..."         # Summarize text
cat article.txt | sprach summarize      # Pipe input
sprach summarize --style bullets file.txt  # Bullet points
```

### Vision

```bash
sprach vision photo.jpg "What's in this image?"
sprach vision screenshot.png "Describe the UI"
```

## Examples

```bash
# OCR → Summarize → Translate
sprach ocr document.png | sprach summarize | sprach translate :pt

# Translate a file
cat article.txt | sprach translate :es

# Code with specific model
sprach -m qwen3.5-4b "Write a Python function"

# Interactive chat with semantic search
sprach chat
>>> /search "What did we discuss about databases?"
>>> /context
>>> /model qwen3.5-4b

# Query with tools
sprach "What's the weather in Tokyo?"
sprach "Read the README.md and explain the project"
sprach "Calculate 15% of 847"

# Think mode for complex reasoning
sprach -m qwen3.5-4b -t "Explain quantum computing step by step"
```

## Requirements

- An OpenAI-compatible LLM server (Ollama, llama.cpp, vLLM, LM Studio, llama-swap) running locally or remotely
- Models configured in `~/.config/sprachspiel/models.toml` — see the [Model Guide](./doc/src/models.md) for details

## Project Context (AGENTS.md)

Sprachspiel automatically loads `AGENTS.md` from the current directory to provide project-specific context:

```bash
# If AGENTS.md exists, context is automatically injected
sprach "Explain the project structure"

# Disable with --ignore-agents
sprach --ignore-agents "General question"
```

Content is sanitized for security (injection patterns, executable code blocks removed).

## Build Features

Tools are organized into compile-time features:

```bash
# Default build (includes: Pokémon, Weather, File, Calculator, Serper, System, Todo, LED, Remember)
make build

# With all tools (adds DuckDuckGo search, Finance)
make build-all-tools

# Install locally with all tools
make install-local-all-tools
```

| Feature | Tools | Default | Notes |
|---------|-------|---------|-------|
| `pokemon-tools` | 9 Pokémon data tools | ✅ Yes | Fetch Pokémon stats, types, abilities |
| `weather-tools` | 3 Weather tools | ✅ Yes | Current weather, forecast, air quality |
| `file-tools` | 5 File tools | ✅ Yes | Read files, search, list directories |
| `calc-tools` | 1 Calculator | ✅ Yes | Mathematical expressions |
| `system-tools` | 2 System tools | ✅ Yes | System info, current directory |
| `todo-tools` | 5 Todo tools | ✅ Yes | Task tracking with priorities |
| `led-tools` | 4 LED control tools | ❌ No | Control GPIO LEDs (embedded) |
| `skill-tools` | 2 AI behavior tools | ✅ Yes | Dynamic skill activation |
| `document-tools` | 3 Document tools | ✅ Yes | Import, list, show documents |
| `subagent-tools` | 1 Subagent tool | ✅ Yes | LLM-initiated subagent delegation |
| `search-tools` | 3 DuckDuckGo tools | ❌ No | May fail due to CAPTCHA |
| `finance-tools` | 1 Stock quote tool | ❌ No | Planned |

## Configuration

Sprachspiel uses two config files:

1. **`~/.config/sprachspiel/models.toml`** — Provider endpoints and model definitions
2. **`~/.config/sprachspiel/config.toml`** — Per-subcommand defaults and display settings

```toml
# models.toml — define providers and models
[provider."llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models."qwen3.5-4b"]
model_id = "qwen3.5-4b"
provider = "llama-swap"
tools = true
thinking = true
```

```toml
# config.toml — per-subcommand defaults
[model]
default = "qwen3.5-4b"

[display]
skin = "dark"                   # Markdown theme: dark, light, mono
```

See the [Model Guide](./doc/src/models.md) and [Configuration Guide](./doc/src/configuration.md) for full details.

## Distribution Tarballs

Create distribution tarballs for release:

```bash
# Linux x86_64
make tarball-linux
make tarball-linux-all-tools

# Termux (Android aarch64)
make tarball-termux
make tarball-termux-all-tools

# All tarballs
make all-tarballs
```

Tarballs include:
- Binary (`sprach`)
- Manpage (`sprach.1`)
- Installation scripts (`install.sh`, `uninstall.sh`)
- Documentation (`README.md`, `LICENSE.txt`)
- Platform-specific instructions (Termux includes `README-TERMUX.txt`)

## Semantic Search & RAG

Sprachspiel features intelligent context retrieval:

- **Chat Mode**: Automatically retrieves relevant past conversations
- **Query Mode**: Access to project-wide conversation history
- **`/search` Command**: Semantic search across all sessions
- **Remember Tool**: Let the LLM query conversation history

The system uses:
- **Hybrid Search**: BM25 (keyword) + Semantic (vector) + RRF fusion
- **Context Enrichment**: User questions are paired with assistant responses
- **Smart Positioning**: Retrieved context placed optimally (not "lost in middle")

## Acknowledgments

Table rendering in the TUI was inspired by the [ratatui-markdown](https://github.com/celestia-island/ratatui-markdown) crate by langyo (MIT OR Apache-2.0).

Code block styling uses colors from the [Catppuccin](https://catppuccin.com) palette (MIT License).

## AI-Assisted Development

Developed with assistance from:
- **GLM 4.7 Flash** (Z.ai) — Project inception and initial scaffolding
- **GLM 5** (Z.ai) — Plan, research, and implementation
- **GLM 5.1** (Z.ai) — Development and refinement
- **GLM 5.2** (Z.ai) — Development, refinement and testing
- **GLM 5.3 Flash** (Z.ai) — Development and testing
- **Kimi K2.5** (Moonshot AI) — Architecture design
- **Kimi K2.7 Code** (Moonshot AI) — Bugfixes
- **Kimi K3** (Moonshot AI) — Skill and workflow management, planning and refinement
- **MiniMax M2.7** (MiniMax) — Code review and testing
- **MiniMax M3** (MiniMax) — Code review
- **Nemotron 3 Super** (NVIDIA) — Bugfixes

Agent harnesses used:
- **Hermes Agent** (Nous Research) — Primary development harness with custom profiles (Hermes for orchestration, Hefesto for implementation), multi-agent Kanban task routing, and RAG-assisted context
- **OpenCode** — Earlier development and code generation

Human oversight for architecture decisions and quality assurance.

## License

MIT License - see [LICENSE.txt](LICENSE.txt)

Copyright (c) 2026 Lucas S. Vieira

---

For complete documentation, see the `doc/` directory or run `man sprach`.