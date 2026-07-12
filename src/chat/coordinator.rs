//! Chat Coordinator with error recovery helpers
//!
//! Provides utilities for handling tool and network errors gracefully
//! by converting them to Tool messages that the model can understand and recover from.

use crate::provider::types::ProviderError;

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
    /// Provider internal error
    ProviderError { message: String },
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
            RecoverableError::ProviderError { message } => {
                format!("{}: {}", crate::consts::app::ERR_LLM_ERROR, message)
            }
        }
    }
}

/// Classify a `ProviderError` for recovery.
///
/// Maps provider error variants to `RecoverableError` so the retry loop
/// can produce a human-readable tool message for the LLM.
pub fn classify_provider_error(
    error: &ProviderError,
    available_tools: &[String],
) -> RecoverableError {
    match error {
        ProviderError::Api { status, body } if *status == 400 => {
            if body.contains("tool") || body.contains("Tool") {
                RecoverableError::InvalidArguments {
                    tool_name: "unknown".to_string(),
                    error: body.clone(),
                }
            } else {
                RecoverableError::ProviderError {
                    message: format!("HTTP {status}: {body}"),
                }
            }
        }
        ProviderError::Api { status, body } => RecoverableError::ProviderError {
            message: format!("HTTP {status}: {body}"),
        },
        ProviderError::RateLimit { message, .. } => RecoverableError::ProviderError {
            message: message.clone(),
        },
        ProviderError::Timeout(msg) | ProviderError::Connection(msg) => {
            RecoverableError::NetworkError {
                message: msg.clone(),
            }
        }
        ProviderError::Config(msg) => RecoverableError::ProviderError {
            message: msg.clone(),
        },
        ProviderError::Unsupported(msg) => RecoverableError::ProviderError {
            message: msg.clone(),
        },
        ProviderError::Other(msg) => {
            if msg.contains("unknown tool") {
                RecoverableError::UnknownTool {
                    tool_name: "unknown".to_string(),
                    available_tools: available_tools.to_vec(),
                }
            } else if msg.contains("invalid tool arguments") {
                RecoverableError::InvalidArguments {
                    tool_name: "unknown".to_string(),
                    error: msg.clone(),
                }
            } else {
                RecoverableError::ProviderError {
                    message: msg.clone(),
                }
            }
        }
    }
}

/// Format a recovery message for the model.
///
/// This message is sent as a Tool message so the model can understand what went wrong.
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
        RecoverableError::ProviderError { message } => {
            format!(
                "Error: The LLM server encountered an internal error: {}. \
                 Please try again or provide a response without using tools if the issue persists.",
                message
            )
        }
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
    fn test_classify_timeout_as_network_error() {
        let err = ProviderError::Timeout("SSE stream idle timeout after 300s".to_string());
        let recovery = classify_provider_error(&err, &[]);
        assert!(matches!(recovery, RecoverableError::NetworkError { .. }));
    }

    #[test]
    fn test_classify_connection_as_network_error() {
        let err = ProviderError::Connection("connection refused".to_string());
        let recovery = classify_provider_error(&err, &[]);
        assert!(matches!(recovery, RecoverableError::NetworkError { .. }));
    }

    #[test]
    fn test_classify_api_500_as_provider_error() {
        let err = ProviderError::Api {
            status: 500,
            body: "Internal Server Error".to_string(),
        };
        let recovery = classify_provider_error(&err, &[]);
        assert!(matches!(recovery, RecoverableError::ProviderError { .. }));
    }

    #[test]
    fn test_classify_api_400_with_tool_as_invalid_arguments() {
        let err = ProviderError::Api {
            status: 400,
            body: "invalid tool call arguments".to_string(),
        };
        let recovery = classify_provider_error(&err, &[]);
        assert!(matches!(
            recovery,
            RecoverableError::InvalidArguments { .. }
        ));
    }

    #[test]
    fn test_classify_other_with_unknown_tool() {
        let err = ProviderError::Other("unknown tool name".to_string());
        let recovery = classify_provider_error(&err, &["read_file".to_string()]);
        assert!(matches!(recovery, RecoverableError::UnknownTool { .. }));
    }
}
