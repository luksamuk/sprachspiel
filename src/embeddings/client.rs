//! Embedding client for Ollama API
//!
//! Generates embeddings using nomic-embed-text-v2-moe model.

use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use ollama_rs::Ollama;

use super::truncate::truncate_and_normalize;

/// Default embedding model (nomic-embed-text-v2-moe)
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text-v2-moe:latest";

/// Embedding dimension (Matryoshka truncated to 256)
pub const EMBEDDING_DIMENSION: usize = 256;

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
            .ok_or_else(|| EmbeddingError::NoEmbedding)?;

        // Truncate and normalize
        Ok(truncate_and_normalize(&embedding))
    }

    /// Generate embeddings for multiple texts
    ///
    /// More efficient than calling embed() multiple times.
    /// Uses "search_document: " prefix for each text.
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
}

/// Errors from embedding generation
#[derive(Debug, Clone)]
pub enum EmbeddingError {
    /// API call failed
    ApiError(String),
    /// No embedding returned
    NoEmbedding,
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(msg) => write!(f, "Embedding API error: {}", msg),
            Self::NoEmbedding => write!(f, "No embedding returned from API"),
        }
    }
}

impl std::error::Error for EmbeddingError {}

#[cfg(test)]
mod tests {
    // Note: Integration tests require Ollama running with nomic-embed-text-v2-moe model
    // These are unit tests for the structure only
    
    #[test]
    fn test_embedding_dimension() {
        assert_eq!(super::EMBEDDING_DIMENSION, 256);
    }

    #[test]
    fn test_default_model() {
        assert_eq!(super::DEFAULT_EMBEDDING_MODEL, "nomic-embed-text-v2-moe:latest");
    }
}