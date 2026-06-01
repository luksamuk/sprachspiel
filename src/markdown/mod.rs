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
//!
//! LaTeX formula rendering is gated behind the `latex` feature flag.
//! When disabled, ` ```latex ` / ` ```math ` blocks and `$$` display math
//! are treated as regular code blocks.

#[cfg(feature = "latex")]
pub mod latex;
#[cfg(feature = "mermaid")]
pub mod mermaid;
pub mod standalone;
pub mod table;

pub use standalone::{print_markdown, print_markdown_plain};

#[cfg(feature = "latex")]
pub(crate) use latex::call_latex_safely;
#[cfg(feature = "mermaid")]
pub(crate) use mermaid::call_mermaid_safely;
