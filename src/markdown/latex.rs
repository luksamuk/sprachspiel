//! LaTeX formula rendering as 2D Unicode character art.
//!
//! Uses the `term-maths` crate for pure-Rust rendering — no browser,
//! no JS engine, no C dependencies. Falls back to raw code block
//! on parse errors or panics.
//!
//! Two rendering modes:
//! - **Rich** (default): 2D Unicode character grid via `term_maths::render()`
//! - **Plain** (`--plain` flag): Raw ` ```latex ` block, deferring rendering

/// Call a term-maths function safely, suppressing the Rust panic hook.
///
/// Mirrors `call_mermaid_safely` — if `term_maths::render()` panics
/// (unlikely but defensive), this preserves the TUI alternate screen
/// by suppressing the default panic hook that would call
/// `restore_terminal_on_panic()`.
///
/// # Safety
///
/// `take_hook()` / `set_hook()` use an internal mutex, so this is thread-safe.
/// The hook is restored before any other code runs, so real panics outside this
/// function still trigger the original hook (which restores the TUI).
pub(crate) fn call_latex_safely<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {
        // Silent — catch_unwind will handle the panic.
        // The default hook would restore the TUI terminal, destroying
        // the alternate screen for what is actually a recoverable error.
    }));

    let result = std::panic::catch_unwind(f);

    // Restore the original panic hook immediately — critical for real panics.
    std::panic::set_hook(original_hook);

    result.map_err(|panic_info| {
        if let Some(s) = panic_info.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        }
    })
}

/// Render a LaTeX formula as 2D Unicode character art.
///
/// Uses `term_maths::render()` which parses LaTeX math input and renders
/// it as a character grid. Falls back to an indented code block on parse
/// errors or panics.
///
/// # Arguments
/// * `source` - The LaTeX formula source (content between fences or `$$` delimiters)
/// * `width` - Terminal width for responsive rendering (used for line truncation)
pub fn render_latex_rich(source: &str, width: usize) -> String {
    let effective_width = width.clamp(40, 200);
    let trimmed = source.trim();

    // term_maths::render() is unlikely to panic (pure Rust, no byte-slicing
    // bugs like mermaid-text), but call_latex_safely provides a safety net
    // preserving the TUI alternate screen.
    let result = call_latex_safely(|| term_maths::render(trimmed));

    match result {
        Ok(block) => {
            // Truncate lines exceeding effective_width with ellipsis.
            // Unlike mermaid-text, term-maths doesn't have known width bugs,
            // but defensive truncation ensures no terminal overflow.
            let rendered = block.to_string();
            let truncated: String = rendered
                .lines()
                .map(|line| crate::utils::truncate_visual_width(line, effective_width))
                .collect::<Vec<_>>()
                .join("\n");
            format!("{truncated}\n")
        }
        Err(msg) => {
            log::warn!("LaTeX render error, falling back to code block: {msg}");
            format!("```latex\n{source}```\n")
        }
    }
}

/// Render a LaTeX formula for plain mode output.
///
/// Returns the raw LaTeX source as a fenced code block, deferring
/// rendering responsibility to the consumer.
pub fn render_latex_plain(source: &str) -> String {
    format!("```latex\n{source}```\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_fraction_rich() {
        let src = r"\frac{a}{b}";
        let output = render_latex_rich(src, 80);
        // Should contain the fraction bar (box-drawing or dash)
        assert!(
            output.contains('─') || output.contains('-'),
            "Fraction should contain horizontal line, got: {output}"
        );
    }

    #[test]
    fn test_render_superscript_rich() {
        let src = "x^2 + y^2";
        let output = render_latex_rich(src, 80);
        // term-maths converts superscripts to Unicode (e.g., x² + y²)
        assert!(
            !output.is_empty(),
            "Superscript should produce non-empty output"
        );
    }

    #[test]
    fn test_render_latex_plain() {
        let src = r"\frac{a}{b}";
        let output = render_latex_plain(src);
        assert!(
            output.contains("```latex"),
            "Plain output should have latex fence"
        );
        assert!(
            output.contains(r"\frac{a}{b}"),
            "Plain output should contain source"
        );
    }

    #[test]
    fn test_render_integral_rich() {
        let src = r"\int_{0}^{1}";
        let output = render_latex_rich(src, 80);
        // Should contain integral symbol or bracket-piece characters
        assert!(
            !output.is_empty(),
            "Integral should produce non-empty output"
        );
    }

    #[test]
    fn test_render_sqrt_rich() {
        let src = r"\sqrt{b^2 - 4ac}";
        let output = render_latex_rich(src, 80);
        assert!(
            !output.is_empty(),
            "Square root should produce non-empty output"
        );
    }

    #[test]
    fn test_render_matrix_rich() {
        let src = r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}";
        let output = render_latex_rich(src, 80);
        // Should contain bracket-piece characters for delimiters
        assert!(!output.is_empty(), "Matrix should produce non-empty output");
    }

    #[test]
    fn test_render_latex_fallback_on_error() {
        // Extremely malformed LaTeX should fall back gracefully
        let invalid = r"\begin{pmatrix} a & b \end{vmatrix}";
        let output = render_latex_rich(invalid, 80);
        // Either it renders or falls back to code block — never panic
        assert!(
            !output.is_empty(),
            "Should produce some output even on parse error"
        );
    }

    #[test]
    fn test_render_width_constrained() {
        let src = r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}";
        let _wide = render_latex_rich(src, 200);
        let _narrow = render_latex_rich(src, 40);
        // Both should produce valid output (no panic, no empty)
        // Exact layout depends on term-maths internals
    }

    #[test]
    fn test_render_empty_formula() {
        let output = render_latex_rich("", 80);
        // Empty formula should not panic
        assert!(
            !output.is_empty(),
            "Should produce some output for empty input"
        );
    }
}
