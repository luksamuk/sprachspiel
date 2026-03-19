//! Ask-AI library module
//!
//! Exposes internal modules for testing and external use.

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
pub mod macros;
pub mod markdown;
pub mod ocr;
pub mod platform;
pub mod project;
pub mod prompts;
pub mod query;
pub mod retrieval;
pub mod settings;
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
