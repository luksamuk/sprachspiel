//! Retry backoff and cancel-aware sleep for LLM provider errors.
//!
//! `RetryCategory`, `ProviderError::retry_category()`, and `retry_delay()`
//! are defined in [`crate::provider::types`]. This module provides
//! `sleep_or_cancel`, a temporary `OllamaError`→`ProviderError` bridge,
//! and a `classify_for_retry` convenience wrapper.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::provider::types::{ProviderError, RetryCategory};

pub use crate::provider::types::retry_delay;

/// Classify a `ProviderError` into a `RetryCategory`.
///
/// Delegates to `ProviderError::retry_category()`.
pub fn classify_for_retry(error: &ProviderError) -> RetryCategory {
    error.retry_category()
}

/// Temporary bridge: convert `OllamaError` to `ProviderError` for the retry loop.
///
/// This exists only during Phase 1 of #123. It will be removed in Phase 2
/// (LUC-41) when `Coordinator` returns `ProviderError` directly.
pub fn ollama_error_to_provider_error(error: &ollama_rs::error::OllamaError) -> ProviderError {
    use ollama_rs::error::{OllamaError, ToolCallError};

    match error {
        OllamaError::ToolCallError(e) => match e {
            ToolCallError::UnknownToolName => ProviderError::Other("unknown tool name".to_string()),
            ToolCallError::InvalidToolArguments(json_err) => {
                ProviderError::Other(format!("invalid tool arguments: {json_err}"))
            }
            ToolCallError::InternalToolError(tool_err) => {
                ProviderError::Other(format!("internal tool error: {tool_err}"))
            }
        },
        OllamaError::ReqwestError(e) => {
            let msg = e.to_string();
            if msg.contains("timed out") || msg.contains("Timeout") {
                ProviderError::Timeout(msg)
            } else {
                ProviderError::Connection(msg)
            }
        }
        OllamaError::InternalError(e) => ProviderError::Api {
            status: 500,
            body: e.message.clone(),
        },
        OllamaError::JsonError(e) => ProviderError::Other(format!("JSON error: {e}")),
        OllamaError::Other(msg) => {
            if msg.contains("Timeout:") || msg.contains("SSE stream idle timeout") {
                ProviderError::Timeout(msg.clone())
            } else {
                ProviderError::Other(msg.clone())
            }
        }
    }
}

/// Sleep for the given duration, aborting early if `cancel_token` fires.
///
/// Returns `true` if the sleep completed normally, `false` if cancelled.
pub async fn sleep_or_cancel(delay: Duration, cancel_token: Option<&CancellationToken>) -> bool {
    if delay.is_zero() {
        return true;
    }
    match cancel_token {
        Some(token) => tokio::select! {
            _ = tokio::time::sleep(delay) => true,
            _ = token.cancelled() => {
                log::info!("Retry backoff cancelled by user");
                false
            }
        },
        None => {
            tokio::time::sleep(delay).await;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- retry_delay (delegated from types.rs) ---

    #[test]
    fn test_retry_delay_server() {
        let cat = RetryCategory::ServerRetry { max_attempts: 3 };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(5));
        assert_eq!(retry_delay(&cat, 2), Duration::from_secs(10));
        assert_eq!(retry_delay(&cat, 3), Duration::from_secs(15));
    }

    #[test]
    fn test_retry_delay_network_exponential() {
        let cat = RetryCategory::NetworkRetry { max_attempts: 5 };
        assert_eq!(retry_delay(&cat, 1), Duration::from_millis(100));
        assert_eq!(retry_delay(&cat, 2), Duration::from_millis(200));
        assert_eq!(retry_delay(&cat, 3), Duration::from_millis(400));
        assert_eq!(retry_delay(&cat, 4), Duration::from_millis(800));
        assert_eq!(retry_delay(&cat, 5), Duration::from_millis(1600));
    }

    #[test]
    fn test_retry_delay_immediate_zero() {
        let cat = RetryCategory::ImmediateRetry { max_attempts: 3 };
        assert_eq!(retry_delay(&cat, 1), Duration::ZERO);
    }

    #[test]
    fn test_retry_delay_ratelimit_with_retry_after() {
        let cat = RetryCategory::RateLimitRetry {
            max_attempts: 3,
            retry_after: Some(Duration::from_secs(3)),
        };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(3));
    }

    #[test]
    fn test_retry_delay_ratelimit_without_retry_after() {
        let cat = RetryCategory::RateLimitRetry {
            max_attempts: 3,
            retry_after: None,
        };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(2));
    }

    // --- classify_for_retry (delegates to ProviderError::retry_category) ---

    #[test]
    fn test_classify_server_error() {
        let err = ProviderError::Api {
            status: 500,
            body: "Internal Server Error".to_string(),
        };
        assert!(matches!(
            classify_for_retry(&err),
            RetryCategory::ServerRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_classify_timeout_error() {
        let err = ProviderError::Timeout("SSE stream idle timeout after 300s".to_string());
        assert!(matches!(
            classify_for_retry(&err),
            RetryCategory::NetworkRetry { max_attempts: 5 }
        ));
    }

    #[test]
    fn test_classify_connection_error() {
        let err = ProviderError::Connection("connection refused".to_string());
        assert!(matches!(
            classify_for_retry(&err),
            RetryCategory::NetworkRetry { max_attempts: 5 }
        ));
    }

    #[test]
    fn test_classify_ratelimit_error() {
        let err = ProviderError::RateLimit {
            message: "429 Too Many Requests".to_string(),
            retry_after: Some(Duration::from_secs(5)),
        };
        assert!(matches!(
            classify_for_retry(&err),
            RetryCategory::RateLimitRetry {
                max_attempts: 3,
                retry_after: Some(_)
            }
        ));
    }

    #[test]
    fn test_classify_config_error() {
        let err = ProviderError::Config("missing api key".to_string());
        assert!(matches!(classify_for_retry(&err), RetryCategory::NoRetry));
    }

    #[test]
    fn test_classify_other_error() {
        let err = ProviderError::Other("cancelled by user".to_string());
        assert!(matches!(classify_for_retry(&err), RetryCategory::NoRetry));
    }

    #[test]
    fn test_classify_api_400_error() {
        let err = ProviderError::Api {
            status: 400,
            body: "invalid tool call arguments".to_string(),
        };
        assert!(matches!(classify_for_retry(&err), RetryCategory::NoRetry));
    }

    // --- sleep_or_cancel ---

    #[tokio::test]
    async fn test_sleep_or_cancel_zero_delay() {
        assert!(sleep_or_cancel(Duration::ZERO, None).await);
    }

    #[tokio::test]
    async fn test_sleep_or_cancel_no_token() {
        let start = std::time::Instant::now();
        let result = sleep_or_cancel(Duration::from_millis(10), None).await;
        assert!(result);
        assert!(start.elapsed() >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_sleep_or_cancel_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        let start = std::time::Instant::now();
        let result = sleep_or_cancel(Duration::from_secs(10), Some(&token)).await;
        assert!(!result);
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
