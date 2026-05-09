# Mixed-Domain Test Prompts

Tests combining OCR, vision, and comprehension in a single prompt.

## Sprachspiel Architecture PDF (sprachspiel-architecture.pdf)

A 3-page PDF with real content from the Sprachspiel project. Tests the two-phase document processing pipeline (LLM-orchestrated).

### Chat Mode: Phase 1 — Text Extraction

Ask the LLM in chat mode:

```
Use run_command("pdftotext", ["assets/mixed/sprachspiel-architecture.pdf", "-"]) to extract all text from this PDF.
```

Then follow up:
- `List the five layers of the chat architecture and their purposes.`
- `What are the four context overflow thresholds and what does each trigger?`
- `Explain the difference between exclusive and accumulative predicates in the fact system.`

Expected: All text from pages 1 and 3 should extract cleanly. Page 2 text extracts but the diagram's spatial structure is lost.

### Chat Mode: Phase 2 — Vision Analysis of Diagram

Ask the LLM to convert the diagram page to an image and analyze it:

```
Use run_command("pdftoppm", ["-png", "-f", "2", "-l", "2", "-r", "150", "assets/mixed/sprachspiel-architecture.pdf", "/tmp/arch-page2"]) to convert page 2 to an image.
Then use spawn_vision_agent("Describe the sub-agent architecture diagram. What does each colored box represent?", "/tmp/arch-page2-2.png") to analyze the diagram.
```

Or for OCR of tables:
```
Use run_command("pdftoppm", ["-png", "-f", "1", "-l", "1", "-r", "150", "assets/mixed/sprachspiel-architecture.pdf", "/tmp/arch-page1"]) to convert page 1 to an image.
Then use spawn_ocr_agent("Extract the tables from this page", "/tmp/arch-page1-1.png", "table") to extract table structure.
```

Expected: Vision gives richer spatial interpretation (hierarchy, connections, color-coded phases). OCR preserves table layout accurately.

### Chat Mode: Full Pipeline Test

```
I have a PDF at assets/mixed/sprachspiel-architecture.pdf. Please process it — extract all text, and for any pages with diagrams, convert them to images and describe what you see.
```

Expected:
1. LLM calls `run_command("pdftotext", [...])` for Phase 1
2. LLM identifies that page 2 has a diagram
3. LLM calls `run_command("pdftoppm", [...])` to convert page 2 to image
4. LLM calls `spawn_vision_agent` or `spawn_ocr_agent` on the resulting image
5. LLM combines results and presents a complete answer

### Chat Mode: PDF Import Pipeline

```
I have a PDF at assets/mixed/sprachspiel-architecture.pdf. Extract the text and import it as a document.
```

Expected:
1. LLM does NOT try `import_document` with the `.pdf` directly
2. LLM uses `run_command("pdftotext")` to extract text
3. LLM uses `write_file` to save the extracted text to a `.txt` file
4. LLM calls `import_document` with the `.txt` file
5. LLM returns confirmation with doc ID

### CLI Mode: Phase 1 — Text Extraction

```bash
# Extract text to stdout
pdftotext assets/mixed/sprachspiel-architecture.pdf -

# Or extract page by page
pdftotext -f 1 -l 1 assets/mixed/sprachspiel-architecture.pdf -
```

### CLI Mode: Phase 2 — Vision of Diagram

```bash
# Convert page 2 to image
pdftoppm -png -f 2 -l 2 -r 150 assets/mixed/sprachspiel-architecture.pdf /tmp/arch-page2
# Then analyze with vision CLI
sprach vision /tmp/arch-page2-2.png -- "Describe the sub-agent architecture diagram"
```

### Table Extraction from PDF

```bash
# CLI: Convert page 1 to image, then OCR in table mode
pdftoppm -png -f 1 -l 1 -r 150 assets/mixed/sprachspiel-architecture.pdf /tmp/arch-page1
sprach ocr /tmp/arch-page1-1.png --mode table
```

Chat mode:
```
Use run_command("pdftoppm", ["-png", "-f", "1", "-l", "1", "-r", "150", "assets/mixed/sprachspiel-architecture.pdf", "/tmp/arch-page1"]) then spawn_ocr_agent("Extract table structure", "/tmp/arch-page1-1.png", "table").
```

Expected: The two tables (layer architecture, overflow thresholds) on page 1 should be extracted as structured data.

### Regression Notes

- `sprachspiel-architecture.pdf` is generated from the project's own architecture documentation
- Page 1: text-heavy with two structured tables → pdftotext should extract perfectly
- Page 2: diagram with colored boxes, arrows, and phase labels → requires vision for spatial interpretation
- Page 3: text-only RAG section → pdftotext sufficient
- The diagram's box labels (e.g., "OCR Agent", "Vision Agent") DO extract as text via pdftotext, but the arrows, hierarchy, and color-coded phases are lost — this is the exact scenario where Phase 2 is needed
- Tests the full two-phase pipeline: Phase 1 (pdftotext) for text → Phase 2 (pdftoppm → vision/OCR) for visual content
- **Important:** OCR and Vision tools do NOT accept PDF files directly — PDFs must be converted to images first via `pdftoppm`

## ENEM Redação (redacao.png)

### OCR + Comprehension + Generation
```
/ocr assets/mixed/redacao.png text
```

Then follow up:
- `Extraia o tema da redação proposto nesta página.`
- `Quais são os textos motivadores apresentados?`
- `Crie uma redação nota 1000 sobre este tema, seguindo todas as instruções do enunciado.`

### Full Pipeline Test (spawn_ocr_agent)
```
"Use the spawn_ocr_agent tool with file_path='assets/mixed/redacao.png' and ocr_mode='text' to extract all text from this ENEM exam page. Then, based on the extracted content, write an argumentative essay following the prompt instructions."
```

### Table Extraction
```
/ocr assets/mixed/redacao.png table
```
Expected: The enrollment statistics graph (Text II) should be extracted as a structured table.

### Figure Description
```
/ocr assets/mixed/redacao.png figure
```
Expected: The advertisement image (Text III) should be described, noting the visual elements and message.

## Regression Notes

- `redacao.png` is a Brazilian standardized test (ENEM 2017) essay prompt page
- Contains mixed content: printed text, a graph (Text II), an image (Text III), and legal excerpt (Text I/IV)
- Tests all 4 OCR modes on a single complex document
- Table mode should extract the enrollment statistics
- Figure mode should describe the advertisement
- Text mode should extract ALL printed text including Portuguese legal language