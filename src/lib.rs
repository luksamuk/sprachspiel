//! Sprachspiel library module
//!
//! Exposes internal modules for testing and external use.
//!
//! # Print usage
//!
//! Each module declares its own `#![expect(clippy::print_stdout)]` and/or
//! `#![expect(clippy::print_stderr)]` with a justification comment. Modules
//! that have been migrated to the `ChatView` pattern (e.g., `chat/`) have
//! their expects only on the rendering layer (`view/terminal.rs`) and
//! terminal control modules (`repl.rs`). Other modules use direct print
//! calls for CLI output and declare expects locally.

pub mod capabilities;
pub mod chat;
pub mod clipboard;
pub mod config;
pub mod consts;
pub mod content;
pub mod context;
pub mod context_overflow;
pub mod db;
pub mod debug_tools;
pub mod diagnostics;
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
pub mod retry;
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
