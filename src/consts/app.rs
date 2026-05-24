//! Application name and file constants
//!
//! Single source of truth for the application name, database filename,
//! and directory paths. All code that references the app name or data
//! locations MUST use these constants — never hardcode "sprach" or
//! "sprachspiel" or path components directly.
//!
//! Naming convention:
//! - `APP_NAME` = binary/cli command name (short, for typing)
//! - `APP_CONFIG_DIR` / `APP_DATA_DIR` / `APP_PROJECT_DIR` = directory names

/// Application binary name (the command users type)
/// Must match Cargo.toml `[[bin]] name`.
pub const APP_NAME: &str = "sprach";

/// Log filename
pub const LOG_FILENAME: &str = "sprachspiel.log";

/// Database filename
pub const DB_FILENAME: &str = "sprachspiel.db";

/// Legacy database filename (v0.42 and earlier)
/// Used by `migrate_legacy_db()` to auto-rename on first run.
pub const DB_FILENAME_LEGACY_V2: &str = "ask-ai.db";

/// Original database filename (v0.27 and earlier)
/// Used by `migrate_legacy_db()` to auto-rename on first run.
pub const DB_FILENAME_LEGACY_V1: &str = "embeddings.db";

/// XDG config directory name (appended to ~/.config/ or $XDG_CONFIG_HOME)
pub const APP_CONFIG_DIR: &str = "sprachspiel";

/// XDG data directory name (appended to ~/.local/share/ or $XDG_DATA_HOME)
pub const APP_DATA_DIR: &str = "sprachspiel";

/// Project-level directory name (e.g., .sprachspiel/skills/)
pub const APP_PROJECT_DIR: &str = ".sprachspiel";

// ── Provider-agnostic error messages ──────────────────────────
// These replace "Ollama" with "LLM server" or "embedding service"
// for multi-backend readiness. DO NOT use "Ollama" in these strings.

/// Error prefix for LLM connection failures.
/// Used in error messages when the backend server is unreachable.
pub const ERR_LLM_CONNECTION: &str =
    "Could not connect to the LLM server. Make sure it is running.";

/// Error message when the LLM server is not running.
/// Used in vision/OCR error display where `ollama serve` was previously suggested.
pub const ERR_LLM_NOT_RUNNING: &str = "LLM server is not running (start it with `ollama serve`)";

/// Error prefix for LLM server errors.
/// Used as format string prefix: `format!("{ERR_LLM_ERROR}: {details}")`
pub const ERR_LLM_ERROR: &str = "LLM error";

/// Error message when the LLM client is unavailable in tool context.
/// Used by subagent tools that can't access the LLM backend.
pub const ERR_LLM_CLIENT_UNAVAILABLE: &str = "Error: LLM client not available in tool context.";
