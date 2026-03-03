# Markdown Skin Implementation Plan

**Status:** COMPLETED ✓  
**Date:** 2026-03-03  
**Issue:** `settings.display.skin` configuration is parsed but never applied

---

## Implementation Summary

### Files Changed

| File | Change |
|------|--------|
| `src/markdown.rs` | **NEW** - Global skin module with `init_markdown_skin()` and `print_markdown()` |
| `src/lib.rs` | Added `pub mod markdown;` |
| `src/main.rs` | Added `mod markdown;` and call to `init_markdown_skin()` |
| `src/query.rs` | Replaced `termimad::print_text` with `markdown::print_markdown` |
| `src/chat/repl.rs` | Replaced `termimad::print_text` with `markdown::print_markdown` |
| `src/retrieval/search.rs` | Replaced `termimad::print_text` with `markdown::print_markdown` |

### Theme Support

- `dark`: `MadSkin::default_dark()` - Transparent background for dark terminals
- `light`: `MadSkin::default_light()` - Transparent background for light terminals
- `mono`: Custom `MadSkin::no_style()` with gray bold/italic

### Usage

```toml
# ~/.config/ask-ai/config.toml
[display]
skin = "mono"  # Options: "dark", "light", "mono"
```

---

## Original Plan

---

## Solution

### Architecture

Create a new module `src/markdown.rs` with:
- Global skin initialized once at startup
- Wrapper function `print_markdown()` that uses the global skin
- Support for all termimad themes: `dark`, `light`, `mono`

### File Changes

| File | Action |
|------|--------|
| `src/markdown.rs` | **NEW** - Global skin module |
| `src/main.rs` | Add init call + replace `print_text()` calls |
| `src/query.rs` | Replace `print_text()` with `print_markdown()` |
| `src/chat/repl.rs` | Replace `print_text()` with `print_markdown()` |
| `src/retrieval/search.rs` | Replace `print_text()` with `print_markdown()` |
| `src/settings.rs` | Update sample config documentation |

### Implementation Details

```rust
// src/markdown.rs
use std::sync::OnceLock;
use termimad::MadSkin;

static MARKDOWN_SKIN: OnceLock<MadSkin> = OnceLock::new();

/// Initialize skin once at startup
pub fn init_markdown_skin(theme: &str) {
    let skin = match theme.to_lowercase().as_str() {
        "dark" => MadSkin::default_dark(),
        "light" => MadSkin::default_light(),
        "mono" | "monochrome" | "nocolor" => {
            let mut skin = MadSkin::no_style();
            // Keep bold/italic, no colors
            skin.bold = termimad::CompoundStyle::with_fg(termimad::gray(20));
            skin.italic = termimad::CompoundStyle::with_fg(termimad::gray(17));
            skin
        }
        _ => MadSkin::default(),
    };
    let _ = MARKDOWN_SKIN.set(skin);
}

/// Print markdown using global skin
pub fn print_markdown(text: &str) {
    MARKDOWN_SKIN.get().unwrap_or(&MadSkin::default()).print_text(text);
}

/// Get global skin reference
pub fn get_markdown_skin() -> &'static MadSkin {
    MARKDOWN_SKIN.get().unwrap_or(&MadSkin::default())
}
```

### Theme Details

| Theme | Termimad Function | Background | Problem Solved |
|-------|-------------------|------------|----------------|
| `dark` | `MadSkin::default_dark()` | Transparent | Gray bars on line breaks |
| `light` | `MadSkin::default_light()` | Transparent | Gray bars on line breaks |
| `mono` | Custom no-style | Transparent | No colors, formatting preserved |

### Locations to Update

```
src/main.rs:
  - Line ~33: Add `mod markdown;`
  - Line ~startup: Call `markdown::init_markdown_skin(&settings.display.skin)`
  - Line 246: `print_markdown()` (translate output)
  - Line 499: `print_markdown()` (summarize output)
  - Line 599: `print_markdown()` (vision output)

src/query.rs:
  - Line 88: `print_markdown()` (query output)
  - Line 163: `print_markdown()` (query output)

src/chat/repl.rs:
  - Line 1003: `print_markdown()` (chat output)

src/retrieval/search.rs:
  - Line 103: `print_markdown()` (search output)

src/chat/thinking.rs:
  - Line 142: **KEEP MadSkin::default()** - thinking display unchanged
```

### DO NOT CHANGE

- `src/chat/thinking.rs` - keeps its own `MadSkin::default()` for consistent thinking display
- Thinking blocks should remain visually distinct regardless of user's skin choice

---

## Verification

1. Add `skin = "light"` to `~/.config/ask-ai/config.toml`
2. Run `ask chat` and send a message with markdown formatting
3. Verify colors match light theme (dark text on transparent bg)
4. Test `skin = "mono"` - should have no colors
5. Test `skin = "dark"` - should match current default

---

## Notes

- The "gray bars on line breaks" that the user mentioned are likely caused by `MadSkin::default()` having a background color
- `default_dark()` and `default_light()` use transparent backgrounds
- Custom `mono` skin preserves bold/italic for readability while removing colors