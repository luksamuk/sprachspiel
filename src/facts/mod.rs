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
//! # Phases
//!
//! - Phase 0.2: Core module (types, db, decay) - DONE
//! - Phase 0.3: LLM tools (fact_add/search/remove) - DONE
//! - Phase 0.4: Prompt injection - IN PROGRESS
//! - Phase 0.5: Decay startup + /fact prune - TODO
//! - Phase 0.6: User commands - TODO
//! - Phase 0.7: Conflict resolution - TODO

// Phase 0.5-0.7 exports - will be used in future phases
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod classify;
pub mod conflict;
pub mod db;
pub mod decay;
pub mod prompt;
pub mod types;

pub use classify::classify_fact;
pub use conflict::{Conflict, ConflictType, ResolutionAction};
pub use db::{DecayStats, FactSearchResult};
pub use decay::{compute_retention, get_half_life, should_prune};
pub use prompt::build_facts_section;
pub use types::{Category, Fact, Scope, Source};
