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

/// Run an async function with the tool context set
///
/// This allows tools to access DB and EmbeddingClient via task-local storage.
/// Use this wrapper when calling coordinator.chat() or similar async operations.
///
/// # Example
/// ```ignore
/// let result = with_context(db, embedding, async {
///     coordinator.chat(messages).await
/// }).await;
/// ```
#[allow(clippy::redundant_async_block, dead_code)]
pub async fn with_context<F, T>(db: Arc<Database>, embedding: Arc<EmbeddingClient>, f: F) -> T
where
    F: Future<Output = T>,
{
    REMEMBER_DB
        .scope(db, async move {
            REMEMBER_EMBEDDING
                .scope(embedding, async move { f.await })
                .await
        })
        .await
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
#[allow(clippy::redundant_async_block, dead_code)]
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
