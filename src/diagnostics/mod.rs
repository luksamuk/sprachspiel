//! Diagnostics module for analyzing embedding geometry and retrieval health
//!
//! Provides the `sprach diagnostics embeddings` subcommand that performs spectral
//! analysis on stored embedding vectors, reporting:
//!
//! - **d_eff** (participation ratio): effective dimensionality of the embedding space
//! - **d̄** (mean cosine distance): average pairwise distance between vectors
//! - **Regime classification**: SPREAD or TIGHT at thresholds 0.70–0.85
//! - **Variance explained**: which principal components carry the signal
//!
//! This is foundational infrastructure for the W4 embedding geometry phases:
//! #134 (threshold validation), #135 (model benchmarking), #136 (geometry-aware
//! dimensions), and #137 (geometry-aware RRF weights).
//!
//! # Module Structure
//!
//! - [`embeddings`] — Spectral analysis algorithms (d_eff, eigenvalues, regime)
//! - [`display`] — Terminal output formatting

pub mod display;
pub mod embeddings;
