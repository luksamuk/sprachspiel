//! OCR module
//!
//! Provides OCR functionality using GLM-OCR model via Ollama.
//! Supports text, table, figure, and formula extraction from images.

pub mod cli;
pub mod error;
pub mod mode;
pub mod processor;

// Re-export commonly used items
pub use cli::OcrArgs;
pub use processor::{OcrProcessor, print_results};
