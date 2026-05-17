//! Logging initialization with dual output (stderr + file) and data sensitivity policy.
//!
//! Provides a unified logging system using the `log` crate with a custom `MultiLogger`
//! that routes messages to both stderr (colored, level-filtered) and a log file
//! (`~/.local/share/sprachspiel/sprachspiel.log`).
//!
//! # Verbosity Levels
//!
//! | Level   | Flag   | Terminal | File  | Shows                                    |
//! |---------|--------|----------|-------|------------------------------------------|
//! | Quiet   | `-q`   | `error`  | warn  | Errors only (no spinner, no tool calls)  |
//! | Normal  | (def)  | `warn`   | info  | Warnings + errors                        |
//! | Verbose | `-v`   | `debug`  | debug | Detailed internals + warnings + errors   |
//! | Trace   | `-vv`  | `trace`  | trace | Everything including embedding internals  |
//!
//! # TUI Mode
//!
//! When the TUI (ratatui alternate screen) is active, stderr output is **suppressed**
//! entirely to prevent corrupting the display. All logging goes to the file instead.
//! File logging is boosted to at least `debug` in TUI mode so that the log file
//! captures useful diagnostic information that would otherwise appear on stderr.
//!
//! Use `/debug` in TUI mode to toggle between debug and trace file logging.
//! Use `--verbose` / `-v` at launch to start with debug-level file logging.
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
//! (previous backup deleted). The file level varies by verbosity:
//! - Normal: info+ (warnings, errors, and informational messages)
//! - Verbose/-v: debug+ (detailed internals)
//! - Trace/-vv: trace (everything)
//!
//! In TUI mode, the file level is boosted to at least debug regardless of verbosity,
//! since stderr is suppressed and the file is the only way to capture diagnostics.
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
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use chrono::Local;
use log::{Level, LevelFilter, Metadata, Record};

// ---------------------------------------------------------------------------
// TUI mode and dynamic file level overrides
// ---------------------------------------------------------------------------

/// When true, the TUI alternate screen is active and all stderr output is
/// suppressed to prevent corrupting the display.
///
/// Set via [`set_tui_mode()`] when entering/exiting TUI mode.
static TUI_MODE: AtomicBool = AtomicBool::new(false);

/// Sentinel value for `FILE_LEVEL_OVERRIDE` meaning "use construction default".
const USE_DEFAULT_FILE_LEVEL: u8 = 255;

/// Dynamic override for the file logger level.
///
/// When set to a value other than `USE_DEFAULT_FILE_LEVEL`, this takes
/// precedence over the construction-time `FileLogger.level`. Used to boost
/// file logging in TUI mode (where stderr is suppressed) and when `/debug`
/// is toggled.
///
/// Stored as u8: 0=Off, 1=Error, 2=Warn, 3=Info, 4=Debug, 5=Trace.
static FILE_LEVEL_OVERRIDE: AtomicU8 = AtomicU8::new(USE_DEFAULT_FILE_LEVEL);

/// Convert a `LevelFilter` to a u8 for storage in `FILE_LEVEL_OVERRIDE`.
fn level_filter_to_u8(level: LevelFilter) -> u8 {
    match level {
        LevelFilter::Off => 0,
        LevelFilter::Error => 1,
        LevelFilter::Warn => 2,
        LevelFilter::Info => 3,
        LevelFilter::Debug => 4,
        LevelFilter::Trace => 5,
    }
}

/// Convert a u8 from `FILE_LEVEL_OVERRIDE` back to a `LevelFilter`.
/// Returns `None` for invalid values (including the sentinel).
fn level_filter_from_u8(v: u8) -> Option<LevelFilter> {
    match v {
        0 => Some(LevelFilter::Off),
        1 => Some(LevelFilter::Error),
        2 => Some(LevelFilter::Warn),
        3 => Some(LevelFilter::Info),
        4 => Some(LevelFilter::Debug),
        5 => Some(LevelFilter::Trace),
        _ => None,
    }
}

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
    ///
    /// The file logger is more verbose than the terminal so the log file
    /// captures useful diagnostic information even at normal verbosity:
    /// - Quiet: `warn` (errors + warnings only)
    /// - Normal: `info` (adds informational messages)
    /// - Verbose: `debug` (adds debug internals)
    /// - Trace: `trace` (everything)
    ///
    /// In TUI mode, the file level is boosted to at least `debug` via
    /// [`set_tui_mode()`] since stderr is suppressed.
    pub fn to_file_level_filter(self) -> LevelFilter {
        match self {
            Verbosity::Quiet => LevelFilter::Warn,
            Verbosity::Normal => LevelFilter::Info,
            Verbosity::Verbose => LevelFilter::Debug,
            Verbosity::Trace => LevelFilter::Trace,
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
        // In TUI mode, stderr is completely suppressed
        if TUI_MODE.load(Ordering::Relaxed) {
            return false;
        }
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        // Suppress ALL stderr output in TUI mode to avoid corrupting alternate screen
        if TUI_MODE.load(Ordering::Relaxed) {
            return;
        }
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

    /// Effective level: uses `FILE_LEVEL_OVERRIDE` if set, otherwise construction default.
    fn effective_level(&self) -> LevelFilter {
        let override_val = FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed);
        if override_val != USE_DEFAULT_FILE_LEVEL {
            level_filter_from_u8(override_val).unwrap_or(self.level)
        } else {
            self.level
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
        metadata.level() <= self.effective_level()
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
///
/// In TUI mode, also adjusts the file logger level:
/// - Trace → file level Trace (captures everything)
/// - Normal → file level Debug (TUI default, captures useful diagnostics)
pub fn toggle_verbosity() -> Verbosity {
    let current = log::max_level();
    let new_verbosity = if current >= LevelFilter::Trace {
        Verbosity::Normal
    } else {
        Verbosity::Trace
    };
    set_verbosity(new_verbosity);

    // In TUI mode, adjust file level to match the new verbosity
    if is_tui_mode() {
        let file_level = match new_verbosity {
            Verbosity::Trace => LevelFilter::Trace,
            _ => LevelFilter::Debug, // TUI default is debug
        };
        set_file_level(file_level);
    }

    new_verbosity
}

// ---------------------------------------------------------------------------
// TUI mode and dynamic file level control
// ---------------------------------------------------------------------------

/// Activate or deactivate TUI mode for logging.
///
/// When TUI mode is activated:
/// - Stderr output is completely suppressed (prevents alternate screen corruption)
/// - File logging is boosted to `debug` level (so the log file captures useful
///   diagnostics that would otherwise appear on stderr)
///
/// When TUI mode is deactivated:
/// - Stderr output is restored
/// - File logging reverts to the construction-default level
///
/// Called from `RatatuiView::new()` and `RatatuiView::restore()`.
pub fn set_tui_mode(active: bool) {
    TUI_MODE.store(active, Ordering::Relaxed);
    if active {
        // Boost file logging to debug so TUI debugging is useful.
        // This also ensures the global max level allows debug+ messages through.
        set_file_level(LevelFilter::Debug);
    } else {
        // Restore file logging to construction default
        clear_file_level();
    }
}

/// Check if TUI mode is currently active.
///
/// Used internally by loggers to decide whether to suppress stderr output.
/// Can also be called from other modules that need to know if they should
/// avoid writing to stderr.
pub fn is_tui_mode() -> bool {
    TUI_MODE.load(Ordering::Relaxed)
}

/// Set a dynamic override for the file logger level.
///
/// This takes precedence over the construction-time level set during `init()`.
/// Also ensures the global `log::set_max_level()` is raised if needed so that
/// messages at the requested level actually reach the logger.
pub fn set_file_level(level: LevelFilter) {
    FILE_LEVEL_OVERRIDE.store(level_filter_to_u8(level), Ordering::Relaxed);
    // Ensure global max level allows messages through to the file logger
    let current_max = log::max_level();
    if level > current_max {
        log::set_max_level(level);
    }
}

/// Clear the dynamic file level override, reverting to the construction default.
pub fn clear_file_level() {
    FILE_LEVEL_OVERRIDE.store(USE_DEFAULT_FILE_LEVEL, Ordering::Relaxed);
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
        assert_eq!(Verbosity::Normal.to_file_level_filter(), LevelFilter::Info);
        assert_eq!(
            Verbosity::Verbose.to_file_level_filter(),
            LevelFilter::Debug
        );
        assert_eq!(Verbosity::Trace.to_file_level_filter(), LevelFilter::Trace);
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

    // -----------------------------------------------------------------------
    // TUI mode and file level override tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_level_filter_roundtrip() {
        // u8 ↔ LevelFilter round-trip integrity
        for level in [
            LevelFilter::Off,
            LevelFilter::Error,
            LevelFilter::Warn,
            LevelFilter::Info,
            LevelFilter::Debug,
            LevelFilter::Trace,
        ] {
            let encoded = level_filter_to_u8(level);
            let decoded = level_filter_from_u8(encoded);
            assert_eq!(decoded, Some(level), "Round-trip failed for {:?}", level);
        }
    }

    #[test]
    fn test_level_filter_from_u8_rejects_invalid() {
        assert_eq!(level_filter_from_u8(USE_DEFAULT_FILE_LEVEL), None);
        assert_eq!(level_filter_from_u8(100), None);
        assert_eq!(level_filter_from_u8(6), None);
    }

    #[test]
    fn test_tui_mode_default_off() {
        // TUI mode starts off
        assert!(!is_tui_mode());
    }

    #[test]
    fn test_tui_mode_toggle() {
        // TUI mode can be enabled and disabled
        set_tui_mode(true);
        assert!(is_tui_mode());
        set_tui_mode(false);
        assert!(!is_tui_mode());
    }

    #[test]
    fn test_set_and_clear_file_level() {
        // File level override starts at default sentinel
        let initial = FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed);
        assert_eq!(initial, USE_DEFAULT_FILE_LEVEL);

        // Set to debug
        set_file_level(LevelFilter::Debug);
        assert_eq!(
            FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed),
            level_filter_to_u8(LevelFilter::Debug)
        );

        // Clear restores default
        clear_file_level();
        assert_eq!(
            FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed),
            USE_DEFAULT_FILE_LEVEL
        );
    }

    #[test]
    fn test_file_logger_effective_level_default() {
        let file_logger = FileLogger::new(PathBuf::from("/dev/null"), LevelFilter::Warn);
        // No override → should return construction default
        clear_file_level();
        assert_eq!(file_logger.effective_level(), LevelFilter::Warn);
    }

    #[test]
    fn test_file_logger_effective_level_override() {
        let file_logger = FileLogger::new(PathBuf::from("/dev/null"), LevelFilter::Warn);
        // Set override to debug
        set_file_level(LevelFilter::Debug);
        assert_eq!(file_logger.effective_level(), LevelFilter::Debug);

        // Clear override → back to construction default
        clear_file_level();
        assert_eq!(file_logger.effective_level(), LevelFilter::Warn);
    }

    #[test]
    fn test_stderr_logger_suppressed_in_tui_mode() {
        // TUI mode starts off — stderr should not be suppressed
        set_tui_mode(false);
        assert!(!is_tui_mode());

        // Enable TUI mode — stderr is suppressed via the TUI_MODE atomic
        set_tui_mode(true);
        assert!(is_tui_mode());

        // Disable TUI mode — back to normal
        set_tui_mode(false);
        assert!(!is_tui_mode());
    }

    #[test]
    fn test_tui_mode_sets_file_level_to_debug() {
        clear_file_level();
        set_tui_mode(true);
        // File level should be boosted to debug
        assert_eq!(
            FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed),
            level_filter_to_u8(LevelFilter::Debug)
        );
        set_tui_mode(false);
    }

    #[test]
    fn test_tui_mode_clear_restores_file_level() {
        set_file_level(LevelFilter::Debug);
        set_tui_mode(false);
        // Clearing TUI mode should restore default
        assert_eq!(
            FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed),
            USE_DEFAULT_FILE_LEVEL
        );
    }

    #[test]
    fn test_tui_mode_toggle_verbosity_trace() {
        // Simulate TUI mode + /debug toggle
        set_tui_mode(true);

        // The toggle_verbosity function goes from current max level to Trace
        // Since we cleared the level at start of test, simulate toggle
        set_file_level(LevelFilter::Trace);
        assert_eq!(
            FILE_LEVEL_OVERRIDE.load(Ordering::Relaxed),
            level_filter_to_u8(LevelFilter::Trace)
        );

        // Clean up
        set_tui_mode(false);
    }
}
