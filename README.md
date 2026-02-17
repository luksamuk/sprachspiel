# Ask-Ollama

A Rust CLI tool for interacting with Ollama LLM models with support for translation, OCR, summarization, and tool-assisted queries.

## Overview

Ask-Ollama provides a powerful command-line interface to local and cloud-based LLMs through Ollama, featuring:

- **Multiple Model Support**: Switch between various models (LFM 2.5, Mistral Small, GPT-OSS, and cloud models like GLM-5, Kimi-K2.5, etc.)
- **Translation Subcommand**: Translate text between 50+ languages using TranslateGemma
- **OCR Subcommand**: Extract text from images using GLM-OCR (supports text, tables, figures, formulas)
- **Summarize Subcommand**: Create concise summaries with customizable styles
- **Tool Integration**: Automatic capability detection with support for Pokémon data tools
- **Markdown Rendering**: Beautiful terminal output via termimad
- **Think Mode**: Support for reasoning models
- **Stdin Support**: Pipe content directly into the tool

## Quick Start

```bash
# Basic query (uses default model)
ask "What is Rust?"

# Translation
ask translate en:pt "Hello world"
ask translate :pt "Hello world"  # Auto-detect source

# OCR (extract text from images)
ask ocr document.png
ask ocr --mode table spreadsheet.png

# Summarization
echo "Long text here..." | ask summarize
ask summarize --style technical documentation.txt

# List available models
ask --list
```

## AI-Assisted Development Disclaimer

This project was developed with assistance from Large Language Models:

- **Kimi K2.5** (Moonshot AI): Used for **Build** and **Research** phases
  - Code generation and implementation
  - Debugging and error resolution
  - Research on Rust patterns and best practices

- **GLM 5** (Z.ai): Used for **Plan** and **Brainstorm** phases
  - Architecture design and planning
  - Feature brainstorming and roadmap development
  - Documentation structure and organization

The human developer maintained oversight, made architectural decisions, reviewed all generated code, and ensured quality standards.

## Installation

```bash
# Build from source
git clone <repository>
cd ask-ollama-rs
cargo build --release

# The binary will be at target/release/ask-ollama
```

## Configuration

Models are configured in `src/config.rs`. Available presets include:

| Model | Description | Context |
|-------|-------------|---------|
| `lfm` | LFM 2.5 Thinking (default) | 32K |
| `mistral-small` | Mistral Small 3.2 | 32K |
| `gpt-oss` | GPT-OSS 20B | 64K |
| `qwen3-coder` | Qwen3 Coder | 64K |
| `glm-5` | GLM-5 Cloud | 198K |
| `kimi-k2.5` | Kimi K2.5 Cloud | 256K |
| `translate` | TranslateGemma 12B | 32K |

## Usage

### Query Mode

```bash
ask "Your question here"
ask -m mistral-small "Generate a Python function"
ask -t "Explain quantum computing"  # Think mode
cat file.txt | ask "Summarize this"
```

### Translation

```bash
ask translate en:pt "Hello"
ask translate :pt "Auto-detect source"
ask translate he:en "שלום"
ask translate --list  # Show all languages
```

### OCR

```bash
ask ocr image.png                    # Extract text
ask ocr --mode table invoice.png     # Extract tables
ask ocr --mode formula math.png     # Extract LaTeX formulas
ask ocr --json image1.png image2.png # JSON output
```

### Summarize

```bash
ask summarize "Long text..."
ask summarize --max-length 200 "Text..."
ask summarize --format bullets --style technical code.md
```

## Documentation

- `AGENTS.md` - Development guidelines and code style
- `IMPLEMENTATION.md` - Detailed feature roadmap and architecture decisions
- `TOOL_CALLING_RESEARCH.md` - Research on tool calling implementation

## License

This project is licensed under the MIT License - see the [LICENSE.txt](LICENSE.txt) file for details.

Copyright (c) 2026 Lucas S. Vieira

## Contributing

[Add contribution guidelines if applicable]
