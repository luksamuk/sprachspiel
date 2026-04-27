//! Vision module
//!
//! Provides image analysis functionality using vision models via Ollama.
//! Supports single and multi-image processing, plus PDF page-by-page analysis.

pub mod cli;
pub mod error;
pub mod processor;

pub use cli::VisionArgs;
pub use processor::{VisionProcessor, print_results};