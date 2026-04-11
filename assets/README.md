# Assets

This directory contains visual assets used by Ask-AI.

## Extended Mind Braille Art

The welcome banner features a braille art representation of the "Extended Mind" concept — a brain with external connections to tools, memory, and Zettelkasten, generated from `extended-mind.png`.

### Files

| File | Description |
|------|-------------|
| `extended-mind-original.png` | Original source image (1536x1024) |
| `extended-mind.png` | Optimized source image for braille conversion |
| `extended-mind-resized.png` | Resized for ASCII fallback (120x85) |
| `extended-mind-ascii.txt` | ASCII art versions (with and without colors, legacy) |
| `braille_art.py` | Python script that converts images to braille art |

### Generation Process

The braille art is generated using `braille_art.py` (Pillow required):

```bash
# Color braille art (True Color ANSI) — used in the welcome banner
python3 braille_art.py extended-mind.png -w 39 --color

# Plain braille art (no colors) — for comparison
python3 braille_art.py extended-mind.png -w 39
```

The current banner uses **width 39** (14 lines). To regenerate with different parameters:

```bash
# Adjust width (default: 39)
python3 braille_art.py extended-mind.png -w 35 --color

# Use a different source image
python3 braille_art.py extended-mind-original.png -w 39 --color
```

### Color Scheme

- **Cyan/Turquoise**: Main brain structure
- **Orange/Brown/Yellow**: External connections and neural activity
- **White/Gray**: Supporting elements

### Usage in Code

The braille art is embedded in `src/chat/view/mod.rs` as the `EXTENDED_MIND_ART` constant. The ANSI escape codes are preserved to maintain the color information.

### Legacy ASCII Art

The previous banner used jp2a-generated ASCII art. The process is preserved for reference:

```bash
# 1. Crop and resize the original image
magick extended-mind-original.png -crop 900x600+300+200 -resize 120x85 extended-mind-resized.png

# 2. Convert to ASCII with colors
jp2a --width=40 --colors extended-mind-resized.png

# 3. Convert to plain ASCII
jp2a --width=40 extended-mind-resized.png
```

## Logo (ASK-AI)

The logo uses the `toilet` font "future" pre-rendered as ANSI escape codes:

```bash
# Generate the logo
toilet -f future "ASK-AI" --metal
```

The metallic color scheme uses ANSI codes:
- Bold bright blue: `\x1B[1;34;94m`
- Blue: `\x1B[0;34m`
- Reset: `\x1B[0m`