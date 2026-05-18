//! Shared table parsing and rendering functions for markdown tables.
//!
//! This module contains pure-string functions for detecting, parsing, and
//! rendering markdown tables with box-drawing characters. It is shared
//! between the TUI renderer (which adds ratatui styles) and the standalone
//! renderer (which outputs plain strings for stdout).
//!
//! All functions in this module operate on `&str` and `String` only —
//! no ratatui dependency.

use unicode_width::UnicodeWidthStr;

// ── Column alignment ─────────────────────────────────────────────────

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

// ── Box-drawing constants for table borders ──────────────────────────

/// `│` Light vertical
pub const BD_VLINE: &str = "│";
/// `─` Light horizontal
pub const BD_HLINE: &str = "─";
/// `┌` Top-left corner
pub const BD_TL: &str = "┌";
/// `┬` Top-middle (down + horizontal)
pub const BD_TM: &str = "┬";
/// `┐` Top-right corner
pub const BD_TR: &str = "┐";
/// `├` Middle-left (right + horizontal)
pub const BD_ML: &str = "├";
/// `┼` Cross (vertical + horizontal)
pub const BD_MC: &str = "┼";
/// `┤` Middle-right (left + horizontal)
pub const BD_MR: &str = "┤";
/// `└` Bottom-left corner
pub const BD_BL: &str = "└";
/// `┴` Bottom-middle (up + horizontal)
pub const BD_BM: &str = "┴";
/// `┘` Bottom-right corner
pub const BD_BR: &str = "┘";

// ── Table detection ─────────────────────────────────────────────────

/// A segment of markdown content — either a regular block or a table.
#[derive(Debug)]
pub enum ContentSegment {
    /// Regular markdown content (rendered via tui-markdown or standalone)
    Markdown(String),
    /// Table block (rendered with box-drawing borders)
    Table(String),
}

/// Check if a line looks like a table row (starts and ends with `|`).
pub fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 2
}

/// Check if a line is a table separator (`|---|---|`, `|:---:|:---:|`, etc.).
pub fn is_table_separator(line: &str) -> bool {
    parse_separator_line(line).is_some()
}

/// Detect markdown table blocks in content and split into segments.
///
/// Tables inside fenced code blocks are NOT detected as tables.
pub fn extract_table_segments(content: &str) -> Vec<ContentSegment> {
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

// ── Separator parsing ───────────────────────────────────────────────

/// Parse a separator line and extract alignment hints.
///
/// Returns `None` if the line is not a valid table separator.
/// Returns `Some(Vec<ColumnAlign>)` with one alignment per column.
pub fn parse_separator_line(line: &str) -> Option<Vec<ColumnAlign>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }

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
            aligns.push(ColumnAlign::Left);
            continue;
        }
        if !cell_trimmed
            .chars()
            .all(|c| c == '-' || c == ':' || c == ' ')
        {
            return None;
        }
        if cell_trimmed.contains('-') {
            has_dash = true;
        }
        let starts_with_colon = cell_trimmed.starts_with(':');
        let ends_with_colon = cell_trimmed.ends_with(':');
        let align = match (starts_with_colon, ends_with_colon) {
            (true, true) => ColumnAlign::Center,
            (false, true) => ColumnAlign::Right,
            _ => ColumnAlign::Left,
        };
        aligns.push(align);
    }

    if !has_dash {
        return None;
    }

    Some(aligns)
}

// ── Table parsing ───────────────────────────────────────────────────

/// Parsed table structure with headers, data rows, and column alignments.
pub struct ParsedTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub aligns: Vec<ColumnAlign>,
}

/// Parse a raw table block into headers, rows, and column alignments.
pub fn parse_table_rows(content: &str) -> ParsedTable {
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
pub fn split_table_cells(line: &str) -> Vec<String> {
    let parts: Vec<&str> = line.trim().split('|').collect();
    let cell_count = parts.len().saturating_sub(2);
    (0..cell_count)
        .filter_map(|i| parts.get(i + 1).map(|s| s.trim().to_string()))
        .collect()
}

// ── Visual width ─────────────────────────────────────────────────────

/// Measure the visual width of a string using `unicode-width`.
pub fn visual_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

// ── Column width calculation ─────────────────────────────────────────

/// Threshold for classifying a column as "rigid" (non-wrappable).
///
/// A column is rigid if **all** of its cells (header + data) have
/// `visual_width ≤ RIGID_THRESHOLD`. Rigid columns receive their
/// natural width exactly and never wrap.
pub const RIGID_THRESHOLD: usize = 6;

/// Calculate column widths for a table, fitting within `max_width`.
///
/// Uses an intelligent rigid/elastic classification:
/// - **Rigid columns**: natural width ≤ threshold (short content like IDs,
///   numbers). Allocated their exact natural width — never wrap.
/// - **Elastic columns**: natural width > threshold (long content like
///   descriptions, text). Receive the remaining space after rigid
///   allocation — may wrap their cell content.
pub fn calculate_col_widths(
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

    // Step 3: Redistribute any unused space
    let total_allocated: usize = col_widths.iter().sum();
    if total_allocated > available {
        let deficit = total_allocated - available;
        let mut remaining_deficit = deficit;
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

    // Step 4: Redistribute any surplus
    let total_final: usize = col_widths.iter().sum();
    if total_final < available {
        let surplus = available - total_final;
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
pub fn distribute_proportionally(
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

// ── Table border construction ────────────────────────────────────────

/// Build a horizontal border line for a table.
///
/// Example with 2 columns of width 10 and 8:
/// `"┌──────────┬────────┐"`
pub fn build_hline(col_widths: &[usize], left: &str, mid: &str, right: &str) -> String {
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
pub fn wrap_cell_content(text: &str, width: usize, _align: ColumnAlign) -> Vec<String> {
    if width <= 3 {
        return vec![crate::utils::truncate_visual_width(text, width)];
    }

    let text_width = visual_width(text);
    if text_width <= width {
        return vec![text.to_string()];
    }

    // Word-wrap the cell content
    crate::chat::tui::wrap::wrap_line(text, width)
}

/// Apply alignment padding to a sub-line within a cell.
///
/// Given the text of a sub-line and the column width, returns
/// `(left_pad, content, right_pad)` strings for proper alignment.
pub fn align_cell_text(
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

// ── Standalone table rendering (String output, no ratatui) ──────────

/// Build a table row as plain strings (no ratatui styles).
///
/// Each sub-line gets `│` borders. Returns `Vec<String>` where each
/// string is one visual line of the expanded row.
pub fn build_row_expanded_string(
    col_widths: &[usize],
    aligns: &[ColumnAlign],
    cells: &[String],
    is_header: bool,
) -> Vec<String> {
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

    // Build strings: one per sub-line row
    let mut result = Vec::with_capacity(max_height);

    for sub_idx in 0..max_height {
        let mut line = BD_VLINE.to_string();

        for (col_idx, &width) in col_widths.iter().enumerate() {
            let align = aligns.get(col_idx).copied().unwrap_or(ColumnAlign::Left);
            let sub_text = wrapped_cells
                .get(col_idx)
                .and_then(|lines| lines.get(sub_idx))
                .map(|s| s.as_str())
                .unwrap_or("");

            let (left_pad, content, right_pad) = align_cell_text(sub_text, width, align);

            line.push(' '); // cell left pad
            line.push_str(&left_pad);
            line.push_str(&content);
            line.push_str(&right_pad);
            line.push(' '); // cell right pad
            line.push_str(BD_VLINE);
        }

        // Add ANSI bold for header rows
        if is_header {
            result.push(format!("\x1B[1m{}\x1B[0m", line));
        } else {
            result.push(line);
        }
    }

    result
}

/// Render a table block as plain strings with box-drawing borders.
///
/// Returns a vector of strings, one per visual line, ready for printing
/// to stdout. Headers use ANSI bold.
pub fn render_table_box_string(content: &str, max_width: usize) -> Vec<String> {
    let parsed = parse_table_rows(content);
    if parsed.headers.is_empty() {
        // Not a valid table — return original lines
        return content.lines().map(|l| l.to_string()).collect();
    }

    let col_widths = calculate_col_widths(&parsed.headers, &parsed.rows, &parsed.aligns, max_width);

    if col_widths.is_empty() {
        return content.lines().map(|l| l.to_string()).collect();
    }

    let mut lines = Vec::new();

    // Top border
    lines.push(build_hline(&col_widths, BD_TL, BD_TM, BD_TR));

    // Header row
    lines.extend(build_row_expanded_string(
        &col_widths,
        &parsed.aligns,
        &parsed.headers,
        true,
    ));

    // Header/data separator
    lines.push(build_hline(&col_widths, BD_ML, BD_MC, BD_MR));

    // Data rows with separators
    for (row_idx, row) in parsed.rows.iter().enumerate() {
        lines.extend(build_row_expanded_string(
            &col_widths,
            &parsed.aligns,
            row,
            false,
        ));
        // Row separator (between data rows only)
        if row_idx < parsed.rows.len() - 1 {
            lines.push(build_hline(&col_widths, BD_ML, BD_MC, BD_MR));
        }
    }

    // Bottom border
    lines.push(build_hline(&col_widths, BD_BL, BD_BM, BD_BR));

    lines
}

/// Render a plain table (pipe-delimited, no box-drawing) for `--plain` mode.
///
/// Tables are rendered as simple `| col1 | col2 |` with alignment
/// padding. No ANSI codes, no box-drawing characters.
pub(super) fn render_table_plain(content: &str, _max_width: usize) -> Vec<String> {
    let parsed = parse_table_rows(content);

    if parsed.headers.is_empty() {
        return content.lines().map(|l| l.to_string()).collect();
    }

    // For plain mode, use natural column widths (no wrapping)
    let col_count = parsed.headers.len();

    let natural_widths: Vec<usize> = (0..col_count)
        .map(|c| {
            let hw = visual_width(&parsed.headers[c]);
            let rw = parsed
                .rows
                .iter()
                .filter_map(|r| r.get(c).map(|cell| visual_width(cell)))
                .max()
                .unwrap_or(0);
            hw.max(rw)
        })
        .collect();

    let mut lines = Vec::new();

    // Header
    let mut header_line = String::from("| ");
    for (i, header) in parsed.headers.iter().enumerate() {
        let width = natural_widths[i];
        let align = parsed.aligns.get(i).copied().unwrap_or(ColumnAlign::Left);
        let (left, content, right) = align_cell_text(header, width, align);
        header_line.push_str(&left);
        header_line.push_str(&content);
        header_line.push_str(&right);
        if i < col_count - 1 {
            header_line.push_str(" | ");
        }
    }
    header_line.push_str(" |");
    lines.push(header_line);

    // Separator
    let mut sep_line = String::from("| ");
    for (i, _) in natural_widths.iter().enumerate() {
        let align = parsed.aligns.get(i).copied().unwrap_or(ColumnAlign::Left);
        let prefix = if matches!(align, ColumnAlign::Center | ColumnAlign::Left) {
            ":"
        } else {
            ""
        };
        let suffix = if matches!(align, ColumnAlign::Center | ColumnAlign::Right) {
            ":"
        } else {
            ""
        };
        let total = natural_widths[i].saturating_sub(prefix.len() + suffix.len());
        sep_line.push_str(prefix);
        sep_line.push_str(&"-".repeat(total.max(1)));
        sep_line.push_str(suffix);
        if i < col_count - 1 {
            sep_line.push_str(" | ");
        }
    }
    sep_line.push_str(" |");
    lines.push(sep_line);

    // Data rows
    for row in &parsed.rows {
        let mut row_line = String::from("| ");
        for (i, cell) in row.iter().enumerate() {
            let width = natural_widths.get(i).copied().unwrap_or(cell.len());
            let align = parsed.aligns.get(i).copied().unwrap_or(ColumnAlign::Left);
            let (left, content, right) = align_cell_text(cell, width, align);
            row_line.push_str(&left);
            row_line.push_str(&content);
            row_line.push_str(&right);
            if i < col_count - 1 {
                row_line.push_str(" | ");
            }
        }
        row_line.push_str(" |");
        lines.push(row_line);
    }

    lines
}
