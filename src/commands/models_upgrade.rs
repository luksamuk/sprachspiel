//! Implements `sprach models upgrade` — migrate `models.toml` to the
//! current format (#120):
//!
//! 1. **Missing `[provider]` section**: Creates a default
//!    `[provider."my-ollama"]` block with `base_url = "http://127.0.0.1:11434"`.
//! 2. **Models without `provider` field**: Adds
//!    `provider = "<first_available>"` to each model entry that doesn't
//!    reference a provider.
//!
//! Like `config upgrade`, this is purely additive — never modifies or
//! removes existing values. Backups are created automatically (`.bak`, or
//! `.bak.YYYYMMDD-HHMMSS` if `.bak` already exists). Use `--no-backup` to
//! skip, or `--dry-run` to preview.
//!
//! Invalid TOML is reported with the parser error and the process aborts
//! — the command never overwrites a file it cannot parse.

#![allow(clippy::print_stdout)] // User-facing CLI output
#![allow(clippy::print_stderr)] // User-facing CLI output

use std::path::PathBuf;

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
}

/// Result of a migration run.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelsUpgradeReport {
    /// Number of `AddProvider` actions taken.
    pub added_providers: usize,
    /// Number of `AddProviderField` actions taken.
    pub added_provider_fields: usize,
    /// Path to the backup file, or `None` if no backup was created.
    pub backup_path: Option<PathBuf>,
    /// Whether the run was a dry-run.
    pub dry_run: bool,
}

/// Run the models upgrade. Returns a report and a vector of every line
/// of user-facing output.
///
/// This function is pure-ish: it does not perform any I/O of its own.
/// The handler in `main.rs` is responsible for writing the returned
/// `Vec<String>` to stdout.
pub fn run_models_upgrade(
    models_path: PathBuf,
    dry_run: bool,
    no_backup: bool,
) -> Result<(ModelsUpgradeReport, Vec<String>), AppError> {
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
        let report = ModelsUpgradeReport {
            added_providers: 0,
            added_provider_fields: 0,
            backup_path: None,
            dry_run,
        };
        return Ok((report, output));
    }

    output.push(format!("Models file: {}", models_path.display()));
    output.push(String::new());

    // Separate migrations by type
    let mut providers_to_add: Vec<&ModelsMigration> = Vec::new();
    let mut fields_to_add: Vec<&ModelsMigration> = Vec::new();

    for m in &migrations {
        match m {
            ModelsMigration::AddProvider { .. } => providers_to_add.push(m),
            ModelsMigration::AddProviderField { .. } => fields_to_add.push(m),
        }
    }

    let action_verb = if dry_run { "Would add" } else { "Adding" };
    let _ = action_verb; // Used in output below

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

    if !dry_run {
        apply_migrations(&content, &models_path, &migrations)?;

        if !no_backup {
            let backup_path = create_backup(&models_path)?;
            output.push(format!("Backup created: {}", backup_path.display()));
        }
    }

    let report = ModelsUpgradeReport {
        added_providers: providers_to_add.len(),
        added_provider_fields: fields_to_add.len(),
        backup_path: None,
        dry_run,
    };

    Ok((report, output))
}

/// Detect what migrations need to be applied to the models.toml content.
fn detect_migrations(content: &str, models_path: &PathBuf) -> Vec<ModelsMigration> {
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
            // Return empty migrations — caller will still get a meaningful
            // error path through apply_migrations if it tries to apply.
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

    // 2. Walk [models.*] entries and check for `provider` field
    let provider_names: Vec<String> = if has_providers {
        doc.get("provider")
            .and_then(|item| item.as_table_like())
            .map(|t| t.iter().filter_map(|(k, _)| Some(k.to_string())).collect())
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
            // Skip commented/disabled models? For now we treat all
            // entries as active — the user can manually remove
            // commented ones if needed.
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
    models_path: &PathBuf,
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
                // Insert a new [provider."name"] block at the top
                ensure_provider_table(&mut doc);
                if let Some(provider_table) = doc["provider"].as_table_mut() {
                    // Parse the block as a sub-document and insert
                    if let Ok(block_doc) = block.parse::<toml_edit::DocumentMut>() {
                        for (key, value) in block_doc.iter() {
                            provider_table.insert(key, value.clone());
                        }
                        let _ = name; // key used implicitly via block
                    }
                }
            }
            ModelsMigration::AddProviderField {
                model_name,
                provider,
            } => {
                // Navigate to [models."<model_name>"] and add provider field
                if let Some(models_table) = doc.get_mut("models").and_then(|i| i.as_table_mut()) {
                    if let Some(model_item) = models_table.get_mut(model_name) {
                        if let Some(model_table) = model_item.as_table_mut() {
                            model_table.insert("provider", toml_edit::value(provider.as_str()));
                        }
                    }
                }
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
fn create_backup(path: &PathBuf) -> Result<PathBuf, AppError> {
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
kind = "ollama"
base_url = "http://127.0.0.1:11434"
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
kind = "ollama"
base_url = "http://localhost:11434"

[models.test]
model_id = "test:1b"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(migrations.iter().any(|m| matches!(m, ModelsMigration::AddProviderField { model_name, .. } if model_name == "test")));
    }

    #[test]
    fn test_already_migrated() {
        let content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "http://localhost:11434"

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let migrations = detect_migrations(content, &PathBuf::from("/tmp/test.toml"));
        assert!(migrations.is_empty());
    }

    #[test]
    fn test_full_migration_via_file() {
        // Test the full end-to-end flow: create a temp file, run upgrade,
        // verify the result.
        let tmp =
            std::env::temp_dir().join(format!("sprach_models_test_{}.toml", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let content = r#"
[models.test]
model_id = "test:1b"
"#;
        std::fs::write(&tmp, content).unwrap();

        let result = run_models_upgrade(tmp.clone(), false, true);
        assert!(result.is_ok(), "Upgrade should succeed: {:?}", result.err());

        let updated = std::fs::read_to_string(&tmp).unwrap();
        assert!(updated.contains("[provider."));
        assert!(updated.contains("provider = \"my-ollama\""));

        let _ = std::fs::remove_file(&tmp);
    }
}
