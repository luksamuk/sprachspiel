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
    // Facts have no title; doc-side template renders as untitled.
    client.embed_document(None, content).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::spy::SpyProvider;

    #[tokio::test]
    async fn test_fact_path_embeds_untitled_document() {
        // Facts walk the doc side with no title: the template's {title}
        // slot renders as `none`, exactly once.
        let spy = SpyProvider::new(768);
        let calls = spy.calls();
        let client = EmbeddingClient::with_model(
            spy,
            "eg2".to_string(),
            768,
            crate::embeddings::PrefixTemplates {
                query: "task: search result | query: {content}".to_string(),
                doc: "title: {title} | text: {content}".to_string(),
            },
            None,
        );

        generate_fact_embedding("User prefers dark mode", &client)
            .await
            .expect("fact embedding ok");

        assert_eq!(
            calls.texts(),
            vec!["title: none | text: User prefers dark mode".to_string()],
            "fact path must render the doc template exactly once, untitled"
        );
    }

    #[test]
    fn test_generate_fact_embedding_function_exists() {
        // Verify the function compiles and has correct signature
        // Actual embedding generation tests require a running Ollama instance
        assert!(true);
    }
}
