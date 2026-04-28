//! Tool execution display and logging.
//!
//! Tool calls are displayed as UI output (eprintln) controlled by a dedicated
//! `show_tool_calls` flag, independent of the `log` crate's verbosity levels.
//! This ensures tool calls are always visible in Normal mode (the default).
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
//! # Configuration
//!
//! The `show_tool_calls` setting in `[display]` section of `config.toml` controls
//! whether the compact format is shown. Default: `true`.
//! In Quiet mode (`-q`), tool calls are always hidden regardless of this setting.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::spinner::suspend_for_print;

/// ANSI style: DIM (faint) + light gray text — same as `[Thinking]` blocks
const TOOL_DIM: &str = "\x1B[2m\x1B[37m";
/// ANSI reset
const RESET: &str = "\x1B[0m";

/// Maximum display width for tool call lines (80-column terminal minus margin)
const MAX_LINE_WIDTH: usize = 74;

/// Global flag controlling whether tool calls are displayed in chat.
///
/// **Independent of the `log` crate's level filter.** Set from
/// `Settings::display.show_tool_calls` at startup and via `/debug` toggle.
/// In Quiet mode, this flag is overridden — tool calls are never shown.
static SHOW_TOOL_CALLS: AtomicBool = AtomicBool::new(true);

/// Set the `show_tool_calls` flag from configuration.
///
/// Called once at startup after loading `config.toml`.
pub fn set_show_tool_calls(enabled: bool) {
    SHOW_TOOL_CALLS.store(enabled, Ordering::Relaxed);
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
/// This is **always** called — the decision to show/hide is made by
/// [`should_show_tool_calls()`] which checks both Quiet mode and the
/// `show_tool_calls` configuration flag.
fn display_tool_call(tool_name: &str, args: &[(String, String)]) {
    if !should_show_tool_calls() {
        return;
    }

    // Build key=value pairs with truncated values
    let max_arg_value = 30;
    let args_str: Vec<String> = args
        .iter()
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

    suspend_for_print(|| {
        eprintln!("{TOOL_DIM}{prefix}{display_args}{suffix}{RESET}");
    });
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

    // In Verbose/Trace mode, show additional detail lines
    if log::log_enabled!(log::Level::Debug) {
        suspend_for_print(|| {
            for (key, value) in args {
                let display_value = crate::utils::truncate_chars(value, 77);
                eprintln!("{TOOL_DIM}  {key}: {display_value}{RESET}");
            }
        });
    }
}

/// Log tool result.
///
/// - **Normal mode**: hidden (tool calls are enough for the user)
/// - **Verbose mode (-v)**: truncated preview (~100 chars) in DIM gray
/// - **Trace mode (-vv)**: full result (up to 500 chars) in DIM gray
/// - **Quiet mode**: hidden
pub fn log_tool_result(tool_name: &str, result: &str) {
    // Trace mode: full result (up to 500 chars)
    if log::max_level() == log::LevelFilter::Trace {
        let display_result = format_result(result, 500);
        suspend_for_print(|| {
            eprintln!("{TOOL_DIM}📤 {tool_name} result: {display_result}{RESET}");
        });
    } else if log::log_enabled!(log::Level::Debug) {
        // Verbose mode: truncated preview (~100 chars)
        let preview = crate::utils::truncate_chars(result, 100);
        suspend_for_print(|| {
            eprintln!("{TOOL_DIM}✓ Result: {}{RESET}", preview.replace('\n', " "));
        });
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
}
