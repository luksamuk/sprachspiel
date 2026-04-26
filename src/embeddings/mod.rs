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
mod recovery;
mod regenerate;
mod truncate;

pub use chunk_config::DynamicChunkConfig;
pub use chunker::{ChunkConfig, chunk_text, chunk_text_with_config, needs_chunking};
pub use client::{DEFAULT_CONTEXT_LENGTH, EmbeddingClient, EmbeddingError};
pub use fallback::{
    EmbedContext, EmbedItemContext, embed_chunk_with_fallback, embed_item_with_fallback,
};
pub use recovery::{recover_missing_embeddings, recover_missing_embeddings_with_progress};
pub use regenerate::regenerate_all_embeddings;
pub use truncate::cosine_similarity;
