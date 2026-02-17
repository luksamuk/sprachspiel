use crate::debug_tools::{log_tool_call, log_tool_result};
use regex::Regex;
use std::path::{Path, PathBuf};

const MAX_FILE_SIZE: usize = 1_000_000; // 1MB max file size
const MAX_RESULTS: usize = 100; // Maximum search results

/// Read the contents of a file
#[ollama_rs::function]
pub async fn read_file(
    path: String,
    max_lines: Option<u32>,
    sandbox: Option<bool>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "read_file",
        &[
            ("path".to_string(), path.clone()),
            (
                "max_lines".to_string(),
                max_lines.map(|l| l.to_string()).unwrap_or_else(|| "all".to_string()),
            ),
        ],
    );

    // Validate and canonicalize path
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox.unwrap_or(true)) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Error: {}", e);
            log_tool_result("read_file", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check if file exists and is readable
    if !canonical_path.exists() {
        let err_msg = format!("Error: File not found: {}. Please check if the file exists or try a different file name (e.g., README.org instead of README.md).", path);
        log_tool_result("read_file", &err_msg);
        return Ok(err_msg);
    }

    if !canonical_path.is_file() {
        let err_msg = format!("Error: Path is not a file: {}", path);
        log_tool_result("read_file", &err_msg);
        return Ok(err_msg);
    }

    // Check file size
    let metadata = std::fs::metadata(&canonical_path)?;
    if metadata.len() > MAX_FILE_SIZE as u64 {
        let err_msg = format!(
            "Error: File too large ({} bytes, max: {} bytes): {}",
            metadata.len(),
            MAX_FILE_SIZE,
            path
        );
        log_tool_result("read_file", &err_msg);
        return Ok(err_msg);
    }

    // Read file content
    let content = std::fs::read_to_string(&canonical_path)?;

    // Apply max_lines limit if specified
    let result = if let Some(lines) = max_lines {
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
#[ollama_rs::function]
pub async fn read_file_segment(
    path: String,
    start_line: u32,
    num_lines: u32,
    sandbox: Option<bool>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "read_file_segment",
        &[
            ("path".to_string(), path.clone()),
            ("start_line".to_string(), start_line.to_string()),
            ("num_lines".to_string(), num_lines.to_string()),
        ],
    );

    // Validate and canonicalize path
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox.unwrap_or(true)) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Error: {}", e);
            log_tool_result("read_file_segment", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check if file exists and is readable
    if !canonical_path.exists() {
        let err_msg = format!("Error: File not found: {}. Please check if the file exists.", path);
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    if !canonical_path.is_file() {
        let err_msg = format!("Error: Path is not a file: {}", path);
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    // Check file size
    let metadata = std::fs::metadata(&canonical_path)?;
    if metadata.len() > MAX_FILE_SIZE as u64 {
        let err_msg = format!(
            "Error: File too large ({} bytes, max: {} bytes): {}",
            metadata.len(),
            MAX_FILE_SIZE,
            path
        );
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    // Read file content
    let content = std::fs::read_to_string(&canonical_path)?;

    // Extract segment
    let start = start_line as usize;
    let count = num_lines as usize;
    
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if start == 0 || start > total_lines {
        let err_msg = format!(
            "Error: Invalid start_line {}. File has {} lines. Line numbers start at 1.",
            start_line, total_lines
        );
        log_tool_result("read_file_segment", &err_msg);
        return Ok(err_msg);
    }

    let start_idx = start - 1; // Convert to 0-based index
    let end_idx = std::cmp::min(start_idx + count, total_lines);
    let segment_lines: Vec<&str> = lines[start_idx..end_idx].to_vec();

    let result = if segment_lines.is_empty() {
        format!("File has {} lines. No lines to read from line {}.", total_lines, start_line)
    } else {
        let mut output = Vec::new();
        let end_line = start_line as usize + segment_lines.len() - 1;
        output.push(format!("Lines {}-{} of {}:", start_line, end_line, total_lines));
        output.push("-".repeat(40));
        for (i, line) in segment_lines.iter().enumerate() {
            output.push(format!("{:>6} | {}", start as usize + i, line));
        }
        output.join("\n")
    };

    log_tool_result("read_file_segment", &result);
    Ok(result)
}

/// List contents of a directory
#[ollama_rs::function]
pub async fn list_directory(
    path: String,
    recursive: Option<bool>,
    sandbox: Option<bool>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "list_directory",
        &[
            ("path".to_string(), path.clone()),
            (
                "recursive".to_string(),
                recursive.map(|r| r.to_string()).unwrap_or_default(),
            ),
        ],
    );

    // Validate and canonicalize path
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox.unwrap_or(true)) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Error: {}", e);
            log_tool_result("list_directory", &err_msg);
            return Ok(err_msg);
        }
    };

    // Check if directory exists
    if !canonical_path.exists() {
        let err_msg = format!("Error: Directory not found: {}", path);
        log_tool_result("list_directory", &err_msg);
        return Ok(err_msg);
    }

    if !canonical_path.is_dir() {
        let err_msg = format!("Error: Path is not a directory: {}", path);
        log_tool_result("list_directory", &err_msg);
        return Ok(err_msg);
    }

    // List directory contents
    let mut entries = Vec::new();

    if recursive.unwrap_or(false) {
        collect_entries_recursive(&canonical_path, &canonical_path, &mut entries, 0, 10)?;
    } else {
        for entry in std::fs::read_dir(&canonical_path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
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
                format!(" ({} bytes)", metadata.len())
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

    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let _name = entry.file_name().to_string_lossy().to_string();

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
            format!(" ({} bytes)", metadata.len())
        } else {
            String::new()
        };

        entries.push(format!("{}[{}] {}{}", indent, entry_type, display_path, size_info));

        // Recurse into subdirectories
        if metadata.is_dir() {
            collect_entries_recursive(base_path, &full_path, entries, depth + 1, max_depth)?;
        }
    }

    Ok(())
}

/// Search for a pattern in files
#[ollama_rs::function]
pub async fn search_files(
    pattern: String,
    path: String,
    file_pattern: Option<String>,
    sandbox: Option<bool>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
            let err_msg = format!("Error: Invalid regex pattern '{}': {}. Please check your regex syntax.", pattern, e);
            log_tool_result("search_files", &err_msg);
            return Ok(err_msg);
        }
    };

    // Validate path
    let path_buf = PathBuf::from(&path);
    let canonical_path = match validate_path(&path_buf, sandbox.unwrap_or(true)) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Error: {}", e);
            log_tool_result("search_files", &err_msg);
            return Ok(err_msg);
        }
    };

    if !canonical_path.exists() {
        let err_msg = format!("Error: Path not found: {}", path);
        log_tool_result("search_files", &err_msg);
        return Ok(err_msg);
    }

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
                        "... (stopped after {} matches)",
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

    for entry in walkdir::WalkDir::new(dir).max_depth(5).into_iter().flatten() {
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
        std::env::current_dir()?.join(path)
    };

    // Canonicalize to resolve symlinks and normalize
    let canonical_path = abs_path
        .canonicalize()
        .map_err(|_| format!("Invalid path: {}", path.display()))?;

    if sandbox {
        // Get current working directory
        let cwd = std::env::current_dir()?;
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
        if result.is_err() {
            let err_msg = result.unwrap_err().to_string();
            assert!(!err_msg.contains("sandboxed"));
        }
    }
}
