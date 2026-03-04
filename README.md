# Ask-AI

A powerful Rust CLI tool for interacting with Ollama LLM models with support for translation, OCR, summarization, and tool-assisted queries.

## Overview

Ask-AI provides a comprehensive command-line interface to local and cloud-based LLMs through Ollama.

## Quick Start

```bash
# Install (one-liner)
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash

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

### Option 1: One-Liner (Recommended)

Install directly from GitHub releases:

```bash
# Latest version
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash

# Specific version
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai-rs/main/scripts/install-ask-ai.sh | bash -s -- --version 0.25.0

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
tar -xzf ask-ai-0.25.0-linux-x86_64.tar.gz
cd ask-ai-0.25.0-linux-x86_64

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
cd ask-ai

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
wget https://github.com/luksamuk/ask-ai-rs/releases/download/v0.25.0/ask-ai-0.25.0-termux-aarch64.tar.gz
tar -xzf ask-ai-0.25.0-termux-aarch64.tar.gz
cd ask-ai-0.25.0-termux-aarch64
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

- `ask-ai [query]` - General LLM queries (default command)
- `ask-ai query [QUERY]` - General LLM queries
- `ask-ai chat` - Interactive chat session
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

# Interactive chat
ask-ai chat

# Query with tools
ask-ai "What's the weather in Tokyo?"

# Query specific model with think mode
ask-ai -m glm-5:cloud -t "Explain quantum computing"
```

## Requirements

- [Ollama](https://ollama.ai) running locally (or on a remote server for Termux)
- Required models: `llama3.1:8b`, `translategemma:4b`, `glm-ocr:bf16`, `moondream:1.8b`

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