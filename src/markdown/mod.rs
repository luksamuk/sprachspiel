//! Standalone markdown rendering for stdout output.
//!
//! This module provides markdown rendering without termimad, using
//! the shared table algorithm from the `table` submodule for responsive
//! table formatting with box-drawing characters (monochrome, no colors).
//!
//! Two rendering modes:
//! - **Rich** (default): ANSI bold for headings, box-drawing tables
//! - **Plain** (`--plain`): No formatting, pipe-delimited tables

pub mod standalone;
pub mod table;

pub use standalone::{print_markdown, print_markdown_plain};
