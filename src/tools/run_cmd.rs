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
/// No shell interpretation (pipes, redirects, command chaining blocked).
/// Landlock sandbox enabled by default on Linux (kernel 5.13+).
///
/// # Arguments
/// * `command_line` - Command to execute (e.g., "pdftotext -f 1 -l 5 doc.pdf -")
/// * `head` - Return only first N lines (null = no limit)
/// * `tail` - Return only last N lines (null = no limit)
/// * `timeout_seconds` - Optional timeout in seconds (default: 30)
///
/// # Controlling Output Size
/// Use head/tail to limit output instead of pipes:
/// - head=100, tail=null: First 100 lines
/// - head=null, tail=50: Last 50 lines
/// - head=50, tail=50: First 50 + last 50 (with truncation notice)
/// - head=null, tail=null: Full output (be careful with large files!)
///
/// # Blocked Patterns
/// These are rejected for security:
/// - Pipes: |
/// - Command chaining: ;, &&, ||
/// - Command substitution: $(), backticks
/// - Redirects: >, <, >>, <<
///
/// # Examples
/// ```ignore
/// // First 100 lines
/// run_command("pdftotext doc.pdf -", 100, null, null).await
///
/// // Last 50 lines (conclusion)
/// run_command("pdftotext doc.pdf -", null, 50, null).await
///
/// // Specific pages
/// run_command("pdftotext -f 1 -l 5 doc.pdf -", null, null, null).await
///
/// // OCR with language
/// run_command("tesseract image.png stdout -l jpn", null, null, 120).await
/// ```
#[function]
pub async fn run_command(
    command_line: String,
    head: Option<usize>,
    tail: Option<usize>,
    timeout_seconds: Option<u32>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "run_command",
        &[
            ("command_line".to_string(), command_line.clone()),
            (
                "head".to_string(),
                head.map(|h| h.to_string()).unwrap_or_default(),
            ),
            (
                "tail".to_string(),
                tail.map(|t| t.to_string()).unwrap_or_default(),
            ),
            (
                "timeout_seconds".to_string(),
                timeout_seconds.map(|t| t.to_string()).unwrap_or_default(),
            ),
        ],
    );

    // Validate command (blocks dangerous patterns)
    let parts = match validate_command(&command_line) {
        Ok(parts) => parts,
        Err(e) => {
            log_tool_result("run_command", &e);
            return Ok(e);
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

    // Apply sandbox (Linux only, if enabled)
    if let Err(e) = apply_sandbox_if_enabled(config) {
        log_tool_result("run_command", &e);
        return Ok(e);
    }

    // Determine timeout
    let _timeout = timeout_seconds
        .map(|t| std::time::Duration::from_secs(t as u64))
        .unwrap_or(tool_config.timeout);

    // Execute command
    let output_result = execute_command(binary, &args);

    let result = match output_result {
        Ok(output) => {
            if output.success {
                // Success - apply head/tail and return
                apply_head_tail(output.stdout, head, tail)
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
        Err(e) => format!("Error: {}", e),
    };

    log_tool_result("run_command", &result);
    Ok(result)
}

/// Validate command line and parse into parts.
/// Blocks dangerous shell patterns.
fn validate_command(command_line: &str) -> Result<Vec<String>, String> {
    // Block dangerous patterns (check multi-char patterns first)
    let forbidden_patterns = [
        ("&&", "AND operator"),
        ("||", "OR operator"),
        ("$(", "command substitution"),
        (";", "command separator"),
        ("|", "pipe"),
        ("`", "backtick substitution"),
        (">>", "append redirect"),
        ("<<", "here-document"),
        (">", "output redirect"),
        ("<", "input redirect"),
    ];

    for (pattern, name) in forbidden_patterns {
        if command_line.contains(pattern) {
            return Err(format!(
                "Error: Shell feature '{}' is not allowed. Use tool-specific flags instead.",
                name
            ));
        }
    }

    // Parse command
    let parts = match shell_words::split(command_line) {
        Ok(parts) => parts,
        Err(e) => {
            return Err(format!(
                "Error: Invalid command syntax: {}. Use quotes for filenames with spaces.",
                e
            ));
        }
    };

    if parts.is_empty() {
        return Err("Error: Empty command line. Provide a command to execute.".to_string());
    }

    Ok(parts)
}

/// Apply head/tail truncation to output.
fn apply_head_tail(output: String, head: Option<usize>, tail: Option<usize>) -> String {
    if head.is_none() && tail.is_none() {
        return output;
    }

    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    match (head, tail) {
        (Some(h), None) if h < total_lines => {
            lines.iter().take(h).cloned().collect::<Vec<_>>().join("\n")
        }
        (None, Some(t)) if t < total_lines => {
            lines
                .iter()
                .rev()
                .take(t)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n")
        }
        (Some(h), Some(t)) if h + t < total_lines => {
            let head_part: Vec<&str> = lines.iter().take(h).cloned().collect();
            let tail_part: Vec<&str> = lines
                .iter()
                .rev()
                .take(t)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();

            let mut result = head_part.join("\n");
            result.push_str("\n\n... [output truncated, use specific page range for full content] ...\n\n");
            result.push_str(&tail_part.join("\n"));
            result
        }
        _ => output, // head + tail >= total_lines, return full output
    }
}

/// Apply Landlock sandbox if enabled (Linux only).
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn apply_sandbox_if_enabled(config: &ExternalToolsConfig) -> Result<(), String> {
    if !config.enable_sandbox {
        return Ok(());
    }

    use landlock::{AccessFs, PathBeneath, Ruleset, ABI};

    let abi = ABI::V1;

    // Create ruleset
    let ruleset = match Ruleset::new()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| format!("Failed to create Landlock ruleset: {}", e))?
        .create()
    {
        Ok(rs) => rs,
        Err(e) => {
            // Landlock not supported (kernel too old)
            eprintln!(
                "Warning: Landlock sandbox not supported (kernel may be too old): {}",
                e
            );
            eprintln!("Running without filesystem isolation.");
            return Ok(());
        }
    };

    // Add rules
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let usr = std::path::PathBuf::from("/usr");
    let lib = std::path::PathBuf::from("/lib");
    let lib64 = std::path::PathBuf::from("/lib64");
    let etc = std::path::PathBuf::from("/etc");
    let tmp = std::path::PathBuf::from("/tmp");

    // CWD: read/write
    if let Err(e) = ruleset.add_rule(PathBeneath::new(
        cwd,
        AccessFs::ReadFile | AccessFs::WriteFile | AccessFs::ReadDir | AccessFs::MakeDir
            | AccessFs::MakeReg,
    )) {
        eprintln!("Warning: Failed to add CWD rule to sandbox: {}", e);
    }

    // /usr: read-only (binaries)
    if usr.exists() {
        if let Err(e) =
            ruleset.add_rule(PathBeneath::new(&usr, AccessFs::ReadFile | AccessFs::ReadDir))
        {
            eprintln!("Warning: Failed to add /usr rule to sandbox: {}", e);
        }
    }

    // /lib: read-only (libraries)
    if lib.exists() {
        if let Err(e) =
            ruleset.add_rule(PathBeneath::new(&lib, AccessFs::ReadFile | AccessFs::ReadDir))
        {
            eprintln!("Warning: Failed to add /lib rule to sandbox: {}", e);
        }
    }

    // /lib64: read-only (libraries on 64-bit systems)
    if lib64.exists() {
        if let Err(e) =
            ruleset.add_rule(PathBeneath::new(&lib64, AccessFs::ReadFile | AccessFs::ReadDir))
        {
            eprintln!("Warning: Failed to add /lib64 rule to sandbox: {}", e);
        }
    }

    // /etc: read-only (config)
    if etc.exists() {
        if let Err(e) =
            ruleset.add_rule(PathBeneath::new(&etc, AccessFs::ReadFile | AccessFs::ReadDir))
        {
            eprintln!("Warning: Failed to add /etc rule to sandbox: {}", e);
        }
    }

    // /tmp: read/write
    if tmp.exists() {
        if let Err(e) = ruleset.add_rule(PathBeneath::new(
            &tmp,
            AccessFs::ReadFile | AccessFs::WriteFile | AccessFs::ReadDir,
        )) {
            eprintln!("Warning: Failed to add /tmp rule to sandbox: {}", e);
        }
    }

    // Restrict self
    let status = ruleset
        .restrict_self()
        .map_err(|e| format!("Failed to apply Landlock sandbox: {}", e))?;

    if !status.ruleset.is_fully_enforced() {
        eprintln!("Warning: Landlock sandbox not fully enforced (kernel may not support all features)");
    }

    Ok(())
}

/// Apply sandbox if enabled (non-Linux platforms).
#[cfg(not(all(feature = "sandbox", target_os = "linux")))]
fn apply_sandbox_if_enabled(config: &ExternalToolsConfig) -> Result<(), String> {
    if config.enable_sandbox {
        #[cfg(target_os = "android")]
        eprintln!("Warning: Sandbox not available on Termux. Running without filesystem isolation.");
        
        #[cfg(target_os = "macos")]
        eprintln!("Warning: Sandbox not yet supported on macOS. Running without filesystem isolation.");
        
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        eprintln!("Warning: Sandbox not supported on this platform. Running without filesystem isolation.");
    }
    Ok(())
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
    fn test_validate_command_blocks_semicolon() {
        let result = validate_command("pdftotext file.pdf - ; rm -rf /");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("'command separator' is not allowed"));
    }

    #[test]
    fn test_validate_command_blocks_and() {
        let result = validate_command("pdftotext file.pdf - && cat /etc/passwd");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("'AND operator' is not allowed"));
    }

    #[test]
    fn test_validate_command_blocks_or() {
        let result = validate_command("pdftotext file.pdf - || echo failed");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'OR operator'"));
    }

    #[test]
    fn test_validate_command_blocks_command_substitution() {
        let result = validate_command("pdftotext $(cat file.txt).pdf -");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("'command substitution' is not allowed"));
    }

    #[test]
    fn test_validate_command_blocks_redirect() {
        let result = validate_command("pdftotext file.pdf - > output.txt");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("'output redirect' is not allowed"));
    }

    #[test]
    fn test_validate_command_accepts_valid() {
        let result = validate_command("pdftotext -f 1 -l 5 file.pdf -");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            vec!["pdftotext", "-f", "1", "-l", "5", "file.pdf", "-"]
        );
    }

    #[test]
    fn test_validate_command_accepts_quoted_filename() {
        let result = validate_command("pdftotext \"file name.pdf\" -");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec!["pdftotext", "file name.pdf", "-"]);
    }

    #[test]
    fn test_validate_command_rejects_empty() {
        let result = validate_command("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty command line"));
    }

    #[test]
    fn test_apply_head_only() {
        let output = "line1\nline2\nline3\nline4\nline5".to_string();
        let result = apply_head_tail(output, Some(2), None);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_apply_tail_only() {
        let output = "line1\nline2\nline3\nline4\nline5".to_string();
        let result = apply_head_tail(output, None, Some(2));
        assert_eq!(result, "line4\nline5");
    }

    #[test]
    fn test_apply_head_and_tail() {
        let output: String = (1..=100)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = apply_head_tail(output, Some(10), Some(5));
        assert!(result.contains("line1"));
        assert!(result.contains("line10"));
        assert!(result.contains("[output truncated"));
        assert!(result.contains("line96"));
        assert!(result.contains("line100"));
    }

    #[test]
    fn test_apply_head_tail_no_truncation_needed() {
        let output = "line1\nline2\nline3".to_string();
        let result = apply_head_tail(output.clone(), Some(10), Some(10));
        assert_eq!(result, output); // No truncation if head+tail >= total
    }

    #[test]
    fn test_apply_no_head_no_tail() {
        let output = "line1\nline2\nline3".to_string();
        let result = apply_head_tail(output.clone(), None, None);
        assert_eq!(result, output);
    }
}
