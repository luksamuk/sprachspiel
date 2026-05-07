#!/usr/bin/env python3
"""
Sprachspiel README Banner — v1 (adapted from ask-ai-banner.py v3)

Brain + gold connections rendered as braille art (inherited from v3).
Title "SPRACHSPIEL" uses Pillow font with gold/cyan color split.
Everything visual: braille art brain/connections + clean font text.

Changes from ask-ai-banner.py:
- Title: "ask-ai" → "SPRACHSPIEL" (SPRACH in gold, SPIEL in cyan)
- Subtitle: "Cognitive interaction harness for LLMs" → "A language game with LLMs"
- Tags: arrows → pipes (│)
- Hardcoded path → relative path using Path(__file__).parent
- Output path → relative
"""

from PIL import Image, ImageDraw, ImageFont, ImageFilter
import numpy as np
import random
import math
from pathlib import Path

ASSETS_DIR = Path(__file__).parent
W, H = 1280, 400
BG_COLOR = (8, 8, 18)

# Color palette — Sprachspiel branding
AURA_CYAN = (0, 210, 220)
AURA_CYAN_DIM = (0, 80, 90)
BRAIN_WHITE = (220, 230, 240)
LINE_GOLD = (255, 200, 60)
LINE_GOLD_DIM = (100, 75, 15)
NODE_GOLD = (255, 215, 80)
NODE_BRIGHT = (255, 240, 150)
TEXT_GOLD = (210, 170, 40)
TEXT_GOLD_BRIGHT = (255, 220, 60)  # SPRACH color
TEXT_CYAN = (0, 210, 220)           # SPIEL color
TEXT_WHITE = (200, 210, 225)
GRID_COLOR = (14, 18, 30)
BRAILLE_COLOR = (14, 18, 28)
BRAILLE_ACCENT = (22, 38, 55)

# Dense braille for lines
LINE_BRAILLE = "⠶⠷⠾⠽⠿⡿⣿⠛⠟⠻⠽⠾⠼⠴⠦⠧⣿⣷⣶⣤⣀"
# Cluster braille for nodes
CLUSTER_BRAILLE = "⣿⡿⢿⠿⠛⠟⠻⠽⠶⠷⠾⣶⣷⣿⡇⡄⡆⡋⡏⡗⡧⡥⡳⡵⡷⢀⢄⢈⢐⢘⢠⢰⢸"
# Background braille
BRAILLE_CHARS = "⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏⠐⠑⠒⠓⠔⠕⠖⠗⠘⠙⠚⠛⠜⠝⠞⠟⠠⠡⠢⠣⠤⠥⠦⠧⠨⠩⠪⠫⠬⠭⠮⠯⠰⠱⠲⠳⠴⠵⠶⠷⠸⠹⠺⠻⠼⠽⠾⠿"

img = Image.new("RGB", (W, H), BG_COLOR)
draw = ImageDraw.Draw(img)

try:
    font_braille = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 9)
    font_braille_md = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 11)
    font_braille_lg = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 14)
    font_title_gold = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf", 48)
    font_title_cyan = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf", 48)
    font_sub = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 17)
    font_tiny = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 11)
except:
    font_braille = font_braille_md = font_braille_lg = font_title_gold = font_title_cyan = font_sub = font_tiny = ImageFont.load_default()

random.seed(42)

# === Load logo and build dot map ===
logo = Image.open(ASSETS_DIR / "extended-mind-original.png").convert("RGB")
logo_arr = np.array(logo)
lh, lw, _ = logo_arr.shape
mask = np.sum(logo_arr, axis=2) > 30
rows_content = np.any(mask, axis=1)
cols_content = np.any(mask, axis=0)
row_min = np.argmax(rows_content)
row_max = len(rows_content) - 1 - np.argmax(rows_content[::-1])
col_min = np.argmax(cols_content)
col_max = len(cols_content) - 1 - np.argmax(cols_content[::-1])

GRID_COLS = 100
GRID_ROWS = int(GRID_COLS * (row_max - row_min) / max(1, col_max - col_min))
cell_w = (col_max - col_min) / GRID_COLS
cell_h = (row_max - row_min) / GRID_ROWS

dot_map = []
for gy in range(GRID_ROWS):
    row = []
    for gx in range(GRID_COLS):
        x0 = int(col_min + gx * cell_w)
        y0 = int(row_min + gy * cell_h)
        x1 = int(col_min + (gx + 1) * cell_w)
        y1 = int(row_min + (gy + 1) * cell_h)
        cell = logo_arr[y0:y1, x0:x1]
        if cell.size == 0:
            row.append(' ')
            continue
        avg = cell.mean(axis=(0, 1))
        brightness = avg.sum() / 3
        if brightness < 15:
            row.append(' ')
        elif avg[2] > avg[0] * 1.3 and avg[2] > avg[1] * 0.9 and brightness > 80:
            row.append('A')
        elif avg[2] > avg[0] * 1.1 and brightness > 30:
            row.append('a')
        elif avg[0] > avg[2] * 1.5 and avg[1] > avg[2] * 1.5 and brightness > 40:
            row.append('G')
        elif brightness > 160 and avg[0] > 150 and avg[1] > 150 and avg[2] > 150:
            row.append('B')
        elif brightness > 30:
            row.append('.')
        else:
            row.append(' ')
    dot_map.append(row)

brain_cx = W // 2 - 180
brain_cy = H // 2
target_w = 310
px_w = target_w / GRID_COLS
px_h = px_w
origin_x = brain_cx - (GRID_COLS * px_w) / 2
origin_y = brain_cy - (GRID_ROWS * px_h) / 2


# ============================================================
# LAYER 1: Background grid + braille rain
# ============================================================
for x in range(0, W, 48):
    for y in range(0, H, 48):
        draw.ellipse([x - 1, y - 1, x + 1, y + 1], fill=(18, 22, 38))
        if x + 48 < W:
            draw.line([(x, y), (x + 48, y)], fill=GRID_COLOR, width=1)
        if y + 48 < H:
            draw.line([(x, y), (x, y + 48)], fill=GRID_COLOR, width=1)

for y in range(0, H, 10):
    for x in range(0, W, 6):
        dx = (x - W / 2) / (W / 2)
        dy = (y - H / 2) / (H / 2)
        dist = math.sqrt(dx * dx + dy * dy)
        prob = 0.05 + 0.12 * (1 - max(0, 1 - dist * 0.7))
        if random.random() < prob:
            ch = random.choice(BRAILLE_CHARS)
            if dist < 0.3:
                if random.random() < 0.3:
                    continue
                color = (12, 22, 38)
            elif dist < 0.6:
                color = BRAILLE_ACCENT
            else:
                color = BRAILLE_COLOR
            draw.text((x, y), ch, fill=color, font=font_braille)


# ============================================================
# LAYER 2: Aura + Brain (dot-based) — identical to original
# ============================================================
for i in range(22, 0, -1):
    r = 35 + i * 7
    af = 0.018 / (i * 0.12 + 0.1)
    color = tuple(min(255, int(6 + c * af)) for c in AURA_CYAN)
    draw.ellipse([brain_cx - r, brain_cy - r, brain_cx + r, brain_cy + r], fill=color)

brain_cells = []
aura_cells = []
aura_dim_cells = []
gold_cells = []
for gy in range(GRID_ROWS):
    for gx in range(GRID_COLS):
        ct = dot_map[gy][gx]
        px = origin_x + gx * px_w + px_w / 2
        py = origin_y + gy * px_h + px_h / 2
        if ct == 'B': brain_cells.append((px, py, gx, gy))
        elif ct == 'A': aura_cells.append((px, py, gx, gy))
        elif ct == 'a': aura_dim_cells.append((px, py, gx, gy))
        elif ct == 'G': gold_cells.append((px, py, gx, gy))

for px, py, gx, gy in aura_dim_cells:
    dot_r = max(1, int(px_w * 0.35))
    draw.ellipse([px - dot_r, py - dot_r, px + dot_r, py + dot_r], fill=AURA_CYAN_DIM)

for px, py, gx, gy in aura_cells:
    dot_r = max(1, int(px_w * 0.4))
    edge_dist = math.sqrt(((gx - GRID_COLS / 2) / (GRID_COLS / 2 + 0.1)) ** 2 +
                          ((gy - GRID_ROWS / 2) / (GRID_ROWS / 2 + 0.1)) ** 2)
    brightness = max(0.5, 1.0 - edge_dist * 0.3)
    color = tuple(min(255, int(c * brightness)) for c in AURA_CYAN)
    draw.ellipse([px - dot_r, py - dot_r, px + dot_r, py + dot_r], fill=color)

for px, py, gx, gy in brain_cells:
    sulci = (math.sin(gx * 0.25) * math.cos(gy * 0.3) +
             math.sin(gx * 0.12 + gy * 0.15) * 0.4)
    if sulci < -0.3:
        brightness = 0.55; dot_r = max(1, int(px_w * 0.3))
    elif sulci > 0.3:
        brightness = 1.0; dot_r = max(2, int(px_w * 0.5))
    else:
        brightness = 0.8; dot_r = max(1, int(px_w * 0.4))
    edge_dist = math.sqrt(((gx - GRID_COLS / 2) / (GRID_COLS / 2 + 0.1)) ** 2 +
                          ((gy - GRID_ROWS / 2) / (GRID_ROWS / 2 + 0.1)) ** 2)
    b = brightness * max(0.4, 1.0 - edge_dist * 0.25)
    color = tuple(min(255, int(c * b)) for c in BRAIN_WHITE)
    draw.ellipse([px - dot_r, py - dot_r, px + dot_r, py + dot_r], fill=color)


# ============================================================
# LAYER 3: Gold connections — BRAILLE ART LINES + BRAILLE CLUSTER NODES
# ============================================================

arm_positions = []
arm_starts = []
for direction in ['tl', 'tr', 'bl', 'br']:
    candidates_end = []
    candidates_start = []
    for gx, gy_ in [(c[2], c[3]) for c in gold_cells]:
        if direction == 'tl' and gx < GRID_COLS * 0.45 and gy_ < GRID_ROWS * 0.45:
            outer = gx + gy_
            inner = (gx - GRID_COLS / 2) ** 2 + (gy_ - GRID_ROWS / 2) ** 2
            candidates_end.append((outer, gx, gy_))
            candidates_start.append((inner, gx, gy_))
        elif direction == 'tr' and gx >= GRID_COLS * 0.55 and gy_ < GRID_ROWS * 0.45:
            outer = (GRID_COLS - gx) + gy_
            inner = (gx - GRID_COLS / 2) ** 2 + (gy_ - GRID_ROWS / 2) ** 2
            candidates_end.append((outer, gx, gy_))
            candidates_start.append((inner, gx, gy_))
        elif direction == 'bl' and gx < GRID_COLS * 0.45 and gy_ >= GRID_ROWS * 0.55:
            outer = gx + (GRID_ROWS - gy_)
            inner = (gx - GRID_COLS / 2) ** 2 + (gy_ - GRID_ROWS / 2) ** 2
            candidates_end.append((outer, gx, gy_))
            candidates_start.append((inner, gx, gy_))
        elif direction == 'br' and gx >= GRID_COLS * 0.55 and gy_ >= GRID_ROWS * 0.55:
            outer = (GRID_COLS - gx) + (GRID_ROWS - gy_)
            inner = (gx - GRID_COLS / 2) ** 2 + (gy_ - GRID_ROWS / 2) ** 2
            candidates_end.append((outer, gx, gy_))
            candidates_start.append((inner, gx, gy_))

    if candidates_end:
        best_end = min(candidates_end, key=lambda c: c[0])
        epx = origin_x + best_end[1] * px_w + px_w / 2
        epy = origin_y + best_end[2] * px_h + px_h / 2
        arm_positions.append((epx, epy))
    if candidates_start:
        best_start = min(candidates_start, key=lambda c: c[0])
        spx = origin_x + best_start[1] * px_w + px_w / 2
        spy = origin_y + best_start[2] * px_h + px_w / 2
        arm_starts.append((spx, spy))

# Gold cells as bright braille dots
random.seed(123)
for px, py, gx, gy in gold_cells:
    ch = random.choice(LINE_BRAILLE)
    brightness = random.uniform(0.65, 1.0)
    color = tuple(min(255, int(c * brightness)) for c in LINE_GOLD)
    draw.text((px - 4, py - 5), ch, fill=color, font=font_braille_md)

# Lines as braille art strings
for start, end in zip(arm_starts, arm_positions):
    x0, y0 = start
    x1, y1 = end
    length = math.sqrt((x1 - x0) ** 2 + (y1 - y0) ** 2)
    if length < 1:
        continue
    dx = (x1 - x0) / length
    dy = (y1 - y0) / length
    nx = -dy
    ny = dx

    step = 7
    t = 0
    idx = 0
    while t < length:
        cx = x0 + dx * t
        cy = y0 + dy * t
        progress = t / length
        ch = LINE_BRAILLE[idx % len(LINE_BRAILLE)]
        brightness = 0.7 + 0.3 * (1.0 - abs(progress - 0.5) * 2 * 0.3)
        color = tuple(min(255, int(c * brightness)) for c in LINE_GOLD)
        draw.text((cx - 4, cy - 5), ch, fill=color, font=font_braille_lg)

        for offset in [-7, 7]:
            gx_pos = cx + nx * offset
            gy_pos = cy + ny * offset
            ch_glow = random.choice(LINE_BRAILLE)
            glow_brightness = brightness * 0.35
            glow_color = tuple(min(255, int(c * glow_brightness)) for c in LINE_GOLD)
            draw.text((gx_pos - 3, gy_pos - 4), ch_glow, fill=glow_color, font=font_braille)

        for offset in [-14, 14]:
            gx_pos = cx + nx * offset
            gy_pos = cy + ny * offset
            ch_glow = random.choice(BRAILLE_CHARS)
            glow_brightness = brightness * 0.15
            glow_color = tuple(min(255, int(c * glow_brightness)) for c in LINE_GOLD)
            draw.text((gx_pos - 3, gy_pos - 4), ch_glow, fill=glow_color, font=font_braille)

        t += step
        idx += 1

# Endpoint nodes as braille clusters
for epx, epy in arm_positions:
    for angle in range(0, 360, 15):
        for r in [12, 18, 24, 30]:
            rad = math.radians(angle)
            bx = epx + math.cos(rad) * r - 4
            by = epy + math.sin(rad) * r - 5
            brightness = max(0.08, 0.4 * (1 - r / 35))
            color = tuple(min(255, int(c * brightness)) for c in NODE_GOLD)
            ch = random.choice(CLUSTER_BRAILLE)
            draw.text((bx, by), ch, fill=color, font=font_braille_md)

    for angle in range(0, 360, 30):
        for r in [3, 6, 9]:
            rad = math.radians(angle)
            bx = epx + math.cos(rad) * r - 4
            by = epy + math.sin(rad) * r - 5
            brightness = 0.7 + 0.3 * (1 - r / 12)
            color = tuple(min(255, int(c * brightness)) for c in NODE_GOLD)
            ch = random.choice(CLUSTER_BRAILLE)
            draw.text((bx, by), ch, fill=color, font=font_braille_md)

    draw.text((epx - 5, epy - 6), "⣿", fill=NODE_BRIGHT, font=font_braille_lg)


# ============================================================
# LAYER 4: Text — SPRACHSPIEL with gold/cyan split
# ============================================================
def draw_text_bg(draw, pos, text, fill, font, padding=5):
    x, y = pos
    bbox = draw.textbbox((x, y), text, font=font)
    bg = [bbox[0] - padding, bbox[1] - padding, bbox[2] + padding, bbox[3] + padding]
    draw.rectangle(bg, fill=BG_COLOR)
    draw.text((x, y), text, fill=fill, font=font)

title_x = brain_cx + int(GRID_COLS * px_w / 2) + 80
title_y = brain_cy - 55

# Glow effect for title
title_text_sprach = "SPRACH"
title_text_spiel = "SPIEL"
title_text_full = "SPRACHSPIEL"

for g in range(6, 0, -1):
    # Gold glow for SPRACH
    gc = tuple(min(255, c // (g + 1) + d) for c, d in zip(TEXT_GOLD_BRIGHT, [15, 10, 4]))
    draw.text((title_x, title_y), title_text_sprach, fill=gc, font=font_title_gold)
    # Cyan glow for SPIEL
    sprach_bbox = draw.textbbox((title_x, title_y), title_text_sprach, font=font_title_gold)
    spiel_x = sprach_bbox[2]
    cc = tuple(min(255, c // (g + 2) + d) for c, d in zip(AURA_CYAN, [4, 8, 10]))
    draw.text((spiel_x, title_y), title_text_spiel, fill=cc, font=font_title_cyan)

# Main title text
draw.text((title_x, title_y), title_text_sprach, fill=TEXT_GOLD_BRIGHT, font=font_title_gold)
sprach_bbox = draw.textbbox((title_x, title_y), title_text_sprach, font=font_title_gold)
spiel_x = sprach_bbox[2]
draw.text((spiel_x, title_y), title_text_spiel, fill=AURA_CYAN, font=font_title_cyan)

# Subtitle
sub_text = "A language game with LLMs"
draw_text_bg(draw, (title_x, title_y + 52), sub_text, TEXT_WHITE, font_sub)

# Tags
tag_text = "memory │ tools │ personality │ RAG │ translation │ OCR"
draw_text_bg(draw, (title_x, title_y + 74), tag_text, (100, 110, 130), font_tiny)

# Footer — leave empty for now (GitHub URL swap comes later)
# tag_v = "github.com/luksamuk/sprachspiel"
# draw.text((W - 310, H - 20), tag_v, fill=(45, 50, 65), font=font_tiny)


# === SAVE ===
output_path = ASSETS_DIR / "sprachspiel-banner.png"
img.save(str(output_path), "PNG")
print(f"Sprachspiel banner saved to {output_path}")
print(f"Size: {img.size}, Arms: {len(arm_positions)}, Starts: {len(arm_starts)}")