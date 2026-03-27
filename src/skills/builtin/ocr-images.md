---
name: ocr-images
description: Perform OCR on images to extract text content using tesseract.
---

# OCR Image Processing

When asked to extract text from images:

1. **Check tool availability** using `check_tool_availability`:
   - `tesseract` (OCR engine)
   - `exiftool` (optional, for image metadata)
   - `magick` (ImageMagick, for image conversion)

2. **For single images**:
   - Use `run_command("tesseract", ["<image>", "stdout"])` to extract text
   - The `stdout` argument outputs directly instead of a file
   - Process and analyze the extracted text

3. **For screenshots and photos**:
   - Consider that text may be skewed or low quality
   - Suggest preprocessing (rotation, contrast) if OCR quality is poor
   - ImageMagick can help: `run_command("magick", ["<image>", "-rotate", "90", "output.png"])`

4. **For image metadata**:
   - Use `run_command("exiftool", ["<image>"])` to get dimensions, camera info
   - Useful for understanding image context

5. **Multi-language OCR**:
   - Tesseract supports multiple languages with `-l` flag
   - Use `run_command("tesseract", ["<image>", "stdout", "-l", "por"])` for Portuguese

6. **Error handling**:
   - If tesseract not found: Inform user with installation hint
   - If image unreadable: Suggest converting to PNG or improving quality
   - Language packs: Mention they may need separate installation

## Supported Image Formats

- PNG, JPEG, TIFF, BMP, GIF, WebP
- PDF pages (convert with pdftoppm first)

## Common Issues

- **No text found**: Image may have no text, or text is handwriting
- **Poor quality**: Suggest higher resolution image
- **Wrong language**: Remember to specify language with `-l` flag