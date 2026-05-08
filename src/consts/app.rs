//! Application name and file constants
//!
//! Single source of truth for the application name, database filename,
//! and directory paths. All code that references the app name or data
//! locations MUST use these constants — never hardcode "sprach" or
//! "sprachspiel" or path components directly.
//!
//! Naming convention:
//! - `APP_NAME` = binary/cli command name (short, for typing)
//! - `APP_PROJECT_NAME` = project identity (long, for dirs, docs, branding)

/// Application binary name (the command users type)
/// Must match Cargo.toml `[[bin]] name`.
pub const APP_NAME: &str = "sprach";

/// Project identity name (for directories, branding, docs)
/// Used for config dir, data dir, project dir, DB filename, etc.
pub const APP_PROJECT_NAME: &str = "sprachspiel";

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