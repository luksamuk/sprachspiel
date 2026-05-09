# Pipeline Examples

Sprachspiel commands can be chained together using pipes to create powerful workflows. This page showcases practical pipeline examples.

## Basic Pipe Concept

Sprachspiel commands read from stdin when no argument is provided:

```bash
# Output of first command becomes input of second
cat file.txt | sprach summarize
```

## OCR → Summarize

Extract text from an image and create a summary:

```bash
sprach ocr document.png | sprach summarize
```

**With options:**

```bash
# OCR → Academic summary
sprach ocr research-paper.png | sprach summarize --style academic

# OCR → Technical summary with length limit
sprach ocr manual.png | sprach summarize --style technical -l 150

# OCR → Bullet summary
sprach ocr report.png | sprach summarize -f bullets
```

## OCR → Translate

Extract text from an image and translate it:

```bash
# Japanese document to Portuguese
sprach ocr japanese.png | sprach translate ja:pt

# Chinese to English
sprach ocr chinese.png | sprach translate zh-Hans:en

# Auto-detect to Portuguese
sprach ocr document.png | sprach translate :pt
```

## OCR → Summarize → Translate

Full document processing pipeline:

```bash
sprach ocr document.png | sprach summarize | sprach translate :pt
```

**Breakdown:**
1. Extract text from image
2. Create summary
3. Translate summary to Portuguese

**With specific styles:**

```bash
# Research paper pipeline
sprach ocr paper.png | sprach summarize --style academic | sprach translate :pt

# Technical manual pipeline
sprach ocr manual.png | sprach summarize --style technical | sprach translate :es

# Business report pipeline
sprach ocr report.png | sprach summarize --style business | sprach translate :fr
```

## File → Summarize

Process text files:

```bash
# Summarize text file
cat article.txt | sprach summarize

# With specific style
cat documentation.md | sprach summarize --style technical

# Academic paper
pdftotext paper.pdf - | sprach summarize --style academic
```

## File → Summarize → Translate

Process files in other languages:

```bash
# English to Portuguese
cat report.txt | sprach summarize | sprach translate :pt

# Technical docs to Spanish
cat api-docs.md | sprach summarize --style technical | sprach translate :es
```

## PDF Processing

Process PDF documents (requires `pdftotext`):

```bash
# PDF → Summary
pdftotext document.pdf - | sprach summarize

# PDF → Summary → Translate
pdftotext document.pdf - | sprach summarize | sprach translate :pt

# PDF → OCR → Summary
pdftotext scanned.pdf - | sprach summarize --style academic
```

## Batch Processing

Process multiple files:

```bash
# OCR all images and summarize
for img in *.png; do
    echo "=== $img ==="
    sprach ocr "$img" | sprach summarize
    echo
done

# OCR → Translate all images
for img in scans/*.png; do
    out="${img%.png}-pt.txt"
    sprach ocr "$img" | sprach translate :pt > "$out"
    echo "Created $out"
done
```

## Advanced Workflows

### Document Translation

Translate documents while preserving structure:

```bash
# Complete workflow for scanned documents
sprach ocr scanned-ja.png | \
    sprach translate ja:pt | \
    sprach summarize -l 200 | \
    tee translated-summary.txt
```

### Research Paper Analysis

```bash
# Extract and analyze research
sprach ocr paper.png | \
    sprach summarize --style academic -l 300 | \
    sprach translate :pt | \
    tee analysis-pt.txt
```

### Code Documentation

```bash
# Generate documentation from code
head -50 src/main.rs | \
    sprach summarize --style technical | \
    sprach translate :pt
```

### Multi-Document Processing

```bash
# Combine multiple documents
for doc in chapter*.txt; do
    cat "$doc" | sprach summarize -l 100
done | sprach summarize -l 300 > book-summary.txt
```

## Common Patterns

### 1. OCR → Process → Save

```bash
sprach ocr document.png | sprach summarize > output.txt
```

### 2. File → Translate → Save

```bash
cat document.txt | sprach translate :pt > translated.txt
```

### 3. Query → Process → Query

```bash
sprach "Find information" | sprach summarize | sprach "Analyze this"
```

### 4. Chain with Unix Tools

```bash
# Sort results
sprach ocr doc.png | sort | sprach summarize

# Filter content
sprach ocr doc.png | grep "important" | sprach translate :pt

# Word count
sprach summarize text.txt | wc -w
```

## Error Handling

Handle errors in pipelines:

```bash
# Continue on error
for img in *.png; do
    sprach ocr "$img" 2>/dev/null | sprach summarize || echo "Failed: $img"
done

# Stop on first error
set -e
sprach ocr doc.png | sprach summarize
```

## Performance Tips

1. **Process in batches** - Group similar operations
2. **Use specific models** - Match model to task
3. **Limit token usage** - Use `--max-tokens` for OCR
4. **Save intermediate results** - Use `tee` for debugging

```bash
# Save intermediate results
sprach ocr doc.png | tee extracted.txt | sprach summarize | tee summary.txt | sprach translate :pt
```

## Debugging Pipelines

Add debug output:

```bash
# Show each step
sprach ocr doc.png | tee /dev/tty | sprach summarize

# Debug with timing
time sprach ocr doc.png | time sprach summarize

# Full debug
sprach ocr -v doc.png 2> debug.log | sprach summarize -v 2> summary.log
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

sprach ocr "$IMAGE" | \
    sprach summarize | \
    sprach translate ":$LANG"
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
