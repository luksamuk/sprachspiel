//! Model capability detection via the LLM server API
//!
//! Capabilities (tools, vision, thinking) are detected at runtime
//! by querying the server's model info endpoint.
//!
//! # W2 Wave Context (Issue #116)
//!
//! `check_server_health()` is the entry point for the startup health check.
//! It calls `ollama.list_local_models()` (which hits `/api/tags`) with a
//! 3-second timeout. This is a **pre-check** that catches "Ollama is not
//! running" before the heavier `show_model_info()` call would hang
//! indefinitely. When #120 (OllamaProvider reqwest direct) lands, this
//! will be replaced by a `ProviderError`-aware health check.

#![expect(clippy::print_stderr)] // Model capability detection output
use std::time::Duration;

use crate::provider::Ollama;
use ollama_rs::models::ModelInfo;

/// Maximum time to wait for the Ollama server to respond to a health check.
///
/// Tuned for the "Ollama is not running" case: localhost connection
/// refused returns in milliseconds, so a 3s timeout is more than enough.
/// Network issues (firewall, DNS) will hit this timeout and abort cleanly.
pub const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

/// Check whether the Ollama server is reachable and responsive.
///
/// Hits the `/api/tags` endpoint via `ollama.list_local_models()` with a
/// 3-second timeout. Returns `Ok(())` if the server responds (even with
/// zero models), `Err` with a user-friendly message if it doesn't.
///
/// **W2 Wave Context:** This is a small, standalone fix for the startup
/// hang reported during #116 manual testing (Scenario 2). The hang happens
/// because `ollama-rs` does not expose a configurable request timeout, so
/// when the server is unreachable, the HTTP request hangs indefinitely.
/// This health check with explicit timeout is the minimum-viable fix until
/// #120 replaces `ollama-rs` with direct reqwest.
#[allow(dead_code)] // Called from repl.rs startup in this same PR (#116)
pub async fn check_server_health(ollama: &crate::provider::Ollama) -> crate::AppResult<()> {
    let check = async {
        ollama.list_local_models().await.map_err(|e| {
            format!(
                "Failed to reach Ollama server: {e}. \
                 Make sure Ollama is running (try `ollama serve` in another terminal)."
            )
        })?;
        Ok::<(), String>(())
    };

    match tokio::time::timeout(HEALTH_CHECK_TIMEOUT, check).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e.into()),
        Err(_elapsed) => Err(format!(
            "Ollama server did not respond within {}s at the configured URL. \
             Make sure Ollama is running and accessible.",
            HEALTH_CHECK_TIMEOUT.as_secs()
        )
        .into()),
    }
}

/// Detected capabilities for a specific model
#[derive(Debug, Clone)]
pub struct ModelCapabilities {
    pub tools: bool,
    pub vision: bool,
    pub completion: bool,
    pub thinking: bool,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            tools: false,
            vision: false,
            completion: true,
            thinking: false,
        }
    }
}

impl ModelCapabilities {
    /// Detect model capabilities by querying the LLM server
    ///
    /// # Arguments
    /// * `ollama` - The LLM server client instance
    /// * `model_name` - The name of the model to check (e.g., "qwen3.5:4b")
    ///
    /// # Returns
    /// Detected capabilities for the model
    pub async fn detect(ollama: &crate::provider::Ollama, model_name: &str) -> crate::AppResult<Self> {
        let info: ModelInfo = ollama
            .show_model_info(model_name.to_string())
            .await
            .map_err(|e| format!("Failed to query model info: {}", e))?;

        Ok(Self {
            tools: info.capabilities.contains(&"tools".to_string()),
            vision: info.capabilities.contains(&"vision".to_string()),
            completion: info.capabilities.contains(&"completion".to_string()),
            thinking: info.capabilities.contains(&"thinking".to_string()),
        })
    }

    /// Detect model capabilities or return defaults on error
    ///
    /// Prints a warning on detection failure and returns default capabilities
    /// with completion enabled (safe fallback for most operations).
    pub async fn detect_or_default(ollama: &crate::provider::Ollama, model_name: &str) -> Self {
        match Self::detect(ollama, model_name).await {
            Ok(caps) => caps,
            Err(e) => {
                eprintln!("Warning: Could not detect model capabilities: {}", e);
                eprintln!("Continuing without capability detection...");
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_detection() {
        let caps = ModelCapabilities {
            tools: true,
            vision: false,
            completion: true,
            thinking: false,
        };

        assert!(caps.tools);
        assert!(!caps.vision);
        assert!(caps.completion);
        assert!(!caps.thinking);
    }

    #[test]
    fn test_health_check_timeout_constant() {
        // The 3s timeout is a deliberate UX trade-off. Make sure it
        // doesn't drift to a value that would make the startup too slow
        // or the timeout too tight.
        assert_eq!(HEALTH_CHECK_TIMEOUT, Duration::from_secs(3));
    }

    #[tokio::test]
    async fn test_health_check_returns_error_for_unreachable_server() {
        // Point at a port that nothing is listening on. The health check
        // should return an Err quickly (well under the 3s timeout).
        let ollama = Ollama::new("http://127.0.0.1", 1);
        let start = std::time::Instant::now();
        let result = check_server_health(&ollama).await;
        let elapsed = start.elapsed();
        assert!(result.is_err());
        // Should return fast (connection refused is instant), not after timeout
        assert!(elapsed < HEALTH_CHECK_TIMEOUT);
    }
}
