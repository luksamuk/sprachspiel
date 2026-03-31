# Quick Start Guide

Get up and running with Ask-AI in just 5 minutes. This guide covers the essential commands you'll use every day.

## Prerequisites

Before starting, ensure:
- Ask-AI is installed (see [Installation](./installation.md))
- Ollama is running (`ollama serve`)
- Default model is pulled (`ollama pull qwen3.5:4b`)

## Your First Query

Let's start with a simple question:

```bash
ask-ai "What is the capital of France?"
```

You should see a nicely formatted markdown response. That's it - Ask-AI is working!

## Essential Commands

### 1. Query Mode (Default)

The default mode when you don't specify a subcommand:

```bash
# Basic query
ask-ai "Explain quantum computing"

# With think mode (for reasoning models)
ask-ai -t "Solve this step by step"

# With specific model
ask-ai -m mistral-small "Generate a Python function"

# Plain text (no markdown)
ask-ai --plain "List Rust keywords"
```

### 2. Translation

Translate text between languages:

```bash
# English to Portuguese
ask-ai translate en:pt "Hello world"

# Auto-detect source
ask-ai translate :pt "Hello world"

# From stdin
echo "Hello" | ask-ai translate :es
```

### 3. OCR (Text Extraction)

Extract text from images:

```bash
# Extract text from image
ask-ai ocr document.png

# Extract tables
ask-ai ocr --mode table spreadsheet.png

# Extract formulas (LaTeX output)
ask-ai ocr --mode formula equation.png
```

### 4. Summarization

Summarize long text:

```bash
# Summarize text directly
ask-ai summarize "Long text here..."

# From file
cat article.txt | ask-ai summarize

# With style
ask-ai summarize --style academic "Research paper text..."
```

## Working with Pipes

Ask-AI shines when combined with pipes:

```bash
# OCR then summarize
ask-ai ocr document.png | ask-ai summarize

# OCR then translate
ask-ai ocr japanese.png | ask-ai translate ja:pt

# Full pipeline
ask-ai ocr document.png | ask-ai summarize | ask-ai translate :pt
```

## Useful Flags

Here are flags you'll use often:

| Flag | Description | Example |
|------|-------------|---------|
| `-m` | Select model | `ask-ai -m mistral-small "query"` |
| `-t` | Think mode | `ask-ai -t "complex question"` |
| `--plain` | No markdown | `ask-ai --plain "text"` |
| `-d` | Debug mode | `ask-ai -d "query"` |
| `--help` | Show help | `ask-ai --help` |

## List Available Resources

```bash
# List all models
ask-ai --list

# List supported languages
ask-ai translate --list

# Filter languages
ask-ai translate --list pt
```

## Common Workflows

### Document Processing Pipeline

Process a scanned document end-to-end:

```bash
# 1. OCR the scanned document
ask-ai ocr scanned-document.png > extracted.txt

# 2. Summarize the content
cat extracted.txt | ask-ai summarize --style technical > summary.txt

# 3. Translate if needed
cat summary.txt | ask-ai translate :pt > summary-pt.txt
```

### Code Generation

Generate code with the right model:

```bash
# Use code mode for better code output
ask-ai -m qwen3-coder -c "Write a Rust function to parse JSON"

# Or code_with_tools for web research + code
ask-ai -p code_with_tools "Latest Rust async patterns with examples"
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
cat to-translate.txt | ask-ai translate :pt
```

## Quick Tips

1. **Use `-d` for troubleshooting**: If something isn't working, add `-d` to see what's happening

2. **Pipe-friendly**: Most commands accept input from stdin, making them perfect for scripts

3. **Markdown by default**: Output is formatted with markdown. Use `--plain` for raw text

4. **Model selection**: Different tasks benefit from different models:
   - General queries: `qwen3.5-4b` (default)
   - Coding: `qwen3-coder` or `devstral-small-2`
   - Tools: `mistral-small`
   - Summarization: `llama3.2`
   - Reasoning: `lfm` (think mode)

5. **Chaining**: Combine commands for powerful workflows. The output of one command becomes the input of the next

## Next Steps

Now that you know the basics:

- **[Commands Reference](./commands/README.md)** - Detailed documentation for each command
- **[Models](./models.md)** - Learn about available models and when to use them
- **[Pipelines](./pipelines.md)** - Advanced piping and scripting examples

Happy querying! 🎉
