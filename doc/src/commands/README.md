# Commands Reference

Sprachspiel provides six main commands for different tasks. Each command is designed to be composable with pipes for building powerful workflows.

## Command Overview

| Command | Purpose | Default Model |
|---------|---------|---------------|
| `query` | General LLM queries | qwen3.5-4b |
| `chat` | Interactive chat with history | qwen3.5-4b |
| `translate` | Language translation | translategemma-4b |
| `ocr` | Image text extraction | glm-ocr |
| `vision` | Image description and analysis | qwen3.5-4b |
| `summarize` | Text summarization | qwen3.5-4b |

## Default Mode

When no subcommand is specified, Sprachspiel defaults to `query` mode:

```bash
# These are equivalent:
sprach "What is Rust?"
sprach query "What is Rust?"
```

## Common Patterns

All commands share these patterns:

### Input Sources

Commands accept input from arguments **or** stdin:

```bash
# From argument
sprach summarize "Text to summarize"

# From stdin
echo "Text to summarize" | sprach summarize
cat file.txt | sprach summarize
```

### Global Options

Most commands support these options:

| Option | Description |
|--------|-------------|
| `-v, -vv` | Verbosity: verbose / trace level |
| `-h, --help` | Show command help |

### Output Redirection

All commands output to stdout, making them pipe-friendly:

```bash
# Save to file
sprach "Query" > output.txt

# Pipe to another command
sprach ocr image.png | sprach summarize

# Chain multiple commands
sprach ocr doc.png | sprach summarize | sprach translate :pt
```

## Command Comparison

| Feature | query | chat | translate | ocr | vision | summarize |
|---------|-------|------|-----------|-----|--------|-----------|
| **Input** | Text query | Interactive | Text + language codes | Image files | Image files | Text |
| **Output** | AI response | Interactive | Translated text | Extracted text | Description | Summary |
| **Tools** | Yes (auto) | Yes (auto) | No | No | No | No |
| **Stdin** | Yes | No | Yes | No | No | Yes |
| **History** | No | Yes | No | No | No | No |

## Detailed Documentation

For complete documentation on each command:

- **[query](./query.md)** - General purpose queries with tool support
- **[chat](./chat.md)** - Interactive chat with conversation history
- **[translate](./translate.md)** - Language translation with 50+ languages
- **[ocr](./ocr.md)** - Image text, table, formula, and figure extraction
- **[vision](./vision.md)** - Image description and analysis
- **[summarize](./summarize.md)** - Text summarization with multiple styles

## Workflow Examples

### Research Workflow

Extract and summarize research papers:

```bash
# OCR a scanned paper, summarize in academic style
sprach ocr paper.png | sprach summarize --style academic

# Then translate to another language
sprach ocr paper.png | sprach summarize --style academic | sprach translate :pt
```

### Document Processing

Process documents end-to-end:

```bash
# Extract text, summarize, translate
cat document.pdf | pdftotext - - | \
    sprach summarize --style technical | \
    sprach translate :es
```

### Batch Processing

Process multiple files:

```bash
# OCR multiple images
for img in *.png; do
    sprach ocr "$img" > "${img%.png}.txt"
done

# Translate all extracted text
for txt in *.txt; do
    sprach translate :pt < "$txt" > "${txt%.txt}-pt.txt"
done
```

## Getting Help

Get help for any command:

```bash
# General help
sprach --help

# Command-specific help
sprach query --help
sprach translate --help
sprach ocr --help
sprach summarize --help

# Man page
man sprach
```

## Next Steps

- Learn about available **[Models](../models.md)**
- Explore **[Tools](../tools.md)** that enhance queries
- See **[Pipelines](../pipelines.md)** for advanced workflows
