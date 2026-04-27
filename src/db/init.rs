//! Database initialization utilities
//!
//! Provides shared logic for initializing database and embedding client.

use std::path::PathBuf;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::EmbeddingClient;

/// Core database initialization logic shared between modes.
///
/// # Arguments
/// * `ollama` - Ollama client instance
/// * `skip_persistence` - If true, skip database creation (anonymous/code mode)
/// * `use_debug` - Enable debug logging
/// * `db_path` - Optional custom database path (overrides default XDG path)
///
/// # Returns
/// Tuple of (db, embedding_client) - both None if skip_persistence or on failure
pub fn init_database_core(
    ollama: ollama_rs::Ollama,
    skip_persistence: bool,
    _use_debug: bool,
    db_path: Option<PathBuf>,
) -> (Option<Arc<Database>>, Option<Arc<EmbeddingClient>>) {
    if skip_persistence {
        return (None, None);
    }

    let db = match db_path {
        Some(ref path) => Database::with_path(path),
        None => Database::new(),
    };

    match db {
        Ok(db) => {
            log::debug!("Database initialized for message persistence");
            let embedding = Arc::new(EmbeddingClient::new(ollama));
            (Some(Arc::new(db)), Some(embedding))
        }
        Err(e) => {
            log::debug!("Database initialization failed: {}", e);
            (None, None)
        }
    }
}
