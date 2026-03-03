//! Embedding client for Ollama API
//!
//! Generates embeddings using nomic-embed-text-v2-moe model.

use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use ollama_rs::Ollama;

use super::truncate::{truncate_and_normalize, FULL_DIMENSIONS, TRUNCATED_DIMENSIONS};

/// Default embedding model (nomic-embed-text-v2-moe)
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text-v2-moe:latest";

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

    /// Generate embedding for a single text
    ///
    /// Uses the prefix "search_document: " for nomic-embed-text-v2-moe model.
    /// Truncates to 256 dimensions and normalizes.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        // Add prefix for nomic-embed-text-v2-moe
        let prefixed_text = format!("search_document: {}", text);
        
        let request = GenerateEmbeddingsRequest::new(self.model.clone(), prefixed_text.into());
        
        let response = self.ollama.generate_embeddings(request).await
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        let embedding = response.embeddings.into_iter().next()
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
        
        let response = self.ollama.generate_embeddings(request).await
            .map_err(|e| EmbeddingError::ApiError(e.to_string()))?;

        // Truncate and normalize each embedding
        Ok(response.embeddings
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
pub enum EmbeddingError {
    /// API call failed
    ApiError(String),
    /// No embedding returned
    NoEmbedding,
    /// Invalid embedding dimensions
    InvalidDimensions {
        expected: usize,
        got: usize,
    },
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(msg) => write!(f, "Embedding API error: {}", msg),
            Self::NoEmbedding => write!(f, "No embedding returned from API"),
            Self::InvalidDimensions { expected, got } => {
                write!(f, "Invalid embedding dimensions: expected {}, got {}", expected, got)
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
}