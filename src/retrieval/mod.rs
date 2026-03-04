//! Retrieval module for semantic search
//!
//! Provides hybrid search (BM25 + semantic + RRF) for conversation history
//! and context building for optimal LLM context composition.

mod context_builder;
mod search;

pub use context_builder::{build_context, build_query_context, update_retrieval_time, RetrievalConfig};
// ContextResult is public for external consumers but not used internally
#[allow(unused_imports)]
pub use context_builder::ContextResult;
pub use search::run_search;