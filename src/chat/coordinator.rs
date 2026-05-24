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
                format!("{}: {}", crate::consts::app::ERR_LLM_ERROR, message)
            }
            RecoverableError::JsonError { message } => {
                format!("JSON error: {}", message)
            }
        }
    }
}

/// Classify an OllamaError for recovery
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
                "Error: The LLM server encountered an internal error: {}. \
                 Please try again or provide a response without using tools if the issue persists.",
                message
            )
        }
        RecoverableError::JsonError { message } => {
            format!(
                "Error: Could not parse tool call: {}. \
                 The tool call syntax was malformed (JSON/XML parsing error). \
                 Please check the syntax and try again, or provide a direct response.",
                message
            )
        }
    }
}

/// Check if an OllamaError is recoverable
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
    fn test_is_ollama_error_recoverable() {
        use ollama_rs::error::{OllamaError, ToolCallError};

        // ToolCallError::UnknownToolName is recoverable
        let err = OllamaError::ToolCallError(ToolCallError::UnknownToolName);
        assert!(is_ollama_error_recoverable(&err));

        // ToolCallError::InvalidToolArguments is recoverable
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = OllamaError::ToolCallError(ToolCallError::InvalidToolArguments(json_err));
        assert!(is_ollama_error_recoverable(&err));

        // JsonError is recoverable
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = OllamaError::JsonError(json_err);
        assert!(is_ollama_error_recoverable(&err));

        // InternalError is NOT recoverable
        let err = OllamaError::InternalError(ollama_rs::error::InternalOllamaError {
            message: "test".to_string(),
        });
        assert!(!is_ollama_error_recoverable(&err));
    }
}
