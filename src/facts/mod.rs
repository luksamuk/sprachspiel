//! Factual Memory System
//!
//! Persistent fact storage with automatic decay, heuristic classification,
//! and keyword search (FTS5).
//!
//! # Categories
//!
//! - `preference`: User preferences (180 days half-life)
//! - `fact`: Objective facts about environment/project (30 days half-life)
//!
//! # Scope
//!
//! - `project`: Facts specific to current project
//! - `global`: Facts that apply to all projects
//!
//! # Design Decisions
//!
//! - No `context` category (handled by RAG)
//! - Heuristic classification only (no LLM for classification)
//! - FTS5 keyword search only (no embeddings)
//! - Hard limit: 500 chars per fact
//! - Soft limit: 2200 chars total in prompt
//!
//! # Auto-Extraction (autoDream-lite)
//!
//! After each user response, the system can automatically extract
//! preferences and identity facts using heuristic pattern matching.
//! See `extract` module for details.
//!
//! # Embedding-Based Semantic Dedup (P6.7)
//!
//! Facts also have vector embeddings stored in `fact_embeddings` vec0.
//! These enable semantic similarity comparison for deduplication:
//! - Layer 4: Cosine similarity >= 0.90 catches paraphrases and translations
//! - Startup verification removes semantic duplicates
//! - Eager embedding generation at insert time (fire-and-forget)

pub mod classify;
pub mod conflict;
pub mod db;
pub mod decay;
pub mod embedding;
pub mod extract;
pub mod lang;
pub mod prompt;
pub mod recovery;
pub mod types;
pub mod verify;
