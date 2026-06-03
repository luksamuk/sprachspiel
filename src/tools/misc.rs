use sprachspiel_tool_derive::tool;
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::parse_bool;

/// Debug tool for testing tool calling and error handling.
///
/// This tool always succeeds but can return an error message as its result.
/// Use this to test how the model handles tool errors and retries.
///
/// # Arguments
/// * `should_fail` - Whether to return an error message. Optional.
///   - "true", "1", or "yes": Returns an error message
///   - "false", "0", or empty: Returns success message (default)
///
/// # Returns
/// Success message or error message depending on the should_fail parameter.
/// The model should see either result and react appropriately.
///
/// # Note
/// This tool is primarily for debugging and testing tool calling behavior.
#[tool]
pub async fn test_tool(
    should_fail: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "test_tool",
        &[("should_fail".to_string(), should_fail.clone())],
    );

    // Always return Ok - the model sees the result and can react
    let result = if parse_bool(Some(&should_fail), false) {
        "Error: The tool execution has failed intentionally. This is a test error. \
         The model should acknowledge this error and try again with should_fail=false, \
         or provide a direct response explaining what happened."
    } else {
        "Success: Tool calling works correctly! The test passed."
    };

    log_tool_result("test_tool", result);
    Ok(result.to_string())
}
