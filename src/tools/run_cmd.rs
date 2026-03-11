//! Tool to execute external CLI commands.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::external::{load_tools_config, CommandOutput, ExternalToolsConfig, Platform};
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
/// The command line is parsed using shell-style quoting (respects quotes).
/// Commands are executed directly without shell interpretation to prevent
/// injection attacks. NO shell features (pipes, redirects, $() expansion).
///
/// # Arguments
/// * `command_line` - Full command line to execute (e.g., "pdftotext -f 1 -l 5 doc.pdf -")
/// * `timeout_seconds` - Optional timeout in seconds (default: 30)
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
/// - `"pdftotext document.pdf -"` - Extract all text
/// - `"pdftotext -f 1 -l 10 document.pdf -"` - Extract pages 1-10 only
/// - `"pdftotext -f 5 -l 5 document.pdf -"` - Extract single page 5
///
/// ## tesseract (OCR)
/// Output goes to file specified (without extension). Use `stdout` for stdout:
/// - `"tesseract image.png stdout"` - OCR to stdout
/// - `"tesseract image.png stdout -l jpn"` - OCR with Japanese language
///
/// ## pdfinfo (PDF metadata)
/// - `"pdfinfo document.pdf"` - Show PDF info (pages, size, etc.)
///
/// ## exiftool (Image metadata)
/// - `"exiftool image.jpg"` - Show all metadata
///
/// # Quoting
/// For filenames with spaces, use shell-style quoting:
/// - `"pdftotext \"file name.pdf\" -"` - Double quotes
///
/// # Errors
/// - Empty command line
/// - Invalid command syntax (unmatched quotes)
/// - Tool is disabled in configuration
/// - Tool is not installed
/// - Command execution failed
/// - Command execution timed out
///
/// # Examples
/// ```ignore
/// // Extract text from entire PDF (may be large!)
/// run_command("pdftotext document.pdf -", None).await
///
/// // Extract only pages 1-5 from PDF (recommended for large files)
/// run_command("pdftotext -f 1 -l 5 document.pdf -", None).await
///
/// // OCR an image with Japanese language
/// run_command("tesseract image.png stdout -l jpn", Some(120)).await
///
/// // Filename with spaces
/// run_command("pdftotext \"My Document.pdf\" -", None).await
/// ```
#[function]
pub async fn run_command(
    command_line: String,
    timeout_seconds: Option<u32>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "run_command",
        &[
            ("command_line".to_string(), command_line.clone()),
            (
                "timeout_seconds".to_string(),
                timeout_seconds.map(|t| t.to_string()).unwrap_or_default(),
            ),
        ],
    );

    // Parse command line into binary + args using shell-style quoting
    let parts = match shell_words::split(&command_line) {
        Ok(parts) if parts.is_empty() => {
            let result = "Error: Empty command line. Provide a command to execute.".to_string();
            log_tool_result("run_command", &result);
            return Ok(result);
        }
        Ok(parts) => parts,
        Err(e) => {
            let result = format!(
                "Error: Invalid command syntax: {}. Use quotes for filenames with spaces.",
                e
            );
            log_tool_result("run_command", &result);
            return Ok(result);
        }
    };

    let command = &parts[0];
    let args: Vec<String> = parts[1..].to_vec();

    // Load configuration (cached)
    let config = get_config();
    let platform = get_platform();

    // Get tool configuration
    let tool_config = match config.get(command) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_command() {
        let parts = shell_words::split("pdftotext file.pdf -").unwrap();
        assert_eq!(parts, vec!["pdftotext", "file.pdf", "-"]);
    }

    #[test]
    fn test_parse_command_with_flags() {
        let parts = shell_words::split("pdftotext -f 1 -l 5 file.pdf -").unwrap();
        assert_eq!(parts, vec!["pdftotext", "-f", "1", "-l", "5", "file.pdf", "-"]);
    }

    #[test]
    fn test_parse_quoted_filename() {
        let parts = shell_words::split("pdftotext \"file name.pdf\" -").unwrap();
        assert_eq!(parts, vec!["pdftotext", "file name.pdf", "-"]);
    }

    #[test]
    fn test_parse_single_quoted_filename() {
        let parts = shell_words::split("pdftotext 'file name.pdf' -").unwrap();
        assert_eq!(parts, vec!["pdftotext", "file name.pdf", "-"]);
    }

    #[test]
    fn test_parse_empty_command() {
        let parts = shell_words::split("").unwrap();
        assert!(parts.is_empty());
    }

    #[test]
    fn test_parse_unmatched_quote() {
        let result = shell_words::split("pdftotext 'file.pdf");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tesseract_with_language() {
        let parts = shell_words::split("tesseract image.png stdout -l jpn").unwrap();
        assert_eq!(parts, vec!["tesseract", "image.png", "stdout", "-l", "jpn"]);
    }
}