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

/// Configuration for embedding initialization (W2 #121).
///
/// The chat provider and the embedding provider are decoupled. The
/// chat subcommand's `crate::provider::Ollama` is for chat, and
/// `embedding_provider` is the `crate::provider::Ollama` configured
/// to talk to the embedding-capable provider from
/// `models.toml [provider.X]`. (W2 #121 extension: the embedding
/// capability is now declared per-`[models.X]` via `embeddings =
/// true` + `dimensions = N`, not on the provider.)
pub struct EmbeddingInit {
    /// The provider (shim) used for embedding calls. Points to a
    /// provider in `models.toml` with `embedding = true`.
    pub provider: crate::provider::Ollama,
    /// Embedding model name (from `[embedding].model` in `config.toml`).
    pub model_name: String,
    /// Whether to probe `/v1/embeddings` at startup (from
    /// `[embedding].probe` in `config.toml`).
    pub probe: bool,
}

/// Core database initialization logic shared between modes.
///
/// # Arguments
/// * `embedding_init` - Embedding configuration (provider + model name + probe flag)
/// * `skip_persistence` - If true, skip database creation (anonymous/code mode)
/// * `_use_debug` - Enable debug logging (unused, kept for API compatibility)
/// * `db_path` - Optional custom database path (overrides default XDG path)
///
/// # Returns
/// `DatabaseInitResult` with db/embedding on success, or error details on failure.
pub fn init_database_core(
    embedding_init: EmbeddingInit,
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

    // W2 #121: model name must not be empty. The [embedding] section
    // in config.toml is required, and the model field is the only
    // way to know which model to use.
    if embedding_init.model_name.trim().is_empty() {
        let error_detail = String::from(
            "Error: [embedding].model is empty in config.toml.\n\
             \n\
             The [embedding] section is required for sprach to work.\n\
             Add to your config.toml:\n\
             \n\
             [embedding]\n\
             model = \"nomic-embed-text-v2-moe\"\n\
             # provider = \"llama-swap\"   # optional, defaults to chat provider\n\
             # probe = true                # optional, default true\n\
             \n\
             Run `sprach config upgrade` to insert a documented placeholder.",
        );
        log::error!("{}", error_detail);
        return DatabaseInitResult {
            db: None,
            embedding: None,
            error_detail: Some(error_detail),
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
            let embedding = Arc::new(EmbeddingClient::with_model(
                embedding_init.provider.clone(),
                embedding_init.model_name.clone(),
            ));
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

/// Run the embedding probe (W2 #121).
///
/// This is a separate function from `init_database_core` so it can
/// be called *after* the chat provider is confirmed reachable (in
/// `init_chat_database`) but *before* the database is initialized.
/// Returning a structured `Result` lets the caller surface a
/// user-friendly error message.
///
/// Returns `Ok(())` if:
/// - `probe = false` (skip probe, trust config)
/// - the probe call succeeded (HTTP 2xx)
///
/// Returns `Err(message)` if:
/// - the probe call failed (HTTP 4xx/5xx, network error, timeout)
pub async fn run_embedding_probe(
    provider: &crate::provider::Ollama,
    model_name: &str,
    probe_enabled: bool,
) -> Result<(), String> {
    if !probe_enabled {
        log::info!("Embedding probe disabled (probe = false). Trusting config.");
        return Ok(());
    }
    log::info!(
        "Probing embedding endpoint for model '{}' (1 POST /v1/embeddings, ~30s timeout)...",
        model_name
    );
    match provider.probe_embedding(model_name).await {
        Ok(()) => {
            log::info!(
                "Embedding probe OK: provider serves /v1/embeddings for '{}'",
                model_name
            );
            Ok(())
        }
        Err(e) => {
            let base = provider.base_url().trim_end_matches("/v1").to_string();
            let msg = format!(
                "Error: Probe embedding call to provider at {base} with model '{model_name}' failed: {e}.\n\
                 \n\
                 Possible causes:\n\
                 1. The model is not loaded on the provider (cold start; wait and retry, or set [embedding].probe = false)\n\
                 2. The provider does not serve /v1/embeddings for this model\n\
                 3. The provider base_url is wrong in models.toml [provider.<name>].base_url\n\
                 4. The provider is unreachable (network error, server down)\n\
                 \n\
                 To fix:\n\
                 - Verify the model is served: curl {base}/v1/models | grep '{model_name}'\n\
                 - Verify the provider is reachable: curl {base}/v1/models\n\
                 - Set [embedding].probe = false in config.toml to skip the probe and trust the config"
            );
            log::error!("{}", msg);
            Err(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_init_struct_construction() {
        let provider = crate::provider::Ollama::new("http://localhost", 11434);
        let init = EmbeddingInit {
            provider,
            model_name: "nomic-embed-text-v2-moe".to_string(),
            probe: true,
        };
        assert_eq!(init.model_name, "nomic-embed-text-v2-moe");
        assert!(init.probe);
    }

    #[test]
    fn test_init_database_core_skip_persistence() {
        let provider = crate::provider::Ollama::new("http://localhost", 11434);
        let result = init_database_core(
            EmbeddingInit {
                provider,
                model_name: "test".to_string(),
                probe: false,
            },
            true, // skip_persistence
            false,
            None,
        );
        assert!(result.db.is_none());
        assert!(result.embedding.is_none());
        assert!(result.error_detail.is_none());
    }

    #[test]
    fn test_init_database_core_rejects_empty_model() {
        let provider = crate::provider::Ollama::new("http://localhost", 11434);
        let result = init_database_core(
            EmbeddingInit {
                provider,
                model_name: "".to_string(),
                probe: false,
            },
            false,
            false,
            None,
        );
        assert!(result.db.is_none());
        assert!(result.embedding.is_none());
        let detail = result.error_detail.expect("error_detail should be set");
        assert!(detail.contains("[embedding].model is empty"));
        assert!(detail.contains("nomic-embed-text-v2-moe"));
    }

    #[test]
    fn test_init_database_core_rejects_whitespace_model() {
        let provider = crate::provider::Ollama::new("http://localhost", 11434);
        let result = init_database_core(
            EmbeddingInit {
                provider,
                model_name: "   ".to_string(),
                probe: false,
            },
            false,
            false,
            None,
        );
        assert!(result.db.is_none());
        assert!(result.error_detail.is_some());
    }
}
