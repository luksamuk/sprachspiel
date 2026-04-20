//! Feedback Signal System
//!
//! Persistent feedback signals for content items with automatic decay,
//! source weighting, and prompt integration.
//!
//! # Signal Types
//!
//! - `Good`: Positive signal (+1.0 base value)
//! - `Bad`: Negative signal (-1.0 base value)
//! - `Correction`: Corrective signal with text (+1.0 base value)
//!
//! # Sources
//!
//! - `User`: Direct user feedback (weight factor 1.0)
//! - `Llm`: LLM-generated feedback (weight factor 0.3)
//!
//! # Design Decisions
//!
//! - User feedback weighted higher than LLM feedback (ADR-004)
//! - Correction has positive base value (ADR-005)
//! - Soft-delete for pruning (pruned flag, not hard delete)
//! - Feedback targets content_items only (messages, notes, documents)

pub mod db;
pub mod decay;
pub mod prompt;
pub mod types;
