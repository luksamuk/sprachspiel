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
/// - "Enabled (Landlock)" - Linux with sandbox feature and enable_sandbox=true
/// - "Available" - Linux with sandbox feature but disabled in config
/// - "Not compiled" - Not built with sandbox feature
/// - "Not supported" - Platform doesn't support Landlock (Termux, macOS)
pub fn get_sandbox_status() -> &'static str {
    // First check if sandbox feature is compiled in
    #[cfg(all(feature = "sandbox", target_os = "linux"))]
    {
        // Check config
        let config = crate::tools::run_cmd::get_config();
        if config.enable_sandbox {
            "Enabled (Landlock)"
        } else {
            "Available (disabled in config)"
        }
    }

    #[cfg(all(feature = "sandbox", not(target_os = "linux")))]
    {
        // Compiled with sandbox but not on Linux
        #[cfg(target_os = "android")]
        {
            "Not supported (Termux)"
        }

        #[cfg(target_os = "macos")]
        {
            "Not supported (macOS)"
        }

        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        {
            "Not supported"
        }
    }

    #[cfg(not(feature = "sandbox"))]
    {
        // Not compiled with sandbox feature
        #[cfg(target_os = "linux")]
        {
            "Not compiled (use --features sandbox)"
        }

        #[cfg(not(target_os = "linux"))]
        {
            "Not available"
        }
    }
}
