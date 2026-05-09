//! Sprachspiel library module
//!
//! Exposes internal modules for testing and external use.
//!
//! TODO(TUI): Remove crate-level expect(print_stdout/print_stderr) when
//! migrating to TUI. Each module should then decide whether to print directly
//! or delegate to the view layer.

#![expect(clippy::print_stdout)]
#![expect(clippy::print_stderr)]

pub mod capabilities;
pub mod chat;
pub mod config;
pub mod consts;
pub mod content;
pub mod context;
pub mod context_overflow;
pub mod db;
pub mod debug_tools;
pub mod embeddings;
pub mod external;
pub mod facts;
pub mod feedback;
pub mod logging;
pub mod macros;
pub mod markdown;
pub mod ocr;
pub mod platform;
pub mod project;
pub mod prompts;
pub mod query;
pub mod retrieval;
pub mod security;
pub mod settings;
pub mod skills;
pub mod soul;
pub mod spinner;
pub mod summarize;
pub mod tokens;
pub mod tool_robustness;
pub mod tools;
pub mod translate;
pub mod user_models;
pub mod utils;
pub mod vision;

/// Result type alias for application errors
pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
