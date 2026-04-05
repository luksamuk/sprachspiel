//! Database initialization utilities
//!
//! Provides shared logic for initializing database and embedding client.

use std::sync::Arc;

use crate::db::Database;
use crate::debug_tools::log_debug;
use crate::embeddings::EmbeddingClient;

/// Core database initialization logic shared between modes.
///
/// # Arguments
/// * `ollama` - Ollama client instance
/// * `skip_persistence` - If true, skip database creation (anonymous/code mode)
/// * `use_debug` - Enable debug logging
///
/// # Returns
/// Tuple of (db, embedding_client) - both None if skip_persistence or on failure
pub fn init_database_core(
    ollama: ollama_rs::Ollama,
    skip_persistence: bool,
    use_debug: bool,
) -> (Option<Arc<Database>>, Option<Arc<EmbeddingClient>>) {
    if skip_persistence {
        return (None, None);
    }

    match Database::new() {
        Ok(db) => {
            if use_debug {
                log_debug("Database initialized for message persistence");
            }
            let embedding = Arc::new(EmbeddingClient::new(ollama));
            (Some(Arc::new(db)), Some(embedding))
        }
        Err(e) => {
            if use_debug {
                log_debug(&format!("Database initialization failed: {}", e));
            }
            (None, None)
        }
    }
}
