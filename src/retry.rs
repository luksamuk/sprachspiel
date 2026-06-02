//! Retry classification and backoff for LLM provider errors.
//!
//! **W2 Wave Context:** This module is the foundation of the W2 Provider
//! Chain (Issue #116 → #123). The `RetryCategory` enum and backoff logic
//! are provider-agnostic. In #119 (Agnostic Provider Types), this module
//! is relocated to `src/provider/retry.rs` and `classify_for_retry` is
//! reimplemented to accept `ProviderError` instead of `OllamaError`.
//!
//! The `RateLimitRetry` variant is pre-emptive support for #122
//! (OpenAI-compatible provider) — the `retry_after` field is parsed
//! from the `Retry-After` header when that issue is implemented.
//!
//! Within the W2 mini-sprint, the project-wide `#[allow(dead_code)]`
//! policy is **flexibilized**: code prepared for W2 future use is
//! acceptable as long as it is resolved by the W2 closure (#123).
//! Every `#[allow(dead_code)]` in this module carries the future W2
//! issue number as justification.
//!
//! See: `IMPLEMENTATION.md` — W2 Provider Chain

use std::time::Duration;

use ollama_rs::error::{OllamaError, ToolCallError};
use tokio_util::sync::CancellationToken;

// Per-category retry limits
const MAX_SERVER_RETRIES: usize = 3;
const MAX_NETWORK_RETRIES: usize = 5;
const MAX_TOOL_RETRIES: usize = 3;
#[allow(dead_code)] // Used in #122 RateLimitRetry construction
const MAX_RATELIMIT_RETRIES: usize = 3;

// Server backoff: 5s, 10s, 15s (linear)
const SERVER_BACKOFF_BASE_SECS: u64 = 5;

// Network backoff: 100ms, 200ms, 400ms, 800ms, 1.6s (exponential, base 2)
const NETWORK_BACKOFF_BASE_MS: u64 = 100;
const NETWORK_BACKOFF_MAX_MS: u64 = 1600;

// RateLimit default backoff when Retry-After header is absent
const RATELIMIT_DEFAULT_BACKOFF_SECS: u64 = 2;

/// Retry strategy for a classified error.
///
/// Each variant carries its own `max_attempts` so per-category limits
/// are visible in the type signature. The `RateLimitRetry::retry_after`
/// field is set when the `Retry-After` HTTP header is parsed (W2 #122).
#[allow(clippy::enum_variant_names)] // The "Retry" suffix is intentional and semantic
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryCategory {
    /// Tool execution failures — retry immediately, no delay
    ImmediateRetry { max_attempts: usize },
    /// Network/timeout errors — exponential backoff
    NetworkRetry { max_attempts: usize },
    /// HTTP 500, OOM, cold start — long linear backoff
    ServerRetry { max_attempts: usize },
    /// Rate limiting (HTTP 429) — respects `Retry-After` header.
    /// `retry_after` is `None` until #122 wires up header parsing.
    #[allow(dead_code)] // Constructed in #122 OpenAI-compatible provider
    RateLimitRetry {
        max_attempts: usize,
        #[allow(dead_code)] // Used in #122 Retry-After header parsing
        retry_after: Option<Duration>,
    },
    /// Non-recoverable errors (cancel, context overflow, malformed JSON)
    NoRetry,
}

impl RetryCategory {
    /// Maximum number of retry attempts for this category
    pub fn max_attempts(&self) -> usize {
        match self {
            RetryCategory::ImmediateRetry { max_attempts } => *max_attempts,
            RetryCategory::NetworkRetry { max_attempts } => *max_attempts,
            RetryCategory::ServerRetry { max_attempts } => *max_attempts,
            RetryCategory::RateLimitRetry { max_attempts, .. } => *max_attempts,
            RetryCategory::NoRetry => 0,
        }
    }

    /// Whether the category is eligible for retry at all
    pub fn is_retryable(&self) -> bool {
        !matches!(self, RetryCategory::NoRetry)
    }
}

/// Classify an `OllamaError` into a `RetryCategory`.
///
/// This maps every variant of `OllamaError` (today's provider error type)
/// to a retry strategy. The function is the **foundation** of the W2 retry
/// subsystem. In #119, it gets a sibling `classify_for_retry(&ProviderError)`
/// with the same shape but different variant names.
pub fn classify_for_retry(error: &OllamaError) -> RetryCategory {
    match error {
        OllamaError::ToolCallError(e) => match e {
            ToolCallError::UnknownToolName
            | ToolCallError::InvalidToolArguments(_)
            | ToolCallError::InternalToolError(_) => RetryCategory::ImmediateRetry {
                max_attempts: MAX_TOOL_RETRIES,
            },
        },
        OllamaError::ReqwestError(_) => RetryCategory::NetworkRetry {
            max_attempts: MAX_NETWORK_RETRIES,
        },
        OllamaError::InternalError(_) => RetryCategory::ServerRetry {
            max_attempts: MAX_SERVER_RETRIES,
        },
        OllamaError::JsonError(_) => RetryCategory::NoRetry,
        OllamaError::Other(_) => RetryCategory::NoRetry,
    }
}

/// Calculate the backoff delay for a given retry category and attempt number.
///
/// `attempt` is 1-indexed: the first retry is attempt 1.
pub fn retry_delay(category: &RetryCategory, attempt: usize) -> Duration {
    match category {
        RetryCategory::ImmediateRetry { .. } => Duration::ZERO,
        RetryCategory::NetworkRetry { .. } => {
            // Exponential: 100ms, 200ms, 400ms, 800ms, 1.6s, capped at 1.6s
            let shift = attempt.saturating_sub(1).min(4) as u32;
            let ms = NETWORK_BACKOFF_BASE_MS.saturating_mul(1u64 << shift);
            Duration::from_millis(ms.min(NETWORK_BACKOFF_MAX_MS))
        }
        RetryCategory::ServerRetry { .. } => {
            // Linear: 5s × attempt → 5s, 10s, 15s
            Duration::from_secs(SERVER_BACKOFF_BASE_SECS * attempt as u64)
        }
        RetryCategory::RateLimitRetry { retry_after, .. } => {
            // Use Retry-After header value if present, else 2s default
            retry_after.unwrap_or(Duration::from_secs(RATELIMIT_DEFAULT_BACKOFF_SECS))
        }
        RetryCategory::NoRetry => Duration::ZERO,
    }
}

/// Sleep for the given duration, aborting early if `cancel_token` fires.
///
/// This is the **cancel-aware sleep** that ensures Ctrl+C during a 15s
/// server-retry backoff aborts immediately instead of waiting for the timer.
///
/// If `cancel_token` is `None`, behaves like a regular `tokio::time::sleep`.
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
    use ollama_rs::error::InternalOllamaError;

    // --- classify_for_retry ---

    #[test]
    fn test_classify_server_error() {
        let err = OllamaError::InternalError(InternalOllamaError {
            message: "500 Internal Server Error".to_string(),
        });
        let cat = classify_for_retry(&err);
        assert!(matches!(
            cat,
            RetryCategory::ServerRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_classify_network_error() {
        // We cannot easily construct a real reqwest::Error in a unit test
        // (build() returns Ok for valid URLs). The classification code
        // matches OllamaError::ReqwestError(_) → NetworkRetry unconditionally.
        // This test documents the intent: any reqwest error should classify
        // as NetworkRetry when we can construct one.
        // Integration: see manual tests with `kill ollama` then `sprach query`.
    }

    #[test]
    fn test_classify_tool_error_internal() {
        // InternalToolError is a box<dyn Error>, hard to construct in test.
        // The classification code matches all ToolCallError variants → ImmediateRetry.
        // We verify via the easier-to-construct UnknownToolName.
        let err = OllamaError::ToolCallError(ToolCallError::UnknownToolName);
        let cat = classify_for_retry(&err);
        assert!(matches!(
            cat,
            RetryCategory::ImmediateRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_classify_tool_error_unknown() {
        let err = OllamaError::ToolCallError(ToolCallError::UnknownToolName);
        let cat = classify_for_retry(&err);
        assert!(matches!(
            cat,
            RetryCategory::ImmediateRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_classify_tool_error_invalid_args() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = OllamaError::ToolCallError(ToolCallError::InvalidToolArguments(json_err));
        let cat = classify_for_retry(&err);
        assert!(matches!(
            cat,
            RetryCategory::ImmediateRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_classify_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = OllamaError::JsonError(json_err);
        let cat = classify_for_retry(&err);
        assert!(matches!(cat, RetryCategory::NoRetry));
    }

    #[test]
    fn test_classify_other_error() {
        let err = OllamaError::Other("cancelled by user".to_string());
        let cat = classify_for_retry(&err);
        assert!(matches!(cat, RetryCategory::NoRetry));
    }

    // --- retry_delay ---

    #[test]
    fn test_retry_delay_server_attempt_1() {
        let cat = RetryCategory::ServerRetry { max_attempts: 3 };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(5));
    }

    #[test]
    fn test_retry_delay_server_attempt_2() {
        let cat = RetryCategory::ServerRetry { max_attempts: 3 };
        assert_eq!(retry_delay(&cat, 2), Duration::from_secs(10));
    }

    #[test]
    fn test_retry_delay_server_attempt_3() {
        let cat = RetryCategory::ServerRetry { max_attempts: 3 };
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
        assert_eq!(retry_delay(&cat, 2), Duration::ZERO);
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

    // --- max_attempts ---

    #[test]
    fn test_max_attempts_per_category() {
        assert_eq!(
            RetryCategory::ServerRetry { max_attempts: 3 }.max_attempts(),
            3
        );
        assert_eq!(
            RetryCategory::NetworkRetry { max_attempts: 5 }.max_attempts(),
            5
        );
        assert_eq!(
            RetryCategory::ImmediateRetry { max_attempts: 3 }.max_attempts(),
            3
        );
        assert_eq!(
            RetryCategory::RateLimitRetry {
                max_attempts: 3,
                retry_after: None
            }
            .max_attempts(),
            3
        );
        assert_eq!(RetryCategory::NoRetry.max_attempts(), 0);
    }

    // --- is_retryable ---

    #[test]
    fn test_is_retryable() {
        assert!(!RetryCategory::NoRetry.is_retryable());
        assert!(RetryCategory::ServerRetry { max_attempts: 3 }.is_retryable());
        assert!(RetryCategory::NetworkRetry { max_attempts: 5 }.is_retryable());
        assert!(RetryCategory::ImmediateRetry { max_attempts: 3 }.is_retryable());
        assert!(
            RetryCategory::RateLimitRetry {
                max_attempts: 3,
                retry_after: None
            }
            .is_retryable()
        );
    }

    // --- retry_after field is set but never read (W2 dead_code policy) ---

    #[test]
    fn test_retry_after_field_is_unused_until_122() {
        // This test documents that the field is set but not read yet.
        // The dead_code policy in W2 allows this as long as it is
        // resolved by #122 (Retry-After header parsing).
        let cat = RetryCategory::RateLimitRetry {
            max_attempts: 3,
            retry_after: Some(Duration::from_secs(5)),
        };
        // Field exists, can be constructed and inspected:
        if let RetryCategory::RateLimitRetry { retry_after, .. } = cat {
            assert_eq!(retry_after, Some(Duration::from_secs(5)));
        } else {
            panic!("Expected RateLimitRetry");
        }
    }

    // --- sleep_or_cancel ---

    #[tokio::test]
    async fn test_sleep_or_cancel_zero_delay() {
        // Zero delay returns true immediately
        let result = sleep_or_cancel(Duration::ZERO, None).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_sleep_or_cancel_no_token() {
        // Without a token, sleep completes normally
        let start = std::time::Instant::now();
        let result = sleep_or_cancel(Duration::from_millis(10), None).await;
        let elapsed = start.elapsed();
        assert!(result);
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_sleep_or_cancel_cancelled() {
        // With a cancelled token, sleep returns false immediately
        let token = CancellationToken::new();
        token.cancel();
        let start = std::time::Instant::now();
        let result = sleep_or_cancel(Duration::from_secs(10), Some(&token)).await;
        let elapsed = start.elapsed();
        assert!(!result);
        // Should return in milliseconds, not 10 seconds
        assert!(elapsed < Duration::from_secs(1));
    }
}
