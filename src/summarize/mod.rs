//! Summarize module
//!
//! Provides text summarization functionality using AI with tools disabled.
//! Uses qwen3.5:4b model by default for optimal summarization quality.

pub mod cli;
pub mod processor;

// Re-export commonly used items
pub use cli::SummarizeArgs;
pub use processor::SummarizeProcessor;
