# Ask-AI

<img src="assets/ask-ai-banner.png" alt="ask-ai banner" width="100%">

A Rust CLI harness for research, interaction, and cognitive evolution with local and cloud LLMs.

## Overview

Ask-AI is a cognitive interaction harness — not a code-specific tool — built around Ollama LLMs. It provides persistent memory (factual + semantic), adaptive personality (SOUL.md), 50+ extensible tools, and conversational agent capabilities. Designed for research, knowledge management, and open-ended cognitive interaction rather than narrowly scoped development workflows.

Key capabilities:
- **Persistent memory** — facts, notes, documents with semantic search and Ebbinghaus decay
- **Adaptive personality** — SOUL.md defines agent identity and behavior style
- **50+ tools** — file operations, web search, calculations, task management, and more
- **Context intelligence** — hybrid RAG (BM25 + vector + RRF), auto-compaction, context overflow protection
- **Translation, OCR, summarization** — specialized modes for different cognitive tasks

## Quick Start

```bash
# Install (one-liner)
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash

# Basic query
ask-ai "What is Rust?"

# Interactive chat with semantic search
ask-ai chat

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

### Option 1: One-Liner (Recommended)

Install directly from GitHub releases:

```bash
# Latest version
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash

# Specific version
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash -s -- --version 0.26.0

# With all tools
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash -s -- --tools all

# System-wide (requires sudo)
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash -s -- --prefix /usr
```

Installs to `~/.local/bin` by default. The manpage is installed to `~/.local/share/man/man1`.

### Option 2: Download Tarball

Download from [GitHub Releases](https://github.com/luksamuk/ask-ai-rs/releases):

```bash
# Download and extract
tar -xzf ask-ai-0.26.0-linux-x86_64.tar.gz
cd ask-ai-0.26.0-linux-x86_64

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
git clone https://github.com/luksamuk/ask-ai-rs.git
cd ask-ai-rs

# Install required models first
cd modelfiles && make models-essential && cd ..

# Build and install
make install

# Or install to ~/.local (recommended for development)
make install-local
```

### Termux (Android)

Ask-AI works on Termux! Download the Termux tarball from releases:

```bash
# In Termux
pkg install wget

# Download and install
wget https://github.com/luksamuk/ask-ai-rs/releases/download/v0.26.0/ask-ai-0.26.0-termux-aarch64.tar.gz
tar -xzf ask-ai-0.26.0-termux-aarch64.tar.gz
cd ask-ai-0.26.0-termux-aarch64
./install.sh
```

**Note:** Ollama must run on a separate machine. Configure in `~/.config/ask-ai/config.toml`:

```toml
[ollama]
host = "192.168.1.100:11434"  # Your desktop/server IP
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
- **Man Page**: `man ask-ai`
- **Online**: Build with `cd doc && mdbook build`

## Commands

### Query Mode (Default)

```bash
ask-ai "What is Rust?"           # Basic query
ask-ai -m qwen3.5:4b "Explain async"  # Specific model
ask-ai -c "Write a Python function"  # Code mode
ask-ai -t "Think step by step"   # Think mode
```

### Chat Mode

Interactive chat with persistent history and semantic search:

```bash
ask-ai chat                      # Start chat session
ask-ai chat -m glm-5:cloud       # Specific model
ask-ai chat -t                   # Chat with thinking
ask-ai chat --anonymous          # Anonymous session (no history)
```

**Chat Commands:**
- `/search <query>` - Search conversation history semantically
- `/context` - Show context usage and token count
- `/compact` - Compact old messages to free context
- `/model <name>` - Switch model mid-session
- `/clear` - Clear current session
- `/save [name]` / `/load <name>` - Save/load sessions

### Translate

```bash
ask-ai translate en:pt "Hello world"    # English to Portuguese
ask-ai translate :es "Bonjour"          # Auto-detect to Spanish
cat file.txt | ask-ai translate :pt     # Pipe input
```

### OCR

```bash
ask-ai ocr document.png                 # Extract text
ask-ai ocr --detailed image.jpg         # Detailed extraction
ask-ai ocr page1.png page2.png           # Multiple files
```

### Summarize

```bash
ask-ai summarize "Long text..."         # Summarize text
cat article.txt | ask-ai summarize      # Pipe input
ask-ai summarize --style bullets file.txt  # Bullet points
```

### Vision

```bash
ask-ai vision photo.jpg "What's in this image?"
ask-ai vision screenshot.png "Describe the UI"
```

## Examples

```bash
# OCR → Summarize → Translate
ask-ai ocr document.png | ask-ai summarize | ask-ai translate :pt

# Translate a file
cat article.txt | ask-ai translate :es

# Code with specific model
ask-ai -m qwen3-coder "Write a Python function"

# Interactive chat with semantic search
ask-ai chat
>>> /search "What did we discuss about databases?"
>>> /context
>>> /model glm-5:cloud

# Query with tools
ask-ai "What's the weather in Tokyo?"
ask-ai "Read the README.md and explain the project"
ask-ai "Calculate 15% of 847"

# Think mode for complex reasoning
ask-ai -m glm-5:cloud -t "Explain quantum computing step by step"
```

## Requirements

- [Ollama](https://ollama.ai) running locally (or on a remote server for Termux)
- Required models: `qwen3.5:4b` (default, multimodal), `translategemma:4b`, `glm-ocr:bf16`
- Optional: `moondream:1.8b` (alternative vision), `llama3.1:8b` (alternative general)

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
# Default build (includes: Pokémon, Weather, File, Calculator, Serper, System, Todo, LED, Remember)
make build

# With all tools (adds DuckDuckGo search, Finance)
make build-all-tools

# Install locally with all tools
make install-local-all-tools
```

| Feature | Tools | Default | Notes |
|---------|-------|---------|-------|
| `pokemon-tools` | 9 Pokémon data tools | ❌ No | Fetch Pokémon stats, types, abilities |
| `weather-tools` | 3 Weather tools | ✅ Yes | Current weather, forecast, air quality |
| `file-tools` | 5 File tools | ✅ Yes | Read files, search, list directories |
| `calc-tools` | 1 Calculator | ✅ Yes | Mathematical expressions |
| `serper-tools` | 2 Web search tools | ✅ Yes | Requires `SERPER_API_KEY` |
| `system-tools` | 2 System tools | ✅ Yes | System info, current directory |
| `todo-tools` | 5 Todo tools | ✅ Yes | Task tracking with priorities |
| `led-tools` | 4 LED control tools | Yes | Control GPIO LEDs (embedded) |
|| `skill-tools` | 2 AI behavior tools | ✅ Yes | Dynamic skill activation |
| `search-tools` | 3 DuckDuckGo tools | ❌ No | May fail due to CAPTCHA |
| `finance-tools` | 1 Stock quote tool | ❌ No | Planned |

## Configuration

Create `~/.config/ask-ai/config.toml`:

```toml
[ollama]
host = "localhost:11434"        # Ollama server address

[model]
default = "qwen3.5:4b"         # Default model (multimodal)

[model.code]
# model = "qwen2.5-coder:7b"   # Code mode default (qwen2.5-coder:7b if not set)
# tools = true                  # Tools enabled by default for code mode

[display]
skin = "dark"                   # Markdown theme: dark, light, mono

[retrieval]
enabled = true                  # Enable semantic search in chat
relevant_count = 5              # Number of messages to retrieve
```

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
- Binary (`ask-ai`)
- Manpage (`ask-ai.1`)
- Installation scripts (`install.sh`, `uninstall.sh`)
- Documentation (`README.md`, `LICENSE.txt`)
- Platform-specific instructions (Termux includes `README-TERMUX.txt`)

## Semantic Search & RAG

Ask-AI features intelligent context retrieval:

- **Chat Mode**: Automatically retrieves relevant past conversations
- **Query Mode**: Access to project-wide conversation history
- **`/search` Command**: Semantic search across all sessions
- **Remember Tool**: Let the LLM query conversation history

The system uses:
- **Hybrid Search**: BM25 (keyword) + Semantic (vector) + RRF fusion
- **Context Enrichment**: User questions are paired with assistant responses
- **Smart Positioning**: Retrieved context placed optimally (not "lost in middle")

## AI-Assisted Development

Developed with assistance from:
- **GLM 4.7 Flash** (Z.ai) — Project inception and initial scaffolding
- **GLM 5** (Z.ai) — Plan, research, and implementation
- **GLM 5.1** (Z.ai) — Ongoing development and refinement
- **Kimi K2.5** (Moonshot AI) — Architecture design
- **MiniMax M2.7** (MiniMax) — Code review and testing

Agent harnesses used:
- **OpenCode** — Primary development harness
- **Hermes Agent** — Orchestration, asset creation, and project management

Human oversight for architecture decisions and quality assurance.

## License

MIT License - see [LICENSE.txt](LICENSE.txt)

Copyright (c) 2026 Lucas S. Vieira

---

For complete documentation, see the `doc/` directory or run `man ask-ai`.