---
name: document-processing
description: Extract and process content from PDF and ePub files with metadata extraction, TOC, and structured output.
---

# Document Processing (PDF, ePub)

When asked to process PDF or ePub files:

## 1. Tool Availability Check

First, check available tools using `check_tool_availability`:

**PDF Tools:**
- `pdftotext` - Extract text from PDF
- `pdfinfo` - PDF metadata (pages, title, author)
- `pdftoppm` - Convert PDF pages to images
- `tesseract` - OCR for scanned documents

**ePub Tools:**
- `ebook-convert` - Calibre's ePub converter (full-featured)
- `epub2txt` - Lightweight ePub to text (fallback)

## 2. PDF Processing

### Full Text Extraction

```bash
run_command("pdftotext", ["<file.pdf>", "-"])
```

Outputs to stdout. Parse and analyze the text.

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

### OCR Fallback (for scanned PDFs)

If `pdftotext` returns empty or garbled text, the PDF is likely scanned.

1. Convert pages to images:
   ```bash
   run_command("pdftoppm", ["-png", "<file.pdf>", "output"])
   ```
2. OCR each image:
   ```bash
   run_command("tesseract", ["output-1.png", "stdout"])
   ```
3. Combine OCR results for all pages

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

To extract and OCR images:
1. Extract ePub: `run_command("unzip", ["-o", "<file.epub>", "-d", "temp_epub"])`
2. Find images: `run_command("find", ["temp_epub", "-name", "*.png", "-o", "-name", "*.jpg"])`
3. OCR with tesseract: `run_command("tesseract", ["<image>", "stdout"])`

## 4. Installation Instructions

If tools are not installed, provide installation commands:

### Debian/Ubuntu

```bash
sudo apt install poppler-utils calibre tesseract-ocr

# Optional lightweight ePub fallback:
pip install epub2txt
```

### Arch Linux

```bash
sudo pacman -S poppler calibre tesseract

# Optional AUR package:
yay -S epub2txt
```

### Void Linux

```bash
sudo xbps-install -S poppler calibre tesseract

# epub2txt is available:
sudo xbps-install -S epub2txt
```

### Alpine Linux

```bash
sudo apk add poppler tesseract-ocr

# calibre is in testing/edge only:
sudo apk add calibre --repository=http://dl-cdn.alpinelinux.org/alpine/edge/testing

# epub2txt is available:
sudo apk add epub2txt
```

### Fedora

```bash
sudo dnf install poppler-utils calibre tesseract

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
# Convert PDF pages to images (PNG, JPEG) for OCR
# USAGE: pdftoppm -png <file.pdf> <output_prefix>
# NOTE: Output goes to files, not stdout
enabled = true
timeout = 60
binary = "pdftoppm"

[external.tools.tesseract]
# OCR engine for scanned documents
# USAGE: tesseract <image.png> stdout
enabled = true
timeout = 60
binary = "tesseract"

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

- **Empty output from pdftotext**: File is likely scanned. Use OCR with `tesseract`.
- **Permission denied**: File may be encrypted or DRM-protected.
- **Memory issues**: Large files may need page-by-page processing using `-f` and `-l` flags.
- **Invalid PDF**: File may be corrupted. Try `pdfinfo` first to check validity.

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

### OCR a scanned PDF page
```bash
pdftoppm -png -f 1 -l 1 document.pdf output
tesseract output-1.png stdout
```