# ocr Command

The `ocr` command extracts text from images using the GLM-OCR model. It supports text recognition, table extraction, formula extraction (LaTeX), and figure/diagram recognition.

## Synopsis

```bash
sprachspiel [GLOBAL OPTIONS] ocr <FILE>...
sprachspiel [GLOBAL OPTIONS] o <FILE>...
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
sprachspiel ocr document.png

# Extract from multiple files
sprachspiel ocr page1.png page2.png page3.png

# Using alias
sprachspiel o letter.jpg
```

### Table Extraction

```bash
# Extract table structure
sprachspiel ocr --mode table spreadsheet.png

# The output preserves table formatting:
# | Column 1 | Column 2 | Column 3 |
# |----------|----------|----------|
# | Data 1   | Data 2   | Data 3   |
```

### Formula Extraction

```bash
# Extract mathematical formulas
sprachspiel ocr --mode formula equation.png

# Output in LaTeX format:
# $$E = mc^2$$
# $$\int_{a}^{b} f(x) dx$$
```

### Figure/Diagram Recognition

```bash
# Extract text from diagrams
sprachspiel ocr --mode figure chart.png
sprachspiel ocr --mode figure diagram.jpg
```

### JSON Output

```bash
# Output as JSON for programmatic use
sprachspiel ocr --json document.png

# Example output:
# {"text": "Extracted content...", "mode": "text"}

# Batch processing with JSON
sprachspiel ocr --json *.png > output.jsonl
```

### Custom Token Limit

```bash
# Increase token limit for complex images
sprachspiel ocr --max-tokens 16384 large-document.png

# Decrease for quick extraction
sprachspiel ocr --max-tokens 4096 simple.png
```
### Logging

```bash
# See processing details
sprachspiel ocr -v document.png
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
sprachspiel ocr japanese.png | sprachspiel translate ja:pt

# OCR → Summarize
sprachspiel ocr report.png | sprachspiel summarize --style technical

# OCR → Summarize → Translate
sprachspiel ocr document.png | sprachspiel summarize | sprachspiel translate :pt

# OCR → Save to file
sprachspiel ocr scanned-document.png > extracted.txt
```

### Batch Processing

```bash
# Process multiple images
for img in *.png; do
    echo "Processing $img..."
    sprachspiel ocr "$img" > "${img%.png}.txt"
done

# With JSON output
for img in scans/*.png; do
    sprachspiel ocr --json "$img" >> output.jsonl
done
```

## Use Cases

### 1. Document Digitization

Convert scanned documents to text:

```bash
sprachspiel ocr scanned-contract.png > contract.txt
```

### 2. Data Extraction

Extract tables from invoices or reports:

```bash
sprachspiel ocr --mode table invoice.png | tee invoice-data.txt
```

### 3. Formula Collection

Extract math equations for LaTeX documents:

```bash
sprachspiel ocr --mode formula math-problems.png > formulas.tex
```

### 4. Multilingual Documents

Process documents in any language:

```bash
# Japanese document
sprachspiel ocr japanese-paper.png | sprachspiel translate ja:en

# Hebrew text
sprachspiel ocr hebrew-document.png | sprachspiel translate he:pt
```

### 5. Research Paper Processing

Extract and analyze research papers:

```bash
sprachspiel ocr research-paper.png | sprachspiel summarize --style academic
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
sprachspiel ocr document.png  # Default text mode

# Structured data (spreadsheets, tables)
sprachspiel ocr --mode table data.png

# Scientific papers
sprachspiel ocr --mode formula equations.png

# Charts and diagrams
sprachspiel ocr --mode figure diagram.png
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
sprachspiel ocr /path/to/exists.png

# Unsupported format
# Convert to PNG first: convert image.bmp image.png
```

## See Also

- [query](./query.md) - General LLM queries
- [translate](./translate.md) - Language translation
- [summarize](./summarize.md) - Text summarization
- [Pipelines](../pipelines.md) - Advanced workflows
- [Tools](../tools.md) - Available tools
