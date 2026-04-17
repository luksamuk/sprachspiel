use super::files_blocklist::{BlocklistConfig, is_blocked_for_list, is_blocked_for_read};
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::{expand_tilde_path, format_size, parse_bool, parse_u32};
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
///   - Examples: "README.md", "src/main.rs"
/// * `max_lines` - Maximum number of lines to read (default: all). Optional.
///   - Use for large files to avoid context pollution.
///   - Example: "100" to read first 100 lines
///
/// # Security
/// - File access is always sandboxed to the current working directory
/// - Blocked patterns (`.env`, secrets, SSH keys) are always enforced
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
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let max_lines_parsed = parse_u32(max_lines, None);

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
    let path_buf = expand_tilde_path(&path);
    let canonical_path = match validate_path(&path_buf) {
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
        let total_lines = content.lines().count();
        if lines_to_take >= total_lines {
            // Requested more lines than the file has — no truncation needed
            content
        } else {
            let truncated: String = content
                .lines()
                .take(lines_to_take)
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{}\n\n[TRUNCATED: Showing lines 1-{} of {}. Use read_file_segment to read more.]",
                truncated, lines_to_take, total_lines
            )
        }
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
///   - Examples: "src/main.rs", "logs/app.log"
/// * `start_line` - Line number to start reading from (1-indexed). Required.
///   - Example: "1" to start from the beginning
/// * `num_lines` - Number of lines to read. Required.
///   - Example: "50" to read 50 lines
///
/// # Security
/// - File access is always sandboxed to the current working directory
/// - Blocked patterns (`.env`, secrets, SSH keys) are always enforced
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
    let path_buf = expand_tilde_path(&path);
    let canonical_path = match validate_path(&path_buf) {
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
///
/// # Security
/// - File access is always sandboxed to the current working directory
/// - Blocked patterns (`.env`, secrets, SSH keys) are always enforced
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
pub async fn count_lines(path: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("count_lines", &[("path".to_string(), path.clone())]);

    // Validate and canonicalize path (also checks if exists)
    let path_buf = expand_tilde_path(&path);
    let canonical_path = match validate_path(&path_buf) {
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
///   - Examples: ".", "src"
/// * `recursive` - List subdirectories recursively (default: false). Optional.
///   - "true": List all files in subdirectories
///   - "false" (default): List only immediate contents
///
/// # Security
/// - File access is always sandboxed to the current working directory
/// - Respects `block_list` configuration (blocked filenames shown as "[BLOCKED]")
/// - Blocked patterns from tools.toml are applied to hide sensitive filenames
///
/// # Returns
/// Formatted list with [type] prefix (file/dir/symlink) and sizes.
///
/// # Errors
/// Returns error message if directory doesn't exist or is not accessible.
#[ollama_rs::function]
pub async fn list_directory(
    path: String,
    recursive: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let recursive_parsed = parse_bool(recursive, false);

    log_tool_call(
        "list_directory",
        &[
            ("path".to_string(), path.clone()),
            ("recursive".to_string(), recursive_parsed.to_string()),
        ],
    );

    // Load blocklist configuration
    let config = BlocklistConfig::load();

    // Validate and canonicalize path (also checks if exists)
    let path_buf = expand_tilde_path(&path);
    let canonical_path = match validate_path(&path_buf) {
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
        if let Err(e) = collect_entries_recursive(
            &canonical_path,
            &canonical_path,
            &mut entries,
            0,
            10,
            &config,
        ) {
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

            // Check blocklist for list operations
            let entry_path = entry.path();
            let display_name = if is_blocked_for_list(&entry_path, &config) {
                "[BLOCKED]".to_string()
            } else {
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
                format!("[{}] {}{}", entry_type, name, size)
            };
            entries.push(display_name);
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
    config: &BlocklistConfig,
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

        // Check blocklist for list operations
        if is_blocked_for_list(&full_path, config) {
            let indent = "  ".repeat(depth);
            entries.push(format!("{}[BLOCKED]", indent));
            continue;
        }

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
            let _ = collect_entries_recursive(
                base_path,
                &full_path,
                entries,
                depth + 1,
                max_depth,
                config,
            );
        }
    }

    Ok(())
}

/// Search for a text pattern in files using regex.
///
/// Searches for a regular expression pattern in files within a directory.
/// Useful for finding code, configuration values, or specific text.
/// Only works on text-based files (source code, config, markdown, etc.).
/// Binary files (PDFs, images, executables) are silently skipped.
///
/// # Arguments
/// * `pattern` - Rust regex pattern to search for (searches line by line).
///   - The `^` anchor matches the start of each line, `$` matches end of line.
///   - Use alternation with grouping: `"^(CHAPTER|Chapter|INTRODUCTION)"` instead of
///     `"^CHAPTER|^Chapter|^INTRODUCTION"` to avoid very long patterns that may get truncated.
///   - Examples: "fn main", "import.*react", "TODO", "error.*handler"
/// * `path` - Directory or file to search in (relative to current directory or absolute).
///   - Examples: ".", "src"
///   - If a file path is given, searches only that file.
/// * `file_pattern` - Glob pattern to filter which files to search (default: all files). Optional.
///   - Examples: "*.rs", "*.py", "*.js", "*.txt"
///   - Leave empty or omit to search all text files.
///
/// # Security
/// - File access is always sandboxed to the current working directory
/// - Blocked patterns (`.env`, secrets, SSH keys) are always enforced
///
/// # Returns
/// Search results with:
/// - File paths and line numbers where matches were found
/// - Matching line content (trimmed)
/// - Total match count (up to 100 results)
///
/// # Errors
/// Returns error message for invalid regex pattern or inaccessible directory.
#[ollama_rs::function]
pub async fn search_files(
    pattern: String,
    path: String,
    file_pattern: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty file_pattern to None (LLMs often send "" instead of omitting)
    let file_pattern = file_pattern.filter(|s| !s.is_empty());

    log_tool_call(
        "search_files",
        &[
            ("pattern".to_string(), pattern.clone()),
            ("path".to_string(), path.clone()),
            (
                "file_pattern".to_string(),
                file_pattern.as_deref().unwrap_or("all").to_string(),
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
    let path_buf = expand_tilde_path(&path);
    let canonical_path = match validate_path(&path_buf) {
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
                    matches.push(format!(
                        "[TRUNCATED: Showing {} matches. Refine your search pattern for fewer results.]",
                        MAX_RESULTS
                    ));
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

/// Validate that a path is within the sandbox (current working directory).
///
/// Sandbox is always enforced — the LLM cannot bypass this restriction.
/// This is a security boundary: the entity being restricted must never
/// be able to disable the restriction.
///
/// Allowed paths beyond CWD:
/// - `/tmp` — standard temporary directory (needed for tool interop)
/// - `/var/tmp` — persistent temporary directory
fn validate_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
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

    // Sandbox is ALWAYS enforced — check that the path is within CWD
    // or within allowed temporary directories
    let cwd = std::env::current_dir().map_err(|_| "Could not determine current directory")?;
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|_| "Could not determine current directory")?;

    // Check if path is within CWD
    if canonical_path.starts_with(&canonical_cwd) {
        return Ok(canonical_path);
    }

    // Allow /tmp and /var/tmp (needed for tool interop, e.g., pdftotext output)
    if is_temp_directory(&canonical_path) {
        return Ok(canonical_path);
    }

    Err(format!(
        "Path '{}' is outside the allowed directory. \
         File operations are restricted to the current working directory.",
        path.display()
    )
    .into())
}

/// Check if a canonical path is within an allowed temporary directory.
fn is_temp_directory(canonical_path: &Path) -> bool {
    // Check /tmp (standard temporary directory)
    if let Ok(canonical_tmp) = Path::new("/tmp").canonicalize()
        && canonical_path.starts_with(&canonical_tmp)
    {
        return true;
    }
    // Check /var/tmp (persistent temporary directory)
    if let Ok(canonical_var_tmp) = Path::new("/var/tmp").canonicalize()
        && canonical_path.starts_with(&canonical_var_tmp)
    {
        return true;
    }
    false
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

        let result = validate_path(&relative);
        assert!(result.is_ok());

        // The validated path should be absolute and canonical
        let validated = result.unwrap();
        assert!(validated.is_absolute());
        assert!(validated.starts_with(&cwd));
    }

    #[test]
    fn test_validate_path_outside_cwd() {
        // Sandbox is always enforced — paths outside CWD must always fail
        let outside_path = PathBuf::from("/etc/passwd");
        let result = validate_path(&outside_path);
        assert!(result.is_err());
    }

    // --- Tests for empty file_pattern normalization ---

    #[test]
    fn test_file_pattern_empty_string_normalization() {
        // When LLM sends file_pattern="" (empty string), it should be treated as None.
        // Without normalization, glob_to_regex("") produces "^$" which matches nothing.
        // With normalization, empty string becomes None and all files are searched.
        let empty: Option<String> = Some(String::new());
        let normalized = empty.filter(|s| !s.is_empty());
        assert!(
            normalized.is_none(),
            "Empty file_pattern should normalize to None"
        );

        // Non-empty pattern should remain Some
        let non_empty: Option<String> = Some("*.rs".to_string());
        let normalized = non_empty.filter(|s| !s.is_empty());
        assert_eq!(normalized, Some("*.rs".to_string()));

        // Already None should stay None
        let none_pattern: Option<String> = None;
        let normalized = none_pattern.filter(|s| !s.is_empty());
        assert!(normalized.is_none());
    }

    #[test]
    fn test_glob_to_regex_empty_pattern() {
        // Verify that glob_to_regex("") produces "^$" which matches no filenames.
        // This is the bug: when file_pattern="" is passed without normalization,
        // this regex matches nothing, causing search to return 0 results.
        let result = glob_to_regex("");
        assert_eq!(result, "^$");

        // Verify this regex doesn't match typical filenames
        let re = Regex::new(&result).unwrap();
        assert!(!re.is_match("main.rs"));
        assert!(!re.is_match("file.txt"));
        assert!(!re.is_match("README.md"));
        // Only matches the empty string
        assert!(re.is_match(""));
    }

    // --- Tests for regex patterns used in search_files ---

    #[test]
    fn test_regex_anchor_alternation_patterns() {
        // Test patterns like "^CHAPTER|^Chapter|^INTRODUCTION"
        // This is a common LLM-generated pattern that should work correctly.

        // Alternation with anchors on each branch
        let pattern = "^CHAPTER|^Chapter|^INTRODUCTION";
        let re = Regex::new(pattern).unwrap();

        assert!(re.is_match("CHAPTER 1"));
        assert!(re.is_match("Chapter 2"));
        assert!(re.is_match("INTRODUCTION"));
        assert!(!re.is_match("chapter 3")); // case-sensitive
        assert!(!re.is_match("the CHAPTER")); // ^ doesn't match mid-line

        // Preferred grouped alternation (shorter, less truncation risk)
        let pattern = "^(CHAPTER|Chapter|INTRODUCTION)";
        let re = Regex::new(pattern).unwrap();

        assert!(re.is_match("CHAPTER 1"));
        assert!(re.is_match("Chapter 2"));
        assert!(re.is_match("INTRODUCTION"));
        assert!(!re.is_match("chapter 3"));
        assert!(!re.is_match("the CHAPTER"));
    }

    #[test]
    fn test_regex_case_insensitive_pattern() {
        // Test case-insensitive search pattern
        let pattern = "(?i)^chapter";
        let re = Regex::new(pattern).unwrap();

        assert!(re.is_match("CHAPTER 1"));
        assert!(re.is_match("Chapter 2"));
        assert!(re.is_match("chapter 3"));
        assert!(!re.is_match("the chapter"));
    }

    #[test]
    fn test_regex_common_search_patterns() {
        // Validate that common search patterns compile and match correctly
        let test_cases = [
            ("fn main", "fn main() {", true),
            ("fn main", "fn main_() {", true), // substring match: "fn main" matches inside "fn main_"
            ("import.*react", "import React from 'react'", true),
            ("import.*react", "import { useState } from 'react'", true),
            ("TODO", "// TODO: fix this", true),
            ("TODO", "// todo: fix this", false),
            ("error.*handler", "error in handler: missing field", true),
            ("^pub async fn", "pub async fn search_files(", true),
            ("^pub async fn", "  pub async fn search_files(", false),
        ];

        for (pattern, text, should_match) in test_cases {
            let re = Regex::new(pattern).unwrap();
            assert_eq!(
                re.is_match(text),
                should_match,
                "Pattern '{}' vs text '{}': expected {}, got {}",
                pattern,
                text,
                should_match,
                !should_match
            );
        }
    }

    #[test]
    fn test_regex_invalid_pattern_returns_error() {
        // Verify that invalid regex patterns fail to compile
        let invalid_patterns = [
            "(",        // Unclosed paren
            "[",        // Unclosed bracket
            "*invalid", // Invalid quantifier
        ];

        for pattern in invalid_patterns {
            assert!(
                Regex::new(pattern).is_err(),
                "Pattern '{}' should be invalid but compiled successfully",
                pattern
            );
        }
    }

    #[test]
    fn test_glob_to_regex_patterns() {
        // Verify glob-to-regex conversion produces valid patterns
        let test_cases = [
            ("*.rs", "main.rs", true),
            ("*.rs", "main.rs.bak", false),
            ("*.rs", "README.md", false),
            ("test.{js,ts}", "test.js", true),
            ("test.{js,ts}", "test.ts", true),
            ("test.{js,ts}", "test.py", false),
            ("Makefile", "Makefile", true),
            ("*.min.js", "app.min.js", true),
        ];

        for (glob, filename, should_match) in test_cases {
            let regex_str = glob_to_regex(glob);
            let re = Regex::new(&regex_str).unwrap();
            assert_eq!(
                re.is_match(filename),
                should_match,
                "Glob '{}' converted to '{}' vs filename '{}': expected {}, got {}",
                glob,
                regex_str,
                filename,
                should_match,
                !should_match
            );
        }
    }
}
