//! Debug utilities for tool execution logging
//!
//! Provides functions to log tool calls and their results.
//! Tool calls are ALWAYS logged (user has right to see what's being executed).
//! Detailed results are only shown in debug mode.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::spinner::suspend_for_print;

static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

/// Enable debug mode
pub fn enable_debug() {
    DEBUG_MODE.store(true, Ordering::SeqCst);
}
pub fn toggle_debug() -> bool {
    let value = !is_debug_enabled();
    DEBUG_MODE.store(value, Ordering::SeqCst);
    value
}

/// Check if debug mode is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_MODE.load(Ordering::SeqCst)
}

/// Log a tool call with its arguments
/// ALWAYS logs (user has right to see what's being executed on their system)
pub fn log_tool_call(tool_name: &str, args: &[(String, String)]) {
    if is_debug_enabled() {
        // Detailed format for debug mode
        suspend_for_print(|| {
            eprintln!();
            eprintln!("═══════════════════════════════════════════════════════════════");
            eprintln!("🔧 TOOL CALL: {}", tool_name);
            eprintln!("───────────────────────────────────────────────────────────────");

            for (key, value) in args {
                let display_value = if value.len() > 80 {
                    format!("{}...", &value[..77])
                } else {
                    value.clone()
                };
                eprintln!("  {}: {}", key, display_value);
            }
            eprintln!("───────────────────────────────────────────────────────────────");
        });
    } else {
        // Compact format for normal mode - always show what tool is being called
        let args_str: Vec<String> = args
            .iter()
            .map(|(k, v)| {
                let v_display = if v.len() > 40 {
                    format!("{}...", &v[..37])
                } else {
                    v.clone()
                };
                format!("{}={}", k, v_display)
            })
            .collect();
        suspend_for_print(|| {
            eprintln!("🔧 Calling: {}({})", tool_name, args_str.join(", "));
        });
    }
}

/// Log tool result (only in debug mode)
pub fn log_tool_result(tool_name: &str, result: &str) {
    if !is_debug_enabled() {
        return;
    }

    let display_result = if result.len() > 500 {
        format!(
            "{}...[truncated {} chars]",
            &result[..497],
            result.len() - 497
        )
    } else {
        result.to_string()
    };

    suspend_for_print(|| {
        eprintln!("📤 TOOL RESULT for {}:", tool_name);
        eprintln!("{}", display_result);
        eprintln!("═══════════════════════════════════════════════════════════════");
    });
}

/// Log a debug message (only in debug mode)
pub fn log_debug(msg: &str) {
    if is_debug_enabled() {
        suspend_for_print(|| {
            eprintln!("[DEBUG] {}", msg);
        });
    }
}
