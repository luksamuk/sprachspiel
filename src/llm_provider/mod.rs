//! Provider-agnostic types for the W2 Provider Chain.
//!
//! This module hosts types that decouple the codebase from specific LLM
//! provider APIs. Today it exposes the JSON-schema descriptors used by
//! `crate::tools::Tool` (the `ToolInfo` family); in #119 (Agnostic
//! Provider Types), it grows to include request/response types for
//! multi-provider LLM communication.
//!
//! The separation exists so that `crate::tools::tool_trait` (and its
//! tests) can stay focused on the tool trait, while the project-agnostic
//! schema/info types live in one place that both `CustomToolInfo`
//! (custom_coordinator) and future `LlmProvider` (issue #119) can
//! depend on.

pub mod types;

#[allow(unused_imports)] // W2: will be used by #119 (Agnostic Provider Types)
pub use types::{ToolFunctionInfo, ToolInfo, ToolType};
