# Quick Start Guide

Get up and running with Sprachspiel in just 5 minutes. This guide covers the essential commands you'll use every day.

## Prerequisites

Before starting, ensure:
- Sprachspiel is installed (see [Installation](./installation.md))
- Ollama is running (`ollama serve`)
- Default model is pulled (`ollama pull qwen3.5:4b`)

## Your First Query

Let's start with a simple question:

```bash
sprach "What is the capital of France?"
```

You should see a nicely formatted markdown response. That's it - Sprachspiel is working!

## Essential Commands

### 1. Query Mode (Default)

The default mode when you don't specify a subcommand:

```bash
# Basic query
sprach "Explain quantum computing"

# With think mode (for reasoning models)
sprach -t "Solve this step by step"

# With specific model
sprach -m qwen3.5:4b "Generate a Python function"

# Plain text (no markdown)
sprach --plain "List Rust keywords"
```

### 2. Translation

Translate text between languages:

```bash
# English to Portuguese
sprach translate en:pt "Hello world"

# Auto-detect source
sprach translate :pt "Hello world"

# From stdin
echo "Hello" | sprach translate :es
```

### 3. OCR (Text Extraction)

Extract text from images:

```bash
# Extract text from image
sprach ocr document.png

# Extract tables
sprach ocr --mode table spreadsheet.png

# Extract formulas (LaTeX output)
sprach ocr --mode formula equation.png
```

### 4. Summarization

Summarize long text:

```bash
# Summarize text directly
sprach summarize "Long text here..."

# From file
cat article.txt | sprach summarize

# With style
sprach summarize --style academic "Research paper text..."
```

## Working with Pipes

Sprachspiel shines when combined with pipes:

```bash
# OCR then summarize
sprach ocr document.png | sprach summarize

# OCR then translate
sprach ocr japanese.png | sprach translate ja:pt

# Full pipeline
sprach ocr document.png | sprach summarize | sprach translate :pt
```

## Useful Flags

Here are flags you'll use often:

| Flag | Description | Example |
|------|-------------|---------|
| `-m` | Select model | `sprach -m qwen3.5:4b "query"` |
| `-t` | Think mode | `sprach -t "complex question"` |
| `--plain` | No markdown | `sprach --plain "text"` |
| `-d` | Debug mode | `sprach -d "query"` |
| `--help` | Show help | `sprach --help` |

## List Available Resources

```bash
# List all models
sprach --list

# List supported languages
sprach translate --list

# Filter languages
sprach translate --list pt
```

## Common Workflows

### Document Processing Pipeline

Process a scanned document end-to-end:

```bash
# 1. OCR the scanned document
sprach ocr scanned-document.png > extracted.txt

# 2. Summarize the content
cat extracted.txt | sprach summarize --style technical > summary.txt

# 3. Translate if needed
cat summary.txt | sprach translate :pt > summary-pt.txt
```

### Code Generation

Generate code with the right model:

```bash
# Use code mode for better code output
sprach -m qwen3-coder -c "Write a Rust function to parse JSON"

# Or code_with_tools for web research + code
sprach -p code_with_tools "Latest Rust async patterns with examples"
```

### Translation Batch

Translate multiple lines:

```bash
# Create a file with text to translate
cat > to-translate.txt << 'EOF'
Hello
How are you?
Thank you
Goodbye
EOF

# Translate each line
cat to-translate.txt | sprach translate :pt
```

## Quick Tips

1. **Use `-d` for troubleshooting**: If something isn't working, add `-d` to see what's happening

2. **Pipe-friendly**: Most commands accept input from stdin, making them perfect for scripts

3. **Markdown by default**: Output is formatted with markdown. Use `--plain` for raw text

4. **Model selection**: Different tasks benefit from different models:
   - General queries: `qwen3.5:4b` (default)
   - Coding: `qwen2.5-coder:7b` (code mode)
   - Tools: `qwen3.5:4b`
   - Summarization: `qwen3.5:4b`

5. **Chaining**: Combine commands for powerful workflows. The output of one command becomes the input of the next

## Next Steps

Now that you know the basics:

- **[Commands Reference](./commands/README.md)** - Detailed documentation for each command
- **[Models](./models.md)** - Learn about available models and when to use them
- **[Pipelines](./pipelines.md)** - Advanced piping and scripting examples

Happy querying! 🎉
