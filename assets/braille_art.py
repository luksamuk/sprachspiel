#!/usr/bin/env python3
"""Converte uma imagem em Braille Art (Unicode) para impressão no terminal."""

import argparse
import sys
from PIL import Image

# Braille dot positions (2x4 grid per character):
#  ┌─┐
#  │1│4│
#  │2│5│
#  │3│6│
#  │7│8│
#  └─┘
#
# Unicode offset for each dot:
DOT_OFFSETS = [
    0x01,  # dot 1 (top-left)
    0x02,  # dot 2 (mid-left)
    0x04,  # dot 3 (bot-left)
    0x40,  # dot 7 (very-bot-left)
    0x08,  # dot 4 (top-right)
    0x10,  # dot 5 (mid-right)
    0x20,  # dot 6 (bot-right)
    0x80,  # dot 8 (very-bot-right)
]

BRAILLE_BASE = 0x2800


def pixel_to_braille(img, threshold=128, invert=False):
    """Convert an image to a list of braille character rows."""
    width, height = img.size

    # Each braille char covers 2x4 pixels (width x height)
    cols = width // 2
    rows = height // 4

    # Convert to grayscale
    gray = img.convert("L")

    result = []
    for row in range(rows):
        line = []
        for col in range(cols):
            code = 0
            for idx, offset in enumerate(DOT_OFFSETS):
                # Map dot index to pixel position
                if idx < 4:  # left column: dots 1,2,3,7
                    px = col * 2
                    py = row * 4 + idx
                else:  # right column: dots 4,5,6,8
                    px = col * 2 + 1
                    py = row * 4 + (idx - 4)

                if px < width and py < height:
                    brightness = gray.getpixel((px, py))
                    # Default: bright pixels = dots (for dark bg images)
                    # Invert: dark pixels = dots (for light bg images)
                    is_on = brightness > threshold if not invert else brightness < threshold
                    if is_on:
                        code |= offset

            line.append(chr(BRAILLE_BASE + code))
        result.append("".join(line))

    return result


def image_to_braille(
    path,
    width=80,
    threshold=128,
    invert=False,
    dither=False,
    colored=False,
):
    """Load image, resize for target width, convert to braille art."""
    img = Image.open(path)

    # Calculate target size: 2 pixels wide per braille char, 4 pixels tall per braille row
    target_w = width * 2
    # Maintain aspect ratio. Braille chars are 2px wide x 4px tall in the dot grid,
    # but in most terminal fonts each character cell is roughly twice as tall as it
    # is wide. To produce visually proportional output we need to halve the vertical
    # resolution so the image doesn't appear stretched vertically.
    aspect = img.height / img.width
    target_h = int(target_w * aspect * 1.2)  # stretch vertical by 1.2x
    # Round to multiple of 4 (each braille row = 4 vertical pixels)
    target_h = max(4, (target_h // 4) * 4)

    img = img.resize((target_w, target_h), Image.LANCZOS)

    if dither:
        img = img.convert("1")
        # Convert back to L for consistent processing
        img = img.convert("L")

    if colored:
        # For colored output, process each color channel separately
        return colored_braille(img, width, threshold, invert)

    lines = pixel_to_braille(img, threshold, invert)
    return lines


def colored_braille(img, width, threshold=128, invert=False):
    """Generate ANSI-colored braille art."""
    gray = img.convert("L")
    rgb = img.convert("RGB")

    w, h = img.size
    cols = w // 2
    rows = h // 4

    lines = []
    for row in range(rows):
        line_parts = []
        for col in range(cols):
            code = 0
            r_sum, g_sum, b_sum, count = 0, 0, 0, 0

            for idx, offset in enumerate(DOT_OFFSETS):
                if idx < 4:
                    px = col * 2
                    py = row * 4 + idx
                else:
                    px = col * 2 + 1
                    py = row * 4 + (idx - 4)

                if px < w and py < h:
                    brightness = gray.getpixel((px, py))
                    is_on = brightness > threshold if not invert else brightness < threshold
                    if is_on:
                        code |= offset
                        pr, pg, pb = rgb.getpixel((px, py))
                        r_sum += pr
                        g_sum += pg
                        b_sum += pb
                        count += 1

            char = chr(BRAILLE_BASE + code)

            if code > 0 and count > 0:
                avg_r = r_sum // count
                avg_g = g_sum // count
                avg_b = b_sum // count
                # 24-bit color escape
                line_parts.append(f"\033[38;2;{avg_r};{avg_g};{avg_b}m{char}")
            else:
                line_parts.append(char)

        line_parts.append("\033[0m")  # reset
        lines.append("".join(line_parts))

    return lines


def main():
    parser = argparse.ArgumentParser(description="Convert image to Braille Art")
    parser.add_argument("image", help="Path to image file")
    parser.add_argument("-w", "--width", type=int, default=80, help="Output width in braille chars (default: 80)")
    parser.add_argument("-t", "--threshold", type=int, default=128, help="Brightness threshold 0-255 (default: 128)")
    parser.add_argument("--invert", action="store_true", help="Invert: bright = dot (for dark-on-light images)")
    parser.add_argument("--dither", action="store_true", help="Use Floyd-Steinberg dithering")
    parser.add_argument("--color", action="store_true", help="Colored output using ANSI 24-bit")
    parser.add_argument("-o", "--output", help="Save to file instead of printing")
    args = parser.parse_args()

    lines = image_to_braille(
        args.image,
        width=args.width,
        threshold=args.threshold,
        invert=args.invert,
        dither=args.dither,
        colored=args.color,
    )

    text = "\n".join(lines)

    if args.output:
        with open(args.output, "w") as f:
            f.write(text)
        print(f"Saved to {args.output} ({len(lines)} rows, {len(lines[0]) if lines else 0} cols)")
    else:
        print(text)


if __name__ == "__main__":
    main()