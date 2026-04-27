---
name: document-processing
description: Extract and process content from PDF and ePub files with two-phase pipeline, vision analysis for charts/tables/formulas, and structured output.
---

# Document Processing (PDF, ePub)

When asked to process PDF or ePub files:

## 1. Tool Availability Check

First, check available tools using `check_tool_availability`:

**PDF Tools:**
- `pdftotext` - Extract text from PDF
- `pdfinfo` - PDF metadata (pages, title, author)
- `pdftoppm` - Convert PDF pages to images

**ePub Tools:**
- `ebook-convert` - Calibre's ePub converter (full-featured)
- `epub2txt` - Lightweight ePub to text (fallback)

## 2. PDF Processing — Two-Phase Pipeline

### Phase 1: Full Text Extraction (always run first)

Extract text from the entire PDF using `pdftotext`:

```bash
run_command("pdftotext", ["<file.pdf>", "-"])
```

**Evaluate the output:**
- If the text is rich and complete → done, no need for Phase 2
- If pages have very little text, garbled text, or the PDF contains tables, charts, formulas, or diagrams → proceed to Phase 2

### Phase 2: OCR + Vision (for non-text content)

Pages that `pdftotext` couldn't properly extract need further processing. **Choose the right tool based on content type:**

- **OCR (GLM-OCR)** — Best for: **tables**, **formulas**, **scanned text**, **structured text in images**. OCR preserves table layout and mathematical notation accurately.
- **Vision (qwen3.5, llava)** — Best for: **charts**, **graphs**, **diagrams**, **figures**, **visual content requiring interpretation**. Vision describes what it *sees* — colors, trends, layout, relationships.

**Strategy: try OCR first, then vision if needed.** OCR is faster and more precise for structured text content. Vision is better for visual content that requires interpretation beyond what text extraction can provide.

**How to use these tools for PDF pages:**

1. Convert specific pages to images:
   ```bash
   run_command("pdftoppm", ["-png", "-f", "<start>", "-l", "<end>", "-r", "150", "<file.pdf>", "output"])
   ```
2. For tables, formulas, or scanned text — use the **OCR tool** (calls GLM-OCR internally):
   ```
   Use the ocr tool with the page image file(s)
   ```
3. For charts, graphs, diagrams, or visual figures — use the **vision tool** (calls vision model internally):
   ```
   Use the vision tool with the PDF file (use --pages for specific pages)
   ```
4. If OCR results are unsatisfying (e.g., a chart with labels OCR can't interpret), escalate to the vision tool

**Important: tool access depends on context:**
- **In chat mode (with tools)**: Call the `ocr` and `vision` tools directly — these invoke the built-in sub-agents.
  - ⚠️ **Limitation**: The vision tool does NOT support `--pages` parameter. For PDF page selection, use `pdftoppm` to convert specific pages to images first, then pass those images to the vision/ocr tool.
  - Example workflow: `run_command("pdftoppm", ["-png", "-f", "3", "-l", "3", "-r", "150", "doc.pdf", "page"])` → then `vision tool with page-3.png`
- **In CLI mode (standalone)**: Use `ask-ai ocr <image.png>` or `ask-ai vision --pages <range> <file.pdf>`. The `--pages` flag works in CLI mode.
- **In the document subagent**: Only `run_command` is available. Convert pages with `pdftoppm`, then recommend the user run OCR/vision on the resulting images.

**Quick reference:**
| Content type | Primary tool | When to escalate |
|---|---|---|
| Text-heavy pages | pdftotext | — |
| Tables | OCR (`--table`) | Vision if table has visual layout |
| Formulas / equations | OCR (`--formula`) | Vision if formula is in a diagram |
| Scanned pages | OCR (default mode) | Vision if page has mixed content |
| Charts / graphs | Vision | — |
| Diagrams / figures | Vision | — |
| Mixed content | OCR first, then Vision | — |

### Page Selection Strategy

When determining which pages need vision:

1. Run `pdftotext` page by page for large PDFs:
   ```bash
   run_command("pdftotext", ["-f", "1", "-l", "1", "<file.pdf>", "-"])
   ```
2. If a page returns very little text (<100 chars) or garbled output → that page needs vision
3. For short PDFs (<20 pages), just use vision on all pages if content is visual

### Page Range Extraction

Extract specific pages (e.g., pages 10-25):

```bash
run_command("pdftotext", ["-f", "10", "-l", "25", "<file.pdf>", "-"])
```

### Metadata Extraction

```bash
run_command("pdfinfo", ["<file.pdf>"])
```

Returns:
- Title, Author, Subject, Keywords
- Creator, Producer (software used)
- CreationDate, ModDate
- Pages (page count)

### Table of Contents (TOC)

For PDFs with embedded bookmarks, extract from first pages:

```bash
run_command("pdftotext", ["-layout", "-f", "1", "-l", "5", "<file.pdf>", "-"])
```

The first pages often contain the TOC for structured navigation.

### Internal Search

To find a term inside a PDF:

1. Extract full text: `run_command("pdftotext", ["<file.pdf>", "-"])`
2. Search in the result for the term
3. Note page numbers if visible in text

## 3. ePub Processing

### Tool Priority

1. **`ebook-convert`** (Calibre) - Best quality, preserves formatting, full metadata
2. **`epub2txt`** - Lightweight fallback when Calibre not installed

### Full Text Extraction

**With Calibre:**
```bash
run_command("ebook-convert", ["<file.epub>", ".txt"])
```
Creates `<file>.txt` in current directory.

**With epub2txt:**
```bash
run_command("epub2txt", ["<file.epub>", "-"])
```
Outputs to stdout.

### Metadata Extraction

**Method 1: Calibre metadata**
```bash
run_command("ebook-convert", ["<file.epub>", ".txt", "--get-metadata"])
```

**Method 2: Parse OPF file (fallback)**

ePub is a ZIP archive. Extract and parse metadata:

1. Extract: `run_command("unzip", ["-o", "<file.epub>", "-d", "temp_epub"])`
2. Read metadata: Read `temp_epub/OEBPS/content.opf` or `temp_epub/META-INF/container.xml`
3. Look for: `<dc:title>`, `<dc:creator>`, `<dc:publisher>`, `<dc:date>`

### Chapter/TOC Extraction

ePub TOC is typically in `OEBPS/toc.ncx` or `OEBPS/nav.xhtml`.

1. Extract ePub: `run_command("unzip", ["-o", "<file.epub>", "-d", "temp_epub"])`
2. Parse `temp_epub/OEBPS/toc.ncx` for navigation points
3. Each `<navPoint>` has `<text>` (chapter title) and `<content src>` (file path)

### Images in ePub

ePub can contain embedded images (covers, illustrations, manga).

To extract images:
1. Extract ePub: `run_command("unzip", ["-o", "<file.epub>", "-d", "temp_epub"])`
2. Find images: `run_command("find", ["temp_epub", "-name", "*.png", "-o", "-name", "*.jpg"])`

## 4. Installation Instructions

If tools are not installed, provide installation commands:

### Debian/Ubuntu

```bash
sudo apt install poppler-utils calibre

# Optional lightweight ePub fallback:
pip install epub2txt
```

### Arch Linux

```bash
sudo pacman -S poppler calibre

# Optional AUR package:
yay -S epub2txt
```

### Void Linux

```bash
sudo xbps-install -S poppler calibre

# epub2txt is available:
sudo xbps-install -S epub2txt
```

### Alpine Linux

```bash
sudo apk add poppler

# calibre is in testing/edge only:
sudo apk add calibre --repository=http://dl-cdn.alpinelinux.org/alpine/edge/testing

# epub2txt is available:
sudo apk add epub2txt
```

### Fedora

```bash
sudo dnf install poppler-utils calibre

# epub2txt from PyPI:
pip install epub2txt
```

## 5. tools.toml Configuration

After installing tools, create or edit `~/.config/ask-ai/tools.toml`:

```toml
# =============================================================================
# DOCUMENT PROCESSING TOOLS
# =============================================================================

[external.tools.pdftotext]
# Extract text from PDF files to stdout
# USAGE: pdftotext [-f <first>] [-l <last>] <file.pdf> -
# EXAMPLE: pdftotext -f 1 -l 10 document.pdf -  (extract pages 1-10)
enabled = true
timeout = 30
binary = "pdftotext"

[external.tools.pdfinfo]
# Show PDF metadata (pages, size, title, author)
# USAGE: pdfinfo <file.pdf>
enabled = true
timeout = 5
binary = "pdfinfo"

[external.tools.pdftoppm]
# Convert PDF pages to images (PNG, JPEG) for vision analysis
# USAGE: pdftoppm -png [-f <first>] [-l <last>] [-r <dpi>] <file.pdf> <output_prefix>
# NOTE: Output goes to files, not stdout
enabled = true
timeout = 60
binary = "pdftoppm"

[external.tools.ebook-convert]
# Calibre's ePub to text converter (full-featured)
# USAGE: ebook-convert <file.epub> .txt
# NOTE: Creates <file>.txt in current directory
enabled = true
timeout = 60
binary = "ebook-convert"

[external.tools.epub2txt]
# Lightweight ePub to text converter (fallback)
# USAGE: epub2txt <file.epub> -
# NOTE: Outputs to stdout
enabled = true
timeout = 30
binary = "epub2txt"
```

**Note:** `epub2txt` is optional if `calibre` is installed. Use as lightweight fallback.

## 6. Error Handling

### PDF Errors

- **Empty output from pdftotext**: Page likely contains images/charts. Use the vision tool with the PDF file, or run `ask-ai vision --pages <N> <file.pdf>` in CLI mode.
- **Permission denied**: File may be encrypted or DRM-protected.
- **Memory issues**: Large files may need page-by-page processing using `-f` and `-l` flags.
- **Invalid PDF**: File may be corrupted. Try `pdfinfo` first to check validity.
- **pdftoppm not found**: Install poppler-utils (see Section 4).

### ePub Errors

- **ebook-convert not found**: Install `calibre` package.
- **epub2txt not found**: Install via package manager or `pip install epub2txt`.
- **Corrupted ePub**: Try extracting with `unzip` to diagnose.
- **Missing images**: ePub may not contain images, or they're in unsupported format.

### General Errors

- **Tool not in whitelist**: Add tool to `tools.toml` under `[external.tools.<toolname>]`.
- **Tool disabled**: Set `enabled = true` in `tools.toml`.
- **Timeout**: Increase `timeout` value in `tools.toml` for large files.

## 7. Common Patterns

### Extract specific pages from PDF
```bash
pdftotext -f 5 -l 10 -layout document.pdf -
```

### Get page count from PDF
```bash
pdfinfo document.pdf | grep Pages
```

### Analyze PDF pages with vision (tables, charts, formulas)
```bash
# CLI mode:
ask-ai vision --pages 1-5 document.pdf
# Chat mode (with tools):
# Use the vision tool with the PDF file path
# Note: page selection via tool is not yet supported — use pdftoppm to convert specific pages first
```

### Extract text from ePub preserving chapters
```bash
ebook-convert book.epub .txt --txt
```

### Quick ePub content via epub2txt
```bash
epub2txt book.epub -
```

### Search for term in PDF
```bash
pdftotext document.pdf - | grep -n "search term"
```

### Convert specific page to image for vision
```bash
pdftoppm -png -f 3 -l 3 -r 150 document.pdf output
# Then use the vision tool with output-3.png, or in CLI mode:
ask-ai vision output-3.png "Describe the table in this image"
```