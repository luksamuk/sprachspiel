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

pub mod classify;
pub mod conflict;
pub mod db;
pub mod decay;
pub mod prompt;
pub mod types;
