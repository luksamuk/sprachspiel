//! Tool execution display and logging.
//!
//! Tool calls are displayed as UI output controlled by a dedicated
//! `show_tool_calls` flag, independent of the `log` crate's verbosity levels.
//!
//! # Display Logic
//!
//! Tool call **display** is controlled by [`SHOW_TOOL_CALLS`] (a global flag),
//! **not** by `log::log_enabled!`. This decouples the UI from the logging system:
//!
//! | Level   | Tool Calls (display)    | Tool Results        |
//! |---------|-------------------------|----------------------|
//! | Quiet   | Hidden                  | Hidden               |
//! | Normal  | Compact: 🔧 name(k=v)  | Hidden               |
//! | Verbose | Compact + detail lines | Truncated (~100 chr) |
//! | Trace   | Compact + detail lines | Full output (500 chr) |
//!
//! The compact `🔧 name(k=v)` format always shows in Normal mode regardless of
//! `log::LevelFilter`. In Verbose/Trace mode, additional detail lines are shown
//! (key: value, one per line). Tool results are hidden in Normal mode.
//!
//! # TUI Mode
//!
//! When running in TUI mode (ratatui alternate screen), tool call output must
//! go through the ChatView layer rather than `eprintln!`, which would corrupt
//! the alternate screen. A global callback can be set via [`set_tui_callback`]
//! to route tool call display through the TUI view layer.
//!
//! # Configuration
//!
//! The `show_tool_calls` setting in `[display]` section of `config.toml` controls
//! whether the compact format is shown. Default: `true`.
//! In Quiet mode (`-q`), tool calls are always hidden regardless of this setting.

#![expect(clippy::print_stderr)] // Debug diagnostics output
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::spinner::suspend_for_print;

/// Type alias for the TUI callback that routes tool call output into the chat view.
///
/// Avoids clippy `type_complexity` on the raw `Arc<dyn Fn(&str) + Sync + Send>` type.
type TuiCallback = std::sync::Arc<dyn Fn(&str) + Sync + Send>;

/// ANSI style: DIM (faint) + light gray text — same as thinking blocks.
///
/// Shared across all tool indicator displays (tool calls, skill loading,
/// document import, notes, facts, feedback, command execution).
pub const TOOL_DIM: &str = "\x1B[2m\x1B[37m";
/// ANSI reset — shared across all indicator displays.
pub const RESET: &str = "\x1B[0m";

/// Maximum display width for tool call lines (80-column terminal minus margin)
const MAX_LINE_WIDTH: usize = 74;

/// Global flag controlling whether tool calls are displayed in chat.
///
/// **Independent of the `log` crate's level filter.** Set from
/// `Settings::display.show_tool_calls` at startup and via `/debug` toggle.
/// In Quiet mode, this flag is overridden — tool calls are never shown.
static SHOW_TOOL_CALLS: AtomicBool = AtomicBool::new(true);

/// Global flag controlling plain mode (no ANSI codes in tool indicators).
///
/// Set from query/translate/summarize subcommands when `--plain` is active.
/// In plain mode, tool call indicators omit ANSI styling for pipe-safe output.
static PLAIN_MODE: AtomicBool = AtomicBool::new(false);

/// Global callback for TUI mode tool call display.
///
/// When set, `display_tool_call` invokes this callback with the formatted
/// line instead of printing to stderr. This prevents ANSI escape sequences
/// from corrupting the ratatui alternate screen.
///
/// The callback is set when the TUI starts and cleared on exit.
/// In terminal (non-TUI) mode, this remains `None` and `eprintln!` is used.
static TUI_CALLBACK: Mutex<Option<TuiCallback>> = Mutex::new(None);

/// Set the TUI callback for tool call display.
///
/// When set, tool call lines are sent through this callback instead of
/// `eprintln!`. This is used by the ratatui TUI to route tool calls into
/// the chat area as system messages.
///
/// Call with `None` to clear the callback (e.g., when exiting TUI mode).
pub fn set_tui_callback(callback: Option<TuiCallback>) {
    if let Ok(mut guard) = TUI_CALLBACK.lock() {
        *guard = callback;
    }
}

/// Set the `show_tool_calls` flag from configuration.
///
/// Called once at startup after loading `config.toml`.
pub fn set_show_tool_calls(enabled: bool) {
    SHOW_TOOL_CALLS.store(enabled, Ordering::Relaxed);
}

/// Set plain mode for tool indicators (no ANSI codes).
///
/// Called from subcommands with `--plain` flag to ensure pipe-safe output.
pub fn set_plain_mode(enabled: bool) {
    PLAIN_MODE.store(enabled, Ordering::Relaxed);
}

/// Check if plain mode is active (no ANSI codes in tool indicators).
pub fn is_plain_mode() -> bool {
    PLAIN_MODE.load(Ordering::Relaxed)
}

/// Print a tool visual indicator through the TUI callback (TUI mode) or
/// `suspend_for_print` (terminal mode).
///
/// Tool indicators like ⚡, 📝, 💾, etc. must use this function instead of
/// `suspend_for_print(|| { eprintln!(...) })` directly. In TUI mode
/// (ratatui alternate screen), raw `eprintln!` bypasses the TUI callback
/// and corrupts the status bar. This function routes through [`TUI_CALLBACK`]
/// when available, falling back to `suspend_for_print` in terminal mode.
///
/// The line is printed with [`TOOL_DIM`] + [`RESET`] styling in terminal mode.
/// In TUI mode, styling is handled by the chat view layer.
/// In plain mode, no ANSI styling is applied for pipe-safe output.
pub fn tui_aware_print(line: &str) {
    // Route through TUI callback if set (TUI mode)
    if let Ok(guard) = TUI_CALLBACK.lock()
        && let Some(callback) = guard.as_ref()
    {
        callback(line);
        return;
    }
    // Terminal mode: print to stderr with ANSI styling (unless plain mode)
    if PLAIN_MODE.load(Ordering::Relaxed) {
        suspend_for_print(|| {
            eprintln!("{line}");
        });
    } else {
        suspend_for_print(|| {
            eprintln!("{TOOL_DIM}{line}{RESET}");
        });
    }
}

/// Query whether tool calls should be displayed.
///
/// Returns `false` in Quiet mode regardless of the flag value.
fn should_show_tool_calls() -> bool {
    // Quiet mode: never show tool calls
    if log::max_level() == log::LevelFilter::Error {
        return false;
    }
    SHOW_TOOL_CALLS.load(Ordering::Relaxed)
}

/// Toggle debug/logging verbosity between Normal and Trace.
/// Used by the `/debug` command in chat mode.
/// Returns the new verbosity level.
pub fn toggle_debug() -> crate::logging::Verbosity {
    crate::logging::toggle_verbosity()
}

/// Display a tool call in compact single-line format.
///
/// Shows `🔧 name(k=v, k=v)` in DIM gray, fitting within 80 columns.
/// Empty-string values are omitted from the compact display to reduce
/// visual noise (e.g., `🔧 run_command(command_line=ls)` instead of
/// `🔧 run_command(command_line=ls, head=, tail=, timeout_seconds=)`).
///
/// This is **always** called — the decision to show/hide is made by
/// [`should_show_tool_calls()`] which checks both Quiet mode and the
/// `show_tool_calls` configuration flag.
///
/// In TUI mode, the formatted line is sent through the TUI callback
/// instead of `eprintln!`, preventing alternate screen corruption.
fn display_tool_call(tool_name: &str, args: &[(String, String)]) {
    if !should_show_tool_calls() {
        return;
    }

    // Build key=value pairs with truncated values, filtering out empty values
    let max_arg_value = 30;
    let args_str: Vec<String> = args
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| {
            let v_display = crate::utils::truncate_chars(v, max_arg_value);
            format!("{}={}", k, v_display)
        })
        .collect();
    let args_line = args_str.join(", ");

    // Format: 🔧 name(args) — ensure total fits in MAX_LINE_WIDTH
    let prefix = format!("🔧 {}(", tool_name);
    let suffix = ")";
    let prefix_len = prefix.chars().count();
    let suffix_len = suffix.chars().count();
    let content_budget = MAX_LINE_WIDTH.saturating_sub(prefix_len + suffix_len);
    let display_args = crate::utils::truncate_chars(&args_line, content_budget);

    let line = format!("{prefix}{display_args}{suffix}");

    // Route through TUI callback if set, otherwise print to stderr
    if let Ok(guard) = TUI_CALLBACK.lock()
        && let Some(callback) = guard.as_ref()
    {
        callback(&line);
        return;
    }

    // Terminal mode: print to stderr with ANSI styling (unless plain mode)
    if PLAIN_MODE.load(Ordering::Relaxed) {
        suspend_for_print(|| {
            eprintln!("{line}");
        });
    } else {
        suspend_for_print(|| {
            eprintln!("{TOOL_DIM}{line}{RESET}");
        });
    }
}

/// Log a tool call with its arguments.
///
/// - **Normal mode**: compact single-line format `🔧 name(k=v)` in DIM gray
/// - **Verbose/Trace mode**: compact line + detailed `key: value` lines
/// - **Quiet mode**: hidden
///
/// The compact format is controlled by [`SHOW_TOOL_CALLS`], not by `log_enabled!`.
/// This ensures tool calls are visible in Normal mode (where `LevelFilter::Warn`
/// would otherwise block `Info`-level checks).
pub fn log_tool_call(tool_name: &str, args: &[(String, String)]) {
    // Always display compact format — UI display, not logging
    display_tool_call(tool_name, args);

    // In Verbose/Trace mode, log additional detail lines to the log file.
    // These detail lines are DIAGNOSTIC, not UI — they should NOT appear in
    // the TUI chat area. When TUI mode is active, the global max level is
    // boosted to Debug (so the log file captures diagnostics), which would
    // cause these lines to leak into the TUI callback and clutter the chat
    // display. The compact format from display_tool_call() is sufficient for
    // the user; verbose details belong in the log file.
    if log::log_enabled!(log::Level::Debug) {
        for (key, value) in args {
            if value.is_empty() {
                continue;
            }
            let display_value = crate::utils::truncate_chars(value, 77);
            let detail_line = format!("  {key}: {display_value}");
            log::debug!("{}", detail_line);
        }
    }
}

/// Log tool result.
///
/// - **Normal mode**: hidden (tool calls are enough for the user)
/// - **Verbose mode (-v)**: truncated preview (~100 chars) in DIM gray
/// - **Trace mode (-vv)**: full result (up to 500 chars) in DIM gray
/// - **Quiet mode**: hidden
///
/// In TUI mode, results are routed through the TUI callback.
pub fn log_tool_result(tool_name: &str, result: &str) {
    let has_tui_callback = TUI_CALLBACK.lock().ok().is_some_and(|g| g.is_some());

    // Trace mode: full result (up to 500 chars)
    if log::max_level() == log::LevelFilter::Trace {
        let display_result = format_result(result, 500);
        let line = if has_tui_callback {
            // In TUI mode, indent with two spaces + └ so the result line
            // sits neatly under the 🔧 tool call indicator (emoji is 2-col).
            format!("  ↳ 📤 {tool_name} result: {display_result}")
        } else {
            format!("📤 {tool_name} result: {display_result}")
        };

        if has_tui_callback {
            if let Ok(guard) = TUI_CALLBACK.lock()
                && let Some(callback) = guard.as_ref()
            {
                callback(&line);
            }
        } else if PLAIN_MODE.load(Ordering::Relaxed) {
            suspend_for_print(|| {
                eprintln!("{line}");
            });
        } else {
            suspend_for_print(|| {
                eprintln!("{TOOL_DIM}{line}{RESET}");
            });
        }
    } else if log::log_enabled!(log::Level::Debug) {
        // Verbose mode: truncated preview (~100 chars)
        let preview = crate::utils::truncate_chars(result, 100);
        let line = if has_tui_callback {
            // In TUI mode, indent with two spaces + └ so the result line
            // sits neatly under the 🔧 tool call indicator (emoji is 2-col).
            format!("  ↳ ✓ Result: {}", preview.replace('\n', " "))
        } else {
            format!("✓ Result: {}", preview.replace('\n', " "))
        };

        if has_tui_callback {
            if let Ok(guard) = TUI_CALLBACK.lock()
                && let Some(callback) = guard.as_ref()
            {
                callback(&line);
            }
        } else if PLAIN_MODE.load(Ordering::Relaxed) {
            suspend_for_print(|| {
                eprintln!("{line}");
            });
        } else {
            suspend_for_print(|| {
                eprintln!("{TOOL_DIM}{line}{RESET}");
            });
        }
    }
    // Normal + Quiet mode: tool results are hidden
}

/// Format a tool result string, truncating to `max_chars` if needed.
fn format_result(result: &str, max_chars: usize) -> String {
    if result.chars().count() > max_chars {
        let truncated: String = result.chars().take(max_chars - 3).collect();
        let remaining = result.chars().count() - max_chars + 3;
        format!("{}...[+{} chars]", truncated, remaining)
    } else {
        result.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_format_fits_80_columns() {
        // Verify that a typical tool call line fits within 80 columns
        let args = vec![
            (
                "path".to_string(),
                "/very/long/path/to/some/file/that/should/be/truncated.txt".to_string(),
            ),
            (
                "query".to_string(),
                "a very long query string that would exceed the limit when combined".to_string(),
            ),
        ];

        let max_arg_value = 30;
        let args_str: Vec<String> = args
            .iter()
            .map(|(k, v)| {
                let v_display = crate::utils::truncate_chars(v, max_arg_value);
                format!("{}={}", k, v_display)
            })
            .collect();
        let args_line = args_str.join(", ");

        let prefix = format!("🔧 {}(", "read_file");
        let suffix = ")";
        let prefix_len = prefix.chars().count();
        let suffix_len = suffix.chars().count();
        let content_budget = MAX_LINE_WIDTH.saturating_sub(prefix_len + suffix_len);
        let display_args = crate::utils::truncate_chars(&args_line, content_budget);

        let line = format!("{prefix}{display_args}{suffix}");
        assert!(
            line.chars().count() <= 78,
            "Line too long: {} chars: {}",
            line.chars().count(),
            line
        );
    }

    #[test]
    fn test_should_show_tool_calls_respects_quiet() {
        // We can't easily test log level in unit tests without initializing the logger,
        // but we can test the flag logic directly.
        // Default: true
        assert!(SHOW_TOOL_CALLS.load(Ordering::Relaxed));

        // Disable
        set_show_tool_calls(false);
        assert!(!SHOW_TOOL_CALLS.load(Ordering::Relaxed));

        // Re-enable
        set_show_tool_calls(true);
        assert!(SHOW_TOOL_CALLS.load(Ordering::Relaxed));
    }

    #[test]
    fn test_format_result_truncation() {
        let short = "hello";
        assert_eq!(format_result(short, 10), "hello");

        // Long string: 100 chars, budget 20
        // format_result takes first 17 chars + "...[+83 chars]" = 17 + 10 = 27 chars
        let long = "a".repeat(100);
        let result = format_result(&long, 20);
        assert!(result.ends_with(" chars]"));
        // Truncated content + suffix should not exceed ~2x budget (reasonable bound)
        assert!(
            result.chars().count() <= 40,
            "Result should be reasonable: got '{}' ({} chars)",
            result,
            result.chars().count()
        );
    }

    #[test]
    fn test_tui_aware_print_without_callback_prints_to_stderr() {
        // Without TUI callback set, tui_aware_print falls back to
        // suspend_for_print. We verify it doesn't panic.
        // Note: we can't reliably assert TUI_CALLBACK state in parallel tests,
        // so we just verify the function executes without panicking.
        tui_aware_print("test indicator line");
    }

    #[test]
    fn test_tui_aware_print_routes_through_callback() {
        use std::sync::Arc;

        // Clear any previous callback (in case of parallel test contamination)
        set_tui_callback(None);

        // Set up a callback that captures lines
        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = Arc::clone(&captured);
        let callback = Arc::new(move |line: &str| {
            if let Ok(mut guard) = captured_clone.lock() {
                guard.push(line.to_string());
            }
        }) as std::sync::Arc<dyn Fn(&str) + Sync + Send>;

        set_tui_callback(Some(callback));

        // tui_aware_print should route through the callback, not stderr
        tui_aware_print("⚡ test command");
        tui_aware_print("📝 note #42");
        tui_aware_print("💾 fact #5");

        let guard = captured.lock().unwrap();
        assert_eq!(guard.len(), 3);
        assert_eq!(guard[0], "⚡ test command");
        assert_eq!(guard[1], "📝 note #42");
        assert_eq!(guard[2], "💾 fact #5");

        // Clean up
        drop(guard);
        set_tui_callback(None);
    }

    #[test]
    fn test_display_tool_call_filters_empty_values() {
        // Empty values should be filtered out of the compact format
        let args = vec![
            ("command_line".to_string(), "ls -la".to_string()),
            ("head".to_string(), String::new()),
            ("tail".to_string(), String::new()),
            ("timeout_seconds".to_string(), String::new()),
        ];
        let prefix = format!("🔧 run_command(");
        let suffix = ")";
        let prefix_len = prefix.chars().count();
        let suffix_len = suffix.chars().count();
        let content_budget = MAX_LINE_WIDTH.saturating_sub(prefix_len + suffix_len);

        let args_str: Vec<String> = args
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| {
                let v_display = crate::utils::truncate_chars(v, 30);
                format!("{}={}", k, v_display)
            })
            .collect();
        let args_line = args_str.join(", ");
        let display_args = crate::utils::truncate_chars(&args_line, content_budget);
        let line = format!("{prefix}{display_args}{suffix}");

        // Only command_line should appear — empty values filtered out
        assert!(
            line.contains("command_line="),
            "Should contain command_line: {}",
            line
        );
        assert!(
            !line.contains("head="),
            "Should not contain empty head=: {}",
            line
        );
        assert!(
            !line.contains("tail="),
            "Should not contain empty tail=: {}",
            line
        );
        assert!(
            !line.contains("timeout_seconds="),
            "Should not contain empty timeout_seconds=: {}",
            line
        );
    }

    #[test]
    fn test_plain_mode_flag() {
        // Default: plain mode off
        assert!(!PLAIN_MODE.load(Ordering::Relaxed));

        // Enable
        set_plain_mode(true);
        assert!(is_plain_mode());

        // Disable
        set_plain_mode(false);
        assert!(!is_plain_mode());
    }

    #[test]
    fn test_display_tool_call_plain_mode_no_ansi() {
        // In plain mode, display_tool_call should not emit ANSI codes
        set_plain_mode(true);
        set_tui_callback(None); // Ensure terminal mode (no callback)
        // This test verifies the function doesn't panic and routes correctly.
        // We can't easily capture stderr in unit tests, but we verify the
        // plain mode path exists and executes.
        display_tool_call("test_tool", &[("key".to_string(), "value".to_string())]);

        // Restore
        set_plain_mode(false);
    }
}
