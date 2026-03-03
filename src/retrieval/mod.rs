//! Retrieval module for semantic search
//!
//! Provides hybrid search (BM25 + semantic + RRF) for conversation history.

mod search;

pub use search::run_search;