//! Summarize module
//!
//! Provides text summarization functionality using AI with tools disabled.
//! Uses mistral-small model by default for optimal summarization quality.

pub mod cli;
pub mod processor;

// Re-export commonly used items
pub use cli::SummarizeArgs;
pub use processor::SummarizeProcessor;
