//! Logging initialization with dual output (stderr + file) and data sensitivity policy.
//!
//! Provides a unified logging system using the `log` crate with a custom `MultiLogger`
//! that routes messages to both stderr (colored, level-filtered) and a log file
//! (`~/.local/share/sprachspiel/sprachspiel.log`, always warn+).
//!
//! # Verbosity Levels
//!
//! | Level   | Flag   | Terminal | File | Shows                                    |
//! |---------|--------|----------|------|------------------------------------------|
//! | Quiet   | `-q`   | `error`  | warn | Errors only (no spinner, no tool calls)  |
//! | Normal  | (def)  | `warn`   | warn | Warnings + errors                        |
//! | Verbose | `-v`   | `debug`  | warn | Detailed internals + warnings + errors   |
//! | Trace   | `-vv`  | `trace`  | info | Everything including embedding internals  |
//!
//! # Priority
//!
//! CLI flags > RUST_LOG env var > config.toml > default
//!
//! # Data Sensitivity Policy
//!
//! **NEVER log the following at any level:**
//! - API keys, tokens, or secrets (log only "key found" / "key missing")
//! - Raw user message content or LLM response text
//! - File paths that reveal personal information (use `<redacted>` for content)
//!
//! **Safe to log:**
//! - Counts, sizes, durations (`"Recovered {} embeddings"`)
//! - Status transitions (`"Service started"`, `"Compaction triggered"`)
//! - Error descriptions without user content (`"Failed to store embedding: {}"`)
//! - Metadata (`"Loaded {} facts"`, `"{} tokens estimated"`)
//!
//! When logging potentially sensitive data (e.g., fact content for dedup debugging),
//! truncate to 80 chars and append `"..."`. Never log full user text at debug or below.
//!
//! # File Logging
//!
//! Logs are written to `~/.local/share/sprachspiel/sprachspiel.log` by default.
//! Rotation: when the file exceeds 5 MB, it is renamed to `sprachspiel.log.1`
//! (previous backup deleted). The file always receives warn+ messages
//! regardless of terminal verbosity; trace verbose mode raises file level to info.
//!
//! # Rustyline Suppression
//!
//! Rustyline's internal debug output is always suppressed (filtered to `warn`)
//! regardless of the application's verbosity level.

#![expect(clippy::print_stderr)] // Logging setup output
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;

use chrono::Local;
use log::{Level, LevelFilter, Metadata, Record};

/// Maximum log file size before rotation (5 MB)
const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;
/// Maximum backup log files to keep
const MAX_BACKUPS: usize = 1;

// ---------------------------------------------------------------------------
// Verbosity enum
// ---------------------------------------------------------------------------

/// Verbosity levels for the application
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    /// Only errors
    Quiet,
    /// Warnings + errors (default)
    #[default]
    Normal,
    /// Detailed internals + warnings + errors
    Verbose,
    /// Everything including embedding internals, token budgets
    Trace,
}

impl Verbosity {
    /// Convert to terminal `log::LevelFilter`.
    pub fn to_level_filter(self) -> LevelFilter {
        match self {
            Verbosity::Quiet => LevelFilter::Error,
            Verbosity::Normal => LevelFilter::Warn,
            Verbosity::Verbose => LevelFilter::Debug,
            Verbosity::Trace => LevelFilter::Trace,
        }
    }

    /// Convert to file `log::LevelFilter`.
    /// File always gets at least `warn`; trace mode raises to `info`.
    pub fn to_file_level_filter(self) -> LevelFilter {
        match self {
            Verbosity::Quiet => LevelFilter::Warn,
            Verbosity::Normal => LevelFilter::Warn,
            Verbosity::Verbose => LevelFilter::Warn,
            Verbosity::Trace => LevelFilter::Info,
        }
    }

    /// Resolve effective verbosity from CLI flags, RUST_LOG, and config.
    ///
    /// Priority: explicit_cli > RUST_LOG > config > default
    pub fn resolve(
        quiet: bool,
        verbose_count: u8,
        config_verbosity: Option<Verbosity>,
    ) -> Verbosity {
        if quiet {
            return Verbosity::Quiet;
        }
        match verbose_count {
            0 => {}
            1 => return Verbosity::Verbose,
            _ => return Verbosity::Trace,
        }

        if std::env::var("RUST_LOG").is_ok() {
            return Verbosity::Normal;
        }

        if let Some(v) = config_verbosity {
            return v;
        }

        Verbosity::Normal
    }

    /// Check if RUST_LOG env var is set.
    pub fn has_rust_log_env() -> bool {
        std::env::var("RUST_LOG").is_ok()
    }
}

impl FromStr for Verbosity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quiet" | "q" | "error" => Ok(Verbosity::Quiet),
            "normal" | "n" | "warn" => Ok(Verbosity::Normal),
            "verbose" | "v" | "debug" => Ok(Verbosity::Verbose),
            "trace" | "t" => Ok(Verbosity::Trace),
            _ => Err(format!(
                "Unknown verbosity '{}'. Use: quiet, normal, verbose, trace",
                s
            )),
        }
    }
}

impl std::fmt::Display for Verbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verbosity::Quiet => write!(f, "quiet"),
            Verbosity::Normal => write!(f, "normal"),
            Verbosity::Verbose => write!(f, "verbose"),
            Verbosity::Trace => write!(f, "trace"),
        }
    }
}

// ---------------------------------------------------------------------------
// Colored stderr logger
// ---------------------------------------------------------------------------

struct StderrLogger {
    level: LevelFilter,
}

impl StderrLogger {
    fn new(level: LevelFilter) -> Self {
        Self { level }
    }

    fn colored_level(level: Level) -> &'static str {
        match level {
            Level::Error => "\x1B[31mERROR\x1B[0m",
            Level::Warn => "\x1B[33mWARN\x1B[0m",
            Level::Info => "\x1B[36mINFO\x1B[0m",
            Level::Debug => "\x1B[90mDEBUG\x1B[0m",
            Level::Trace => "\x1B[35mTRACE\x1B[0m",
        }
    }
}

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Suppress TUI internals (crossterm, ratatui, rustyline-derivative)
        if record.target().starts_with("crossterm")
            || record.target().starts_with("ratatui")
            || record.target().starts_with("rustyline")
        {
            return;
        }
        let level = Self::colored_level(record.level());
        eprintln!(
            "[{} {}] {}",
            level,
            record.module_path().unwrap_or("sprachspiel"),
            record.args()
        );
    }

    fn flush(&self) {
        // stderr flushes automatically
    }
}

// ---------------------------------------------------------------------------
// File logger with rotation
// ---------------------------------------------------------------------------

struct FileLogger {
    level: LevelFilter,
    file: Mutex<File>,
}

impl FileLogger {
    fn new(path: PathBuf, level: LevelFilter) -> Self {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Rotate if needed
        Self::rotate_if_needed(&path);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|e| {
                eprintln!("Failed to open log file {}: {}", path.display(), e);
                // Fallback to /dev/null equivalent — we just silently skip
                #[expect(clippy::unwrap_used)] // /dev/null always exists on Unix
                OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .unwrap_or_else(|_| File::open("/dev/null").unwrap())
            });

        Self {
            level,
            file: Mutex::new(file),
        }
    }

    fn rotate_if_needed(path: &PathBuf) {
        let Ok(metadata) = fs::metadata(path) else {
            return;
        };
        if metadata.len() < MAX_LOG_SIZE {
            return;
        }

        // Rotate: sprachspiel.log → sprachspiel.log.1 (delete old .1 first)
        for i in (1..=MAX_BACKUPS).rev() {
            let backup = PathBuf::from(format!("{}.{}", path.display(), i));
            if backup.exists() {
                let _ = fs::remove_file(&backup);
            }
        }

        let backup = PathBuf::from(format!("{}.1", path.display()));
        let _ = fs::rename(path, &backup);
    }

    /// Get the default log file path.
    fn default_path() -> PathBuf {
        use crate::consts::app;

        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(app::APP_DATA_DIR)
            .join(app::LOG_FILENAME)
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Suppress TUI internals (crossterm, ratatui, rustyline-derivative)
        if record.target().starts_with("crossterm")
            || record.target().starts_with("ratatui")
            || record.target().starts_with("rustyline")
        {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!(
            "[{} {} {}] {}\n",
            timestamp,
            record.level(),
            record.module_path().unwrap_or("sprachspiel"),
            record.args()
        );

        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

// ---------------------------------------------------------------------------
// MultiLogger — combines stderr + file
// ---------------------------------------------------------------------------

struct MultiLogger {
    stderr: StderrLogger,
    file: Option<FileLogger>,
}

impl MultiLogger {
    fn new(stderr_level: LevelFilter, file_level: LevelFilter, log_path: Option<PathBuf>) -> Self {
        let path = log_path.unwrap_or_else(FileLogger::default_path);
        let file_logger = FileLogger::new(path, file_level);
        Self {
            stderr: StderrLogger::new(stderr_level),
            file: Some(file_logger),
        }
    }
}

impl log::Log for MultiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.stderr.enabled(metadata) || self.file.as_ref().is_some_and(|f| f.enabled(metadata))
    }

    fn log(&self, record: &Record) {
        self.stderr.log(record);
        if let Some(ref file) = self.file {
            file.log(record);
        }
    }

    fn flush(&self) {
        self.stderr.flush();
        if let Some(ref file) = self.file {
            file.flush();
        }
    }
}

// ---------------------------------------------------------------------------
// Public init API
// ---------------------------------------------------------------------------

/// Initialize the logging system with the given verbosity level.
///
/// Sets up a `MultiLogger` that writes to both stderr (colored) and a log file.
/// If `RUST_LOG` is set, it overrides the terminal level.
/// Rustyline's internal logging is always suppressed to `warn` level.
///
/// This should be called once at program startup, before any `log::info!()` etc. calls.
pub fn init(verbosity: Verbosity) {
    init_with_path(verbosity, None);
}

/// Initialize logging with a custom log file path (useful for tests).
pub fn init_with_path(verbosity: Verbosity, log_path: Option<PathBuf>) {
    let term_level = if std::env::var("RUST_LOG").is_ok() {
        // Let RUST_LOG control terminal output level
        LevelFilter::Trace
    } else {
        verbosity.to_level_filter()
    };

    let file_level = verbosity.to_file_level_filter();

    // Box::leak to get a &'static reference — the logger lives for the program lifetime
    let logger: &'static MultiLogger =
        Box::leak(Box::new(MultiLogger::new(term_level, file_level, log_path)));

    if log::set_logger(logger).is_err() {
        // Logger already initialized (e.g., in tests) — just set the level
    }
    // Set max level to the most permissive of the two
    let max = std::cmp::max(term_level, file_level);
    log::set_max_level(max);
}

/// Re-initialize logging at a different level (e.g., when /debug is toggled)
pub fn set_verbosity(verbosity: Verbosity) {
    let term_level = verbosity.to_level_filter();
    let file_level = verbosity.to_file_level_filter();
    log::set_max_level(std::cmp::max(term_level, file_level));
}

/// Toggle verbosity between Normal and Trace.
/// Used by the /debug command in chat mode — full debug output when enabled.
pub fn toggle_verbosity() -> Verbosity {
    let current = log::max_level();
    let new_verbosity = if current >= LevelFilter::Trace {
        Verbosity::Normal
    } else {
        Verbosity::Trace
    };
    set_verbosity(new_verbosity);
    new_verbosity
}

/// Truncate a string for safe logging (data sensitivity policy).
///
/// Returns the first `max_len` characters with `"..."` appended if truncated.
/// Use this when logging potentially sensitive content like fact text or
/// user messages.
///
/// # Examples
/// ```ignore
/// log::debug!("Fact content: {}", truncate_for_log(&content, 80));
/// ```
pub fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find a safe char boundary near max_len
        let boundary = s
            .char_indices()
            .take_while(|(i, _)| *i < max_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_len.min(s.len()));
        format!("{}...", &s[..boundary])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verbosity_to_level_filter() {
        assert_eq!(Verbosity::Quiet.to_level_filter(), LevelFilter::Error);
        assert_eq!(Verbosity::Normal.to_level_filter(), LevelFilter::Warn);
        assert_eq!(Verbosity::Verbose.to_level_filter(), LevelFilter::Debug);
        assert_eq!(Verbosity::Trace.to_level_filter(), LevelFilter::Trace);
    }

    #[test]
    fn test_verbosity_to_file_level_filter() {
        assert_eq!(Verbosity::Quiet.to_file_level_filter(), LevelFilter::Warn);
        assert_eq!(Verbosity::Normal.to_file_level_filter(), LevelFilter::Warn);
        assert_eq!(Verbosity::Verbose.to_file_level_filter(), LevelFilter::Warn);
        assert_eq!(Verbosity::Trace.to_file_level_filter(), LevelFilter::Info);
    }

    #[test]
    fn test_verbosity_from_str() {
        assert_eq!(Verbosity::from_str("quiet").unwrap(), Verbosity::Quiet);
        assert_eq!(Verbosity::from_str("normal").unwrap(), Verbosity::Normal);
        assert_eq!(Verbosity::from_str("verbose").unwrap(), Verbosity::Verbose);
        assert_eq!(Verbosity::from_str("trace").unwrap(), Verbosity::Trace);
        // Aliases — note: "info" removed since Normal now = warn
        assert_eq!(Verbosity::from_str("q").unwrap(), Verbosity::Quiet);
        assert_eq!(Verbosity::from_str("v").unwrap(), Verbosity::Verbose);
        assert_eq!(Verbosity::from_str("warn").unwrap(), Verbosity::Normal);
        assert!(Verbosity::from_str("invalid").is_err());
    }

    #[test]
    fn test_verbosity_resolve_cli_flags() {
        assert_eq!(Verbosity::resolve(true, 0, None), Verbosity::Quiet);
        assert_eq!(Verbosity::resolve(false, 0, None), Verbosity::Normal);
        assert_eq!(Verbosity::resolve(false, 1, None), Verbosity::Verbose);
        assert_eq!(Verbosity::resolve(false, 2, None), Verbosity::Trace);
        assert_eq!(Verbosity::resolve(false, 3, None), Verbosity::Trace);
    }

    #[test]
    fn test_verbosity_resolve_config() {
        assert_eq!(
            Verbosity::resolve(false, 0, Some(Verbosity::Verbose)),
            Verbosity::Verbose
        );
        assert_eq!(
            Verbosity::resolve(false, 0, Some(Verbosity::Trace)),
            Verbosity::Trace
        );
    }

    #[test]
    fn test_verbosity_display() {
        assert_eq!(format!("{}", Verbosity::Quiet), "quiet");
        assert_eq!(format!("{}", Verbosity::Normal), "normal");
        assert_eq!(format!("{}", Verbosity::Verbose), "verbose");
        assert_eq!(format!("{}", Verbosity::Trace), "trace");
    }

    #[test]
    fn test_verbosity_ordering() {
        assert!(Verbosity::Quiet < Verbosity::Normal);
        assert!(Verbosity::Normal < Verbosity::Verbose);
        assert!(Verbosity::Verbose < Verbosity::Trace);
    }

    #[test]
    fn test_truncate_for_log() {
        assert_eq!(truncate_for_log("hello", 80), "hello");
        assert_eq!(truncate_for_log("hello", 3), "hel...");
        // UTF-8 safe boundary
        assert_eq!(truncate_for_log("café", 3), "caf...");
        // Short string
        assert_eq!(truncate_for_log("ab", 5), "ab");
    }
}
