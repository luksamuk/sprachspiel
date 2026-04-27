//! Summarize module
//!
//! Provides text summarization functionality using AI with tools disabled.
//! Model is resolved from config.toml \[model.summarize\] or \[model\] default settings.

pub mod cli;
pub mod processor;

// Re-export commonly used items
pub use cli::SummarizeArgs;
pub use processor::SummarizeProcessor;
