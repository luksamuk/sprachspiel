//! Tool to execute external CLI commands.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::external::{CommandOutput, ExternalToolsConfig, Platform, load_tools_config};
use ollama_rs::function;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

/// Get the external tools configuration (cached).
pub fn get_config() -> &'static ExternalToolsConfig {
    static CONFIG: std::sync::OnceLock<ExternalToolsConfig> = std::sync::OnceLock::new();
    CONFIG.get_or_init(load_tools_config)
}

/// Get the current platform (cached).
fn get_platform() -> &'static Platform {
    static PLATFORM: std::sync::OnceLock<Platform> = std::sync::OnceLock::new();
    PLATFORM.get_or_init(Platform::detect)
}

/// Log a debug message with format (only in debug mode).
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if crate::debug_tools::is_debug_enabled() {
            eprintln!("[DEBUG] {}", format!($($arg)*));
        }
    };
}

/// Execute an external command and return the output.
///
/// SECURITY: Only whitelisted commands in tools.toml can be executed.
/// No shell interpretation (pipes, redirects, command chaining blocked).
/// Landlock sandbox enabled by default on Linux (kernel 5.13+).
///
/// # Arguments
/// * `command_line` - Command to execute (e.g., "pdftotext -f 1 -l 5 doc.pdf -")
/// * `head` - Return only first N lines (null = no limit). Accepts string or number.
/// * `tail` - Return only last N lines (null = no limit). Accepts string or number.
/// * `timeout_seconds` - Optional timeout in seconds (default: from config). Accepts string or number.
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
/// // First 100 lines (LLM can pass as string or null)
/// run_command("pdftotext doc.pdf -", "100", null, null).await
///
/// // Last 50 lines (conclusion)
/// run_command("pdftotext doc.pdf -", null, "50", null).await
///
/// // Specific pages
/// run_command("pdftotext -f 1 -l 5 doc.pdf -", null, null, null).await
///
/// // OCR with language and 2 minute timeout
/// run_command("tesseract image.png stdout -l jpn", null, null, "120").await
/// ```
#[function]
pub async fn run_command(
    command_line: String,
    head: Option<String>,
    tail: Option<String>,
    timeout_seconds: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Parse optional numeric parameters (accepts strings from LLM or null)
    let head_val: Option<usize> = head.as_deref().and_then(|h| h.parse().ok());
    let tail_val: Option<usize> = tail.as_deref().and_then(|t| t.parse().ok());
    let timeout_val: Option<u32> = timeout_seconds.as_deref().and_then(|t| t.parse().ok());

    log_tool_call(
        "run_command",
        &[
            ("command_line".to_string(), command_line.clone()),
            (
                "head".to_string(),
                head_val.map(|h| h.to_string()).unwrap_or_default(),
            ),
            (
                "tail".to_string(),
                tail_val.map(|t| t.to_string()).unwrap_or_default(),
            ),
            (
                "timeout_seconds".to_string(),
                timeout_val.map(|t| t.to_string()).unwrap_or_default(),
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
        // Generic message - don't reveal that it's just a config flag
        let result = format!(
            "Error: '{}' is not available. Only tools configured in tools.toml can be executed.",
            command
        );
        log_tool_result("run_command", &result);
        return Ok(result);
    }

    // Check if binary exists and get its path
    let binary = &tool_config.binary;
    let binary_path = match which::which(binary) {
        Ok(path) => {
            debug_log!("Found binary '{}' at: {:?}", binary, path);
            Some(path)
        }
        Err(_) => {
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
    };

    // Apply sandbox (Linux only, if enabled)
    if let Err(e) = apply_sandbox_if_enabled(config, binary_path.as_deref()) {
        log_tool_result("run_command", &e);
        return Ok(e);
    }

    // Determine timeout
    let timeout_duration = timeout_val
        .map(|t| Duration::from_secs(t as u64))
        .unwrap_or(tool_config.timeout);

    debug_log!(
        "Command '{}' with timeout {:?}s",
        command,
        timeout_duration.as_secs()
    );

    let output_result = execute_command(binary, &args, timeout_duration, &command_line).await;

    let result = match output_result {
        Ok(output) => {
            if output.success {
                // Success - apply head/tail and return
                apply_head_tail(output.stdout, head_val, tail_val)
            } else {
                // Command failed - return stderr with helpful context
                let exit_code_str = output.exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                
                if output.stderr.is_empty() {
                    format!(
                        "Error: Command '{}' exited with code {}.\n\
                         \n\
                         The command ran but failed without an error message.\n\
                         This usually means:\n\
                         - The input file does not exist\n\
                         - The output path is not writable\n\
                         - Invalid arguments were provided\n\
                         - The tool is not configured correctly\n\
                         \n\
                         Check that the file exists and arguments are correct.",
                        command, exit_code_str
                    )
                } else {
                    format!(
                        "Error: Command '{}' failed (exit code {}):\n\
                         \n\
                         {}\n\
                         \n\
                         Fix the issue above and try again.",
                        command, exit_code_str, output.stderr.trim()
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
        (None, Some(t)) if t < total_lines => lines
            .iter()
            .rev()
            .take(t)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n"),
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
            result.push_str(
                "\n\n... [output truncated, use specific page range for full content] ...\n\n",
            );
            result.push_str(&tail_part.join("\n"));
            result
        }
        _ => output, // head + tail >= total_lines, return full output
    }
}

// Thread-local flag to track if Landlock sandbox has been applied.
//
// Landlock creates stacked rulesets per thread. Each call to `restrict_self()`
// adds a new layer. The kernel limits this to 16 layers. Once applied, we
// don't need to apply again in the same thread.
//
// E2BIG error means the thread already has maximum layers (16), which implies
// it's already well-sandboxed (either by us or by a parent process).
#[cfg(all(feature = "sandbox", target_os = "linux"))]
std::thread_local! {
    static LANDLOCK_APPLIED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Apply Landlock sandbox if enabled (Linux only).
///
/// Design based on landlock sandboxer example:
/// https://github.com/landlock-lsm/rust-landlock/blob/master/examples/sandboxer.rs
///
/// Key insight: from_read(abi) includes Execute, ReadFile, ReadDir
/// So RO paths with from_read() can execute binaries and read files.
///
/// # Layer Stacking
///
/// Landlock rulesets are stacked per-thread. The kernel limits this to 16 layers.
/// Once applied, subsequent calls would add new layers, eventually hitting E2BIG.
/// We use thread-local tracking to apply sandbox only once per thread.
///
/// # E2BIG Handling
///
/// E2BIG from `restrict_self()` means the maximum number of stacked rulesets
/// (16) has been reached for the current thread. This indicates the thread is
/// already sandboxed - either by a previous call or by inheritance from a
/// parent process. We treat this as success, not an error.
///
/// Reference: https://docs.kernel.org/userspace-api/landlock.html
/// "There is a limit of 16 layers of stacked rulesets... E2BIG: The maximum
/// number of stacked rulesets is reached for the current thread."
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn apply_sandbox_if_enabled(
    config: &ExternalToolsConfig,
    _binary_path: Option<&std::path::Path>,
) -> Result<(), String> {
    if !config.enable_sandbox {
        debug_log!("Sandbox disabled in config");
        return Ok(());
    }

    // Check if already applied in this thread
    if LANDLOCK_APPLIED.get() {
        debug_log!("Landlock already applied in this thread, skipping");
        return Ok(());
    }

    use landlock::{
        ABI, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
        path_beneath_rules,
    };

    debug_log!("Applying Landlock sandbox...");

    // CWD: read/write
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    debug_log!("CWD: {}", cwd);

    // Get ask-ai config directory (read-only)
    let config_dir = get_config_dir();
    debug_log!("Config dir: {:?}", config_dir);

    // Get ask-ai data directory (read/write)
    let data_dir = get_data_dir();
    debug_log!("Data dir: {:?}", data_dir);

    let abi = ABI::V1;

    // System read-only paths - using from_read(abi) which includes:
    // Execute, ReadFile, ReadDir
    let ro_paths: Vec<&str> = ["/usr", "/lib", "/lib64", "/etc", "/proc"]
        .iter()
        .filter(|p| std::path::Path::new(p).exists())
        .copied()
        .collect();

    debug_log!("System read-only paths: {:?}", ro_paths);

    // Create ruleset
    let ruleset_result = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| format!("Failed to create Landlock ruleset: {}", e))?
        .create();

    let ruleset_created = match ruleset_result {
        Ok(rs) => {
            debug_log!("Ruleset created successfully");
            rs
        }
        Err(e) => {
            eprintln!(
                "Warning: Landlock sandbox not supported (kernel may be too old): {}",
                e
            );
            eprintln!("Running without filesystem isolation.");
            return Ok(());
        }
    };

    // Add CWD with full access (read/write)
    debug_log!("Adding CWD rule (from_all)...");
    let mut ruleset_created = ruleset_created
        .add_rules(path_beneath_rules([&cwd], AccessFs::from_all(abi)))
        .map_err(|e| format!("Failed to add CWD rule: {}", e))?;

    // Add ask-ai data directory with full access (for database/conversations)
    if let Some(ref data) = data_dir {
        debug_log!("Adding data dir rule (from_all): {:?}", data);
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules([data], AccessFs::from_all(abi)))
            .map_err(|e| format!("Failed to add data dir rule: {}", e))?;
    }

    // Add system read-only paths (from_read includes Execute!)
    if !ro_paths.is_empty() {
        debug_log!("Adding system read-only paths (from_read)...");
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules(ro_paths, AccessFs::from_read(abi)))
            .map_err(|e| format!("Failed to add read-only rules: {}", e))?;
    }

    // Add ask-ai config directory (read-only, for tools.toml, config.toml)
    if let Some(ref config_path) = config_dir {
        debug_log!("Adding config dir rule (from_read): {:?}", config_path);
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules([config_path], AccessFs::from_read(abi)))
            .map_err(|e| format!("Failed to add config dir rule: {}", e))?;
    }

    // Add /tmp with full access
    if std::path::Path::new("/tmp").exists() {
        debug_log!("Adding /tmp rule (from_all)...");
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules(["/tmp"], AccessFs::from_all(abi)))
            .map_err(|e| format!("Failed to add /tmp rule: {}", e))?;
    }

    // Add /dev/null for output redirection (read/write)
    if std::path::Path::new("/dev/null").exists() {
        debug_log!("Adding /dev/null rule (from_all)...");
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules(["/dev/null"], AccessFs::from_all(abi)))
            .map_err(|e| format!("Failed to add /dev/null rule: {}", e))?;
    }

    // Apply restrictions
    debug_log!("Calling restrict_self()...");
    let status = match ruleset_created.restrict_self() {
        Ok(status) => {
            LANDLOCK_APPLIED.set(true);
            status
        }
        Err(e) => {
            let err_str = e.to_string();
            // E2BIG means maximum number of stacked rulesets reached (16 layers)
            // This indicates the thread is already sandboxed - either by us
            // or by inheritance from a parent process. Treat as success.
            if err_str.contains("E2BIG") || err_str.contains("Argument list too long") {
                debug_log!("Landlock layers limit reached (E2BIG), thread already sandboxed");
                LANDLOCK_APPLIED.set(true);
                return Ok(());
            }
            return Err(format!("Failed to apply Landlock sandbox: {}", e));
        }
    };

    debug_log!("Sandbox status: {:?}", status.ruleset);

    if status.ruleset != RulesetStatus::FullyEnforced {
        eprintln!(
            "Warning: Landlock sandbox not fully enforced (kernel may not support all features)"
        );
    }

    Ok(())
}

/// Get the ask-ai config directory path.
/// Returns None if the directory doesn't exist.
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn get_config_dir() -> Option<String> {
    // Check XDG_CONFIG_HOME first
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        let path = std::path::PathBuf::from(xdg_config).join("ask-ai");
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    // Fall back to ~/.config/ask-ai
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".config").join("ask-ai");
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    None
}

/// Get the ask-ai data directory path.
/// Returns None if the directory doesn't exist.
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn get_data_dir() -> Option<String> {
    // Check XDG_DATA_HOME first
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        let path = std::path::PathBuf::from(xdg_data).join("ask-ai");
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    // Fall back to ~/.local/share/ask-ai
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".local").join("share").join("ask-ai");
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    None
}

/// Apply sandbox if enabled (non-Linux platforms).
#[cfg(not(all(feature = "sandbox", target_os = "linux")))]
fn apply_sandbox_if_enabled(
    _config: &ExternalToolsConfig,
    _binary_path: Option<&std::path::Path>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    if _config.enable_sandbox {
        eprintln!(
            "Warning: Sandbox not available on Termux. Running without filesystem isolation."
        );
    }

    #[cfg(target_os = "macos")]
    if _config.enable_sandbox {
        eprintln!(
            "Warning: Sandbox not yet supported on macOS. Running without filesystem isolation."
        );
    }

    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    if _config.enable_sandbox {
        eprintln!(
            "Warning: Sandbox not supported on this platform. Running without filesystem isolation."
        );
    }

    Ok(())
}

/// Execute a command with timeout enforcement.
///
/// Uses tokio::process::Command with kill_on_drop(true) to ensure
/// the process is terminated when timeout expires.
///
/// # Arguments
/// * `binary` - Path to the binary
/// * `args` - Arguments for the command
/// * `timeout_duration` - Maximum execution time
/// * `command_line` - Full command line for error message suggestions
///
/// # Returns
/// * Ok(CommandOutput) - Command result
/// * Err(String) - Error message with suggestions
async fn execute_command(
    binary: &str,
    args: &[String],
    timeout_duration: Duration,
    command_line: &str,
) -> Result<CommandOutput, String> {
    debug_log!("Executing: {} {:?}", binary, args);

    let args_owned = args.to_vec();
    let binary_owned = binary.to_string();

    let result = timeout(timeout_duration, async {
        Command::new(&binary_owned)
            .args(&args_owned)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();
            let success = output.status.success();

            debug_log!(
                "Command '{}' finished: success={}, exit_code={:?}",
                binary_owned,
                success,
                exit_code
            );

            Ok(CommandOutput {
                stdout,
                stderr,
                exit_code,
                success,
            })
        }
        Ok(Err(e)) => {
            let err_msg = format!("Error: Failed to execute '{}': {}", binary_owned, e);
            debug_log!("{}", err_msg);
            Err(err_msg)
        }
        Err(_) => {
            let timeout_secs = timeout_duration.as_secs();
            let err_msg = format!(
                "Error: Command '{}' timed out after {} seconds.\n\n\
                Suggestions:\n\
                1. Increase timeout: run_command(\"{}\", null, null, {})\n\
                2. Use faster approach: reduce input size or add flags to limit work",
                command_line,
                timeout_secs,
                command_line,
                timeout_secs * 2
            );
            debug_log!(
                "Command '{}' timed out after {} seconds",
                binary_owned,
                timeout_secs
            );
            Err(err_msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_command_blocks_semicolon() {
        let result = validate_command("pdftotext file.pdf - ; rm -rf /");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("'command separator' is not allowed")
        );
    }

    #[test]
    fn test_validate_command_blocks_and() {
        let result = validate_command("pdftotext file.pdf - && cat /etc/passwd");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("'AND operator' is not allowed")
        );
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
        assert!(
            result
                .unwrap_err()
                .contains("'command substitution' is not allowed")
        );
    }

    #[test]
    fn test_validate_command_blocks_redirect() {
        let result = validate_command("pdftotext file.pdf - > output.txt");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("'output redirect' is not allowed")
        );
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

    #[test]
    fn test_string_parameter_parsing() {
        // Tests that string parameters are correctly parsed to numbers
        // This is what the LLM sends when it passes "5" as a string

        // Test valid string numbers
        let head: Option<String> = Some("100".to_string());
        let head_val: Option<usize> = head.as_deref().and_then(|h| h.parse().ok());
        assert_eq!(head_val, Some(100));

        // Test null as string (should fail to parse and become None)
        let head: Option<String> = Some("null".to_string());
        let head_val: Option<usize> = head.as_deref().and_then(|h| h.parse().ok());
        assert_eq!(head_val, None);

        // Test empty string (should become None)
        let head: Option<String> = Some("".to_string());
        let head_val: Option<usize> = head.as_deref().and_then(|h| h.parse().ok());
        assert_eq!(head_val, None);

        // Test actual None
        let head: Option<String> = None;
        let head_val: Option<usize> = head.as_deref().and_then(|h| h.parse().ok());
        assert_eq!(head_val, None);

        // Test timeout parsing
        let timeout: Option<String> = Some("30".to_string());
        let timeout_val: Option<u32> = timeout.as_deref().and_then(|t| t.parse().ok());
        assert_eq!(timeout_val, Some(30));

        // Test timeout with "null" string
        let timeout: Option<String> = Some("null".to_string());
        let timeout_val: Option<u32> = timeout.as_deref().and_then(|t| t.parse().ok());
        assert_eq!(timeout_val, None);

        // Test whitespace handling
        let head: Option<String> = Some(" 50 ".to_string());
        let head_val: Option<usize> = head.as_deref().and_then(|h| h.trim().parse().ok());
        assert_eq!(head_val, Some(50));
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_timeout_kills_long_running_command() {
        // Test that timeout kills processes that take too long
        // Requires "sleep" command to be available
        let result = execute_command(
            "sleep",
            &["10".to_string()],
            Duration::from_millis(50),
            "sleep 10",
        )
        .await;

        assert!(result.is_err(), "Should return error on timeout");
        let err = result.unwrap_err();
        assert!(
            err.contains("timed out"),
            "Error should mention timeout, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_timeout_allows_fast_command() {
        // Test that fast commands complete within timeout
        // Requires "echo" command to be available
        let result = execute_command(
            "echo",
            &["hello".to_string()],
            Duration::from_secs(5),
            "echo hello",
        )
        .await;

        assert!(result.is_ok(), "Fast command should succeed");
        let output = result.unwrap();
        assert!(output.success, "Command should succeed");
        assert!(
            output.stdout.contains("hello"),
            "Output should contain 'hello', got: {}",
            output.stdout
        );
    }

    #[tokio::test]
    async fn test_timeout_error_message_format() {
        // Test that timeout error includes suggestions
        let result = execute_command(
            "sleep",
            &["10".to_string()],
            Duration::from_millis(50),
            "sleep 10",
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Suggestions:"),
            "Error should include suggestions"
        );
        assert!(
            err.contains("Increase timeout"),
            "Error should suggest increasing timeout"
        );
        assert!(
            err.contains("reduce input size"),
            "Error should suggest reducing input"
        );
    }
}
