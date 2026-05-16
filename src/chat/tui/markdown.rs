//! Markdown rendering for the TUI
//!
//! This module provides markdown rendering using `tui-markdown` with
//! theme-aware styling. During LLM streaming, plain text is displayed
//! (fast, no parsing overhead). After completion, the full message is
//! re-rendered with `tui-markdown` for syntax highlighting, headers,
//! bold, code blocks, etc.
//!
//! # Table Support
//!
//! `tui-markdown` does not support markdown tables (it logs a warning and
//! silently drops the content). This module works around the limitation by
//! detecting table blocks in the markdown content and rendering them as
//! plain text with preserved line breaks, while the rest of the content
//! is rendered normally through `tui-markdown`.
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
use ratatui::text::{Line, Span, Text};
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

// ── Table detection and extraction ───────────────────────────────────
//
// `tui-markdown` silently drops markdown tables. We work around this by
// detecting table blocks in the content and rendering them as styled
// plain text (preserving pipe characters and line breaks), while
// rendering the rest through `tui-markdown` normally.

/// A segment of markdown content — either a regular block or a table.
#[derive(Debug)]
enum ContentSegment {
    /// Regular markdown content (rendered via tui-markdown)
    Markdown(String),
    /// Table block (rendered as styled plain text to preserve structure)
    Table(String),
}

/// Detect markdown table blocks in content and split into segments.
///
/// A markdown table is detected by:
/// 1. A line starting and ending with `|`
/// 2. Followed by a separator line (`|---|---|` or `|:---:|`)
/// 3. Followed by data rows starting and ending with `|`
///
/// Tables inside fenced code blocks are NOT detected as tables.
fn extract_table_segments(content: &str) -> Vec<ContentSegment> {
    let mut segments = Vec::new();
    let mut current_markdown = String::new();
    let mut in_code_block = false;
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // Track fenced code blocks — tables inside them are NOT tables
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            current_markdown.push_str(line);
            current_markdown.push('\n');
            continue;
        }

        if in_code_block {
            current_markdown.push_str(line);
            current_markdown.push('\n');
            continue;
        }

        // Try to detect the start of a table:
        // Current line looks like a table header row AND next line is a separator
        if is_table_row(trimmed)
            && lines
                .peek()
                .map(|next| is_table_separator(next.trim()))
                .unwrap_or(false)
        {
            // Flush accumulated markdown
            if !current_markdown.is_empty() {
                segments.push(ContentSegment::Markdown(std::mem::take(
                    &mut current_markdown,
                )));
            }

            // Collect all consecutive table lines
            let mut table_block = String::new();
            table_block.push_str(line);
            table_block.push('\n');

            // Consume the remaining table lines (separator + data rows)
            // Cannot use `for` loop because we break conditionally and need
            // the non-table line for the markdown accumulator.
            #[allow(clippy::while_let_on_iterator)] // Conditional break + line recovery
            while let Some(table_line) = lines.next() {
                let table_trimmed = table_line.trim();
                if is_table_row(table_trimmed) || is_table_separator(table_trimmed) {
                    table_block.push_str(table_line);
                    table_block.push('\n');
                } else if table_trimmed.is_empty() {
                    // Blank line after table — include it in the table block
                    // to preserve visual spacing
                    table_block.push('\n');
                    break;
                } else {
                    // Non-table line — push back conceptually by adding to markdown
                    // (We can't peek back, so add this line to the markdown accumulator)
                    current_markdown.push_str(table_line);
                    current_markdown.push('\n');
                    break;
                }
            }

            segments.push(ContentSegment::Table(table_block));
        } else {
            current_markdown.push_str(line);
            current_markdown.push('\n');
        }
    }

    // Flush remaining markdown
    if !current_markdown.is_empty() {
        segments.push(ContentSegment::Markdown(current_markdown));
    }

    // If we only have one markdown segment, return early (common case)
    if segments.len() == 1 {
        return segments;
    }

    segments
}

/// Check if a line looks like a table row (starts and ends with `|`)
fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 2
}

/// Check if a line is a table separator (`|---|---|`, `|:---:|:---:|`, etc.)
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return false;
    }
    // Inner cells must contain only `-`, `:`, spaces, or are empty
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    if inner.is_empty() {
        return false;
    }
    inner.split('|').all(|cell| {
        let cell_trimmed = cell.trim();
        cell_trimmed.is_empty()
            || cell_trimmed
                .chars()
                .all(|c| c == '-' || c == ':' || c == ' ')
    })
}

/// Style for table rows (dim cyan to visually distinguish from body text)
fn table_style(theme: MarkdownTheme) -> Style {
    match theme {
        MarkdownTheme::Dark => Style::default().fg(Color::Cyan),
        MarkdownTheme::Light => Style::default().fg(Color::Blue),
        MarkdownTheme::Mono => Style::default().add_modifier(Modifier::DIM),
    }
}

/// Render markdown content to a ratatui `Text`.
///
/// Applies the given theme's stylesheet for styling. Table blocks are
/// detected and rendered as styled plain text (preserving pipes and
/// line breaks), since `tui-markdown` silently drops tables.
///
/// For streaming content, use `render_plain_text` instead.
pub fn render_markdown<'a>(content: &'a str, theme: MarkdownTheme) -> Text<'a> {
    // Fast path: if no table structure detected, use tui-markdown directly
    if !content_contains_table(content) {
        return render_markdown_inner(content, theme);
    }

    // Slow path: extract table segments and render hybrid
    let segments = extract_table_segments(content);
    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for segment in segments {
        match segment {
            ContentSegment::Markdown(md) => {
                let rendered = render_markdown_inner_owned(&md, theme);
                all_lines.extend(rendered.lines);
            }
            ContentSegment::Table(table) => {
                let style = table_style(theme);
                for line in table.lines() {
                    if line.is_empty() {
                        all_lines.push(Line::raw(String::new()));
                    } else {
                        all_lines.push(Line::from(Span::styled(line.to_string(), style)));
                    }
                }
            }
        }
    }

    Text::from(all_lines)
}

/// Quick check: does the content contain a table-like structure?
///
/// Returns true if any line starts and ends with `|` AND is followed
/// by a separator-like line. This avoids the overhead of segment
/// extraction for the common case of no tables.
fn content_contains_table(content: &str) -> bool {
    let mut in_code_block = false;
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if is_table_row(trimmed)
            && lines
                .peek()
                .map(|next| is_table_separator(next.trim()))
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Internal markdown rendering via `tui-markdown` (no table handling).
fn render_markdown_inner<'a>(content: &'a str, theme: MarkdownTheme) -> Text<'a> {
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

/// Same as `render_markdown_inner` but returns owned `Text<'static>`.
///
/// Used when we need to combine multiple rendered segments into a
/// single `Text` — each segment is owned independently.
fn render_markdown_inner_owned(content: &str, theme: MarkdownTheme) -> Text<'static> {
    let text = render_markdown_inner(content, theme);
    // Convert borrowed Text<'a> to owned Text<'static>
    let owned_lines: Vec<Line<'static>> = text
        .lines
        .into_iter()
        .map(|line| {
            let owned_spans: Vec<Span<'static>> = line
                .spans
                .into_iter()
                .map(|span| Span::styled(span.content.into_owned(), span.style))
                .collect();
            Line::from(owned_spans)
        })
        .collect();
    Text::from(owned_lines)
}

/// Render plain text for streaming content
///
/// Used during LLM streaming when we want fast display
/// without markdown parsing overhead. Once the response completes,
/// the message is re-rendered with `render_markdown`.
#[allow(dead_code)] // PR3: Will be used for streaming text rendering in TUI
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

    // ── Table detection tests ──────────────────────────────────────

    #[test]
    fn test_is_table_row() {
        assert!(is_table_row("| A | B |"));
        assert!(is_table_row("| --- | --- |"));
        assert!(is_table_row("|:---:|:---:|"));
        assert!(!is_table_row("hello world"));
        assert!(!is_table_row("|")); // Too short
        assert!(!is_table_row(""));
    }

    #[test]
    fn test_is_table_separator() {
        assert!(is_table_separator("|---|---|"));
        assert!(is_table_separator("| --- | --- |"));
        assert!(is_table_separator("|:---:|:---:|"));
        assert!(is_table_separator("|:---|---:|"));
        assert!(!is_table_separator("| A | B |"));
        assert!(!is_table_separator("hello"));
        assert!(!is_table_separator("|")); // Too short
    }

    #[test]
    fn test_extract_table_segments_no_table() {
        let content = "Hello world\n\nThis is a paragraph.";
        let segments = extract_table_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            ContentSegment::Markdown(md) => assert!(md.contains("Hello world")),
            ContentSegment::Table(_) => panic!("Expected Markdown segment"),
        }
    }

    #[test]
    fn test_extract_table_segments_simple_table() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let segments = extract_table_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            ContentSegment::Table(table) => {
                assert!(table.contains("| Name | Value |"));
                assert!(table.contains("| Foo  | 42    |"));
            }
            ContentSegment::Markdown(_) => panic!("Expected Table segment"),
        }
    }

    #[test]
    fn test_extract_table_segments_mixed_content() {
        let content = "Here is some text:\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.";
        let segments = extract_table_segments(content);
        assert_eq!(segments.len(), 3); // markdown, table, markdown
        match &segments[0] {
            ContentSegment::Markdown(md) => assert!(md.contains("Here is some text")),
            _ => panic!("Expected Markdown segment first"),
        }
        match &segments[1] {
            ContentSegment::Table(table) => {
                assert!(table.contains("| A | B |"));
                assert!(table.contains("| 1 | 2 |"));
            }
            _ => panic!("Expected Table segment second"),
        }
        match &segments[2] {
            ContentSegment::Markdown(md) => assert!(md.contains("More text")),
            _ => panic!("Expected Markdown segment third"),
        }
    }

    #[test]
    fn test_extract_table_segments_table_in_code_block() {
        let content = "```\n| A | B |\n|---|---|\n| 1 | 2 |\n```\n\nAfter.";
        let segments = extract_table_segments(content);
        // Table is inside code block — should NOT be detected as a table
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            ContentSegment::Markdown(md) => {
                assert!(md.contains("| A | B |"));
                assert!(md.contains("```"));
            }
            ContentSegment::Table(_) => panic!("Table inside code block should not be detected"),
        }
    }

    #[test]
    fn test_render_markdown_table_not_dropped() {
        let content = "# Results\n\n| Name | Value |\n|------|-------|\n| Foo  | 42    |\n\nDone.";
        let text = render_markdown(content, MarkdownTheme::Dark);
        // The rendered text must contain the table content (not silently dropped)
        let rendered_str: String = text
            .lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(
            rendered_str.contains("Name"),
            "Table header 'Name' should be preserved"
        );
        assert!(
            rendered_str.contains("Foo"),
            "Table data 'Foo' should be preserved"
        );
        assert!(
            rendered_str.contains("42"),
            "Table data '42' should be preserved"
        );
    }

    #[test]
    fn test_render_markdown_table_lines_preserved() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let text = render_markdown(content, MarkdownTheme::Dark);
        // Each table row should be on its own line
        // (4 rows = 4 lines minimum, possibly more from trailing newline)
        assert!(
            text.lines.len() >= 4,
            "Table should have at least 4 lines, got {}",
            text.lines.len()
        );
    }
}
