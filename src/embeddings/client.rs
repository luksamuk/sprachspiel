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

use super::chunk_config::DynamicChunkConfig;
use super::prefix::PrefixTemplates;
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
    provider: Box<dyn LlmProvider>,
    model: String,
    /// Output dimension of the embedding model (from the alias's
    /// `dimensions = N` in models.toml). Used for vector store
    /// sizing and probe verification.
    dimensions: u32,
    /// Per-side task-steering templates (`[indexing].query_prefix` /
    /// `[indexing].doc_prefix`). See [`PrefixTemplates`].
    prefixes: PrefixTemplates,
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
    ///
    /// `prefixes` carries the per-side steering templates. Callers must
    /// resolve them through `IndexingSettings::resolved_prefixes()`,
    /// which validates the config at load time.
    pub fn with_model(
        provider: impl LlmProvider + 'static,
        model: String,
        dimensions: u32,
        prefixes: PrefixTemplates,
        context_length: Option<u32>,
    ) -> Self {
        Self {
            provider: Box::new(provider),
            model,
            dimensions,
            prefixes,
            context_length,
            cached_context_length: OnceCell::new(),
            semaphore: Semaphore::new(1),
        }
    }

    /// Token margin the chunker must reserve for the rendered prefix,
    /// so the prefix never silently displaces document content. The
    /// dynamic part grows with the worst-case rendered template; the
    /// constant floor keeps the pre-149 safety headroom when no
    /// template is configured.
    pub fn prefix_margin_tokens(&self) -> usize {
        let chars_per_token = super::chunk_config::DEFAULT_CHARS_PER_TOKEN;
        let dynamic = (self.prefixes.max_rendered_len() as f32 / chars_per_token).ceil() as usize;
        dynamic + super::chunk_config::DEFAULT_PREFIX_MARGIN
    }

    /// A chunk config with the margin that accounts for this client's
    /// rendered prefixes.
    pub fn dynamic_chunk_config(&self, context_length: usize) -> DynamicChunkConfig {
        DynamicChunkConfig::with_prefix_margin(context_length, self.prefix_margin_tokens())
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

    /// Generate an embedding for a retrieval QUERY.
    ///
    /// This is the single render boundary for the query side: the text
    /// must be raw content (never pre-prefixed). The rendered query
    /// template from `[indexing].query_prefix` is applied exactly once
    /// here, before the provider call.
    pub async fn embed_query(&self, text: &str) -> Result<TruncateResult, EmbeddingError> {
        self.embed_rendered(self.prefixes.render_query(text)).await
    }

    /// Generate an embedding for a retrieved DOCUMENT (content item,
    /// chunk, fact) with its optional title.
    ///
    /// This is the single render boundary for the doc side: the text
    /// must be raw content plus the item's title — never pre-prefixed.
    /// A missing, empty, or whitespace-only title renders as `none`
    /// inside the `[indexing].doc_prefix` template. Chunks embed here
    /// as untitled documents.
    pub async fn embed_document(
        &self,
        title: Option<&str>,
        text: &str,
    ) -> Result<TruncateResult, EmbeddingError> {
        self.embed_rendered(self.prefixes.render_doc(title, text))
            .await
    }

    /// Generate an embedding for text that is ALREADY in its final
    /// server-side form.
    ///
    /// Exclusively for diagnostics, evaluations, and A/B probes that
    /// render prefixes themselves — never for persisting memory or
    /// content embeddings: those must go through [`Self::embed_query`]
    /// or [`Self::embed_document`] so the steering prefix is applied
    /// exactly once, at the render boundary.
    pub async fn embed_raw(&self, text: &str) -> Result<TruncateResult, EmbeddingError> {
        self.embed_rendered(text.to_string()).await
    }

    /// Raw-path batch variant of [`Self::embed_raw`] (diagnostics and
    /// evaluations only — see the [`Self::embed_raw`] contract).
    #[allow(dead_code)]
    pub async fn embed_batch_raw(
        &self,
        texts: &[&str],
    ) -> Result<Vec<TruncateResult>, EmbeddingError> {
        let mut results = Vec::new();
        for text in texts {
            results.push(self.embed_raw(text).await?);
        }
        Ok(results)
    }

    /// Core embedding call. Sends the already-rendered text to the
    /// embedding server without any proactive context-length
    /// estimation. If the server returns a context-exceeded error, it
    /// is converted to `EmbeddingError::ContextExceeded` and handled
    /// by the fallback chunking pipeline in `fallback.rs`.
    async fn embed_rendered(&self, prefixed_text: String) -> Result<TruncateResult, EmbeddingError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| EmbeddingError::ApiError("Semaphore closed".to_string()))?;

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
    use crate::embeddings::spy::SpyProvider;

    fn make_dummy_provider() -> crate::provider::OpenAICompatibleProvider {
        crate::provider::OpenAICompatibleProvider::new_local("http://localhost".to_string(), 11434)
    }

    /// eg2-style asymmetric templates used across the spy tests.
    fn eg2_templates() -> PrefixTemplates {
        PrefixTemplates {
            query: "task: search result | query: {content}".to_string(),
            doc: "title: {title} | text: {content}".to_string(),
        }
    }

    fn eg2_spy() -> (EmbeddingClient, crate::embeddings::spy::SpyCalls) {
        let spy = SpyProvider::new(768);
        let calls = spy.calls();
        let client =
            EmbeddingClient::with_model(spy, "eg2".to_string(), 768, eg2_templates(), None);
        (client, calls)
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
            PrefixTemplates::default(),
            Some(8192),
        );
        assert_eq!(client_with_ctx.context_length(), 8192);

        let client_without_ctx = EmbeddingClient::with_model(
            make_dummy_provider(),
            "test".to_string(),
            768,
            PrefixTemplates::default(),
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
            PrefixTemplates::default(),
            None,
        );
    }

    #[test]
    fn test_with_model_stores_model_name() {
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "bge-small-en-v1.5".to_string(),
            768,
            PrefixTemplates::default(),
            None,
        );
        assert_eq!(client.model(), "bge-small-en-v1.5");
        assert_eq!(client.dimensions, 768);
    }

    #[test]
    fn test_with_model_stores_templates_and_context() {
        let templates = eg2_templates();
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "bge-small-en-v1.5".to_string(),
            768,
            templates,
            Some(8192),
        );
        assert_eq!(client.prefixes.render_query("x"), "task: search result | query: x");
        assert_eq!(client.context_length, Some(8192));
    }

    #[test]
    fn test_with_model_default_templates() {
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "nomic-embed-text-v2-moe".to_string(),
            256,
            PrefixTemplates::default(),
            None,
        );
        assert_eq!(client.prefixes.render_query("x"), "x");
        assert_eq!(client.context_length, None);
    }

    #[test]
    fn test_prefix_margin_tokens_defaults_to_floor() {
        // No templates → zero dynamic cost; margin keeps the pre-149 floor.
        let client = EmbeddingClient::with_model(
            make_dummy_provider(),
            "m".to_string(),
            768,
            PrefixTemplates::default(),
            None,
        );
        assert_eq!(client.prefix_margin_tokens(), 40);
    }

    // ============================================================
    // Typed-path spy tests: exact rendered string, single application
    // ============================================================

    #[tokio::test]
    async fn test_embed_query_renders_query_template_exactly_once() {
        let (client, calls) = eg2_spy();
        client.embed_query("rust async traits").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["task: search result | query: rust async traits".to_string()],
            "query side must receive the rendered template text exactly once"
        );
    }

    #[tokio::test]
    async fn test_embed_document_titled_renders_doc_template_exactly_once() {
        let (client, calls) = eg2_spy();
        client.embed_document(Some("Design Doc"), "the body").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["title: Design Doc | text: the body".to_string()],
            "doc side with a title must render the title slot exactly once"
        );
    }

    #[tokio::test]
    async fn test_embed_document_untitled_renders_none() {
        let (client, calls) = eg2_spy();
        client.embed_document(None, "the body").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["title: none | text: the body".to_string()],
            "untitled documents render the `none` title value"
        );
    }

    #[tokio::test]
    async fn test_embed_document_whitespace_title_renders_none() {
        let (client, calls) = eg2_spy();
        client.embed_document(Some("   "), "the body").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["title: none | text: the body".to_string()]
        );
    }

    #[tokio::test]
    async fn test_chunk_path_embeds_untitled_document() {
        // Chunks ride the doc path always untitled — same as facts.
        let (client, calls) = eg2_spy();
        client.embed_document(None, "chunk text").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["title: none | text: chunk text".to_string()]
        );
    }

    #[tokio::test]
    async fn test_embed_raw_is_the_only_unrendered_path() {
        let (client, calls) = eg2_spy();
        client.embed_raw("raw probe text").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["raw probe text".to_string()],
            "raw path reaches the provider verbatim — diagnostics/evals only"
        );
    }

    #[tokio::test]
    async fn test_embed_batch_raw_applies_no_template() {
        let (client, calls) = eg2_spy();
        client.embed_batch_raw(&["probe one", "probe two"]).await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec!["probe one".to_string(), "probe two".to_string()]
        );
    }

    #[tokio::test]
    async fn test_opaque_values_are_not_rescanned() {
        let (client, calls) = eg2_spy();
        // Content containing placeholder-shaped text must reach the
        // server literally — no chained substitution.
        client.embed_query("user wrote {title} in braces").await.expect("ok");
        assert_eq!(
            calls.texts(),
            vec![
                "task: search result | query: user wrote {title} in braces".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_empty_templates_embed_text_verbatim() {
        // Default configuration: zero behavior change; both sides verbatim.
        let spy = SpyProvider::new(768);
        let calls = spy.calls();
        let client = EmbeddingClient::with_model(
            spy,
            "eg2".to_string(),
            768,
            PrefixTemplates::default(),
            None,
        );
        client.embed_query("plain").await.expect("ok");
        client.embed_document(Some("T"), "plain").await.expect("ok");
        assert_eq!(calls.texts(), vec!["plain".to_string(), "plain".to_string()]);
    }
}
