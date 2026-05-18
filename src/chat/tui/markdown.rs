//! Markdown rendering for the TUI
//!
//! This module provides markdown rendering using `tui-markdown` with
//! theme-aware styling. Markdown is rendered incrementally during LLM
//! streaming (like Thinking blocks) and once for completed messages.
//!
//! # Table Support
//!
//! `tui-markdown` does not support markdown tables (it logs a warning and
//! silently drops the content). This module detects table blocks and
//! renders them with box-drawing borders, Unicode-aware column alignment,
//! intelligent rigid/elastic column sizing, cell word-wrapping, and
//! responsive width — inspired by the `ratatui-markdown` crate.
//!
//! # Themes
//!
//! Three themes map from the existing `DisplaySettings.skin` config:
//! - `dark`: Catppuccin Mocha (dark terminal backgrounds)
//! - `light`: Catppuccin Latte (light terminal backgrounds)
//! - `mono`: Monochrome (no colors, bold/italic for code blocks)
//!
//! # API
//!
//! - `render_markdown(content, theme, max_width)` → `Text<'static>` — Full markdown rendering
//! - `MarkdownTheme` — Theme enum with `from_config()` and stylesheet selection

// Table rendering inspired by ratatui-markdown (MIT OR Apache-2.0)
// https://github.com/celestia-island/ratatui-markdown

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use tui_markdown::{Options, StyleSheet, from_str_with_options};
use unicode_width::UnicodeWidthStr;

use super::wrap::wrap_line;

// ── Catppuccin color palette (MIT License, https://catppuccin.com) ────
//
// Only the colors used by the TUI markdown renderer are included here.
// Catppuccin provides 26 named colors per flavor; we define the subset
// needed for code blocks and general markdown styling.
//
// Color names and hex values are taken directly from the Catppuccin
// palette specification. See https://catppuccin.com/palette/ for the
// full palette.

// Mocha (dark flavor) — used by DarkStyleSheet
const MOCHA_TEXT: Color = Color::Rgb(205, 214, 244); // #cdd6f4
const MOCHA_SURFACE0: Color = Color::Rgb(49, 50, 68); // #313244

// Latte (light flavor) — used by LightStyleSheet
const LATTE_TEXT: Color = Color::Rgb(76, 79, 105); // #4c4f69
const LATTE_SURFACE0: Color = Color::Rgb(204, 208, 218); // #ccd0da

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
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            3 => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        }
    }

    // Catppuccin Mocha: text on Surface0 for code blocks
    fn code(&self) -> Style {
        Style::default().fg(MOCHA_TEXT).bg(MOCHA_SURFACE0)
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

/// Light theme stylesheet (Catppuccin Latte, optimized for light terminal backgrounds)
#[derive(Clone, Copy)]
struct LightStyleSheet;

impl StyleSheet for LightStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            3 => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        }
    }

    // Catppuccin Latte: text on Surface0 for code blocks
    fn code(&self) -> Style {
        Style::default().fg(LATTE_TEXT).bg(LATTE_SURFACE0)
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

/// Monochrome theme stylesheet (no colors, only bold/italic/underline/dim)
#[derive(Clone, Copy)]
struct MonoStyleSheet;

impl StyleSheet for MonoStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 | 2 => Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            _ => Style::default().add_modifier(Modifier::BOLD),
        }
    }

    // Monochrome code: bold only, no colors, no background, no REVERSED.
    // Code blocks are visually distinct by bold text — the same style as
    // inline code. The post-processor strips all RGB colors from code block
    // Spans and Line.style to ensure true monochrome rendering.
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
// Unicode-aware column alignment, cell word-wrapping, and responsive width.

/// A segment of markdown content — regular text, a table, or a Mermaid diagram.
#[derive(Debug)]
enum ContentSegment {
    /// Regular markdown content (rendered via tui-markdown)
    Markdown(String),
    /// Table block (rendered with box-drawing borders)
    Table(String),
    /// Mermaid diagram block (rendered as Unicode box-drawing text)
    #[cfg(feature = "mermaid")]
    Mermaid(String),
}

/// Detect markdown table blocks and Mermaid blocks in content and split into segments.
///
/// Tables and Mermaid diagrams inside fenced code blocks are NOT detected as
/// special segments — they remain as regular Markdown content.
fn extract_content_segments(content: &str) -> Vec<ContentSegment> {
    let mut segments = Vec::new();
    let mut current_markdown = String::new();
    let mut in_code_block = false;
    #[cfg(feature = "mermaid")]
    let mut in_mermaid_block = false;
    #[cfg(feature = "mermaid")]
    let mut mermaid_content = String::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        // Track fenced code blocks
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();

            #[cfg(feature = "mermaid")]
            if in_mermaid_block {
                in_mermaid_block = false;
                segments.push(ContentSegment::Mermaid(mermaid_content.clone()));
                mermaid_content.clear();
                continue;
            }

            if in_code_block {
                in_code_block = false;
                current_markdown.push_str(line);
                current_markdown.push('\n');
                continue;
            }

            // Starting a new code block — check if it's Mermaid
            #[cfg(feature = "mermaid")]
            if lang.starts_with("mermaid") {
                if !current_markdown.is_empty() {
                    segments.push(ContentSegment::Markdown(std::mem::take(
                        &mut current_markdown,
                    )));
                }
                in_mermaid_block = true;
                continue;
            }

            in_code_block = true;
            current_markdown.push_str(line);
            current_markdown.push('\n');
            continue;
        }

        #[cfg(feature = "mermaid")]
        if in_mermaid_block {
            mermaid_content.push_str(line);
            mermaid_content.push('\n');
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
            #[allow(clippy::while_let_on_iterator)] // Conditional break + line recovery
            while let Some(table_line) = lines.next() {
                let table_trimmed = table_line.trim();
                if is_table_row(table_trimmed) || is_table_separator(table_trimmed) {
                    table_block.push_str(table_line);
                    table_block.push('\n');
                } else if table_trimmed.is_empty() {
                    table_block.push('\n');
                    break;
                } else {
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

    // Flush remaining content
    #[cfg(feature = "mermaid")]
    if in_mermaid_block && !mermaid_content.is_empty() {
        segments.push(ContentSegment::Mermaid(mermaid_content));
    }
    if !current_markdown.is_empty() {
        segments.push(ContentSegment::Markdown(current_markdown));
    }

    if segments.len() == 1 {
        return segments;
    }

    segments
}

/// Check if a line looks like a table row (starts and ends with `|`).
fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 2
}

/// Check if a line is a table separator (`|---|---|`, `|:---:|:---:|`, etc.)
fn is_table_separator(line: &str) -> bool {
    parse_separator_line(line).is_some()
}

// ── Column alignment ────────────────────────────────────────────────

/// Column alignment extracted from markdown separator syntax.
///
/// Markdown uses colons in the separator row to indicate alignment:
/// - `:---` or `---` → left-aligned (default)
/// - `---:` → right-aligned
/// - `:---:` → center-aligned
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlign {
    /// `---` or `:---` — left-aligned (default)
    Left,
    /// `---:` — right-aligned
    Right,
    /// `:---:` — center-aligned
    Center,
}

/// Parse a separator line and extract alignment hints.
///
/// Returns `None` if the line is not a valid table separator.
/// Returns `Some(Vec<ColumnAlign>)` with one alignment per column.
///
/// # Examples
///
/// ```ignore
/// parse_separator_line("|---|---|")       → Some([Left, Left])
/// parse_separator_line("|:---:|---:|")    → Some([Center, Right])
/// parse_separator_line("| A | B |")       → None (not a separator)
/// ```
fn parse_separator_line(line: &str) -> Option<Vec<ColumnAlign>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }

    // Split into cells between `|` delimiters
    let cells = split_table_cells(trimmed);
    if cells.is_empty() {
        return None;
    }

    // Each cell must contain only `-`, `:`, or spaces.
    // At least one cell must contain `-` (otherwise it's not a separator).
    let mut has_dash = false;
    let mut aligns = Vec::with_capacity(cells.len());
    for cell in &cells {
        let cell_trimmed = cell.trim();
        if cell_trimmed.is_empty() {
            // Empty cell in separator — treated as default left
            aligns.push(ColumnAlign::Left);
            continue;
        }
        // Must contain only `-`, `:`, spaces
        if !cell_trimmed
            .chars()
            .all(|c| c == '-' || c == ':' || c == ' ')
        {
            return None; // Not a separator cell
        }
        if cell_trimmed.contains('-') {
            has_dash = true;
        }
        // Determine alignment from colon positions
        let starts_with_colon = cell_trimmed.starts_with(':');
        let ends_with_colon = cell_trimmed.ends_with(':');
        let align = match (starts_with_colon, ends_with_colon) {
            (true, true) => ColumnAlign::Center,
            (false, true) => ColumnAlign::Right,
            _ => ColumnAlign::Left,
        };
        aligns.push(align);
    }

    // A valid separator must have at least one `-` somewhere
    if !has_dash {
        return None;
    }

    Some(aligns)
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

/// Parsed table structure with headers, data rows, and column alignments.
struct ParsedTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    aligns: Vec<ColumnAlign>,
}

/// Parse a raw table block into headers, rows, and column alignments.
///
/// Input is the raw text of a table block (as extracted by
/// `extract_table_segments`), containing `|…|…|` lines and
/// separator lines. The first non-separator row is the header;
/// separator rows (`|---|---|`) are parsed for alignment hints.
fn parse_table_rows(content: &str) -> ParsedTable {
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut aligns: Vec<ColumnAlign> = Vec::new();
    let mut found_separator = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(parsed_aligns) = parse_separator_line(trimmed) {
            aligns = parsed_aligns;
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

    // Default alignment: Left for all columns if no separator found
    if aligns.is_empty() && !headers.is_empty() {
        aligns = vec![ColumnAlign::Left; headers.len()];
    }

    ParsedTable {
        headers,
        rows,
        aligns,
    }
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

// ── Intelligent column width calculation ─────────────────────────────

/// Threshold for classifying a column as "rigid" (non-wrappable).
///
/// A column is rigid if **all** of its cells (header + data) have
/// `visual_width ≤ RIGID_THRESHOLD`. Rigid columns receive their
/// natural width exactly and never wrap — they are "identifier" columns
/// (IDs, short labels, numbers).
const RIGID_THRESHOLD: usize = 6;

/// Calculate column widths for a table, fitting within `max_width`.
///
/// Uses an intelligent rigid/elastic classification:
/// - **Rigid columns**: natural width ≤ threshold (short content like IDs,
///   numbers). Allocated their exact natural width — never wrap.
/// - **Elastic columns**: natural width > threshold (long content like
///   descriptions, text). Receive the remaining space after rigid
///   allocation — may wrap their cell content.
///
/// If the terminal is too narrow to fit even the rigid columns, all
/// columns are encolhidas equally down to a minimum of 3 chars.
fn calculate_col_widths(
    headers: &[String],
    rows: &[Vec<String>],
    aligns: &[ColumnAlign],
    max_width: usize,
) -> Vec<usize> {
    let _ = aligns; // Used by caller for cell text alignment, not for width calculation
    let col_count = headers
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if col_count == 0 {
        return Vec::new();
    }

    let padding_per_cell: usize = 2; // " " before + " " after content
    let border_overhead = col_count + 1; // │ borders
    let total_padding = col_count * padding_per_cell;
    let available = max_width.saturating_sub(border_overhead + total_padding);

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

    // Classify each column as rigid or elastic
    let is_rigid: Vec<bool> = (0..col_count)
        .map(|c| {
            let natural = natural_widths[c];
            // A column is rigid if all cells (header + data) are ≤ threshold
            let header_fits =
                headers.get(c).map(|h| visual_width(h)).unwrap_or(0) <= RIGID_THRESHOLD;
            let data_fits = rows
                .iter()
                .all(|r| r.get(c).map(|cell| visual_width(cell)).unwrap_or(0) <= RIGID_THRESHOLD);
            natural <= RIGID_THRESHOLD && header_fits && data_fits
        })
        .collect();

    let total_natural: usize = natural_widths.iter().sum::<usize>().max(1);

    if total_natural <= available {
        // All columns fit naturally — use natural widths
        return natural_widths;
    }

    // Need to shrink: allocate rigids first, then distribute remaining to elastics
    let min_col = 3;
    let mut col_widths = vec![0usize; col_count];

    // Step 1: Allocate rigid columns their natural width
    let mut rigid_total = 0usize;
    for (i, &rigid) in is_rigid.iter().enumerate() {
        if rigid {
            col_widths[i] = natural_widths[i];
            rigid_total += natural_widths[i];
        }
    }

    // Step 2: Distribute remaining space among elastic columns
    let elastic_count = is_rigid.iter().filter(|&&r| !r).count();
    if elastic_count == 0 {
        // All columns are rigid but don't fit — shrink proportionally
        return distribute_proportionally(&natural_widths, available, min_col, col_count);
    }

    let remaining_for_elastics = available.saturating_sub(rigid_total);
    let min_elastic_total = min_col * elastic_count;

    if remaining_for_elastics < min_elastic_total {
        // Not enough space even for min elastic + rigids.
        // Fall back to proportional distribution for ALL columns.
        return distribute_proportionally(&natural_widths, available, min_col, col_count);
    }

    // Distribute remaining proportionally among elastic columns
    let elastic_naturals: Vec<(usize, usize)> = is_rigid
        .iter()
        .enumerate()
        .filter(|&(_, &r)| !r)
        .map(|(i, _)| (i, natural_widths[i]))
        .collect();

    let elastic_natural_total: usize = elastic_naturals
        .iter()
        .map(|(_, w)| *w)
        .sum::<usize>()
        .max(1);

    for &(i, natural) in &elastic_naturals {
        col_widths[i] = (remaining_for_elastics * natural / elastic_natural_total).max(min_col);
    }

    // Step 3: Redistribute any unused space from rigids that got less
    // than their allocation (shouldn't happen, but for safety)
    let total_allocated: usize = col_widths.iter().sum();
    if total_allocated > available {
        let deficit = total_allocated - available;
        let mut remaining_deficit = deficit;
        // Shrink elastic columns first (they can wrap)
        let mut sorted_elastics: Vec<usize> = elastic_naturals
            .iter()
            .filter(|&&(i, _)| col_widths[i] > min_col)
            .map(|&(i, _)| i)
            .collect();
        sorted_elastics.sort_by_key(|&i| std::cmp::Reverse(col_widths[i] - min_col));
        for idx in sorted_elastics {
            if remaining_deficit == 0 {
                break;
            }
            let shrinkable = col_widths[idx].saturating_sub(min_col);
            let take = shrinkable.min(remaining_deficit);
            col_widths[idx] -= take;
            remaining_deficit -= take;
        }
        // If still over budget, shrink rigids too
        if remaining_deficit > 0 {
            let mut sorted_rigids: Vec<usize> = is_rigid
                .iter()
                .enumerate()
                .filter(|&(i, &r)| r && col_widths[i] > min_col)
                .map(|(i, _)| i)
                .collect();
            sorted_rigids.sort_by_key(|&i| std::cmp::Reverse(col_widths[i] - min_col));
            for idx in sorted_rigids {
                if remaining_deficit == 0 {
                    break;
                }
                let shrinkable = col_widths[idx].saturating_sub(min_col);
                let take = shrinkable.min(remaining_deficit);
                col_widths[idx] -= take;
                remaining_deficit -= take;
            }
        }
    }

    // Step 4: Redistribute any surplus (rigids that use less than allocated)
    let total_final: usize = col_widths.iter().sum();
    if total_final < available {
        let surplus = available - total_final;
        // Give extra space to elastic columns proportionally
        let elastic_total_current: usize = col_widths
            .iter()
            .enumerate()
            .filter(|(i, _)| !is_rigid[*i])
            .map(|(_, &w)| w)
            .sum::<usize>()
            .max(1);
        for (i, &rigid) in is_rigid.iter().enumerate() {
            if !rigid {
                let share = surplus * col_widths[i] / elastic_total_current;
                col_widths[i] += share;
            }
        }
    }

    col_widths
}

/// Fallback: distribute available space proportionally among all columns.
fn distribute_proportionally(
    natural_widths: &[usize],
    available: usize,
    min_col: usize,
    col_count: usize,
) -> Vec<usize> {
    let total_natural: usize = natural_widths.iter().sum::<usize>().max(1);
    let mut col_widths: Vec<usize> = natural_widths
        .iter()
        .map(|w| (available * w / total_natural).max(min_col))
        .collect();

    // Adjust if we exceeded available space
    let total_allocated: usize = col_widths.iter().sum();
    if total_allocated > available {
        let deficit = total_allocated - available;
        let mut remaining = deficit;
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

// ── Table style helpers ──────────────────────────────────────────────

/// Style for table header cells.
fn table_style_header(theme: MarkdownTheme) -> Style {
    match theme {
        MarkdownTheme::Dark => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        MarkdownTheme::Light => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
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

// ── Table border construction ────────────────────────────────────────

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

// ── Cell word-wrapping and alignment ──────────────────────────────────

/// Wrap cell content to fit within `width` visual columns.
///
/// Returns a vector of wrapped sub-lines. If the text fits in one line,
/// returns `vec![text]`. Otherwise word-wraps at `width` columns using
/// the shared `wrap_line` function.
///
/// If `width` is ≤ 3 (too narrow to wrap), falls back to truncation
/// with `…` ellipsis via `truncate_visual_width`.
fn wrap_cell_content(text: &str, width: usize, _align: ColumnAlign) -> Vec<String> {
    if width <= 3 {
        // Too narrow to wrap — truncate with ellipsis
        return vec![crate::utils::truncate_visual_width(text, width)];
    }

    let text_width = visual_width(text);
    if text_width <= width {
        return vec![text.to_string()];
    }

    // Word-wrap the cell content
    wrap_line(text, width)
}

/// Apply alignment padding to a sub-line within a cell.
///
/// Given the text of a sub-line and the column width, returns
/// (left_pad, content, right_pad) strings for proper alignment.
fn align_cell_text(
    sub_line: &str,
    col_width: usize,
    align: ColumnAlign,
) -> (String, String, String) {
    let text_width = visual_width(sub_line);
    let padding_needed = col_width.saturating_sub(text_width);

    match align {
        ColumnAlign::Left => {
            let right_pad = " ".repeat(padding_needed);
            (String::new(), sub_line.to_string(), right_pad)
        }
        ColumnAlign::Right => {
            let left_pad = " ".repeat(padding_needed);
            (left_pad, sub_line.to_string(), String::new())
        }
        ColumnAlign::Center => {
            let left_pad = " ".repeat(padding_needed / 2);
            let right_pad = " ".repeat(padding_needed - padding_needed / 2);
            (left_pad, sub_line.to_string(), right_pad)
        }
    }
}

/// Build one or more `Line`s for a table row, with cell word-wrapping.
///
/// Each cell is wrapped to its column width. If cells have different
/// numbers of sub-lines, shorter cells are padded with empty strings.
/// Every sub-line gets `│` borders.
///
/// Returns 1 or more `Line`s depending on how much wrapping occurred.
fn build_row_expanded(
    col_widths: &[usize],
    aligns: &[ColumnAlign],
    cells: &[String],
    theme: MarkdownTheme,
    is_header: bool,
) -> Vec<Line<'static>> {
    let border_style = table_style_border(theme);
    let cell_style = if is_header {
        table_style_header(theme)
    } else {
        table_style_cell(theme)
    };

    // Wrap each cell's content
    let wrapped_cells: Vec<Vec<String>> = col_widths
        .iter()
        .enumerate()
        .map(|(i, &width)| {
            let text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let align = aligns.get(i).copied().unwrap_or(ColumnAlign::Left);
            wrap_cell_content(text, width, align)
        })
        .collect();

    // Find the maximum height (number of sub-lines) across all cells
    let max_height = wrapped_cells
        .iter()
        .map(|lines| lines.len())
        .max()
        .unwrap_or(1)
        .max(1);

    // Build visual lines: one Line per sub-line row
    let mut result = Vec::with_capacity(max_height);

    for sub_idx in 0..max_height {
        let mut spans = Vec::new();
        spans.push(Span::styled(BD_VLINE.to_string(), border_style));

        for (col_idx, &width) in col_widths.iter().enumerate() {
            let align = aligns.get(col_idx).copied().unwrap_or(ColumnAlign::Left);
            let sub_text = wrapped_cells
                .get(col_idx)
                .and_then(|lines| lines.get(sub_idx))
                .map(|s| s.as_str())
                .unwrap_or("");

            let (left_pad, content, right_pad) = align_cell_text(sub_text, width, align);

            spans.push(Span::styled(" ".to_string(), cell_style)); // cell left pad
            spans.push(Span::styled(left_pad, cell_style));
            spans.push(Span::styled(content, cell_style));
            spans.push(Span::styled(right_pad, cell_style));
            spans.push(Span::styled(" ".to_string(), cell_style)); // cell right pad
            spans.push(Span::styled(BD_VLINE.to_string(), border_style));
        }

        result.push(Line::from(spans));
    }

    result
}

// ── Table rendering ──────────────────────────────────────────────────

/// Render a table block with box-drawing borders and cell word-wrapping.
///
/// Parses the raw table content, calculates column widths that fit
/// within `max_width` using rigid/elastic classification, wraps cell
/// content as needed, and produces styled `Line`s with Unicode-aware
/// alignment, row separators between every data row, and responsive
/// width.
fn render_table_box(content: &str, max_width: usize, theme: MarkdownTheme) -> Vec<Line<'static>> {
    let table = parse_table_rows(content);
    if table.headers.is_empty() {
        return Vec::new();
    }

    let col_widths = calculate_col_widths(&table.headers, &table.rows, &table.aligns, max_width);
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

    // Header row (may wrap if header text is long)
    let header_lines = build_row_expanded(&col_widths, &table.aligns, &table.headers, theme, true);
    lines.extend(header_lines);

    // Header/data separator
    lines.push(Line::from(Span::styled(
        build_hline(&col_widths, BD_ML, BD_MC, BD_MR),
        border_style,
    )));

    // Data rows with row separators between each
    for (row_idx, row) in table.rows.iter().enumerate() {
        let row_lines = build_row_expanded(&col_widths, &table.aligns, row, theme, false);
        lines.extend(row_lines);

        // Row separator between data rows (not after the last one —
        // the bottom border serves as the final separator)
        if row_idx < table.rows.len() - 1 {
            lines.push(Line::from(Span::styled(
                build_hline(&col_widths, BD_ML, BD_MC, BD_MR),
                border_style,
            )));
        }
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
/// column alignment, cell word-wrapping, and responsive width fitting
/// within `max_width`. This ensures `Paragraph::wrap()` does not
/// break table lines.
///
///
/// Used for both streaming and completed content. Markdown rendering
/// is called on every render frame during streaming (like Thinking blocks)
/// and once for completed messages.
/// Post-process rendered markdown to apply code block background colors,
/// adjust foreground colors for readability, and extend backgrounds to the
/// right edge of the chat area.
///
/// `tui-markdown` with the `highlight-code` feature uses syntect for syntax
/// highlighting (theme: `base16-ocean.dark`), which sets foreground colors
/// per token but never sets background colors. This function:
///
/// 1. Applies theme-appropriate styling to all lines inside fenced code
///    blocks, including the opening and closing fence lines:
///    - Dark: Catppuccin Mocha Surface0 background, syntect colors preserved
///    - Light: Catppuccin Latte Surface0 background, all Span fg overridden
///      to LATTE_TEXT (syntect colors are designed for dark backgrounds and
///      are unreadable on Latte Surface0)
///    - Mono: No background color, no REVERSED — only bold modifier via
///      Line.style.add_modifier(BOLD). All Span colors stripped for true
///      monochrome (terminal's own fg/bg colors apply).
/// 2. Adds trailing padding (styled spaces) to extend the background to the
///    right edge of the chat area for Dark/Light themes (Mono has no bg so
///    no padding needed).
///
/// Clipboard copies are protected: `visual_lines` uses `trim_end()` to strip
/// trailing padding whitespace while preserving code content.
fn apply_code_block_background(
    lines: &mut [Line<'static>],
    theme: MarkdownTheme,
    max_width: usize,
) {
    let bg_color = match theme {
        MarkdownTheme::Dark => MOCHA_SURFACE0,
        MarkdownTheme::Light => LATTE_SURFACE0,
        // Mono does not use background colors. The placeholder is never
        // accessed because all Mono-specific branches skip bg operations.
        MarkdownTheme::Mono => Color::Reset,
    };

    let mut in_code_block = false;
    for line in lines.iter_mut() {
        // Detect code block fence lines: lines that start with ```
        // (tui-markdown renders "```rust" as the opening fence)
        let is_fence = line
            .spans
            .first()
            .map(|span| span.content.starts_with("```"))
            .unwrap_or(false);

        if is_fence && !in_code_block {
            // Opening fence — start of code block
            in_code_block = true;
            match theme {
                MarkdownTheme::Dark | MarkdownTheme::Light => {
                    line.style = line.style.patch(Style::default().bg(bg_color));
                }
                MarkdownTheme::Mono => {
                    // Mono: only bold modifier, no background or REVERSED
                    line.style = line
                        .style
                        .patch(Style::default().add_modifier(Modifier::BOLD));
                    // Strip all colors from fence line
                    line.style.fg = None;
                    line.style.bg = None;
                    for span in line.spans.iter_mut() {
                        span.style.fg = None;
                        span.style.bg = None;
                    }
                }
            }
        } else if is_fence && in_code_block {
            // Closing fence — end of code block
            match theme {
                MarkdownTheme::Dark | MarkdownTheme::Light => {
                    line.style = line.style.patch(Style::default().bg(bg_color));
                }
                MarkdownTheme::Mono => {
                    line.style = line
                        .style
                        .patch(Style::default().add_modifier(Modifier::BOLD));
                    line.style.fg = None;
                    line.style.bg = None;
                    for span in line.spans.iter_mut() {
                        span.style.fg = None;
                        span.style.bg = None;
                    }
                }
            }
            in_code_block = false;
        } else if in_code_block {
            // Content line inside code block
            match theme {
                MarkdownTheme::Dark | MarkdownTheme::Light => {
                    line.style = line.style.patch(Style::default().bg(bg_color));
                }
                MarkdownTheme::Mono => {
                    line.style = line
                        .style
                        .patch(Style::default().add_modifier(Modifier::BOLD));
                    line.style.fg = None;
                    line.style.bg = None;
                    for span in line.spans.iter_mut() {
                        span.style.fg = None;
                        span.style.bg = None;
                    }
                }
            }
        }

        // For Light theme, override ALL Span fg colors inside code blocks.
        // syntect's base16-ocean.dark produces colors designed for dark
        // backgrounds that are unreadable on Latte Surface0 (#ccd0da).
        // We must set fg even on Spans where fg is None (plain text without
        // syntax highlighting), otherwise those Spans inherit the terminal's
        // default fg which may be light/invisible on the light background.
        if in_code_block || is_fence {
            match theme {
                MarkdownTheme::Light => {
                    for span in line.spans.iter_mut() {
                        span.style.fg = Some(LATTE_TEXT);
                    }
                }
                MarkdownTheme::Mono => {
                    // Already stripped above in the per-branch blocks.
                    // This match arm is here for completeness; Mono color
                    // stripping is done in each branch above.
                }
                MarkdownTheme::Dark => {
                    // Dark theme: keep syntect colors as-is
                }
            }
        }

        // Extend background to the right edge with trailing padding.
        // For Dark/Light themes, this makes code blocks appear as solid
        // colored rectangles. For Mono, no background means no padding.
        if (in_code_block || is_fence) && !matches!(theme, MarkdownTheme::Mono) {
            let current_width: usize = line
                .spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum();

            if current_width < max_width {
                let padding_width = max_width - current_width;
                let padding = " ".repeat(padding_width);
                line.spans
                    .push(Span::styled(padding, Style::default().bg(bg_color)));
            }
        }
    }
}

pub fn render_markdown(content: &str, theme: MarkdownTheme, max_width: usize) -> Text<'static> {
    // Fast path: if no table or Mermaid structure detected, use tui-markdown directly
    if !content_contains_special_blocks(content) {
        let mut text = render_markdown_inner_owned(content, theme);
        apply_code_block_background(&mut text.lines, theme, max_width);
        return text;
    }

    // Slow path: extract segments and render hybrid
    let segments = extract_content_segments(content);
    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for segment in segments {
        match segment {
            ContentSegment::Markdown(md) => {
                let rendered = render_markdown_inner_owned(&md, theme);
                let mut rendered_lines = rendered.lines;
                apply_code_block_background(&mut rendered_lines, theme, max_width);
                all_lines.extend(rendered_lines);
            }
            ContentSegment::Table(table) => {
                let table_lines = render_table_box(&table, max_width, theme);
                all_lines.extend(table_lines);
            }
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(mermaid_source) => {
                let mermaid_lines = render_mermaid_tui(&mermaid_source, max_width, theme);
                all_lines.extend(mermaid_lines);
            }
        }
    }

    Text::from(all_lines)
}

/// Quick check: does the content contain a table or Mermaid block?
///
/// Returns true if content has a table-like structure or a ` ```mermaid ` block.
/// This avoids the overhead of segment extraction for the common case of no
/// special blocks.
fn content_contains_special_blocks(content: &str) -> bool {
    let mut in_code_block = false;
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();
            if in_code_block {
                in_code_block = false;
            } else {
                // Check for Mermaid block before marking as code block
                #[cfg(feature = "mermaid")]
                if lang.starts_with("mermaid") {
                    return true;
                }
                in_code_block = true;
            }
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

/// Backward-compatible alias for `content_contains_special_blocks`.
#[deprecated(since = "0.44.0", note = "Use content_contains_special_blocks instead")]
#[allow(dead_code)] // Kept for backward compatibility with tests
fn content_contains_table(content: &str) -> bool {
    content_contains_special_blocks(content)
}

// ── Mermaid diagram rendering (TUI) ──────────────────────────────────

/// Render a Mermaid diagram as styled ratatui `Line`s for the TUI.
///
/// Uses the `mermaid-text` crate to produce Unicode box-drawing text,
/// then converts each line to a ratatui `Line` with theme-aware styling.
///
/// Falls back to rendering the Mermaid source as a code block on parse errors.
#[cfg(feature = "mermaid")]
fn render_mermaid_tui(source: &str, max_width: usize, theme: MarkdownTheme) -> Vec<Line<'static>> {
    let effective_width = max_width.clamp(40, 200);
    let border_style = table_style_border(theme);
    let text_style = table_style_cell(theme);

    match mermaid_text::render_with_width(source.trim(), Some(effective_width)) {
        Ok(rendered) => rendered
            .lines()
            .map(|line| {
                if looks_like_diagram_line(line) {
                    Line::from(Span::styled(line.to_string(), border_style))
                } else {
                    Line::from(Span::styled(line.to_string(), text_style))
                }
            })
            .collect(),
        Err(_) => {
            // Fallback: render as a code block with "mermaid" language tag
            let code_style = table_style_cell(theme);
            let mut lines = Vec::new();
            lines.push(Line::from(Span::styled(
                "```mermaid".to_string(),
                code_style,
            )));
            for line in source.lines() {
                lines.push(Line::from(Span::styled(line.to_string(), code_style)));
            }
            lines.push(Line::from(Span::styled("```".to_string(), code_style)));
            lines
        }
    }
}

/// Check if a line contains box-drawing characters (diagram line vs text label).
#[cfg(feature = "mermaid")]
fn looks_like_diagram_line(line: &str) -> bool {
    const BOX_CHARS: &[char] = &[
        '─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼', '╭', '╮', '╰', '╯', '►', '◂', '▾',
        '▴', '▸', '◀', '▶', '▲', '▼', '━', '┃', '┏', '┓', '┗', '┛', '┣', '┫', '┳', '┻', '╋',
    ];
    line.chars().any(|c| BOX_CHARS.contains(&c))
}

/// Replace markdown table blocks in content with a placeholder string.
///
/// Used by `show_recent_context` to avoid rendering pipe characters and
/// separator lines from tables in the single-line recent context display.
/// Each table block is replaced with `(...)` to indicate omitted tabular
/// content. Tables inside fenced code blocks are left intact.
/// Mermaid blocks are also collapsed to `(...)`.
pub fn collapse_tables(content: &str) -> String {
    // Fast path: no special block structure at all
    if !content_contains_special_blocks(content) {
        return content.to_string();
    }

    let segments = extract_content_segments(content);
    let mut result = String::new();

    for segment in segments {
        match segment {
            ContentSegment::Markdown(md) => {
                result.push_str(&md);
            }
            ContentSegment::Table(_) => {
                // Replace the entire table block with a placeholder
                if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('\n') {
                    result.push(' ');
                }
                result.push_str("(...) ");
            }
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(_) => {
                // Replace Mermaid blocks with a placeholder too
                if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('\n') {
                    result.push(' ');
                }
                result.push_str("(...) ");
            }
        }
    }

    result
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
    // Convert borrowed Text<'a> to owned Text<'static>,
    // preserving Line.style (which carries heading formatting from tui-markdown).
    let owned_lines: Vec<Line<'static>> = text
        .lines
        .into_iter()
        .map(|line| {
            let owned_spans: Vec<Span<'static>> = line
                .spans
                .into_iter()
                .map(|span| Span::styled(span.content.into_owned(), span.style))
                .collect();
            let mut owned_line = Line::from(owned_spans);
            owned_line.style = line.style;
            owned_line
        })
        .collect();
    Text::from(owned_lines)
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
    fn test_render_markdown_dark() {
        let text = render_markdown("# Hello", MarkdownTheme::Dark, 80);
        assert!(!text.lines.is_empty());
    }

    #[test]
    fn test_heading_style_propagation() {
        // tui-markdown produces headings with style in Line.style
        // (fallback for Spans with Style::default). Verify that
        // DarkStyleSheet heading styles are applied correctly.
        let dark_h1 = DarkStyleSheet.heading(1);
        assert!(
            dark_h1.add_modifier == Modifier::BOLD | Modifier::UNDERLINED,
            "Dark H1 should have BOLD | UNDERLINED, got {:?}",
            dark_h1.add_modifier
        );
        assert_eq!(dark_h1.fg, Some(Color::Yellow));

        let dark_h2 = DarkStyleSheet.heading(2);
        assert!(
            dark_h2.add_modifier == Modifier::BOLD | Modifier::UNDERLINED,
            "Dark H2 should have BOLD | UNDERLINED, got {:?}",
            dark_h2.add_modifier
        );
        assert_eq!(dark_h2.fg, Some(Color::Cyan));

        let dark_h3 = DarkStyleSheet.heading(3);
        assert!(
            dark_h3.add_modifier == Modifier::BOLD,
            "Dark H3 should have BOLD only, got {:?}",
            dark_h3.add_modifier
        );
        assert_eq!(dark_h3.fg, Some(Color::Green));

        let light_h1 = LightStyleSheet.heading(1);
        assert!(
            light_h1.add_modifier == Modifier::BOLD | Modifier::UNDERLINED,
            "Light H1 should have BOLD | UNDERLINED, got {:?}",
            light_h1.add_modifier
        );

        let mono_h1 = MonoStyleSheet.heading(1);
        assert!(
            mono_h1.add_modifier == Modifier::BOLD | Modifier::UNDERLINED,
            "Mono H1 should have BOLD | UNDERLINED, got {:?}",
            mono_h1.add_modifier
        );

        let mono_h3 = MonoStyleSheet.heading(3);
        assert!(
            mono_h3.add_modifier == Modifier::BOLD,
            "Mono H3 should have BOLD only, got {:?}",
            mono_h3.add_modifier
        );
    }

    #[test]
    fn test_heading_line_style_in_render_output() {
        // Verify that render_markdown produces lines with Line.style
        // set for headings (tui-markdown puts heading style in Line.style)
        let text = render_markdown(
            "# Heading 1\n\n## Heading 2\n\n### Heading 3",
            MarkdownTheme::Dark,
            80,
        );

        // Should have heading lines
        assert!(
            text.lines.len() >= 3,
            "Expected at least 3 lines for headings, got {}",
            text.lines.len()
        );

        // Check that the first heading line has non-default Line.style
        // (tui-markdown sets Line.style for heading lines)
        let has_non_default_line_style =
            text.lines.iter().any(|line| line.style != Style::default());
        assert!(
            has_non_default_line_style,
            "Expected at least one line with non-default Line.style (heading), but all lines have Style::default()"
        );
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

    // ── Code block styling tests (Catppuccin palette) ────────────────

    #[test]
    fn test_dark_code_style_has_catppuccin_bg() {
        let style = DarkStyleSheet.code();
        assert_eq!(
            style.fg,
            Some(MOCHA_TEXT),
            "Dark code fg should be Catppuccin Mocha Text"
        );
        assert_eq!(
            style.bg,
            Some(MOCHA_SURFACE0),
            "Dark code bg should be Catppuccin Mocha Surface0"
        );
    }

    #[test]
    fn test_light_code_style_has_catppuccin_bg() {
        let style = LightStyleSheet.code();
        assert_eq!(
            style.fg,
            Some(LATTE_TEXT),
            "Light code fg should be Catppuccin Latte Text"
        );
        assert_eq!(
            style.bg,
            Some(LATTE_SURFACE0),
            "Light code bg should be Catppuccin Latte Surface0"
        );
    }

    #[test]
    fn test_mono_code_style_is_bold_only() {
        // Mono theme uses BOLD modifier only — no colors, no background, no
        // REVERSED. Truly monochrome with no RGB colors.
        let style = MonoStyleSheet.code();
        assert_eq!(style.fg, None, "Mono code fg should be None (no color)");
        assert_eq!(style.bg, None, "Mono code bg should be None (no color)");
        assert!(
            style.add_modifier.contains(Modifier::BOLD),
            "Mono code should have BOLD modifier"
        );
        assert!(
            !style.add_modifier.contains(Modifier::REVERSED),
            "Mono code should NOT have REVERSED modifier"
        );
    }

    #[test]
    fn test_render_code_block_has_visual_distinction() {
        // Verify that rendered code blocks have visual distinction
        // in all themes. Dark/Light use background colors; Mono uses BOLD
        // modifier only (no REVERSED, no background).
        let code_md = "```rust\nfn main() {}\n```";
        for theme in [MarkdownTheme::Dark, MarkdownTheme::Light] {
            let text = render_markdown(code_md, theme, 80);
            let has_bg = text.lines.iter().any(|line| {
                if line.style.bg.is_some() {
                    return true;
                }
                line.spans.iter().any(|span| span.style.bg.is_some())
            });
            assert!(
                has_bg,
                "Expected code block lines with background color for theme {:?}, but none found",
                theme
            );
        }
        // Mono: check for BOLD modifier instead of background color or REVERSED
        let mono_text = render_markdown(code_md, MarkdownTheme::Mono, 80);
        let has_bold = mono_text
            .lines
            .iter()
            .any(|line| line.style.add_modifier.contains(Modifier::BOLD));
        assert!(
            has_bold,
            "Expected code block lines with BOLD modifier for Mono theme, but none found"
        );
    }

    #[test]
    fn test_code_block_background_matches_theme() {
        // Verify the specific Catppuccin background color is applied per theme
        // for Dark and Light. Mono uses BOLD modifier instead of bg color.
        let code_md = "```rust\nfn main() {}\n```";
        let expected_bg = [
            (MarkdownTheme::Dark, MOCHA_SURFACE0),
            (MarkdownTheme::Light, LATTE_SURFACE0),
        ];
        for (theme, bg) in expected_bg {
            let text = render_markdown(code_md, theme, 80);
            let first_fence = text
                .lines
                .iter()
                .find(|line| {
                    line.spans
                        .first()
                        .map(|span| span.content.starts_with("```"))
                        .unwrap_or(false)
                })
                .expect("should find opening fence line");
            assert_eq!(
                first_fence.style.bg,
                Some(bg),
                "Opening fence for {:?} should have bg={:?}, got {:?}",
                theme,
                bg,
                first_fence.style.bg
            );
        }
    }

    #[test]
    fn test_code_block_mono_uses_bold_only() {
        // Mono theme uses BOLD modifier only (no REVERSED, no bg color)
        // for code blocks — truly monochrome with no RGB colors.
        let code_md = "```rust\nfn main() {}\n```";
        let text = render_markdown(code_md, MarkdownTheme::Mono, 80);
        let first_fence = text
            .lines
            .iter()
            .find(|line| {
                line.spans
                    .first()
                    .map(|span| span.content.starts_with("```"))
                    .unwrap_or(false)
            })
            .expect("should find opening fence line");
        assert!(
            first_fence.style.add_modifier.contains(Modifier::BOLD),
            "Mono code block should have BOLD modifier, got {:?}",
            first_fence.style
        );
        assert!(
            !first_fence.style.add_modifier.contains(Modifier::REVERSED),
            "Mono code block should NOT have REVERSED modifier, got {:?}",
            first_fence.style
        );
        assert_eq!(
            first_fence.style.bg, None,
            "Mono code block should have no bg color, got {:?}",
            first_fence.style.bg
        );
    }

    #[test]
    fn test_inline_code_has_catppuccin_style() {
        // Inline code uses StyleSheet::code() directly (not highlight-code).
        // Dark/Light: verify both fg and bg are set (Catppuccin colors).
        // Mono: verify BOLD modifier only, no colors, no REVERSED.
        let inline_md = "Use `println!` to print";
        for theme in [MarkdownTheme::Dark, MarkdownTheme::Light] {
            let text = render_markdown(inline_md, theme, 80);
            let has_code_style = text.lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains("println!")
                        && span.style.bg.is_some()
                        && span.style.fg.is_some()
                })
            });
            assert!(
                has_code_style,
                "Inline code should have both fg and bg for theme {:?}",
                theme
            );
        }
        // Mono: verify BOLD modifier only, no RGB colors, no REVERSED
        let mono_text = render_markdown(inline_md, MarkdownTheme::Mono, 80);
        let has_mono_style = mono_text.lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content.contains("println!")
                    && span.style.add_modifier.contains(Modifier::BOLD)
                    && !span.style.add_modifier.contains(Modifier::REVERSED)
                    && span.style.fg.is_none()
                    && span.style.bg.is_none()
            })
        });
        assert!(
            has_mono_style,
            "Mono inline code should have BOLD modifier (no REVERSED) and no colors"
        );
    }

    #[test]
    fn test_mono_code_block_no_rgb_colors() {
        // Verify that Mono theme code blocks contain no RGB colors
        // whatsoever — truly monochrome with only BOLD modifier.
        let code_md = "```rust\nfn main() {}\n```";
        let text = render_markdown(code_md, MarkdownTheme::Mono, 80);
        for line in &text.lines {
            // Line.style should have no fg/bg colors
            assert_eq!(
                line.style.fg, None,
                "Mono Line.style should have no fg color, got {:?}",
                line.style.fg
            );
            assert_eq!(
                line.style.bg, None,
                "Mono Line.style should have no bg color, got {:?}",
                line.style.bg
            );
            // Span styles should have no fg/bg colors
            for span in &line.spans {
                assert_eq!(
                    span.style.fg, None,
                    "Mono Span should have no fg color, got {:?} in content {:?}",
                    span.style.fg, span.content
                );
                assert_eq!(
                    span.style.bg, None,
                    "Mono Span should have no bg color, got {:?} in content {:?}",
                    span.style.bg, span.content
                );
            }
        }
    }

    #[test]
    fn test_code_block_right_edge_padding() {
        // Verify that Dark/Light code block lines have trailing padding extending
        // the background to the right edge. Mono uses bold text instead of
        // background colors and does not add trailing padding.
        let code_md = "```rust\nfn main() {}\n```";
        let max_width = 80;
        for theme in [MarkdownTheme::Dark, MarkdownTheme::Light] {
            let text = render_markdown(code_md, theme, max_width);
            // All lines inside the code block should have total width == max_width
            // (content width + padding width)
            let mut found_code_line = false;
            for line in &text.lines {
                let is_fence = line
                    .spans
                    .first()
                    .map(|span| span.content.starts_with("```"))
                    .unwrap_or(false);
                let has_bg = line.style.bg.is_some()
                    || line.spans.iter().any(|span| span.style.bg.is_some());

                if is_fence || has_bg {
                    found_code_line = true;
                    let total_width: usize = line
                        .spans
                        .iter()
                        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                        .sum();
                    assert_eq!(
                        total_width, max_width,
                        "Code block line should extend to max_width={} for theme {:?}, got width={}",
                        max_width, theme, total_width
                    );
                }
            }
            assert!(
                found_code_line,
                "Should find at least one code block line for theme {:?}",
                theme
            );
        }
    }

    #[test]
    fn test_code_block_empty_line_padding() {
        // Verify that empty lines inside code blocks are padded to max_width
        // (so the background covers the full line, not just empty space).
        let code_md = "```rust\nfn main() {\n    \n}\n```";
        let max_width = 60;
        let text = render_markdown(code_md, MarkdownTheme::Dark, max_width);
        // Find the empty line (original content "    " — 4 spaces)
        // It should be padded to max_width total
        let empty_line = text.lines.iter().find(|line| {
            line.style.bg.is_some()
                && line
                    .spans
                    .iter()
                    .map(|s| s.content.trim().len())
                    .sum::<usize>()
                    == 0
        });
        assert!(
            empty_line.is_some(),
            "Should find a code block line with only whitespace content"
        );
        if let Some(line) = empty_line {
            let total_width: usize = line
                .spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum();
            assert_eq!(
                total_width, max_width,
                "Empty code block line should be padded to max_width={}, got {}",
                max_width, total_width
            );
        }
    }

    #[test]
    fn test_code_block_no_padding_beyond_max_width() {
        // Verify that lines wider than max_width do NOT get padding
        // (they already fill the available width).
        let long_line = "x".repeat(100);
        let code_md = format!("```rust\n{}\n```", long_line);
        let max_width = 80;
        let text = render_markdown(&code_md, MarkdownTheme::Dark, max_width);
        // The long line should NOT have trailing padding (it's already > max_width)
        let long_line_entry = text.lines.iter().find(|line| {
            line.spans
                .iter()
                .any(|span| span.content.starts_with('x') && span.content.len() > 90)
        });
        assert!(
            long_line_entry.is_some(),
            "Should find the long line in the code block"
        );
        if let Some(line) = long_line_entry {
            let total_width: usize = line
                .spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum();
            // Should be > max_width (no padding was added)
            assert!(
                total_width > max_width,
                "Long code line should exceed max_width without padding, got {}",
                total_width
            );
        }
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

    // ── Column alignment tests ────────────────────────────────────

    #[test]
    fn test_parse_separator_line_left() {
        let result = parse_separator_line("|---|---|");
        assert_eq!(result, Some(vec![ColumnAlign::Left, ColumnAlign::Left]));
    }

    #[test]
    fn test_parse_separator_line_right() {
        let result = parse_separator_line("|---:|---:|");
        assert_eq!(result, Some(vec![ColumnAlign::Right, ColumnAlign::Right]));
    }

    #[test]
    fn test_parse_separator_line_center() {
        let result = parse_separator_line("|:---:|:---:|");
        assert_eq!(result, Some(vec![ColumnAlign::Center, ColumnAlign::Center]));
    }

    #[test]
    fn test_parse_separator_line_mixed() {
        let result = parse_separator_line("|:---:|---:|---|");
        assert_eq!(
            result,
            Some(vec![
                ColumnAlign::Center,
                ColumnAlign::Right,
                ColumnAlign::Left
            ])
        );
    }

    #[test]
    fn test_parse_separator_line_invalid() {
        // Not a separator — contains letters
        assert!(parse_separator_line("| A | B |").is_none());
        // Too short
        assert!(parse_separator_line("|").is_none());
        // Empty cells with no dashes
        assert!(parse_separator_line("| | |").is_none());
    }

    #[test]
    fn test_parse_separator_line_with_spaces() {
        let result = parse_separator_line("| --- | ---: | :---: |");
        assert_eq!(
            result,
            Some(vec![
                ColumnAlign::Left,
                ColumnAlign::Right,
                ColumnAlign::Center
            ])
        );
    }

    // ── Table segment extraction tests ─────────────────────────────

    #[test]
    fn test_extract_table_segments_no_table() {
        let content = "Hello world\n\nThis is a paragraph.";
        let segments = extract_content_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            ContentSegment::Markdown(md) => assert!(md.contains("Hello world")),
            ContentSegment::Table(_) => panic!("Expected Markdown segment"),
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(_) => panic!("Expected Markdown segment"),
        }
    }

    #[test]
    fn test_extract_table_segments_simple_table() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let segments = extract_content_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            ContentSegment::Table(table) => {
                assert!(table.contains("| Name | Value |"));
                assert!(table.contains("| Foo  | 42    |"));
            }
            ContentSegment::Markdown(_) => panic!("Expected Table segment"),
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(_) => panic!("Expected Table segment"),
        }
    }

    #[test]
    fn test_extract_table_segments_mixed_content() {
        let content = "Here is some text:\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.";
        let segments = extract_content_segments(content);
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
        let segments = extract_content_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            ContentSegment::Markdown(md) => {
                assert!(md.contains("| A | B |"));
                assert!(md.contains("```"));
            }
            ContentSegment::Table(_) => panic!("Table inside code block should not be detected"),
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(_) => panic!("Table inside code block should be Markdown"),
        }
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_extract_mermaid_block() {
        let content = "Before\n\n```mermaid\ngraph LR\n  A --> B\n```\n\nAfter";
        let segments = extract_content_segments(content);
        assert_eq!(
            segments.len(),
            3,
            "Expected 3 segments: markdown, mermaid, markdown"
        );
        match &segments[0] {
            ContentSegment::Markdown(md) => assert!(md.contains("Before")),
            _ => panic!("Expected Markdown first"),
        }
        match &segments[1] {
            ContentSegment::Mermaid(mermaid) => {
                assert!(mermaid.contains("graph LR"));
                assert!(mermaid.contains("A --> B"));
            }
            _ => panic!("Expected Mermaid second"),
        }
        match &segments[2] {
            ContentSegment::Markdown(md) => assert!(md.contains("After")),
            _ => panic!("Expected Markdown third"),
        }
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_mermaid_not_in_code_block() {
        // A ```java block containing the word "mermaid" should NOT be detected as mermaid
        let content = "```java\nmermaid.init();\n```\n\nAfter";
        let segments = extract_content_segments(content);
        assert_eq!(
            segments.len(),
            1,
            "Java code block should not be detected as Mermaid"
        );
        match &segments[0] {
            ContentSegment::Markdown(md) => assert!(md.contains("mermaid.init()")),
            _ => panic!("Expected Markdown segment"),
        }
    }

    // ── Table parsing tests ───────────────────────────────────────

    #[test]
    fn test_parse_table_rows_basic() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let table = parse_table_rows(content);
        assert_eq!(table.headers, vec!["Name", "Value"]);
        assert_eq!(table.rows, vec![vec!["Foo", "42"]]);
        assert_eq!(table.aligns, vec![ColumnAlign::Left, ColumnAlign::Left]);
    }

    #[test]
    fn test_parse_table_rows_with_alignment() {
        let content = "| Name | Value |\n|:---:|---:|\n| Foo  | 42    |";
        let table = parse_table_rows(content);
        assert_eq!(table.headers, vec!["Name", "Value"]);
        assert_eq!(table.aligns, vec![ColumnAlign::Center, ColumnAlign::Right]);
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
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Left];
        let widths = calculate_col_widths(&headers, &rows, &aligns, 80);
        // Natural widths: Name=4, Value=5 — both fit comfortably
        assert_eq!(widths.len(), 2);
        assert!(
            widths[0] >= 4,
            "Column 0 should fit 'Name' (width {})",
            widths[0]
        );
        assert!(
            widths[1] >= 5,
            "Column 1 should fit 'Value' (width {})",
            widths[1]
        );
    }

    #[test]
    fn test_calculate_col_widths_narrow_terminal() {
        let headers = vec!["VeryLongColumnName".to_string(), "B".to_string()];
        let rows = vec![vec!["data".to_string(), "x".to_string()]];
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Left];
        let widths = calculate_col_widths(&headers, &rows, &aligns, 20);
        // Total must fit within 20 chars (borders + padding + content)
        let total: usize = widths.iter().sum();
        let border_overhead = widths.len() + 1; // │ borders
        let padding = widths.len() * 2;
        assert!(
            total + border_overhead + padding <= 20 || widths.iter().all(|&w| w >= 3),
            "Columns should be at least 3 chars wide or fit in terminal"
        );
    }

    #[test]
    fn test_calculate_col_widths_unicode() {
        let headers = vec!["名前".to_string(), "値".to_string()];
        let rows = vec![vec!["日本語".to_string(), "42".to_string()]];
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Left];
        let widths = calculate_col_widths(&headers, &rows, &aligns, 80);
        assert_eq!(widths.len(), 2);
        // CJK chars take 2 columns each: 名前=4, 日本語=6
        assert!(
            widths[0] >= 4,
            "Column 0 should fit CJK header (width {})",
            widths[0]
        );
    }

    #[test]
    fn test_calculate_col_widths_rigid_elastic() {
        // ID column: all cells ≤ RIGID_THRESHOLD → rigid
        // Description column: long cells → elastic
        let headers = vec!["ID".to_string(), "Description".to_string()];
        let rows = vec![
            vec!["1".to_string(), "A very long description text".to_string()],
            vec!["42".to_string(), "Another long description".to_string()],
        ];
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Left];
        let widths = calculate_col_widths(&headers, &rows, &aligns, 60);

        // ID column should be rigid: width = natural (1 or 2, header "ID"=2)
        assert!(
            widths[0] <= RIGID_THRESHOLD,
            "Rigid column should keep natural width (got {})",
            widths[0]
        );

        // Description column should be elastic: gets the remaining space
        assert!(
            widths[1] > widths[0],
            "Elastic column should be wider than rigid (got {} vs {})",
            widths[1],
            widths[0]
        );
    }

    #[test]
    fn test_calculate_col_widths_rigid_preserved() {
        // Rigid columns keep their exact natural width when room allows
        let headers = vec!["ID".to_string(), "Name".to_string()];
        let rows = vec![vec!["1".to_string(), "Alice".to_string()]];
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Left];
        let widths = calculate_col_widths(&headers, &rows, &aligns, 80);

        // Both are rigid (ID=1, Name=4 — both ≤ RIGID_THRESHOLD)
        // Natural widths: ID header=2, ID data=1 → 2; Name header=4, Name data=5 → 5
        assert_eq!(widths[0], 2, "Rigid column 'ID' should be natural width 2");
        assert_eq!(
            widths[1], 5,
            "Rigid column 'Name' should be natural width 5"
        );
    }

    #[test]
    fn test_calculate_col_widths_all_rigid_narrow() {
        // All columns rigid but don't fit — proportional fallback
        let headers = vec!["ColumnA".to_string(), "ColumnB".to_string()];
        let rows = vec![vec!["val1".to_string(), "val2".to_string()]];
        let aligns = vec![ColumnAlign::Left, ColumnAlign::Left];
        // Width 15: need borders (3) + padding (4) + content → only 8 for content
        let widths = calculate_col_widths(&headers, &rows, &aligns, 15);
        assert_eq!(widths.len(), 2);
        // Should still allocate at least min_col chars each
        assert!(widths.iter().all(|&w| w >= 3));
    }

    // ── Cell wrapping tests ─────────────────────────────────────────

    #[test]
    fn test_wrap_cell_content_short() {
        let result = wrap_cell_content("hello", 20, ColumnAlign::Left);
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn test_wrap_cell_content_long() {
        let result = wrap_cell_content("hello world foo bar", 10, ColumnAlign::Left);
        // Should wrap into multiple lines, each ≤ 10 visual cols
        for line in &result {
            assert!(
                visual_width(line) <= 10,
                "Wrapped line '{}' exceeds 10 cols (width {})",
                line,
                visual_width(line)
            );
        }
        assert!(result.len() > 1, "Long text should wrap to multiple lines");
    }

    #[test]
    fn test_wrap_cell_content_unicode() {
        let result = wrap_cell_content("日本語テストデータ", 8, ColumnAlign::Left);
        // CJK chars = 2 cols each, so 8 cols = 4 CJK chars max per line
        for line in &result {
            assert!(
                visual_width(line) <= 8,
                "Wrapped CJK line '{}' exceeds 8 cols (width {})",
                line,
                visual_width(line)
            );
        }
    }

    #[test]
    fn test_wrap_cell_content_narrow_truncate() {
        // Width ≤ 3: fallback to truncation with ellipsis
        let result = wrap_cell_content("hello world", 3, ColumnAlign::Left);
        assert_eq!(
            result.len(),
            1,
            "Narrow cell should produce single truncated line"
        );
        assert!(visual_width(&result[0]) <= 3);
    }

    // ── Cell alignment tests ───────────────────────────────────────

    #[test]
    fn test_align_cell_text_left() {
        let (left, content, right) = align_cell_text("hi", 10, ColumnAlign::Left);
        assert_eq!(content, "hi");
        assert_eq!(left, "");
        assert_eq!(visual_width(&right), 10 - 2); // 10 - "hi" width
    }

    #[test]
    fn test_align_cell_text_right() {
        let (left, content, right) = align_cell_text("hi", 10, ColumnAlign::Right);
        assert_eq!(content, "hi");
        assert_eq!(right, "");
        assert_eq!(visual_width(&left), 10 - 2); // padding on left
    }

    #[test]
    fn test_align_cell_text_center() {
        let (left, content, right) = align_cell_text("hi", 10, ColumnAlign::Center);
        assert_eq!(content, "hi");
        // 10 - 2 = 8 padding total, split: floor(4) left, ceil(4) right
        assert_eq!(visual_width(&left) + visual_width(&right), 8);
    }

    // ── Box-drawing table rendering tests ─────────────────────────

    #[test]
    fn test_render_table_box_basic() {
        let content = "| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let lines = render_table_box(content, 80, MarkdownTheme::Dark);
        // top border + header + header separator + data row + row separator + bottom border
        // = 6 lines (1 data row gets a separator before the bottom border)
        assert!(
            lines.len() >= 5,
            "Table should have at least 5 lines, got {}",
            lines.len()
        );
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
        assert!(
            rendered.contains("Name"),
            "Header 'Name' should be in output"
        );
        assert!(
            rendered.contains("Value"),
            "Header 'Value' should be in output"
        );
        assert!(rendered.contains("Foo"), "Data 'Foo' should be in output");
        assert!(rendered.contains("42"), "Data '42' should be in output");
    }

    #[test]
    fn test_render_table_box_respects_max_width() {
        let content = "| Name | Very Long Value Content |\n|------|--------------------------|\n| Foo  | A very long value that should wrap inside the cell |";
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
        let content = "| 名前   | 値 |\n|--------|-----|\n| 日本語  | 42  |";
        let lines = render_table_box(content, 60, MarkdownTheme::Dark);
        // Just verify no panic or crash with CJK content
        assert!(!lines.is_empty(), "Table should produce output");
    }

    #[test]
    fn test_render_table_box_single_column() {
        let content = "| Items |\n|-------|\n| A     |\n| B     |";
        let lines = render_table_box(content, 40, MarkdownTheme::Dark);
        // top border + header + header separator + 2 data rows + 1 row separator + bottom border = 7
        assert!(
            lines.len() >= 6,
            "Single-column table should have at least 6 lines, got {}",
            lines.len()
        );
    }

    #[test]
    fn test_render_table_box_row_separators() {
        // 3 data rows → 2 row separators between them
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |";
        let lines = render_table_box(content, 40, MarkdownTheme::Dark);

        // Count internal row separators (├─┼─┤ lines between data rows)
        let mut mid_count = 0;
        for line in &lines {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.contains('┼') || text.contains('├') {
                mid_count += 1;
            }
        }
        // Header separator + 2 row separators = 3 mid-border lines
        assert!(
            mid_count >= 3,
            "Should have header separator + 2 row separators (got {} mid-border lines)",
            mid_count
        );
    }

    #[test]
    fn test_render_table_box_wrap_multi_line_cell() {
        // Long text in one cell should wrap to multiple sub-lines
        let content = "| ID | Description |\n|---|---|\n| 1 | A very long description that should wrap inside the cell when the terminal is narrow |";
        let lines = render_table_box(content, 40, MarkdownTheme::Dark);

        // The table should produce more than the basic 5-6 lines
        // because the long description wraps
        assert!(
            lines.len() > 6,
            "Table with wrapped cell should have extra lines, got {}",
            lines.len()
        );

        // Verify content is preserved across wrapped lines
        let rendered: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join(" ");
        assert!(
            rendered.contains("description"),
            "Wrapped content 'description' should be preserved"
        );
    }

    #[test]
    fn test_render_table_box_rigid_elastic() {
        // ID column rigid, Description column elastic
        let content =
            "| ID | Description |\n|---|---|\n| 1 | A very long description text |\n| 2 | Short |";
        let lines = render_table_box(content, 40, MarkdownTheme::Dark);

        // Should render without overflow
        for line in &lines {
            let line_width: usize = line.spans.iter().map(|s| visual_width(&s.content)).sum();
            assert!(
                line_width <= 40,
                "Line width {} exceeds max 40: {:?}",
                line_width,
                line
            );
        }

        // ID content should appear in output
        let rendered: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<&str>>()
            .join(" ");
        assert!(rendered.contains("1"));
        assert!(rendered.contains("2"));
    }

    #[test]
    fn test_render_table_responsiveness() {
        // Same table at two widths — should produce different column widths
        let content =
            "| Name | Description |\n|------|-------------|\n| Foo  | A long description |";

        let lines_wide = render_table_box(content, 80, MarkdownTheme::Dark);
        let lines_narrow = render_table_box(content, 30, MarkdownTheme::Dark);

        // Both should render without overflow
        for line in &lines_wide {
            let w: usize = line.spans.iter().map(|s| visual_width(&s.content)).sum();
            assert!(w <= 80);
        }
        for line in &lines_narrow {
            let w: usize = line.spans.iter().map(|s| visual_width(&s.content)).sum();
            assert!(w <= 30);
        }

        // Narrow rendering may have more lines due to wrapping
        assert!(
            lines_narrow.len() >= lines_wide.len(),
            "Narrow table ({} lines) should have >= wide table ({} lines) due to wrapping",
            lines_narrow.len(),
            lines_wide.len()
        );
    }

    #[test]
    fn test_render_markdown_table_not_dropped() {
        let content = "# Results\n\n| Name | Value |\n|------|-------|\n| Foo  | 42    |\n\nDone.";
        let text = render_markdown(content, MarkdownTheme::Dark, 80);
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

    #[test]
    fn test_build_hline_basic() {
        let hline = build_hline(&[6, 4], BD_TL, BD_TM, BD_TR);
        assert!(hline.starts_with('┌'));
        assert!(hline.contains('┬'));
        assert!(hline.ends_with('┐'));
    }

    // ── Table collapsing tests ──────────────────────────────────────

    #[test]
    fn test_collapse_tables_no_table() {
        let content = "Hello world\n\nThis is a paragraph.";
        assert_eq!(collapse_tables(content), content);
    }

    #[test]
    fn test_collapse_tables_simple() {
        let content = "Results:\n\n| Name | Value |\n|------|-------|\n| Foo  | 42    |";
        let collapsed = collapse_tables(content);
        assert!(
            collapsed.contains("(...)"),
            "Table should be replaced with (...)"
        );
        assert!(
            collapsed.contains("Results:"),
            "Text before table should be preserved"
        );
        assert!(
            !collapsed.contains("| Name |"),
            "Table content should be removed"
        );
        assert!(
            !collapsed.contains("|------|"),
            "Table separator should be removed"
        );
    }

    #[test]
    fn test_collapse_tables_between_text() {
        let content = "Before\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nAfter";
        let collapsed = collapse_tables(content);
        assert!(collapsed.contains("Before"), "Text before table preserved");
        assert!(collapsed.contains("(...)"), "Table replaced with (...)");
        assert!(collapsed.contains("After"), "Text after table preserved");
    }

    #[test]
    fn test_collapse_tables_in_code_block_preserved() {
        let content = "```\n| A | B |\n|---|---|\n| 1 | 2 |\n```";
        let collapsed = collapse_tables(content);
        // Table inside code block should NOT be collapsed
        assert!(
            collapsed.contains("| A | B |"),
            "Table inside code block should be preserved"
        );
        assert!(
            !collapsed.contains("(...)"),
            "Code block table should not be replaced"
        );
    }

    #[test]
    fn test_collapse_tables_multiple() {
        let content =
            "| X | Y |\n|---|---|\n| 1 | 2 |\n\nSome text\n\n| A | B |\n|---|---|\n| 3 | 4 |";
        let collapsed = collapse_tables(content);
        let count = collapsed.matches("(...)").count();
        assert_eq!(count, 2, "Two tables should produce two (...) placeholders");
    }
}
