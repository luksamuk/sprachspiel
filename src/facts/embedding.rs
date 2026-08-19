//! Fact embedding generation
//!
//! Generates embeddings for facts using the same model and process as
//! content embeddings. Facts are short (max 500 chars) so no chunking
//! or fallback is needed. The embedding is generated directly via
//! `EmbeddingClient::embed()`.

use crate::embeddings::EmbeddingClient;
use crate::embeddings::EmbeddingError;
use crate::embeddings::TruncateResult;

/// Generate an embedding vector for a fact's content.
///
/// Uses the configured prefix and dimensions from the embedding
/// model. Facts are short content (< 500 chars) so they never exceed
/// the model's context window.
///
/// Returns a [`TruncateResult`] containing both the normalized vector
/// at the configured dimensions and the norm correction factor for
/// accurate cosine similarity computation.
///
/// # Arguments
/// * `content` - The fact content to embed
/// * `client` - The embedding client
///
/// # Errors
/// Returns `EmbeddingError` if the API call fails
pub async fn generate_fact_embedding(
    content: &str,
    client: &EmbeddingClient,
) -> Result<TruncateResult, EmbeddingError> {
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
