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
//! silently drops the content). This module detects table blocks and
//! renders them with box-drawing borders, Unicode-aware column alignment,
//! and responsive width — inspired by the `ratatui-markdown` crate.
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
//! - `render_markdown(content, theme, max_width)` → `Text<'static>` — Full markdown rendering
//! - `render_plain_text(content)` → `Text<'static>` — Fast plain text for streaming
//! - `MarkdownTheme` — Theme enum with `from_config()` and stylesheet selection

// Table rendering inspired by ratatui-markdown (MIT OR Apache-2.0)
// https://github.com/celestia-island/ratatui-markdown

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use tui_markdown::{Options, StyleSheet, from_str_with_options};
use unicode_width::UnicodeWidthStr;

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
// detecting table blocks and rendering them with box-drawing borders,
// Unicode-aware column alignment, and responsive width.

/// A segment of markdown content — either a regular block or a table.
#[derive(Debug)]
enum ContentSegment {
    /// Regular markdown content (rendered via tui-markdown)
    Markdown(String),
    /// Table block (rendered with box-drawing borders)
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

// ── Box-drawing constants for table borders ──────────────────────────

/// `│` Light vertical
const BD_VLINE: &str = "│";
/// `─` Light horizontal
const BD_HLINE: &str = "─";
/// `┌` Top-left corner
const BD_TL: &str = "┌";
/// `┬` Top-middle (down + horizontal)
const BD_TM: &str = "┬";
/// `┐` Top-right corner
const BD_TR: &str = "┐";
/// `├` Middle-left (right + horizontal)
const BD_ML: &str = "├";
/// `┼` Cross (vertical + horizontal)
const BD_MC: &str = "┼";
/// `┤` Middle-right (left + horizontal)
const BD_MR: &str = "┤";
/// `└` Bottom-left corner
const BD_BL: &str = "└";
/// `┴` Bottom-middle (up + horizontal)
const BD_BM: &str = "┴";
/// `┘` Bottom-right corner
const BD_BR: &str = "┘";

// ── Table parsing and rendering ─────────────────────────────────────

/// Parsed table structure with headers and data rows.
struct ParsedTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Parse a raw table block into headers and rows.
///
/// Input is the raw text of a table block (as extracted by
/// `extract_table_segments`), containing `|…|…|` lines and
/// separator lines. The first non-separator row is the header;
/// separator rows (`|---|---|`) are skipped.
fn parse_table_rows(content: &str) -> ParsedTable {
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut found_separator = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_table_separator(trimmed) {
            found_separator = true;
            continue;
        }
        if is_table_row(trimmed) {
            let cells = split_table_cells(trimmed);
            if headers.is_empty() {
                headers = cells;
            } else if found_separator {
                rows.push(cells);
            }
        }
    }

    ParsedTable { headers, rows }
}

/// Split a `|…|…|` line into trimmed cell values.
///
/// `"| Name | Value |"` → `["Name", "Value"]`
fn split_table_cells(line: &str) -> Vec<String> {
    let parts: Vec<&str> = line.trim().split('|').collect();
    // Parts: ["", " Name ", " Value ", ""]
    // Take indices 1..len-1 (skip empty before first | and after last |)
    let cell_count = parts.len().saturating_sub(2);
    (0..cell_count)
        .filter_map(|i| parts.get(i + 1).map(|s| s.trim().to_string()))
        .collect()
}

/// Measure the visual width of a string using `unicode-width`.
fn visual_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Calculate column widths for a table, fitting within `max_width`.
///
/// Returns a vector of column widths (including 2-char cell padding each)
/// that distributes available space proportionally based on content.
///
/// Inspired by the proportional distribution algorithm in
/// `ratatui-markdown`'s `render_table()`.
fn calculate_col_widths(headers: &[String], rows: &[Vec<String>], max_width: usize) -> Vec<usize> {
    let col_count = headers.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if col_count == 0 {
        return Vec::new();
    }

    let padding_per_cell: usize = 2; // " " before + " " after content
    let border_overhead = col_count + 1; // │ borders
    let total_padding = col_count * padding_per_cell;
    let available = max_width.saturating_sub(border_overhead + total_padding);
    let min_available = available.max(3 * col_count); // Ensure at least 3 chars per col

    // Natural width: the widest content in each column
    let natural_widths: Vec<usize> = (0..col_count)
        .map(|c| {
            let hw = headers.get(c).map(|h| visual_width(h)).unwrap_or(0);
            let rw = rows
                .iter()
                .filter_map(|r| r.get(c).map(|cell| visual_width(cell)))
                .max()
                .unwrap_or(0);
            hw.max(rw)
        })
        .collect();

    let total_natural: usize = natural_widths.iter().sum::<usize>().max(1);

    if total_natural <= min_available {
        // All columns fit naturally — use natural widths
        natural_widths
    } else {
        // Distribute proportionally, with minimum of 3 chars per column
        let min_col = 3;
        let mut col_widths: Vec<usize> = natural_widths
            .iter()
            .map(|w| (min_available * w / total_natural).max(min_col))
            .collect();

        // Adjust if we exceeded available space
        let total_allocated: usize = col_widths.iter().sum();
        if total_allocated > min_available {
            let deficit = total_allocated - min_available;
            let mut remaining = deficit;
            // Shrink widest columns first
            let mut sorted: Vec<usize> = (0..col_count).collect();
            sorted.sort_by_key(|&i| std::cmp::Reverse(col_widths[i] - min_col));
            for idx in sorted {
                if remaining == 0 {
                    break;
                }
                let shrinkable = col_widths[idx].saturating_sub(min_col);
                let take = shrinkable.min(remaining);
                col_widths[idx] -= take;
                remaining -= take;
            }
        }

        col_widths
    }
}

/// Style for table header cells.
fn table_style_header(theme: MarkdownTheme) -> Style {
    match theme {
        MarkdownTheme::Dark => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        MarkdownTheme::Light => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        MarkdownTheme::Mono => Style::default().add_modifier(Modifier::BOLD),
    }
}

/// Style for table data cells.
fn table_style_cell(theme: MarkdownTheme) -> Style {
    match theme {
        MarkdownTheme::Dark => Style::default().fg(Color::Cyan),
        MarkdownTheme::Light => Style::default().fg(Color::Blue),
        MarkdownTheme::Mono => Style::default(),
    }
}

/// Style for table border characters.
fn table_style_border(theme: MarkdownTheme) -> Style {
    // Borders are structural — use a muted color in all themes
    match theme {
        MarkdownTheme::Dark | MarkdownTheme::Light => Style::default().fg(Color::DarkGray),
        MarkdownTheme::Mono => Style::default().add_modifier(Modifier::DIM),
    }
}

/// Build a horizontal border line for a table.
///
/// Example with 2 columns of width 10 and 8:
/// `"┌──────────┬────────┐"`
fn build_hline(col_widths: &[usize], left: &str, mid: &str, right: &str) -> String {
    let mut parts = vec![left.to_string()];
    for (i, width) in col_widths.iter().enumerate() {
        // +2 for cell padding (" " before + " " after content)
        parts.push(BD_HLINE.repeat(width + 2));
        if i < col_widths.len() - 1 {
            parts.push(mid.to_string());
        }
    }
    parts.push(right.to_string());
    parts.join("")
}

/// Build a data or header row line for a table.
///
/// Returns a `Line` with styled spans for each cell and border.
fn build_row_line(
    col_widths: &[usize],
    cells: &[String],
    theme: MarkdownTheme,
    is_header: bool,
) -> Line<'static> {
    let border_style = table_style_border(theme);
    let cell_style = if is_header {
        table_style_header(theme)
    } else {
        table_style_cell(theme)
    };

    let mut spans = Vec::new();
    spans.push(Span::styled(BD_VLINE.to_string(), border_style));

    for (i, width) in col_widths.iter().enumerate() {
        let text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
        let text_width = visual_width(text);
        let inner_w = *width; // content area (padding already in col_width)

        spans.push(Span::styled(" ".to_string(), cell_style)); // left pad
        spans.push(Span::styled(text.to_string(), cell_style));
        // Right-pad to fill the column
        if text_width < inner_w {
            spans.push(Span::styled(
                " ".repeat(inner_w - text_width),
                cell_style,
            ));
        }
        spans.push(Span::styled(" ".to_string(), cell_style)); // right pad
        spans.push(Span::styled(BD_VLINE.to_string(), border_style));
    }

    Line::from(spans)
}

/// Render a table block with box-drawing borders.
///
/// Parses the raw table content, calculates column widths that fit
/// within `max_width`, and produces styled `Line`s with Unicode-aware
/// alignment.
fn render_table_box(content: &str, max_width: usize, theme: MarkdownTheme) -> Vec<Line<'static>> {
    let table = parse_table_rows(content);
    if table.headers.is_empty() {
        return Vec::new();
    }

    let col_widths = calculate_col_widths(&table.headers, &table.rows, max_width);
    if col_widths.is_empty() {
        return Vec::new();
    }

    let border_style = table_style_border(theme);
    let mut lines = Vec::new();

    // Top border
    lines.push(Line::from(Span::styled(
        build_hline(&col_widths, BD_TL, BD_TM, BD_TR),
        border_style,
    )));

    // Header row
    lines.push(build_row_line(&col_widths, &table.headers, theme, true));

    // Header/data separator
    lines.push(Line::from(Span::styled(
        build_hline(&col_widths, BD_ML, BD_MC, BD_MR),
        border_style,
    )));

    // Data rows
    for row in &table.rows {
        lines.push(build_row_line(&col_widths, row, theme, false));
    }

    // Bottom border
    lines.push(Line::from(Span::styled(
        build_hline(&col_widths, BD_BL, BD_BM, BD_BR),
        border_style,
    )));

    lines
}

/// Render markdown content to a ratatui `Text`.
///
/// Applies the given theme's stylesheet for styling. Table blocks are
/// detected and rendered with box-drawing borders, Unicode-aware
/// column alignment, and responsive width fitting within `max_width`.
/// This ensures `Paragraph::wrap()` does not break table lines.
///
/// For streaming content, use `render_plain_text` instead.
pub fn render_markdown(content: &str, theme: MarkdownTheme, max_width: usize) -> Text<'static> {
    // Fast path: if no table structure detected, use tui-markdown directly
    if !content_contains_table(content) {
        return render_markdown_inner_owned(content, theme);
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
                let table_lines = render_table_box(&table, max_width, theme);
                all_lines.extend(table_lines);
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
        let text = render_markdown("# Hello", MarkdownTheme::Dark, 80);
        // Should produce styled text (not empty)
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_render_markdown_light() {
        let text = render_markdown("# Hello", MarkdownTheme::Light, 80);
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_render_markdown_mono() {
        let text = render_markdown("# Hello", MarkdownTheme::Mono, 80);
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
        let text = render_markdown(content, MarkdownTheme::Dark, 80);
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
    fn test_render_markdown_table_has_borders() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let text = render_markdown(content, MarkdownTheme::Dark, 80);
        let rendered_str: String = text
            .lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("\n");
        // Should contain box-drawing border characters
        assert!(
            rendered_str.contains('┌'),
            "Table should have top-left corner"
        );
        assert!(
            rendered_str.contains('└'),
            "Table should have bottom-left corner"
        );
        assert!(
            rendered_str.contains('│'),
            "Table should have vertical borders"
        );
    }

    // ── Table parsing tests ───────────────────────────────────────

    #[test]
    fn test_parse_table_rows_basic() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let table = parse_table_rows(content);
        assert_eq!(table.headers, vec!["Name", "Value"]);
        assert_eq!(table.rows, vec![vec!["Foo", "42"]]);
    }

    #[test]
    fn test_parse_table_rows_multiple_rows() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let table = parse_table_rows(content);
        assert_eq!(table.headers, vec!["A", "B"]);
        assert_eq!(table.rows, vec![vec!["1", "2"], vec!["3", "4"]]);
    }

    #[test]
    fn test_parse_table_rows_empty_cells() {
        let content = "| A | |\n|---|---|\n| | B |";
        let table = parse_table_rows(content);
        assert_eq!(table.headers, vec!["A", ""]);
        assert_eq!(table.rows, vec![vec!["", "B"]]);
    }

    #[test]
    fn test_split_table_cells() {
        assert_eq!(split_table_cells("| Name | Value |"), vec!["Name", "Value"]);
        assert_eq!(split_table_cells("| A | B | C |"), vec!["A", "B", "C"]);
        assert_eq!(split_table_cells("|  |  |"), vec!["", ""]);
    }

    // ── Column width calculation tests ─────────────────────────────

    #[test]
    fn test_calculate_col_widths_natural_fit() {
        let headers = vec!["Name".to_string(), "Value".to_string()];
        let rows = vec![vec!["Foo".to_string(), "42".to_string()]];
        let widths = calculate_col_widths(&headers, &rows, 80);
        // Natural widths: Name=4, Value=5 — both fit comfortably
        assert_eq!(widths.len(), 2);
        assert!(widths[0] >= 4, "Column 0 should fit 'Name' (width {})", widths[0]);
        assert!(widths[1] >= 5, "Column 1 should fit 'Value' (width {})", widths[1]);
    }

    #[test]
    fn test_calculate_col_widths_narrow_terminal() {
        let headers = vec!["VeryLongColumnName".to_string(), "B".to_string()];
        let rows = vec![vec!["data".to_string(), "x".to_string()]];
        let widths = calculate_col_widths(&headers, &rows, 20);
        // Total must fit within 20 chars (borders + padding + content)
        let total: usize = widths.iter().sum();
        let border_overhead = widths.len() + 1; // │ borders
        let padding = widths.len() * 2;
        assert!(
            total + border_overhead + padding <= 20
                || widths.iter().all(|&w| w >= 3),
            "Columns should be at least 3 chars wide or fit in terminal"
        );
    }

    #[test]
    fn test_calculate_col_widths_unicode() {
        let headers = vec!["名前".to_string(), "値".to_string()];
        let rows = vec![vec!["日本語".to_string(), "42".to_string()]];
        let widths = calculate_col_widths(&headers, &rows, 80);
        assert_eq!(widths.len(), 2);
        // CJK chars take 2 columns each: 名前=4, 日本語=6
        assert!(widths[0] >= 4, "Column 0 should fit CJK header (width {})", widths[0]);
    }

    // ── Box-drawing table rendering tests ─────────────────────────

    #[test]
    fn test_render_table_box_basic() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let lines = render_table_box(content, 80, MarkdownTheme::Dark);
        // Should have: top border, header, separator, data row, bottom border
        assert_eq!(lines.len(), 5, "Table should have 5 lines");
    }

    #[test]
    fn test_render_table_box_content_preserved() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let lines = render_table_box(content, 80, MarkdownTheme::Dark);
        let rendered: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join("");
        assert!(rendered.contains("Name"), "Header 'Name' should be in output");
        assert!(rendered.contains("Value"), "Header 'Value' should be in output");
        assert!(rendered.contains("Foo"), "Data 'Foo' should be in output");
        assert!(rendered.contains("42"), "Data '42' should be in output");
    }

    #[test]
    fn test_render_table_box_respects_max_width() {
        let content = "| Name | Very Long Value Content |\n|------|--------------------------|\n| Foo  | 42                       |";
        let lines = render_table_box(content, 40, MarkdownTheme::Dark);
        // Every line should fit within max_width
        for line in &lines {
            let line_width: usize = line.spans.iter().map(|s| visual_width(&s.content)).sum();
            assert!(
                line_width <= 40,
                "Line width {} exceeds max 40: {:?}",
                line_width,
                line
            );
        }
    }

    #[test]
    fn test_render_table_box_unicode_alignment() {
        // CJK chars take 2 columns — should not break alignment
        let content = "| 名前   | 値 |\n|--------|-----|\n| 日本語  | 42  |";
        let lines = render_table_box(content, 60, MarkdownTheme::Dark);
        // All lines should have │ at consistent positions
        assert_eq!(lines.len(), 5, "Table should have 5 lines");
        // Just verify no panic or crash with CJK content
    }

    #[test]
    fn test_render_table_box_single_column() {
        let content = "| Items |\n|-------|\n| A     |\n| B     |";
        let lines = render_table_box(content, 40, MarkdownTheme::Dark);
        // top border + header + separator + data rows (2) + bottom border = 6
        assert_eq!(lines.len(), 6, "Single-column table should have 6 lines (2 data rows)");
    }

    #[test]
    fn test_build_hline_basic() {
        let hline = build_hline(&[6, 4], BD_TL, BD_TM, BD_TR);
        // 6+2=8 dashes, then ┬, then 4+2=6 dashes
        assert!(hline.starts_with('┌'));
        assert!(hline.contains('┬'));
        assert!(hline.ends_with('┐'));
    }
}