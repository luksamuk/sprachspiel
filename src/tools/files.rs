use super::files_blocklist::{is_blocked_for_read, BlocklistConfig};
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::{format_size, parse_bool, parse_u32};
use regex::Regex;
use std::path::{Path, PathBuf};

const MAX_FILE_SIZE: usize = 1_000_000; // 1MB max file size
const MAX_RESULTS: usize = 100; // Maximum search results

/// Read the contents of a text file.
///
/// Reads and returns the contents of a file. Use this to examine code,
/// configuration files, or any text-based documents.
///
/// # Arguments
/// * `path` - Path to the file (relative to current directory or absolute).
///   - Examples: "README.md", "src/main.rs", "/etc/config.yml"
/// * `max_lines` - Maximum number of lines to read (default: all). Optional.
///   - Use for large files to avoid context pollution.
///   - Example: "100" to read first 100 lines
/// * `sandbox` - Restrict to current directory tree (default: true). Optional.
///   - "true" (default): Only allow files within current directory tree
///   - "false": Allow any absolute path
///
/// # Returns
/// The file contents with line numbers, or an error message.
/// For files over 1MB, use count_lines first, then read_file_segment.
///
/// # Errors
/// Returns error message if file doesn't exist, is not readable, or is too large.
#[ollama_rs::function]
pub async fn read_file(
    path: String,
    max_lines: Option<String>,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let max_lines_parsed = parse_u32(max_lines, None);
    let sandbox_parsed = parse_bool(sandbox, true);

    log_tool_call(
        "read_file",
        &[
            ("path".to_string(), path.clone()),
            (
                "max_lines".to_string(),
                max_lines_parsed
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "all".to_string()),
            ),
        ],
    );

    // Validate and canonicalize path (also checks if exists)
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox_parsed) {
        Ok(p) => p,
        Err(e) => {
            // validate_path already returns a complete error message
            let err_msg = e.to_string();
            log_tool_result("read_file", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check blocklist for sensitive files
    let config = BlocklistConfig::load();
    if is_blocked_for_read(&canonical_path, &config) {
        let err_msg = format!(
            "Error: BLOCKED - '{}' matches a protected file pattern. \
             This file may contain sensitive information (credentials, secrets, keys). \
             Reading such files is restricted for security.",
            path
        );
        log_tool_result("read_file", &err_msg);
        return Ok(err_msg);
    }

    // Check if it's a file (validate_path already confirmed it exists)
    if !canonical_path.is_file() {
        let err_msg = format!(
            "Error: NOT A FILE: '{}'. The path exists but is not a file (it may be a directory). Use list_directory to see contents.",
            path
        );
        log_tool_result("read_file", &err_msg);
        return Ok(err_msg);
    }

    // Check file size
    let metadata = match std::fs::metadata(&canonical_path) {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("Error: Cannot read file metadata: {}", e);
            log_tool_result("read_file", &err_msg);
            return Ok(err_msg);
        }
    };
    if metadata.len() > MAX_FILE_SIZE as u64 {
        let err_msg = format!(
            "Error: File too large ({:.1} MB). Use count_lines to check file size, then read_file_segment to read in chunks.",
            metadata.len() as f64 / 1_000_000.0
        );
        log_tool_result("read_file", &err_msg);
        return Ok(err_msg);
    }

    // Read file content
    let content = match std::fs::read_to_string(&canonical_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Error: Cannot read file: {}", e);
            log_tool_result("read_file", &err_msg);
            return Ok(err_msg);
        }
    };

    // Apply max_lines limit if specified
    let result = if let Some(lines) = max_lines_parsed {
        let lines_to_take = lines as usize;
        content
            .lines()
            .take(lines_to_take)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content
    };

    log_tool_result("read_file", &result);
    Ok(result)
}

/// Read a specific segment of a file (from start_line for num_lines).
/// Useful for reading parts of large files without loading the entire file.
/// Read a specific range of lines from a file.
///
/// Use this to read large files in chunks instead of loading the entire file.
/// Ideal for examining specific sections of code or log files.
///
/// # Arguments
/// * `path` - Path to the file (relative to current directory or absolute).
///   - Examples: "src/main.rs", "/var/log/app.log"
/// * `start_line` - Line number to start reading from (1-indexed). Required.
///   - Example: "1" to start from the beginning
/// * `num_lines` - Number of lines to read. Required.
///   - Example: "50" to read 50 lines
/// * `sandbox` - Restrict to current directory tree (default: true). Optional.
///   - "true" (default): Only allow files within current directory tree
///   - "false": Allow any absolute path
///
/// # Returns
/// The specified lines with line numbers, or an error message.
///
/// # Errors
/// Returns error message if file doesn't exist, start_line is invalid, or num_lines is 0.
#[ollama_rs::function]
pub async fn read_file_segment(
    path: String,
    start_line: String,
    num_lines: String,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let start_line_parsed = match parse_u32(Some(start_line.clone()), None) {
        Some(n) => n,
        None => {
            let err_msg = format!(
                "Error: Invalid start_line '{}'. Must be a positive number.",
                start_line
            );
            log_tool_result("read_file_segment", &err_msg);
            return Ok(err_msg);
        }
    };
    let num_lines_parsed = match parse_u32(Some(num_lines.clone()), None) {
        Some(n) => n,
        None => {
            let err_msg = format!(
                "Error: Invalid num_lines '{}'. Must be a positive number.",
                num_lines
            );
            log_tool_result("read_file_segment", &err_msg);
            return Ok(err_msg);
        }
    };
    let sandbox_parsed = parse_bool(sandbox, true);

    if start_line_parsed == 0 {
        let err_msg =
            "Error: start_line must be 1 or greater. Line numbers start at 1.".to_string();
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    if num_lines_parsed == 0 {
        let err_msg = "Error: num_lines must be 1 or greater.".to_string();
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    log_tool_call(
        "read_file_segment",
        &[
            ("path".to_string(), path.clone()),
            ("start_line".to_string(), start_line_parsed.to_string()),
            ("num_lines".to_string(), num_lines_parsed.to_string()),
        ],
    );

    // Validate and canonicalize path (also checks if exists)
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox_parsed) {
        Ok(p) => p,
        Err(e) => {
            // validate_path already returns a complete error message
            let err_msg = e.to_string();
            log_tool_result("read_file_segment", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check blocklist for sensitive files
    let config = BlocklistConfig::load();
    if is_blocked_for_read(&canonical_path, &config) {
        let err_msg = format!(
            "Error: BLOCKED - '{}' matches a protected file pattern. \
             This file may contain sensitive information (credentials, secrets, keys). \
             Reading such files is restricted for security.",
            path
        );
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    // Check if it's a file (validate_path already confirmed it exists)
    if !canonical_path.is_file() {
        let err_msg = format!(
            "Error: NOT A FILE: '{}'. The path exists but is not a file (it may be a directory). Use list_directory to see contents.",
            path
        );
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    // Check file size
    let metadata = match std::fs::metadata(&canonical_path) {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("Error: Cannot read file metadata: {}", e);
            log_tool_result("read_file_segment", &err_msg);
            return Ok(err_msg);
        }
    };
    if metadata.len() > MAX_FILE_SIZE as u64 {
        let err_msg = format!(
            "Error: File too large ({:.1} MB). Use count_lines to check file size, then read_file_segment to read in chunks.",
            metadata.len() as f64 / 1_000_000.0
        );
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    // Read file content
    let content = match std::fs::read_to_string(&canonical_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Error: Cannot read file: {}", e);
            log_tool_result("read_file_segment", &err_msg);
            return Ok(err_msg);
        }
    };

    // Extract segment
    let start = start_line_parsed as usize;
    let count = num_lines_parsed as usize;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if start == 0 || start > total_lines {
        let err_msg = format!(
            "Error: Invalid start_line {}. File has {} lines. Line numbers start at 1.",
            start, total_lines
        );
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    let start_idx = start - 1; // Convert to 0-based index
    let end_idx = std::cmp::min(start_idx + count, total_lines);
    let segment_lines: Vec<&str> = lines[start_idx..end_idx].to_vec();

    let result = if segment_lines.is_empty() {
        format!(
            "File has {} lines. No lines to read from line {}.",
            total_lines, start
        )
    } else {
        let mut output = Vec::new();
        let end_line = start + segment_lines.len() - 1;
        output.push(format!("Lines {}-{} of {}:", start, end_line, total_lines));
        output.push("-".repeat(40));
        for (i, line) in segment_lines.iter().enumerate() {
            output.push(format!("{:>6} | {}", start + i, line));
        }
        output.join("\n")
    };

    log_tool_result("read_file_segment", &result);
    Ok(result)
}

/// Count the number of lines in a file.
///
/// Use this before reading large files to determine if you need to use
/// read_file_segment. Helps avoid polluting context with huge files.
///
/// # Arguments
/// * `path` - Path to the file (relative to current directory or absolute).
///   - Examples: "large_file.txt", "src/module.rs"
/// * `sandbox` - Restrict to current directory tree (default: true). Optional.
///   - "true" (default): Only allow files within current directory tree
///   - "false": Allow any absolute path
///
/// # Returns
/// File information including:
/// - Total line count
/// - File size in human-readable format (KB/MB)
/// - Line count recommendation for reading strategy
///
/// # Errors
/// Returns error message if file doesn't exist or is not readable.
#[ollama_rs::function]
pub async fn count_lines(
    path: String,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let sandbox_parsed = parse_bool(sandbox, true);

    log_tool_call("count_lines", &[("path".to_string(), path.clone())]);

    // Validate and canonicalize path (also checks if exists)
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox_parsed) {
        Ok(p) => p,
        Err(e) => {
            // validate_path already returns a complete error message
            let err_msg = e.to_string();
            log_tool_result("count_lines", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check blocklist for sensitive files
    let config = BlocklistConfig::load();
    if is_blocked_for_read(&canonical_path, &config) {
        let err_msg = format!(
            "Error: BLOCKED - '{}' matches a protected file pattern. \
             This file may contain sensitive information (credentials, secrets, keys). \
             Reading such files is restricted for security.",
            path
        );
        log_tool_result("count_lines", &err_msg);
        return Ok(err_msg);
    }

    // Check if it's a file (validate_path already confirmed it exists)
    if !canonical_path.is_file() {
        let err_msg = format!(
            "Error: NOT A FILE: '{}'. The path exists but is not a file (it may be a directory). Use list_directory to see contents.",
            path
        );
        log_tool_result("count_lines", &err_msg);
        return Ok(err_msg);
    }

    // Read and count lines
    let content = match std::fs::read_to_string(&canonical_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Error: Cannot read file: {}", e);
            log_tool_result("count_lines", &err_msg);
            return Ok(err_msg);
        }
    };

    let line_count = content.lines().count();

    // Suggest using read_file_segment for large files
    let suggestion = if line_count > 100 {
        "\n\nTip: Use read_file_segment(path, start_line, num_lines) to read specific sections and avoid polluting the context window.".to_string()
    } else {
        String::new()
    };

    let result = format!("File: {}\nLines: {}{}", path, line_count, suggestion);

    log_tool_result("count_lines", &result);
    Ok(result)
}

/// List contents of a directory.
///
/// Shows files and subdirectories in a directory. Use this to explore
/// project structure or find specific files.
///
/// # Arguments
/// * `path` - Path to the directory (relative to current directory or absolute).
///   - Examples: ".", "src", "/home/user/projects"
/// * `recursive` - List subdirectories recursively (default: false). Optional.
///   - "true": List all files in subdirectories
///   - "false" (default): List only immediate contents
/// * `sandbox` - Restrict to current directory tree (default: true). Optional.
///   - "true" (default): Only allow directories within current directory tree
///   - "false": Allow any absolute path
///
/// # Returns
/// Directory listing with:
/// - File/directory names with type indicators ([file], [dir], [symlink])
/// - File sizes for regular files
/// - Tree structure for recursive listings
///
/// # Errors
/// Returns error message if directory doesn't exist or is not accessible.
#[ollama_rs::function]
pub async fn list_directory(
    path: String,
    recursive: Option<String>,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let recursive_parsed = parse_bool(recursive, false);
    let sandbox_parsed = parse_bool(sandbox, true);

    log_tool_call(
        "list_directory",
        &[
            ("path".to_string(), path.clone()),
            ("recursive".to_string(), recursive_parsed.to_string()),
        ],
    );

    // Validate and canonicalize path (also checks if exists)
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox_parsed) {
        Ok(p) => p,
        Err(e) => {
            // validate_path already returns a complete error message
            let err_msg = e.to_string();
            log_tool_result("list_directory", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check if it's a directory (validate_path already confirmed it exists)
    if !canonical_path.is_dir() {
        let err_msg = format!(
            "Error: NOT A DIRECTORY: '{}'. The path exists but is not a directory (it may be a file).",
            path
        );
        log_tool_result("list_directory", &err_msg);
        return Ok(err_msg);
    }

    // List directory contents
    let mut entries = Vec::new();

    if recursive_parsed {
        if let Err(e) =
            collect_entries_recursive(&canonical_path, &canonical_path, &mut entries, 0, 10)
        {
            let err_msg = format!("Error: Failed to list directory recursively: {}", e);
            log_tool_result("list_directory", &err_msg);
            return Ok(err_msg);
        }
    } else {
        let read_dir = match std::fs::read_dir(&canonical_path) {
            Ok(rd) => rd,
            Err(e) => {
                let err_msg = format!("Error: Cannot read directory: {}", e);
                log_tool_result("list_directory", &err_msg);
                return Ok(err_msg);
            }
        };
        for entry in read_dir {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    continue; // Skip entries that fail
                }
            };
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue, // Skip entries without metadata
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let entry_type = if metadata.is_dir() {
                "dir"
            } else if metadata.is_file() {
                "file"
            } else if metadata.is_symlink() {
                "symlink"
            } else {
                "other"
            };
            let size = if metadata.is_file() {
                format!(" ({})", format_size(metadata.len()))
            } else {
                String::new()
            };
            entries.push(format!("[{}] {}{}", entry_type, name, size));
        }
    }

    // Sort entries for consistent output
    entries.sort();

    let result = if entries.is_empty() {
        "Directory is empty".to_string()
    } else {
        entries.join("\n")
    };

    log_tool_result("list_directory", &result);
    Ok(result)
}

fn collect_entries_recursive(
    base_path: &Path,
    current_path: &Path,
    entries: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if depth > max_depth {
        entries.push(format!("{}... (max depth reached)", "  ".repeat(depth)));
        return Ok(());
    }

    let read_dir = match std::fs::read_dir(current_path) {
        Ok(rd) => rd,
        Err(_) => return Ok(()), // Skip directories we can't read
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // Skip failed entries
        };
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue, // Skip entries without metadata
        };

        // Calculate relative path from base
        let full_path = entry.path();
        let relative_path = full_path
            .strip_prefix(base_path)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| full_path.clone());
        let display_path = relative_path.to_string_lossy();

        let indent = "  ".repeat(depth);
        let entry_type = if metadata.is_dir() {
            "dir"
        } else if metadata.is_file() {
            "file"
        } else if metadata.is_symlink() {
            "symlink"
        } else {
            "other"
        };

        let size_info = if metadata.is_file() {
            format!(" ({})", format_size(metadata.len()))
        } else {
            String::new()
        };

        entries.push(format!(
            "{}[{}] {}{}",
            indent, entry_type, display_path, size_info
        ));

        // Recurse into subdirectories
        if metadata.is_dir() {
            let _ = collect_entries_recursive(base_path, &full_path, entries, depth + 1, max_depth);
        }
    }

    Ok(())
}

/// Search for a text pattern in files using regex.
///
/// Searches for a regular expression pattern in files within a directory.
/// Useful for finding code, configuration values, or specific text.
///
/// # Arguments
/// * `pattern` - Regular expression pattern to search for.
///   - Examples: "fn main", "import.*react", "TODO", "error.*handler"
/// * `path` - Directory to search in (relative to current directory or absolute).
///   - Examples: ".", "src", "/home/user/project"
/// * `file_pattern` - Glob pattern to filter files (default: all files). Optional.
///   - Examples: "*.rs", "*.py", "*.js", "*.txt"
/// * `sandbox` - Restrict to current directory tree (default: true). Optional.
///   - "true" (default): Only search within current directory tree
///   - "false": Allow searching any directory
///
/// # Returns
/// Search results with:
/// - File paths and line numbers where matches were found
/// - Matching line content with highlighted match
/// - Total match count
///
/// # Errors
/// Returns error message for invalid regex pattern or inaccessible directory.
#[ollama_rs::function]
pub async fn search_files(
    pattern: String,
    path: String,
    file_pattern: Option<String>,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let sandbox_parsed = parse_bool(sandbox, true);

    log_tool_call(
        "search_files",
        &[
            ("pattern".to_string(), pattern.clone()),
            ("path".to_string(), path.clone()),
            (
                "file_pattern".to_string(),
                file_pattern.clone().unwrap_or_default(),
            ),
        ],
    );

    // Compile regex pattern
    let regex = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!(
                "Error: Invalid regex pattern '{}': {}. Please check your regex syntax.",
                pattern, e
            );
            log_tool_result("search_files", &err_msg);
            return Ok(err_msg);
        }
    };

    // Validate path (also checks if exists)
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox_parsed) {
        Ok(p) => p,
        Err(e) => {
            // validate_path already returns a complete error message
            let err_msg = e.to_string();
            log_tool_result("search_files", &err_msg);
            return Ok(err_msg);
        }
    };

    // Determine search scope
    let files_to_search = if canonical_path.is_file() {
        vec![canonical_path.clone()]
    } else {
        match collect_files(&canonical_path, file_pattern.as_deref(), MAX_RESULTS) {
            Ok(f) => f,
            Err(e) => {
                let err_msg = format!("Error: {}", e);
                log_tool_result("search_files", &err_msg);
                return Ok(err_msg);
            }
        }
    };

    // Filter out blocked files
    let config = BlocklistConfig::load();
    let files_to_search: Vec<_> = files_to_search
        .into_iter()
        .filter(|f| !is_blocked_for_read(f, &config))
        .collect();

    // Search for pattern
    let mut matches = Vec::new();
    let mut files_searched = 0;

    for file_path in files_to_search {
        // Skip files that are too large
        let metadata = match std::fs::metadata(&file_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.len() > MAX_FILE_SIZE as u64 {
            continue;
        }

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue, // Skip binary or unreadable files
        };

        files_searched += 1;

        // Find matches in this file
        for (line_num, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                let relative_path = file_path
                    .strip_prefix(canonical_path.parent().unwrap_or(&canonical_path))
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

                matches.push(format!(
                    "{}:{}: {}",
                    relative_path,
                    line_num + 1,
                    line.trim()
                ));

                if matches.len() >= MAX_RESULTS {
                    matches.push(format!("... (stopped after {} matches)", MAX_RESULTS));
                    break;
                }
            }
        }
    }

    let result = if matches.is_empty() {
        format!(
            "No matches found for pattern '{}' in {} files",
            pattern, files_searched
        )
    } else {
        format!(
            "Found {} matches in {} files:\n{}",
            matches.len(),
            files_searched,
            matches.join("\n")
        )
    };

    log_tool_result("search_files", &result);
    Ok(result)
}

fn collect_files(
    dir: &Path,
    file_pattern: Option<&str>,
    max_files: usize,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let mut files = Vec::new();
    let pattern_regex = file_pattern
        .map(|p| Regex::new(&glob_to_regex(p)))
        .transpose()
        .map_err(|e| format!("Invalid file pattern: {}", e))?;

    for entry in walkdir::WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            let path = entry.path();

            // Check file pattern if specified
            if let Some(ref regex) = pattern_regex {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                if !regex.is_match(&filename) {
                    continue;
                }
            }

            files.push(path.to_path_buf());

            if files.len() >= max_files {
                break;
            }
        }
    }

    Ok(files)
}

/// Convert glob pattern to regex
fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::new();
    regex.push('^');

    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' => regex.push_str("\\."),
            '{' => regex.push('('),
            '}' => regex.push(')'),
            ',' => regex.push('|'),
            '[' | ']' | '(' | ')' | '^' | '$' | '+' | '\\' | '|' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }

    regex.push('$');
    regex
}

/// Validate that a path is within the sandbox (current working directory)
fn validate_path(
    path: &Path,
    sandbox: bool,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    // Get the absolute path
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| "Could not determine current directory")?
            .join(path)
    };

    // Check if path exists before canonicalizing
    if !abs_path.exists() {
        return Err(format!(
            "FILE NOT FOUND: '{}'. The file or directory does not exist. Use list_directory to see available files.",
            path.display()
        ).into());
    }

    // Canonicalize to resolve symlinks and normalize
    let canonical_path = abs_path
        .canonicalize()
        .map_err(|e| format!("Cannot access path '{}': {}", path.display(), e))?;

    if sandbox {
        // Get current working directory
        let cwd = std::env::current_dir().map_err(|_| "Could not determine current directory")?;
        let canonical_cwd = cwd
            .canonicalize()
            .map_err(|_| "Could not determine current directory")?;

        // Check that the path starts with cwd
        if !canonical_path.starts_with(&canonical_cwd) {
            return Err(format!(
                "Path '{}' is outside the allowed directory. \
                 File operations are sandboxed to the current working directory.",
                path.display()
            )
            .into());
        }
    }

    Ok(canonical_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_to_regex() {
        assert_eq!(glob_to_regex("*.rs"), "^.*\\.rs$");
        // ? becomes . (any char), . becomes \\. (literal)
        assert_eq!(glob_to_regex("file?.txt"), "^file.\\.txt$");
        assert_eq!(glob_to_regex("test.{js,ts}"), "^test\\.(js|ts)$");
        assert_eq!(glob_to_regex("*.min.js"), "^.*\\.min\\.js$");
    }

    #[test]
    fn test_validate_path_within_cwd() {
        let cwd = std::env::current_dir().unwrap();
        let relative = PathBuf::from("src/main.rs");

        let result = validate_path(&relative, true);
        assert!(result.is_ok());

        // The validated path should be absolute and canonical
        let validated = result.unwrap();
        assert!(validated.is_absolute());
        assert!(validated.starts_with(&cwd));
    }

    #[test]
    fn test_validate_path_outside_cwd() {
        // This should fail when sandbox is enabled
        let outside_path = PathBuf::from("/etc/passwd");
        let result = validate_path(&outside_path, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_no_sandbox() {
        // Should succeed even outside CWD when sandbox is disabled
        let outside_path = PathBuf::from("/tmp");
        let result = validate_path(&outside_path, false);
        // This might fail if /tmp doesn't exist, but should not fail due to sandbox
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(!err_msg.contains("sandboxed"));
        }
    }
}
