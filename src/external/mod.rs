//! External tool configuration and types.
//!
//! This module provides types and configuration for external CLI tools.

mod config;
mod types;

pub use types::{CommandOutput, ExternalTool, ExternalToolsConfig, FileToolsConfig, Platform};

pub use config::{load_file_tools_config, load_tools_config};

/// Get the sandbox status for display.
///
/// Returns:
/// - "enabled (landlock)" - Linux with sandbox feature (always enabled)
/// - "not compiled" - Not built with sandbox feature
/// - "not supported" - Platform doesn't support Landlock (Termux, macOS)
pub fn get_sandbox_status() -> &'static str {
    // First check if sandbox feature is compiled in
    #[cfg(all(feature = "sandbox", target_os = "linux"))]
    {
        // Sandbox is always enabled — no configuration option to disable it
        "enabled (landlock)"
    }

    #[cfg(all(feature = "sandbox", not(target_os = "linux")))]
    {
        // Compiled with sandbox but not on Linux
        #[cfg(target_os = "android")]
        {
            "not supported (termux)"
        }

        #[cfg(target_os = "macos")]
        {
            "not supported (macOS)"
        }

        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            "not supported"
        }
    }

    #[cfg(not(feature = "sandbox"))]
    {
        // Not compiled with sandbox feature
        #[cfg(target_os = "linux")]
        {
            "not compiled"
        }

        #[cfg(not(target_os = "linux"))]
        {
            "not available"
        }
    }
}
