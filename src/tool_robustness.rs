//! Tool robustness utilities
//!
//! Provides utilities for handling tool call errors gracefully.
//! Instead of crashing, provides detailed error feedback to users.

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
struct ProviderErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

pub fn format_tool_error(error: &str) -> String {
    // Check if it's a JSON error from the provider
    if (error.starts_with('{') || error.contains("\"error\":"))
        && let Some(formatted) = try_format_provider_error(error)
    {
        return formatted;
    }

    if error.contains("Error calling tool") {
        return format!(
            "{}\n\n\
            💡 **Tip:** A tool execution failed. Common causes:\n\
            - File not found or wrong path\n\
            - Invalid parameters (e.g., start_line > file length)\n\
            - Permission denied\n\n\
            The tool should have returned a helpful error message. Try with -d for debug output.",
            error
        );
    }

    if error.contains("invalid character") || error.contains("unexpected character") {
        return format!(
            "{}\n\n\
            💡 **Tip:** This error usually means the model generated malformed JSON in a tool call.\n\
            The model will retry automatically.",
            error
        );
    }

    if error.contains("tool") && error.contains("not found") {
        return format!(
            "{}\n\n\
            💡 **Tip:** The model tried to call a tool that doesn't exist.\n\
            Available tools: weather, pokemon, file operations.",
            error
        );
    }

    if error.contains("timeout") || error.contains("timed out") {
        return format!(
            "{}\n\n\
            💡 **Tip:** The request timed out. Try again or use a lighter model.",
            error
        );
    }

    if error.contains("Failed to get response from the provider") {
        return format!(
            "{}\n\n\
            💡 **Tip:** {}",
            error,
            crate::consts::app::ERR_LLM_CONNECTION
        );
    }

    error.to_string()
}

fn try_format_provider_error(error: &str) -> Option<String> {
    // The error string is usually prefixed with status info from
    // convert_provider_error, e.g. "HTTP 400: {<body>}". Try the
    // whole string first; if that fails, try just the JSON portion
    // after the first colon+space.
    let candidates: &[&str] = &[
        error,
        // Strip leading "HTTP NNN: " / "HTTP NNN " prefix.
        if let Some(idx) = error.find("{") {
            &error[idx..]
        } else {
            ""
        },
    ];

    // Try to parse as provider error JSON
    for candidate in candidates {
        if let Ok(mut provider_err) = serde_json::from_str::<ProviderErrorResponse>(candidate) {
            // Some errors have the message nested
            if let Some(msg) = provider_err.error.take().or(provider_err.message.take()) {
                return Some(format_error_with_status(&msg));
            }
        }
    }

    // Parse OpenAI-style error envelope used by
    // llama-swap / vLLM:
    //   {"error": {"code": 400, "message": "request (N tokens) exceeds
    //    the available context size (M tokens)"}}
    //
    // Older format is just {"error": "message"} or
    // {"message": "message"}; both forms use a string value, not an
    // object. The ProviderErrorResponse struct above catches the simple
    // string forms. For the OpenAI object form, walk the JSON
    // manually so we can extract the nested "message" field.
    for candidate in candidates {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
            if let Some(err_obj) = value.get("error") {
                if let Some(msg) = err_obj.get("message").and_then(|v| v.as_str()) {
                    let code = err_obj
                        .get("code")
                        .and_then(|v| v.as_u64())
                        .map(|c| c as u16);
                    return Some(format_openai_error(code, msg));
                }
                if let Some(msg) = err_obj.as_str() {
                    return Some(format_error_with_status(msg));
                }
            }
            if let Some(msg) = value.get("message").and_then(|v| v.as_str()) {
                return Some(format_error_with_status(msg));
            }
        }
    }

    // Try to find error message in the string
    if error.contains("\"error\"") {
        // Extract error message using simple string parsing
        if let Some(start) = error.find("\"error\"") {
            let rest = &error[start..];
            if let Some(content_start) = rest.find(':') {
                let content = &rest[content_start + 1..];
                // Find the string value
                if let Some(quote_start) = content.find('"') {
                    let after_quote = &content[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find('"') {
                        let msg = &after_quote[..quote_end];
                        return Some(format_error_with_status(msg));
                    }
                }
            }
        }
    }

    None
}

fn format_error_with_status(msg: &str) -> String {
    // In plain mode (pipe-safe) or TUI mode (ratatui handles styling),
    // produce plain text without ANSI codes. The TUI renderer applies its
    // own colors via Span::styled(), so ANSI codes would appear as garbled
    // text like "␛[31mError:␛[0m Internal Server Error".
    if crate::debug_tools::is_plain_mode() || crate::logging::is_tui_mode() {
        return format_error_plain(msg);
    }

    format_error_with_ansi(msg)
}

/// Format an OpenAI-style error envelope with the HTTP status
/// code, if present. This is the format llama-swap and vLLM use for
/// context-overflow and similar errors:
///
/// ```json
/// {"error": {"code": 400, "message": "request (...) exceeds ..."}}
/// ```
///
/// The plain formatter annotates the message with the standard HTTP
/// status name (e.g. "400 Bad Request") so the TUI ⛔ ERROR banner
/// tells the user what kind of error they hit, not just a status
/// number.
fn format_openai_error(code: Option<u16>, msg: &str) -> String {
    if let Some(c) = code {
        let status_name = match c {
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            408 => "Request Timeout",
            413 => "Payload Too Large",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "HTTP Error",
        };
        let header = format!("HTTP {c} {status_name}");
        if crate::debug_tools::is_plain_mode() || crate::logging::is_tui_mode() {
            format!("{header}: {msg}")
        } else {
            format!("\x1B[31m{header}\x1B[0m: {msg}")
        }
    } else {
        format_error_with_status(msg)
    }
}

/// Format error without any ANSI codes (for plain mode / pipe-safe output).
fn format_error_plain(msg: &str) -> String {
    let status_patterns = [
        ("status 400", "400 Bad Request"),
        ("status 404", "404 Not Found"),
        ("status 500", "500 Internal Server Error"),
        ("code 400", "400 Bad Request"),
        ("code 404", "404 Not Found"),
        ("code 500", "500 Internal Server Error"),
    ];

    let mut formatted = msg.to_string();

    for (pattern, status_text) in status_patterns {
        if formatted.to_lowercase().contains(pattern) {
            formatted = formatted.replace(pattern, status_text);
            formatted = formatted.replace(&pattern.to_uppercase(), status_text);
        }
    }

    if formatted.contains("error") || formatted.contains("Error") || formatted.contains("ERROR") {
        format!("Error: {}", formatted)
    } else {
        formatted
    }
}

/// Format error with ANSI color codes (for terminal mode).
fn format_error_with_ansi(msg: &str) -> String {
    // Check for status codes and format them in red
    let status_patterns = [
        ("status 400", "400 Bad Request"),
        ("status 404", "404 Not Found"),
        ("status 500", "500 Internal Server Error"),
        ("code 400", "400 Bad Request"),
        ("code 404", "404 Not Found"),
        ("code 500", "500 Internal Server Error"),
    ];

    let mut formatted = msg.to_string();

    for (pattern, status_text) in status_patterns {
        if formatted.to_lowercase().contains(pattern) {
            // Red ANSI: \x1B[31m, Reset: \x1B[0m
            let red_status = format!("\x1B[31m{}\x1B[0m", status_text);
            formatted = formatted.replace(pattern, &red_status);
            formatted = formatted.replace(&pattern.to_uppercase(), &red_status);
        }
    }

    // Add red color to common error indicators
    if formatted.contains("error") || formatted.contains("Error") || formatted.contains("ERROR") {
        format!("\x1B[31mError:\x1B[0m {}", formatted)
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shared RAII guard that restores plain mode on drop.
    // Defined in debug_tools but re-imported here for convenience.
    use crate::debug_tools::PlainModeGuard;

    // All tests below mutate the global PLAIN_MODE AtomicBool.
    // #[serial] prevents flaky cross-test interference in parallel runs.
    // (Same pattern as spinner.rs tests that touch global spinner state.)

    #[serial_test::serial]
    #[test]
    fn test_format_error_plain_no_ansi() {
        // Plain mode: no ANSI codes in output
        let _guard = PlainModeGuard::new();
        let result = format_error_with_status("connection error: status 404 not found");
        assert!(
            !result.contains("\x1B["),
            "Plain mode should not contain ANSI codes, got: {result}"
        );
        assert!(
            result.contains("404 Not Found"),
            "Plain mode should contain readable status text, got: {result}"
        );
    }

    #[serial_test::serial]
    #[test]
    fn test_format_error_ansi_contains_codes() {
        // Non-plain mode: should contain ANSI codes.
        // PlainModeGuard saves/restores original state; we override to false.
        let original = crate::debug_tools::is_plain_mode();
        crate::debug_tools::set_plain_mode(false);
        let result = format_error_with_status("connection error: status 404 not found");
        assert!(
            result.contains("\x1B["),
            "Terminal mode should contain ANSI codes, got: {result}"
        );
        crate::debug_tools::set_plain_mode(original);
    }

    #[serial_test::serial]
    #[test]
    fn test_format_error_plain_error_prefix() {
        // Plain mode: "Error:" prefix without ANSI red
        let _guard = PlainModeGuard::new();
        let result = format_error_with_status("server error occurred");
        assert!(
            result.starts_with("Error:"),
            "Plain mode should have plain 'Error:' prefix, got: {result}"
        );
        assert!(
            !result.contains("\x1B["),
            "Plain mode should not contain ANSI codes, got: {result}"
        );
    }

    #[serial_test::serial]
    #[test]
    fn test_format_error_tui_mode_no_ansi() {
        // TUI mode: no ANSI codes (ratatui handles styling)
        let original_plain = crate::debug_tools::is_plain_mode();
        let original_tui = crate::logging::is_tui_mode();
        crate::debug_tools::set_plain_mode(false);
        crate::logging::set_tui_mode(true);
        let result = format_error_with_status("connection error: status 404 not found");
        assert!(
            !result.contains("\x1B["),
            "TUI mode should not contain ANSI codes, got: {result}"
        );
        assert!(
            result.contains("404 Not Found"),
            "TUI mode should contain readable status text, got: {result}"
        );
        crate::debug_tools::set_plain_mode(original_plain);
        crate::logging::set_tui_mode(original_tui);
    }

    #[serial_test::serial]
    #[test]
    fn test_format_tool_error_no_ansi_in_tui() {
        // format_tool_error (public API) should not produce ANSI in TUI mode
        let original_plain = crate::debug_tools::is_plain_mode();
        let original_tui = crate::logging::is_tui_mode();
        crate::debug_tools::set_plain_mode(false);
        crate::logging::set_tui_mode(true);
        let result = format_tool_error(r#"{"error":"status 400 Bad Request"}"#);
        assert!(
            !result.contains("\x1B["),
            "format_tool_error in TUI mode should not contain ANSI, got: {result}"
        );
        crate::debug_tools::set_plain_mode(original_plain);
        crate::logging::set_tui_mode(original_tui);
    }

    // Regression tests for the OpenAI-style error envelope
    // produced by llama-swap / vLLM. Before this fix, the parser
    // extracted only the first quoted fragment ("code") and dropped
    // the human-readable message nested under `error.message`.

    #[serial_test::serial]
    #[test]
    fn test_format_tool_error_openai_envelope_extracts_message() {
        let original_plain = crate::debug_tools::is_plain_mode();
        let original_tui = crate::logging::is_tui_mode();
        crate::debug_tools::set_plain_mode(true);
        crate::logging::set_tui_mode(false);
        let body = r#"HTTP 400: {"error":{"code":400,"message":"request (38449 tokens) exceeds the available context size (32768 tokens)","type":"exceed_context_size_error","n_prompt_tokens":38449,"n_ctx":32768}}"#;
        let result = format_tool_error(body);
        assert!(
            result.contains("38449 tokens"),
            "OpenAI error parser should extract the message field, got: {result}"
        );
        assert!(
            result.contains("32768 tokens"),
            "OpenAI error parser should preserve context size, got: {result}"
        );
        assert!(
            !result.contains(r#""error""#) || result.contains("exceeds"),
            "OpenAI error parser should not just dump raw JSON keys, got: {result}"
        );
        crate::debug_tools::set_plain_mode(original_plain);
        crate::logging::set_tui_mode(original_tui);
    }

    #[serial_test::serial]
    #[test]
    fn test_format_tool_error_openai_envelope_with_status() {
        // The status code in the envelope should be reflected in the
        // formatted output (e.g. "400 Bad Request").
        let original_plain = crate::debug_tools::is_plain_mode();
        let original_tui = crate::logging::is_tui_mode();
        crate::debug_tools::set_plain_mode(true);
        crate::logging::set_tui_mode(false);
        let body = r#"HTTP 400: {"error":{"code":503,"message":"Service unavailable"}}"#;
        let result = format_tool_error(body);
        assert!(
            result.contains("503") && result.contains("Service Unavailable"),
            "OpenAI error parser should annotate 503 as Service Unavailable, got: {result}"
        );
        crate::debug_tools::set_plain_mode(original_plain);
        crate::logging::set_tui_mode(original_tui);
    }

    #[serial_test::serial]
    #[test]
    fn test_format_tool_error_legacy_provider_string_error() {
        // The legacy format uses "error" as a string, not an
        // object. The original parser must still work.
        let original_plain = crate::debug_tools::is_plain_mode();
        let original_tui = crate::logging::is_tui_mode();
        crate::debug_tools::set_plain_mode(true);
        crate::logging::set_tui_mode(false);
        let body = r#"HTTP 500: {"error":"status 500 Internal Server Error"}"#;
        let result = format_tool_error(body);
        assert!(
            result.contains("500 Internal Server Error"),
            "Legacy provider string error must still parse, got: {result}"
        );
        crate::debug_tools::set_plain_mode(original_plain);
        crate::logging::set_tui_mode(original_tui);
    }
}
