# Introduction

Sprachspiel is a powerful command-line interface for interacting with Large Language Models (LLMs) via Ollama and compatible backends. Built in Rust for performance and reliability, it brings AI capabilities directly to your terminal with an elegant, markdown-rendered output.

## Why Sprachspiel?

In a world of web-based AI interfaces, Sprachspiel stands out by:

- **Keeping you in the terminal** - No context switching, no browser tabs
- **Working offline** - Uses local LLM models (via Ollama or compatible backends) when available
- **Being scriptable** - Pipe content in, pipe results out
- **Rendering beautifully** - Markdown formatting in the terminal via termimad
- **Supporting tools** - Automatically uses Pokémon, Weather, and Web Search data

## What Can You Do?

### 1. Ask Questions

Get answers from AI models with beautiful markdown formatting:

```bash
sprach "Explain quantum computing in simple terms"
sprach -m qwen3.5:4b "Generate a Python function for Fibonacci"
sprach -t "Solve this step by step"  # Think mode
```

### 2. Translate Text

Translate between 50+ languages with the TranslateGemma model:

```bash
sprach translate en:pt "Hello world"
sprach translate :pt "Auto-detected source language"
cat document.txt | sprach translate :es
```

### 3. Extract Text from Images (OCR)

Use GLM-OCR to extract text, tables, formulas, and figures:

```bash
sprach ocr document.png
sprach ocr --mode table spreadsheet.png
sprach ocr --formula equation.png
```

### 4. Summarize Documents

Create concise summaries with customizable styles:

```bash
sprach summarize "Long text here..."
sprach summarize --style academic research-paper.txt
sprach summarize --format bullets --max-length 200
```

### 5. Chain Commands

Build powerful pipelines by combining commands:

```bash
# OCR → Translate
sprach ocr japanese.png | sprach translate ja:pt

# OCR → Summarize
sprach ocr report.png | sprach summarize --style technical

# OCR → Summarize → Translate
sprach ocr document.png | sprach summarize | sprach translate :pt
```

## Target Users

Sprachspiel is designed for:

- **Developers** who live in the terminal and want AI assistance without leaving it
- **Researchers** who need to process documents, extract text, and summarize papers
- **Translators** who need quick translations between multiple languages
- **System administrators** who need to extract information from images or documents
- **Power users** who value efficiency and prefer command-line interfaces
- **Anyone** who works with text and wants AI assistance integrated into their workflow

## Design Philosophy

Sprachspiel follows these principles:

1. **Terminal-First**: Everything should work beautifully in a terminal
2. **Unix Philosophy**: Do one thing well, compose with pipes
3. **Markdown Everywhere**: Beautiful formatting without GUI overhead
4. **Zero Configuration**: Works out of the box with sensible defaults
5. **Extensible**: Easy to add new models, tools, and commands

## How It Works

Sprachspiel communicates with an LLM server (such as Ollama). When you run a command:

1. **Input is collected** - From arguments or stdin
2. **Model is selected** - Based on flags or defaults
3. **Capabilities are detected** - Tools enabled if model supports them
4. **Request is sent** - To the LLM server
5. **Response is rendered** - Beautiful markdown in the terminal

```mermaid
graph LR
    A[User Input] --> B[CLI Parser]
    B --> C[Model Selection]
    C --> D[Capability Detection]
    D --> E[Send to LLM Server]
    E --> F[Receive Response]
    F --> G[Markdown Rendering]
    G --> H[Terminal Output]
```

## Getting Help

- Use `sprach --help` for quick reference
- Use `man sprach` for detailed man page
- Check this documentation for comprehensive guides
- Enable debug mode with `-d` flag for troubleshooting

## Next Steps

- **[Install Sprachspiel](./installation.md)** - Get it running on your system
- **[Quick Start Guide](./quickstart.md)** - Your first 5 minutes with Sprachspiel
- **[Commands Reference](./commands/README.md)** - Detailed command documentation
