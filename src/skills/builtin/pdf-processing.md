---
name: pdf-processing
description: Extract and process content from PDF files using external tools like pdftotext and tesseract.
---

# PDF Processing

When asked to process PDF files:

1. **Check tool availability** using `check_tool_availability`:
   - `pdftotext` (text extraction from PDFs)
   - `pdfinfo` (PDF metadata)
   - `pdftoppm` (PDF to image conversion)
   - `tesseract` (OCR for scanned PDFs)

2. **For text-based PDFs**:
   - Use `run_command("pdftotext", ["<file>", "-"])` to extract text
   - The `-` argument outputs to stdout
   - Parse and summarize the text content as needed

3. **For scanned PDFs** (when pdftotext returns empty or garbled text):
   - Convert pages to images: `run_command("pdftoppm", ["-png", "<file>", "output"])`
   - Then use OCR: `run_command("tesseract", ["output-1.png", "stdout"])`
   - Process the OCR output

4. **For PDF metadata**:
   - Use `run_command("pdfinfo", ["<file>"])` to get page count, title, author

5. **Error handling**:
   - If tool not found: Inform user with installation hint
   - If command fails: Display error and suggest alternatives
   - For large files: Mention page limits or suggest chunking

## Common Issues

- **Empty output from pdftotext**: File is likely scanned. Suggest OCR.
- **Permission denied**: File is encrypted or protected.
- **Memory issues**: File is too large. Suggest processing pages individually.