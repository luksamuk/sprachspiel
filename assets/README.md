# Assets

This directory contains visual assets used by Ask-AI.

## Extended Mind ASCII Art

The welcome banner features an ASCII art representation of the "Extended Mind" concept - a brain with external connections to tools, memory, and Zettelkasten.

### Files

| File | Description |
|------|-------------|
| `extended-mind-original.png` | Original source image (1536x1024) |
| `extended-mind-resized.png` | Resized for ASCII conversion (120x85) |
| `extended-mind-ascii.txt` | ASCII art versions (with and without colors) |

### Generation Process

The ASCII art is generated using [jp2a](https://github.com/Talinx/jp2a) (JPEG/PNG to ASCII):

```bash
# 1. Crop and resize the original image
magick extended-mind-original.png -crop 900x600+300+200 -resize 120x85 extended-mind-resized.png

# 2. Convert to ASCII with colors (True Color ANSI)
jp2a --width=40 --colors extended-mind-resized.png

# 3. Convert to plain ASCII (no colors)
jp2a --width=40 extended-mind-resized.png
```

### Color Scheme

- **Cyan/Turquoise**: Main brain structure
- **Orange/Brown/Yellow**: External connections and neural activity
- **White/Gray**: Supporting elements

### Usage in Code

The ASCII art is embedded in `src/chat/view/mod.rs` as the `EXTENDED_MIND_ART` constant. The ANSI escape codes are preserved to maintain the color information.

### Regenerating

If you need to regenerate the ASCII art with different parameters:

```bash
# Adjust width (default: 40)
jp2a --width=35 --colors extended-mind-resized.png

# Adjust crop region (if image focus changes)
magick extended-mind-original.png -crop 800x500+350+250 -resize 100x70 extended-mind-resized.png
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