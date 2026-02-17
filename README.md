# Ask-AI

A powerful Rust CLI tool for interacting with Ollama LLM models with support for translation, OCR, summarization, and tool-assisted queries.

## Overview

Ask-AI provides a comprehensive command-line interface to local and cloud-based LLMs through Ollama.

## Quick Start

```bash
# Install
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
cd ask-ollama-rs
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
- Required models: `lfm2.5-thinking`, `translategemma`, `glm-ocr`, `llama3.2`

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
