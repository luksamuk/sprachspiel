//! Embedding client for the LLM server API
//!
//! Generates embeddings using nomic-embed-text-v2-moe model.
//!
//! # Concurrency
//!
//! All embedding requests are serialized through an internal semaphore
//! (max concurrency = 1). This prevents overwhelming the LLM server
//! when multiple embedding requests arrive simultaneously (e.g., fact
//! insertion + message embedding + document indexing). Without serialization,
//! concurrent requests can cause timeouts, model loading conflicts, or
//! silent failures that leave `has_embedding = 0` in the database.
//!
//! # Timeout
//!
//! Each embedding request has a 30-second timeout. If Ollama is loading
//! a model or is otherwise slow, the request will fail with
//! `EmbeddingError::Timeout` rather than hanging indefinitely. Failed
//! requests are recovered on the next startup via
//! `recover_missing_embeddings()` / `recover_missing_fact_embeddings()`.

use std::time::Duration;

use ollama_rs::Ollama;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use ollama_rs::models::ModelInfo;
use tokio::sync::{OnceCell, Semaphore};

use super::truncate::{
    FULL_DIMENSIONS, TRUNCATED_DIMENSIONS, TruncateResult,
    truncate_and_normalize_with_correction,
};

/// Default embedding model (nomic-embed-text-v2-moe)
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text-v2-moe:latest";

/// Default context length when model info is unavailable
/// Conservative value suitable for most embedding models.
/// Use this for spawned tasks that can't await context_length.
pub const DEFAULT_CONTEXT_LENGTH: usize = 512;

/// Timeout for individual embedding requests (30 seconds).
///
/// Ollama may need to load the embedding model on first use, which can take
/// several seconds. 30 seconds provides a generous window while still
/// preventing indefinite hangs when the server is unresponsive.
const EMBEDDING_TIMEOUT_SECS: u64 = 30;

/// Characters per token ratio for estimating context overflow.
///
/// This is an ESTIMATE, not an exact count. The Ollama embedding API returns
/// `prompt_eval_count` (exact token count) in its JSON response, but the
/// `ollama-rs` library v0.3.4 does not capture this field in
/// `GenerateEmbeddingsResponse`. When we implement provider abstraction with
/// direct reqwest calls (see GitHub issue for multi-provider embedding support),
/// we can use exact token counts and remove this estimate.
///
/// Portuguese/code content averages ~3 chars/token (lower than English ~4)
/// because of diacritics and special characters. This conservative ratio
/// ensures we overestimate rather than underestimate.
///
/// See: https://github.com/ollama/ollama/blob/main/docs/api.md
/// The embeddings endpoint returns `prompt_eval_count` but ollama-rs ignores it.
const CHARS_PER_TOKEN: f32 = 3.0;

/// Prefix used for nomic-embed-text models.
/// Approximately 30 tokens ("search_document: " + space + overhead + margin).
/// Increased from 20 to account for tokenizer overhead and prefix formatting.
const EMBEDDING_PREFIX_TOKENS: usize = 30;

/// Safety margin as fraction of context length (20%).
/// Prevents borderline content from passing the check but failing at the API.
/// Increased from 10% to 20% to account for estimation imprecision — since
/// we estimate tokens via chars/3.0 rather than exact tokenization, a wider
/// margin avoids false negatives where estimated tokens fit but actual tokens
/// exceed the context window.
const CONTEXT_SAFETY_MARGIN: f32 = 0.20;

/// Client for generating embeddings via Ollama
///
/// Wraps the Ollama API with concurrency control (serialized requests via
/// an internal semaphore) and a request timeout to prevent indefinite hangs.
pub struct EmbeddingClient {
    ollama: Ollama,
    model: String,
    /// Cached context length to avoid repeated API calls.
    /// Once set, the same value is used for the lifetime of the client.
    cached_context_length: OnceCell<usize>,
    /// Semaphore controlling concurrency for embedding requests.
    /// Max permits = 1 means requests are serialized: only one embedding
    /// request is sent to Ollama at a time. This prevents model loading
    /// conflicts, timeouts, and silent failures when multiple callers
    /// (message embedding, fact embedding, document indexing) request
    /// embeddings concurrently.
    semaphore: Semaphore,
}

impl EmbeddingClient {
    /// Create a new embedding client with default model
    pub fn new(ollama: Ollama) -> Self {
        Self {
            ollama,
            model: DEFAULT_EMBEDDING_MODEL.to_string(),
            cached_context_length: OnceCell::new(),
            semaphore: Semaphore::new(1),
        }
    }

    /// Create a new embedding client with custom model
    #[allow(dead_code)]
    pub fn with_model(ollama: Ollama, model: String) -> Self {
        Self {
            ollama,
            model,
            cached_context_length: OnceCell::new(),
            semaphore: Semaphore::new(1),
        }
    }

    /// Check if an API error indicates context length exceeded.
    ///
    /// Ollama returns various error messages for context overflow:
    /// - "context_length_exceeded"
    /// - "maximum context length"
    /// - "token limit"
    /// - "sequence length"
    pub fn is_context_exceeded(error: &str) -> bool {
        let error_lower = error.to_lowercase();
        error_lower.contains("context_length")
            || error_lower.contains("context length")
            || error_lower.contains("maximum context")
            || error_lower.contains("token limit")
            || error_lower.contains("sequence length")
    }

    /// Get the context length for the embedding model from Ollama API.
    ///
    /// Queries the model info and extracts the context_length from the
    /// model_info field (e.g., "nomic-bert-moe.context_length": 512).
    ///
    /// Results are cached — subsequent calls return the cached value
    /// without querying the API again.
    ///
    /// Returns DEFAULT_CONTEXT_LENGTH (512) if unable to determine.
    pub async fn get_context_length(&self) -> Result<usize, EmbeddingError> {
        if let Some(&ctx) = self.cached_context_length.get() {
            return Ok(ctx);
        }

        let context_length = self.fetch_context_length().await?;
        // Cache the result — all subsequent calls use the cached value.
        // OnceCell::set returns Ok(()) on first set, Err if already set.
        let _ = self.cached_context_length.set(context_length);
        Ok(context_length)
    }

    /// Fetch context length from Ollama API (internal helper).
    async fn fetch_context_length(&self) -> Result<usize, EmbeddingError> {
        let info: ModelInfo = self
            .ollama
            .show_model_info(self.model.clone())
            .await
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        // Look for "*.context_length" in model_info
        for (key, value) in info.model_info.iter() {
            if key.ends_with(".context_length")
                && let Some(ctx) = value.as_u64()
            {
                return Ok(ctx as usize);
            }
        }

        // Fallback to conservative default
        Ok(DEFAULT_CONTEXT_LENGTH)
    }

    /// Estimate the number of tokens in a text using conservative ratio.
    ///
    /// # Why Estimate Instead of Exact Count
    ///
    /// The Ollama `/api/embeddings` endpoint returns `prompt_eval_count` in its
    /// response, which is the exact token count. However, the `ollama-rs` crate
    /// v0.3.4 only deserializes the `embeddings` field and discards this value.
    /// Until we implement direct API calls (planned as part of provider abstraction),
    /// we must estimate token counts proactively to avoid making API calls that
    /// will fail due to context overflow.
    ///
    /// This estimate is intentionally conservative — it overestimates token count
    /// to ensure borderline content is caught before wasting an API call.
    fn estimate_tokens(text: &str) -> usize {
        (text.len() as f32 / CHARS_PER_TOKEN).ceil() as usize
    }

    /// Check if text is likely to exceed the model's context window.
    ///
    /// Proactively estimates token count including the "search_document: " prefix
    /// and a safety margin. Returns true if the estimated tokens exceed the
    /// available context.
    ///
    /// This avoids making an API call only to receive an error, which wastes
    /// time and can cause startup failures. The 20% safety margin accounts for
    /// the imprecision of character-based token estimation.
    fn is_likely_context_exceeded(text: &str, context_length: usize) -> bool {
        let estimated_tokens = Self::estimate_tokens(text);
        // Available context = total - prefix overhead - safety margin
        let available = context_length
            .saturating_sub(EMBEDDING_PREFIX_TOKENS)
            .saturating_sub((context_length as f32 * CONTEXT_SAFETY_MARGIN) as usize);
        estimated_tokens > available
    }

    /// Generate embedding for a single text
    ///
    /// Uses the prefix "search_document: " for nomic-embed-text-v2-moe model.
    /// Truncates to 256 dimensions and normalizes.
    ///
    /// # Concurrency
    ///
    /// Requests are serialized through an internal semaphore (max concurrency = 1)
    /// to prevent overwhelming the Ollama server. If Ollama is loading a model
    /// or processing another request, this call will wait for its turn.
    ///
    /// # Timeout
    ///
    /// Each request has a 30-second timeout. If the server is unresponsive or
    /// the model is still loading, the request will fail with
    /// `EmbeddingError::Timeout`.
    ///
    /// # Proactive Context Check
    ///
    /// Before sending to the API, estimates whether the text exceeds the model's
    /// context window. If so, returns `EmbeddingError::ContextExceeded` immediately
    /// without making an API call. This prevents startup failures when content
    /// is too large for the embedding model.
    ///
    /// # Errors
    ///
    /// Returns `EmbeddingError::ContextExceeded` if the text exceeds the model's
    /// context window. Use the `fallback` module's `embed_chunk_with_fallback`
    /// for automatic chunking when context is exceeded.
    pub async fn embed(&self, text: &str) -> Result<TruncateResult, EmbeddingError> {
        // Proactive check: estimate if content exceeds context window
        // Use cached context length if available, otherwise use conservative default
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

        // Acquire semaphore permit — serializes all embedding requests
        // so only one request is sent to Ollama at a time.
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| EmbeddingError::ApiError("Semaphore closed".to_string()))?;

        // Add prefix for nomic-embed-text-v2-moe
        let prefixed_text = format!("search_document: {}", text);

        let request = GenerateEmbeddingsRequest::new(self.model.clone(), prefixed_text.into());

        // Send request with timeout to prevent indefinite hangs
        let response = tokio::time::timeout(
            Duration::from_secs(EMBEDDING_TIMEOUT_SECS),
            self.ollama.generate_embeddings(request),
        )
        .await
        .map_err(|_| EmbeddingError::Timeout {
            duration_secs: EMBEDDING_TIMEOUT_SECS,
        })?
        .map_err(|e| {
            // Check if the API error is a context exceeded error
            let error_msg = e.to_string();
            if Self::is_context_exceeded(&error_msg) {
                EmbeddingError::ContextExceeded { message: error_msg }
            } else {
                EmbeddingError::ApiError(error_msg)
            }
        })?;

        let embedding = response
            .embeddings
            .into_iter()
            .next()
            .ok_or(EmbeddingError::NoEmbedding)?;

        // Validate dimensions
        if embedding.len() != FULL_DIMENSIONS {
            return Err(EmbeddingError::InvalidDimensions {
                expected: FULL_DIMENSIONS,
                got: embedding.len(),
            });
        }

        // Truncate and normalize with norm correction
        Ok(truncate_and_normalize_with_correction(&embedding))
    }

    /// Generate embeddings for multiple texts in batch
    ///
    /// More efficient than calling `embed()` multiple times.
    /// Uses "search_document: " prefix for each text.
    ///
    /// Future use: Bulk migration with `/migrate` for better performance
    /// when migrating large conversation histories.
    #[allow(dead_code)]
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<TruncateResult>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Add prefix for each text
        let prefixed_texts: Vec<String> = texts
            .iter()
            .map(|t| format!("search_document: {}", t))
            .collect();

        let request = GenerateEmbeddingsRequest::new(self.model.clone(), prefixed_texts.into());

        let response = self
            .ollama
            .generate_embeddings(request)
            .await
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        // Truncate and normalize each embedding with norm correction
        Ok(response
            .embeddings
            .into_iter()
            .map(|e| truncate_and_normalize_with_correction(&e))
            .collect())
    }

    /// Get the model name
    #[allow(dead_code)]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the truncated embedding dimension
    ///
    /// Useful for validating embedding sizes and dimension checks.
    /// Currently used by tests only.
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

    #[test]
    fn test_embedding_dimension() {
        assert_eq!(TRUNCATED_DIMENSIONS, 256);
    }

    #[test]
    fn test_full_dimensions() {
        assert_eq!(FULL_DIMENSIONS, 768);
    }

    #[test]
    fn test_default_model() {
        assert_eq!(DEFAULT_EMBEDDING_MODEL, "nomic-embed-text-v2-moe:latest");
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
        assert!(!EmbeddingClient::is_context_exceeded("connection refused"));
        assert!(!EmbeddingClient::is_context_exceeded("network error"));
    }

    #[test]
    fn test_estimate_tokens() {
        // Conservative estimate: 3 chars/token
        assert_eq!(EmbeddingClient::estimate_tokens(""), 0);
        assert_eq!(EmbeddingClient::estimate_tokens("abc"), 1); // 3/3 = 1.0 → 1
        assert_eq!(EmbeddingClient::estimate_tokens("abcdefgh"), 3); // 8/3 = 2.67 → 3
        assert_eq!(EmbeddingClient::estimate_tokens("a"), 1); // 1/3 = 0.33 → 1
    }

    #[test]
    fn test_is_likely_context_exceeded_short_text() {
        // Short text should NOT exceed context
        let text = "Hello, world!";
        assert!(!EmbeddingClient::is_likely_context_exceeded(text, 512));
    }

    #[test]
    fn test_is_likely_context_exceeded_long_text() {
        // Very long text SHOULD exceed context
        // 512 context - 30 prefix - 102 margin (20%) = 380 available tokens ≈ 1140 chars
        // Create text longer than available context
        let long_text = "a".repeat(5000);
        assert!(EmbeddingClient::is_likely_context_exceeded(&long_text, 512));
    }

    #[test]
    fn test_is_likely_context_exceeded_edge_case() {
        // Text near the boundary should be detected
        // 512 - 30 - 102 = 380 tokens available ≈ 1140 chars
        // 1200 chars / 3.0 = 400 estimated tokens, which exceeds 380 available
        let borderline_text = "x".repeat(1200);
        assert!(EmbeddingClient::is_likely_context_exceeded(
            &borderline_text,
            512
        ));
    }

    #[test]
    fn test_context_safety_margin_values() {
        // Verify constants are reasonable
        assert_eq!(EMBEDDING_PREFIX_TOKENS, 30);
        assert!((CONTEXT_SAFETY_MARGIN - 0.20).abs() < f32::EPSILON);
        assert!((CHARS_PER_TOKEN - 3.0).abs() < f32::EPSILON);
    }
}
