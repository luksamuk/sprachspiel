//! Our own `Tool` trait and supporting types.
//!
//! This module decouples the tool system from `ollama-rs`. The `#[sprachspiel::tool]`
//! proc-macro generates an impl of this trait for each tool function. The trait
//! uses our own types and lives in our codebase.
//!
//! # W2 Wave Context
//!
//! This trait is the foundation of the W2 Provider Chain. In #119 (Agnostic
//! Provider Types), the `Tool::Params` will continue to use schemars but
//! `CustomToolInfo` and `CustomCoordinator::ToolHolder` will be unified with
//! agnostic types. In #123 (Remove ollama-rs), this trait becomes the only
//! `Tool` trait in the codebase.

use std::future::Future;

use schemars::generate::SchemaSettings;
use schemars::Schema;
use schemars::{JsonSchema, SchemaGenerator};
use serde::de::DeserializeOwned;

/// Result type for tool execution.
///
/// Returns either a successful string (passed back to the LLM) or a boxed
/// error (propagated to the caller).
#[allow(dead_code)] // W2: used in Commit 6 (migration)
pub type ToolResult = std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;

/// Tool parameter trait: `DeserializeOwned + JsonSchema`.
///
/// Any type that can be deserialized from JSON and has a JSON schema
/// representation can be a tool's `Params` type. The proc-macro
/// `#[sprachspiel::tool]` generates a `Params` struct with both derives.
pub trait Parameters: DeserializeOwned + JsonSchema {}

impl<P: DeserializeOwned + JsonSchema> Parameters for P {}

/// Tool trait: our own implementation, decoupled from `ollama-rs`.
///
/// `#[sprachspiel::tool]`-generated tools implement this trait. The blanket
/// impl below allows `#[ollama_rs::function]`-generated tools (and ollama-rs
/// built-in types like `DDGSearcher`) to be used wherever our `Tool` is
/// expected. This is the bridge that lets us migrate tools incrementally
/// while keeping ollama-rs coexistent.
///
/// # W2 Wave Context (Issue #118)
///
/// This trait is the foundation of the W2 Provider Chain. In #123 (Remove
/// ollama-rs), the blanket impl and the `ollama_rs::Tool` reference go away.
///
/// The `#[allow(dead_code)]` here is removed in the migration commit (Commit 10)
/// when `CustomCoordinator` and `ToolRegistrar` adopt our `Tool` as their
/// primary bound. Until then, the trait is "live" but unreachable from the
/// public API surface.
#[allow(dead_code)] // W2: will be removed in Commit 10 (migration)
pub trait Tool: Send + Sync {
    type Params: Parameters;

    fn name() -> &'static str;
    fn description() -> &'static str;

    /// Call the tool. Returning an `Err` will propagate it to the caller.
    /// To allow the LLM to recover from the error, return the error as a
    /// string via `Ok(error_message)`.
    fn call(&mut self, parameters: Self::Params) -> impl Future<Output = ToolResult> + Send + Sync;
}

/// Blanket impl: any ollama-rs `Tool` automatically implements our `Tool`.
///
/// This is the **forward bridge** for the W2 migration:
///
/// - Tools using `#[ollama_rs::function]` (still the majority in this PR's
///   transition window) get our `Tool` impl for free.
/// - ollama-rs built-in types like `DDGSearcher` also work via this bridge.
/// - `ToolRegistrar` and the coordinator can then accept `crate::tools::Tool`
///   uniformly.
///
/// The reverse direction (`impl<T: crate::tools::Tool> ollama_rs::Tool for T`)
/// is blocked by orphan rules; the proc-macro emits a `#[sprachspiel::tool]`
/// tool's `ollama_rs::Tool` impl explicitly (see `sprachspiel-tool-derive`).
///
/// We require `<T::Params as Parameters>: Sync` to satisfy our `Tool::call`
/// signature (which demands `impl Future + Send + Sync`). The ollama-rs
/// `Tool::Params` is `DeserializeOwned + JsonSchema`; ollama-rs's
/// `ToolHolder`-bound call path doesn't need `Sync`, so we add the bound
/// here.
/// Blanket impl: any ollama-rs `Tool` automatically implements our `Tool`.
///
/// This is the **forward bridge** for the W2 migration:
///
/// - Tools using `#[ollama_rs::function]` (still the majority in this PR's
///   transition window) get our `Tool` impl for free.
/// - ollama-rs built-in types like `DDGSearcher` also work via this bridge.
/// - `ToolRegistrar` and the coordinator can then accept `crate::tools::Tool`
///   uniformly.
///
/// The reverse direction (`impl<T: crate::tools::Tool> ollama_rs::Tool for T`)
/// is blocked by orphan rules; the proc-macro emits a `#[sprachspiel::tool]`
/// tool's `ollama_rs::Tool` impl explicitly (see `sprachspiel-tool-derive`).
///
/// We require `<T::Params as Parameters>: Send + Sync` to satisfy our
/// `Tool::call` signature (which demands `impl Future + Send + Sync`). The
/// ollama-rs `Tool::Params` is `DeserializeOwned + JsonSchema` (same shape
/// as our `Parameters`); `Send + Sync` is added here as a stricter bound.
impl<T> Tool for T
where
    T: ollama_rs::generation::tools::Tool,
    T::Params: ollama_rs::generation::tools::Parameters + Send + Sync,
{
    type Params = T::Params;

    fn name() -> &'static str {
        <T as ollama_rs::generation::tools::Tool>::name()
    }

    fn description() -> &'static str {
        <T as ollama_rs::generation::tools::Tool>::description()
    }

    async fn call(&mut self, parameters: Self::Params) -> ToolResult {
        <T as ollama_rs::generation::tools::Tool>::call(self, parameters).await
    }
}

/// A tool's JSON schema info, generated from a `Tool` impl.
///
/// **W2 Wave Context:** Used by the LLM tool schema serialization layer.
// Currently unused because no `#[sprachspiel::tool]` tools are registered
// with the coordinator yet (gradual migration). Resolved in #121 (Consumer
// Migration) when the custom coordinator adopts our `ToolInfo` directly.
#[allow(dead_code)] // W2: used in #121 Consumer Migration
#[derive(Clone, Debug)]
pub struct ToolInfo {
    /// The tool type discriminator (always "function" for now).
    pub tool_type: ToolType,
    /// The tool's function info (name, description, parameters).
    pub function: ToolFunctionInfo,
}

impl ToolInfo {
    /// Create a new `ToolInfo` for the given `Tool` type.
    #[allow(dead_code)] // Used by tests in #[cfg(test)]; will be used in #121
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
///
/// **W2 Wave Context:** Used by the LLM tool schema serialization
/// layer. Currently unused because no `#[sprachspiel::tool]` tools
/// are registered with the coordinator yet (gradual migration).
/// Resolved in #121 (Consumer Migration) when the custom coordinator
/// adopts our `ToolInfo` directly.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum ToolType {
    /// A function-style tool.
    Function,
}

/// Tool function info (name, description, JSON schema parameters).
///
/// **W2 Wave Context:** Same as `ToolType` — placeholder for #121
/// migration of the custom coordinator.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ToolFunctionInfo {
    /// Tool name (function name).
    pub name: String,
    /// Tool description (from docstring).
    pub description: String,
    /// JSON schema of the tool's parameters (root schema).
    pub parameters: Schema,
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    /// A minimal test struct that manually implements `Tool` to verify the
    /// trait works as expected without depending on the proc-macro's
    /// generated code structure.
    #[derive(Deserialize, Serialize, JsonSchema)]
    struct ExampleParams {
        name: String,
    }

    struct ExampleTool;

    impl Tool for ExampleTool {
        type Params = ExampleParams;

        fn name() -> &'static str {
            "example_tool"
        }

        fn description() -> &'static str {
            "A minimal example tool used to verify the `Tool` trait works."
        }

        async fn call(&mut self, params: Self::Params) -> ToolResult {
            Ok(format!("Hello, {}", params.name))
        }
    }

    #[test]
    fn test_tool_name_and_description() {
        assert_eq!(ExampleTool::name(), "example_tool");
        let desc = ExampleTool::description();
        assert!(desc.contains("minimal example tool"));
    }

    #[test]
    fn test_tool_call() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut tool = ExampleTool;
        let result = rt.block_on(async {
            tool.call(ExampleParams {
                name: "world".to_string(),
            })
            .await
        });
        assert_eq!(result.unwrap(), "Hello, world");
    }

    #[test]
    fn test_tool_info_generation() {
        let info = ToolInfo::new::<ExampleParams, ExampleTool>();
        assert_eq!(info.function.name, "example_tool");
        assert!(info.function.description.contains("minimal example tool"));
    }

    #[test]
    fn test_tool_result_type_alias() {
        let f: ToolResult = Ok("test".to_string());
        assert_eq!(f.unwrap(), "test");
    }
}
