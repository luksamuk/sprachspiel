//! Embedding client for Ollama API
//!
//! Generates embeddings using nomic-embed-text-v2-moe model.

use ollama_rs::Ollama;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use ollama_rs::models::ModelInfo;

use super::chunk_config::DynamicChunkConfig;
use super::chunker::{chunk_text_with_config, ChunkConfig};
use super::truncate::{FULL_DIMENSIONS, TRUNCATED_DIMENSIONS, truncate_and_normalize};

/// Type alias for boxed future returned by embed_with_fallback_inner
type EmbedFallbackFuture<'a> = std::pin::Pin<std::boxed::Box<dyn std::future::Future<Output = Result<Vec<Vec<f32>>, EmbeddingError>> + Send + 'a>>;

/// Default embedding model (nomic-embed-text-v2-moe)
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text-v2-moe:latest";

/// Default context length when model info is unavailable
/// Conservative value suitable for most embedding models
const DEFAULT_CONTEXT_LENGTH: usize = 512;

/// Client for generating embeddings via Ollama
pub struct EmbeddingClient {
    ollama: Ollama,
    model: String,
}

impl EmbeddingClient {
    /// Create a new embedding client with default model
    pub fn new(ollama: Ollama) -> Self {
        Self {
            ollama,
            model: DEFAULT_EMBEDDING_MODEL.to_string(),
        }
    }

    /// Create a new embedding client with custom model
    #[allow(dead_code)]
    pub fn with_model(ollama: Ollama, model: String) -> Self {
        Self { ollama, model }
    }

    /// Check if an API error indicates context length exceeded.
    ///
    /// Ollama returns various error messages for context overflow:
    /// - "context_length_exceeded"
    /// - "maximum context length"
    /// - "token limit"
    /// - "sequence length"
    fn is_context_exceeded(error: &str) -> bool {
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
    /// Returns DEFAULT_CONTEXT_LENGTH (512) if unable to determine.
    pub async fn get_context_length(&self) -> Result<usize, EmbeddingError> {
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

    /// Generate embedding for a single text
    ///
    /// Uses the prefix "search_document: " for nomic-embed-text-v2-moe model.
    /// Truncates to 256 dimensions and normalizes.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        // Add prefix for nomic-embed-text-v2-moe
        let prefixed_text = format!("search_document: {}", text);

        let request = GenerateEmbeddingsRequest::new(self.model.clone(), prefixed_text.into());

        let response = self
            .ollama
            .generate_embeddings(request)
            .await
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

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

    /// Generate embedding with automatic fallback for oversized content.
    ///
    /// If the text exceeds the model's context window, automatically
    /// splits into smaller chunks and retries. Each chunk gets its own
    /// embedding, and the result is a list of embeddings (one per chunk).
    ///
    /// # Arguments
    /// * `text` - The text to embed
    /// * `max_iterations` - Maximum recursion depth (3 recommended)
    ///
    /// # Returns
    /// Vector of embeddings (one per chunk after fallback).
    /// For normal-sized content, returns a single-element vector.
    /// For oversized content that needed chunking, returns multiple embeddings.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ContextExceeded` if content still exceeds
    /// context after maximum iterations.
    pub async fn embed_with_fallback(
        &self,
        text: &str,
        max_iterations: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.embed_with_fallback_inner(text, max_iterations).await
    }

    /// Inner implementation that can be boxed for recursion.
    #[allow(clippy::type_complexity)]
    fn embed_with_fallback_inner<'a>(
        &'a self,
        text: &'a str,
        max_iterations: usize,
    ) -> EmbedFallbackFuture<'a> {
        Box::pin(async move {
            // Try direct embed first
            match self.embed(text).await {
                Ok(emb) => Ok(vec![emb]),
                Err(EmbeddingError::ApiError(ref msg)) if Self::is_context_exceeded(msg) => {
                    // Context exceeded - need to chunk
                    if max_iterations == 0 {
                        return Err(EmbeddingError::ContextExceeded {
                            message: format!(
                                "Content exceeds context even after maximum retries: {}",
                                msg
                            ),
                        });
                    }

                    // Get context length and create halved config
                    let context_length = self.get_context_length().await?;
                    let smaller_config = DynamicChunkConfig::halved(context_length);
                    let chunks = chunk_text_with_config(text, &ChunkConfig::from(&smaller_config));

                    let mut results = Vec::new();
                    for chunk in chunks {
                        let embeddings = self
                            .embed_with_fallback_inner(&chunk.content, max_iterations - 1)
                            .await?;
                        results.extend(embeddings);
                    }
                    Ok(results)
                }
                Err(e) => Err(e),
            }
        })
    }
}

/// Errors from embedding generation
#[derive(Debug, Clone)]
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
        assert!(EmbeddingClient::is_context_exceeded("sequence length exceeded"));
        assert!(!EmbeddingClient::is_context_exceeded("connection refused"));
        assert!(!EmbeddingClient::is_context_exceeded("network error"));
    }
}