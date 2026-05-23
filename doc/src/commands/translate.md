# translate Command

The `translate` command translates text between 50+ languages using the TranslateGemma model.

## Synopsis

```bash
sprach [GLOBAL OPTIONS] translate <LANGUAGE> [TEXT]
```

## Description

Translate text between languages with automatic source language detection. The command uses the TranslateGemma model (12B parameters) optimized for translation tasks.

## Arguments

| Argument | Description |
|----------|-------------|
| `LANGUAGE` | Language pair in format `[source:]target` or just `target` for auto-detect |
| `TEXT` | Text to translate. Reads from stdin if not provided. |

| Option | Description |
|--------|-------------|
| `-v` | Verbose logging |
| `-vv` | Trace logging |
| `--help` | Show help |

## Subcommand Options

These options are specific to the translate subcommand:

| Option | Short | Description |
|--------|-------|-------------|
| `--prompt` | `-p` | Translation style: formal, casual, technical, literary |
| `--list` | | List supported languages (optionally filter) |

## Language Codes

### Common Codes

| Code | Language |
|------|----------|
| `pt` | Portuguese |
| `pt-BR` / `br` / `brazil` | Portuguese (Brazil) |
| `en` / `english` | English |
| `es` / `spanish` | Spanish |
| `fr` / `french` | French |
| `de` / `german` | German |
| `it` / `italian` | Italian |
| `ja` / `japanese` | Japanese |
| `zh-Hans` / `zh-cn` | Chinese Simplified |
| `zh-Hant` / `zh-tw` | Chinese Traditional |
| `ko` / `korean` | Korean |
| `ru` / `russian` | Russian |
| `ar` / `arabic` | Arabic |
| `hi` / `hindi` | Hindi |
| `he` / `hebrew` | Hebrew |

### Supported Formats

```bash
# Explicit source and target
sprach translate en:pt "Hello"

# Auto-detect source (colon required)
sprach translate :pt "Hello"

# Just target language (auto-detect implied)
sprach translate pt "Hello"
```

## Examples

### Basic Translation

```bash
# English to Portuguese
sprach translate en:pt "Hello world"
# Output: Olá mundo

# Spanish to English
sprach translate es:en "Hola mundo"
# Output: Hello world

# French to Portuguese
sprach translate fr:pt "Bonjour le monde"
# Output: Olá mundo
```

### Auto-Detect Source

```bash
# Auto-detect to Portuguese
sprach translate :pt "Hello world"
sprach translate pt "Hello world"

# Works with any language
sprach translate :en "こんにちは"
sprach translate :pt "שלום"
```

### From Stdin

```bash
# Translate file content
cat document.txt | sprach translate :pt

# Translate command output
echo "Hello world" | sprach translate :es

# Chain with OCR
sprach ocr japanese.png | sprach translate ja:pt
```

### Translation Styles

Use `-p` or `--prompt` for different styles:

```bash
# Formal style
sprach translate en:pt -p formal "Hey, what's up?"

# Casual style
sprach translate en:pt -p casual "Greetings and salutations"

# Technical style
sprach translate en:pt -p technical "API endpoint response"

# Literary style
sprach translate en:pt -p literary "It was the best of times"
```

### List Languages

```bash
# List all supported languages
sprach translate --list

# Filter by substring
sprach translate --list portuguese
sprach translate --list pt
sprach translate --list spanish
```

### Plain Text Output

```bash
# No markdown formatting
sprach --plain translate :pt "Hello **world**"
# Output: Olá **mundo**
```
### Logging

```bash
# See translation process
sprach -v translate en:pt "Test"
# Shows model configuration and processing
```

## Language Pairs

Common translation pairs:

| From | To | Example |
|------|-----|---------|
| English | Portuguese | `en:pt` |
| English | Spanish | `en:es` |
| Portuguese | English | `pt:en` |
| Spanish | Portuguese | `es:pt` |
| Japanese | English | `ja:en` |
| Hebrew | Portuguese | `he:pt` |
| Chinese | Portuguese | `zh-Hans:pt` |
| Arabic | English | `ar:en` |
| Hindi | English | `hi:en` |
| Russian | Portuguese | `ru:pt` |

## Pipelines

Translation works great in pipelines:

```bash
# OCR → Translate
sprach ocr japanese.png | sprach translate ja:pt

# OCR → Summarize → Translate
sprach ocr document.png | sprach summarize | sprach translate :pt

# File → Translate → Save
cat english.txt | sprach translate :pt > portuguese.txt
```

## Best Practices

1. **Use auto-detect when unsure** - The model is good at detecting languages
2. **Specify style for context** - Technical texts translate differently than casual
3. **Chain with OCR** - Great for processing scanned documents
4. **Check with --list** - Verify your language code is supported

## Limitations

- Requires `translategemma:4b` model
- Best for single sentences or short paragraphs
- Very long texts may be truncated

## See Also

- [query](./query.md) - General LLM queries
- [ocr](./ocr.md) - Image text extraction
- [summarize](./summarize.md) - Text summarization
- [Pipelines](../pipelines.md) - Advanced workflows
