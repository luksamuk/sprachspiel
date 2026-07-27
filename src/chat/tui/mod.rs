//! TUI module — Terminal setup and ratatui infrastructure
//!
//! This module provides the ratatui-based TUI for the chat REPL,
//! replacing the previous println+ANSI approach with responsive rendering.
//!
//! # Architecture
//!
//! ```text
//! app.rs (event loop + state)
//!     ↓ uses
//! tui/mod.rs (terminal enter/exit/restore via ratatui::init/restore)
//! tui/components/ (chat_area, status_bar, input_line)
//! tui/markdown.rs (tui-markdown rendering with themes)
//! tui/styles.rs (ANSI → ratatui color mapping)
//! ```
//!
//! # Terminal Lifecycle
//!
//! ratatui 0.30 provides `ratatui::init()` and `ratatui::restore()` which
//! handle raw mode, alternate screen, and panic hooks automatically.
//! We use `DefaultTerminal` (an alias for `Terminal<CrosstermBackend<Stdout>>`)
//! for the simplest possible setup.

pub mod banner;
pub mod components;
pub mod diff_render;
pub mod live_turn;
pub mod markdown;
pub mod styles;
pub mod wrap;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Stdout};

/// The terminal type used by the TUI
pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Result type for TUI operations
pub type TuiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize the terminal for TUI rendering
///
/// Uses ratatui's `init()` which:
/// - Enables raw mode
/// - Enters alternate screen
/// - Enables mouse capture
/// - Installs a panic hook that restores the terminal
///
/// # Errors
///
/// Returns an error if terminal setup fails (e.g., not a TTY).
pub fn enter_tui() -> TuiResult<TuiTerminal> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    )?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

/// Restore the terminal to its original state
///
/// Disables raw mode, leaves alternate screen, and restores
/// the terminal to cooked mode. Should be called on exit.
///
/// # Errors
///
/// Returns an error if terminal restoration fails.
pub fn exit_tui(terminal: &mut TuiTerminal) -> TuiResult<()> {
    terminal.show_cursor()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}

/// Emergency terminal restore on panic
///
/// Call this from a panic hook to ensure the terminal is usable
/// after a crash. Without this, a panic during TUI mode leaves the
/// terminal in raw mode with alternate screen active.
///
/// Also restores stderr logging so panic messages are visible.
pub fn restore_terminal_on_panic() {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    // Restore stderr logging so panic messages are visible after TUI crash
    crate::logging::set_tui_mode(false);
}
