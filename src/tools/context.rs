//! Task-local storage for async tools
//!
//! Provides async-safe access to Database, EmbeddingClient, Ollama client,
//! and Settings for tools that need them.
//!
//! Uses `tokio::task_local!` instead of `thread_local!` because
//! thread-local storage is unsafe in async contexts where tasks
//! can move between threads after `await` points.

use std::future::Future;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::settings::Settings;
use ollama_rs::Ollama;

tokio::task_local! {
    /// Database for tools that need conversation history access
    pub static REMEMBER_DB: Arc<Database>;
    /// Embedding client for tools that need semantic search
    pub static REMEMBER_EMBEDDING: Arc<EmbeddingClient>;
    /// Ollama client for tools that need LLM access (e.g., spawn_subagent)
    pub static TOOL_OLLAMA: Ollama;
    /// Settings for tools that need configuration (e.g., spawn_subagent)
    pub static TOOL_SETTINGS: Arc<Settings>;
}

/// Helper to get database from task-local context
///
/// Returns None if context is not set (e.g., anonymous session)
pub fn get_db() -> Option<Arc<Database>> {
    REMEMBER_DB.try_with(|db| db.clone()).ok()
}

/// Helper to get embedding client from task-local context
///
/// Returns None if context is not set (e.g., anonymous session)
pub fn get_embedding() -> Option<Arc<EmbeddingClient>> {
    REMEMBER_EMBEDDING.try_with(|e| e.clone()).ok()
}

/// Helper to get Ollama client from task-local context
///
/// Returns None if context is not set
pub fn get_ollama() -> Option<Ollama> {
    TOOL_OLLAMA.try_with(|o| o.clone()).ok()
}

/// Helper to get Settings from task-local context
///
/// Returns None if context is not set
pub fn get_settings() -> Option<Arc<Settings>> {
    TOOL_SETTINGS.try_with(|s| s.clone()).ok()
}

/**
 * Run an async function with tool context (TOOL_OLLAMA and TOOL_SETTINGS).
 *
 * This allows tools to access the Ollama client and Settings via task-local storage.
 * Use this wrapper when calling coordinator.chat() or similar async operations
 * in contexts that don't need DB/Embedding access (e.g., anonymous sessions).
 *
 * # Example
 * ```ignore
 * let result = with_tool_context(ollama, settings, async {
 *     coordinator.chat(messages).await
 * }).await;
 * ```
 */
#[allow(clippy::redundant_async_block)]
pub async fn with_tool_context<F, T>(ollama: Ollama, settings: Arc<Settings>, f: F) -> T
where
    F: Future<Output = T>,
{
    TOOL_OLLAMA
        .scope(ollama, async move {
            TOOL_SETTINGS.scope(settings, async move { f.await }).await
        })
        .await
}

/// Run an async function with full tool context including Ollama and Settings
///
/// This allows tools like spawn_subagent to access the Ollama client
/// and Settings while still having DB and Embedding access.
#[allow(clippy::redundant_async_block)]
pub async fn with_full_context<F, T>(
    db: Arc<Database>,
    embedding: Arc<EmbeddingClient>,
    ollama: Ollama,
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
                    with_tool_context(ollama, settings, async move { f.await }).await
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
        let dummy_ollama = Ollama::new("http://localhost".to_string(), 11434);
        let dummy_settings = Arc::new(Settings::default());

        let result = with_tool_context(dummy_ollama, dummy_settings, async {
            let ollama = get_ollama();
            let settings = get_settings();

            assert!(
                ollama.is_some(),
                "get_ollama should return Some inside scope"
            );
            assert!(
                settings.is_some(),
                "get_settings should return Some inside scope"
            );

            (ollama.is_some(), settings.is_some())
        })
        .await;

        assert!(result.0, "ollama should be scoped");
        assert!(result.1, "settings should be scoped");
    }

    #[test]
    async fn with_full_context_scopes_all_four() {
        let dummy_ollama = Ollama::new("http://localhost".to_string(), 11434);
        let dummy_embedding = Arc::new(EmbeddingClient::new(dummy_ollama.clone()));
        let dummy_ollama_for_test = Ollama::new("http://localhost".to_string(), 11434);
        let dummy_settings = Arc::new(Settings::default());
        let dummy_db = Arc::new(Database::in_memory().unwrap());

        let result = with_full_context(
            dummy_db,
            dummy_embedding,
            dummy_ollama_for_test,
            dummy_settings,
            async {
                let db = get_db();
                let embedding = get_embedding();
                let ollama = get_ollama();
                let settings = get_settings();

                assert!(db.is_some(), "get_db should return Some inside full scope");
                assert!(
                    embedding.is_some(),
                    "get_embedding should return Some inside full scope"
                );
                assert!(
                    ollama.is_some(),
                    "get_ollama should return Some inside full scope"
                );
                assert!(
                    settings.is_some(),
                    "get_settings should return Some inside full scope"
                );

                (
                    db.is_some(),
                    embedding.is_some(),
                    ollama.is_some(),
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
        // Clear any existing context by running in a fresh task
        let result = tokio::spawn(async {
            let ollama = get_ollama();
            let settings = get_settings();
            let db = get_db();
            let embedding = get_embedding();

            (
                ollama.is_none(),
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
