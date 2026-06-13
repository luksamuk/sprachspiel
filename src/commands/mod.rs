//! User-facing subcommands that don't fit the existing domain modules.
//!
//! Currently houses:
//! - `config upgrade` machinery, which merges missing default fields into
//!   the user's existing `config.toml` while preserving all values, comments,
//!   and formatting.
//! - `models upgrade` machinery (#120), which migrates `models.toml` to
//!   the new provider-based format.

pub mod config_upgrade;
pub mod models_upgrade;
