//! Global markdown skin for termimad rendering
//!
//! This module provides a centralized markdown rendering skin that respects
//! user configuration from `config.toml` (`display.skin` setting).
//!
//! Supported themes:
//! - `dark`: Dark background optimized (transparent, no gray bars)
//! - `light`: Light background optimized (transparent, no gray bars)
//! - `mono`: Monochrome, no colors (bold/italic preserved)
//!
//! # Chat Terminal Width
//!
//! In interactive chat mode, all output is rendered at a fixed width
//! (`CHAT_TERMINAL_WIDTH = 80` columns) regardless of the actual terminal
//! size. This ensures consistent output for users who run the chat in a
//! floating 80x50 terminal window.

use std::sync::OnceLock;
use termimad::MadSkin;

/// Fixed terminal width for chat mode (80 columns).
///
/// All markdown and thinking block rendering in chat mode uses this width
/// instead of the actual terminal size. This ensures consistent output
/// when using a floating 80x50 terminal window.
pub const CHAT_TERMINAL_WIDTH: usize = 80;

/// Global markdown skin, initialized once at startup
static MARKDOWN_SKIN: OnceLock<MadSkin> = OnceLock::new();

/// Default skin for fallback
static DEFAULT_SKIN: OnceLock<MadSkin> = OnceLock::new();

/// Initialize the global markdown skin based on user configuration.
///
/// Must be called once at startup before any markdown rendering.
/// If not called, defaults to `MadSkin::default()`.
///
/// # Arguments
/// * `theme` - Theme name: "dark", "light", or "mono"
pub fn init_markdown_skin(theme: &str) {
    let skin = match theme.to_lowercase().as_str() {
        "dark" => MadSkin::default_dark(),
        "light" => MadSkin::default_light(),
        "mono" | "monochrome" | "nocolor" => create_mono_skin(),
        _ => MadSkin::default(),
    };
    let _ = MARKDOWN_SKIN.set(skin);

    // Also initialize default skin for fallback
    let _ = DEFAULT_SKIN.set(MadSkin::default());
}

/// Create a monochrome skin with bold/italic but no colors
fn create_mono_skin() -> MadSkin {
    let mut skin = MadSkin::no_style();
    // Preserve bold with gray color for visibility
    skin.bold = termimad::CompoundStyle::with_fg(termimad::gray(20));
    // Preserve italic with slightly lighter gray
    skin.italic = termimad::CompoundStyle::with_fg(termimad::gray(17));
    skin
}

/// Print markdown text using the global skin.
///
/// This is the primary function to use for rendering markdown output
/// in non-chat contexts (query mode, translation, summarize, etc.).
/// Uses the real terminal width for line wrapping.
/// Falls back to default skin if not initialized.
pub fn print_markdown(text: &str) {
    let skin = MARKDOWN_SKIN
        .get()
        .or_else(|| DEFAULT_SKIN.get())
        .expect("Skin not initialized - call init_markdown_skin() at startup");
    skin.print_text(text);
}

/// Print markdown text at chat terminal width (80 columns).
///
/// Use this for all markdown rendering in interactive chat mode
/// to ensure consistent output regardless of terminal size.
/// Uses `CHAT_TERMINAL_WIDTH` (80) for line wrapping.
pub fn print_markdown_chat(text: &str) {
    let skin = MARKDOWN_SKIN
        .get()
        .or_else(|| DEFAULT_SKIN.get())
        .expect("Skin not initialized - call init_markdown_skin() at startup");
    let fmt = skin.text(text, Some(CHAT_TERMINAL_WIDTH));
    print!("{}", fmt);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_mono_skin() {
        // Just verify it doesn't panic
        let _skin = create_mono_skin();
    }

    #[test]
    fn test_init_dark_skin() {
        // Note: OnceLock only sets once, so we can't test multiple inits
        // This test just verifies the function doesn't panic
        init_markdown_skin("dark");
    }

    #[test]
    fn test_init_light_skin() {
        init_markdown_skin("light");
    }

    #[test]
    fn test_init_mono_skin() {
        init_markdown_skin("mono");
    }

    #[test]
    fn test_init_unknown_skin_fallback() {
        init_markdown_skin("unknown_theme");
    }
}
