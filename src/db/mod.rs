//! Database module for semantic search storage
//!
//! Provides SQLite storage with sqlite-vec extension for vector embeddings.

mod connection;
mod operations;
mod schema;

pub use connection::Database;
pub use operations::{SearchResult, SearchType};

/// Initialize sqlite-vec extension globally.
/// Must be called once at startup before any database operations.
pub fn init() {
    connection::Database::init_sqlite_vec();
}
