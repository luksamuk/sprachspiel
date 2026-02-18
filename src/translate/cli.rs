//! CLI structures for translation subcommand
//!
//! Defines the TranslateArgs struct and related CLI parsing for the
//! `ask translate` subcommand.

use clap::{Args, Subcommand};

use crate::ocr::OcrArgs;
use crate::summarize::SummarizeArgs;

/// Commands for the ask-ai CLI
#[derive(Subcommand, Debug)]
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

    /// Generate shell completions
    #[command(about = "Generate shell completions for ask-ai")]
    Completion(CompletionArgs),
}

/// Arguments for the translate subcommand
#[derive(Args, Debug)]
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
    /// Language specification in format [source:]target
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

    /// Output plain text without formatting
    #[arg(long)]
    pub plain: bool,

    /// Enable debug mode with detailed logging
    #[arg(short, long)]
    pub debug: bool,
}

/// Arguments for the query subcommand (original CLI behavior)
#[derive(Args, Debug)]
#[command(
    about = "Query an Ollama LLM model",
    long_about = r#"
Send a query to an Ollama LLM model.

EXAMPLES:
  ask query "What is Rust?"
  ask q "Explain async/await"
  ask -m lfm "Hello"
  echo "text" | ask query
  ask query -t "Think deeply about this"
"#
)]
pub struct QueryArgs {
    /// The query to send to the model (optional, reads from stdin if not provided)
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,

    /// Model preset to use
    #[arg(short, long, default_value = "lfm", value_name = "MODEL")]
    pub model: String,

    /// System prompt mode
    #[arg(short, long, default_value = "default", value_name = "PROMPT")]
    pub prompt: String,

    /// Enable think mode for models that support it
    #[arg(short, long)]
    pub think: bool,

    /// Output plain text without markdown formatting
    #[arg(long)]
    pub plain: bool,

    /// Dry-run mode: print config without executing
    #[arg(short, long)]
    pub debug: bool,

    /// Force enable tools even if model doesn't advertise tool support
    #[arg(long)]
    pub tools: bool,

    /// Code mode: optimize response for code output
    #[arg(short, long)]
    pub code: bool,

    /// Ignore AGENTS.md file if present in current directory
    #[arg(long)]
    pub ignore_agents: bool,
}

impl Default for QueryArgs {
    fn default() -> Self {
        Self {
            query: None,
            model: "lfm".to_string(),
            prompt: "default".to_string(),
            think: false,
            plain: false,
            debug: false,
            tools: false,
            code: false,
            ignore_agents: false,
        }
    }
}

impl TranslateArgs {
    /// Check if this is a list-only operation
    #[allow(dead_code)]
    pub fn is_list_only(&self) -> bool {
        self.list.is_some() && self.language.is_none()
    }

    /// Get the list filter if provided
    #[allow(dead_code)]
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
#[derive(Args, Debug)]
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
            plain: false,
            debug: false,
        };
        assert!(args.is_list_only());

        let args2 = TranslateArgs {
            language: Some("en:pt".to_string()),
            text: None,
            prompt: None,
            list: Some(None),
            plain: false,
            debug: false,
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
            plain: false,
            debug: false,
        };
        assert_eq!(args.list_filter(), Some("port"));

        let args2 = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: Some(None),
            plain: false,
            debug: false,
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
            plain: false,
            debug: false,
        };
        assert!(args.validate().is_ok());

        let args2 = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: None,
            plain: false,
            debug: false,
        };
        assert!(args2.validate().is_err());

        let args3 = TranslateArgs {
            language: None,
            text: None,
            prompt: None,
            list: Some(None),
            plain: false,
            debug: false,
        };
        assert!(args3.validate().is_ok());
    }
}
