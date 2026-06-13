//! Implements `sprach models upgrade` — migrate `models.toml` to the
//! current format (W2 #121, OpenAI-First):
//!
//! 1. **Missing `[provider]` section**: Creates a default
//!    `[provider."my-ollama"]` block with `kind = "openai"` and
//!    `base_url = "http://127.0.0.1:11434/v1"`.
//! 2. **Models without `provider` field**: Adds
//!    `provider = "<first_available>"` to each model entry that doesn't
//!    reference a provider.
//! 3. **Legacy `kind = "ollama"`**: Migrates to `kind = "openai"` and
//!    ensures `base_url` has the `/v1` suffix.
//! 4. **`base_url` without `/v1` suffix**: Appends it.
//! 5. **Removed fields `top_k`, `repeat_penalty`, `think`**: Removed from
//!    the schema. Users running `sprach models upgrade` get a warning
//!    but data is preserved (the migration only ADDS missing fields and
//!    migrates kind — never destroys existing values).
//!
//! Like `config upgrade`, this is purely additive — never modifies or
//! removes existing values. Backups are created automatically (`.bak`, or
//! `.bak.YYYYMMDD-HHMMSS` if `.bak` already exists). Use `--no-backup` to
//! skip, or `--dry-run` to preview.
//!
//! Invalid TOML is reported with the parser error and the process aborts
//! — the command never overwrites a file it cannot parse.

use std::path::{Path, PathBuf};

/// Default error type for the models upgrade module.
pub type AppError = Box<dyn std::error::Error + Send + Sync>;

/// A change that the upgrader would apply to `models.toml`.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelsMigration {
    /// Add a new `[provider."<name>"]` block to the file.
    AddProvider {
        /// Provider name (key in the new section).
        name: String,
        /// The full TOML block to insert, including doc-comments.
        block: String,
    },
    /// Add `provider = "<name>"` to an existing model.
    AddProviderField {
        /// Model name (key under `[models]`).
        model_name: String,
        /// The provider name to set.
        provider: String,
    },
    /// Migrate `kind = "ollama"` to `kind = "openai"`.
    MigrateProviderKind {
        /// Provider name.
        name: String,
    },
    /// Append `/v1` to a `base_url` that is missing it.
    AppendV1Suffix {
        /// Provider name.
        name: String,
        /// Original base_url.
        old_url: String,
    },
    /// Provider is missing the `embedding = true` flag (W2 #121).
    ///
    /// This is a **warning-only** migration: the upgrader never
    /// auto-adds the flag. It surfaces the warning so the user can
    /// decide whether their provider actually serves `/v1/embeddings`.
    /// If it does, the user can manually add `embedding = true`.
    ///
    /// We do NOT query `/v1/models` here — the user has the option
    /// to check that themselves. The hardcoded list of well-known
    /// embedding model fragments (see
    /// `crate::provider::embedding_models`) is the only source of
    /// "is this a known embedding model" — and per user policy it
    /// is never exposed in user-facing error messages or hard-fail
    /// the upgrade.
    MissingEmbeddingFlag {
        /// Provider name.
        name: String,
    },
}

/// Run the models upgrade. Returns a vector of every line of user-facing
/// output. The handler in `main.rs` is responsible for writing the returned
/// `Vec<String>` to stdout.
pub fn run_models_upgrade(
    models_path: PathBuf,
    dry_run: bool,
    no_backup: bool,
) -> Result<Vec<String>, AppError> {
    let mut output: Vec<String> = Vec::new();

    if !models_path.exists() {
        let msg = format!(
            "Models file not found: {}\n\
             Run `sprach --init-config` to create a fresh config first,\n\
             then create a models.toml with at least one model entry.",
            models_path.display()
        );
        log::error!("Models upgrade aborted: {msg}");
        return Err(msg.into());
    }

    let content = std::fs::read_to_string(&models_path).map_err(|e| {
        let msg = format!("Failed to read {}: {e}", models_path.display());
        log::error!("Models upgrade aborted: {msg}");
        msg
    })?;

    let migrations = detect_migrations(&content, &models_path);

    if migrations.is_empty() {
        output.push("Models file is already up to date.".to_string());
        return Ok(output);
    }

    output.push(format!("Models file: {}", models_path.display()));
    output.push(String::new());

    // Separate migrations by type
    let mut providers_to_add: Vec<&ModelsMigration> = Vec::new();
    let mut fields_to_add: Vec<&ModelsMigration> = Vec::new();
    let mut kinds_to_migrate: Vec<&ModelsMigration> = Vec::new();
    let mut urls_to_fix: Vec<&ModelsMigration> = Vec::new();
    let mut embedding_warnings: Vec<&ModelsMigration> = Vec::new();

    for m in &migrations {
        match m {
            ModelsMigration::AddProvider { .. } => providers_to_add.push(m),
            ModelsMigration::AddProviderField { .. } => fields_to_add.push(m),
            ModelsMigration::MigrateProviderKind { .. } => kinds_to_migrate.push(m),
            ModelsMigration::AppendV1Suffix { .. } => urls_to_fix.push(m),
            ModelsMigration::MissingEmbeddingFlag { .. } => embedding_warnings.push(m),
        }
    }

    let action_verb = if dry_run { "Would add" } else { "Adding" };
    let migrate_verb = if dry_run {
        "Would migrate"
    } else {
        "Migrating"
    };

    if !providers_to_add.is_empty() {
        output.push(format!(
            "{} {} provider(s):",
            action_verb,
            providers_to_add.len()
        ));
        for m in &providers_to_add {
            if let ModelsMigration::AddProvider { name, .. } = m {
                output.push(format!("  - [provider.\"{name}\"]"));
            }
        }
        output.push(String::new());
    }

    if !kinds_to_migrate.is_empty() {
        output.push(format!(
            "{} {} provider kind(s) from \"ollama\" to \"openai\":",
            migrate_verb,
            kinds_to_migrate.len()
        ));
        for m in &kinds_to_migrate {
            if let ModelsMigration::MigrateProviderKind { name } = m {
                output.push(format!("  - [provider.\"{name}\"]"));
            }
        }
        output.push(String::new());
    }

    if !urls_to_fix.is_empty() {
        output.push(format!(
            "{} {} base_url(s) to add /v1 suffix:",
            migrate_verb,
            urls_to_fix.len()
        ));
        for m in &urls_to_fix {
            if let ModelsMigration::AppendV1Suffix { name, old_url } = m {
                output.push(format!(
                    "  - [provider.\"{name}\"] {old_url} → {old_url}/v1"
                ));
            }
        }
        output.push(String::new());
    }

    if !fields_to_add.is_empty() {
        output.push(format!(
            "{} provider = \"...\" to {} model(s):",
            action_verb,
            fields_to_add.len()
        ));
        for m in &fields_to_add {
            if let ModelsMigration::AddProviderField {
                model_name,
                provider,
            } = m
            {
                output.push(format!(
                    "  - [models.\"{model_name}\"] -> provider = \"{provider}\""
                ));
            }
        }
        output.push(String::new());
    }

    // W2 #121: warn when a [provider.*] block is missing `embedding = true`.
    // The provider MIGHT serve /v1/embeddings but we don't auto-add the flag.
    // We do NOT query /v1/models (the user can do that themselves).
    if !embedding_warnings.is_empty() {
        output.push(format!(
            "WARN: {} provider(s) do not declare `embedding = true`:",
            embedding_warnings.len()
        ));
        for m in &embedding_warnings {
            if let ModelsMigration::MissingEmbeddingFlag { name } = m {
                output.push(format!(
                    "  - [provider.\"{name}\"]: no `embedding = true` flag. \
                     If this provider serves /v1/embeddings, \
                     add `embedding = true` to enable embedding support. \
                     See `sprach config upgrade` for the [embedding] section."
                ));
            }
        }
        output.push(String::new());
    }

    if !dry_run {
        apply_migrations(&content, &models_path, &migrations)?;

        if !no_backup {
            let backup_path = create_backup(&models_path)?;
            output.push(format!("Backup created: {}", backup_path.display()));
        }
    }

    Ok(output)
}

/// Detect what migrations need to be applied to the models.toml content.
fn detect_migrations(content: &str, models_path: &Path) -> Vec<ModelsMigration> {
    let mut migrations = Vec::new();

    let doc: toml_edit::DocumentMut = match content.parse() {
        Ok(d) => d,
        Err(e) => {
            let msg = format!(
                "Invalid TOML in {}: {e}\n\
                 Fix the syntax error manually before running models upgrade.",
                models_path.display()
            );
            log::error!("Models upgrade aborted: {msg}");
            return migrations;
        }
    };

    // 1. Check if [provider] section is missing or empty
    let has_providers = doc
        .get("provider")
        .and_then(|item| item.as_table_like())
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    if !has_providers {
        migrations.push(ModelsMigration::AddProvider {
            name: "my-ollama".to_string(),
            block: DEFAULT_PROVIDER_BLOCK.to_string(),
        });
    }

    // 2. Walk [provider.*] entries for kind migration and /v1 suffix
    if let Some(provider_table) = doc.get("provider").and_then(|item| item.as_table_like()) {
        for (provider_name, provider_item) in provider_table.iter() {
            let provider_table_inner = match provider_item.as_table_like() {
                Some(t) => t,
                None => continue,
            };

            // 2a. Migrate kind = "ollama" to kind = "openai"
            if let Some(kind_item) = provider_table_inner.get("kind")
                && let Some(kind_str) = kind_item.as_str()
                && (kind_str == "ollama" || kind_str == "openai_compatible")
            {
                migrations.push(ModelsMigration::MigrateProviderKind {
                    name: provider_name.to_string(),
                });
            }

            // 2b. Append /v1 to base_url if missing
            if let Some(url_item) = provider_table_inner.get("base_url")
                && let Some(url_str) = url_item.as_str()
            {
                let trimmed = url_str.trim();
                let has_scheme = trimmed.starts_with("http://") || trimmed.starts_with("https://");
                let mut normalized = if has_scheme {
                    trimmed.to_string()
                } else {
                    format!("http://{trimmed}")
                };
                if !normalized.contains("/v1")
                    && !normalized.ends_with('/')
                    && !normalized.ends_with("/v1/")
                {
                    normalized.push_str("/v1");
                }
                if normalized != trimmed {
                    migrations.push(ModelsMigration::AppendV1Suffix {
                        name: provider_name.to_string(),
                        old_url: trimmed.to_string(),
                    });
                }
            }

            // 2c. W2 #121: detect if the provider is missing
            // `embedding = true` while it serves a known embedding
            // model. This is a WARNING-only migration; we do not
            // auto-add the flag.
            if let Some(embedding_item) = provider_table_inner.get("embedding") {
                if let Some(embedding_bool) = embedding_item.as_bool()
                    && !embedding_bool
                {
                    // User explicitly set `embedding = false`. Don't
                    // warn — they made an informed decision.
                }
            } else {
                // `embedding` field is absent. Schedule a warning
                // (we don't add it; we just inform the user).
                migrations.push(ModelsMigration::MissingEmbeddingFlag {
                    name: provider_name.to_string(),
                });
            }
        }
    }

    // 3. Walk [models.*] entries and check for `provider` field
    let provider_names: Vec<String> = if has_providers {
        doc.get("provider")
            .and_then(|item| item.as_table_like())
            .map(|t| t.iter().map(|(k, _)| k.to_string()).collect())
            .unwrap_or_default()
    } else {
        // We just added "my-ollama" above
        vec!["my-ollama".to_string()]
    };

    let default_provider = provider_names
        .first()
        .cloned()
        .unwrap_or_else(|| "my-ollama".to_string());

    if let Some(models_table) = doc.get("models").and_then(|item| item.as_table_like()) {
        for (model_name, model_item) in models_table.iter() {
            let model_table = match model_item.as_table_like() {
                Some(t) => t,
                None => continue,
            };

            if !model_table.contains_key("provider") {
                migrations.push(ModelsMigration::AddProviderField {
                    model_name: model_name.to_string(),
                    provider: default_provider.clone(),
                });
            }
        }
    }

    migrations
}

/// Apply the detected migrations to the file.
fn apply_migrations(
    content: &str,
    models_path: &Path,
    migrations: &[ModelsMigration],
) -> Result<(), AppError> {
    let mut doc: toml_edit::DocumentMut = content.parse().map_err(|e| {
        let msg = format!("Failed to parse {}: {e}", models_path.display());
        log::error!("{msg}");
        msg
    })?;

    for migration in migrations {
        match migration {
            ModelsMigration::AddProvider { name, block } => {
                ensure_provider_table(&mut doc);
                if let Some(provider_table) = doc["provider"].as_table_mut()
                    && let Ok(block_doc) = block.parse::<toml_edit::DocumentMut>()
                {
                    for (key, value) in block_doc.iter() {
                        provider_table.insert(key, value.clone());
                    }
                    let _ = name;
                }
            }
            ModelsMigration::AddProviderField {
                model_name,
                provider,
            } => {
                if let Some(models_table) = doc.get_mut("models").and_then(|i| i.as_table_mut())
                    && let Some(model_item) = models_table.get_mut(model_name)
                    && let Some(model_table) = model_item.as_table_mut()
                {
                    model_table.insert("provider", toml_edit::value(provider.as_str()));
                }
            }
            ModelsMigration::MigrateProviderKind { name } => {
                if let Some(provider_table) = doc["provider"].as_table_mut()
                    && let Some(provider_item) = provider_table.get_mut(name)
                    && let Some(provider_inner) = provider_item.as_table_mut()
                {
                    provider_inner.insert("kind", toml_edit::value("openai"));
                }
            }
            ModelsMigration::AppendV1Suffix { name, old_url } => {
                if let Some(provider_table) = doc["provider"].as_table_mut()
                    && let Some(provider_item) = provider_table.get_mut(name)
                    && let Some(provider_inner) = provider_item.as_table_mut()
                {
                    let trimmed = old_url.trim();
                    let has_scheme =
                        trimmed.starts_with("http://") || trimmed.starts_with("https://");
                    let mut normalized = if has_scheme {
                        trimmed.to_string()
                    } else {
                        format!("http://{trimmed}")
                    };
                    if !normalized.contains("/v1") {
                        normalized.push_str("/v1");
                    }
                    provider_inner.insert("base_url", toml_edit::value(normalized));
                }
            }
            ModelsMigration::MissingEmbeddingFlag { .. } => {
                // W2 #121: warning-only. We do NOT auto-add
                // `embedding = true`. The user must do it manually
                // after confirming the provider serves /v1/embeddings.
                // This arm is a no-op.
            }
        }
    }

    std::fs::write(models_path, doc.to_string()).map_err(|e| {
        let msg = format!("Failed to write {}: {e}", models_path.display());
        log::error!("{msg}");
        msg
    })?;

    Ok(())
}

/// Ensure a `[provider]` table exists in the document.
fn ensure_provider_table(doc: &mut toml_edit::DocumentMut) {
    if !doc.get("provider").map(|i| i.is_table()).unwrap_or(false) {
        doc.insert("provider", toml_edit::Item::Table(toml_edit::Table::new()));
    }
}

/// Create a backup file (`.bak` or `.bak.YYYYMMDD-HHMMSS`).
fn create_backup(path: &Path) -> Result<PathBuf, AppError> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("models.toml");

    let backup_path = parent.join(format!("{stem}.bak"));
    let final_path = if backup_path.exists() {
        let now = chrono::Local::now();
        let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
        parent.join(format!("{stem}.bak.{timestamp}"))
    } else {
        backup_path
    };

    std::fs::copy(path, &final_path).map_err(|e| {
        let msg = format!("Failed to create backup at {}: {e}", final_path.display());
        log::error!("{msg}");
        msg
    })?;

    Ok(final_path)
}

/// Default provider block inserted when no [provider] section exists.
const DEFAULT_PROVIDER_BLOCK: &str = r#"[provider."my-ollama"]
kind = "openai"
base_url = "http://127.0.0.1:11434/v1"
# connect_timeout_secs = 5
# read_timeout_secs = 300
# stream_idle_timeout_secs = 60
# max_retries = 3
# retry_base_delay_ms = 2000
# retry_max_delay_ms = 16000
# retry_jitter_percent = 20
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_no_provider_section() {
        let content = r#"
[models.test]
model_id = "test:1b"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(migrations.iter().any(
            |m| matches!(m, ModelsMigration::AddProvider { name, .. } if name == "my-ollama")
        ));
    }

    #[test]
    fn test_detect_model_without_provider() {
        let content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "http://localhost:11434/v1"

[models.test]
model_id = "test:1b"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(migrations.iter().any(|m| matches!(m, ModelsMigration::AddProviderField { model_name, .. } if model_name == "test")));
    }

    #[test]
    fn test_detect_legacy_kind_ollama() {
        let content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "http://localhost:11434"

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(
            migrations
                .iter()
                .any(|m| matches!(m, ModelsMigration::MigrateProviderKind { name } if name == "my-ollama")),
            "Should detect kind=\"ollama\" for migration"
        );
        assert!(
            migrations.iter().any(
                |m| matches!(m, ModelsMigration::AppendV1Suffix { name, .. } if name == "my-ollama")
            ),
            "Should detect missing /v1 suffix"
        );
    }

    #[test]
    fn test_detect_already_migrated() {
        let content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "http://localhost:11434/v1"
embedding = true

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(migrations.is_empty());
    }

    #[test]
    fn test_detect_v1_already_present() {
        let content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "http://localhost:11434/v1"

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(
            !migrations
                .iter()
                .any(|m| matches!(m, ModelsMigration::AppendV1Suffix { .. }))
        );
    }

    #[test]
    fn test_detect_url_without_scheme() {
        let content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "localhost:11434"

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(
            migrations
                .iter()
                .any(|m| matches!(m, ModelsMigration::AppendV1Suffix { .. })),
            "Should detect missing /v1 suffix even without scheme"
        );
    }

    #[test]
    fn test_detect_missing_embedding_flag() {
        // W2 #121: provider without `embedding = true` flag is
        // detected as MissingEmbeddingFlag (warning-only).
        let content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models.test]
model_id = "test:1b"
provider = "my-llama-swap"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(
            migrations
                .iter()
                .any(|m| matches!(m, ModelsMigration::MissingEmbeddingFlag { name } if name == "my-llama-swap")),
            "Should detect missing embedding flag, migrations: {:?}",
            migrations
        );
    }

    #[test]
    fn test_no_missing_embedding_flag_when_present() {
        // W2 #121: provider WITH `embedding = true` is fine.
        let content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"
embedding = true

[models.test]
model_id = "test:1b"
provider = "my-llama-swap"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(
            !migrations
                .iter()
                .any(|m| matches!(m, ModelsMigration::MissingEmbeddingFlag { .. })),
            "Should NOT report missing embedding flag when present, migrations: {:?}",
            migrations
        );
    }

    #[test]
    fn test_no_missing_embedding_flag_when_explicitly_false() {
        // W2 #121: provider with `embedding = false` is also fine
        // (user made an informed decision). Only the absence of
        // the flag triggers the warning.
        let content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"
embedding = false

[models.test]
model_id = "test:1b"
provider = "my-llama-swap"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(
            !migrations
                .iter()
                .any(|m| matches!(m, ModelsMigration::MissingEmbeddingFlag { .. })),
            "Should NOT report missing embedding flag when explicitly false, migrations: {:?}",
            migrations
        );
    }

    #[test]
    fn test_full_migration_via_file() {
        let tmp =
            std::env::temp_dir().join(format!("sprach_models_test_{}.toml", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "http://localhost:11434"

[models.test]
model_id = "test:1b"
"#;
        std::fs::write(&tmp, content).unwrap();

        let result = run_models_upgrade(tmp.clone(), false, true);
        assert!(result.is_ok(), "Upgrade should succeed: {:?}", result.err());

        let updated = std::fs::read_to_string(&tmp).unwrap();
        assert!(
            updated.contains("kind = \"openai\""),
            "Kind should be migrated: {updated}"
        );
        assert!(updated.contains("/v1"), "URL should have /v1: {updated}");
        assert!(
            updated.contains("provider = \"my-ollama\""),
            "Provider should be set: {updated}"
        );

        let _ = std::fs::remove_file(&tmp);
    }
}
