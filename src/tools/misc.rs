use crate::debug_tools::{log_tool_call, log_tool_result};

/// Parse boolean from string (handles "true", "false", "1", "0", empty = default)
fn parse_bool(value: Option<String>, default: bool) -> bool {
    match value {
        None => default,
        Some(s) if s.is_empty() => default,
        Some(s) => matches!(s.to_lowercase().as_str(), "true" | "1" | "yes"),
    }
}

/// Debug tool for testing tool calling.
/// 
/// When should_fail is true, returns an error message as the tool result.
/// This allows the model to see the error and react accordingly.
#[ollama_rs::function]
pub async fn test_tool(should_fail: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "test_tool",
        &[("should_fail".to_string(), should_fail.clone())],
    );
    
    // Always return Ok - the model sees the result and can react
    let result = if parse_bool(Some(should_fail), false) {
        "Error: The tool execution has failed intentionally. This is a test error. \
         The model should acknowledge this error and try again with should_fail=false, \
         or provide a direct response explaining what happened."
    } else {
        "Success: Tool calling works correctly! The test passed."
    };
    
    log_tool_result("test_tool", result);
    Ok(result.to_string())
}