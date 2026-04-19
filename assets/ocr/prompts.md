# OCR Test Prompts

Test prompts for OCR functionality, organized by difficulty and mode.

## Text Mode (`/ocr <file> text`)

### Basic (English printed text)
- `Extraia todo o texto desta imagem.`
- `Read all the text in this image.`

### CJK Characters (japanese.jpg)
- `Extraia o texto japonês contido nesta imagem.`
- `Transcreva todo o texto japonês presente na imagem, preservando a formatação.`

### Handwriting (jpocr.jpg)
- `Extraia todo o texto manuscrito desta imagem.`
- `Transcreva a lista manuscrita mostrada na imagem.`

## Table Mode (`/ocr <file> table`)

### Structured data extraction (japanese.jpg)
- `Extraia a estrutura tabular dos dados contidos nesta imagem em formato markdown.`

**Expected behavior for Table mode:** GLM-OCR uses `"Table Recognition:"` prefix → outputs HTML/marked table. Vision models use `"Extract the table structure from this image. Preserve rows and columns. Output ONLY the table data in markdown format, no analysis or commentary."`

## Figure Mode (`/ocr <file> figure`)

- `Descreva a figura/diagrama contido nesta imagem.`
- `Identify and describe the diagram shown in this image.`

**Expected behavior for Figure mode:** GLM-OCR uses `"Figure Recognition:"` prefix. Vision models use `"Extract and describe the figure or diagram in this image. Output ONLY a description of what is depicted, no analysis or commentary beyond the figure content."`

## Formula Mode (`/ocr <file> formula`)

- `Extraia as fórmulas matemáticas desta imagem em notação LaTeX.`
- `Extract all mathematical formulas from this image in LaTeX notation.`

**Expected behavior for Formula mode:** GLM-OCR uses `"Formula Recognition:"` prefix → outputs LaTeX. Vision models use `"Extract all mathematical formulas from this image. Output ONLY the formulas in LaTeX notation, no analysis or commentary."`

## Regression Notes

- GLM-OCR (default model) uses **rigid prefix prompts** like `"Text Recognition:"` — these produce clean, structured output without commentary
- Vision model substitutes (e.g., qwen3.5, moondream) use **descriptive prompts** containing `"ONLY"` and `"no analysis or commentary"` to constrain output
- Both paths use `/api/generate` for single-shot image processing
- GLM-OCR detection uses `is_glm_ocr_model()` → `model_id.starts_with("glm-ocr")`