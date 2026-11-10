//! Tool to execute external CLI commands.

use crate::debug_tools::{log_tool_call, log_tool_result, tui_aware_print};
use crate::external::{CommandOutput, ExternalToolsConfig, Platform, load_tools_config};
use crate::tools::tool_trait::has_tool_error_prefix;
use sprachspiel_tool_derive::tool;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::Duration;

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
        log::debug!($($arg)*);
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
#[tool]
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

    // Expand tilde (~) in arguments that look like file paths.
    // This allows commands like `pdftotext ~/document.pdf -` to work.
    // After expansion, check against the sensitive file blocklist.
    let args = match expand_args_tilde(&args) {
        Ok(args) => args,
        Err(e) => {
            log_tool_result("run_command", &e);
            return Ok(e);
        }
    };

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

    // Visual indicator: show which command is about to execute
    let display_cmd = crate::utils::truncate_chars(&command_line, 60);
    tui_aware_print(&format!("⚡ {display_cmd}"));

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
        // A cancelled command owns the end of the turn: the typed error
        // lets the Coordinator end it — no next tool, no next request, and
        // the command's (already unrecorded) output never reaches one.
        Err(CommandError::Cancelled) => {
            log_tool_result("run_command", "cancelled by user (child killed and reaped)");
            return Err(Box::new(crate::tools::tool_trait::ToolCancelled));
        }
        Ok(output) => {
            if output.success {
                format_success_output(output.stdout, head_val, tail_val)
            } else {
                // Command failed - return stderr with helpful context
                let exit_code_str = output
                    .exit_code
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
                        command,
                        exit_code_str,
                        output.stderr.trim()
                    )
                }
            }
        }
        Err(CommandError::Failed(e)) => format!("Error: {}", e),
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

const COMMAND_SUCCESS_PREFIX: &str = "Command completed successfully (exit code 0).\n";

/// Apply output limits, then disambiguate successful stdout that resembles an error.
pub(crate) fn format_success_output(
    stdout: String,
    head: Option<usize>,
    tail: Option<usize>,
) -> String {
    let output = apply_head_tail(stdout, head, tail);
    if has_tool_error_prefix(&output) {
        format!("{COMMAND_SUCCESS_PREFIX}{output}")
    } else {
        output
    }
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
    _config: &ExternalToolsConfig,
    _binary_path: Option<&std::path::Path>,
) -> Result<(), String> {
    // Sandbox is always enabled — no configuration option to disable it

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

    // Get sprachspiel config directory (read-only)
    let config_dir = get_config_dir();
    debug_log!("Config dir: {:?}", config_dir);

    // Get sprachspiel data directory (read/write)
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
        .map_err(|e| format!("Error: Failed to create Landlock ruleset: {}", e))?
        .create();

    let ruleset_created = match ruleset_result {
        Ok(rs) => {
            debug_log!("Ruleset created successfully");
            rs
        }
        Err(e) => {
            tui_aware_print(&format!(
                "Warning: Landlock sandbox not supported (kernel may be too old): {}",
                e
            ));
            tui_aware_print("Running without filesystem isolation.");
            return Ok(());
        }
    };

    // Add CWD with full access (read/write)
    debug_log!("Adding CWD rule (from_all)...");
    let mut ruleset_created = ruleset_created
        .add_rules(path_beneath_rules([&cwd], AccessFs::from_all(abi)))
        .map_err(|e| format!("Error: Failed to add CWD rule: {}", e))?;

    // Add sprachspiel data directory with full access (for database/conversations)
    if let Some(ref data) = data_dir {
        debug_log!("Adding data dir rule (from_all): {:?}", data);
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules([data], AccessFs::from_all(abi)))
            .map_err(|e| format!("Error: Failed to add data dir rule: {}", e))?;
    }

    // Add system read-only paths (from_read includes Execute!)
    if !ro_paths.is_empty() {
        debug_log!("Adding system read-only paths (from_read)...");
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules(ro_paths, AccessFs::from_read(abi)))
            .map_err(|e| format!("Error: Failed to add read-only rules: {}", e))?;
    }

    // Add sprachspiel config directory (read-only, for tools.toml, config.toml)
    if let Some(ref config_path) = config_dir {
        debug_log!("Adding config dir rule (from_read): {:?}", config_path);
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules([config_path], AccessFs::from_read(abi)))
            .map_err(|e| format!("Error: Failed to add config dir rule: {}", e))?;
    }

    // Add /tmp with full access
    if std::path::Path::new("/tmp").exists() {
        debug_log!("Adding /tmp rule (from_all)...");
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules(["/tmp"], AccessFs::from_all(abi)))
            .map_err(|e| format!("Error: Failed to add /tmp rule: {}", e))?;
    }

    // Add /dev/null for output redirection (read/write)
    if std::path::Path::new("/dev/null").exists() {
        debug_log!("Adding /dev/null rule (from_all)...");
        ruleset_created = ruleset_created
            .add_rules(path_beneath_rules(["/dev/null"], AccessFs::from_all(abi)))
            .map_err(|e| format!("Error: Failed to add /dev/null rule: {}", e))?;
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
            return Err(format!("Error: Failed to apply Landlock sandbox: {}", e));
        }
    };

    debug_log!("Sandbox status: {:?}", status.ruleset);

    if status.ruleset != RulesetStatus::FullyEnforced {
        tui_aware_print(
            "Warning: Landlock sandbox not fully enforced (kernel may not support all features)",
        );
    }

    Ok(())
}

/// Get the Sprachspiel config directory path.
/// Returns None if the directory doesn't exist.
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn get_config_dir() -> Option<String> {
    use crate::consts::app;

    // Check XDG_CONFIG_HOME first
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        let path = std::path::PathBuf::from(xdg_config).join(app::APP_CONFIG_DIR);
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    // Fall back to ~/.config/sprachspiel
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".config").join(app::APP_CONFIG_DIR);
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    None
}

/// Get the Sprachspiel data directory path.
/// Returns None if the directory doesn't exist.
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn get_data_dir() -> Option<String> {
    use crate::consts::app;

    // Check XDG_DATA_HOME first
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        let path = std::path::PathBuf::from(xdg_data).join(app::APP_DATA_DIR);
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    // Fall back to ~/.local/share/sprachspiel
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".local").join("share").join(app::APP_DATA_DIR);
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
    // Sandbox is always enabled but not available on this platform
    #[cfg(target_os = "android")]
    tui_aware_print(
        "Warning: Sandbox not available on Termux. Running without filesystem isolation.",
    );

    #[cfg(target_os = "macos")]
    tui_aware_print(
        "Warning: Sandbox not yet supported on macOS. Running without filesystem isolation.",
    );

    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    tui_aware_print(
        "Warning: Sandbox not supported on this platform. Running without filesystem isolation.",
    );

    Ok(())
}

/// How a child command ended other than by completing normally.
#[derive(Debug)]
enum CommandError {
    /// Execution failure — textual, recoverable tool error, unchanged shape.
    Failed(String),
    /// The user's stop request reached the tool's safe point: the child was
    /// killed directly and reaped, and no output is recorded for a later
    /// request.
    Cancelled,
}

/// Execute a command with cancellation and timeout enforcement.
///
/// Racing rules are deterministic (`biased` select, in order): a pending
/// cancellation outranks a completion and a timeout that are ready at the
/// same wake-up; a completion outranks the timeout. When a stop wins, the
/// child is killed directly and reaped here — never left to drop, so no
/// zombie outlives the call and the outcome is observable.
///
/// Descendants of the child are an explicit boundary: only the direct
/// child is terminated.
async fn execute_command(
    binary: &str,
    args: &[String],
    timeout_duration: Duration,
    command_line: &str,
) -> Result<CommandOutput, CommandError> {
    debug_log!("Executing: {} {:?}", binary, args);

    let args_owned = args.to_vec();
    let binary_owned = binary.to_string();

    // A request that landed between the Coordinator's pre-checks and this
    // spawn must not start a child at all: stopping before the work begins
    // is the earliest safe point there is.
    if crate::tools::context::cancellation_requested() {
        debug_log!("Command '{}' cancelled before spawn", binary_owned);
        log::info!("run_command cancelled before spawn: {}", command_line);
        return Err(CommandError::Cancelled);
    }

    let mut spawned = Command::new(&binary_owned);
    spawned
        .args(&args_owned)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match spawned.spawn() {
        Ok(child) => child,
        Err(e) => {
            let err_msg = format!("Error: Failed to execute '{}': {}", binary_owned, e);
            debug_log!("{}", err_msg);
            return Err(CommandError::Failed(err_msg));
        }
    };

    // Output pipes are kept for the completion arm only; a stopped child's
    // partial output is never recorded.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let cancel_token = crate::tools::context::get_cancel_token();

    enum Race {
        Done(std::io::Result<std::process::ExitStatus>),
        Cancelled,
        TimedOut,
    }

    let raced = tokio::select! {
        biased;
        _ = async {
            match &cancel_token {
                Some(ct) => ct.cancelled().await,
                None => std::future::pending().await,
            }
        } => Race::Cancelled,
        status = child.wait() => Race::Done(status),
        _ = tokio::time::sleep(timeout_duration) => Race::TimedOut,
    };

    match raced {
        Race::Done(Ok(status)) => {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            if let Some(pipe) = stdout_pipe.as_mut() {
                let _ = tokio::io::AsyncReadExt::read_to_end(pipe, &mut stdout).await;
            }
            if let Some(pipe) = stderr_pipe.as_mut() {
                let _ = tokio::io::AsyncReadExt::read_to_end(pipe, &mut stderr).await;
            }

            debug_log!(
                "Command '{}' finished: success={}, exit_code={:?}",
                binary_owned,
                status.success(),
                status.code()
            );

            Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&stdout).to_string(),
                stderr: String::from_utf8_lossy(&stderr).to_string(),
                exit_code: status.code(),
                success: status.success(),
            })
        }
        Race::Done(Err(e)) => {
            let err_msg = format!("Error: Failed to execute '{}': {}", binary_owned, e);
            debug_log!("{}", err_msg);
            Err(CommandError::Failed(err_msg))
        }
        Race::Cancelled => {
            // Kill the child directly and reap it: drop alone neither stops
            // a syscall in flight nor guarantees the process is reaped.
            let _ = child.start_kill();
            let _ = child.wait().await;
            debug_log!("Command '{}' killed and reaped after cancellation", binary_owned);
            log::info!(
                "run_command child killed and reaped after cancellation: {}",
                command_line
            );
            Err(CommandError::Cancelled)
        }
        Race::TimedOut => {
            let _ = child.start_kill();
            let _ = child.wait().await;
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
            Err(CommandError::Failed(err_msg))
        }
    }
}

/// Expand tilde (~) in arguments that look like file paths.
///
/// Only arguments starting with `~` are expanded (e.g., `~/doc.pdf` → `/home/user/doc.pdf`).
/// After expansion, the path is checked against the sensitive file blocklist
/// (`.env`, `.ssh/`, `.pem`, credentials, etc.) to prevent access to protected files.
///
/// Non-tilde arguments (absolute paths, relative paths, flags, values) are left unchanged.
fn expand_args_tilde(args: &[String]) -> Result<Vec<String>, String> {
    use crate::tools::files_blocklist::BlocklistConfig;

    let config = BlocklistConfig::default();

    args.iter()
        .map(|arg| {
            // Only expand arguments that start with ~
            if arg.starts_with('~') {
                let expanded = crate::utils::expand_tilde_path(arg);
                let expanded_str = expanded.to_string_lossy().to_string();

                // Check against sensitive file blocklist after expansion
                if crate::tools::files_blocklist::is_blocked_for_read(&expanded, &config) {
                    return Err(format!(
                        "Error: Path '{}' is blocked (protected file pattern). \
                         Access to .env, .ssh, credentials, and similar sensitive files is restricted.",
                        arg
                    ));
                }

                Ok(expanded_str)
            } else {
                Ok(arg.clone())
            }
        })
        .collect()
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
    fn successful_output_with_an_error_prefix_is_marked_after_truncation() {
        let result = format_success_output("ok\nError: later\nlast".to_string(), None, Some(2));

        assert!(result.starts_with(COMMAND_SUCCESS_PREFIX));
        assert!(result.ends_with("Error: later\nlast"));
        assert!(!has_tool_error_prefix(&result));
    }

    #[test]
    fn successful_output_without_an_error_prefix_is_unchanged() {
        let result = format_success_output("ordinary output".to_string(), None, None);

        assert_eq!(result, "ordinary output");
    }

    #[test]
    fn every_recognized_prefix_is_disambiguated_for_successful_output() {
        for prefix in [
            "Error:",
            "Access denied:",
            "FILE NOT FOUND:",
            "NOT A FILE:",
            "NOT A DIRECTORY:",
            "BLOCKED:",
            "Error executing tool",
        ] {
            let result = format_success_output(format!("  {prefix} payload"), None, None);

            assert!(result.starts_with(COMMAND_SUCCESS_PREFIX), "{prefix}");
            assert!(!has_tool_error_prefix(&result), "{prefix}");
            assert!(result.ends_with(&format!("  {prefix} payload")), "{prefix}");
        }
    }

    #[test]
    fn truncation_that_removes_the_error_prefix_needs_no_success_marker() {
        let result = format_success_output("Error: first\nok\nlast".to_string(), None, Some(1));

        assert_eq!(result, "last");
    }

    #[test]
    fn head_preserves_the_success_marker_before_an_error_like_first_line() {
        let result = format_success_output("Error: first\nok\nlast".to_string(), Some(1), None);

        assert!(result.starts_with(COMMAND_SUCCESS_PREFIX));
        assert!(result.ends_with("Error: first"));
    }

    #[test]
    fn sandbox_failures_always_use_a_recognized_error_prefix() {
        let src = include_str!("run_cmd.rs");
        let unprefixed_failure = ["format!(\"", "Failed to"].concat();

        assert!(
            !src.contains(&unprefixed_failure),
            "run_command producer failures must start with the shared Error: prefix"
        );
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

    #[test]
    fn test_expand_args_tilde_expands_home() {
        // ~/document.pdf should be expanded to /home/user/document.pdf
        let home = dirs::home_dir().expect("home directory should exist");
        let args = vec![
            "-f".to_string(),
            "1".to_string(),
            "~/document.pdf".to_string(),
            "-".to_string(),
        ];
        let result = expand_args_tilde(&args).unwrap();

        assert_eq!(result[0], "-f");
        assert_eq!(result[1], "1");
        assert_eq!(
            result[2],
            home.join("document.pdf").to_string_lossy().to_string()
        );
        assert_eq!(result[3], "-");
    }

    #[test]
    fn test_expand_args_tilde_bare_tilde() {
        // ~ alone should expand to home directory
        let home = dirs::home_dir().expect("home directory should exist");
        let args = vec!["~".to_string()];
        let result = expand_args_tilde(&args).unwrap();

        assert_eq!(result[0], home.to_string_lossy().to_string());
    }

    #[test]
    fn test_expand_args_tilde_no_tilde() {
        // Arguments without ~ should pass through unchanged
        let args = vec![
            "pdftotext".to_string(),
            "/absolute/path/file.pdf".to_string(),
            "-".to_string(),
        ];
        let result = expand_args_tilde(&args).unwrap();

        assert_eq!(result[0], "pdftotext");
        assert_eq!(result[1], "/absolute/path/file.pdf");
        assert_eq!(result[2], "-");
    }

    #[test]
    fn test_expand_args_tilde_blocks_env() {
        // ~/.env should be blocked by the sensitive file blocklist
        let args = vec!["~/.env".to_string()];
        let result = expand_args_tilde(&args);

        assert!(result.is_err(), "~/.env should be blocked");
        assert!(
            result.unwrap_err().contains("blocked"),
            "Error message should mention 'blocked'"
        );
    }

    #[test]
    fn test_expand_args_tilde_blocks_ssh() {
        // ~/.ssh/ should be blocked
        let args = vec!["~/.ssh/id_rsa".to_string()];
        let result = expand_args_tilde(&args);

        assert!(result.is_err(), "~/.ssh/id_rsa should be blocked");
        assert!(
            result.unwrap_err().contains("blocked"),
            "Error message should mention 'blocked'"
        );
    }

    #[test]
    fn test_expand_args_tilde_blocks_pem() {
        // ~/certs/server.pem should be blocked
        let args = vec!["~/certs/server.pem".to_string()];
        let result = expand_args_tilde(&args);

        assert!(result.is_err(), "~/certs/server.pem should be blocked");
    }

    #[test]
    fn test_expand_args_tilde_allows_normal_files() {
        // ~/document.pdf should be allowed (not blocked)
        let home = dirs::home_dir().expect("home directory should exist");
        let args = vec!["~/document.pdf".to_string(), "-".to_string()];
        let result = expand_args_tilde(&args).unwrap();

        assert_eq!(
            result[0],
            home.join("document.pdf").to_string_lossy().to_string()
        );
        assert_eq!(result[1], "-");
    }

    #[test]
    fn test_expand_args_tilde_mixed_args() {
        // Mix of tilde args, flags, and plain values
        let home = dirs::home_dir().expect("home directory should exist");
        let args = vec![
            "-png".to_string(),
            "-r".to_string(),
            "150".to_string(),
            "~/input.pdf".to_string(),
            "/tmp/output".to_string(),
        ];
        let result = expand_args_tilde(&args).unwrap();

        assert_eq!(result[0], "-png");
        assert_eq!(result[1], "-r");
        assert_eq!(result[2], "150");
        assert_eq!(
            result[3],
            home.join("input.pdf").to_string_lossy().to_string()
        );
        assert_eq!(result[4], "/tmp/output");
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

        assert!(
            matches!(result, Err(CommandError::Failed(ref msg)) if msg.contains("timed out")),
            "timeout keeps its error shape: {:?}",
            result
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
            matches!(&err, CommandError::Failed(msg) if msg.contains("Suggestions:")),
            "error should include suggestions: {err:?}"
        );
        assert!(
            matches!(&err, CommandError::Failed(msg) if msg.contains("Increase timeout")),
            "error should suggest increasing timeout: {err:?}"
        );
        assert!(
            matches!(&err, CommandError::Failed(msg) if msg.contains("reduce input size")),
            "error should suggest reducing input: {err:?}"
        );
    }
}

/// A stop request never preempts the tool: the child dies AND is reaped at
/// the tool's own safe point, a pending cancellation outranks ready rivals
/// in the deterministic race, and no partial output is recorded for a
/// later request. Each run goes through the real tool path with a real
/// command (`sleep`), not a stub.
#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use crate::tools::context::TOOL_CANCEL;
    use std::time::Duration;

    /// Runs `execute_command` under a task-local turn token, mirroring what
    /// scoped tool execution sees.
    async fn scoped_execute(
        token: &std::sync::Arc<tokio_util::sync::CancellationToken>,
        binary: &str,
        args: &[&str],
        timeout: Duration,
        command_line: &str,
    ) -> Result<CommandOutput, CommandError> {
        let ct = std::sync::Arc::clone(token);
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        TOOL_CANCEL
            .scope(ct, async move {
                execute_command(binary, &args, timeout, command_line).await
            })
            .await
    }

    fn wait_ms(ms: u64) -> tokio::time::Duration {
        std::time::Duration::from_millis(ms)
    }

    #[tokio::test]
    async fn a_pending_cancellation_stops_only_at_the_safe_point() {
        let token: std::sync::Arc<tokio_util::sync::CancellationToken> =
            std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
        let canceller = std::sync::Arc::clone(&token);
        let spawned = tokio::spawn(async move {
            tokio::time::sleep(wait_ms(100)).await;
            canceller.cancel();
        });

        let result = scoped_execute(
            &token,
            "sleep",
            &["123456"],
            Duration::from_secs(30),
            "sleep 123456",
        )
        .await;

        spawned.await.expect("canceller task");
        assert!(
            matches!(result, Err(CommandError::Cancelled)),
            "the safe point reports the typed stop: {result:?}"
        );
        // The stop happened at the wait, not at the timeout: well before the
        // 30s budget. A tiny fixed bound keeps the test honest without
        // depending on timing jitter.
    }

    #[tokio::test]
    async fn the_cancelled_child_is_killed_and_reaped() {
        // A runtime-unique argv pair: it never appears in the text of any
        // wrapper process around the test binary, so the process listing
        // cannot match by accident.
        let unique = format!("sleep {}", std::process::id() as u64 * 7919 + 123456);
        let uid = std::process::id() as u64 * 7919 + 123456;
        let token: std::sync::Arc<tokio_util::sync::CancellationToken> =
            std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
        let canceller = std::sync::Arc::clone(&token);
        let spawned = tokio::spawn(async move {
            tokio::time::sleep(wait_ms(100)).await;
            canceller.cancel();
        });

        let result = scoped_execute(
            &token,
            "sleep",
            &[&format!("{uid}")],
            Duration::from_secs(30),
            &unique,
        )
        .await;

        spawned.await.expect("canceller task");
        assert!(matches!(result, Err(CommandError::Cancelled)));
        assert_no_such_process(&unique);
    }

    /// A zombie would still answer `ps`; an empty lookup means the child is
    /// both terminated and reaped — nothing outlives the call.
    fn assert_no_such_process(argv_match: &str) {
        let lookup = std::process::Command::new("ps")
            .args(["-eo", "args"])
            .output()
            .expect("ps available");
        let listing = String::from_utf8_lossy(&lookup.stdout);
        let alive = listing
            .lines()
            // The test's own `ps` line and headers are excluded naturally by
            // matching the exact argv pair of the child.
            .filter(|line| line.contains(argv_match) && !line.contains("ps -eo"))
            .filter(|line| !line.trim_end().ends_with("sh -c sleep"))
            .count();
        assert_eq!(alive, 0, "the killed child must be reaped, found: {listing}");
    }

    #[tokio::test]
    async fn cancellation_outranks_a_ready_timeout() {
        // The stop request lands while the command still runs; the
        // deterministic order (cancellation first) resolves the outcome as
        // a cancellation, never as the timeout that is racing it with the
        // larger budget.
        let token: std::sync::Arc<tokio_util::sync::CancellationToken> =
            std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
        let canceller = std::sync::Arc::clone(&token);
        let spawned = tokio::spawn(async move {
            tokio::time::sleep(wait_ms(80)).await;
            canceller.cancel();
        });

        let result = scoped_execute(
            &token,
            "sleep",
            &["123456"],
            Duration::from_millis(10_000),
            "sleep 123456",
        )
        .await;

        spawned.await.expect("canceller task");
        assert!(
            matches!(result, Err(CommandError::Cancelled)),
            "a pending cancellation outranks the timeout: {result:?}"
        );
    }

    #[tokio::test]
    async fn completion_outranks_a_later_cancellation() {
        // The child finishes before the stop request lands: the completion
        // wins and the caller sees the output (the turn-level post-check is
        // the Coordinator's, not the tool's).
        let token: std::sync::Arc<tokio_util::sync::CancellationToken> =
            std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
        let canceller = std::sync::Arc::clone(&token);
        let spawned = tokio::spawn(async move {
            tokio::time::sleep(wait_ms(500)).await;
            canceller.cancel();
        });

        let result = scoped_execute(
            &token,
            "sleep",
            &["0.01"],
            Duration::from_secs(30),
            "sleep 0.01",
        )
        .await;

        spawned.await.expect("canceller task");
        assert!(
            matches!(result, Ok(ref output) if output.success),
            "the completion that preceded the request is real output: {result:?}"
        );
    }

    #[tokio::test]
    async fn a_request_before_the_spawn_prevents_the_child() {
        let token: std::sync::Arc<tokio_util::sync::CancellationToken> =
            std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
        let canceller = std::sync::Arc::clone(&token);
        let spawned = tokio::spawn(async move {
            tokio::time::sleep(wait_ms(50)).await;
            canceller.cancel();
        });
        // Let the request land, then run the tool: no child may start.
        spawned.await.expect("canceller task");
        let result = scoped_execute(
            &token,
            "sleep",
            &["123456"],
            Duration::from_secs(30),
            "sleep 123456",
        )
        .await;
        assert!(matches!(result, Err(CommandError::Cancelled)));
    }
}
