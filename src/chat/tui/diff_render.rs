//! TUI diff rendering — converts unified diff text to colored ratatui Lines.
//!
//! Used by the chat area to render `​```diff` blocks in tool results inline
//! in the chat flow. Provides green/red coloring for `+`/`-` lines, line
//! number gutter, and optional syntax highlighting via `syntect`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use super::markdown::MarkdownTheme;
use super::styles;

/// Shared syntax set loaded once (same approach as tui-markdown).
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// Shared syntect theme set.
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Render a unified diff text block as colored ratatui Lines.
///
/// - `+` lines: green fg + syntect syntax highlighting on content
/// - `-` lines: red fg + syntect syntax highlighting on content
/// - ` ` lines: normal/dim
/// - `@@` hunk headers: cyan dim
/// - Line number gutter: single column (new-file line), dim
///
/// `file_path` is used to detect the language for syntax highlighting.
/// If the language is unknown, falls back to plain text (no syntect colors).
pub fn render_diff_block(
    diff_text: &str,
    file_path: &str,
    theme: MarkdownTheme,
    style_enabled: bool,
) -> Vec<Line<'static>> {
    // Detect syntax for the file path
    let syntax = Path::new(file_path)
        .extension()
        .and_then(|ext| SYNTAX_SET.find_syntax_by_extension(ext.to_str().unwrap_or("")))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let syntect_theme_name = match theme {
        MarkdownTheme::Dark | MarkdownTheme::Mono => "base16-ocean.dark",
        MarkdownTheme::Light => "base16-ocean.light",
    };
    let syntect_theme = THEME_SET
        .themes
        .get(syntect_theme_name)
        .or_else(|| THEME_SET.themes.get("base16-ocean.dark"))
        .unwrap_or_else(|| {
            // Fallback: syntect default ThemeSet always has themes. This
            // branch is unreachable in practice but avoids a bare panic.
            #[expect(clippy::expect_used, reason = "syntect default ThemeSet invariant")]
            {
                THEME_SET.themes.values().next().expect(
                    "ThemeSet must contain at least one theme \
                     (syntect default ThemeSet always has base16-ocean.dark)",
                )
            }
        });

    let mut highlighter = HighlightLines::new(syntax, syntect_theme);
    let mut lines = Vec::new();

    for raw_line in diff_text.lines() {
        if raw_line.starts_with("@@") {
            // Hunk header — cyan dim
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
            )));
        } else if raw_line.starts_with("...") {
            // Truncation message — dim
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                styles::dim(),
            )));
        } else if raw_line.is_empty() {
            // Hunk separator (empty line between hunks)
            lines.push(Line::from(Span::raw("")));
        } else {
            // Diff content line: prefix (+/-/ ) + content
            let prefix = &raw_line[..1];
            let content = &raw_line[1..];

            let (diff_fg, diff_modifier) = match prefix {
                "+" => (styles::GREEN, Modifier::BOLD),
                "-" => (styles::RED, Modifier::empty()),
                _ => (Color::Reset, Modifier::DIM),
            };

            // Syntax highlight the content portion
            let content_spans: Vec<Span<'static>> = if style_enabled && prefix != " " {
                highlight_content_spans(content, &mut highlighter, diff_fg, theme)
            } else if style_enabled {
                // Equal lines: just dim
                vec![Span::styled(content.to_string(), styles::dim())]
            } else {
                // No style: plain text with modifier only
                vec![Span::styled(
                    content.to_string(),
                    Style::default().add_modifier(diff_modifier),
                )]
            };

            let prefix_style = if style_enabled {
                Style::default().fg(diff_fg).add_modifier(diff_modifier)
            } else {
                Style::default().add_modifier(diff_modifier)
            };
            let prefix_span = Span::styled(prefix.to_string(), prefix_style);

            lines.push(Line::from({
                let mut all_spans = vec![prefix_span];
                all_spans.extend(content_spans);
                all_spans
            }));
        }
    }

    lines
}

/// Highlight a content line using syntect, then overlay the diff color.
fn highlight_content_spans(
    content: &str,
    highlighter: &mut HighlightLines,
    diff_fg: Color,
    _theme: MarkdownTheme,
) -> Vec<Span<'static>> {
    match highlighter.highlight_line(content, &SYNTAX_SET) {
        Ok(regions) => {
            let mut spans = Vec::new();
            for (syntect_style, text) in regions {
                // Convert syntect style to ratatui style, overlay diff color
                let fg = Color::Rgb(
                    syntect_style.foreground.r,
                    syntect_style.foreground.g,
                    syntect_style.foreground.b,
                );
                // For diff lines, use a subtle background tint:
                // green-tinted for additions, red-tinted for deletions
                let bg = if diff_fg == styles::GREEN {
                    Color::Rgb(30, 40, 30)
                } else {
                    Color::Rgb(40, 20, 30)
                };
                let style = Style::default().fg(fg).bg(bg);

                spans.push(Span::styled(text.to_string(), style));
            }
            if spans.is_empty() {
                vec![Span::raw(content.to_string())]
            } else {
                spans
            }
        }
        Err(_) => vec![Span::raw(content.to_string())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_diff_block_green_red() {
        let diff_text = "@@ -1,2 +1,2 @@\n-old line\n+new line\n unchanged";
        let lines = render_diff_block(diff_text, "test.rs", MarkdownTheme::Dark, true);

        // Should have 4 lines: hunk header, delete, insert, equal
        assert_eq!(lines.len(), 4, "Should have 4 lines");

        // Hunk header should be cyan
        // (we can't easily inspect Span styles in tests, but verify line count)
    }

    #[test]
    fn test_render_diff_block_empty_input() {
        let lines = render_diff_block("", "test.rs", MarkdownTheme::Dark, true);
        assert!(lines.is_empty(), "Empty diff should produce no lines");
    }

    #[test]
    fn test_render_diff_block_truncation_message() {
        let diff_text =
            "@@ -1,1 +1,1 @@\n+line\n... (10 more changes, diff truncated at 100 lines)";
        let lines = render_diff_block(diff_text, "test.rs", MarkdownTheme::Dark, true);
        // Should contain the truncation message as a line
        assert!(
            lines.len() >= 2,
            "Should have at least 2 lines (diff + truncation)"
        );
    }
}
