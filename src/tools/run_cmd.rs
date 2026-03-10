//! Tool to execute external CLI commands.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::external::{load_tools_config, ExternalToolsConfig, Platform, CommandOutput};
use ollama_rs::function;

/// Get the external tools configuration (cached).
fn get_config() -> &'static ExternalToolsConfig {
    static CONFIG: std::sync::OnceLock<ExternalToolsConfig> = std::sync::OnceLock::new();
    CONFIG.get_or_init(load_tools_config)
}

/// Get the current platform (cached).
fn get_platform() -> &'static Platform {
    static PLATFORM: std::sync::OnceLock<Platform> = std::sync::OnceLock::new();
    PLATFORM.get_or_init(Platform::detect)
}

/// Execute an external command and return the output.
///
/// SECURITY: Only whitelisted commands in tools.toml can be executed.
/// Commands are executed directly without shell interpretation to prevent
/// injection attacks. NO shell features (pipes, redirects, $() expansion).
///
/// # Arguments
/// * `command` - The command name (must be in whitelist configured in tools.toml)
/// * `args` - List of arguments for the command
/// * `timeout_seconds` - Optional timeout in seconds (default: from config, usually 30)
///
/// # Returns
/// Command output (stdout) on success, or an error message.
/// IMPORTANT: Output is NOT truncated. For large outputs, use tool-specific flags
/// to limit the result (see tool-specific notes below).
///
/// # Tool-Specific Usage Notes
///
/// ## pdftotext (PDF text extraction)
/// Use `-` as output file to write to stdout. For large PDFs, extract specific pages:
/// - `pdftotext -f 1 -l 10 document.pdf -` - Extract pages 1-10 only
/// - `pdftotext -f 5 -l 5 document.pdf -` - Extract single page 5
///
/// ## tesseract (OCR)
/// Output goes to file specified (without extension). Use `stdout` for stdout:
/// - `tesseract image.png stdout` - OCR to stdout
///
/// ## pdfinfo (PDF metadata)
/// - `pdfinfo document.pdf` - Show PDF info (pages, size, etc.)
///
/// ## exiftool (Image metadata)
/// - `exiftool image.jpg` - Show all metadata
///
/// # Errors
/// - Tool is disabled in configuration
/// - Tool is not installed
/// - Command execution failed
/// - Command execution timed out
///
/// # Examples
/// ```ignore
/// // Extract text from entire PDF (may be large!)
/// run_command("pdftotext".to_string(), vec!["document.pdf".to_string(), "-".to_string()], None).await
///
/// // Extract only pages 1-5 from PDF (recommended for large files)
/// run_command("pdftotext".to_string(), vec!["-f".to_string(), "1".to_string(), "-l".to_string(), "5".to_string(), "document.pdf".to_string(), "-".to_string()], None).await
///
/// // OCR an image
/// run_command("tesseract".to_string(), vec!["scan.png".to_string(), "stdout".to_string()], Some(60)).await
/// ```
#[function]
pub async fn run_command(
    command: String,
    args: Vec<String>,
    timeout_seconds: Option<u32>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Truncate args for display
    let args_display: String = args
        .iter()
        .map(|a| {
            if a.len() > 50 {
                format!("{}...", &a[..47])
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    log_tool_call(
        "run_command",
        &[
            ("command".to_string(), command.clone()),
            ("args".to_string(), args_display),
            (
                "timeout_seconds".to_string(),
                timeout_seconds.map(|t| t.to_string()).unwrap_or_default(),
            ),
        ],
    );

    // Load configuration (cached)
    let config = get_config();
    let platform = get_platform();

    // Get tool configuration
    let tool_config = match config.get(&command) {
        Some(tool) => tool.clone(),
        None => {
            let result = format!(
                "Error: '{}' is not in the whitelist. Only tools configured in tools.toml can be executed.",
                command
            );
            log_tool_result("run_command", &result);
            return Ok(result);
        }
    };

    // Check if tool is enabled
    if !tool_config.enabled {
        let result = format!(
            "Error: '{}' is disabled in tools.toml. Enable it to use this command.",
            command
        );
        log_tool_result("run_command", &result);
        return Ok(result);
    }

    // Check if binary exists
    let binary = &tool_config.binary;
    if which::which(binary).is_err() {
        let hint = tool_config.install_hints.get(platform);
        let result = if let Some(hint) = hint {
            format!(
                "Error: '{}' is not installed. Install with: {}",
                command, hint
            )
        } else {
            format!(
                "Error: '{}' is not installed. Install with your package manager.",
                command
            )
        };
        log_tool_result("run_command", &result);
        return Ok(result);
    }

    // Determine timeout (note: actual timeout not implemented in sync version)
    let _timeout = timeout_seconds
        .map(|t| std::time::Duration::from_secs(t as u64))
        .unwrap_or(tool_config.timeout);

    // Execute command
    let output_result = execute_command(binary, &args);

    let result = match output_result {
        Ok(output) => {
            if output.success {
                // Success - return stdout
                output.stdout
            } else {
                // Command failed - return stderr
                if output.stderr.is_empty() {
                    format!(
                        "Error: Command '{}' failed with exit code {:?}",
                        command, output.exit_code
                    )
                } else {
                    format!(
                        "Error: Command '{}' failed with exit code {:?}\n{}",
                        command, output.exit_code, output.stderr
                    )
                }
            }
        }
        Err(e) => {
            format!("Error: {}", e)
        }
    };

    log_tool_result("run_command", &result);
    Ok(result)
}

/// Execute a command synchronously.
fn execute_command(binary: &str, args: &[String]) -> Result<CommandOutput, String> {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(binary);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Execute
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute '{}': {}", binary, e))?;

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
}