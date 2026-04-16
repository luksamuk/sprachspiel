//! Macros for tool implementation
//!
//! Provides macros to reduce boilerplate in tool implementations.

/// Log a tool call with its parameters.
///
/// This macro logs the tool name and parameters at the start of a tool function.
/// It should be paired with `log_tool_result` at the end.
///
/// # Example
/// ```ignore
/// use crate::debug_tools::log_tool_call;
///
/// let value1 = "hello";
/// let value2 = 42;
/// log_tool_call("my_tool", &[
///     ("param1".to_string(), value1.to_string()),
///     ("param2".to_string(), value2.to_string()),
/// ]);
/// ```
#[macro_export]
macro_rules! log_tool_call {
    ($tool_name:expr, $params:expr) => {
        $crate::debug_tools::log_tool_call($tool_name, $params);
    };
}

/// Log a tool result.
///
/// This macro logs the result of a tool call before returning.
///
/// # Example
/// ```ignore
/// use crate::debug_tools::log_tool_result;
///
/// let result = "success".to_string();
/// log_tool_result("my_tool", &result);
/// ```
#[macro_export]
macro_rules! log_tool_result {
    ($tool_name:expr, $result:expr) => {
        $crate::debug_tools::log_tool_result($tool_name, $result);
    };
}

/// Wrap a tool function body with automatic logging.
///
/// This macro logs the tool call at the start and the result before returning.
/// It automatically returns `Ok(result)`.
///
/// # Example
/// ```ignore
/// use crate::tool_wrapper;
///
/// async fn my_tool(param: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
///     tool_wrapper!("my_tool", &[("param".to_string(), param.clone())], {
///         // Do work here
///         format!("Result for {}", param)
///     })
/// }
/// ```
#[macro_export]
macro_rules! tool_wrapper {
    ($tool_name:expr, $params:expr, $body:block) => {{
        $crate::debug_tools::log_tool_call($tool_name, $params);
        let result = $body;
        $crate::debug_tools::log_tool_result($tool_name, &result);
        Ok(result)
    }};
}

/// Log debug message if debug mode is enabled (for tool registration).
///
/// This macro is used in tool registration functions to log debug messages
/// only when debug mode is enabled.
///
/// # Example
/// ```ignore
/// use crate::log_if_debug;
///
/// fn register_tools(use_debug: bool) {
///     log_if_debug!(use_debug, "🔑 [Tool] Registering search tools");
/// }
/// ```
#[macro_export]
macro_rules! log_if_debug {
    ($debug:expr, $($arg:tt)*) => {
        if $debug && log::log_enabled!(log::Level::Debug) {
            log::debug!($($arg)*);
        }
    };
}
