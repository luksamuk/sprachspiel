//! Content System Module
//!
//! Unified storage for messages, notes, and documents.
//!
//! # Architecture
//!
//! The content system uses a unified `content_items` table that stores:
//! - Messages (from chat sessions)
//! - Notes (user-created persistent notes)
//! - Documents (imported files)
//!
//! All content types benefit from:
//! - FTS5 full-text search
//! - Vector embeddings for semantic search
//! - Decay-based relevance scoring
//!
//! # Usage
//!
//! ```ignore
//! use crate::content::types::{Note, ContentScope, ContentSource};
//! use crate::content::document::{Document, FileType, detect_file_type};
//! use crate::content::db::Database;
//!
//! let note = Note::new(
//!     "Important note content".to_string(),
//!     ContentScope::Project,
//!     Some("my-project".to_string()),
//!     ContentSource::User,
//!     Some("Title".to_string()),
//! )?;
//!
//! let id = db.insert_note(&note)?;
//! let retrieved = db.get_note(id)?;
//! ```

pub mod db;
pub mod document;
pub mod types;
pub mod decay;

pub use db::ContentSearchParams;
pub use document::{Document, FileType, MAX_DOCUMENT_SIZE, detect_file_type};
pub use types::{
    ContentScope, ContentSearchResult, ContentSearchType, ContentSource, ContentType,
    MAX_NOTE_CONTENT_SIZE, Note,
};
