//! CLI structures for translation subcommand
//!
//! Defines the TranslateArgs struct and related CLI parsing for the
//! `ask translate` subcommand.

use clap::{Args, Subcommand};

use crate::chat::ChatArgs;
use crate::diagnostics::embeddings::EmbeddingSource;
use crate::ocr::OcrArgs;
use crate::summarize::SummarizeArgs;
use crate::vision::VisionArgs;

/// Commands for the sprach CLI
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Translate text between languages using TranslateGemma
    #[command(visible_alias = "t")]
    Translate(TranslateArgs),

    /// Query an Ollama LLM model
    #[command(visible_alias = "q")]
    Query(QueryArgs),

    /// Extract text from images using GLM-OCR
    #[command(visible_alias = "o")]
    Ocr(OcrArgs),

    /// Summarize text using AI
    #[command(visible_alias = "sum")]
    Summarize(SummarizeArgs),

    /// Interactive chat with conversation history
    #[command(visible_alias = "c")]
    Chat(ChatArgs),

    /// Analyze and describe images using vision models
    #[command(visible_alias = "v")]
    Vision(VisionArgs),

    /// Diagnose embedding geometry and retrieval health
    #[command(visible_alias = "diag")]
    Diagnostics(DiagArgs),

    /// Generate shell completions
    #[command(about = "Generate shell completions for sprach")]
    Completion(CompletionArgs),

    /// Manage configuration files
    #[command(visible_alias = "cfg")]
    Config(ConfigArgs),

    /// Manage models.toml (provider config, model entries)
    #[command(visible_alias = "m")]
    Models(ModelsArgs),
}

/// Arguments for the translate subcommand
#[derive(Args, Debug, Clone)]
#[command(
    about = "Translate text between languages",
    long_about = r#"
Translate text using TranslateGemma model.

LANGUAGE FORMAT:
  source:target    - Explicit source and target (e.g., 'en:pt', 'he:en')
  :target          - Auto-detect source language (e.g., ':pt')
  target           - Auto-detect source language (e.g., 'pt')

LANGUAGE CODES:
  - Standard ISO codes: 'en', 'es', 'fr', 'de', 'pt', 'zh-Hans', 'zh-Hant', etc.
  - Names: 'english', 'spanish', 'portuguese', etc.
  - Shorthands: 'br' → pt-BR, 'us' → en-US

EXAMPLES:
  ask translate en:pt "Hello world"
  ask translate :pt "Hello world"              # Auto-detect source
  ask translate pt "Hello world"                 # Auto-detect source
  ask translate english:brazilian "Hello"        # Using names
  ask translate he:en "שלום"                   # Hebrew to English
  cat file.txt | ask translate en:pt
  ask translate en:pt --prompt formal "Hello"    # Formal style
  ask translate --list                           # List all languages
  ask translate --list port                      # Filter by 'port'
"#
)]
pub struct TranslateArgs {
    /// Language specification in format \[source:\]target
    /// Examples: en:pt, :pt, pt, english:brazilian
    #[arg(value_name = "LANGUAGE")]
    pub language: Option<String>,

    /// Text to translate. If not provided, reads from stdin.
    #[arg(value_name = "TEXT")]
    pub text: Option<String>,

    /// Translation style: formal, casual, technical, literary, or custom instruction
    /// Examples: formal, casual, technical, literary, "use Brazilian slang"
    #[arg(short, long, value_name = "STYLE")]
    pub prompt: Option<String>,

    /// List supported languages (optionally filter by substring)
    /// Example: --list, --list zh, --list port
    #[arg(long, value_name = "FILTER")]
    pub list: Option<Option<String>>,
}

/// Arguments for the query subcommand (original CLI behavior)
#[derive(Args, Debug, Clone)]
#[command(
    about = "Query an LLM model",
    long_about = r#"
Send a query to an LLM model.

EXAMPLES:
  ask query "What is Rust?"
  ask q "Explain async/await"
  ask -m lfm query "Hello"
  echo "text" | ask query
  ask -t query "Think deeply about this"
"#
)]
#[derive(Default)]
pub struct QueryArgs {
    /// The query to send to the model (optional, reads from stdin if not provided)
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,
}

/// Arguments for the diagnostics subcommand
#[derive(Args, Debug, Clone)]
#[command(
    about = "Diagnose embedding geometry and retrieval health",
    long_about = r#"
Analyze stored embedding vectors to assess retrieval quality.

Computes spectral metrics (d_eff, d̄, regime classification, variance
distribution) on all embedding vectors in the database. Reports whether
the embedding geometry supports effective vector search at standard
similarity thresholds.

By default, combines vectors from all sources (content, chunks, facts).
Use --source to analyze a specific source only.

The --db flag can be used before or after the subcommand:
  sprach --db /path/to/db diagnostics
  sprach diagnostics --db /path/to/db

EXAMPLES:
  sprach diagnostics
  sprach diagnostics --source facts
  sprach diagnostics --db /path/to/sprachspiel.db
  sprach --db /path/to/sprachspiel.db diagnostics
"#
)]
pub struct DiagArgs {
    /// Which embedding source to analyze
    ///
    /// If not specified, combines all sources (content, chunks, facts).
    #[arg(long, value_name = "SOURCE")]
    pub source: Option<EmbeddingSource>,

    /// Database path (overrides global --db)
    ///
    /// Use a specific SQLite database file for the embedding vectors.
    /// Overrides the global --db flag if both are specified.
    #[arg(long, value_name = "PATH")]
    pub db: Option<String>,
}

impl DiagArgs {
    /// Get the source filter, or None for all sources
    pub fn source_filter(&self) -> Option<EmbeddingSource> {
        self.source
    }
}

impl TranslateArgs {
    /// Check if this is a list-only operation
    #[cfg(test)]
    pub fn is_list_only(&self) -> bool {
        self.list.is_some() && self.language.is_none()
    }

    /// Get the list filter if provided
    #[cfg(test)]
    pub fn list_filter(&self) -> Option<&str> {
        self.list
            .as_ref()
            .and_then(|opt| opt.as_ref().map(|s| s.as_str()))
    }

    /// Validate that we have required arguments (language or list)
    pub fn validate(&self) -> Result<(), String> {
        if self.list.is_some() {
            return Ok(()); // List operation is valid on its own
        }

        if self.language.is_none() {
            return Err("Missing required argument: LANGUAGE. Use --help for usage.".to_string());
        }

        Ok(())
    }
}

use clap::ValueEnum;

/// Arguments for the `config` subcommand family
#[derive(Args, Debug, Clone)]
#[command(
    about = "Manage sprachspiel configuration",
    long_about = r#"
Subcommands for managing the user's `config.toml` file (e.g. merging
new fields added by newer versions of sprachspiel into an existing
configuration without overwriting user values).

EXAMPLES:
  sprach config upgrade               # Merge missing default fields
  sprach config upgrade --dry-run     # Preview changes without modifying
  sprach config upgrade --no-backup   # Skip creating a .bak file
"#
)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Subcommands available under `sprach config`
#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    /// Merge missing default fields into existing config.toml
    #[command(visible_alias = "up")]
    Upgrade(UpgradeArgs),
}

/// Arguments for `sprach config upgrade`
#[derive(Args, Debug, Clone)]
#[command(
    about = "Merge missing default fields into config.toml",
    long_about = r#"
Merge missing default fields into the existing configuration file at
~/.config/sprachspiel/config.toml (or $XDG_CONFIG_HOME/sprachspiel/).

This preserves all existing values and comments. New fields are
inserted with their default values and doc-comments extracted from
the sample configuration. The command is purely additive — it never
modifies or removes existing values.

A backup file (`config.toml.bak`, or `config.toml.bak.YYYYMMDD-HHMMSS`
if `.bak` already exists) is created by default. Use `--no-backup` to
skip the backup, or `--dry-run` to preview changes without modifying
the file.

EXAMPLES:
  sprach config upgrade               # Upgrade with backup
  sprach config upgrade --dry-run     # Show what would be added
  sprach config upgrade --no-backup   # Skip the backup file
"#
)]
pub struct UpgradeArgs {
    /// Show what would be added without modifying the file
    #[arg(long)]
    pub dry_run: bool,

    /// Skip creating a backup file
    #[arg(long)]
    pub no_backup: bool,
}

/// Arguments for the `sprach models` subcommand
#[derive(Args, Debug, Clone)]
pub struct ModelsArgs {
    #[command(subcommand)]
    pub action: ModelsAction,
}

/// Subcommands available under `sprach models`
#[derive(Subcommand, Debug, Clone)]
pub enum ModelsAction {
    /// Merge missing fields into models.toml (e.g., add `provider` to models
    /// that don't have one, create a default `[provider]` block if missing).
    #[command(visible_alias = "up")]
    Upgrade(ModelsUpgradeArgs),
}

/// Arguments for `sprach models upgrade`
#[derive(Args, Debug, Clone)]
#[command(
    about = "Migrate models.toml to current format (adds provider field, creates [provider] section)",
    long_about = r#"
Migrate ~/.config/sprachspiel/models.toml to the current format.

This command handles migrations that can't be done with simple field
defaults, specifically:

1. **Missing `[provider]` section**: If models.toml has no providers
   defined, a default `[provider."my-ollama"]` block is created with
   `base_url = "http://127.0.0.1:11434"`.

2. **Models without `provider` field**: Any model entry that doesn't
   reference a named provider gets `provider = "<first_available>"`
   added automatically (using the first provider defined in the file).

3. **Duplicate model names**: Detected but not auto-fixed — reported
   as warnings for the user to resolve manually.

The command is purely additive — it never modifies or removes existing
values. Provider field additions only happen for models that don't
have one already.

A backup file (`models.toml.bak`, or `models.toml.bak.YYYYMMDD-HHMMSS`
if `.bak` already exists) is created by default. Use `--no-backup` to
skip the backup, or `--dry-run` to preview changes without modifying
the file.

EXAMPLES:
  sprach models upgrade               # Migrate with backup
  sprach models upgrade --dry-run     # Show what would be added
  sprach models upgrade --no-backup   # Skip the backup file
"#
)]
pub struct ModelsUpgradeArgs {
    /// Show what would be added without modifying the file
    #[arg(long)]
    pub dry_run: bool,

    /// Skip creating a backup file
    #[arg(long)]
    pub no_backup: bool,
}

/// Shell types for completion generation
#[derive(ValueEnum, Clone, Debug)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    #[allow(clippy::enum_variant_names)]
    PowerShell,
    Elvish,
}

/// Arguments for shell completion generation
#[derive(Args, Debug, Clone)]
#[command(about = "Generate shell completions")]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

impl QueryArgs {
    /// Get query from args or stdin
    pub fn get_query(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref query) = self.query {
            return Ok(query.trim().to_string());
        }

        // Read from stdin
        use std::io::{self, Read};
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        Ok(input.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_args_is_list_only() {
        let args = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: Some(None),
        };
        assert!(args.is_list_only());

        let args2 = TranslateArgs {
            language: Some("en:pt".to_string()),
            text: None,
            prompt: None,
            list: Some(None),
        };
        assert!(!args2.is_list_only());
    }

    #[test]
    fn test_translate_args_list_filter() {
        let args = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: Some(Some("port".to_string())),
        };
        assert_eq!(args.list_filter(), Some("port"));

        let args2 = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: Some(None),
        };
        assert_eq!(args2.list_filter(), None);
    }

    #[test]
    fn test_translate_args_validate() {
        let args = TranslateArgs {
            language: Some("en:pt".to_string()),
            text: Some("Hello".to_string()),
            prompt: None,
            list: None,
        };
        assert!(args.validate().is_ok());

        let args2 = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: None,
        };
        assert!(args2.validate().is_err());

        let args3 = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: Some(None),
        };
        assert!(args3.validate().is_ok());
    }
}
