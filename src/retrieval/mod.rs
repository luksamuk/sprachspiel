//! Retrieval module for semantic search
//!
//! Provides hybrid search (BM25 + semantic + RRF) for conversation history
//! and context building for optimal LLM context composition.

mod context_builder;
mod search;

pub use context_builder::{build_context, update_retrieval_time, ContextResult, RetrievalConfig};
pub use search::run_search;