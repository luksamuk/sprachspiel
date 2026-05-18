//! Mermaid diagram rendering as Unicode box-drawing text.
//!
//! Uses the `mermaid-text` crate for pure-Rust rendering — no browser,
//! no image protocols, no external tools. Falls back to raw code block
//! on parse errors or panics.
//!
//! Two rendering modes:
//! - **Rich** (default): Unicode box-drawing characters with responsive width
//! - **Plain** (`--plain` flag): Raw ` ```mermaid ` block, deferring rendering
//!   to the consumer (preparation for ACP integration)

/// Call a mermaid-text function safely, suppressing the Rust panic hook.
///
/// The `mermaid-text` crate can panic on non-ASCII labels (byte-slicing bug
/// in gantt/task parsers). `catch_unwind` catches the panic value, but Rust's
/// panic hook runs FIRST — in TUI mode, the default hook calls
/// `restore_terminal_on_panic()`, which disables raw mode and leaves the
/// alternate screen, destroying the TUI before we can fall back gracefully.
///
/// This function:
/// 1. Saves the current panic hook via `take_hook()`
/// 2. Installs a silent no-op hook
/// 3. Calls the closure inside `catch_unwind`
/// 4. Restores the original panic hook immediately
///
/// This ensures the TUI remains intact when mermaid-text panics, because
/// the default hook (which restores the terminal) is not called.
///
/// # Safety
///
/// `take_hook()` / `set_hook()` use an internal mutex, so this is thread-safe.
/// The hook is restored before any other code runs, so real panics outside this
/// function still trigger the original hook (which restores the TUI).
pub(crate) fn call_mermaid_safely<F, R>(f: F) -> Result<R, String>
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

/// Render a Mermaid diagram as Unicode box-drawing text.
///
/// Uses `mermaid_text::render_with_width()` for responsive output that
/// adapts to the terminal width. Falls back to an indented code block
/// on parse errors or panics (the crate has known UTF-8 boundary bugs
/// with non-ASCII labels in gantt/task diagrams).
///
/// # Arguments
/// * `source` - The Mermaid diagram source (content between ` ```mermaid ` fences)
/// * `width` - Terminal width for responsive rendering
pub fn render_mermaid_rich(source: &str, width: usize) -> String {
    let effective_width = width.clamp(40, 200);
    let trimmed = source.trim();

    // mermaid-text can panic on non-ASCII characters in labels (byte-slicing
    // bug in gantt/task parsers). call_mermaid_safely suppresses the panic
    // hook (which would destroy the TUI alternate screen) and catches the
    // panic, falling back to a code block.
    let result =
        call_mermaid_safely(|| mermaid_text::render_with_width(trimmed, Some(effective_width)));

    match result {
        Ok(Ok(rendered)) => format!("{rendered}\n"),
        Ok(Err(e)) => {
            log::warn!("Mermaid parse error, falling back to code block: {e}");
            format!("```mermaid\n{source}```\n")
        }
        Err(msg) => {
            log::warn!("Mermaid crate panicked, falling back to code block: {msg}");
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

    #[test]
    fn test_render_mermaid_non_ascii_labels_no_panic() {
        // mermaid-text has a byte-slicing bug in gantt/task diagrams with
        // non-ASCII characters (e.g., "Chat básico" — the 'á' spans bytes
        // 6-8, but the crate slices at byte 7). Verify we catch the panic
        // and fall back to code block instead of crashing the process.
        let src = "gantt\ntitle Test\nsection Phase 1\nChat básico        :a1, 2024-01-01, 1d";
        let output = render_mermaid_rich(src, 80);
        // Either it renders successfully, or we get a code block fallback.
        // Either way, it MUST NOT panic.
        assert!(
            !output.is_empty(),
            "Non-ASCII gantt labels should not produce empty output"
        );
    }
}
