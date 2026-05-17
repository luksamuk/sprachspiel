//! Ratatui style mappings for the chat TUI
//!
//! This module provides ratatui `Style` and `Color` mappings that
//! correspond to the ANSI color codes used in `view/mod.rs::colors`.
//! The ANSI module is kept for non-chat subcommands (query, translate,
//! OCR, summarize) which still use termimad+println.

use ratatui::style::{Color, Modifier, Style};

// ── Ratatui Style constants (for TUI rendering) ────────────────────

/// Cyan text color
pub const CYAN: Color = Color::Cyan;

/// Yellow text color
pub const YELLOW: Color = Color::Yellow;

/// Green text color
pub const GREEN: Color = Color::Green;

/// Red text color
pub const RED: Color = Color::Red;

/// Dim (faint) text style
pub const DIM: Modifier = Modifier::DIM;

/// Bold text style
pub const BOLD: Modifier = Modifier::BOLD;

/// Bold cyan style
pub fn bold_cyan() -> Style {
    Style::default().fg(CYAN).add_modifier(BOLD)
}

/// Bold yellow style
#[allow(dead_code)] // Will be used for assistant streaming indicators
pub fn bold_yellow() -> Style {
    Style::default().fg(YELLOW).add_modifier(BOLD)
}

/// Dim style
pub fn dim() -> Style {
    Style::default().add_modifier(DIM)
}

/// Dim cyan style (for thinking blocks)
pub fn dim_cyan() -> Style {
    Style::default().fg(CYAN).add_modifier(DIM)
}

/// Default style (reset)
#[allow(dead_code)] // PR3: Will be used for TUI style reset
pub fn reset() -> Style {
    Style::default()
}

/// System message style (dim)
pub fn system_style() -> Style {
    Style::default().add_modifier(DIM)
}

/// Error style (red, bold)
pub fn error_style() -> Style {
    Style::default().fg(RED).add_modifier(BOLD)
}

/// Warning style (yellow)
#[allow(dead_code)] // PR3: Will be used for TUI warning messages
pub fn warning_style() -> Style {
    Style::default().fg(YELLOW)
}

/// Success style (green)
#[allow(dead_code)] // PR3: Will be used for TUI success messages
pub fn success_style() -> Style {
    Style::default().fg(GREEN)
}

/// Style for the thinking block left border (│)
///
/// Same as `dim_cyan()` — the border and header share the same
/// accent color for visual coherence.
pub fn thinking_border_style() -> Style {
    dim_cyan()
}

/// Style for the thinking block header (🧠 Thinking)
///
/// Same as `dim_cyan()` — the header introduces the block and
/// should match the border accent.
pub fn thinking_header_style() -> Style {
    dim_cyan()
}

/// Progress bar color based on percentage
pub fn progress_color(percent: u8) -> Color {
    if percent < 50 {
        Color::Green
    } else if percent < 75 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Style for the progress bar based on percentage
#[allow(dead_code)] // PR3: Will be used for TUI progress bar styling
pub fn progress_style(percent: u8) -> Style {
    Style::default().fg(progress_color(percent))
}

/// Style for the input prompt (">>> ")
pub fn prompt_style() -> Style {
    bold_cyan()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_color_thresholds() {
        assert_eq!(progress_color(0), Color::Green);
        assert_eq!(progress_color(49), Color::Green);
        assert_eq!(progress_color(50), Color::Yellow);
        assert_eq!(progress_color(74), Color::Yellow);
        assert_eq!(progress_color(75), Color::Red);
        assert_eq!(progress_color(100), Color::Red);
    }
}
