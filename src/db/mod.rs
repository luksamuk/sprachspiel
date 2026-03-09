//! Database module for semantic search storage
//!
//! Provides SQLite storage with sqlite-vec extension for vector embeddings.

mod connection;
mod legacy_check;
mod migration;
mod operations;
mod schema;

pub use connection::Database;
pub use legacy_check::{
    migrate_all_legacy_sessions, restore_session, LegacySession, MigrationStats,
};
pub use migration::{migrate_session, reindex_conversation};
pub use operations::{
    reciprocal_rank_fusion, ConversationMetadata, SearchResult, SearchType, SourceType, TodoRow,
};

/// Initialize sqlite-vec extension globally.
/// Must be called once at startup before any database operations.
pub fn init() {
    connection::Database::init_sqlite_vec();
}
