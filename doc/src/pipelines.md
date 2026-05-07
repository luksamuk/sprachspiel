# Pipeline Examples

Sprachspiel commands can be chained together using pipes to create powerful workflows. This page showcases practical pipeline examples.

## Basic Pipe Concept

Sprachspiel commands read from stdin when no argument is provided:

```bash
# Output of first command becomes input of second
cat file.txt | sprachspiel summarize
```

## OCR → Summarize

Extract text from an image and create a summary:

```bash
sprachspiel ocr document.png | sprachspiel summarize
```

**With options:**

```bash
# OCR → Academic summary
sprachspiel ocr research-paper.png | sprachspiel summarize --style academic

# OCR → Technical summary with length limit
sprachspiel ocr manual.png | sprachspiel summarize --style technical -l 150

# OCR → Bullet summary
sprachspiel ocr report.png | sprachspiel summarize -f bullets
```

## OCR → Translate

Extract text from an image and translate it:

```bash
# Japanese document to Portuguese
sprachspiel ocr japanese.png | sprachspiel translate ja:pt

# Chinese to English
sprachspiel ocr chinese.png | sprachspiel translate zh-Hans:en

# Auto-detect to Portuguese
sprachspiel ocr document.png | sprachspiel translate :pt
```

## OCR → Summarize → Translate

Full document processing pipeline:

```bash
sprachspiel ocr document.png | sprachspiel summarize | sprachspiel translate :pt
```

**Breakdown:**
1. Extract text from image
2. Create summary
3. Translate summary to Portuguese

**With specific styles:**

```bash
# Research paper pipeline
sprachspiel ocr paper.png | sprachspiel summarize --style academic | sprachspiel translate :pt

# Technical manual pipeline
sprachspiel ocr manual.png | sprachspiel summarize --style technical | sprachspiel translate :es

# Business report pipeline
sprachspiel ocr report.png | sprachspiel summarize --style business | sprachspiel translate :fr
```

## File → Summarize

Process text files:

```bash
# Summarize text file
cat article.txt | sprachspiel summarize

# With specific style
cat documentation.md | sprachspiel summarize --style technical

# Academic paper
pdftotext paper.pdf - | sprachspiel summarize --style academic
```

## File → Summarize → Translate

Process files in other languages:

```bash
# English to Portuguese
cat report.txt | sprachspiel summarize | sprachspiel translate :pt

# Technical docs to Spanish
cat api-docs.md | sprachspiel summarize --style technical | sprachspiel translate :es
```

## PDF Processing

Process PDF documents (requires `pdftotext`):

```bash
# PDF → Summary
pdftotext document.pdf - | sprachspiel summarize

# PDF → Summary → Translate
pdftotext document.pdf - | sprachspiel summarize | sprachspiel translate :pt

# PDF → OCR → Summary
pdftotext scanned.pdf - | sprachspiel summarize --style academic
```

## Batch Processing

Process multiple files:

```bash
# OCR all images and summarize
for img in *.png; do
    echo "=== $img ==="
    sprachspiel ocr "$img" | sprachspiel summarize
    echo
done

# OCR → Translate all images
for img in scans/*.png; do
    out="${img%.png}-pt.txt"
    sprachspiel ocr "$img" | sprachspiel translate :pt > "$out"
    echo "Created $out"
done
```

## Advanced Workflows

### Document Translation

Translate documents while preserving structure:

```bash
# Complete workflow for scanned documents
sprachspiel ocr scanned-ja.png | \
    sprachspiel translate ja:pt | \
    sprachspiel summarize -l 200 | \
    tee translated-summary.txt
```

### Research Paper Analysis

```bash
# Extract and analyze research
sprachspiel ocr paper.png | \
    sprachspiel summarize --style academic -l 300 | \
    sprachspiel translate :pt | \
    tee analysis-pt.txt
```

### Code Documentation

```bash
# Generate documentation from code
head -50 src/main.rs | \
    sprachspiel summarize --style technical | \
    sprachspiel translate :pt
```

### Multi-Document Processing

```bash
# Combine multiple documents
for doc in chapter*.txt; do
    cat "$doc" | sprachspiel summarize -l 100
done | sprachspiel summarize -l 300 > book-summary.txt
```

## Common Patterns

### 1. OCR → Process → Save

```bash
sprachspiel ocr document.png | sprachspiel summarize > output.txt
```

### 2. File → Translate → Save

```bash
cat document.txt | sprachspiel translate :pt > translated.txt
```

### 3. Query → Process → Query

```bash
sprachspiel "Find information" | sprachspiel summarize | sprachspiel "Analyze this"
```

### 4. Chain with Unix Tools

```bash
# Sort results
sprachspiel ocr doc.png | sort | sprachspiel summarize

# Filter content
sprachspiel ocr doc.png | grep "important" | sprachspiel translate :pt

# Word count
sprachspiel summarize text.txt | wc -w
```

## Error Handling

Handle errors in pipelines:

```bash
# Continue on error
for img in *.png; do
    sprachspiel ocr "$img" 2>/dev/null | sprachspiel summarize || echo "Failed: $img"
done

# Stop on first error
set -e
sprachspiel ocr doc.png | sprachspiel summarize
```

## Performance Tips

1. **Process in batches** - Group similar operations
2. **Use specific models** - Match model to task
3. **Limit token usage** - Use `--max-tokens` for OCR
4. **Save intermediate results** - Use `tee` for debugging

```bash
# Save intermediate results
sprachspiel ocr doc.png | tee extracted.txt | sprachspiel summarize | tee summary.txt | sprachspiel translate :pt
```

## Debugging Pipelines

Add debug output:

```bash
# Show each step
sprachspiel ocr doc.png | tee /dev/tty | sprachspiel summarize

# Debug with timing
time sprachspiel ocr doc.png | time sprachspiel summarize

# Full debug
sprachspiel ocr -v doc.png 2> debug.log | sprachspiel summarize -v 2> summary.log
```

## Creating Scripts

Save common pipelines as scripts:

```bash
#!/bin/bash
# process-document.sh - OCR → Summarize → Translate

if [ $# -lt 2 ]; then
    echo "Usage: $0 <image> <target-lang>"
    exit 1
fi

IMAGE=$1
LANG=$2

sprachspiel ocr "$IMAGE" | \
    sprachspiel summarize | \
    sprachspiel translate ":$LANG"
```

Usage:

```bash
chmod +x process-document.sh
./process-document.sh document.png pt
```

## Pipeline Visualization

```mermaid
graph LR
    A[Image] --> B[OCR]
    B --> C[Text]
    C --> D[Summarize]
    D --> E[Summary]
    E --> F[Translate]
    F --> G[Translated Summary]
```

## Tips

1. **Start simple** - Test each step individually
2. **Add tee** - Save intermediate results
3. **Check errors** - Handle failures gracefully
4. **Document** - Comment complex pipelines
5. **Optimize** - Profile and improve speed

## See Also

- [ocr](./commands/ocr.md) - Image text extraction
- [translate](./commands/translate.md) - Language translation
- [summarize](./commands/summarize.md) - Text summarization
- [query](./commands/query.md) - General queries
