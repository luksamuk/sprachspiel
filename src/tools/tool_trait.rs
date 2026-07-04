//! Our own `Tool` trait.
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

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// Result type for tool execution.
///
/// Returns either a successful string (passed back to the LLM) or a boxed
/// error (propagated to the caller).
pub type ToolResult = std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;

/// Tool parameter trait: `DeserializeOwned + JsonSchema`.
///
/// Any type that can be deserialized from JSON and has a JSON schema
/// representation can be a tool's `Params` type. The proc-macro
/// `#[sprachspiel::tool]` generates a `Params` struct with both derives.
pub trait Parameters: DeserializeOwned + JsonSchema {}

impl<P: DeserializeOwned + JsonSchema> Parameters for P {}

/// Tool trait: decoupled from `ollama-rs`.
///
/// `#[sprachspiel::tool]`-generated tools implement this trait. Tool
/// definitions are serialized to the provider via `serde` (JSON Schema),
/// not via `ollama-rs` types. The `CustomCoordinator` sends tool
/// definitions through the `OpenAICompatibleProvider` shim.
pub trait Tool: Send + Sync {
    type Params: Parameters;

    fn name() -> &'static str;
    fn description() -> &'static str;

    /// Call the tool. Returning an `Err` will propagate it to the caller.
    /// To allow the LLM to recover from the error, return the error as a
    /// string via `Ok(error_message)`.
    /// Bound is `Send` (not `Send + Sync`) because `LlmProvider::embed`/
    /// `chat` (via async_trait) returns `Pin<Box<dyn Future + Send>>`
    /// which is not Sync. This is safe because the tool future is always
    /// awaited on a single thread.
    fn call(&mut self, parameters: Self::Params) -> impl Future<Output = ToolResult> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::ToolInfo as ProviderToolInfo;
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
        let info = ProviderToolInfo::new::<ExampleParams, ExampleTool>();
        assert_eq!(info.function.name, "example_tool");
        assert!(info.function.description.contains("minimal example tool"));
    }

    #[test]
    fn test_tool_result_type_alias() {
        let f: ToolResult = Ok("test".to_string());
        assert_eq!(f.unwrap(), "test");
    }
}
