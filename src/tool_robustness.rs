//! Tool robustness utilities
//!
//! Provides utilities for handling tool call errors gracefully.
//! Instead of crashing, provides detailed error feedback to users.

pub fn format_tool_error(error: &str) -> String {
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

    error.to_string()
}
