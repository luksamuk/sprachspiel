# Pipeline Examples

Ask-AI commands can be chained together using pipes to create powerful workflows. This page showcases practical pipeline examples.

## Basic Pipe Concept

Ask-AI commands read from stdin when no argument is provided:

```bash
# Output of first command becomes input of second
cat file.txt | ask-ai summarize
```

## OCR → Summarize

Extract text from an image and create a summary:

```bash
ask-ai ocr document.png | ask-ai summarize
```

**With options:**

```bash
# OCR → Academic summary
ask-ai ocr research-paper.png | ask-ai summarize --style academic

# OCR → Technical summary with length limit
ask-ai ocr manual.png | ask-ai summarize --style technical -l 150

# OCR → Bullet summary
ask-ai ocr report.png | ask-ai summarize -f bullets
```

## OCR → Translate

Extract text from an image and translate it:

```bash
# Japanese document to Portuguese
ask-ai ocr japanese.png | ask-ai translate ja:pt

# Chinese to English
ask-ai ocr chinese.png | ask-ai translate zh-Hans:en

# Auto-detect to Portuguese
ask-ai ocr document.png | ask-ai translate :pt
```

## OCR → Summarize → Translate

Full document processing pipeline:

```bash
ask-ai ocr document.png | ask-ai summarize | ask-ai translate :pt
```

**Breakdown:**
1. Extract text from image
2. Create summary
3. Translate summary to Portuguese

**With specific styles:**

```bash
# Research paper pipeline
ask-ai ocr paper.png | ask-ai summarize --style academic | ask-ai translate :pt

# Technical manual pipeline
ask-ai ocr manual.png | ask-ai summarize --style technical | ask-ai translate :es

# Business report pipeline
ask-ai ocr report.png | ask-ai summarize --style business | ask-ai translate :fr
```

## File → Summarize

Process text files:

```bash
# Summarize text file
cat article.txt | ask-ai summarize

# With specific style
cat documentation.md | ask-ai summarize --style technical

# Academic paper
pdftotext paper.pdf - | ask-ai summarize --style academic
```

## File → Summarize → Translate

Process files in other languages:

```bash
# English to Portuguese
cat report.txt | ask-ai summarize | ask-ai translate :pt

# Technical docs to Spanish
cat api-docs.md | ask-ai summarize --style technical | ask-ai translate :es
```

## PDF Processing

Process PDF documents (requires `pdftotext`):

```bash
# PDF → Summary
pdftotext document.pdf - | ask-ai summarize

# PDF → Summary → Translate
pdftotext document.pdf - | ask-ai summarize | ask-ai translate :pt

# PDF → OCR → Summary
pdftotext scanned.pdf - | ask-ai summarize --style academic
```

## Batch Processing

Process multiple files:

```bash
# OCR all images and summarize
for img in *.png; do
    echo "=== $img ==="
    ask-ai ocr "$img" | ask-ai summarize
    echo
done

# OCR → Translate all images
for img in scans/*.png; do
    out="${img%.png}-pt.txt"
    ask-ai ocr "$img" | ask-ai translate :pt > "$out"
    echo "Created $out"
done
```

## Advanced Workflows

### Document Translation

Translate documents while preserving structure:

```bash
# Complete workflow for scanned documents
ask-ai ocr scanned-ja.png | \
    ask-ai translate ja:pt | \
    ask-ai summarize -l 200 | \
    tee translated-summary.txt
```

### Research Paper Analysis

```bash
# Extract and analyze research
ask-ai ocr paper.png | \
    ask-ai summarize --style academic -l 300 | \
    ask-ai translate :pt | \
    tee analysis-pt.txt
```

### Code Documentation

```bash
# Generate documentation from code
head -50 src/main.rs | \
    ask-ai summarize --style technical | \
    ask-ai translate :pt
```

### Multi-Document Processing

```bash
# Combine multiple documents
for doc in chapter*.txt; do
    cat "$doc" | ask-ai summarize -l 100
done | ask-ai summarize -l 300 > book-summary.txt
```

## Common Patterns

### 1. OCR → Process → Save

```bash
ask-ai ocr document.png | ask-ai summarize > output.txt
```

### 2. File → Translate → Save

```bash
cat document.txt | ask-ai translate :pt > translated.txt
```

### 3. Query → Process → Query

```bash
ask-ai "Find information" | ask-ai summarize | ask-ai "Analyze this"
```

### 4. Chain with Unix Tools

```bash
# Sort results
ask-ai ocr doc.png | sort | ask-ai summarize

# Filter content
ask-ai ocr doc.png | grep "important" | ask-ai translate :pt

# Word count
ask-ai summarize text.txt | wc -w
```

## Error Handling

Handle errors in pipelines:

```bash
# Continue on error
for img in *.png; do
    ask-ai ocr "$img" 2>/dev/null | ask-ai summarize || echo "Failed: $img"
done

# Stop on first error
set -e
ask-ai ocr doc.png | ask-ai summarize
```

## Performance Tips

1. **Process in batches** - Group similar operations
2. **Use specific models** - Match model to task
3. **Limit token usage** - Use `--max-tokens` for OCR
4. **Save intermediate results** - Use `tee` for debugging

```bash
# Save intermediate results
ask-ai ocr doc.png | tee extracted.txt | ask-ai summarize | tee summary.txt | ask-ai translate :pt
```

## Debugging Pipelines

Add debug output:

```bash
# Show each step
ask-ai ocr doc.png | tee /dev/tty | ask-ai summarize

# Debug with timing
time ask-ai ocr doc.png | time ask-ai summarize

# Full debug
ask-ai ocr -d doc.png 2> debug.log | ask-ai summarize -d 2> summary.log
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

ask-ai ocr "$IMAGE" | \
    ask-ai summarize | \
    ask-ai translate ":$LANG"
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
