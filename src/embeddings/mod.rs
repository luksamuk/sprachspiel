//! Embeddings module for vector embeddings generation
//!
//! Provides:
//! - LLM server embedding client
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

/// Minimum content length (in bytes) for meaningful embedding generation.
///
/// Content shorter than this threshold produces embeddings with too little
/// semantic signal to be useful in vector search. Items below this limit
/// are excluded from recovery/reindex queries and will never receive
/// embeddings. This prevents the infinite recovery loop where short items
/// are found by `WHERE has_embedding = 0`, skipped, and left at `has_embedding = 0`
/// on every startup.
///
/// This constant is used in:
/// - SQL queries (`WHERE length(content) >= MIN_EMBED_CONTENT_LEN`)
/// - Rust defenses in `regenerate.rs` and `recovery.rs`
pub const MIN_EMBED_CONTENT_LEN: usize = 10;

pub use chunk_config::DynamicChunkConfig;
pub use chunker::{ChunkConfig, chunk_text, chunk_text_with_config, needs_chunking};
pub use client::{
    DEFAULT_CONTEXT_LENGTH, DEFAULT_EMBEDDING_MODEL, EmbeddingClient, EmbeddingError,
};
pub use fallback::{
    EmbedContext, EmbedItemContext, embed_chunk_with_fallback, embed_item_with_fallback,
};
pub use recovery::recover_missing_embeddings;
pub use regenerate::regenerate_all_embeddings;
pub use truncate::{TRUNCATED_DIMENSIONS, TruncateResult, cosine_similarity};
