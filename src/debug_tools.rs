//! Tool execution logging using the `log` crate.
//!
//! Tool calls are logged at `info` level (visible in Normal mode).
//! Detailed tool parameters and results are logged at `debug` level
//! (visible in Verbose mode, i.e., `-v`).
//!
//! # Verbosity and Tool Logging
//!
//! | Level   | Tool Calls          | Tool Results |
//! |---------|---------------------|--------------|
//! | Quiet   | Hidden (error only) | Hidden       |
//! | Normal  | Compact: 🔧 name()  | Hidden       |
//! | Verbose | Detailed: key=val   | Full output  |
//! | Trace   | Same as Verbose     | Same + extra |

use crate::spinner::suspend_for_print;

/// Toggle debug/logging verbosity between Normal and Trace.
/// Used by the `/debug` command in chat mode.
/// Returns the new verbosity level.
pub fn toggle_debug() -> crate::logging::Verbosity {
    crate::logging::toggle_verbosity()
}

/// Log a tool call with its arguments.
///
/// In Normal mode (info level): compact single-line format `🔧 name(args)`
/// In Verbose mode (debug level): detailed multi-line format with full params
pub fn log_tool_call(tool_name: &str, args: &[(String, String)]) {
    if log::log_enabled!(log::Level::Debug) {
        // Detailed format for verbose/trace mode
        suspend_for_print(|| {
            log::debug!("🔧 {}", tool_name);
            for (key, value) in args {
                let display_value = crate::utils::truncate_chars(value, 77);
                log::debug!("  {}: {}", key, display_value);
            }
        });
    } else {
        // Compact format for normal mode
        let args_str: Vec<String> = args
            .iter()
            .map(|(k, v)| {
                let v_display = crate::utils::truncate_chars(v, 37);
                format!("{}={}", k, v_display)
            })
            .collect();
        suspend_for_print(|| {
            log::info!("🔧 {}({})", tool_name, args_str.join(", "));
        });
    }
}

/// Log tool result (only visible at Verbose/Trace level)
pub fn log_tool_result(tool_name: &str, result: &str) {
    let display_result = if result.chars().count() > 500 {
        let truncated: String = result.chars().take(497).collect();
        let remaining = result.chars().count() - 497;
        format!("{}...[truncated {} chars]", truncated, remaining)
    } else {
        result.to_string()
    };

    suspend_for_print(|| {
        log::debug!("📤 {} result: {}", tool_name, display_result);
    });
}
