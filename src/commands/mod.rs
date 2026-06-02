//! User-facing subcommands that don't fit the existing domain modules.
//!
//! Currently houses the `config upgrade` machinery, which merges missing
//! default fields into the user's existing `config.toml` while preserving
//! all values, comments, and formatting.

pub mod config_upgrade;
