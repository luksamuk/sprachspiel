# Sprachspiel Launch Reel

Cinematic cyberpunk launch video for [Sprachspiel](https://github.com/luksamuk/ask-ollama-rs) — an open-source, Rust-native cognitive interaction harness.

## Final Renders

| File | Resolution | FPS | Size |
|------|-----------|-----|------|
| `renders/sprachspiel_v8.mp4` | 1920×1080 | 30 | 12 MB |
| `renders/sprachspiel_v8_4k.mp4` | 1920×1080 | 60 | 17 MB |

## Rebuild from Source

```bash
cd hf-askai-launch
npx hyperframes lint                  # validate composition
npx hyperframes render --quality standard --fps 30 --output sprachspiel.mp4
npx hyperframes render --quality high --fps 60 --output sprachspiel_60fps.mp4
```

## Audio Pipeline

Narration generated with HyperFrames TTS (`af_nova` voice):

```bash
# Generate individual segments
npx hyperframes tts "Sprachspiel." --voice af_nova --output assets/v2/s01.mp3
npx hyperframes tts "Cognitive interaction harness. Open source. Rust-native." --voice af_nova --output assets/v2/s02.mp3
# ... (see timestamps.json for full script)
```

Final audio: `assets/vo_final.mp3` — segments concatenated with Blade Runner-style pauses via `ffmpeg -filter_complex`.

## Brain Wireframe

The 3D wireframe brain background uses a **real human brain MRI mesh** (not a procedurally-generated icosphere). The mesh is derived from the pial cortical surface by Anderson Winkler (brainder.org, CC BY-SA 3.0).

### Decimation Pipeline

1. Download pial surface OBJ from [brainder.org](https://brainder.org/research/brain-for-blender/)
2. Run `scripts/decimate_brain.py` with vertex clustering (grid_size=30)
3. Output: 120 vertices, 467 edges → inline JS arrays in `index.html`

```bash
python3 scripts/decimate_brain.py --grid 30 --output brain_mesh_data.js
```

### Axis Mapping (FreeSurfer → Canvas)

| MRI Axis | Meaning | Canvas Axis |
|----------|---------|-------------|
| X | Left/Right | Screen X |
| Y | Posterior/Anterior | Depth (Z rotation) |
| Z | Inferior/Superior | Screen Y (flipped) |

Inline transform: `vx = brainV[i][0], vy = -brainV[i][2], vz = brainV[i][1]`

## Composition Structure

| Time | Section | Palette | Brain Color |
|------|---------|---------|-------------|
| 0–3.5s | Boot sequence | Green CRT | — |
| 3.5–7s | Title card | Amber | — |
| 7–11.5s | Open Source / Architecture | Amber | Dark green |
| 11.5–16s | Persistent Memory | Green | Medium green |
| 16–21.5s | Hybrid RAG | Blue | Blue-teal |
| 21.5–26.5s | 50 Tools | Purple | Purple |
| 26.5–30.5s | Adaptive Personality | Cyan | Bright cyan |
| 30.5–34s | One Binary | Amber | Near white-green |
| 34–38s | CTA | Warm glow | — |

## Key Files

- `index.html` — Main composition (all canvas-rendered, 650 lines)
- `DESIGN.md` — Visual design system (cyberpunk palette, typography, CRT layers)
- `timestamps.json` — Narration segment timings
- `hyperframes.json` — Project configuration
- `assets/v2/` — Individual af_nova narration segments (s01–s08.mp3)
- `assets/vo_final.mp3` — Final concatenated narration (38s)
- `scripts/decimate_brain.py` — MRI mesh decimation script

## Attribution

- Brain mesh: Anderson Winkler, brainder.org, [CC BY-SA 3.0](https://brainder.org/research/brain-for-blender/)
- Sprachspiel: © 2026 luksamuk. Independent project, not affiliated with Nous Research.
- Video: © 2026 luksamuk