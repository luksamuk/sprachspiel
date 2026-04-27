# Mixed-Domain Test Prompts

Tests combining OCR, vision, and comprehension in a single prompt.

## Ask-AI Architecture PDF (ask-ai-architecture.pdf)

A 3-page PDF with real content from the ask-ai project. Tests the two-phase document processing pipeline.

### Phase 1: Text Extraction (pdftotext)
```
/ocr assets/mixed/ask-ai-architecture.pdf text
```
Then follow up:
- `List the five layers of the chat architecture and their purposes.`
- `What are the four context overflow thresholds and what does each trigger?`
- `Explain the difference between exclusive and accumulative predicates in the fact system.`

Expected: All text from pages 1 and 3 should extract cleanly. Page 2 text extracts but the diagram's spatial structure is lost.

### Phase 2: Vision Analysis of Diagram (pdftoppm → spawn_vision_agent)
```
/ocr assets/mixed/ask-ai-architecture.pdf figure
```
Then follow up:
- `Describe the sub-agent architecture diagram. What does each colored box represent?`
- `Explain the two-phase pipeline shown in the diagram — how does Phase 1 differ from Phase 2?`

Expected: OCR figure mode describes the diagram elements. Vision gives richer spatial interpretation (hierarchy, connections, color-coded phases).

### Full Pipeline Test (spawn_vision_agent with pages)
```
"Use the spawn_vision_agent tool with file_path='assets/mixed/ask-ai-architecture.pdf' and pages='2' to analyze the sub-agent architecture diagram on page 2. Describe the flow of data from the main LLM to the external tools."
```

Expected: Vision agent processes only page 2, describes the diagram with color-coding and hierarchy that pdftotext cannot capture.

### Table Extraction from PDF
```
/ocr assets/mixed/ask-ai-architecture.pdf table
```
Expected: The two tables (layer architecture, overflow thresholds) on page 1 should be extracted as structured data.

### Regression Notes

- `ask-ai-architecture.pdf` is generated from the project's own architecture documentation
- Page 1: text-heavy with two structured tables → pdftotext should extract perfectly
- Page 2: diagram with colored boxes, arrows, and phase labels → requires vision for spatial interpretation
- Page 3: text-only RAG section → pdftotext sufficient
- The diagram's box labels (e.g., "OCR Agent", "Vision Agent") DO extract as text via pdftotext, but the arrows, hierarchy, and color-coded phases are lost — this is the exact scenario where Phase 2 is needed
- Tests the full two-phase pipeline: Phase 1 (pdftotext) for text → Phase 2 (pdftoppm → vision) for visual content

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