//! Banner rendering for the TUI welcome screen
//!
//! Parses the ANSI-colored braille art from `EXTENDED_MIND_ART` into ratatui
//! `Line`s with brightened RGB colors. Provides responsive banner layout:
//!
//! - **>= 80 cols**: ASCII art title + braille art left / info right
//! - **35-79 cols**: Styled "SPRACHSPIEL" text + info below
//! - **< 35 cols**: Info only

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};

use super::styles;
use crate::chat::view::EXTENDED_MIND_ART;

// ── Ratatui colors — brightened +20-30 from originals ──────────────────
// Originals (preserved for potential revert):
//   SPRACH gold:    ANSI 220 → R=255 G=215 B=0   (original)
//   SPIEL cyan:     ANSI 45  → R=0   G=175 B=215 (original)
//   SPRACH amber:   ANSI 136 → R=175 G=135 B=0   (original)
//   SPIEL teal:     ANSI 36  → R=0   G=175 B=135 (original)

const GOLD: Color = Color::Rgb(255, 225, 30);
// Original: Color::Rgb(255, 215, 0) — ANSI 220

const BRIGHT_CYAN: Color = Color::Rgb(30, 195, 245);
// Original: Color::Rgb(0, 175, 215) — ANSI 45

const DARK_AMBER: Color = Color::Rgb(195, 155, 30);
// Original: Color::Rgb(175, 135, 0) — ANSI 136

const TEAL: Color = Color::Rgb(30, 195, 155);
// Original: Color::Rgb(0, 175, 135) — ANSI 36

// ── ASCII art logo (from BANNER_LOGO, stripped of ANSI) ──────────────
// Line 1: SPRACH(18) + SPIEL(13) = 31 cols
// Line 2: SPRACH(18) + SPIEL(13) = 31 cols
// Line 3: SPRACH(18) + SPIEL(13) = 31 cols

const LOGO_SPRACH_LINE1: &str = "┏━┓┏━┓┏━┓┏━┓┏━╸╻ ╻";
const LOGO_SPIEL_LINE1: &str = "┏━┓┏━┓╻┏━╸╻  ";
const LOGO_SPRACH_LINE2: &str = "┗━┓┣━┛┣┳┛┣━┫┃  ┣━┫";
const LOGO_SPIEL_LINE2: &str = "┗━┓┣━┛┃┣╸ ┃  ";
const LOGO_SPRACH_LINE3: &str = "┗━┛╹  ╹┗╸╹ ╹┗━╸╹ ╹";
const LOGO_SPIEL_LINE3: &str = "┗━┛╹  ╹┗━╸┗━╸";

/// Minimum terminal width for wide layout (ASCII art + braille art + info)
pub const WIDE_LAYOUT_MIN_COLS: u16 = 80;
/// Minimum terminal width for compact layout (styled text + info)
const COMPACT_LAYOUT_MIN_COLS: u16 = 35;

/// Visual width of the braille art (39 columns)
const BRILLE_ART_WIDTH: usize = 39;

// ── ANSI parsing ─────────────────────────────────────────────────────

/// Parse an ANSI-escape-colored string into a ratatui `Line`.
///
/// Supports `\x1B[38;2;R;G;Bm` true-color sequences and `\x1B[0m` resets.
/// Brightens each RGB color channel by +30 (clamped to 255) for a more
/// vivid TUI appearance.
fn parse_ansi_to_line(ansi_str: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_color: Option<Color> = None;
    let mut chars = ansi_str.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1B' {
            // Flush accumulated text with current color
            if !current_text.is_empty() {
                let style = current_color
                    .map(|c| ratatui::style::Style::default().fg(c))
                    .unwrap_or_default();
                spans.push(Span::styled(std::mem::take(&mut current_text), style));
            }

            // Parse the escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                let mut seq = String::new();
                for seq_ch in chars.by_ref() {
                    if seq_ch.is_ascii_alphabetic() {
                        seq.push(seq_ch);
                        break;
                    }
                    seq.push(seq_ch);
                }

                if seq.starts_with("38;2;") && seq.ends_with('m') {
                    // True-color: \x1B[38;2;R;G;Bm
                    // Brighten each channel by +30 (u8::saturating_add clamps to 255)
                    let parts: Vec<&str> = seq["38;2;".len()..seq.len() - 1].split(';').collect();
                    if parts.len() == 3
                        && let (Ok(r), Ok(g), Ok(b)) = (
                            parts[0].parse::<u8>(),
                            parts[1].parse::<u8>(),
                            parts[2].parse::<u8>(),
                        )
                    {
                        let brightened = Color::Rgb(
                            r.saturating_add(30),
                            g.saturating_add(30),
                            b.saturating_add(30),
                        );
                        current_color = Some(brightened);
                    }
                } else if seq == "0m" {
                    // Reset
                    current_color = None;
                }
                // Other escape sequences (38;5;Xm, etc.) — ignore and continue
            }
        } else {
            current_text.push(ch);
        }
    }

    // Flush remaining text
    if !current_text.is_empty() {
        let style = current_color
            .map(|c| ratatui::style::Style::default().fg(c))
            .unwrap_or_default();
        spans.push(Span::styled(current_text, style));
    }

    if spans.is_empty() {
        Line::raw(String::new())
    } else {
        Line::from(spans)
    }
}

/// Parse all 14 lines of EXTENDED_MIND_ART into ratatui Lines with brightened colors.
fn parse_braille_art_lines() -> Vec<Line<'static>> {
    EXTENDED_MIND_ART
        .iter()
        .map(|line| parse_ansi_to_line(line))
        .collect()
}

// ── ASCII art title lines ────────────────────────────────────────────

/// Line 1: SPRACH (gold) + SPIEL (bright cyan)
fn logo_line1() -> Line<'static> {
    Line::from(vec![
        Span::styled(LOGO_SPRACH_LINE1, ratatui::style::Style::default().fg(GOLD)),
        Span::styled(
            LOGO_SPIEL_LINE1,
            ratatui::style::Style::default().fg(BRIGHT_CYAN),
        ),
    ])
}

/// Line 2: SPRACH (gold) + SPIEL (bright cyan)
fn logo_line2() -> Line<'static> {
    Line::from(vec![
        Span::styled(LOGO_SPRACH_LINE2, ratatui::style::Style::default().fg(GOLD)),
        Span::styled(
            LOGO_SPIEL_LINE2,
            ratatui::style::Style::default().fg(BRIGHT_CYAN),
        ),
    ])
}

/// Line 3: SPRACH (dark amber) + SPIEL (teal)
fn logo_line3() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            LOGO_SPRACH_LINE3,
            ratatui::style::Style::default().fg(DARK_AMBER),
        ),
        Span::styled(LOGO_SPIEL_LINE3, ratatui::style::Style::default().fg(TEAL)),
    ])
}

/// All 3 logo lines
fn logo_lines() -> Vec<Line<'static>> {
    vec![logo_line1(), logo_line2(), logo_line3()]
}

// ── Session info helpers ─────────────────────────────────────────────

/// Build session info lines as styled ratatui `Line`s.
///
/// Each line has a bold label and a dim value, using the system style.
fn build_session_spans(lines: &[String]) -> Vec<Line<'static>> {
    lines
        .iter()
        .map(|line| {
            // Split on ": " to separate label from value
            if let Some((label, value)) = line.split_once(": ") {
                Line::from(vec![
                    Span::styled(
                        format!("{label}: "),
                        styles::system_style().add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::styled(value.to_string(), styles::system_style()),
                ])
            } else {
                Line::from(Span::styled(line.to_string(), styles::system_style()))
            }
        })
        .collect()
}

// ── Layout variants ──────────────────────────────────────────────────

/// Wide layout (>= 80 cols): ASCII art title, then braille art left / info right.
///
/// Structure:
/// ```text
/// ──────────────────────────────────────────────────...
/// ┏━┓┏━┓┏━┓┏━┓┏━╸╻ ╻┏━┓┏━┓╻┏━╸╻       (gold/cyan)
/// ┗━┓┣━┛┣┳┛┣━┫┃  ┣━┫┗━┓┣━┛┃┣╸ ┃       (gold/cyan)
/// ┗━┛╹  ╹┗╸╹ ╹┗━╸╹ ╹┗━┛╹  ╹┗━╸┗━╸      (amber/teal)
///                                                   (blank line)
///    ⣀⣤⡀          Model: glm-5:cloud
///    ⠈⠻⠟⠓⠦⣤⣀    Server: 127.0.0.1:11434
///    ...           ...
/// ──────────────────────────────────────────────────...
/// ```
fn wide_layout(session_lines: &[String]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // 1. ASCII art title (3 lines)
    lines.extend(logo_lines());

    // 2. Blank line after title
    lines.push(Line::raw(String::new()));

    // 3. Braille art left (39 cols) + session info right (side by side)
    let braille_lines = parse_braille_art_lines();
    let session_spans = build_session_spans(session_lines);
    let max_rows = braille_lines.len().max(session_spans.len());

    // Gap between braille art and session info
    const GAP: usize = 2;

    for i in 0..max_rows {
        let braille_line = if i < braille_lines.len() {
            &braille_lines[i]
        } else {
            // Braille art ended — just pad with empty line for alignment
            &EMPTY_LINE
        };

        let art_width = line_visual_width(braille_line);

        // Calculate padding after braille art
        let padding = BRILLE_ART_WIDTH.saturating_sub(art_width) + GAP;

        if i < session_spans.len() {
            // Combine braille art + padding + session info
            let mut combined_spans: Vec<Span<'static>> = braille_line.spans.to_vec();
            combined_spans.push(Span::raw(" ".repeat(padding)));
            let session_formatted: Vec<Span<'static>> = session_spans[i].spans.to_vec();
            combined_spans.extend(session_formatted);
            lines.push(Line::from(combined_spans));
        } else {
            // No session info for this row — braille art (already "penduradas"/hanging)
            // Pad the braille line to full art width to maintain alignment
            let mut padded_spans: Vec<Span<'static>> = braille_line.spans.to_vec();
            if art_width < BRILLE_ART_WIDTH {
                padded_spans.push(Span::raw(" ".repeat(BRILLE_ART_WIDTH - art_width)));
            }
            lines.push(Line::from(padded_spans));
        }
    }

    lines
}

/// Empty line for padding (LazyLock because `Line::raw()` is not const)
static EMPTY_LINE: std::sync::LazyLock<Line<'static>> = std::sync::LazyLock::new(|| Line::raw(""));

/// Calculate the visual width of a ratatui Line (total characters of all spans).
fn line_visual_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

/// Compact layout (35-79 cols): styled "SPRACHSPIEL" text + session info.
///
/// When the terminal is too narrow for the ASCII art logo and braille art,
/// we use a styled text title instead. "SPRACHSPIEL" is ONE word in German
/// and must never be split with a space.
fn compact_layout(session_lines: &[String]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Styled title as one word (SPRACHSPIEL, not "SPRACH SPIEL")
    lines.push(Line::from(vec![
        Span::styled(
            "SPRACH",
            ratatui::style::Style::default()
                .fg(GOLD)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(
            "SPIEL",
            ratatui::style::Style::default()
                .fg(BRIGHT_CYAN)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
    ]));

    lines.push(Line::raw(String::new())); // blank line

    lines.extend(build_session_spans(session_lines));

    lines
}

/// Narrow layout (< 35 cols): session info only (minimum viable info).
fn narrow_layout(session_lines: &[String]) -> Vec<Line<'static>> {
    build_session_spans(session_lines)
}

/// Create a separator line using dim horizontal rules.
fn separator_line(width: u16) -> Line<'static> {
    let separator = "─".repeat(width as usize);
    Line::from(Span::styled(
        separator,
        ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::DIM),
    ))
}

/// Build the banner paragraph for the given area width.
///
/// Returns ratatui `Line`s containing:
/// - Responsive banner content based on terminal width
/// - Separators above and below
///
/// No image protocol needed — braille art is pure unicode text that
/// scrolls naturally with the chat content.
pub fn build_banner_lines(area: Rect, session_lines: &[String]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Top separator
    lines.push(separator_line(area.width));

    if area.width >= WIDE_LAYOUT_MIN_COLS {
        // Wide layout: ASCII art title + braille art + side-by-side info
        lines.extend(wide_layout(session_lines));
    } else if area.width >= COMPACT_LAYOUT_MIN_COLS {
        // Compact layout: styled text title + info below
        lines.extend(compact_layout(session_lines));
    } else {
        // Narrow layout: info only
        lines.extend(narrow_layout(session_lines));
    }

    // Bottom separator
    lines.push(separator_line(area.width));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logo_lines_are_valid() {
        let line1 = logo_line1();
        let line2 = logo_line2();
        let line3 = logo_line3();

        assert!(!line1.spans.is_empty());
        assert!(!line2.spans.is_empty());
        assert!(!line3.spans.is_empty());
    }

    #[test]
    fn test_parse_ansi_to_line_plain_text() {
        // Plain text without ANSI should produce a single unstyled span
        let line = parse_ansi_to_line("hello world");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "hello world");
    }

    #[test]
    fn test_parse_ansi_to_line_with_color() {
        // True-color ANSI sequence should produce colored span
        let input = "\x1B[38;2;255;0;0mRed\x1B[0m text";
        let line = parse_ansi_to_line(input);
        // Should have at least 2 spans: "Red" with color, " text" without
        assert!(line.spans.len() >= 2);
    }

    #[test]
    fn test_parse_ansi_to_line_brightness_boost() {
        // Verify that colors are brightened by +30
        let input = "\x1B[38;2;100;150;200mHello\x1B[0m";
        let line = parse_ansi_to_line(input);
        assert!(!line.spans.is_empty());
        // First span should have brightened color: (130, 180, 230)
        if let Some(ratatui::style::Style {
            fg: Some(Color::Rgb(r, g, b)),
            ..
        }) = line.spans.first().map(|s| s.style)
        {
            assert_eq!(r, 130);
            assert_eq!(g, 180);
            assert_eq!(b, 230);
        } else {
            panic!("Expected Rgb color in parsed span");
        }
    }

    #[test]
    fn test_parse_ansi_to_line_brightness_clamp() {
        // Colors near 255 should be clamped, not overflow
        let input = "\x1B[38;2;250;240;255mMax\x1B[0m";
        let line = parse_ansi_to_line(input);
        if let Some(ratatui::style::Style {
            fg: Some(Color::Rgb(r, g, b)),
            ..
        }) = line.spans.first().map(|s| s.style)
        {
            assert_eq!(r, 255); // 250+30=280 → clamped to 255
            assert_eq!(g, 255); // 240+30=270 → clamped to 255
            assert_eq!(b, 255); // 255+30=285 → clamped to 255
        }
    }

    #[test]
    fn test_braille_art_lines_parse() {
        let lines = parse_braille_art_lines();
        assert_eq!(lines.len(), 14, "EXTENDED_MIND_ART has 14 lines");
        // Each line should have at least one span
        for (i, line) in lines.iter().enumerate() {
            assert!(
                !line.spans.is_empty(),
                "Braille art line {i} should have at least one span"
            );
        }
    }

    #[test]
    fn test_build_banner_lines_narrow() {
        let area = Rect::new(0, 0, 20, 10);
        let lines = vec!["Model: test".to_string(), "Version: 0.1".to_string()];
        let result = build_banner_lines(area, &lines);

        // Should have separator + info lines + separator
        assert!(result.len() >= 4); // At least 2 separators + 2 info lines
    }

    #[test]
    fn test_build_banner_lines_compact() {
        let area = Rect::new(0, 0, 40, 15);
        let lines = vec!["Model: test".to_string(), "Version: 0.1".to_string()];
        let result = build_banner_lines(area, &lines);

        // Compact layout should include "SPRACH" + "SPIEL" styled title
        let has_title = result.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("SPRACH") || s.content.contains("SPIEL"))
        });
        assert!(has_title, "Compact layout should include styled title");
    }

    #[test]
    fn test_build_banner_lines_wide() {
        let area = Rect::new(0, 0, 100, 25);
        let lines = vec!["Model: test".to_string(), "Version: 0.1".to_string()];
        let result = build_banner_lines(area, &lines);

        // Wide layout should include logo lines and braille art
        let has_logo = result.iter().any(|l| {
            l.spans
                .iter()
                .any(|s| s.content.contains("┏━┓") || s.content.contains("┗━┓"))
        });
        assert!(has_logo, "Wide layout should include ASCII art logo");
    }
}
