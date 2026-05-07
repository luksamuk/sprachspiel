# Commands Reference

Sprachspiel provides six main commands for different tasks. Each command is designed to be composable with pipes for building powerful workflows.

## Command Overview

| Command | Alias | Purpose | Default Model |
|---------|-------|---------|---------------|
| `query` | `q` | General LLM queries | lfm |
| `chat` | `c` | Interactive chat with history | lfm |
| `translate` | `t` | Language translation | translate |
| `ocr` | `o` | Image text extraction | glm-ocr |
| `vision` | `v` | Image description and analysis | moondream |
| `summarize` | `sum` | Text summarization | qwen3.5:4b |

## Default Mode

When no subcommand is specified, Sprachspiel defaults to `query` mode:

```bash
# These are equivalent:
sprachspiel "What is Rust?"
sprachspiel query "What is Rust?"
sprachspiel q "What is Rust?"
```

## Common Patterns

All commands share these patterns:

### Input Sources

Commands accept input from arguments **or** stdin:

```bash
# From argument
sprachspiel summarize "Text to summarize"

# From stdin
echo "Text to summarize" | sprachspiel summarize
cat file.txt | sprachspiel summarize
```

### Global Options

Most commands support these options:

| `-v, -vv` | Verbosity: verbose / trace level
| `-h, --help` | Show command help

### Output Redirection

All commands output to stdout, making them pipe-friendly:

```bash
# Save to file
sprachspiel "Query" > output.txt

# Pipe to another command
sprachspiel ocr image.png | sprachspiel summarize

# Chain multiple commands
sprachspiel ocr doc.png | sprachspiel summarize | sprachspiel translate :pt
```

## Command Comparison

| Feature | query | chat | translate | ocr | vision | summarize |
|---------|-------|------|-----------|-----|--------|-----------|
| **Input** | Text query | Interactive | Text + language codes | Image files | Image files | Text |
| **Output** | AI response | Interactive | Translated text | Extracted text | Description | Summary |
| **Model** | Configurable | Configurable | translate (fixed) | glm-ocr (fixed) | moondream | Configurable |
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
sprachspiel ocr paper.png | sprachspiel summarize --style academic

# Then translate to another language
sprachspiel ocr paper.png | sprachspiel summarize --style academic | sprachspiel translate :pt
```

### Document Processing

Process documents end-to-end:

```bash
# Extract text, summarize, translate
cat document.pdf | pdftotext - - | \
    sprachspiel summarize --style technical | \
    sprachspiel translate :es
```

### Batch Processing

Process multiple files:

```bash
# OCR multiple images
for img in *.png; do
    sprachspiel ocr "$img" > "${img%.png}.txt"
done

# Translate all extracted text
for txt in *.txt; do
    sprachspiel translate :pt < "$txt" > "${txt%.txt}-pt.txt"
done
```

## Getting Help

Get help for any command:

```bash
# General help
sprachspiel --help

# Command-specific help
sprachspiel query --help
sprachspiel translate --help
sprachspiel ocr --help
sprachspiel summarize --help

# Man page
man sprachspiel
```

## Next Steps

- Learn about available **[Models](../models.md)**
- Explore **[Tools](../tools.md)** that enhance queries
- See **[Pipelines](../pipelines.md)** for advanced workflows
