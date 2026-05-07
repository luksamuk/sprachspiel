# Sprachspiel Launch Reel — Design System

## Style
Cyberpunk terminal. Late-90s hacker workstation meets analog video decay.

## Colors
| Role | Hex | Usage |
|------|-----|-------|
| Background | `#000000` | Pure black ground truth |
| Phosphor green | `#33FF33` | Terminal text, boot lines, status |
| Warm amber | `#FFD080` | Hero text, accents, install cmd |
| Cool cyan | `#5599DD` | Secondary accents, code |
| Hot magenta | `#CC00FF` | Highlights, interference |
| Dim green | `rgba(51,255,51,0.35)` | Muted terminal text |
| Dark amber | `rgba(255,208,128,0.4)` | Secondary amber |

## Typography
- **VT323** — terminal text, boot sequences, system labels, subtitles
- **IBM Plex Mono** (400/600/700) — hero titles, code, install commands, feature names
- Never use sans-serif. This is a terminal world.

## CRT Layers (persistent across ALL scenes)
1. **Scanlines** — `repeating-linear-gradient(to bottom, transparent 0px, transparent 2px, rgba(0,0,0,0.12) 2px, rgba(0,0,0,0.12) 4px)` z-index:80
2. **Vignette** — `radial-gradient(ellipse at center, transparent 30%, rgba(0,0,0,0.6) 100%)` z-index:85
3. **Color bleed bars** — top/bottom 3-5px gradient strips with cycling neon colors z-index:90

## Transitions
Hard cuts with 1-2 frame glitch bursts. No dissolves. No fades. Everything switches like a corrupt signal.