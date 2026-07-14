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

use super::truncate::{
    TRUNCATED_DIMENSIONS, TruncateResult, truncate_and_normalize_with_correction,
};
use crate::provider::LlmProvider;

/// Default context length when model info is unavailable.
pub const DEFAULT_CONTEXT_LENGTH: usize = 512;

const EMBEDDING_TIMEOUT_SECS: u64 = 30;

/// Characters per token ratio for estimating context overflow.
pub(crate) const CHARS_PER_TOKEN: f32 = 2.0;

/// Prefix used for nomic-embed-text models.
const EMBEDDING_PREFIX_TOKENS: usize = 30;

/// Safety margin as fraction of context length (20%).
const CONTEXT_SAFETY_MARGIN: f32 = 0.20;

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
    ) -> Self {
        Self {
            provider,
            model,
            dimensions,
            cached_context_length: OnceCell::new(),
            semaphore: Semaphore::new(1),
        }
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

    /// Get the context length for the embedding model.
    ///
    /// Uses `LlmProvider::embed()` semantics to derive context length.
    /// We use a known reasonable default (512) for now; future work can
    /// read this from `/v1/models` metadata.
    pub async fn get_context_length(&self) -> Result<usize, EmbeddingError> {
        if let Some(&ctx) = self.cached_context_length.get() {
            return Ok(ctx);
        }
        // Derive from capability detection. For now, use the
        // conservative default; can be enhanced to read from
        // /v1/models response metadata.
        let context_length = DEFAULT_CONTEXT_LENGTH;
        let _ = self.cached_context_length.set(context_length);
        Ok(context_length)
    }

    fn estimate_tokens(text: &str) -> usize {
        (text.len() as f32 / CHARS_PER_TOKEN).ceil() as usize
    }

    fn is_likely_context_exceeded(text: &str, context_length: usize) -> bool {
        let estimated_tokens = Self::estimate_tokens(text);
        let available = context_length
            .saturating_sub(EMBEDDING_PREFIX_TOKENS)
            .saturating_sub((context_length as f32 * CONTEXT_SAFETY_MARGIN) as usize);
        estimated_tokens > available
    }

    /// Generate embedding for a single text
    pub async fn embed(&self, text: &str) -> Result<TruncateResult, EmbeddingError> {
        let context_length = self
            .cached_context_length
            .get()
            .copied()
            .unwrap_or(DEFAULT_CONTEXT_LENGTH);

        if Self::is_likely_context_exceeded(text, context_length) {
            return Err(EmbeddingError::ContextExceeded {
                message: format!(
                    "Estimated {} tokens exceed available context ({} total - {} prefix - {} margin)",
                    Self::estimate_tokens(text),
                    context_length,
                    EMBEDDING_PREFIX_TOKENS,
                    (context_length as f32 * CONTEXT_SAFETY_MARGIN) as usize
                ),
            });
        }

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| EmbeddingError::ApiError("Semaphore closed".to_string()))?;

        let prefixed_text = format!("search_document: {}", text);

        // Pass the alias's declared `dimensions` (not the hardcoded
        // TRUNCATED_DIMENSIONS constant). The startup probe already
        // verified the server returns this exact dim count.
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

        // If the model returns more than the storage format
        // (TRUNCATED_DIMENSIONS = 256), apply Matryoshka
        // truncation for compact storage with norm correction.
        // If the alias's dimensions are <= TRUNCATED_DIMENSIONS,
        // store the full vector (no truncation needed).
        if (self.dimensions as usize) > TRUNCATED_DIMENSIONS {
            Ok(truncate_and_normalize_with_correction(&embedding))
        } else {
            // No truncation; just L2-normalize and compute norm
            // correction.
            use super::truncate::TruncateResult;
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < f32::EPSILON {
                Ok(TruncateResult {
                    vector: vec![0.0; embedding.len()],
                    norm_correction: 1.0,
                })
            } else {
                let vector: Vec<f32> = embedding.iter().map(|x| x / norm).collect();
                let norm_correction = 1.0 / (norm * norm);
                Ok(TruncateResult {
                    vector,
                    norm_correction,
                })
            }
        }
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

    /// Get the truncated embedding dimension
    #[allow(dead_code)]
    pub fn embedding_dimension() -> usize {
        TRUNCATED_DIMENSIONS
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
    fn test_embedding_dimension() {
        assert_eq!(TRUNCATED_DIMENSIONS, 256);
    }

    #[test]
    fn test_full_dimensions() {
        use crate::embeddings::truncate::FULL_DIMENSIONS;
        assert_eq!(FULL_DIMENSIONS, 768);
    }

    #[test]
    fn test_embedding_dimension_method() {
        assert_eq!(EmbeddingClient::embedding_dimension(), 256);
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
    fn test_estimate_tokens() {
        // CHARS_PER_TOKEN = 2.0
        assert_eq!(EmbeddingClient::estimate_tokens(""), 0);
        assert_eq!(EmbeddingClient::estimate_tokens("ab"), 1); // 2/2 = 1
        assert_eq!(EmbeddingClient::estimate_tokens("abcdefgh"), 4); // 8/2 = 4
        assert_eq!(EmbeddingClient::estimate_tokens("a"), 1); // 1/2 = 0.5 → ceil = 1
    }

    #[test]
    fn test_is_likely_context_exceeded_short_text() {
        let text = "Hello, world!";
        assert!(!EmbeddingClient::is_likely_context_exceeded(text, 512));
    }

    #[test]
    fn test_is_likely_context_exceeded_long_text() {
        let long_text = "a".repeat(5000);
        assert!(EmbeddingClient::is_likely_context_exceeded(&long_text, 512));
    }

    #[test]
    fn test_context_safety_margin_values() {
        assert_eq!(EMBEDDING_PREFIX_TOKENS, 30);
        assert!((CONTEXT_SAFETY_MARGIN - 0.20).abs() < f32::EPSILON);
        assert!((CHARS_PER_TOKEN - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_with_model_constructor() {
        // with_model also takes `dimensions`.
        let _client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "nomic-embed-text-v2-moe".to_string(),
            768,
        );
    }

    #[test]
    fn test_with_model_stores_model_name() {
        let client =
            EmbeddingClient::with_model(make_dummy_provider(), "bge-small-en-v1.5".to_string(), 768);
        assert_eq!(client.model(), "bge-small-en-v1.5");
        assert_eq!(client.dimensions, 768);
    }
}
