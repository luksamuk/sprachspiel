//! Task-local storage for async tools
//!
//! Provides async-safe access to Database, EmbeddingClient, LLM provider,
//! and Settings for tools that need them.
//!
//! Stores `crate::provider::OpenAICompatibleProvider` (the shim) for backward compatibility
//! with the existing tool implementations. The shim delegates to
//! `OpenAICompatibleProvider` internally.

use std::future::Future;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::settings::Settings;

tokio::task_local! {
    /// Database for tools that need conversation history access
    pub static REMEMBER_DB: Arc<Database>;
    /// Embedding client for tools that need semantic search
    pub static REMEMBER_EMBEDDING: Arc<EmbeddingClient>;
    /// Ollama-compatible shim for tools that need LLM access
    pub static TOOL_LLM: crate::provider::OpenAICompatibleProvider;
    /// Settings for tools that need configuration (e.g., agent spawning tools)
    pub static TOOL_SETTINGS: Arc<Settings>;
}

/// Helper to get database from task-local context
pub fn get_db() -> Option<Arc<Database>> {
    REMEMBER_DB.try_with(|db| db.clone()).ok()
}

/// Helper to get embedding client from task-local context
pub fn get_embedding() -> Option<Arc<EmbeddingClient>> {
    REMEMBER_EMBEDDING.try_with(|e| e.clone()).ok()
}

/// Helper to get Ollama-compatible shim from task-local context
pub fn get_ollama() -> Option<crate::provider::OpenAICompatibleProvider> {
    TOOL_LLM.try_with(|o| o.clone()).ok()
}

/// Helper to get LLM provider from task-local context (alias for get_provider)
pub fn get_llm() -> Option<crate::provider::OpenAICompatibleProvider> {
    get_ollama()
}

/// Helper to get Settings from task-local context
pub fn get_settings() -> Option<Arc<Settings>> {
    TOOL_SETTINGS.try_with(|s| s.clone()).ok()
}

#[expect(clippy::redundant_async_block)]
pub async fn with_tool_context<F, T>(
    provider: crate::provider::OpenAICompatibleProvider,
    settings: Arc<Settings>,
    f: F,
) -> T
where
    F: Future<Output = T>,
{
    TOOL_LLM
        .scope(provider, async move {
            TOOL_SETTINGS.scope(settings, async move { f.await }).await
        })
        .await
}

#[expect(clippy::redundant_async_block)]
pub async fn with_full_context<F, T>(
    db: Arc<Database>,
    embedding: Arc<EmbeddingClient>,
    provider: crate::provider::OpenAICompatibleProvider,
    settings: Arc<Settings>,
    f: F,
) -> T
where
    F: Future<Output = T>,
{
    REMEMBER_DB
        .scope(db, async move {
            REMEMBER_EMBEDDING
                .scope(embedding, async move {
                    with_tool_context(provider, settings, async move { f.await }).await
                })
                .await
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn with_tool_context_scopes_ollama_and_settings() {
        let dummy_provider = crate::provider::OpenAICompatibleProvider::new_local(
            "http://localhost".to_string(),
            11434,
        );
        let dummy_settings = Arc::new(Settings::default());

        let result = with_tool_context(dummy_provider, dummy_settings, async {
            let provider = get_ollama();
            let settings = get_settings();

            assert!(
                provider.is_some(),
                "get_ollama should return Some inside scope"
            );
            assert!(
                settings.is_some(),
                "get_settings should return Some inside scope"
            );

            (provider.is_some(), settings.is_some())
        })
        .await;

        assert!(result.0, "ollama should be scoped");
        assert!(result.1, "settings should be scoped");
    }

    #[test]
    async fn with_full_context_scopes_all_four() {
        let dummy_provider = crate::provider::OpenAICompatibleProvider::new_local(
            "http://localhost".to_string(),
            11434,
        );
        let dummy_settings = Arc::new(Settings::default());
        let dummy_embedding = Arc::new(EmbeddingClient::with_model(
            crate::provider::OpenAICompatibleProvider::new_local(
                "http://localhost".to_string(),
                11434,
            ),
            dummy_settings.indexing_model_alias().to_string(),
            768, // TRANSITIONAL: placeholder
        ));
        let dummy_provider_for_test = crate::provider::OpenAICompatibleProvider::new_local(
            "http://localhost".to_string(),
            11434,
        );
        let dummy_db = Arc::new(Database::in_memory().unwrap());

        let result = with_full_context(
            dummy_db,
            dummy_embedding,
            dummy_provider_for_test,
            dummy_settings,
            async {
                let db = get_db();
                let embedding = get_embedding();
                let provider = get_ollama();
                let settings = get_settings();

                assert!(db.is_some(), "get_db should return Some inside full scope");
                assert!(
                    embedding.is_some(),
                    "get_embedding should return Some inside full scope"
                );
                assert!(
                    provider.is_some(),
                    "get_ollama should return Some inside full scope"
                );
                assert!(
                    settings.is_some(),
                    "get_settings should return Some inside full scope"
                );

                (
                    db.is_some(),
                    embedding.is_some(),
                    provider.is_some(),
                    settings.is_some(),
                )
            },
        )
        .await;

        assert!(result.0, "db should be scoped");
        assert!(result.1, "embedding should be scoped");
        assert!(result.2, "ollama should be scoped");
        assert!(result.3, "settings should be scoped");
    }

    #[test]
    async fn get_ollama_returns_none_outside_scope() {
        let result = tokio::spawn(async {
            let provider = get_ollama();
            let settings = get_settings();
            let db = get_db();
            let embedding = get_embedding();

            (
                provider.is_none(),
                settings.is_none(),
                db.is_none(),
                embedding.is_none(),
            )
        })
        .await;

        let (o, s, d, e) = result.unwrap();
        assert!(o, "get_ollama should return None outside scope");
        assert!(s, "get_settings should return None outside scope");
        assert!(d, "get_db should return None outside scope");
        assert!(e, "get_embedding should return None outside scope");
    }
}
