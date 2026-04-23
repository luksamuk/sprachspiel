//! Embedding client for Ollama API
//!
//! Generates embeddings using nomic-embed-text-v2-moe model.
//!
//! For content that exceeds the model's context window, use the `fallback` module
//! which provides `embed_chunk_with_fallback` and `embed_item_with_fallback`.

use ollama_rs::Ollama;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use ollama_rs::models::ModelInfo;
use tokio::sync::OnceCell;

use super::truncate::{FULL_DIMENSIONS, TRUNCATED_DIMENSIONS, truncate_and_normalize};

/// Default embedding model (nomic-embed-text-v2-moe)
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text-v2-moe:latest";

/// Default context length when model info is unavailable
/// Conservative value suitable for most embedding models.
/// Use this for spawned tasks that can't await context_length.
pub const DEFAULT_CONTEXT_LENGTH: usize = 512;

/// Characters per token ratio for estimating context overflow.
/// Conservative estimate: Portuguese/code averages ~3 chars/token.
const CHARS_PER_TOKEN: f32 = 3.0;

/// Prefix used for nomic-embed-text models.
/// Approximately 20 tokens ("search_document: " + space + overhead).
const EMBEDDING_PREFIX_TOKENS: usize = 20;

/// Safety margin as fraction of context length (10%).
/// Prevents borderline content from passing the check but failing at the API.
const CONTEXT_SAFETY_MARGIN: f32 = 0.10;

/// Client for generating embeddings via Ollama
pub struct EmbeddingClient {
    ollama: Ollama,
    model: String,
    /// Cached context length to avoid repeated API calls.
    /// Once set, the same value is used for the lifetime of the client.
    cached_context_length: OnceCell<usize>,
}

impl EmbeddingClient {
    /// Create a new embedding client with default model
    pub fn new(ollama: Ollama) -> Self {
        Self {
            ollama,
            model: DEFAULT_EMBEDDING_MODEL.to_string(),
            cached_context_length: OnceCell::new(),
        }
    }

    /// Create a new embedding client with custom model
    #[allow(dead_code)]
    pub fn with_model(ollama: Ollama, model: String) -> Self {
        Self {
            ollama,
            model,
            cached_context_length: OnceCell::new(),
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
    /// Uses CHARS_PER_TOKEN (3.0) which accounts for Portuguese/code content
    /// where tokenization is less efficient than pure English.
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
    /// time and can cause startup failures.
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
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
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

        // Add prefix for nomic-embed-text-v2-moe
        let prefixed_text = format!("search_document: {}", text);

        let request = GenerateEmbeddingsRequest::new(self.model.clone(), prefixed_text.into());

        let response = self
            .ollama
            .generate_embeddings(request)
            .await
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

        // Truncate and normalize
        Ok(truncate_and_normalize(&embedding))
    }

    /// Generate embeddings for multiple texts in batch
    ///
    /// More efficient than calling `embed()` multiple times.
    /// Uses "search_document: " prefix for each text.
    ///
    /// Future use: Bulk migration with `/migrate` for better performance
    /// when migrating large conversation histories.
    #[allow(dead_code)]
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
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

        // Truncate and normalize each embedding
        Ok(response
            .embeddings
            .into_iter()
            .map(|e| truncate_and_normalize(&e))
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
        // 512 context - 20 prefix - 51 margin = 441 available tokens ≈ 1323 chars
        // Create text longer than available context
        let long_text = "a".repeat(5000);
        assert!(EmbeddingClient::is_likely_context_exceeded(&long_text, 512));
    }

    #[test]
    fn test_is_likely_context_exceeded_edge_case() {
        // Text near the boundary should be detected
        // 512 - 20 - 51 = 441 tokens available ≈ 1323 chars
        // 1323 chars / 3.0 = 441 tokens, so 1400 chars should exceed
        let borderline_text = "x".repeat(1400);
        assert!(EmbeddingClient::is_likely_context_exceeded(
            &borderline_text,
            512
        ));
    }

    #[test]
    fn test_context_safety_margin_values() {
        // Verify constants are reasonable
        assert_eq!(EMBEDDING_PREFIX_TOKENS, 20);
        assert!((CONTEXT_SAFETY_MARGIN - 0.10).abs() < f32::EPSILON);
        assert!((CHARS_PER_TOKEN - 3.0).abs() < f32::EPSILON);
    }
}
