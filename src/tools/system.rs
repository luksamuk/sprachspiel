//! System tools for context and environment information
//!
//! Provides tools for getting current date/time and project context.

use crate::debug_tools::{log_tool_call, log_tool_result};
use chrono::Datelike;
use sprachspiel_tool_derive::tool;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const MAX_DEPTH: usize = 3;

const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    "venv",
    ".venv",
    "env",
    ".env",
    "vendor",
    "cache",
    ".cache",
    "tmp",
    "temp",
    ".idea",
    ".vscode",
    "out",
    "bin",
    "obj",
    "Pods",
    "coverage",
    ".next",
    ".nuxt",
    "bower_components",
    "jspm_packages",
    ".serverless",
    ".fuse_box",
    ".dynamodb",
    "dist-server",
    "dist-client",
    "dist-build",
    "logs",
    "pkg",
    "site-packages",
    "eggs",
    ".eggs",
    "*.egg-info",
    "*.egg",
    "develop-eggs",
    "downloads",
    "lib",
    "lib64",
    "parts",
    "sdist",
    "var",
    "wheels",
    "htmlcov",
    ".tox",
    ".nox",
    ".hypothesis",
    ".pytest_cache",
    ".mypy_cache",
    ".dmypy.json",
    "dmypy.json",
    "cython_debug",
];

const IGNORED_FILES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    ".env.development",
    ".env.test",
    ".env.staging",
    "credentials",
    "secrets",
    "secrets.yaml",
    "secrets.yml",
    "secrets.json",
];

const LANGUAGE_EXTENSIONS: &[(&str, &str)] = &[
    ("rs", "Rust"),
    ("js", "JavaScript"),
    ("jsx", "JavaScript (JSX)"),
    ("ts", "TypeScript"),
    ("tsx", "TypeScript (JSX)"),
    ("mjs", "JavaScript (ESM)"),
    ("cjs", "JavaScript (CJS)"),
    ("py", "Python"),
    ("pyi", "Python (stub)"),
    ("go", "Go"),
    ("java", "Java"),
    ("kt", "Kotlin"),
    ("kts", "Kotlin Script"),
    ("c", "C"),
    ("h", "C/C++ Header"),
    ("cpp", "C++"),
    ("hpp", "C++ Header"),
    ("cc", "C++"),
    ("cxx", "C++"),
    ("cs", "C#"),
    ("rb", "Ruby"),
    ("php", "PHP"),
    ("swift", "Swift"),
    ("sh", "Shell"),
    ("bash", "Bash"),
    ("zsh", "Zsh"),
    ("json", "JSON"),
    ("yaml", "YAML"),
    ("yml", "YAML"),
    ("toml", "TOML"),
    ("xml", "XML"),
    ("sql", "SQL"),
    ("html", "HTML"),
    ("htm", "HTML"),
    ("css", "CSS"),
    ("scss", "SCSS"),
    ("sass", "Sass"),
    ("less", "Less"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("md", "Markdown"),
    ("rst", "reStructuredText"),
    ("org", "Org"),
    ("tex", "LaTeX"),
    ("bib", "BibTeX"),
    ("r", "R"),
    ("rmd", "R Markdown"),
    ("lua", "Lua"),
    ("jl", "Julia"),
    ("ex", "Elixir"),
    ("exs", "Elixir Script"),
    ("erl", "Erlang"),
    ("hrl", "Erlang Header"),
    ("hs", "Haskell"),
    ("lhs", "Literate Haskell"),
    ("scala", "Scala"),
    ("sc", "Scala"),
    ("clj", "Clojure"),
    ("cljs", "ClojureScript"),
    ("cljc", "Clojure CLR"),
    ("dart", "Dart"),
    ("scala", "Scala"),
    ("f90", "Fortran"),
    ("f95", "Fortran 95"),
    ("f03", "Fortran 2003"),
    ("asm", "Assembly"),
    ("s", "Assembly"),
    ("zig", "Zig"),
    ("nim", "Nim"),
    ("v", "V"),
    ("pl", "Perl"),
    ("pm", "Perl Module"),
    ("tcl", "Tcl"),
    ("ps1", "PowerShell"),
    ("psm1", "PowerShell Module"),
    ("dockerfile", "Dockerfile"),
    ("makefile", "Makefile"),
    ("cmake", "CMake"),
    ("gradle", "Gradle"),
];

const PROJECT_MARKERS: &[(&str, &str)] = &[
    ("Cargo.toml", "Rust (Cargo)"),
    ("Cargo.toml", "Rust"),
    ("package.json", "Node.js"),
    ("requirements.txt", "Python (pip)"),
    ("pyproject.toml", "Python (Poetry)"),
    ("setup.py", "Python (setuptools)"),
    ("go.mod", "Go"),
    ("pom.xml", "Java (Maven)"),
    ("build.gradle", "Java (Gradle)"),
    ("build.gradle.kts", "Kotlin/Java (Gradle)"),
    ("Gemfile", "Ruby (Bundler)"),
    ("composer.json", "PHP (Composer)"),
    ("Package.swift", "Swift"),
    ("CMakeLists.txt", "C/C++ (CMake)"),
    ("Makefile", "Make"),
    ("ducks", "Dune (OCaml)"),
    ("mix.exs", "Elixir"),
    ("build.sbt", "Scala (SBT)"),
    ("Project.toml", "Julia"),
    ("shard.yml", "Crystal"),
    ("zig.build", "Zig"),
    ("nimble", "Nim"),
    ("v.mod", "V"),
    ("pkg/Cargo.toml", "Rust Workspace"),
    ("pnpm-workspace.yaml", "Node.js (pnpm)"),
    ("lerna.json", "Node.js (Lerna)"),
    ("nx.json", "Node.js (Nx)"),
    ("turbo.json", "Node.js (Turborepo)"),
];

/// Get current date and time information.
///
/// Returns the current date, time, timezone, and related information.
/// Use this when you need to know the current time for any reason:
/// scheduling, deadlines, timestamps, or time-sensitive decisions.
///
/// # Arguments
/// None
///
/// # Returns
/// Current datetime information including:
/// - Date in human-readable format (e.g., "Monday, January 15, 2024")
/// - Time with timezone (e.g., "14:30:45 (UTC-3)")
/// - Day of week
/// - Week of year
/// - ISO 8601 format
/// - Unix timestamp
///
#[tool]
pub async fn get_current_datetime() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("get_current_datetime", &[]);

    let now = chrono::Local::now();

    let result = format!(
        "**Current Date & Time**\n\n\
         Date: {}\n\
         Time: {} ({})\n\
         Timezone: {}\n\
         Day of week: {}\n\
         Week of year: {}\n\
         ISO 8601: {}\n\
         Unix timestamp: {}",
        now.format("%A, %B %d, %Y"),
        now.format("%H:%M:%S"),
        now.offset(),
        now.offset(),
        now.format("%A"),
        now.iso_week().week(),
        now.to_rfc3339(),
        now.timestamp()
    );

    log_tool_result("get_current_datetime", &result);
    Ok(result)
}

/// Get current project context (git, languages, file structure).
///
/// Provides dynamic information about the current project state.
/// Use this to understand the project structure, git branch, and detected
/// technologies. This complements AGENTS.md with runtime information.
///
/// # Arguments
/// None
///
/// # Returns
/// Project context information including:
/// - Current working directory
/// - Git branch (if in a git repository)
/// - Detected programming languages and frameworks
/// - Key configuration files found
/// - Directory structure overview
///
/// # Notes
/// This tool does NOT replace AGENTS.md - it provides dynamic state.
/// Follow AGENTS.md for conventions and coding guidelines.
///
#[tool]
pub async fn get_project_context() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("get_project_context", &[]);

    let cwd =
        std::env::current_dir().map_err(|e| format!("Error getting current directory: {}", e))?;

    let mut output = vec!["📁 **Project Context**".to_string()];

    // Directory
    output.push(format!("\n\nDirectory: {}", cwd.display()));

    // Git info
    if let Some(git_info) = get_git_info(&cwd) {
        output.push(git_info);
    }

    // Languages (max depth 3)
    let languages = detect_languages(&cwd, MAX_DEPTH);
    if !languages.is_empty() {
        output.push("\n\n**Languages:**".to_string());
        let total: usize = languages.values().sum();
        let mut sorted: Vec<_> = languages.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (lang, count) in sorted {
            let percent = (*count as f64 / total as f64 * 100.0) as u32;
            output.push(format!("\n  {}: {} files ({}%)", lang, count, percent));
        }
    }

    // Stack detection
    let stack = detect_stack(&cwd);
    if !stack.is_empty() {
        output.push("\n\n**Stack detected:**".to_string());
        for s in stack {
            output.push(format!("\n  {}", s));
        }
    }

    // Key files
    let key_files = find_key_files(&cwd);
    if !key_files.is_empty() {
        output.push("\n\n**Key files:**".to_string());
        for f in key_files.iter().take(10) {
            output.push(format!("\n  {}", f));
        }
    }

    let result = output.join("");
    log_tool_result("get_project_context", &result);
    Ok(result)
}

fn get_git_info(dir: &Path) -> Option<String> {
    let git_dir = dir.join(".git");
    if !git_dir.exists() {
        return None;
    }

    let mut info = String::new();

    // Branch
    if let Some(branch) = get_git_branch_from_dir(dir) {
        info.push_str(&format!("\nGit Branch: {}", branch));
    }

    // Remote URL (without credentials)
    if let Some(remote) = get_git_remote(dir) {
        info.push_str(&format!("\nGit Remote: {}", remote));
    }

    // Modified files count
    if let Some(modified) = count_modified_files(dir) {
        if modified > 0 {
            info.push_str(&format!("\nGit Status: {} modified", modified));
        } else {
            info.push_str("\nGit Status: clean");
        }
    }

    Some(info)
}

fn get_git_branch_from_dir(dir: &Path) -> Option<String> {
    let git_head = dir.join(".git/HEAD");
    let content = fs::read_to_string(git_head).ok()?;
    let content = content.trim();

    if content.starts_with("ref: refs/heads/") {
        Some(content.strip_prefix("ref: refs/heads/")?.to_string())
    } else {
        let hash_len = content.len().min(7);
        Some(format!("detached@{}", &content[..hash_len]))
    }
}

fn get_git_remote(dir: &Path) -> Option<String> {
    let config = fs::read_to_string(dir.join(".git/config")).ok()?;

    for line in config.lines() {
        let line_trimmed = line.trim();
        if line_trimmed.starts_with("url = ") {
            let url = line_trimmed.strip_prefix("url = ")?;
            // Remove credentials from URL
            if url.contains('@') {
                // Format: user:pass@host or user@host
                let safe_url = url.split('@').next_back().unwrap_or(url);
                return Some(safe_url.to_string());
            }
            return Some(url.to_string());
        }
    }
    None
}

fn count_modified_files(dir: &Path) -> Option<usize> {
    let index_dir = dir.join(".git/index");
    if index_dir.exists() {
        // Count entries in git index (approximate)
        let metadata = fs::metadata(&index_dir).ok()?;
        // Rough estimate based on index size
        let count = (metadata.len() as usize).saturating_sub(12) / 62;
        Some(count.min(1000))
    } else {
        None
    }
}

fn detect_languages(dir: &Path, max_depth: usize) -> HashMap<String, usize> {
    let mut languages = HashMap::new();
    scan_dir_for_languages(dir, 0, max_depth, &mut languages);
    languages
}

fn scan_dir_for_languages(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    languages: &mut HashMap<String, usize>,
) {
    if depth > max_depth {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && (IGNORED_DIRS.contains(&name) || name.starts_with('.'))
            {
                continue;
            }
            scan_dir_for_languages(&path, depth + 1, max_depth, languages);
        } else if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && IGNORED_FILES.iter().any(|p| {
                    let pattern = p.trim_start_matches('*').trim_end_matches('*');
                    name.contains(pattern)
                })
            {
                continue;
            }

            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                for (pattern, lang) in LANGUAGE_EXTENSIONS {
                    if ext.eq_ignore_ascii_case(pattern) {
                        *languages.entry(lang.to_string()).or_insert(0) += 1;
                        break;
                    }
                }
            }
        }
    }
}

fn detect_stack(dir: &Path) -> Vec<String> {
    let mut stack = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (marker, name) in PROJECT_MARKERS {
        if dir.join(marker).exists() && !seen.contains(name) {
            stack.push(name.to_string());
            seen.insert(name);
        }
    }

    stack
}

fn find_key_files(dir: &Path) -> Vec<String> {
    let key_patterns = [
        "README",
        "readme",
        "Readme",
        "LICENSE",
        "license",
        "License",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
        "requirements.txt",
        "Makefile",
        "Dockerfile",
        "docker-compose",
        ".github",
        "docs",
    ];

    let mut files = Vec::new();

    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        for pattern in &key_patterns {
            if name_str.starts_with(pattern) || name_str.eq_ignore_ascii_case(pattern) {
                files.push(name_str.to_string());
                break;
            }
        }
    }

    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_git_branch_outside_repo() {
        let result = get_git_branch_from_dir(Path::new("/"));
        println!("Git branch outside repo: {:?}", result);
    }

    #[test]
    fn test_detect_languages_empty() {
        let languages = detect_languages(Path::new("/nonexistent"), 3);
        assert!(languages.is_empty());
    }

    #[test]
    fn test_detect_stack_empty() {
        let stack = detect_stack(Path::new("/nonexistent"));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_find_key_files_empty() {
        let files = find_key_files(Path::new("/nonexistent"));
        assert!(files.is_empty());
    }

    #[test]
    fn test_ignored_dirs_list() {
        assert!(IGNORED_DIRS.contains(&"node_modules"));
        assert!(IGNORED_DIRS.contains(&"target"));
        assert!(IGNORED_DIRS.contains(&".git"));
    }

    #[test]
    fn test_ignored_files_list() {
        assert!(IGNORED_FILES.contains(&".env"));
        assert!(IGNORED_FILES.contains(&"secrets"));
    }

    #[test]
    fn test_language_extensions_list() {
        for (ext, lang) in LANGUAGE_EXTENSIONS {
            assert!(!ext.is_empty());
            assert!(!lang.is_empty());
        }
    }

    #[test]
    fn test_project_markers_list() {
        for (marker, name) in PROJECT_MARKERS {
            assert!(!marker.is_empty());
            assert!(!name.is_empty());
        }
    }
}
