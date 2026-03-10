//! Command executor for external tools.

use std::process::{Command, Stdio};
use std::time::Duration;

use tokio::time::{timeout, error::Elapsed};

use super::types::{CommandError, CommandOutput, ExternalTool};
use super::registry::ToolRegistry;

/// Global executor instance.
static EXECUTOR: std::sync::OnceLock<CommandExecutor> = std::sync::OnceLock::new();

/// Get or initialize the global executor.
pub fn get_executor() -> &'static CommandExecutor {
    EXECUTOR.get_or_init(|| {
        let registry = super::registry::get_registry();
        CommandExecutor::new(registry)
    })
}

/// Command executor with timeout and error handling.
///
/// Executes external commands safely with:
/// - Whitelist enforcement (only configured tools)
/// - Timeout enforcement
/// - Output capture (stdout/stderr)
/// - No shell interpretation
pub struct CommandExecutor {
    #[allow(dead_code)]
    registry: &'static ToolRegistry,
}

impl CommandExecutor {
    /// Create a new executor with the given registry.
    pub fn new(registry: &'static ToolRegistry) -> Self {
        CommandExecutor { registry }
    }

    /// Execute a command synchronously.
    ///
    /// # Arguments
    /// * `tool_name` - The tool name (must be in whitelist)
    /// * `args` - Arguments for the command
    /// * `input` - Optional stdin input
    /// * `timeout_override` - Optional timeout override (seconds)
    ///
    /// # Returns
    /// `CommandOutput` on success, `CommandError` on failure.
    pub fn execute_sync(
        &self,
        tool_name: &str,
        args: &[String],
        input: Option<&str>,
        timeout_override: Option<u64>,
    ) -> Result<CommandOutput, CommandError> {
        // This is implemented as sync for now, but uses tokio::runtime
        // We'll provide an async version as well
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| CommandError::Execution(format!("Failed to create runtime: {}", e)))?;

        rt.block_on(self.execute_async(tool_name, args, input, timeout_override))
    }

    /// Execute a command asynchronously.
    ///
    /// # Arguments
    /// * `tool_name` - The tool name (must be in whitelist)
    /// * `args` - Arguments for the command
    /// * `input` - Optional stdin input
    /// * `timeout_override` - Optional timeout override (seconds)
    ///
    /// # Returns
    /// `CommandOutput` on success, `CommandError` on failure.
    pub async fn execute_async(
        &self,
        tool_name: &str,
        args: &[String],
        input: Option<&str>,
        timeout_override: Option<u64>,
    ) -> Result<CommandOutput, CommandError> {
        // Get mutable access to registry for cache updates
        // We need to use interior mutability for the global registry
        // For now, we'll validate inline

        // Get tool configuration
        let tool = self.get_tool_config(tool_name)?;

        // Determine timeout
        let timeout_duration = timeout_override
            .map(Duration::from_secs)
            .unwrap_or(tool.timeout);

        // Build command - use std::process::Command for sync execution
        // For async, we use tokio::process::Command
        let binary = tool.binary.clone();

        // Execute with timeout
        let result = timeout(
            timeout_duration,
            self.execute_command(&binary, args, input),
        )
        .await;

        match result {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CommandError::Timeout(tool_name.to_string())),
        }
    }

    /// Internal: execute a command.
    async fn execute_command(
        &self,
        binary: &str,
        args: &[String],
        input: Option<&str>,
    ) -> Result<CommandOutput, CommandError> {
        // Use blocking execution for now (simpler and more reliable)
        // We spawn a blocking task
        let binary = binary.to_string();
        let args = args.to_vec();
        let input = input.map(|s| s.to_string());

        tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new(&binary);
            cmd.args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // Execute
            let output = cmd
                .output()
                .map_err(|e| CommandError::Execution(format!("Failed to execute '{}': {}", binary, e)))?;

            // Capture output
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();
            let success = output.status.success();

            Ok(CommandOutput {
                stdout,
                stderr,
                exit_code,
                success,
            })
        })
        .await
        .map_err(|e| CommandError::Execution(format!("Task join error: {}", e)))?
    }

    /// Get tool configuration.
    ///
    /// Returns error if tool is disabled or not found.
    fn get_tool_config(&self, tool_name: &str) -> Result<ExternalTool, CommandError> {
        // For now, we access the GLOBAL registry through static
        // This is safe because we're only reading

        // We need to check if tool exists and is enabled
        // Since registry has mutable cache, we need to work around this

        // Use the global registry function to get availability
        let tool = super::registry::get_registry()
            .config
            .get(tool_name)
            .cloned();

        match tool {
            Some(tool) => {
                if !tool.enabled {
                    return Err(CommandError::Disabled(tool_name.to_string()));
                }

                // Check if binary exists
                if which::which(&tool.binary).is_err() {
                    return Err(CommandError::NotFound(tool_name.to_string()));
                }

                Ok(tool)
            }
            None => {
                Err(CommandError::Disabled(format!("{} (not in whitelist)", tool_name)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_output_success() {
        let output = CommandOutput {
            stdout: "test output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        };

        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
    }

    #[test]
    fn test_command_output_failure() {
        let output = CommandOutput {
            stdout: String::new(),
            stderr: "error message".to_string(),
            exit_code: Some(1),
            success: false,
        };

        assert!(!output.success);
        assert_eq!(output.exit_code, Some(1));
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::Disabled("pdftotext".to_string());
        assert!(err.to_string().contains("disabled"));

        let err = CommandError::NotFound("tesseract".to_string());
        assert!(err.to_string().contains("not installed"));

        let err = CommandError::Timeout("ffmpeg".to_string());
        assert!(err.to_string().contains("timed out"));
    }

    // Note: Integration tests that actually run commands should be in tests/ directory
    // and check for tool availability before running

    #[test]
    #[cfg(target_os = "linux")]
    fn test_execute_echo() {
        // This test only runs on Linux where echo is available
        // It's a basic sanity check for command execution

        // First, create a minimal test config with "echo" as a tool
        // For now, skip this test since "echo" is not in default config
        // Integration tests will handle this better
    }
}