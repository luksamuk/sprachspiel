//! Database module for semantic search storage
//!
//! Provides SQLite storage with sqlite-vec extension for vector embeddings.

mod connection;
mod init;
mod operations;
mod query;
mod schema;

pub use connection::Database;
pub use init::init_database_core;
pub use operations::{ConversationMetadataParams, SourceType, TodoRow, fts5_escape};
pub use query::WhereBuilder;

/// Initialize sqlite-vec extension globally.
/// Must be called once at startup before any database operations.
pub fn init() {
    connection::Database::init_sqlite_vec();
}
