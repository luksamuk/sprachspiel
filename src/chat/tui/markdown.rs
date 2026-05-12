//! Markdown rendering for the TUI
//!
//! This module provides markdown rendering using `tui-markdown` with
//! theme-aware styling. During LLM streaming, plain text is displayed
//! (fast, no parsing overhead). After completion, the full message is
//! re-rendered with `tui-markdown` for syntax highlighting, headers,
//! bold, code blocks, etc.
//!
//! # Themes
//!
//! Three themes map from the existing `DisplaySettings.skin` config:
//! - `dark`: Dark background optimized (default ratatui colors)
//! - `light`: Light background optimized (bright colors for light BG)
//! - `mono`: Monochrome (bold/italic preserved, no colors)
//!
//! # API
//!
//! - `render_markdown(content, theme)` → `Text<'static>` — Full markdown rendering
//! - `render_plain_text(content)` → `Text<'static>` — Fast plain text for streaming
//! - `MarkdownTheme` — Theme enum with `from_config()` and stylesheet selection

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use tui_markdown::{Options, StyleSheet, from_str_with_options};

/// Markdown theme matching the user's `display.skin` configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownTheme {
    Dark,
    Light,
    Mono,
}

impl MarkdownTheme {
    /// Create theme from config string
    ///
    /// Matches the same values as `DisplaySettings.skin`:
    /// "dark", "light", "mono"/"monochrome"/"nocolor"
    pub fn from_config(skin: &str) -> Self {
        match skin.to_lowercase().as_str() {
            "light" => MarkdownTheme::Light,
            "mono" | "monochrome" | "nocolor" => MarkdownTheme::Mono,
            _ => MarkdownTheme::Dark, // default
        }
    }
}

// ── Theme-specific style sheets ─────────────────────────────────────

/// Dark theme stylesheet (optimized for dark terminal backgrounds)
#[derive(Clone, Copy)]
struct DarkStyleSheet;

impl StyleSheet for DarkStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            2 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            3 => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        }
    }

    fn code(&self) -> Style {
        Style::default().fg(Color::Cyan)
    }

    fn link(&self) -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::ITALIC)
    }

    fn heading_meta(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn metadata_block(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }
}

/// Light theme stylesheet (optimized for light terminal backgrounds)
#[derive(Clone, Copy)]
struct LightStyleSheet;

impl StyleSheet for LightStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            2 => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            3 => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        }
    }

    fn code(&self) -> Style {
        Style::default().fg(Color::Blue)
    }

    fn link(&self) -> Style {
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC)
    }

    fn heading_meta(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn metadata_block(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }
}

/// Monochrome theme stylesheet (no colors, just bold/italic)
#[derive(Clone, Copy)]
struct MonoStyleSheet;

impl StyleSheet for MonoStyleSheet {
    fn heading(&self, _level: u8) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    fn code(&self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    fn link(&self) -> Style {
        Style::default().add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::default().add_modifier(Modifier::ITALIC)
    }

    fn heading_meta(&self) -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    fn metadata_block(&self) -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }
}

/// Render markdown content to a ratatui `Text` using tui-markdown
///
/// Applies the given theme's stylesheet for styling.
/// For streaming content, use `render_plain_text` instead.
///
/// Note: The returned `Text` borrows from `content`, so it has the same lifetime.
pub fn render_markdown<'a>(content: &'a str, theme: MarkdownTheme) -> Text<'a> {
    match theme {
        MarkdownTheme::Dark => {
            let options = Options::new(DarkStyleSheet);
            from_str_with_options(content, &options)
        }
        MarkdownTheme::Light => {
            let options = Options::new(LightStyleSheet);
            from_str_with_options(content, &options)
        }
        MarkdownTheme::Mono => {
            let options = Options::new(MonoStyleSheet);
            from_str_with_options(content, &options)
        }
    }
}

/// Render plain text for streaming content
///
/// Used during LLM streaming when we want fast display
/// without markdown parsing overhead. Once the response completes,
/// the message is re-rendered with `render_markdown`.
pub fn render_plain_text(content: &str) -> Text<'static> {
    Text::from(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_config() {
        assert_eq!(MarkdownTheme::from_config("dark"), MarkdownTheme::Dark);
        assert_eq!(MarkdownTheme::from_config("light"), MarkdownTheme::Light);
        assert_eq!(MarkdownTheme::from_config("mono"), MarkdownTheme::Mono);
        assert_eq!(
            MarkdownTheme::from_config("monochrome"),
            MarkdownTheme::Mono
        );
        assert_eq!(MarkdownTheme::from_config("nocolor"), MarkdownTheme::Mono);
        assert_eq!(MarkdownTheme::from_config("unknown"), MarkdownTheme::Dark);
    }

    #[test]
    fn test_render_plain_text() {
        let text = render_plain_text("Hello, world!");
        assert_eq!(text.to_string(), "Hello, world!");
    }

    #[test]
    fn test_render_markdown_dark() {
        let text = render_markdown("# Hello", MarkdownTheme::Dark);
        // Should produce styled text (not empty)
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_render_markdown_light() {
        let text = render_markdown("# Hello", MarkdownTheme::Light);
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_render_markdown_mono() {
        let text = render_markdown("# Hello", MarkdownTheme::Mono);
        assert!(!text.lines.is_empty());
    }
}
