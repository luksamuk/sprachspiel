//! Embeddings module for vector embeddings generation
//!
//! Provides:
//! - Ollama embedding client
//! - Matryoshka truncation (768d → 256d)

mod client;
mod truncate;

pub use client::EmbeddingClient;
pub use truncate::truncate_and_normalize;