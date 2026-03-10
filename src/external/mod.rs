//! External tool configuration and types.
//!
//! This module provides types and configuration for external CLI tools.

mod config;
mod types;

pub use types::{
    CommandError, CommandOutput, ExternalTool, ExternalToolsConfig, Platform, ToolAvailability,
};

pub use config::load_tools_config;
