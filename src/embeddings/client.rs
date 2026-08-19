//! Embedding client for the LLM server API
//!
//! Generates embeddings via `LlmProvider::embed()`.
//! Compatible with any provider that implements the OpenAI-spec
//! `/v1/embeddings` endpoint.
//!
//! `EmbeddingClient` does not carry a hardcoded model name. The model
//! is supplied explicitly via [`EmbeddingClient::with_model`] and
//! resolved from `[indexing].model` in `config.toml`.

use std::time::Duration;

use tokio::sync::{OnceCell, Semaphore};

use super::vector::{TruncateResult, truncate_and_normalize_with_correction};
use crate::provider::LlmProvider;

/// Default context length when model info is unavailable.
pub const DEFAULT_CONTEXT_LENGTH: usize = 512;

const EMBEDDING_TIMEOUT_SECS: u64 = 30;

/// Client for generating embeddings via LlmProvider.
///
/// Holds an `Ollama` (the shim) which implements both the
/// `crate::provider::OpenAICompatibleProvider` API (for backward compat) and `LlmProvider`
/// (via internal delegation to `OpenAICompatibleProvider`).
pub struct EmbeddingClient {
    provider: crate::provider::OpenAICompatibleProvider,
    model: String,
    /// Output dimension of the embedding model (from the alias's
    /// `dimensions = N` in models.toml). Used for vector store
    /// sizing and probe verification.
    dimensions: u32,
    /// Prefix prepended to each text before embedding (from
    /// `[indexing].prefix` in `config.toml`). Empty string = no
    /// prefix.
    prefix: String,
    /// Context length for the embedding model (from `num_ctx` on
    /// the model alias in `models.toml`). `None` = fallback to
    /// `DEFAULT_CONTEXT_LENGTH` (512).
    context_length: Option<u32>,
    /// Cached context length to avoid repeated API calls.
    cached_context_length: OnceCell<usize>,
    semaphore: Semaphore,
}

impl EmbeddingClient {
    /// Create a new embedding client with the given provider, model
    /// name, and output dimensions.
    ///
    /// `dimensions` is mandatory. The caller must resolve the alias
    /// via `Settings::resolve_indexing_model()` and pass the resulting
    /// `dimensions` value. There is no sensible default — silent
    /// assumptions about dimensions would mask user configuration
    /// errors.
    pub fn with_model(
        provider: crate::provider::OpenAICompatibleProvider,
        model: String,
        dimensions: u32,
        prefix: String,
        context_length: Option<u32>,
    ) -> Self {
        Self {
            provider,
            model,
            dimensions,
            prefix,
            context_length,
            cached_context_length: OnceCell::new(),
            semaphore: Semaphore::new(1),
        }
    }

    /// Get the context length for the embedding model.
    ///
    /// Returns the `num_ctx` from the model alias in `models.toml`,
    /// or `DEFAULT_CONTEXT_LENGTH` (512) if not configured.
    pub async fn get_context_length(&self) -> Result<usize, EmbeddingError> {
        if let Some(&ctx) = self.cached_context_length.get() {
            return Ok(ctx);
        }
        let context_length = self
            .context_length
            .map(|c| c as usize)
            .unwrap_or(DEFAULT_CONTEXT_LENGTH);
        let _ = self.cached_context_length.set(context_length);
        Ok(context_length)
    }

    /// Check if an API error indicates context length exceeded.
    pub fn is_context_exceeded(error: &str) -> bool {
        let error_lower = error.to_lowercase();
        error_lower.contains("context_length")
            || error_lower.contains("context length")
            || error_lower.contains("maximum context")
            || error_lower.contains("token limit")
            || error_lower.contains("sequence length")
            || error_lower.contains("batch size")
            || error_lower.contains("too large to process")
            || error_lower.contains("too long")
    }

    /// Get the configured context length for this embedding model.
    ///
    /// Returns the `num_ctx` from the model alias in `models.toml`,
    /// or `DEFAULT_CONTEXT_LENGTH` (512) if not configured.
    pub fn context_length(&self) -> usize {
        self.context_length
            .map(|c| c as usize)
            .unwrap_or(DEFAULT_CONTEXT_LENGTH)
    }

    /// Generate embedding for a single text.
    ///
    /// Sends the text to the embedding server without any proactive
    /// context-length estimation. If the server returns a
    /// context-exceeded error, it is converted to
    /// `EmbeddingError::ContextExceeded` and handled by the fallback
    /// chunking pipeline in `fallback.rs`.
    pub async fn embed(&self, text: &str) -> Result<TruncateResult, EmbeddingError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| EmbeddingError::ApiError("Semaphore closed".to_string()))?;

        let prefixed_text = if self.prefix.is_empty() {
            text.to_string()
        } else {
            format!("{}{}", self.prefix, text)
        };

        let result = tokio::time::timeout(
            Duration::from_secs(EMBEDDING_TIMEOUT_SECS),
            self.provider
                .embed(&prefixed_text, &self.model, Some(self.dimensions as usize)),
        )
        .await;

        let embedding = match result {
            Ok(Ok(vec)) => vec,
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                if Self::is_context_exceeded(&error_msg) {
                    return Err(EmbeddingError::ContextExceeded { message: error_msg });
                }
                return Err(EmbeddingError::ApiError(error_msg));
            }
            Err(_) => {
                return Err(EmbeddingError::Timeout {
                    duration_secs: EMBEDDING_TIMEOUT_SECS,
                });
            }
        };

        // Validate against the alias's declared dimensions (not the
        // hardcoded FULL_DIMENSIONS constant). The startup probe
        // already verified the server returns this exact dim count.
        if embedding.len() as u32 != self.dimensions {
            return Err(EmbeddingError::InvalidDimensions {
                expected: self.dimensions as usize,
                got: embedding.len(),
            });
        }

        // Truncate (if needed) and normalize with norm correction.
        // When the server returns exactly self.dimensions dims
        // (normal case with server-side Matryoshka), truncation is
        // identity — just normalizes. When the server returns more
        // dims (fallback for non-Matryoshka servers), truncates to
        // self.dimensions.
        Ok(truncate_and_normalize_with_correction(
            &embedding,
            self.dimensions as usize,
        ))
    }

    /// Generate embeddings for multiple texts in batch
    #[allow(dead_code)]
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<TruncateResult>, EmbeddingError> {
        let mut results = Vec::new();
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    /// Get the model name
    #[allow(dead_code)]
    pub fn model(&self) -> &str {
        &self.model
    }
}

/// Errors from embedding generation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EmbeddingError {
    /// API call failed
    ApiError(String),
    /// No embedding returned
    NoEmbedding,
    /// Invalid embedding dimensions
    InvalidDimensions { expected: usize, got: usize },
    /// Content exceeds model's context window
    ContextExceeded { message: String },
    /// Embedding request timed out
    Timeout { duration_secs: u64 },
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(msg) => write!(f, "Embedding API error: {}", msg),
            Self::NoEmbedding => write!(f, "No embedding returned from API"),
            Self::InvalidDimensions { expected, got } => {
                write!(
                    f,
                    "Invalid embedding dimensions: expected {}, got {}",
                    expected, got
                )
            }
            Self::ContextExceeded { message } => {
                write!(f, "Content exceeds context limit: {}", message)
            }
            Self::Timeout { duration_secs } => {
                write!(
                    f,
                    "Embedding request timed out after {} seconds",
                    duration_secs
                )
            }
        }
    }
}

impl std::error::Error for EmbeddingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_provider() -> crate::provider::OpenAICompatibleProvider {
        crate::provider::OpenAICompatibleProvider::new_local("http://localhost".to_string(), 11434)
    }

    #[test]
    fn test_full_dimensions() {
        use crate::embeddings::vector::FULL_DIMENSIONS;
        assert_eq!(FULL_DIMENSIONS, 768);
    }

    #[test]
    fn test_is_context_exceeded() {
        assert!(EmbeddingClient::is_context_exceeded(
            "context_length exceeded"
        ));
        assert!(EmbeddingClient::is_context_exceeded(
            "maximum context length exceeded"
        ));
        assert!(EmbeddingClient::is_context_exceeded("token limit exceeded"));
        assert!(EmbeddingClient::is_context_exceeded(
            "sequence length exceeded"
        ));
        // BeeLama/llama.cpp batch size error
        assert!(EmbeddingClient::is_context_exceeded(
            "input (544 tokens) is too large to process. increase the physical batch size (current batch size: 512)"
        ));
        assert!(EmbeddingClient::is_context_exceeded(
            "input is too large to process"
        ));
        assert!(EmbeddingClient::is_context_exceeded("sequence is too long"));
        assert!(!EmbeddingClient::is_context_exceeded("connection refused"));
        assert!(!EmbeddingClient::is_context_exceeded("network error"));
    }

    #[test]
    fn test_context_length_method() {
        let client_with_ctx = EmbeddingClient::with_model(
            make_dummy_provider(),
            "test".to_string(),
            768,
            "search_document: ".to_string(),
            Some(8192),
        );
        assert_eq!(client_with_ctx.context_length(), 8192);

        let client_without_ctx = EmbeddingClient::with_model(
            make_dummy_provider(),
            "test".to_string(),
            768,
            "search_document: ".to_string(),
            None,
        );
        assert_eq!(client_without_ctx.context_length(), DEFAULT_CONTEXT_LENGTH);
    }

    #[test]
    fn test_with_model_constructor() {
        let _client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "nomic-embed-text-v2-moe".to_string(),
            768,
            "search_document: ".to_string(),
            None,
        );
    }

    #[test]
    fn test_with_model_stores_model_name() {
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "bge-small-en-v1.5".to_string(),
            768,
            "search_document: ".to_string(),
            None,
        );
        assert_eq!(client.model(), "bge-small-en-v1.5");
        assert_eq!(client.dimensions, 768);
    }

    #[test]
    fn test_with_model_stores_prefix_and_context() {
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "bge-small-en-v1.5".to_string(),
            768,
            "".to_string(),
            Some(8192),
        );
        assert_eq!(client.prefix, "");
        assert_eq!(client.context_length, Some(8192));
    }

    #[test]
    fn test_with_model_default_prefix() {
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "nomic-embed-text-v2-moe".to_string(),
            256,
            "search_document: ".to_string(),
            None,
        );
        assert_eq!(client.prefix, "search_document: ");
        assert_eq!(client.context_length, None);
    }
}
