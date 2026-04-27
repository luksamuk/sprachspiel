//! Vision module
//!
//! Provides image analysis functionality using vision models via Ollama.
//! Supports single and multi-image processing.

pub mod cli;
pub mod error;
pub mod processor;

pub use cli::VisionArgs;
pub use processor::{VisionProcessor, print_results};
