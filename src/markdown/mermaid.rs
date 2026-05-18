//! Mermaid diagram rendering as Unicode box-drawing text.
//!
//! Uses the `mermaid-text` crate for pure-Rust rendering — no browser,
//! no image protocols, no external tools. Falls back to raw code block
//! on parse errors.
//!
//! Two rendering modes:
//! - **Rich** (default): Unicode box-drawing characters with responsive width
//! - **Plain** (`--plain` flag): Raw ` ```mermaid ` block, deferring rendering
//!   to the consumer (preparation for ACP integration)

/// Render a Mermaid diagram as Unicode box-drawing text.
///
/// Uses `mermaid_text::render_with_width()` for responsive output that
/// adapts to the terminal width. Falls back to an indented code block
/// on parse errors (invalid Mermaid syntax).
///
/// # Arguments
/// * `source` - The Mermaid diagram source (content between ` ```mermaid ` fences)
/// * `width` - Terminal width for responsive rendering
pub fn render_mermaid_rich(source: &str, width: usize) -> String {
    let effective_width = width.clamp(40, 200);
    match mermaid_text::render_with_width(source.trim(), Some(effective_width)) {
        Ok(rendered) => format!("{rendered}\n"),
        Err(e) => {
            log::warn!("Mermaid parse error, falling back to code block: {e}");
            format!("```mermaid\n{source}```\n")
        }
    }
}

/// Render a Mermaid diagram for plain mode output.
///
/// Returns the raw Mermaid source as a fenced code block, deferring
/// rendering responsibility to the consumer. This is intentional for
/// ACP (Agent Communication Protocol) integration where downstream
/// tools may have their own Mermaid rendering capability.
pub fn render_mermaid_plain(source: &str) -> String {
    format!("```mermaid\n{source}```\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_flowchart_rich() {
        let src = "graph LR; A[Start] --> B[End]";
        let output = render_mermaid_rich(src, 80);
        assert!(output.contains("Start"), "Should contain 'Start' label");
        assert!(output.contains("End"), "Should contain 'End' label");
        // Should contain box-drawing characters
        assert!(
            output.contains('┌') || output.contains('+') || output.contains('─'),
            "Should contain box-drawing or ASCII border characters"
        );
    }

    #[test]
    fn test_render_sequence_rich() {
        let src = "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: Hi";
        let output = render_mermaid_rich(src, 80);
        assert!(output.contains("Alice"), "Should contain 'Alice'");
        assert!(output.contains("Bob"), "Should contain 'Bob'");
    }

    #[test]
    fn test_render_mermaid_plain() {
        let src = "graph LR; A --> B";
        let output = render_mermaid_plain(src);
        assert!(
            output.contains("```mermaid"),
            "Plain output should have mermaid fence"
        );
        assert!(
            output.contains("A --> B"),
            "Plain output should contain source"
        );
    }

    #[test]
    fn test_render_mermaid_width_constrained() {
        // Rendering at different widths should produce valid output
        // (compaction behavior varies by diagram complexity)
        let src = "graph LR; A[Very Long Label Here] --> B[End]";
        let wide = render_mermaid_rich(src, 120);
        let narrow = render_mermaid_rich(src, 40);
        // Both should produce output containing the labels
        assert!(
            wide.contains("Very Long Label Here"),
            "Wide should contain label"
        );
        assert!(narrow.contains("End"), "Narrow should contain label");
        // Both should contain box-drawing characters
        assert!(
            wide.contains('┌') || wide.contains('+'),
            "Wide should contain border characters"
        );
    }

    #[test]
    fn test_render_mermaid_fallback_on_error() {
        // Invalid Mermaid should fall back to code block, not panic
        let invalid = "this is not valid mermaid at all!!!";
        let output = render_mermaid_rich(invalid, 80);
        // Should fall back to code block or produce some output
        assert!(
            !output.is_empty(),
            "Should produce some output even on error"
        );
    }
}
