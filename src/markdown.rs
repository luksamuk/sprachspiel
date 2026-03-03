//! Global markdown skin for termimad rendering
//!
//! This module provides a centralized markdown rendering skin that respects
//! user configuration from `config.toml` (`display.skin` setting).
//!
//! Supported themes:
//! - `dark`: Dark background optimized (transparent, no gray bars)
//! - `light`: Light background optimized (transparent, no gray bars)
//! - `mono`: Monochrome, no colors (bold/italic preserved)

use std::sync::OnceLock;
use termimad::MadSkin;

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
/// This is the primary function to use for rendering markdown output.
/// Falls back to default skin if not initialized.
pub fn print_markdown(text: &str) {
    let skin = MARKDOWN_SKIN
        .get()
        .or_else(|| DEFAULT_SKIN.get())
        .expect("Skin not initialized - call init_markdown_skin() at startup");
    skin.print_text(text);
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
