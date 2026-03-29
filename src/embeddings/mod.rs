//! Embeddings module for vector embeddings generation
//!
//! Provides:
//! - Ollama embedding client
//! - Matryoshka truncation (768d → 256d)
//! - Text chunking for long messages
//! - Embedding recovery for interrupted processes
//! - Embedding regeneration after schema migration
//! - Dynamic chunk configuration based on model context
//! - Fallback with recursive chunking for oversized content

mod chunk_config;
mod chunker;
mod client;
mod fallback;
mod regenerate;
mod recovery;
mod truncate;

pub use chunk_config::DynamicChunkConfig;
pub use chunker::{chunk_text, chunk_text_with_config, needs_chunking, ChunkConfig};
pub use client::{EmbeddingClient, DEFAULT_CONTEXT_LENGTH};
pub use fallback::{embed_chunk_with_fallback, embed_item_with_fallback, EmbedContext, EmbedItemContext};
pub use regenerate::regenerate_all_embeddings;
pub use recovery::{recover_missing_embeddings, recover_missing_embeddings_with_progress};
#[allow(unused_imports)]
pub use truncate::truncate_and_normalize;
