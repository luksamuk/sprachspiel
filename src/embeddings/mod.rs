//! Embeddings module for vector embeddings generation
//!
//! Provides:
//! - Ollama embedding client
//! - Matryoshka truncation (768d → 256d)
//! - Text chunking for long messages
//! - Embedding recovery for interrupted processes

mod chunker;
mod client;
mod recovery;
mod truncate;

pub use chunker::{chunk_text, needs_chunking};
pub use client::EmbeddingClient;
pub use recovery::recover_missing_embeddings;
#[allow(unused_imports)]
pub use truncate::truncate_and_normalize;
