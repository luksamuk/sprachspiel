//! Chat Coordinator with error recovery helpers
//!
//! Provides utilities for handling tool and network errors gracefully
//! by converting them to Tool messages that the model can understand and recover from.

use ollama_rs::error::OllamaError;

/// Maximum retry attempts for recoverable errors
pub const MAX_RETRIES: usize = 3;

/// Error classification for recovery
#[derive(Debug, Clone)]
pub enum RecoverableError {
    /// Model called a tool that doesn't exist
    UnknownTool {
        tool_name: String,
        available_tools: Vec<String>,
    },
    /// Tool arguments were invalid (malformed JSON)
    InvalidArguments { tool_name: String, error: String },
    /// Network/timeout error
    NetworkError { message: String },
    /// Ollama internal error
    OllamaError { message: String },
    /// JSON parsing error
    JsonError { message: String },
}

impl RecoverableError {
    /// Human-readable description of the error
    pub fn description(&self) -> String {
        match self {
            RecoverableError::UnknownTool { tool_name, .. } => {
                format!("Unknown tool: {}", tool_name)
            }
            RecoverableError::InvalidArguments { tool_name, .. } => {
                format!("Invalid arguments for tool: {}", tool_name)
            }
            RecoverableError::NetworkError { message } => {
                format!("Network error: {}", message)
            }
            RecoverableError::OllamaError { message } => {
                format!("Ollama error: {}", message)
            }
            RecoverableError::JsonError { message } => {
                format!("JSON error: {}", message)
            }
        }
    }

    /// Check if this error is recoverable (model can retry)
    #[allow(dead_code)]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RecoverableError::UnknownTool { .. }
                | RecoverableError::InvalidArguments { .. }
                | RecoverableError::NetworkError { .. }
        )
    }
}

/// Check if an error string matches a pattern (case insensitive)
fn error_matches(error_str: &str, pattern: &str) -> bool {
    error_str.to_lowercase().contains(&pattern.to_lowercase())
}

/// Classify an OllamaError for recovery
#[allow(dead_code)]
pub fn classify_ollama_error(error: &OllamaError, available_tools: &[String]) -> RecoverableError {
    match error {
        OllamaError::ToolCallError(e) => match e {
            ollama_rs::error::ToolCallError::UnknownToolName => RecoverableError::UnknownTool {
                tool_name: "unknown".to_string(),
                available_tools: available_tools.to_vec(),
            },
            ollama_rs::error::ToolCallError::InvalidToolArguments(json_err) => {
                RecoverableError::InvalidArguments {
                    tool_name: "unknown".to_string(),
                    error: json_err.to_string(),
                }
            }
            ollama_rs::error::ToolCallError::InternalToolError(tool_err) => {
                RecoverableError::OllamaError {
                    message: tool_err.to_string(),
                }
            }
        },
        OllamaError::ReqwestError(e) => RecoverableError::NetworkError {
            message: e.to_string(),
        },
        OllamaError::InternalError(e) => RecoverableError::OllamaError {
            message: e.message.clone(),
        },
        OllamaError::JsonError(e) => RecoverableError::JsonError {
            message: e.to_string(),
        },
        OllamaError::Other(msg) => RecoverableError::OllamaError {
            message: msg.clone(),
        },
    }
}

/// Classify an error string for recovery (for non-OllamaError cases)
pub fn classify_error_str(error_str: &str, available_tools: &[String]) -> RecoverableError {
    if error_matches(error_str, "unknown tool")
        || (error_matches(error_str, "tool") && error_matches(error_str, "not found"))
    {
        RecoverableError::UnknownTool {
            tool_name: "unknown".to_string(),
            available_tools: available_tools.to_vec(),
        }
    } else if error_matches(error_str, "invalid") && error_matches(error_str, "argument") {
        RecoverableError::InvalidArguments {
            tool_name: "unknown".to_string(),
            error: error_str.to_string(),
        }
    } else if error_matches(error_str, "network")
        || error_matches(error_str, "timeout")
        || error_matches(error_str, "connection")
        || error_matches(error_str, "reqwest")
    {
        RecoverableError::NetworkError {
            message: error_str.to_string(),
        }
    } else if error_matches(error_str, "json") || error_matches(error_str, "parse") {
        RecoverableError::JsonError {
            message: error_str.to_string(),
        }
    } else {
        RecoverableError::OllamaError {
            message: error_str.to_string(),
        }
    }
}

/// Format a recovery message for the model
/// This message is sent as a Tool message so the model can understand what went wrong
pub fn format_recovery_message(error: &RecoverableError) -> String {
    match error {
        RecoverableError::UnknownTool {
            tool_name,
            available_tools,
        } => {
            if available_tools.is_empty() {
                format!(
                    "Error: The tool '{}' does not exist. \
                     No tools are currently available. \
                     Please provide a direct response without using tools.",
                    tool_name
                )
            } else {
                format!(
                    "Error: The tool '{}' does not exist. \
                     Available tools are: {}. \
                     Please use one of the available tools or provide a direct response.",
                    tool_name,
                    available_tools.join(", ")
                )
            }
        }
        RecoverableError::InvalidArguments { tool_name, error } => {
            format!(
                "Error: Invalid arguments for tool '{}'. \
                 The JSON was malformed or incorrect: {}. \
                 Please fix the arguments and try again, or provide a direct response.",
                tool_name, error
            )
        }
        RecoverableError::NetworkError { message } => {
            format!(
                "Error: Network or connection issue: {}. \
                 Please try your request again, or provide a response without using tools.",
                message
            )
        }
        RecoverableError::OllamaError { message } => {
            format!(
                "Error: Ollama encountered an internal error: {}. \
                 Please try again or provide a response without using tools if the issue persists.",
                message
            )
        }
        RecoverableError::JsonError { message } => {
            format!(
                "Error: Could not parse response: {}. \
                 This might be a malformed tool call. \
                 Please try again with correct formatting, or provide a direct response.",
                message
            )
        }
    }
}

/// Check if an OllamaError is recoverable
#[allow(dead_code)]
pub fn is_ollama_error_recoverable(error: &OllamaError) -> bool {
    match error {
        OllamaError::ToolCallError(e) => matches!(
            e,
            ollama_rs::error::ToolCallError::UnknownToolName
                | ollama_rs::error::ToolCallError::InvalidToolArguments(_)
        ),
        OllamaError::ReqwestError(_) => true,
        OllamaError::InternalError(_) => false,
        OllamaError::JsonError(_) => true,
        OllamaError::Other(_) => false,
    }
}

/// Check if an error string indicates a recoverable error
pub fn is_error_str_recoverable(error_str: &str) -> bool {
    let lower = error_str.to_lowercase();

    // Network and timeout errors are recoverable
    if lower.contains("network")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("reqwest")
    {
        return true;
    }

    // Tool errors are recoverable
    if lower.contains("unknown tool")
        || (lower.contains("tool") && lower.contains("not found"))
        || (lower.contains("invalid") && lower.contains("argument"))
    {
        return true;
    }

    // JSON parse errors might be recoverable
    if lower.contains("json") || lower.contains("parse") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_unknown_tool_error() {
        let error = RecoverableError::UnknownTool {
            tool_name: "foo_bar".to_string(),
            available_tools: vec!["read_file".to_string(), "web_search".to_string()],
        };
        let msg = format_recovery_message(&error);
        assert!(msg.contains("foo_bar"));
        assert!(msg.contains("read_file, web_search"));
    }

    #[test]
    fn test_format_network_error() {
        let error = RecoverableError::NetworkError {
            message: "connection timeout".to_string(),
        };
        let msg = format_recovery_message(&error);
        assert!(msg.contains("connection timeout"));
        assert!(msg.contains("Network or connection"));
    }

    #[test]
    fn test_format_invalid_arguments() {
        let error = RecoverableError::InvalidArguments {
            tool_name: "read_file".to_string(),
            error: "missing field 'path'".to_string(),
        };
        let msg = format_recovery_message(&error);
        assert!(msg.contains("read_file"));
        assert!(msg.contains("missing field 'path'"));
    }

    #[test]
    fn test_recoverable_error_is_recoverable() {
        assert!(
            RecoverableError::UnknownTool {
                tool_name: "test".to_string(),
                available_tools: vec![],
            }
            .is_recoverable()
        );

        assert!(
            RecoverableError::NetworkError {
                message: "test".to_string(),
            }
            .is_recoverable()
        );

        assert!(
            !RecoverableError::OllamaError {
                message: "test".to_string(),
            }
            .is_recoverable()
        );
    }

    #[test]
    fn test_is_error_str_recoverable() {
        assert!(is_error_str_recoverable(
            "Network error: connection refused"
        ));
        assert!(is_error_str_recoverable("Request timeout after 30s"));
        assert!(is_error_str_recoverable("Unknown tool not found"));
        assert!(is_error_str_recoverable("Invalid arguments for tool"));
        assert!(is_error_str_recoverable("JSON parse error"));
        assert!(!is_error_str_recoverable("Internal server error"));
    }
}
