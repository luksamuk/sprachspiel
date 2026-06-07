//! Tool to check if an external CLI tool is available on this system.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::external::{ExternalToolsConfig, Platform, load_tools_config};
use sprachspiel_tool_derive::tool;
use which;

/// Get the external tools configuration (cached).
fn get_config() -> &'static ExternalToolsConfig {
    static CONFIG: std::sync::OnceLock<ExternalToolsConfig> = std::sync::OnceLock::new();
    CONFIG.get_or_init(load_tools_config)
}

/// Get the current platform (cached).
fn get_platform() -> &'static Platform {
    static PLATFORM: std::sync::OnceLock<Platform> = std::sync::OnceLock::new();
    PLATFORM.get_or_init(Platform::detect)
}

/// Check if an external tool is available on this system.
///
/// Returns information about tool availability, whether it's enabled in
/// configuration, and installation hints for the current platform.
///
/// Use this tool before attempting to use `run_command` to verify that
/// the required tool is installed.
///
/// # Arguments
/// * `tool` - The tool name to check (e.g., "pdftotext", "tesseract", "exiftool")
///
/// # Available Tools
/// - **pdftotext** - Extract text from PDF files
/// - **pdfinfo** - Show PDF metadata (pages, size, etc.)
/// - **pdftoppm** - Convert PDF pages to images
/// - **tesseract** - OCR (extract text from images)
/// - **exiftool** - Image/video metadata
/// - **imagemagick** - Image conversion (binary name: "magick")
///
/// # Returns
/// A message indicating:
/// - ✓ Tool is available (if installed and enabled)
/// - ✗ Tool is installed but disabled in config
/// - ✗ Tool is not installed, with installation command
/// - ✗ Tool is not in the whitelist
///
/// # Examples
/// ```ignore
/// // Check if pdftotext is available
/// check_tool_availability("pdftotext".to_string()).await
/// // Returns: "✓ pdftotext is available"
///
/// // Check if tesseract is available
/// check_tool_availability("tesseract".to_string()).await
/// // Returns: "✗ tesseract is not installed. Install with: sudo apt install tesseract-ocr"
/// ```
#[tool]
pub async fn check_tool_availability(
    tool: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "check_tool_availability",
        &[("tool".to_string(), tool.clone())],
    );

    // Load configuration (cached)
    let config = get_config();
    let platform = get_platform();

    // Check if tool is in config
    if let Some(tool_config) = config.get(&tool) {
        let binary = &tool_config.binary;
        let enabled = tool_config.enabled;
        let installed = which::which(binary).is_ok();
        let install_hint = tool_config.install_hints.get(platform);

        let result = if installed && enabled {
            format!("✓ {} is available", tool)
        } else if installed && !enabled {
            format!("✗ {} is installed but disabled in tools.toml", tool)
        } else if !installed && enabled {
            if let Some(hint) = install_hint {
                format!("✗ {} is not installed. Install with: {}", tool, hint)
            } else {
                format!(
                    "✗ {} is not installed. Install with your package manager (e.g., {} install {}).",
                    tool,
                    platform.package_manager(),
                    binary
                )
            }
        } else {
            format!("✗ {} is not configured", tool)
        };

        log_tool_result("check_tool_availability", &result);
        Ok(result)
    } else {
        // Tool not in config
        let result = format!(
            "✗ {} is not in the whitelist. Only tools configured in tools.toml can be executed.",
            tool
        );
        log_tool_result("check_tool_availability", &result);
        Ok(result)
    }
}
