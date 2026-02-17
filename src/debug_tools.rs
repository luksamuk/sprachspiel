//! Debug utilities for tool execution logging
//!
//! Provides functions to log tool calls and their results when debug mode is enabled.

use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

/// Enable debug mode
pub fn enable_debug() {
    DEBUG_MODE.store(true, Ordering::SeqCst);
}

/// Check if debug mode is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_MODE.load(Ordering::SeqCst)
}

/// Log a tool call with its arguments (only in debug mode)
pub fn log_tool_call(tool_name: &str, args: &[(String, String)]) {
    if !is_debug_enabled() {
        return;
    }

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

    eprintln!("📤 TOOL RESULT for {}:", tool_name);
    eprintln!("{}", display_result);
    eprintln!("═══════════════════════════════════════════════════════════════");
}

/// Log a debug message (only in debug mode)
pub fn log_debug(msg: &str) {
    if is_debug_enabled() {
        eprintln!("[DEBUG] {}", msg);
    }
}
