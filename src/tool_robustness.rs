//! Tool robustness utilities
//!
//! Provides utilities for handling tool call errors gracefully.
//! Instead of crashing, provides detailed error feedback to users.

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
struct OllamaError {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

pub fn format_tool_error(error: &str) -> String {
    // Check if it's a JSON error from Ollama
    if (error.starts_with('{') || error.contains("\"error\":"))
        && let Some(formatted) = try_format_ollama_error(error)
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

    if error.contains("Failed to get response from Ollama") {
        return format!(
            "{}\n\n\
            💡 **Tip:** Could not connect to Ollama. Make sure Ollama is running.",
            error
        );
    }

    error.to_string()
}

fn try_format_ollama_error(error: &str) -> Option<String> {
    // Try to parse as Ollama error JSON
    if let Ok(mut ollama_err) = serde_json::from_str::<OllamaError>(error) {
        // Some errors have the message nested
        if let Some(msg) = ollama_err.error.take().or(ollama_err.message.take()) {
            return Some(format_error_with_status(&msg));
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