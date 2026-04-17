//! Tool execution logging using the `log` crate.
//!
//! Tool calls are displayed as UI output (eprintln) in Normal mode,
//! and logged at `debug` level in Verbose mode (-v) with full parameters.
//!
//! # Verbosity and Tool Logging
//!
//! | Level   | Tool Calls          | Tool Results |
//! |---------|---------------------|--------------|
//! | Quiet   | Hidden (error only) | Hidden       |
//! | Normal  | Compact: 🔧 name()  | Hidden       |
//! | Verbose | Detailed: key=val   | Full output  |
//! | Trace   | Same as Verbose     | Same + extra |
//!
//! # Implementation Notes
//!
//! In Normal mode, tool calls are printed directly to stderr (via suspend_for_print)
//! so they appear cleanly on the terminal without log-level prefixes.
//! In Verbose/Trace mode, they're logged at `debug` level which includes
//! the full module path, timestamp, and detailed parameters.
//!
//! Tool result previews (`✓ Result:`) are always shown in Normal mode
//! as UI output — they are part of the user experience, not diagnostic logging.

use crate::spinner::suspend_for_print;

/// Toggle debug/logging verbosity between Normal and Trace.
/// Used by the `/debug` command in chat mode.
/// Returns the new verbosity level.
pub fn toggle_debug() -> crate::logging::Verbosity {
    crate::logging::toggle_verbosity()
}

/// Log a tool call with its arguments.
///
/// In Normal mode: compact single-line format `🔧 name(args)` printed to stderr
/// In Verbose/Trace mode: detailed multi-line format via `log::debug!()`
pub fn log_tool_call(tool_name: &str, args: &[(String, String)]) {
    if log::log_enabled!(log::Level::Debug) {
        // Detailed format for verbose/trace mode — goes through env_logger
        // which adds the [LEVEL module] prefix and timestamp
        log::debug!("🔧 {}", tool_name);
        for (key, value) in args {
            let display_value = crate::utils::truncate_chars(value, 77);
            log::debug!("  {}: {}", key, display_value);
        }
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
            eprintln!("🔧 {}({})", tool_name, args_str.join(", "));
        });
    }
    // In Quiet mode (Error level only), neither branch executes — tool calls are hidden
}

/// Log tool result (only visible at Verbose/Trace level).
///
/// In Normal mode, a compact preview is shown as UI output.
/// In Verbose/Trace mode, the full result is logged at debug level.
pub fn log_tool_result(tool_name: &str, result: &str) {
    let display_result = if result.chars().count() > 500 {
        let truncated: String = result.chars().take(497).collect();
        let remaining = result.chars().count() - 497;
        format!("{}...[truncated {} chars]", truncated, remaining)
    } else {
        result.to_string()
    };

    if log::log_enabled!(log::Level::Debug) {
        // Full result in verbose/trace mode
        log::debug!("📤 {} result: {}", tool_name, display_result);
    } else if log::log_enabled!(log::Level::Info) {
        // Compact preview in normal mode — shown as UI, not log
        let preview = crate::utils::truncate_chars(&display_result, 100);
        suspend_for_print(|| {
            eprintln!("✓ Result: {}", preview.replace('\n', " "));
        });
    }
    // In Quiet mode, neither branch executes — tool results are hidden
}
