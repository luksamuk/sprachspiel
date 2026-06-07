//! JSON-schema descriptors for `crate::tools::Tool` implementations.
//!
//! These types mirror the JSON shape used by LLM tool APIs (the `type`
//! discriminator + `function` block with name/description/parameters).
//! They live here (rather than in `crate::tools`) because the LLM
//! provider-agnostic types belong with the provider abstraction, not
//! with the tool trait itself.
//!
//! `CustomToolInfo` (in `src/chat/custom_coordinator.rs`) is the
//! runtime equivalent used by the active coordinator today. It is
//! expected to be unified with `ToolInfo` in #119 (Agnostic Provider
//! Types) when the `LlmProvider` trait is introduced.

use schemars::Schema;
use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;

use crate::tools::tool_trait::{Parameters, Tool};

/// A tool's JSON schema info, generated from a `Tool` impl.
///
/// Mirrors the JSON shape used by LLM tool APIs:
/// ```json
/// {
///   "type": "function",
///   "function": {
///     "name": "...",
///     "description": "...",
///     "parameters": { ... JSON schema ... }
///   }
/// }
/// ```
#[allow(dead_code)] // W2: will be used by #119 (Agnostic Provider Types)
#[derive(Clone, Debug)]
pub struct ToolInfo {
    /// The tool type discriminator (always "function" for now).
    pub tool_type: ToolType,
    /// The tool's function info (name, description, parameters).
    pub function: ToolFunctionInfo,
}

impl ToolInfo {
    /// Create a new `ToolInfo` for the given `Tool` type.
    ///
    /// Intended for the future `LlmProvider` abstraction (issue #119)
    /// which will own the JSON-serialization layer. Not used by the
    /// runtime coordinator today (which uses `CustomToolInfo` in
    /// `custom_coordinator.rs`).
    #[allow(dead_code)] // W2: will be used by #119 (Agnostic Provider Types)
    pub fn new<P: Parameters, T: Tool<Params = P>>() -> Self {
        let mut settings = SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator: SchemaGenerator = settings.into_generator();

        let parameters = generator.into_root_schema_for::<P>();

        Self {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: T::name().to_string(),
                description: T::description().to_string(),
                parameters,
            },
        }
    }
}

/// Tool type discriminator.
#[allow(dead_code)] // W2: will be used by #119 (Agnostic Provider Types)
#[derive(Clone, Debug)]
pub enum ToolType {
    /// A function-style tool.
    Function,
}

/// Tool function info (name, description, JSON schema parameters).
#[allow(dead_code)] // W2: will be used by #119 (Agnostic Provider Types)
#[derive(Clone, Debug)]
pub struct ToolFunctionInfo {
    /// Tool name (function name).
    pub name: String,
    /// Tool description (from docstring).
    pub description: String,
    /// JSON schema of the tool's parameters (root schema).
    pub parameters: Schema,
}
