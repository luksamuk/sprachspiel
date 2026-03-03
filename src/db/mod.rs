//! Database module for semantic search storage
//!
//! Provides SQLite storage with sqlite-vec extension for vector embeddings.

mod connection;
mod migration;
mod operations;
mod schema;

pub use connection::Database;
pub use migration::{migrate_project, migrate_session, reindex_conversation};
pub use operations::{SearchResult, SearchType, reciprocal_rank_fusion};

/// Initialize sqlite-vec extension globally.
/// Must be called once at startup before any database operations.
pub fn init() {
    connection::Database::init_sqlite_vec();
}
