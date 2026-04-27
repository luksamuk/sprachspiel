# Mixed-Domain Test Prompts

Tests combining OCR, vision, and comprehension in a single prompt.

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