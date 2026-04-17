//! Tool execution logging using the `log` crate.
//!
//! Tool calls are displayed as UI output (eprintln) in Normal mode,
//! and logged at `debug` level in Verbose mode (-v) with full parameters.
//!
//! # Verbosity and Tool Logging
//!
//! | Level   | Tool Calls          | Tool Results        |
//! |---------|---------------------|----------------------|
//! | Quiet   | Hidden (error only) | Hidden               |
//! | Normal  | Compact: 🔧 name()  | Hidden               |
//! | Verbose | Detailed: key=val   | Truncated (~100 chr) |
//! | Trace   | Detailed: key=val   | Full output (500 chr) |
//!
//! # Implementation Notes
//!
//! In Normal mode, tool calls are printed directly to stderr (via suspend_for_print)
//! so they appear cleanly on the terminal without log-level prefixes.
//! Tool results are hidden in Normal mode — they're diagnostic info, not essential UI.
//!
//! In Verbose/Trace mode, both calls and results are printed via `eprintln!`
//! (not `log::debug!`) so we can apply consistent DIM+gray styling.
//! The `log::debug!` path is only used for internal coordinator diagnostics.

use crate::spinner::suspend_for_print;

/// ANSI style: DIM (faint) + light gray text — same as `[Thinking]` blocks
const TOOL_DIM: &str = "\x1B[2m\x1B[37m";
/// ANSI reset
const RESET: &str = "\x1B[0m";

/// Toggle debug/logging verbosity between Normal and Trace.
/// Used by the `/debug` command in chat mode.
/// Returns the new verbosity level.
pub fn toggle_debug() -> crate::logging::Verbosity {
    crate::logging::toggle_verbosity()
}

/// Log a tool call with its arguments.
///
/// - **Normal mode**: compact single-line format `🔧 name(args)` in DIM gray
/// - **Verbose/Trace mode**: detailed multi-line format in DIM gray
///   (printed via `eprintln!`, not `log::debug!`, for consistent styling)
/// - **Quiet mode**: hidden
pub fn log_tool_call(tool_name: &str, args: &[(String, String)]) {
    if log::log_enabled!(log::Level::Debug) {
        // Detailed format for verbose/trace mode — printed directly
        // so we can apply DIM gray styling consistently
        suspend_for_print(|| {
            eprintln!("{TOOL_DIM}🔧 {tool_name}{RESET}");
            for (key, value) in args {
                let display_value = crate::utils::truncate_chars(value, 77);
                eprintln!("{TOOL_DIM}  {key}: {display_value}{RESET}");
            }
        });
    } else if log::log_enabled!(log::Level::Info) {
        // Compact format for normal mode — printed directly to stderr
        // without log-level prefix, so it appears clean in the terminal
        let args_str: Vec<String> = args
            .iter()
            .map(|(k, v)| {
                let v_display = crate::utils::truncate_chars(v, 37);
                format!("{}={}", k, v_display)
            })
            .collect();
        suspend_for_print(|| {
            eprintln!("{TOOL_DIM}🔧 {}({}){RESET}", tool_name, args_str.join(", "));
        });
    }
    // In Quiet mode (Error level only), neither branch executes — tool calls are hidden
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
