# Sprachspiel Test Assets

Organized test images and prompts for OCR and Vision testing.

## Directory Structure

```
assets/
├── ocr/                    # OCR-specific test images
│   ├── japanese.jpg        # Printed Japanese text (pedagogy)
│   ├── jpocr.jpg           # Handwritten Japanese (hiragana counters)
│   └── prompts.md          # OCR test prompts
├── vision/                 # Vision test images
│   ├── protagonist.jpg     # Alucard (Castlevania: SotN cover art)
│   ├── protagonist2.jpg    # Soma Cruz (Castlevania: Aria of Sorrow cover art)
│   ├── manga.jpg           # Manga/comic panel
│   ├── manuscrito01.jpg   # Handwritten notes (Portuguese)
│   ├── manuscrito02.jpg   # Handwritten notes (Portuguese)
│   └── prompts.md          # Vision test prompts
├── mixed/                  # Multi-domain test images
│   ├── redacao.png         # ENEM 2017 essay prompt page (Portuguese)
│   ├── sprachspiel-architecture.pdf  # 3-page architecture PDF (tables + diagram)
│   └── prompts.md          # Mixed test prompts
├── sprachspiel-banner.png   # Project banner
├── sprachspiel-banner.py    # Banner generator script
├── braille_art.py           # Braille art generator
├── extended-mind-*.png      # Extended mind map images
└── README.md               # This file
```

## Test Categories

### OCR Tests (assets/ocr/)
Designed to test all 4 OCR modes with varying difficulty:

| Image | Type | Text Difficulty | Modes |
|-------|------|----------------|-------|
| japanese.jpg | Printed | High (CJK characters) | text, table |
| jpocr.jpg | Handwritten | Medium (hiragana) | text |

### Vision Tests (assets/vision/)
Designed to test multi-image, character recognition, and description:

| Image | Type | Challenge |
|-------|------|-----------|
| protagonist.jpg | Game cover art | Character + game identification |
| protagonist2.jpg | Game cover art | Multi-image comparison |
| manga.jpg | Manga panel | Scene description |
| manuscrito01.jpg | Handwritten notes | OCR + description |
| manuscrito02.jpg | Handwritten notes | OCR + description |

### Mixed Tests (assets/mixed/)
Cross-domain tests combining OCR, vision, and comprehension:

| Image | Type | Challenge |
|-------|------|-----------|
| redacao.png | Official exam page | Text extraction + comprehension + generation |
| sprachspiel-architecture.pdf | 3-page PDF | Two-phase pipeline: pdftotext + pdftoppm→vision/OCR |

#### With PDF Pipeline (chat mode)

The `sprachspiel-architecture.pdf` tests the LLM-orchestrated two-phase document processing pipeline:

1. **Phase 1 (pdftotext):** Extract text from PDF → `run_command("pdftotext", [...])`
2. **Phase 2 (pdftoppm → vision/OCR):** For pages with diagrams → `run_command("pdftoppm", [...])` then `spawn_vision_agent` or `spawn_ocr_agent`

**Important:** OCR and Vision tools do NOT accept PDF files directly. PDFs must be converted to images first via `pdftoppm`.

#### With PDF Pipeline (CLI mode)

```bash
# Phase 1: Extract text
pdftotext assets/mixed/sprachspiel-architecture.pdf -

# Phase 2: Convert page to image, then analyze
pdftoppm -png -f 2 -l 2 -r 150 assets/mixed/sprachspiel-architecture.pdf /tmp/arch-page2
sprachspiel vision /tmp/arch-page2-2.png -- "Describe the diagram"
sprachspiel ocr /tmp/arch-page2-2.png --mode table   # For tables
```

## Usage in Tests

### With `/ocr` command (chat mode)
```
/ocr assets/ocr/japanese.jpg text
/ocr assets/ocr/japanese.jpg table
/ocr assets/vision/manuscrito01.jpg text
```

### With `/vision` command (chat mode)
```
/vision assets/vision/protagonist.jpg Describe the character
/vision assets/vision/protagonist.jpg,assets/vision/protagonist2.jpg Compare these two images
```

### With CLI
```bash
sprachspiel ocr assets/ocr/japanese.jpg --mode text
sprachspiel vision assets/vision/protagonist.jpg -- "Describe this character"
sprachspiel vision assets/vision/protagonist.jpg,assets/vision/protagonist2.jpg -- "Compare both"
```

### With spawn tools (LLM tool)
```
"Use the spawn_ocr_agent tool with file_path='assets/ocr/japanese.jpg' and ocr_mode='text'"
"Use spawn_vision_agent with file_path='assets/vision/protagonist.jpg'"
```

## Notes

- **research/** directory from `~/testfiles` is intentionally excluded
- All images are real-world test cases (not synthetic) for maximum realism
- The `prompts.md` files contain curated Portuguese/English prompts suitable for each test category