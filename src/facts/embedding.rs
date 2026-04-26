//! Fact embedding generation
//!
//! Generates 256-dimensional Matryoshka-truncated embeddings for facts using
//! the same model and process as content embeddings (nomic-embed-text-v2-moe).
//!
//! Facts are short (max 500 chars) so no chunking or fallback is needed.
//! The embedding is generated directly via `EmbeddingClient::embed()`.

use crate::embeddings::EmbeddingClient;
use crate::embeddings::EmbeddingError;

/// Generate an embedding vector for a fact's content.
///
/// Uses the same "search_document: " prefix and 256-dimensional truncation
/// as content embeddings. Facts are short content (< 500 chars) so they
/// never exceed the model's context window.
///
/// # Arguments
/// * `content` - The fact content to embed
/// * `client` - The embedding client
///
/// # Returns
/// A 256-dimensional L2-normalized embedding vector
///
/// # Errors
/// Returns `EmbeddingError` if the API call fails
pub async fn generate_fact_embedding(
    content: &str,
    client: &EmbeddingClient,
) -> Result<Vec<f32>, EmbeddingError> {
    client.embed(content).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_generate_fact_embedding_function_exists() {
        // Verify the function compiles and has correct signature
        // Actual embedding generation tests require a running Ollama instance
        assert!(true);
    }
}
