//! The `Tool` trait for sprachspiel's tool system.
//!
//! The `#[sprachspiel::tool]` proc-macro generates an impl of this trait
//! for each tool function. Uses our own types and lives in our codebase.

use std::future::Future;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// Result type for tool execution.
///
/// Returns either a successful string (passed back to the LLM) or a boxed
/// error (propagated to the caller).
pub type ToolResult = std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;

/// A tool stopped itself because the user asked for a safe stop.
///
/// Typed, never a textual prefix: the Coordinator downcasts this error and
/// reads it as the END OF THE TURN — no next tool, no next LLM call — never
/// as a recoverable failure that would feed another request. The tool owns
/// its safe point: cancellation cannot preempt it, so whatever side effects
/// the tool already committed are real (cancellation is a stop request,
/// not an undo), and whatever output never reaches the next request.
#[derive(Debug)]
pub struct ToolCancelled;

impl std::fmt::Display for ToolCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tool cancelled: the user asked for a safe stop")
    }
}

impl std::error::Error for ToolCancelled {}

/// Return whether tool text begins with one of the harness's error prefixes.
///
/// Raw command output is untrusted content; producers that know a command
/// succeeded must disambiguate it before this predicate is applied.
pub(crate) fn has_tool_error_prefix(result: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "Error:",
        "Access denied:",
        "FILE NOT FOUND:",
        "NOT A FILE:",
        "NOT A DIRECTORY:",
        "BLOCKED:",
        "Error executing tool",
    ];

    let trimmed = result.trim_start();
    PREFIXES.iter().any(|prefix| trimmed.starts_with(prefix))
}

/// Tool parameter trait: `DeserializeOwned + JsonSchema`.
///
/// Any type that can be deserialized from JSON and has a JSON schema
/// representation can be a tool's `Params` type. The proc-macro
/// `#[sprachspiel::tool]` generates a `Params` struct with both derives.
pub trait Parameters: DeserializeOwned + JsonSchema {}

impl<P: DeserializeOwned + JsonSchema> Parameters for P {}

/// Tool trait for LLM tool calling.
///
/// `#[sprachspiel::tool]`-generated tools implement this trait. Tool
/// definitions are serialized to the provider via `serde` (JSON Schema).
/// The `Coordinator` sends tool definitions through `OpenAICompatibleProvider`.
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
