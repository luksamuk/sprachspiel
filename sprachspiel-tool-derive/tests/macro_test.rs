//! Unit tests for the `#[sprachspiel::tool]` proc-macro.
//!
//! These tests verify that the macro correctly generates `Tool` impls and
//! parameter structs from async function signatures.
//!
//! Note: the macro emits `impl crate::tools::Tool` which requires a `tools`
//! module in scope. We provide a minimal mock here for testing purposes.

use sprachspiel_tool_derive::tool;
#[allow(unused_imports)]
use tools::Tool;

// Mock the tools module so the macro can find the `Tool` trait.
mod tools {
    use std::future::Future;

    use schemars::JsonSchema;
    use serde::de::DeserializeOwned;

    pub type ToolResult = std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;

    pub trait Parameters: DeserializeOwned + JsonSchema {}

    impl<P: DeserializeOwned + JsonSchema> Parameters for P {}

    pub trait Tool: Send + Sync {
        type Params: Parameters;
        fn name() -> &'static str;
        fn description() -> &'static str;
        fn call(
            &mut self,
            parameters: Self::Params,
        ) -> impl Future<Output = ToolResult> + Send + Sync;
    }
}

/// Greets a person by name.
///
/// # Arguments
/// * `name` - The name to greet
#[tool]
pub async fn hello_world(name: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!("Hello, {}!", name))
}

#[test]
fn test_macro_generates_name() {
    assert_eq!(hello_world::name(), "hello_world");
}

#[test]
fn test_macro_generates_description() {
    let desc = hello_world::description();
    assert!(!desc.is_empty(), "description should not be empty");
    assert!(
        desc.contains("Greets") || desc.contains("greet"),
        "description should mention greeting, got: {:?}",
        desc
    );
}

#[test]
fn test_macro_generates_call() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut tool = hello_world;
    let result = rt.block_on(async {
        tool.call(__hello_world_data::__hello_world__Params {
            name: "world".to_string(),
        })
        .await
    });
    assert_eq!(result.unwrap(), "Hello, world!");
}

#[test]
fn test_macro_params_struct_derives_deserialize() {
    // Verify that the Params struct can be deserialized from JSON.
    // (The macro derives `Deserialize` so the LLM's tool-call arguments
    // can be parsed into the Params type.)
    let json = r#"{"name":"test"}"#;
    let parsed: __hello_world_data::__hello_world__Params = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.name, "test");
}

#[test]
fn test_macro_params_struct_derives_jsonschema() {
    fn assert_jsonschema<T: schemars::JsonSchema>() {}
    assert_jsonschema::<__hello_world_data::__hello_world__Params>();
}

/// Adds two numbers.
///
/// # Arguments
/// * `a` - The first number
/// * `b` - The second number
#[tool]
pub async fn add(a: i32, b: i32) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!("{}", a + b))
}

#[test]
fn test_macro_multi_param_call() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut tool = add;
    let result = rt.block_on(async { tool.call(__add_data::__add__Params { a: 2, b: 3 }).await });
    assert_eq!(result.unwrap(), "5");
}

/// Computes statistics.
///
/// Exemplo de uso:
/// * Veja também `Math` - `std::ops`
/// * Para erros, consulte Section 4 - Debugging
#[tool]
pub async fn stats_with_bullets() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok("stats".to_string())
}

#[test]
fn test_doc_heuristic_does_not_steal_unmarked_bullets() {
    // The `stats_with_bullets` tool has a docstring with bullets that do
    // NOT match the `* `valid_ident` - desc` pattern:
    //   - `* Veja também \`Math\` - \`std::ops\`` — `Math` is a valid ident
    //     but the bullet has multi-word content; it must be preserved as
    //     description text, not captured as a parameter doc.
    //   - `* Para erros, consulte Section 4 - Debugging` — no backticks.
    //
    // The tightened heuristic must NOT capture these as parameter docs.
    // We verify this indirectly: the description should preserve the
    // bullet text exactly.
    let desc = stats_with_bullets::description();
    assert!(
        desc.contains("Veja também") || desc.contains("Veja"),
        "description should preserve 'Veja também' bullet, got: {:?}",
        desc
    );
    assert!(
        desc.contains("Para erros") || desc.contains("Debugging"),
        "description should preserve 'Para erros' bullet, got: {:?}",
        desc
    );
}
