//! Unicode-aware word-wrapping for TUI rendering
//!
//! Provides `wrap_line` and `hard_break_word` for breaking text at
//! visual column boundaries. Shared by `chat_area` (thinking blocks)
//! and `markdown` (table cell wrapping).
//!
//! # Unicode awareness
//!
//! CJK characters count as 2 visual columns; combining characters as 0.
//! Uses `unicode_width::UnicodeWidthChar` for character width lookup.

use unicode_width::UnicodeWidthChar;

/// Word-wrap a line of text to fit within `width` visual columns,
/// breaking at spaces when possible. Unicode-aware: CJK characters
/// count as 2 columns, combining characters as 0, etc.
///
/// If a single word exceeds `width`, it is hard-broken at the column limit.
/// Returns owned Strings because Unicode-aware slicing cannot return &str.
pub fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }

    let visual_len: usize = line.chars().map(|c| c.width().unwrap_or(0)).sum();
    if visual_len <= width {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0usize;

    for word in line.split_whitespace() {
        let word_width: usize = word.chars().map(|c| c.width().unwrap_or(0)).sum();

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

#[cfg(test)]
mod tests {
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
}
