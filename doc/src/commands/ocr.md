# ocr Command

The `ocr` command extracts text from images using the GLM-OCR model. It supports text recognition, table extraction, formula extraction (LaTeX), and figure/diagram recognition.

## Synopsis

```bash
ask-ai [GLOBAL OPTIONS] ocr <FILE>...
ask-ai [GLOBAL OPTIONS] o <FILE>...
```

## Description

OCR (Optical Character Recognition) extracts text from images. GLM-OCR is a multimodal model capable of understanding:

- **Text** - General document text
- **Tables** - Structured tabular data
- **Formulas** - Mathematical equations (outputs LaTeX)
- **Figures** - Diagrams, charts, and illustrations

## Arguments

| Argument | Description |
|----------|-------------|
| `FILE` | One or more image files to process |

| `-v` | Verbose logging |
| `-vv` | Trace logging |
| `--help` | Show help |

## Subcommand Options

These options are specific to the ocr subcommand:

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--mode` | `-m` | Extraction mode: text, table, figure, formula | `text` |
| `--json` | | Output as JSON (one object per line) | disabled |
| `--max-tokens` | | Maximum tokens per image | `8192` |

## Extraction Modes

| Mode | Description | Best For |
|------|-------------|----------|
| `text` | General text recognition | Documents, articles, letters |
| `table` | Table structure extraction | Spreadsheets, invoices, data tables |
| `figure` | Figure and diagram recognition | Charts, graphs, diagrams |
| `formula` | Mathematical formula extraction | Equations, formulas (outputs LaTeX) |

## Supported Image Formats

- **PNG** (`.png`) - Recommended for best quality
- **JPEG/JPG** (`.jpg`, `.jpeg`)
- **WebP** (`.webp`)
- **GIF** (`.gif`) - First frame only

## Examples

### Basic Text Extraction

```bash
# Extract text from document
ask-ai ocr document.png

# Extract from multiple files
ask-ai ocr page1.png page2.png page3.png

# Using alias
ask-ai o letter.jpg
```

### Table Extraction

```bash
# Extract table structure
ask-ai ocr --mode table spreadsheet.png

# The output preserves table formatting:
# | Column 1 | Column 2 | Column 3 |
# |----------|----------|----------|
# | Data 1   | Data 2   | Data 3   |
```

### Formula Extraction

```bash
# Extract mathematical formulas
ask-ai ocr --mode formula equation.png

# Output in LaTeX format:
# $$E = mc^2$$
# $$\int_{a}^{b} f(x) dx$$
```

### Figure/Diagram Recognition

```bash
# Extract text from diagrams
ask-ai ocr --mode figure chart.png
ask-ai ocr --mode figure diagram.jpg
```

### JSON Output

```bash
# Output as JSON for programmatic use
ask-ai ocr --json document.png

# Example output:
# {"text": "Extracted content...", "mode": "text"}

# Batch processing with JSON
ask-ai ocr --json *.png > output.jsonl
```

### Custom Token Limit

```bash
# Increase token limit for complex images
ask-ai ocr --max-tokens 16384 large-document.png

# Decrease for quick extraction
ask-ai ocr --max-tokens 4096 simple.png
```
### Logging

```bash
# See processing details
ask-ai ocr -v document.png
#
# Shows:
# - Model being used
# - Image processing info
# - Token usage
```

## Pipelines

OCR works great in pipelines:

```bash
# OCR → Translate
ask-ai ocr japanese.png | ask-ai translate ja:pt

# OCR → Summarize
ask-ai ocr report.png | ask-ai summarize --style technical

# OCR → Summarize → Translate
ask-ai ocr document.png | ask-ai summarize | ask-ai translate :pt

# OCR → Save to file
ask-ai ocr scanned-document.png > extracted.txt
```

### Batch Processing

```bash
# Process multiple images
for img in *.png; do
    echo "Processing $img..."
    ask-ai ocr "$img" > "${img%.png}.txt"
done

# With JSON output
for img in scans/*.png; do
    ask-ai ocr --json "$img" >> output.jsonl
done
```

## Use Cases

### 1. Document Digitization

Convert scanned documents to text:

```bash
ask-ai ocr scanned-contract.png > contract.txt
```

### 2. Data Extraction

Extract tables from invoices or reports:

```bash
ask-ai ocr --mode table invoice.png | tee invoice-data.txt
```

### 3. Formula Collection

Extract math equations for LaTeX documents:

```bash
ask-ai ocr --mode formula math-problems.png > formulas.tex
```

### 4. Multilingual Documents

Process documents in any language:

```bash
# Japanese document
ask-ai ocr japanese-paper.png | ask-ai translate ja:en

# Hebrew text
ask-ai ocr hebrew-document.png | ask-ai translate he:pt
```

### 5. Research Paper Processing

Extract and analyze research papers:

```bash
ask-ai ocr research-paper.png | ask-ai summarize --style academic
```

## Best Practices

1. **Choose the right mode** - Use `--mode table` for structured data
2. **High resolution images** - Better quality = better results
3. **Good lighting** - Avoid shadows and glare
4. **Straight alignment** - Deskew images if possible
5. **Clear text** - Avoid handwritten or heavily stylized fonts

## Tips for Better Results

### Image Quality

- Use PNG format when possible (lossless)
- Ensure minimum 150 DPI for documents
- Crop to remove unnecessary borders
- Convert color images to grayscale if text is hard to read

### Mode Selection

```bash
# Documents with mixed content
ask-ai ocr document.png  # Default text mode

# Structured data (spreadsheets, tables)
ask-ai ocr --mode table data.png

# Scientific papers
ask-ai ocr --mode formula equations.png

# Charts and diagrams
ask-ai ocr --mode figure diagram.png
```

## Limitations

- Requires `glm-ocr:bf16` model
- Handwriting recognition may be limited
- Very small text might not be recognized
- Complex layouts may need multiple passes
- Maximum image size depends on available memory

## Error Handling

Common errors and solutions:

```bash
# Model not found
ollama pull glm-ocr:bf16

# File not found
ask-ai ocr /path/to/exists.png

# Unsupported format
# Convert to PNG first: convert image.bmp image.png
```

## See Also

- [query](./query.md) - General LLM queries
- [translate](./translate.md) - Language translation
- [summarize](./summarize.md) - Text summarization
- [Pipelines](../pipelines.md) - Advanced workflows
- [Tools](../tools.md) - Available tools
