//! Logging initialization and verbosity configuration
//!
//! Provides a unified logging system using the `log` crate with `env_logger` backend.
//! Verbosity is controlled via:
//! - CLI flags: `-q` (quiet), `-v` (verbose), `-vv` (trace)
//! - Environment variable: `RUST_LOG=ask_ai=debug` (fine-grained control)
//! - Config file: `[output] verbosity = "verbose"` in config.toml
//!
//! # Verbosity Levels
//!
//! | Level   | Flag   | Log Level | Shows                                    |
//! |---------|--------|-----------|------------------------------------------|
//! | Quiet   | `-q`   | `error`   | Errors only (no spinner, no tool calls)  |
//! | Normal  | (def)  | `info`    | Tool calls (compact), warnings, errors  |
//! | Verbose | `-v`   | `debug`   | Detailed tool calls, results, internals  |
//! | Trace   | `-vv`  | `trace`   | Everything (embedding internals, tokens) |
//!
//! # Priority
//!
//! CLI flags > RUST_LOG env var > config.toml > default
//!
//! The default level is `info` (normal) so that tool calls are visible
//! in normal operation — users should see what tools the LLM is calling.
//!
//! # Rustyline Suppression
//!
//! Rustyline's internal debug output is always suppressed (filtered to `warn`)
//! regardless of the application's verbosity level. This prevents noisy readline
//! internals from cluttering the output even at trace level.
//!
//! # Known Limitation: Chat Mode and Verbose/Trace
//!
//! In interactive chat mode, the terminal is managed by rustyline which captures
//! the screen. `env_logger` output goes to stderr, which may not be visible inline
//! in the chat terminal. This means `-v`/`-vv` flags are primarily useful in
//! **query mode** (non-interactive). In chat mode, only tool call display
//! (via `eprintln!` with `suspend_for_print`) is reliably visible.
//!
//! The `/debug` command toggles the log level but trace/debug output from
//! `log::debug!()` / `log::trace!()` will appear on stderr, which may be
//! scrolled off or not visible depending on the terminal.
//!
//! # Future Note (TUI)
//!
//! When the TUI (ratatui.rs) is implemented, the chat REPL will be replaced.
//! At that point, debug/trace logging should be redirected to a file
//! (`~/.local/share/ask-ai/debug.log`) rather than stderr, keeping the TUI
//! output clean. The `/debug` command will toggle file logging instead of
//! stderr output.

use std::str::FromStr;

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
    /// Only errors — no spinner, no tool calls, no thinking
    Quiet,
    /// Tool calls (compact) + warnings + errors (default)
    #[default]
    Normal,
    /// Detailed tool calls + results + internal state
    Verbose,
    /// Everything including embedding internals, token budgets
    Trace,
}

impl Verbosity {
    /// Convert to `log::LevelFilter` for the `log` crate
    pub fn to_level_filter(self) -> log::LevelFilter {
        match self {
            Verbosity::Quiet => log::LevelFilter::Error,
            Verbosity::Normal => log::LevelFilter::Info,
            Verbosity::Verbose => log::LevelFilter::Debug,
            Verbosity::Trace => log::LevelFilter::Trace,
        }
    }

    /// Get the effective verbosity from CLI flags, RUST_LOG, and config
    ///
    /// Priority: explicit_cli > RUST_LOG > config > default
    pub fn resolve(
        quiet: bool,
        verbose_count: u8,
        config_verbosity: Option<Verbosity>,
    ) -> Verbosity {
        // CLI flags take highest priority
        if quiet {
            return Verbosity::Quiet;
        }
        match verbose_count {
            0 => {} // No CLI flag, check other sources
            1 => return Verbosity::Verbose,
            _ => return Verbosity::Trace,
        }

        // Check if RUST_LOG is set (user explicitly wants fine-grained control)
        if std::env::var("RUST_LOG").is_ok() {
            // RUST_LOG takes precedence over config
            // env_logger will handle it when we init with builder
            return Verbosity::Normal;
        }

        // Config file setting
        if let Some(v) = config_verbosity {
            return v;
        }

        // Default
        Verbosity::Normal
    }

    /// Check if RUST_LOG env var is set (for env_logger passthrough)
    pub fn has_rust_log_env() -> bool {
        std::env::var("RUST_LOG").is_ok()
    }
}

impl FromStr for Verbosity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quiet" | "q" | "error" => Ok(Verbosity::Quiet),
            "normal" | "n" | "info" => Ok(Verbosity::Normal),
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

/// Initialize the logging system with the given verbosity level.
///
/// This should be called once at program startup, before any `log::info!()` etc. calls.
/// If `RUST_LOG` is set, it takes precedence (env_logger handles this natively).
///
/// Rustyline's internal logging is always suppressed to `warn` level,
/// preventing noisy readline internals from cluttering output at debug/trace.
pub fn init(verbosity: Verbosity) {
    let mut builder = env_logger::Builder::new();

    // If RUST_LOG is set, let env_logger handle it (fine-grained control)
    if std::env::var("RUST_LOG").is_ok() {
        builder.parse_default_env();
    } else {
        // Set the level from our verbosity
        builder.filter_level(verbosity.to_level_filter());
    }

    // Always suppress rustyline's internal debug/trace output
    builder.filter_module("rustyline", log::LevelFilter::Warn);

    // Custom format: [LEVEL] message (without timestamp by default)
    // Timestamps are only shown at verbose/trace level
    let show_timestamp = matches!(verbosity, Verbosity::Verbose | Verbosity::Trace);
    if show_timestamp {
        builder.format_timestamp_secs();
    } else {
        builder.format_timestamp(None);
    }

    // Custom format that matches our existing debug output style
    builder.format(move |buf, record| {
        use std::io::Write;
        let level = match record.level() {
            log::Level::Error => "\x1B[31mERROR\x1B[0m", // Red
            log::Level::Warn => "\x1B[33mWARN\x1B[0m",   // Yellow
            log::Level::Info => "\x1B[36mINFO\x1B[0m",   // Cyan
            log::Level::Debug => "\x1B[90mDEBUG\x1B[0m", // Gray
            log::Level::Trace => "\x1B[35mTRACE\x1B[0m", // Magenta
        };

        if show_timestamp {
            writeln!(
                buf,
                "[{} {} {}] {}",
                buf.timestamp(),
                level,
                record.module_path().unwrap_or("ask-ai"),
                record.args()
            )
        } else {
            writeln!(
                buf,
                "[{} {}] {}",
                level,
                record.module_path().unwrap_or("ask-ai"),
                record.args()
            )
        }
    });

    // Initialize the logger
    if builder.try_init().is_err() {
        // Logger already initialized (e.g., in tests) — just set the level
        log::set_max_level(verbosity.to_level_filter());
    }
}

/// Re-initialize logging at a different level (e.g., when /debug is toggled)
pub fn set_verbosity(verbosity: Verbosity) {
    log::set_max_level(verbosity.to_level_filter());
}

/// Toggle verbosity between Normal and Trace.
/// Used by the /debug command in chat mode — full debug output when enabled.
pub fn toggle_verbosity() -> Verbosity {
    let current = log::max_level();
    let new_verbosity = if current >= log::LevelFilter::Trace {
        Verbosity::Normal
    } else {
        // When toggling debug ON via /debug, go to Trace for maximum information
        Verbosity::Trace
    };
    set_verbosity(new_verbosity);
    new_verbosity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verbosity_to_level_filter() {
        assert_eq!(Verbosity::Quiet.to_level_filter(), log::LevelFilter::Error);
        assert_eq!(Verbosity::Normal.to_level_filter(), log::LevelFilter::Info);
        assert_eq!(
            Verbosity::Verbose.to_level_filter(),
            log::LevelFilter::Debug
        );
        assert_eq!(Verbosity::Trace.to_level_filter(), log::LevelFilter::Trace);
    }

    #[test]
    fn test_verbosity_from_str() {
        assert_eq!(Verbosity::from_str("quiet").unwrap(), Verbosity::Quiet);
        assert_eq!(Verbosity::from_str("normal").unwrap(), Verbosity::Normal);
        assert_eq!(Verbosity::from_str("verbose").unwrap(), Verbosity::Verbose);
        assert_eq!(Verbosity::from_str("trace").unwrap(), Verbosity::Trace);
        // Aliases
        assert_eq!(Verbosity::from_str("q").unwrap(), Verbosity::Quiet);
        assert_eq!(Verbosity::from_str("v").unwrap(), Verbosity::Verbose);
        assert_eq!(Verbosity::from_str("info").unwrap(), Verbosity::Normal);
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
}
