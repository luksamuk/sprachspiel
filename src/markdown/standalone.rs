//! Standalone monochrome markdown renderer for stdout output.
//!
//! Renders markdown to plain strings for non-chat subcommands (query,
//! translate, summarize, OCR, vision). Uses box-drawing characters
//! for tables (same algorithm as the TUI renderer) but without any
//! ratatui styles or colors.
//!
//! Two modes:
//! - **Rich** (default): ANSI bold for headings, box-drawing tables,
//!   code blocks with indentation
//! - **Plain** (`--plain` flag): No ANSI codes, pipe-delimited tables,
//!   no heading formatting

#![expect(clippy::print_stdout)] // Standalone markdown renderer output

use super::table::{
    ContentSegment, extract_content_segments, render_table_box_string, render_table_plain,
};

/// Default terminal width when detection fails.
const DEFAULT_WIDTH: usize = 80;

/// Get the current terminal width, falling back to DEFAULT_WIDTH.
fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(DEFAULT_WIDTH)
}

/// Render markdown content to a string with monochrome formatting.
///
/// Rich mode: ANSI bold for headings, box-drawing tables, Mermaid diagrams.
/// Tables use the same responsive column-width algorithm as the TUI renderer.
///
/// Use this for completed messages where Mermaid diagrams should be rendered.
/// During streaming, use [`render_markdown_streaming`] instead.
pub fn render_markdown(content: &str, width: usize) -> String {
    render_segments(content, width, true)
}

/// Render markdown during streaming — Mermaid blocks shown as code blocks.
///
/// During LLM streaming, Mermaid blocks may be incomplete (missing closing
/// fence) or have source that panics the `mermaid-text` crate (non-ASCII
/// labels). Rendering them as diagrams on every frame wastes CPU and produces
/// visual noise. This function treats Mermaid blocks as regular code blocks.
///
/// Currently unused in standalone mode (terminal path is single-shot, not
/// streaming), but provided for API symmetry with the TUI rendering path.
#[allow(dead_code)] // API symmetry with TUI; will be used if terminal gets streaming
pub fn render_markdown_streaming(content: &str, width: usize) -> String {
    render_segments(content, width, false)
}

/// Internal implementation shared between streaming and final rendering.
///
/// When `render_special` is true, Mermaid blocks and LaTeX blocks are rendered
/// as Unicode art. When false, they are treated as regular code blocks.
fn render_segments(content: &str, width: usize, render_special: bool) -> String {
    #[cfg(not(any(feature = "mermaid", feature = "latex")))]
    let _ = render_special; // suppress unused when no rendering features
    let mut output = String::new();
    let segments = extract_content_segments(content);
    let mut first_segment = true;

    for segment in segments {
        // Add vertical padding (blank line) between segments so that
        // consecutive special blocks (LaTeX, Mermaid, Table) are visually
        // separated from preceding content instead of glued together.
        if !first_segment {
            output.push('\n');
        }
        first_segment = false;

        match segment {
            ContentSegment::Markdown(text) => {
                output.push_str(&render_markdown_inline(&text, width));
            }
            ContentSegment::Table(table_text) => {
                let lines = render_table_box_string(&table_text, width);
                for line in lines {
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(mermaid_source) => {
                if render_special {
                    let rendered = super::mermaid::render_mermaid_rich(&mermaid_source, width);
                    output.push_str(&rendered);
                } else {
                    // Streaming mode: treat as regular code block
                    output.push_str("```mermaid\n");
                    output.push_str(&mermaid_source);
                    output.push_str("```\n");
                }
            }
            #[cfg(feature = "latex")]
            ContentSegment::Latex(latex_source) => {
                if render_special {
                    let rendered = super::latex::render_latex_rich(&latex_source, width);
                    output.push_str(&rendered);
                } else {
                    // Streaming mode: treat as regular code block
                    output.push_str("```latex\n");
                    output.push_str(&latex_source);
                    output.push_str("```\n");
                }
            }
        }
    }

    output
}

/// Render markdown content in plain text mode (no ANSI codes, pipe-delimited tables).
///
/// Mermaid blocks are emitted as raw ` ```mermaid ` code blocks, deferring
/// rendering responsibility to the consumer (ACP integration).
pub fn render_markdown_plain(content: &str, width: usize) -> String {
    let mut output = String::new();
    let segments = extract_content_segments(content);
    let mut first_segment = true;

    for segment in segments {
        // Add vertical padding (blank line) between segments so that
        // consecutive special blocks (LaTeX, Mermaid, Table) are visually
        // separated from preceding content instead of glued together.
        if !first_segment {
            output.push('\n');
        }
        first_segment = false;

        match segment {
            ContentSegment::Markdown(text) => {
                output.push_str(&render_markdown_inline_plain(&text, width));
            }
            ContentSegment::Table(table_text) => {
                let lines = render_table_plain(&table_text, width);
                for line in lines {
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            #[cfg(feature = "mermaid")]
            ContentSegment::Mermaid(mermaid_source) => {
                let rendered = super::mermaid::render_mermaid_plain(&mermaid_source);
                output.push_str(&rendered);
            }
            #[cfg(feature = "latex")]
            ContentSegment::Latex(latex_source) => {
                let rendered = super::latex::render_latex_plain(&latex_source);
                output.push_str(&rendered);
            }
        }
    }

    output
}

/// Print markdown to stdout with monochrome formatting.
///
/// Detects terminal width automatically. Tables use responsive
/// column widths with box-drawing borders. Headings get ANSI bold.
pub fn print_markdown(content: &str) {
    let width = terminal_width();
    let rendered = render_markdown(content, width);
    print!("{}", rendered);
}

/// Print markdown to stdout in plain text mode (no formatting).
///
/// For `--plain` flag and ACP integration. Tables are pipe-delimited.
/// No ANSI codes at all.
pub fn print_markdown_plain(content: &str) {
    let width = terminal_width();
    let rendered = render_markdown_plain(content, width);
    print!("{}", rendered);
}

/// Render inline markdown (non-table content) with monochrome formatting.
///
/// Handles:
/// - Headings: `# H1` → `# H1` with ANSI bold, `## H2` → `## H2` with ANSI underline
/// - Code blocks: indented with 4 spaces, no color
/// - Inline code: wrapped with backticks (no formatting)
/// - Bold: `**text**` → ANSI bold
/// - Italic: `_text_` or `*text*` → text (kept as-is in mono)
/// - Blockquotes: `> text` → `│ text` with dim
/// - Lists: kept as-is
/// - Links: `[text](url)` → `text (url)`
/// - Paragraph breaks: double newline
fn render_markdown_inline(content: &str, width: usize) -> String {
    let mut output = String::new();
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();
    let mut code_block_lang = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Code block toggle
        if trimmed.starts_with("```") {
            if in_code_block {
                // End code block
                in_code_block = false;
                let is_diff = code_block_lang == "diff";
                for code_line in &code_block_lines {
                    if is_diff {
                        // Render diff lines with ANSI colors
                        output.push_str(&crate::tools::diff_render::render_diff_ansi(code_line));
                    } else {
                        output.push_str("    ");
                        output.push_str(code_line);
                        output.push('\n');
                    }
                }
                code_block_lines.clear();
                code_block_lang.clear();
                output.push('\n');
            } else {
                // Start code block
                in_code_block = true;
                // Print the language tag if present
                let lang = trimmed.trim_start_matches('`').trim();
                code_block_lang = lang.to_string();
                if !lang.is_empty() {
                    output.push_str(&format!("\x1B[2m{}\x1B[0m\n", lang));
                }
            }
            continue;
        }

        if in_code_block {
            code_block_lines.push(line.to_string());
            continue;
        }

        // Empty line → paragraph break
        if trimmed.is_empty() {
            output.push('\n');
            continue;
        }

        // Headings
        if trimmed.starts_with("# ") {
            output.push_str(&format!("\x1B[1m{}\x1B[0m\n", line));
            continue;
        }
        if trimmed.starts_with("## ") {
            output.push_str(&format!("\x1B[4m{}\x1B[0m\n", line));
            continue;
        }
        if trimmed.starts_with("### ")
            || trimmed.starts_with("#### ")
            || trimmed.starts_with("##### ")
        {
            output.push_str(&format!("\x1B[1m{}\x1B[0m\n", line));
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            output.push_str(&"─".repeat(width.min(40)));
            output.push('\n');
            continue;
        }

        // Blockquote
        if let Some(quote_text) = trimmed.strip_prefix("> ") {
            output.push_str(&format!("\x1B[2m│\x1B[0m {}\n", quote_text));
            continue;
        }
        if trimmed == ">" {
            output.push_str("\x1B[2m│\x1B[0m\n");
            continue;
        }

        // Regular line: wrap to width if needed, then apply inline formatting
        let formatted = format_inline_styles(trimmed);
        output.push_str(&formatted);
        output.push('\n');
    }

    output
}

/// Render inline markdown in plain mode (no ANSI codes).
fn render_markdown_inline_plain(content: &str, _width: usize) -> String {
    let mut output = String::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            output.push_str("    ");
            output.push_str(trimmed);
            output.push('\n');
            continue;
        }

        if in_code_block {
            output.push_str("    ");
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if trimmed.is_empty() {
            output.push('\n');
            continue;
        }

        // Strip inline formatting but keep structure
        let plain = strip_inline_styles(trimmed);
        output.push_str(&plain);
        output.push('\n');
    }

    output
}

/// Apply inline formatting (bold, italic, code) with ANSI codes.
fn format_inline_styles(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 16);

    let mut chars = text.chars().peekable();
    let mut in_bold = false;
    let mut in_code = false;

    while let Some(ch) = chars.next() {
        // Inline code: `text`
        if ch == '`' && !in_bold {
            if in_code {
                result.push_str("\x1B[0m");
                in_code = false;
            } else {
                result.push_str("\x1B[7m"); // reverse video
                in_code = true;
            }
            continue;
        }

        // Bold: **text**
        if ch == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            if in_bold {
                result.push_str("\x1B[0m");
                in_bold = false;
            } else {
                result.push_str("\x1B[1m");
                in_bold = true;
            }
            continue;
        }

        result.push(ch);
    }

    // Close any open formatting
    if in_bold || in_code {
        result.push_str("\x1B[0m");
    }

    result
}

/// Strip inline markdown formatting for plain mode.
fn strip_inline_styles(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_code = false;

    while let Some(ch) = chars.next() {
        if ch == '`' {
            in_code = !in_code;
            continue;
        }

        // Bold: **text** → text
        if ch == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            continue;
        }

        // Italic: *text* → text (only outside code)
        if ch == '*' && !in_code {
            continue;
        }

        // Links: [text](url) → text (url)
        if ch == '[' && !in_code {
            // Collect link text and URL
            let mut link_text = String::new();
            let mut found_closing_bracket = false;
            while let Some(&next) = chars.peek() {
                if next == ']' {
                    chars.next();
                    found_closing_bracket = true;
                    break;
                }
                let Some(c) = chars.next() else { break };
                link_text.push(c);
            }
            if found_closing_bracket && chars.peek() == Some(&'(') {
                chars.next(); // consume (
                let mut url = String::new();
                while let Some(&next) = chars.peek() {
                    if next == ')' {
                        chars.next();
                        break;
                    }
                    let Some(c) = chars.next() else { break };
                    url.push(c);
                }
                result.push_str(&link_text);
                if !url.is_empty() {
                    result.push_str(" (");
                    result.push_str(&url);
                    result.push(')');
                }
            } else {
                result.push('[');
                result.push_str(&link_text);
                if found_closing_bracket {
                    result.push(']');
                }
            }
            continue;
        }

        result.push(ch);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_heading() {
        let input = "# Hello World\n\nSome text\n";
        let output = render_markdown(input, 80);
        assert!(output.contains("\x1B[1m# Hello World\x1B[0m"));
        assert!(output.contains("Some text"));
    }

    #[test]
    fn test_render_markdown_plain_heading() {
        let input = "# Hello World\n\nSome text\n";
        let output = render_markdown_plain(input, 80);
        assert!(output.contains("# Hello World\n"));
        assert!(output.contains("Some text"));
        assert!(!output.contains("\x1B[")); // no ANSI codes
    }

    #[test]
    fn test_render_markdown_table() {
        let input = "| Name | Value |\n|------|-------|\n| foo  | bar   |";
        let output = render_markdown(input, 80);
        assert!(output.contains("┌")); // top border
        assert!(output.contains("│")); // cell border
        assert!(output.contains("└")); // bottom border
    }

    #[test]
    fn test_render_table_plain() {
        let input = "| Name | Value |\n|------|-------|\n| foo  | bar   |";
        let output = render_markdown_plain(input, 80);
        assert!(output.contains("| Name |"));
        assert!(!output.contains("┌")); // no box-drawing
        assert!(!output.contains("\x1B[")); // no ANSI codes
    }

    #[test]
    fn test_format_inline_styles_bold() {
        let result = format_inline_styles("hello **world** end");
        assert!(result.contains("\x1B[1mworld\x1B[0m"));
    }

    #[test]
    fn test_format_inline_styles_code() {
        let result = format_inline_styles("use `cargo build` to compile");
        assert!(result.contains("\x1B[7mcargo build\x1B[0m"));
    }

    #[test]
    fn test_strip_inline_styles() {
        let result = strip_inline_styles("hello **world** end");
        assert_eq!(result, "hello world end");
    }

    #[test]
    fn test_strip_inline_styles_link() {
        let result = strip_inline_styles("click [here](https://example.com)");
        assert_eq!(result, "click here (https://example.com)");
    }

    #[test]
    fn test_terminal_width_fallback() {
        // Should not panic even if terminal detection fails
        let width = terminal_width();
        assert!(width > 0);
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_render_markdown_mermaid_rich() {
        let input = "Before\n\n```mermaid\ngraph LR; A --> B\n```\n\nAfter";
        let output = render_markdown(input, 80);
        // Rich mode should render the Mermaid block (contain box-drawing or label)
        assert!(
            output.contains("A") && output.contains("B"),
            "Rich render should contain diagram labels"
        );
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_render_markdown_mermaid_plain() {
        let input = "Before\n\n```mermaid\ngraph LR; A --> B\n```\n\nAfter";
        let output = render_markdown_plain(input, 80);
        // Plain mode should emit raw mermaid code block
        assert!(
            output.contains("```mermaid"),
            "Plain render should contain mermaid fence"
        );
        assert!(
            output.contains("A --> B"),
            "Plain render should contain original source"
        );
    }

    #[cfg(feature = "mermaid")]
    #[test]
    fn test_streaming_mermaid_as_code_block() {
        // During streaming, Mermaid blocks should be shown as code blocks, not diagrams
        let input = "Before\n\n```mermaid\ngraph LR; A --> B\n```\n\nAfter";
        let output = render_markdown_streaming(input, 80);
        // Streaming mode should emit mermaid source as a code block
        assert!(
            output.contains("```mermaid"),
            "Streaming should contain mermaid fence"
        );
        assert!(
            output.contains("A --> B"),
            "Streaming should contain original source"
        );
        // It should NOT contain box-drawing diagram output (no rendered Mermaid)
        // The output should look like regular markdown with a code block
    }
}
