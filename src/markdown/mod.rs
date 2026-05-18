//! Standalone markdown rendering for stdout output.
//!
//! This module provides markdown rendering without termimad, using
//! the shared table algorithm from the `table` submodule for responsive
//! table formatting with box-drawing characters (monochrome, no colors).
//!
//! Rendering modes:
//! - **Rich** (default): ANSI bold for headings, box-drawing tables, Mermaid diagrams
//! - **Plain** (`--plain`): No ANSI codes, pipe-delimited tables, raw Mermaid blocks
//!
//! Mermaid diagram rendering is gated behind the `mermaid` feature flag.
//! When disabled, ` ```mermaid ` blocks are treated as regular code blocks.

#[cfg(feature = "mermaid")]
pub mod mermaid;
pub mod standalone;
pub mod table;

pub use standalone::{print_markdown, print_markdown_plain};
