# summarize Command

The `summarize` command creates concise summaries of text with customizable styles and formats.

## Synopsis

```bash
sprachspiel [GLOBAL OPTIONS] summarize [TEXT]
sprachspiel [GLOBAL OPTIONS] sum [TEXT]
```

## Description

Create summaries of long documents, articles, or any text. The summarize command:

- Uses the Llama 3.2 model by default (optimized for summarization)
- Supports multiple output formats (paragraph, bullets, both)
- Offers style presets for different contexts
- Accepts input from arguments or stdin
- Does not use tools (ensures focused summarization)

## Arguments

| Argument | Description |
|----------|-------------|
| `TEXT` | Text to summarize. Reads from stdin if not provided. |

| `-v` | Verbose logging |
| `-vv` | Trace logging |
| `--help` | Show help |

## Subcommand Options

These options are specific to the summarize subcommand:

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--max-length` | `-l` | Maximum words in summary | `300` |
| `--format` | `-f` | Output format: paragraph, bullets, both | `both` |
| `--style` | | Focus style: general, technical, academic, business | `general` |

## Output Formats

| Format | Description | Example Use |
|--------|-------------|-------------|
| `paragraph` | Single paragraph | Quick overviews |
| `bullets` | Bullet points | Presentations, notes |
| `both` | Paragraph + bullets | Complete summaries |

## Style Presets

| Style | Best For | Tone |
|-------|----------|------|
| `general` | Any content | Balanced, accessible |
| `technical` | Documentation, code | Precise, detailed |
| `academic` | Research papers | Formal, analytical |
| `business` | Reports, emails | Professional, actionable |

## Examples

### Basic Summarization

```bash
# Summarize text directly
sprachspiel summarize "Long text to summarize..."

# Using alias
sprachspiel sum "Long text to summarize..."

# From stdin
echo "Long text..." | sprachspiel summarize
```

### Length Control

```bash
# Short summary (100 words)
sprachspiel summarize --max-length 100 "Very long text..."
sprachspiel summarize -l 100 "Very long text..."

# Long summary (500 words)
sprachspiel summarize -l 500 "Detailed document..."
```

### Format Options

```bash
# Paragraph only
sprachspiel summarize --format paragraph "Text..."
sprachspiel summarize -f paragraph "Text..."

# Bullets only
sprachspiel summarize --format bullets "Text..."
sprachspiel summarize -f bullets "Text..."

# Both (default)
sprachspiel summarize --format both "Text..."
```

### Style Presets

```bash
# Technical documentation
sprachspiel summarize --style technical "API documentation..."

# Academic paper
sprachspiel summarize --style academic "Research findings..."

# Business report
sprachspiel summarize --style business "Quarterly results..."

# General (default)
sprachspiel summarize --style general "Article..."
```

### Combining Options

```bash
# Technical style, bullets, 150 words
sprachspiel summarize --style technical --format bullets --max-length 150 "Code docs..."

# Academic, paragraph only, 200 words
sprachspiel summarize --style academic -f paragraph -l 200 "Research paper..."
```

### Model Selection

```bash
# Use default (qwen3.5:4b)
sprachspiel summarize "Text..."

# Use specific model
sprachspiel -m qwen2.5-coder:7b summarize "Text..."

# Use smaller model
sprachspiel -m nanbeige4.1:3b summarize "Text..."
```

### From Files

```bash
# Summarize file content
cat article.txt | sprachspiel summarize

# With options
cat documentation.md | sprachspiel summarize --style technical -l 200

# Summarize code
head -100 src/main.rs | sprachspiel summarize --style technical
```

### Pipelines

```bash
# OCR → Summarize
sprachspiel ocr document.png | sprachspiel summarize

# OCR → Summarize → Translate
sprachspiel ocr japanese.png | sprachspiel summarize | sprachspiel translate ja:pt

# File → Summarize → Save
cat long-article.txt | sprachspiel summarize --style academic > summary.txt
```

## Use Cases

### 1. Document Review

```bash
# Quickly understand a long document
cat contract.txt | sprachspiel summarize -l 150

# Technical documentation review
cat api-docs.md | sprachspiel summarize --style technical
```

### 2. Research Papers

```bash
# Academic paper summary
sprachspiel ocr paper.png | sprachspiel summarize --style academic

# Multiple papers
for paper in *.pdf; do
    pdftotext "$paper" - | sprachspiel summarize --style academic
done
```

### 3. Meeting Notes

```bash
# Summarize meeting transcript
cat meeting-transcript.txt | sprachspiel summarize --style business -f bullets
```

### 4. Email Digest

```bash
# Summarize long email
cat long-email.txt | sprachspiel summarize -l 100 --format paragraph
```

### 5. News Articles

```bash
# News summary
sprachspiel summarize --style general -l 200 "Article text..."
```

## Best Practices

1. **Match style to content** - Use `technical` for code, `academic` for papers
2. **Adjust length** - 150-200 words for quick reads, 300-500 for details
3. **Use bullets for presentations** - Easy to convert to slides
4. **Combine with OCR** - Process scanned documents
5. **Chain with translate** - Summarize then translate

## Comparison of Styles

Given the same input:

```bash
# General - balanced
sprachspiel summarize --style general "API documentation..."

# Technical - focuses on implementation details
sprachspiel summarize --style technical "API documentation..."

# Academic - focuses on methodology and findings
sprachspiel summarize --style academic "Research paper..."

# Business - focuses on action items and implications
sprachspiel summarize --style business "Quarterly report..."
```

## Output Examples

### Paragraph Format

```
This document describes the API endpoints for the user management system. 
It covers authentication, user creation, profile updates, and deletion. 
The API uses REST conventions with JSON responses and requires an 
authentication token for all endpoints except registration.
```

### Bullets Format

```
- User authentication via JWT tokens
- REST API with JSON responses
- Endpoints: create, read, update, delete users
- Token required for all operations except registration
- Rate limiting: 100 requests per minute
```

### Both Format (Default)

```
This document describes the API endpoints for the user management system.

Key points:
- User authentication via JWT tokens
- REST API with JSON responses
- Endpoints: create, read, update, delete users
- Token required for all operations except registration
```

## Tips

### Processing Multiple Files

```bash
# Summarize all text files in directory
for file in *.txt; do
    echo "=== $file ==="
    sprachspiel summarize --style general -l 100 < "$file"
    echo
done
```

### Creating Reading Lists

```bash
# Create summaries of multiple articles
for article in articles/*.txt; do
    echo "# $(basename "$article" .txt)"
    cat "$article" | sprachspiel summarize -l 50 -f bullets
    echo
done > reading-list.md
```

### Code Documentation

```bash
# Summarize function documentation
grep -A 20 "^///" src/*.rs | sprachspiel summarize --style technical
```

## Limitations

- Uses `qwen3.5:4b` model by default
- Requires model to be pulled: `ollama pull qwen3.5:4b`
- Very long texts may be truncated
- Does not use tools (by design)
- Pepe model does not get sarcastic personality (professional mode)

## Error Handling

```bash
# Model not found
ollama pull qwen3.5:4b

# Empty input
# Ensure text is provided or piped correctly
```

## See Also

- [query](./query.md) - General LLM queries
- [translate](./translate.md) - Language translation
- [ocr](./ocr.md) - Image text extraction
- [Pipelines](../pipelines.md) - Advanced workflows
