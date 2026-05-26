//! Database initialization utilities
//!
//! Provides shared logic for initializing database and embedding client.

use std::path::PathBuf;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::EmbeddingClient;

/// Result of database initialization.
///
/// On success, contains the database and embedding client.
/// On failure, contains a human-readable error message with diagnostic details.
pub struct DatabaseInitResult {
    /// Database handle (None on failure or anonymous mode)
    pub db: Option<Arc<Database>>,
    /// Embedding client (None on failure or anonymous mode)
    pub embedding: Option<Arc<EmbeddingClient>>,
    /// Error details if initialization failed (None on success or anonymous mode)
    pub error_detail: Option<String>,
}

/// Core database initialization logic shared between modes.
///
/// # Arguments
/// * `ollama` - Ollama client instance
/// * `skip_persistence` - If true, skip database creation (anonymous/code mode)
/// * `use_debug` - Enable debug logging (unused, kept for API compatibility)
/// * `db_path` - Optional custom database path (overrides default XDG path)
///
/// # Returns
/// `DatabaseInitResult` with db/embedding on success, or error details on failure.
pub fn init_database_core(
    ollama: ollama_rs::Ollama,
    skip_persistence: bool,
    _use_debug: bool,
    db_path: Option<PathBuf>,
) -> DatabaseInitResult {
    if skip_persistence {
        return DatabaseInitResult {
            db: None,
            embedding: None,
            error_detail: None,
        };
    }

    let storage_path = match &db_path {
        Some(path) => path.clone(),
        None => Database::get_storage_path(),
    };

    let db = match db_path {
        Some(ref path) => Database::with_path(path),
        None => Database::new(),
    };

    match db {
        Ok(db) => {
            log::info!("Database initialized for message persistence");
            let embedding = Arc::new(EmbeddingClient::new(ollama));
            DatabaseInitResult {
                db: Some(Arc::new(db)),
                embedding: Some(embedding),
                error_detail: None,
            }
        }
        Err(e) => {
            let error_detail = format!(
                "Database initialization failed: {e}\n\
                 \n\
                 Storage path: {}\n\
                 \n\
                 Possible causes:\n\
                 1. sqlite-vec extension not loaded (check your installation)\n\
                 2. Permission denied for storage directory\n\
                 3. Corrupted database file or failed migration (try deleting and restarting)\n\
                 4. Disk full or I/O error\n\
                 \n\
                 To diagnose:\n\
                 - Check directory permissions: ls -la {}\n\
                 - Run with -v for more information\n\
                 - Use --anonymous for anonymous mode without database persistence.",
                storage_path.display(),
                storage_path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            );
            log::error!("{}", error_detail);
            DatabaseInitResult {
                db: None,
                embedding: None,
                error_detail: Some(error_detail),
            }
        }
    }
}
