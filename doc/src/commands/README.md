# Commands Reference

Ask-AI provides four main commands for different tasks. Each command is designed to be composable with pipes for building powerful workflows.

## Command Overview

| Command | Alias | Purpose | Default Model |
|---------|-------|---------|---------------|
| `query` | `q` | General LLM queries | lfm |
| `translate` | `t` | Language translation | translate |
| `ocr` | `o` | Image text extraction | glm-ocr |
| `summarize` | `sum` | Text summarization | llama3.2 |

## Default Mode

When no subcommand is specified, Ask-AI defaults to `query` mode:

```bash
# These are equivalent:
ask-ai "What is Rust?"
ask-ai query "What is Rust?"
ask-ai q "What is Rust?"
```

## Common Patterns

All commands share these patterns:

### Input Sources

Commands accept input from arguments **or** stdin:

```bash
# From argument
ask-ai summarize "Text to summarize"

# From stdin
echo "Text to summarize" | ask-ai summarize
cat file.txt | ask-ai summarize
```

### Global Options

Most commands support these options:

| Option | Description |
|--------|-------------|
| `-m, --model` | Select model preset |
| `--plain` | Plain text output (no markdown) |
| `-d, --debug` | Debug mode with logging |
| `-h, --help` | Show command help |

### Output Redirection

All commands output to stdout, making them pipe-friendly:

```bash
# Save to file
ask-ai "Query" > output.txt

# Pipe to another command
ask-ai ocr image.png | ask-ai summarize

# Chain multiple commands
ask-ai ocr doc.png | ask-ai summarize | ask-ai translate :pt
```

## Command Comparison

| Feature | query | translate | ocr | summarize |
|---------|-------|-----------|-----|-----------|
| **Input** | Text query | Text + language codes | Image files | Text |
| **Output** | AI response | Translated text | Extracted text | Summary |
| **Model** | Configurable | translate (fixed) | glm-ocr (fixed) | Configurable |
| **Tools** | Yes (auto) | No | No | No |
| **Stdin** | Yes | Yes | No | Yes |

## Detailed Documentation

For complete documentation on each command:

- **[query](./query.md)** - General purpose queries with tool support
- **[translate](./translate.md)** - Language translation with 50+ languages
- **[ocr](./ocr.md)** - Image text, table, formula, and figure extraction
- **[summarize](./summarize.md)** - Text summarization with multiple styles

## Workflow Examples

### Research Workflow

Extract and summarize research papers:

```bash
# OCR a scanned paper, summarize in academic style
ask-ai ocr paper.png | ask-ai summarize --style academic

# Then translate to another language
ask-ai ocr paper.png | ask-ai summarize --style academic | ask-ai translate :pt
```

### Document Processing

Process documents end-to-end:

```bash
# Extract text, summarize, translate
cat document.pdf | pdftotext - - | \
    ask-ai summarize --style technical | \
    ask-ai translate :es
```

### Batch Processing

Process multiple files:

```bash
# OCR multiple images
for img in *.png; do
    ask-ai ocr "$img" > "${img%.png}.txt"
done

# Translate all extracted text
for txt in *.txt; do
    ask-ai translate :pt < "$txt" > "${txt%.txt}-pt.txt"
done
```

## Getting Help

Get help for any command:

```bash
# General help
ask-ai --help

# Command-specific help
ask-ai query --help
ask-ai translate --help
ask-ai ocr --help
ask-ai summarize --help

# Man page
man ask-ai
```

## Next Steps

- Learn about available **[Models](../models.md)**
- Explore **[Tools](../tools.md)** that enhance queries
- See **[Pipelines](../pipelines.md)** for advanced workflows
