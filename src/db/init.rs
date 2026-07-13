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

/// Configuration for indexing initialization.
///
/// Bundles the data the indexing pipeline needs at startup:
/// - the `Ollama` (shim) for the resolved embedding provider
/// - the upstream `model_id` (the name passed to `/v1/embeddings`)
/// - the `dimensions` (from the alias in `models.toml`)
/// - the `probe` flag (whether to verify the provider at startup)
///
/// The model alias itself (`[indexing].model` in `config.toml`) is
/// resolved upstream of this struct by
/// `Settings::resolve_indexing_model`, which produces the
/// `(UserModelConfig, ProviderConfig, model_id, dimensions)` tuple.
pub struct IndexingInit {
    /// The provider (shim) used for embedding calls. Points to a
    /// provider in `models.toml [provider.*]`.
    pub provider: crate::provider::OpenAICompatibleProvider,
    /// Upstream `model_id` (the name passed verbatim to the
    /// provider's `/v1/embeddings` endpoint).
    pub model_id: String,
    /// Output dimension of the embedding model (from the alias's
    /// `dimensions = N` in models.toml). Used for vector store
    /// sizing and probe verification.
    pub dimensions: u32,
    /// Whether to probe `/v1/embeddings` at startup (from
    /// `[indexing].probe` in `config.toml`).
    #[allow(dead_code)]
    // Config flag — passed to run_indexing_probe(), not read via field access
    pub probe: bool,
}

/// Core database initialization logic shared between modes.
///
/// # Arguments
/// * `indexing_init` - Indexing configuration (provider + model id + dimensions + probe flag)
/// * `skip_persistence` - If true, skip database creation (anonymous/code mode)
/// * `_use_debug` - Enable debug logging (unused, kept for API compatibility)
/// * `db_path` - Optional custom database path (overrides default XDG path)
///
/// # Returns
/// `DatabaseInitResult` with db/embedding on success, or error details on failure.
pub fn init_database_core(
    indexing_init: IndexingInit,
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

    // model id must not be empty. The [indexing] section in
    // config.toml is required, and the alias must resolve to a model
    // in models.toml. The actual alias validation is done by
    // Settings::resolve_indexing_model; this defensive check catches
    // any bypass.
    if indexing_init.model_id.trim().is_empty() {
        let error_detail = String::from(
            "Error: indexing model_id is empty. \
             The [indexing] section in config.toml must reference an \
             embedding-capable alias from models.toml [models.*]. \
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
            // EmbeddingClient::with_model now takes (ollama,
            // model_name, dimensions). dimensions is sourced from
            // the alias in models.toml and propagated through
            // IndexingInit; the EmbeddingClient is just a thin
            // holder for now. The probe (Commit 6) uses the
            // dimensions for strict-verify.
            let embedding = Arc::new(EmbeddingClient::with_model(
                indexing_init.provider.clone(),
                indexing_init.model_id.clone(),
                indexing_init.dimensions,
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

/// Run the indexing probe.
///
/// Sends 1 POST `/v1/embeddings` call to verify the provider
/// actually serves the model. The probe does NOT pass
/// `dimensions` in the request body (adaptive — some providers
/// reject it); the response's vector dim count is compared
/// against the alias's declared `dimensions`. Mismatch is a
/// fatal error (strict verify).
///
/// Returns `Ok(())` if:
/// - `probe = false` (skip probe, trust config)
/// - the probe call succeeded AND response dim == dimensions
///
/// Returns `Err(message)` if:
/// - the probe call failed (HTTP 4xx/5xx, network error, timeout)
/// - the response dim count does not match `dimensions`
pub async fn run_indexing_probe(
    provider: &crate::provider::OpenAICompatibleProvider,
    model_id: &str,
    dimensions: u32,
    probe_enabled: bool,
) -> Result<(), String> {
    if !probe_enabled {
        log::info!("Indexing probe disabled (probe = false). Trusting config.");
        return Ok(());
    }
    log::info!(
        "Probing indexing endpoint for model '{model_id}' (1 POST /v1/embeddings, ~30s timeout)..."
    );
    match provider.probe_embedding(model_id).await {
        Ok(response_dim) => {
            if response_dim as u32 != dimensions {
                let msg = format!(
                    "Error: Probe indexing dim mismatch: alias declares dimensions={dimensions}, \
                     but provider returned {response_dim} dimensions for model '{model_id}'. \
                     The model may not support Matryoshka truncation, or the alias is \
                     misconfigured.\n\
                     \n\
                     To fix:\n\
                     - If the model naturally returns {response_dim} dimensions, update the \
                       alias's `dimensions = {response_dim}` in models.toml.\n\
                     - If the alias should use Matryoshka truncation to {dimensions} dims, \
                       verify the model server is configured for it.\n\
                     - Set [indexing].probe = false to skip the probe and trust the config."
                );
                log::error!("{}", msg);
                return Err(msg);
            }
            log::info!(
                "Indexing probe OK: provider serves /v1/embeddings for '{model_id}' \
                 with {response_dim} dimensions (matches alias's dimensions = {dimensions})"
            );
            Ok(())
        }
        Err(e) => {
            let base = provider.base_url().trim_end_matches("/v1").to_string();
            let msg = format!(
                "Error: Probe indexing call to provider at {base} with model '{model_id}' failed: {e}.\n\
                 \n\
                 Possible causes:\n\
                 1. The model is not loaded on the provider (cold start; wait and retry, or set [indexing].probe = false)\n\
                 2. The provider does not serve /v1/embeddings for this model\n\
                 3. The provider base_url is wrong in models.toml [provider.<name>].base_url\n\
                 4. The provider is unreachable (network error, server down)\n\
                 \n\
                 To fix:\n\
                 - Verify the model is served: curl {base}/v1/models | grep '{model_id}'\n\
                 - Verify the provider is reachable: curl {base}/v1/models\n\
                 - Set [indexing].probe = false in config.toml to skip the probe and trust the config"
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
    fn test_indexing_init_struct_construction() {
        let provider =
            crate::provider::OpenAICompatibleProvider::new_local("http://localhost", 11434);
        let init = IndexingInit {
            provider,
            model_id: "nomic-embed-text-v2-moe".to_string(),
            dimensions: 768,
            probe: true,
        };
        assert_eq!(init.model_id, "nomic-embed-text-v2-moe");
        assert_eq!(init.dimensions, 768);
        assert!(init.probe);
    }

    #[test]
    fn test_init_database_core_skip_persistence() {
        let provider =
            crate::provider::OpenAICompatibleProvider::new_local("http://localhost", 11434);
        let result = init_database_core(
            IndexingInit {
                provider,
                model_id: "test".to_string(),
                dimensions: 768,
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
    fn test_init_database_core_rejects_empty_model_id() {
        let provider =
            crate::provider::OpenAICompatibleProvider::new_local("http://localhost", 11434);
        let result = init_database_core(
            IndexingInit {
                provider,
                model_id: "".to_string(),
                dimensions: 768,
                probe: false,
            },
            false,
            false,
            None,
        );
        assert!(result.db.is_none());
        assert!(result.embedding.is_none());
        let detail = result.error_detail.expect("error_detail should be set");
        assert!(detail.contains("indexing model_id is empty"));
    }

    #[test]
    fn test_init_database_core_rejects_whitespace_model_id() {
        let provider =
            crate::provider::OpenAICompatibleProvider::new_local("http://localhost", 11434);
        let result = init_database_core(
            IndexingInit {
                provider,
                model_id: "   ".to_string(),
                dimensions: 768,
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
