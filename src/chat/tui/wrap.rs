//! Unicode-aware word-wrapping for TUI rendering
//!
//! Provides `wrap_line`, `hard_break_word`, and `wrap_styled_line` for
//! breaking text at visual column boundaries. Shared by `chat_area`
//! (thinking blocks) and `markdown` (table cell wrapping).
//!
//! # Unicode awareness
//!
//! CJK characters count as 2 visual columns; combining characters as 0.
//! Uses `unicode_width::UnicodeWidthStr` for string-level width (handles
//! emoji ZWJ sequences like 🇧🇷 and 👨‍💻 correctly) and `UnicodeWidthChar`
//! for per-character iteration in `hard_break_word`.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// Word-wrap a line of text to fit within `width` visual columns,
/// breaking at spaces when possible. Unicode-aware: CJK characters
/// count as 2 columns, combining characters as 0, emoji ZWJ sequences
/// (🇧🇷, 👨‍💻) count as 2 columns.
///
/// If a single word exceeds `width`, it is hard-broken at the column limit.
/// Returns owned Strings because Unicode-aware slicing cannot return &str.
pub fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }

    let visual_len = line.width();
    if visual_len <= width {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0usize;

    for word in line.split_whitespace() {
        let word_width = word.width();

        if current_width == 0 {
            // First word on the line
            if word_width <= width {
                current_line.push_str(word);
                current_width = word_width;
            } else {
                // Word is wider than available width — hard-break it
                let chunks = hard_break_word(word, width);
                result.extend(chunks);
            }
        } else if current_width + 1 + word_width <= width {
            // Word fits on current line with a space
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Word doesn't fit — push current line and start new one
            result.push(current_line);

            if word_width <= width {
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Word is wider than available width — hard-break it
                let chunks = hard_break_word(word, width);
                result.extend(chunks);
                current_line = String::new();
                current_width = 0;
            }
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    result
}

/// Hard-break a word that exceeds `width` visual columns.
///
/// Breaks at Unicode character boundaries, splitting the word into chunks
/// that each fit within `width` columns (accounting for CJK double-width).
pub fn hard_break_word(word: &str, width: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut chunk = String::new();
    let mut chunk_width = 0usize;

    for ch in word.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if chunk_width + ch_width > width && !chunk.is_empty() {
            result.push(chunk);
            chunk = String::new();
            chunk_width = 0;
        }
        chunk.push(ch);
        chunk_width += ch_width;
    }

    if !chunk.is_empty() {
        result.push(chunk);
    }

    result
}

/// Word-wrap a styled `Line` to fit within `width` visual columns,
/// preserving the style of each span.
///
/// Each span's text is split at word boundaries when possible, and
/// the resulting sub-lines inherit the same style as the source span.
/// For hard breaks (words wider than `width`), characters are split
/// at visual column boundaries with the source style preserved.
///
/// Returns a vector of `Line<'static>` values, each fitting within
/// `width` visual columns. The returned `Line` style is `Style::default()`
/// (no line-level style); all styling is carried by the individual spans.
pub fn wrap_styled_line(line: Line<'_>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        // Cannot wrap — return as-is (converted to owned)
        return vec![line_to_owned(line)];
    }

    // Propagate Line.style to each Span before processing.
    // tui-markdown renders headings as Line { spans: [Span::raw(...)], style: heading_style }
    // where heading_style carries color, bold, underline. The Line.style acts as a
    // fallback for Spans with Style::default(). If we don't propagate it, heading
    // styles (including underline) are silently lost during wrapping.
    let base_style = line.style;
    let normalized_spans: Vec<Span<'_>> = line
        .spans
        .into_iter()
        .map(|span| Span::styled(span.content, base_style.patch(span.style)))
        .collect();

    // Flatten spans into (char, style) pairs for width-aware iteration
    let chars: Vec<(char, Style)> = normalized_spans
        .iter()
        .flat_map(|span| span.content.chars().map(|c| (c, span.style)))
        .collect();

    // Use span-level width for quick check (handles emoji ZWJ correctly).
    // The char-level approach would undercount flag emojis (🇧🇷 = 0 per char,
    // 2 per string). Falls through to word-wrap only when needed.
    let total_width: usize = normalized_spans.iter().map(|s| s.content.width()).sum();

    if total_width <= width {
        // Line fits — return the normalized line (with Line.style propagated to Spans)
        let owned_spans: Vec<Span<'static>> = normalized_spans
            .into_iter()
            .map(|span| Span::styled(span.content.into_owned(), span.style))
            .collect();
        return vec![Line::from(owned_spans)];
    }

    // Strategy: word-wrap the flat character stream.
    // 1. Split into "words" (runs of non-space chars separated by spaces)
    // 2. Accumulate words into lines, breaking when width exceeded
    // 3. Each word's chars carry their original style

    // Extract words as Vec<Vec<(char, Style)>>
    let mut words: Vec<Vec<(char, Style)>> = Vec::new();
    let mut current_word: Vec<(char, Style)> = Vec::new();

    for (ch, style) in chars {
        if ch == ' ' {
            if !current_word.is_empty() {
                words.push(std::mem::take(&mut current_word));
            }
            // Spaces are word separators — not included in words.
            // Spaces *between* words are added during line building.
        } else {
            current_word.push((ch, style));
        }
    }
    if !current_word.is_empty() {
        words.push(current_word);
    }

    if words.is_empty() {
        // All spaces or empty — return single empty line
        return vec![Line::raw(String::new())];
    }

    // Build lines from words
    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_line_width: usize = 0;

    for word in &words {
        // Reconstruct string from chars for span-level width measurement.
        // This correctly handles emoji ZWJ sequences (🇧🇷 width 2 per string,
        // 0 per individual regional indicator chars).
        let word_string: String = word.iter().map(|(c, _)| c).collect();
        let word_width = word_string.width();

        if current_line_width == 0 {
            // First word on the line
            if word_width <= width {
                append_word_to_spans(word, &mut current_spans);
                current_line_width = word_width;
            } else {
                // Word wider than width — hard-break it
                let broken_lines = hard_break_word_styled(word, width);
                if broken_lines.is_empty() {
                    continue;
                }
                // All sub-lines except the last go directly to result.
                // The last sub-line becomes the current line (can receive
                // more words if they fit).
                #[expect(clippy::expect_used)] // checked non-empty above
                let (last, rest) = broken_lines.split_last().expect("non-empty");
                result.extend(
                    rest.iter()
                        .map(|l| Line::from(l.spans.clone().into_iter().collect::<Vec<_>>())),
                );
                current_spans = last.spans.clone().into_iter().collect();
                current_line_width = measure_spans_width(&current_spans);
            }
        } else if current_line_width + 1 + word_width <= width {
            // Word fits on current line with a space
            // Use the space style from the first char of the word (or default)
            let space_style = word.first().map(|&(_, s)| s).unwrap_or_default();
            current_spans.push(Span::styled(" ".to_string(), space_style));
            append_word_to_spans(word, &mut current_spans);
            current_line_width += 1 + word_width;
        } else {
            // Word doesn't fit — finish current line, start new one
            result.push(Line::from(std::mem::take(&mut current_spans)));

            if word_width <= width {
                append_word_to_spans(word, &mut current_spans);
                current_line_width = word_width;
            } else {
                // Word wider than width — hard-break it
                let broken_lines = hard_break_word_styled(word, width);
                if broken_lines.is_empty() {
                    current_line_width = 0;
                    continue;
                }
                #[expect(clippy::expect_used)] // checked non-empty above
                let (last, rest) = broken_lines.split_last().expect("non-empty");
                result.extend(
                    rest.iter()
                        .map(|l| Line::from(l.spans.clone().into_iter().collect::<Vec<_>>())),
                );
                current_spans = last.spans.clone().into_iter().collect();
                current_line_width = measure_spans_width(&current_spans);
            }
        }
    }

    // Finish last line
    if !current_spans.is_empty() {
        result.push(Line::from(current_spans));
    }

    result
}

/// Measure the visual width of a slice of `Span` values.
///
/// Uses `UnicodeWidthStr::width()` per span to correctly handle emoji
/// ZWJ sequences and flag emojis (which have different widths when
/// measured as multi-char strings vs. individual codepoints).
fn measure_spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// Append a word's characters to a spans vector, merging consecutive
/// chars with the same style into single `Span`s.
fn append_word_to_spans(word: &[(char, Style)], spans: &mut Vec<Span<'static>>) {
    if word.is_empty() {
        return;
    }
    let mut buf = String::new();
    let mut current_style = word[0].1;
    for &(ch, style) in word {
        if style != current_style && !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut buf), current_style));
        }
        current_style = style;
        buf.push(ch);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, current_style));
    }
}

/// Hard-break a word (list of styled characters) that exceeds `width`.
///
/// Returns a vector of `Line<'static>` values, each at most `width`
/// visual columns wide. Styles are preserved from the source word.
fn hard_break_word_styled(word: &[(char, Style)], width: usize) -> Vec<Line<'static>> {
    if width == 0 || word.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut current_style = word[0].1;
    let mut current_width: usize = 0;

    for &(ch, style) in word {
        let ch_width = ch.width().unwrap_or(0);

        if current_width + ch_width > width && !buf.is_empty() {
            // Flush buffer as a span
            current_spans.push(Span::styled(std::mem::take(&mut buf), current_style));
            result.push(Line::from(std::mem::take(&mut current_spans)));
            current_width = 0;
        }

        if style != current_style && !buf.is_empty() {
            // Style change — flush buffer
            current_spans.push(Span::styled(std::mem::take(&mut buf), current_style));
            current_style = style;
        } else if buf.is_empty() {
            current_style = style;
        }

        buf.push(ch);
        current_width += ch_width;
    }

    // Flush remaining
    if !buf.is_empty() {
        current_spans.push(Span::styled(buf, current_style));
    }
    if !current_spans.is_empty() {
        result.push(Line::from(current_spans));
    }

    result
}

/// Convert a `Line<'a>` to `Line<'static>` by making all spans owned.
fn line_to_owned(line: Line<'_>) -> Line<'static> {
    let owned_spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|span| Span::styled(span.content.into_owned(), span.style))
        .collect();
    let mut owned_line = Line::from(owned_spans);
    // Preserve Line.style (e.g., heading underline/bold from tui-markdown)
    owned_line.style = line.style;
    owned_line
}

#[cfg(test)]
mod wrap_line_tests {
    use super::*;

    // Helper to create Vec<String> for comparison with wrap_line output
    macro_rules! sv {
        ($($s:expr),* $(,)?) => { vec![$($s.to_string()),*] };
    }

    #[test]
    fn test_wrap_line_short() {
        // Line fits within width — returned as-is
        let result = wrap_line("hello", 80);
        assert_eq!(result, sv!["hello"]);
    }

    #[test]
    fn test_wrap_line_break_at_space() {
        // "hello world foo" with width=11:
        // "hello" (5) + " " (1) + "world" (5) = 11 → "hello world" fits
        // Then " foo" → need new line: "foo" (3).
        // Result: ["hello world", "foo"]
        let result = wrap_line("hello world foo", 11);
        assert_eq!(result, sv!["hello world", "foo"]);
    }

    #[test]
    fn test_wrap_line_space_at_boundary() {
        // "one two three" with width=7:
        // "one" (3) → fits. " two" (4) → 3+1+3=7 ≤ 7 → "one two" fits.
        // " three" (6) → need new line: "three" (5).
        let result = wrap_line("one two three", 7);
        assert_eq!(result, sv!["one two", "three"]);
    }

    #[test]
    fn test_wrap_line_no_space() {
        // No space — hard break using Unicode char boundaries
        let result = wrap_line("abcdefghij", 5);
        assert_eq!(result, sv!["abcde", "fghij"]);
    }

    #[test]
    fn test_wrap_line_multiple_breaks() {
        // "one two three four five" with width=8:
        let result = wrap_line("one two three four five", 8);
        assert_eq!(result, sv!["one two", "three", "four", "five"]);
    }

    #[test]
    fn test_wrap_line_empty() {
        let result = wrap_line("", 80);
        assert_eq!(result, sv![""]);
    }

    #[test]
    fn test_wrap_line_zero_width() {
        // Zero width should return as-is (cannot wrap)
        let result = wrap_line("hello", 0);
        assert_eq!(result, sv!["hello"]);
    }

    #[test]
    fn test_wrap_line_unicode() {
        // "olá mundo" — "olá" is 3 visual cols, "mundo" is 5
        let result = wrap_line("olá mundo", 10);
        assert_eq!(result, sv!["olá mundo"]);

        let result = wrap_line("olá mundo", 5);
        assert_eq!(result, sv!["olá", "mundo"]);
    }

    #[test]
    fn test_wrap_line_cjk() {
        // CJK characters are 2 columns wide
        // "日本語" = 3 chars × 2 cols = 6 visual cols
        let result = wrap_line("日本語 test", 8);
        assert_eq!(result, sv!["日本語", "test"]);
    }

    #[test]
    fn test_hard_break_word_basic() {
        let result = hard_break_word("abcdefghij", 5);
        assert_eq!(result, sv!["abcde", "fghij"]);
    }

    #[test]
    fn test_hard_break_word_cjk() {
        // Each CJK char is 2 cols wide; width=4 → 2 chars per chunk
        let result = hard_break_word("日本語", 4);
        assert_eq!(result, sv!["日本", "語"]);
    }

    #[test]
    fn test_hard_break_word_short() {
        // Word fits in width — single chunk
        let result = hard_break_word("hi", 10);
        assert_eq!(result, sv!["hi"]);
    }

    #[test]
    fn test_wrap_line_emoji() {
        // ✅ is a single codepoint emoji with width 2.
        // "✅ done" = 2 + 1 + 4 = 7 visual cols
        let result = wrap_line("✅ done", 10);
        assert_eq!(result, sv!["✅ done"]);

        // Force wrap: "✅ done" at width=5 → "✅" (2) + " done" (5) = 7 > 5 → wrap
        let result = wrap_line("✅ done", 5);
        assert_eq!(result, sv!["✅", "done"]);
    }

    #[test]
    fn test_wrap_line_flag_emoji() {
        // 🇧🇷 is a flag emoji (2 regional indicators: 🇧 + 🇷).
        // UnicodeWidthChar::width() for each regional indicator = None (0 with unwrap_or).
        // UnicodeWidthStr::width() for "🇧🇷" = 2.
        // With str-level width, this works correctly.
        // "🇧🇷 flag" = 2 + 1 + 4 = 7 visual cols
        let result = wrap_line("🇧🇷 flag", 10);
        assert_eq!(result, sv!["🇧🇷 flag"]);

        // Force wrap at width=4: "🇧🇷" (2) + " flag" (5) = 7 > 4 → wrap
        // "🇧🇷" (2) fits in 4, "flag" (4) fits in 4
        let result = wrap_line("🇧🇷 flag", 4);
        assert_eq!(result, sv!["🇧🇷", "flag"]);
    }

    #[test]
    fn test_wrap_line_zwj_emoji() {
        // 👨‍💻 is a ZWJ sequence (man + ZWJ + laptop).
        // UnicodeWidthStr::width() = 2 (emoji presentation).
        // "👨‍💻 code" = 2 + 1 + 4 = 7 visual cols
        let result = wrap_line("👨‍💻 code", 10);
        assert_eq!(result, sv!["👨‍💻 code"]);

        // Force wrap: "👨‍💻" (2) + " code" (5) = 7 > 5 → wrap
        let result = wrap_line("👨‍💻 code", 5);
        assert_eq!(result, sv!["👨‍💻", "code"]);
    }
}

#[cfg(test)]
mod wrap_styled_line_tests {
    use super::*;
    use ratatui::style::Color;

    /// Helper: extract plain text from a Line
    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn test_wrap_styled_line_fits() {
        // Line fits within width — returned as-is
        let line = Line::from(vec![Span::styled(
            "hello world".to_string(),
            Style::default().fg(Color::Red),
        )]);
        let result = wrap_styled_line(line, 80);
        assert_eq!(result.len(), 1);
        assert_eq!(line_text(&result[0]), "hello world");
        // Style preserved
        assert_eq!(result[0].spans[0].style, Style::default().fg(Color::Red));
    }

    #[test]
    fn test_wrap_styled_line_break_at_space() {
        // "hello world foo" with width=11
        let line = Line::from(vec![Span::styled(
            "hello world foo".to_string(),
            Style::default(),
        )]);
        let result = wrap_styled_line(line, 11);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "hello world");
        assert_eq!(line_text(&result[1]), "foo");
    }

    #[test]
    fn test_wrap_styled_line_preserves_styles() {
        // "bold normal" with different styles per word
        let line = Line::from(vec![
            Span::styled("bold".to_string(), Style::default().fg(Color::Red)),
            Span::raw(" ".to_string()),
            Span::styled("normal".to_string(), Style::default().fg(Color::Blue)),
        ]);
        // Width=6 → "bold" (4) + " normal" (7) = 11 > 6 → new line
        // "bold" fits in 6, "normal" (6) fits in 6
        let result = wrap_styled_line(line, 6);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "bold");
        assert_eq!(line_text(&result[1]), "normal");
        // Styles preserved
        assert_eq!(result[0].spans[0].style, Style::default().fg(Color::Red));
        assert_eq!(result[1].spans[0].style, Style::default().fg(Color::Blue));
    }

    #[test]
    fn test_wrap_styled_line_hard_break() {
        // Single long word that exceeds width — hard break
        let line = Line::from(vec![Span::styled(
            "abcdefghij".to_string(),
            Style::default().fg(Color::Green),
        )]);
        let result = wrap_styled_line(line, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "abcde");
        assert_eq!(line_text(&result[1]), "fghij");
        // Style preserved in both sub-lines
        assert_eq!(result[0].spans[0].style, Style::default().fg(Color::Green));
        assert_eq!(result[1].spans[0].style, Style::default().fg(Color::Green));
    }

    #[test]
    fn test_wrap_styled_line_zero_width() {
        // Zero width — return as-is
        let line = Line::from(vec![Span::styled("hello".to_string(), Style::default())]);
        let result = wrap_styled_line(line, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(line_text(&result[0]), "hello");
    }

    #[test]
    fn test_wrap_styled_line_empty() {
        // Empty line — returned as-is
        let line = Line::raw(String::new());
        let result = wrap_styled_line(line, 80);
        assert_eq!(result.len(), 1);
        assert_eq!(line_text(&result[0]), "");
    }

    #[test]
    fn test_wrap_styled_line_multiple_spans_no_break() {
        // Multiple spans, all fit in one line
        let line = Line::from(vec![
            Span::styled("hello".to_string(), Style::default().fg(Color::Red)),
            Span::styled(" world".to_string(), Style::default().fg(Color::Blue)),
        ]);
        let result = wrap_styled_line(line, 80);
        assert_eq!(result.len(), 1);
        assert_eq!(line_text(&result[0]), "hello world");
        // Both spans preserved
        assert_eq!(result[0].spans.len(), 2);
        assert_eq!(result[0].spans[0].style, Style::default().fg(Color::Red));
        assert_eq!(result[0].spans[1].style, Style::default().fg(Color::Blue));
    }

    #[test]
    fn test_wrap_styled_line_style_change_mid_word() {
        // "hel" + "lo" with different styles → treated as one word "hello"
        // because no space between them
        let line = Line::from(vec![
            Span::styled("hel".to_string(), Style::default().fg(Color::Red)),
            Span::styled("lo world".to_string(), Style::default().fg(Color::Blue)),
        ]);
        // Width=5 → "hello" (5) fits, " world" (6) → new line
        let result = wrap_styled_line(line, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "hello");
        assert_eq!(line_text(&result[1]), "world");
    }

    #[test]
    fn test_wrap_styled_line_unicode() {
        // Unicode text with wrapping
        let line = Line::from(vec![Span::styled(
            "olá mundo bonito".to_string(),
            Style::default(),
        )]);
        // "olá" (3 cols), "mundo" (5 cols) → "olá mundo" (9 cols)
        // width=9 → "olá mundo" fits, " bonito" → new line
        let result = wrap_styled_line(line, 9);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "olá mundo");
        assert_eq!(line_text(&result[1]), "bonito");
    }

    #[test]
    fn test_wrap_styled_line_cjk() {
        // CJK characters are 2 columns wide
        let line = Line::from(vec![Span::styled(
            "日本語 test value".to_string(),
            Style::default(),
        )]);
        // "日本語" = 6 cols, "test" = 4, "value" = 5
        // width=8:
        //   "日本語" (6) fits
        //   " test" (1+4=5) → 6+5=11 > 8 → new line
        //   "test" (4) fits in 8
        //   " value" (1+5=6) → 4+6=10 > 8 → new line
        //   "value" (5) fits in 8
        let result = wrap_styled_line(line, 8);
        assert_eq!(result.len(), 3);
        assert_eq!(line_text(&result[0]), "日本語");
        assert_eq!(line_text(&result[1]), "test");
        assert_eq!(line_text(&result[2]), "value");
    }

    #[test]
    fn test_wrap_styled_line_emoji() {
        // ✅ is width 2 (single codepoint emoji)
        let line = Line::from(vec![Span::styled("✅ done".to_string(), Style::default())]);
        // "✅" (2) + " done" (5) = 7 cols
        // width=5: "✅" (2) fits, " done" (5) → 2+5=7 > 5 → wrap
        let result = wrap_styled_line(line, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "✅");
        assert_eq!(line_text(&result[1]), "done");
    }

    #[test]
    fn test_wrap_styled_line_flag_emoji() {
        // 🇧🇷 is width 2 (flag emoji = 2 regional indicators).
        // With UnicodeWidthStr::width() on each span, this is correct.
        let line = Line::from(vec![Span::styled("🇧🇷 flag".to_string(), Style::default())]);
        // "🇧🇷" (2) + " flag" (5) = 7 cols
        // width=5: "🇧🇷" (2) fits, " flag" (5) → 2+5=7 > 5 → wrap
        // But "flag" (4) fits in 5
        let result = wrap_styled_line(line, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "🇧🇷");
        assert_eq!(line_text(&result[1]), "flag");
    }

    #[test]
    fn test_wrap_styled_line_zwj_emoji() {
        // 👨‍💻 is width 2 (ZWJ sequence: man + ZWJ + laptop).
        let line = Line::from(vec![Span::styled("👨‍💻 code".to_string(), Style::default())]);
        // "👨‍💻" (2) + " code" (5) = 7 cols
        // width=5: "👨‍💻" (2) fits, " code" (5) → 2+5=7 > 5 → wrap
        let result = wrap_styled_line(line, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(line_text(&result[0]), "👨‍💻");
        assert_eq!(line_text(&result[1]), "code");
    }
}
