//! Task-local storage for async tools
//!
//! Provides async-safe access to Database and EmbeddingClient
//! for tools that need them (like `remember`).
//!
//! Uses `tokio::task_local!` instead of `thread_local!` because
//! thread-local storage is unsafe in async contexts where tasks
//! can move between threads after `await` points.

use std::future::Future;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::EmbeddingClient;

tokio::task_local! {
    /// Database for tools that need conversation history access
    pub static REMEMBER_DB: Arc<Database>;
    /// Embedding client for tools that need semantic search
    pub static REMEMBER_EMBEDDING: Arc<EmbeddingClient>;
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
#[allow(clippy::redundant_async_block)]
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