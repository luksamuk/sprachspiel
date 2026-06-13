//! Database module for semantic search storage
//!
//! Provides SQLite storage with sqlite-vec extension for vector embeddings.

mod blob;
mod connection;
mod init;
mod operations;
mod query;
pub mod schema;

pub mod content_decay_ops;
pub mod feedback_ops;

pub use blob::{blob_to_f32_vec, embedding_to_le_bytes};
pub use connection::Database;
pub use init::{EmbeddingInit, init_database_core, run_embedding_probe};
pub use operations::{ConversationMetadataParams, SourceType, TodoRow, fts5_escape};
pub use query::WhereBuilder;

/// Initialize sqlite-vec extension globally.
/// Must be called once at startup before any database operations.
pub fn init() {
    connection::Database::init_sqlite_vec();
}
