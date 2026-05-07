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
sprachspiel "What is the capital of France?"
```

You should see a nicely formatted markdown response. That's it - Sprachspiel is working!

## Essential Commands

### 1. Query Mode (Default)

The default mode when you don't specify a subcommand:

```bash
# Basic query
sprachspiel "Explain quantum computing"

# With think mode (for reasoning models)
sprachspiel -t "Solve this step by step"

# With specific model
sprachspiel -m qwen3.5:4b "Generate a Python function"

# Plain text (no markdown)
sprachspiel --plain "List Rust keywords"
```

### 2. Translation

Translate text between languages:

```bash
# English to Portuguese
sprachspiel translate en:pt "Hello world"

# Auto-detect source
sprachspiel translate :pt "Hello world"

# From stdin
echo "Hello" | sprachspiel translate :es
```

### 3. OCR (Text Extraction)

Extract text from images:

```bash
# Extract text from image
sprachspiel ocr document.png

# Extract tables
sprachspiel ocr --mode table spreadsheet.png

# Extract formulas (LaTeX output)
sprachspiel ocr --mode formula equation.png
```

### 4. Summarization

Summarize long text:

```bash
# Summarize text directly
sprachspiel summarize "Long text here..."

# From file
cat article.txt | sprachspiel summarize

# With style
sprachspiel summarize --style academic "Research paper text..."
```

## Working with Pipes

Sprachspiel shines when combined with pipes:

```bash
# OCR then summarize
sprachspiel ocr document.png | sprachspiel summarize

# OCR then translate
sprachspiel ocr japanese.png | sprachspiel translate ja:pt

# Full pipeline
sprachspiel ocr document.png | sprachspiel summarize | sprachspiel translate :pt
```

## Useful Flags

Here are flags you'll use often:

| Flag | Description | Example |
|------|-------------|---------|
| `-m` | Select model | `sprachspiel -m qwen3.5:4b "query"` |
| `-t` | Think mode | `sprachspiel -t "complex question"` |
| `--plain` | No markdown | `sprachspiel --plain "text"` |
| `-d` | Debug mode | `sprachspiel -d "query"` |
| `--help` | Show help | `sprachspiel --help` |

## List Available Resources

```bash
# List all models
sprachspiel --list

# List supported languages
sprachspiel translate --list

# Filter languages
sprachspiel translate --list pt
```

## Common Workflows

### Document Processing Pipeline

Process a scanned document end-to-end:

```bash
# 1. OCR the scanned document
sprachspiel ocr scanned-document.png > extracted.txt

# 2. Summarize the content
cat extracted.txt | sprachspiel summarize --style technical > summary.txt

# 3. Translate if needed
cat summary.txt | sprachspiel translate :pt > summary-pt.txt
```

### Code Generation

Generate code with the right model:

```bash
# Use code mode for better code output
sprachspiel -m qwen3-coder -c "Write a Rust function to parse JSON"

# Or code_with_tools for web research + code
sprachspiel -p code_with_tools "Latest Rust async patterns with examples"
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
cat to-translate.txt | sprachspiel translate :pt
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
