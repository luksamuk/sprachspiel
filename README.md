# Ask-AI

A powerful Rust CLI tool for interacting with Ollama LLM models with support for translation, OCR, summarization, and tool-assisted queries.

## Overview

Ask-AI provides a comprehensive command-line interface to local and cloud-based LLMs through Ollama, featuring:

- **Multiple Model Support**: Switch between various models (LFM 2.5, Mistral Small, GPT-OSS, Llama 3.2, and cloud models like GLM-5, Kimi-K2.5, etc.)
- **Translation**: Translate text between 50+ languages using TranslateGemma
- **OCR**: Extract text from images using GLM-OCR (supports text, tables, figures, formulas)
- **Summarization**: Create concise summaries with customizable styles and markdown rendering
- **Tool Integration**: Automatic capability detection with support for Pokémon data tools
- **Markdown Rendering**: Beautiful terminal output via termimad
- **Think Mode**: Support for reasoning models
- **Stdin Support**: Pipe content directly into the tool
- **Chained Commands**: Combine subcommands for powerful workflows

## Quick Start

```bash
# Basic query (uses default model)
ask-ai "What is Rust?"

# Translation
ask-ai translate en:pt "Hello world"
ask-ai translate :pt "Hello world"  # Auto-detect source

# OCR (extract text from images)
ask-ai ocr document.png
ask-ai ocr --mode table spreadsheet.png

# Summarization (with markdown rendering)
echo "Long text here..." | ask-ai summarize
ask-ai summarize --style technical documentation.txt

# List available models
ask-ai --list
```

## Installation

### From Source

```bash
# Clone the repository
git clone <repository>
cd ask-ollama-rs

# Build and install (default: /usr/local)
make install

# Or specify custom prefix
make install PREFIX=/usr

# Uninstall
make uninstall
```

### Manual Installation

```bash
# Build release binary
cargo build --release

# Copy binary to your PATH
sudo cp target/release/ask-ollama /usr/local/bin/ask-ai
```

## AI-Assisted Development

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

## Available Models

Models are configured in `src/config.rs`. Available presets include:

| Model | Description | Context | Best For |
|-------|-------------|---------|----------|
| `lfm` | LFM 2.5 Thinking (default) | 32K | General queries with reasoning |
| `llama3.2` | Llama 3.2 3B | 32K | **Summarization** (default) |
| `mistral-small` | Mistral Small 3.2 | 32K | Agentic tasks with tools |
| `gpt-oss` | GPT-OSS 20B | 64K | Tool calling |
| `qwen3-coder` | Qwen3 Coder | 64K | Code generation |
| `smollm3` | SmolLM3 3B | 64K | Edge deployment |
| `sead` | SEAD 14B | 32K | General purpose |
| `devstral-small-2` | Devstral 24B | 64K | Coding with min_p |
| `glm-5` | GLM-5 Cloud | 198K | Complex reasoning |
| `kimi-k2.5` | Kimi K2.5 Cloud | 256K | Multimodal agentic |
| `minimax-m2.5` | MiniMax M2.5 Cloud | 198K | Coding and agentic |
| `qwen3.5` | Qwen3.5 Cloud | 256K | Vision-language |
| `translate` | TranslateGemma 12B | 32K | Translation tasks |
| `pepe` | Assistant Pepe 8B | 64K | Character model |

## Commands Reference

### General Query Mode

Query the default model with natural language:

```bash
# Basic query
ask-ai "Your question here"

# Use specific model
ask-ai -m mistral-small "Generate a Python function"

# Enable think mode (for reasoning models)
ask-ai -t "Explain quantum computing"

# Output plain text (no markdown)
ask-ai --plain "List Rust keywords"

# Debug mode (shows configuration)
ask-ai -d "Test query"

# Pipe content from stdin
cat file.txt | ask-ai "Summarize this"
echo "What is the capital of France?" | ask-ai

# Force enable tools
ask-ai --tools "Tell me about Pikachu"

# List all models and prompts
ask-ai --list
```

### Translation (`translate`)

Translate text between 50+ languages:

```bash
# Basic translation (source:target)
ask-ai translate en:pt "Hello"

# Auto-detect source language
ask-ai translate :pt "Auto-detected source"

# Translate from Hebrew to English
ask-ai translate he:en "שלום"

# List all supported languages
ask-ai translate --list

# Filter language list
ask-ai translate --list pt

# Pipe from stdin
cat english.txt | ask-ai translate :pt

# OCR then translate
ask-ai ocr document.png | ask-ai translate :es
```

**Supported language codes and aliases:**
- `pt` / `portuguese` - Portuguese
- `pt-BR` / `br` / `brazil` - Portuguese (Brazil)
- `en` / `english` - English
- `es` / `spanish` - Spanish
- `fr` / `french` - French
- `de` / `german` - German
- `it` / `italian` - Italian
- `ja` / `japanese` - Japanese
- `zh-Hans` / `zh-cn` - Chinese Simplified
- `zh-Hant` / `zh-tw` - Chinese Traditional
- And 40+ more...

### OCR (`ocr`)

Extract text from images using GLM-OCR:

```bash
# Extract text from image
ask-ai ocr document.png

# Extract tables (preserves structure)
ask-ai ocr --mode table invoice.png

# Extract formulas (outputs LaTeX)
ask-ai ocr --mode formula math.png

# Extract text from figures/diagrams
ask-ai ocr --mode figure diagram.png

# Multiple files (batch processing)
ask-ai ocr page1.png page2.png page3.png

# JSON output (one JSON object per line)
ask-ai ocr --json document.png

# OCR then summarize
ask-ai ocr document.png | ask-ai summarize --style academic

# OCR then translate
ask-ai ocr english-document.png | ask-ai translate :pt
```

**Supported image formats:** PNG, JPG, JPEG, WEBP, GIF

### Summarization (`summarize`)

Create concise summaries with customizable styles:

```bash
# Basic summarization (markdown output by default)
ask-ai summarize "Long text to summarize..."

# Limit summary length (approximate word count)
ask-ai summarize --max-length 200 "Very long text..."

# Output format: paragraph, bullets, or both (default)
ask-ai summarize --format bullets "Text..."
ask-ai summarize --format paragraph "Text..."

# Style presets
ask-ai summarize --style general "Text..."      # General audience
ask-ai summarize --style technical "Text..."    # Technical documentation
ask-ai summarize --style academic "Text..."     # Academic papers
ask-ai summarize --style business "Text..."     # Business reports

# Plain text output (no markdown)
ask-ai summarize --plain "Text..."

# Combine options
ask-ai summarize --format bullets --style technical --max-length 150 "Code documentation..."

# Pipe from stdin
cat article.txt | ask-ai summarize --style academic

# OCR then summarize
ask-ai ocr research-paper.png | ask-ai summarize --style academic --format both

# Multi-step pipeline
ask-ai ocr document.png | ask-ai summarize | ask-ai translate :pt
```

### Chained Commands (Pipes)

Combine subcommands for powerful workflows:

```bash
# Extract text from image and summarize it
ask-ai ocr document.png | ask-ai summarize --style academic

# Translate OCR output from English to Portuguese
ask-ai ocr english-text.png | ask-ai translate :pt

# OCR a screenshot, summarize, then translate
ask-ai ocr screenshot.png | ask-ai summarize --format bullets | ask-ai translate :pt

# Extract table from image and get a technical summary
ask-ai ocr --mode table data.png | ask-ai summarize --style technical

# Chain with external tools
cat article.pdf | pdftotext - - | ask-ai summarize --max-length 100

# Process multiple documents
cat doc1.txt doc2.txt | ask-ai summarize --style business
```

## Global Options

These options work with all commands:

| Option | Short | Description |
|--------|-------|-------------|
| `--model` | `-m` | Select model preset (default: lfm) |
| `--prompt` | `-p` | System prompt mode (default, tool_user) |
| `--think` | `-t` | Enable think mode for supported models |
| `--plain` | | Output plain text without markdown |
| `--debug` | `-d` | Dry-run mode, print config |
| `--list` | `-l` | List available models and prompts |
| `--tools` | | Force enable tools |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

## Documentation

- `AGENTS.md` - Development guidelines and code style
- `IMPLEMENTATION.md` - Detailed feature roadmap and architecture decisions
- `TOOL_CALLING_RESEARCH.md` - Research on tool calling implementation
- `LICENSE.txt` - MIT License

## License

This project is licensed under the MIT License - see the [LICENSE.txt](LICENSE.txt) file for details.

Copyright (c) 2026 Lucas S. Vieira

## Contributing

Contributions are welcome! Please see the project structure in `AGENTS.md` for coding guidelines.

## Tips

1. **Use `--plain`** for piping output to other programs
2. **Use `--style technical`** for code and documentation
3. **Use `--style academic`** for research papers
4. **Chain commands** for complex workflows (OCR → Summarize → Translate)
5. **Check `--list`** to see all available models
6. **Use `-d/--debug`** to see what configuration is being used
