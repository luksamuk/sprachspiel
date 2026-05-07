//! Configuration loading for external tools.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use super::types::{ExternalTool, ExternalToolsConfig, FileToolsConfig, Platform};

/// Default file size limit: 5MB
const DEFAULT_MAX_FILE_SIZE: usize = 5_242_880;

/// TOML configuration structure for tools.toml.
#[derive(Debug, Clone, Deserialize, Default)]
struct ToolsToml {
    #[serde(default)]
    external: Option<ExternalSection>,
    #[serde(default)]
    file_tools: Option<FileToolsSection>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExternalSection {
    #[serde(default = "default_timeout")]
    default_timeout: u64,
    #[serde(default)]
    tools: HashMap<String, ToolToml>,
}

/// Per-tool configuration from TOML.
#[derive(Debug, Clone, Deserialize)]
struct ToolToml {
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default = "default_timeout")]
    timeout: u64,
    #[serde(default)]
    binary: Option<String>,
}

/// File tools configuration section.
#[derive(Debug, Clone, Deserialize)]
struct FileToolsSection {
    /// Maximum file size for operations in bytes (default: 5MB)
    #[serde(default = "default_max_file_size")]
    max_file_size: usize,
    /// Additional blocked patterns (added to defaults)
    #[serde(default)]
    blocked_patterns: Vec<String>,
    /// Whether to block read operations for sensitive files
    #[serde(default = "default_true")]
    block_read: bool,
    /// Whether to block list operations (hide filenames)
    #[serde(default)]
    block_list: bool,
    // Note: block_write is always true and cannot be configured
}

// ============================================================================
// Serde default value functions
// ============================================================================
// These functions are required by serde's #[serde(default = "...")] attribute.
// Serde requires a function pointer, not a constant or inline value.
// For simple boolean defaults, we still need functions because:
//   #[serde(default = "true")]  // ERROR: expected function, found `true`
//   #[serde(default)]           // Uses Default trait, which returns false
// So default_true/default_false are needed for non-false boolean defaults.

fn default_timeout() -> u64 {
    30
}

fn default_enabled() -> bool {
    true
}

fn default_max_file_size() -> usize {
    DEFAULT_MAX_FILE_SIZE
}

/// Returns `true` for serde defaults.
/// Required because serde's #[serde(default = "...")] needs a function,
/// and #[serde(default)] uses Default trait which returns false.
fn default_true() -> bool {
    true
}

impl Default for ExternalSection {
    fn default() -> Self {
        ExternalSection {
            default_timeout: 30,
            tools: HashMap::new(),
        }
    }
}

/// Load external tools configuration from tools.toml.
///
/// Checks for the config file in:
/// 1. $XDG_CONFIG_HOME/sprachspiel/tools.toml
/// 2. ~/.config/sprachspiel/tools.toml
///
/// Returns default configuration if file doesn't exist.
pub fn load_tools_config() -> ExternalToolsConfig {
    let config_path = find_tools_config();

    match config_path {
        Some(path) => load_from_file(&path),
        None => create_default_config(),
    }
}

/// Load file tools configuration from tools.toml.
///
/// Checks for the config file in:
/// 1. $XDG_CONFIG_HOME/sprachspiel/tools.toml
/// 2. ~/.config/sprachspiel/tools.toml
///
/// Returns default configuration if file doesn't exist or section is missing.
pub fn load_file_tools_config() -> FileToolsConfig {
    let config_path = find_tools_config();

    match config_path {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<ToolsToml>(&content) {
                Ok(toml_config) => toml_config
                    .file_tools
                    .map(|f| FileToolsConfig {
                        max_file_size: f.max_file_size,
                        blocked_patterns: f.blocked_patterns,
                        block_read: f.block_read,
                        block_list: f.block_list,
                    })
                    .unwrap_or_default(),
                Err(e) => {
                    eprintln!("Warning: Failed to parse tools.toml: {}", e);
                    FileToolsConfig::default()
                }
            },
            Err(e) => {
                eprintln!("Warning: Failed to read tools.toml: {}", e);
                FileToolsConfig::default()
            }
        },
        None => FileToolsConfig::default(),
    }
}

/// Find the tools.toml configuration file.
fn find_tools_config() -> Option<PathBuf> {
    use crate::consts::app;

    // Check XDG_CONFIG_HOME first
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg_config).join(app::APP_CONFIG_DIR).join("tools.toml");
        if path.exists() {
            return Some(path);
        }
    }

    // Fall back to ~/.config/sprachspiel/tools.toml
    if let Some(home_dir) = dirs::home_dir() {
        let path = home_dir.join(".config").join(app::APP_CONFIG_DIR).join("tools.toml");
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Load configuration from a specific file.
fn load_from_file(path: &PathBuf) -> ExternalToolsConfig {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<ToolsToml>(&content) {
            Ok(toml_config) => parse_config(toml_config),
            Err(e) => {
                eprintln!("Warning: Failed to parse tools.toml: {}", e);
                eprintln!("Using default configuration.");
                create_default_config()
            }
        },
        Err(e) => {
            eprintln!("Warning: Failed to read tools.toml: {}", e);
            create_default_config()
        }
    }
}

/// Parse TOML config into ExternalToolsConfig.
fn parse_config(toml_config: ToolsToml) -> ExternalToolsConfig {
    let default_timeout = toml_config
        .external
        .as_ref()
        .map(|e| Duration::from_secs(e.default_timeout))
        .unwrap_or(Duration::from_secs(30));

    // Start with default tools
    let mut config = ExternalToolsConfig::with_defaults();
    config.default_timeout = default_timeout;

    // Merge per-tool configuration from TOML
    if let Some(external) = &toml_config.external {
        for (tool_name, tool_config) in &external.tools {
            // Get the default tool or create a new one
            if let Some(existing) = config.tools.get_mut(tool_name) {
                // Override settings for existing tool
                existing.enabled = tool_config.enabled;
                existing.timeout = Duration::from_secs(tool_config.timeout);
                if let Some(ref binary) = tool_config.binary {
                    existing.binary = binary.clone();
                }
            } else {
                // New tool not in defaults - add it
                let binary = tool_config
                    .binary
                    .clone()
                    .unwrap_or_else(|| tool_name.clone());
                config.tools.insert(
                    tool_name.clone(),
                    ExternalTool {
                        enabled: tool_config.enabled,
                        timeout: Duration::from_secs(tool_config.timeout),
                        binary,
                        sandbox: false,
                        install_hints: default_install_hints(tool_name),
                    },
                );
            }
        }
    }

    config
}

/// Get default install hints for known tools.
fn default_install_hints(tool_name: &str) -> HashMap<Platform, String> {
    match tool_name {
        "pdftotext" | "pdfinfo" | "pdftoppm" => {
            let mut hints = HashMap::new();
            hints.insert(Platform::Arch, "sudo pacman -S poppler".to_string());
            hints.insert(
                Platform::Debian,
                "sudo apt install poppler-utils".to_string(),
            );
            hints.insert(
                Platform::Fedora,
                "sudo dnf install poppler-utils".to_string(),
            );
            hints.insert(Platform::Termux, "pkg install poppler".to_string());
            hints
        }
        "tesseract" => {
            let mut hints = HashMap::new();
            hints.insert(Platform::Arch, "sudo pacman -S tesseract".to_string());
            hints.insert(
                Platform::Debian,
                "sudo apt install tesseract-ocr".to_string(),
            );
            hints.insert(Platform::Fedora, "sudo dnf install tesseract".to_string());
            hints.insert(Platform::Termux, "pkg install tesseract".to_string());
            hints
        }
        "exiftool" => {
            let mut hints = HashMap::new();
            hints.insert(
                Platform::Arch,
                "sudo pacman -S perl-image-exiftool".to_string(),
            );
            hints.insert(
                Platform::Debian,
                "sudo apt install libimage-exiftool-perl".to_string(),
            );
            hints.insert(
                Platform::Fedora,
                "sudo dnf install perl-Image-ExifTool".to_string(),
            );
            hints.insert(Platform::Termux, "pkg install exiftool".to_string());
            hints
        }
        "imagemagick" => {
            let mut hints = HashMap::new();
            hints.insert(Platform::Arch, "sudo pacman -S imagemagick".to_string());
            hints.insert(Platform::Debian, "sudo apt install imagemagick".to_string());
            hints.insert(Platform::Fedora, "sudo dnf install ImageMagick".to_string());
            hints.insert(Platform::Termux, "pkg install imagemagick".to_string());
            hints
        }
        _ => HashMap::new(),
    }
}

/// Create the default configuration.
fn create_default_config() -> ExternalToolsConfig {
    ExternalToolsConfig::with_defaults()
}

/// Get the path to tools.toml (for creating new config).
#[allow(dead_code)]
pub fn tools_config_path() -> Option<PathBuf> {
    use crate::consts::app;

    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg_config).join(app::APP_CONFIG_DIR).join("tools.toml"));
    }

    if let Some(home_dir) = dirs::home_dir() {
        return Some(home_dir.join(".config").join(app::APP_CONFIG_DIR).join("tools.toml"));
    }

    None
}

/// Generate a default tools.toml content.
#[allow(dead_code)]
pub fn generate_default_toml() -> String {
    r#"# External tools configuration for sprachspiel
#
# This file controls which external CLI tools the LLM can execute via run_command.
# Only tools explicitly enabled here can be executed.
#
# If this file doesn't exist, all default tools are enabled.
# If this file exists, ONLY tools defined here are available.
#
# =============================================================================
# SECURITY MODEL
# =============================================================================
#
# Commands are executed WITHOUT shell interpretation (no pipes, redirects).
# This prevents command injection attacks.
#
# Output is returned in FULL. Use head/tail parameters to control size:
#   run_command("pdftotext doc.pdf -", 100, null, null)  # First 100 lines
#   run_command("pdftotext doc.pdf -", null, 50, null)   # Last 50 lines
#
# =============================================================================
# SANDBOX (Linux only)
# =============================================================================
#
# Landlock sandbox: Uses Landlock to isolate filesystem access
# - Requires Linux kernel 5.13+ (graceful degradation on older kernels)
# - Automatically disabled on Termux, macOS, and non-Linux systems
# - Restricts file access to current directory + /usr (read-only)
# - Always enabled — cannot be disabled for security
#
# =============================================================================
# PLATFORM SUPPORT
# =============================================================================
#
# Linux:    Full sandbox support (Landlock)
# Termux:   No sandbox (Android provides app-level isolation)
# macOS:    No sandbox yet (future: sandbox-exec)
# Windows:  Not supported (use WSL)
#

[external]
# Default timeout for all commands (seconds)
default_timeout = 30

# =============================================================================
# PDF TOOLS (from poppler-utils)
# =============================================================================

[external.tools.pdftotext]
# Extract text from PDF files to stdout
# USAGE: pdftotext [-f <first>] [-l <last>] <file.pdf> -
# EXAMPLE: pdftotext -f 1 -l 10 document.pdf -  (extract pages 1-10)
enabled = true
timeout = 30
binary = "pdftotext"

[external.tools.pdfinfo]
# Show PDF metadata (pages, size, encryption, etc.)
# USAGE: pdfinfo <file.pdf>
enabled = true
timeout = 5
binary = "pdfinfo"

[external.tools.pdftoppm]
# Convert PDF pages to images (PNG, JPEG)
# USAGE: pdftoppm -png <file.pdf> <output_prefix>
# NOTE: Output goes to files, not stdout
enabled = true
timeout = 60
binary = "pdftoppm"

# =============================================================================
# OCR TOOLS
# =============================================================================

[external.tools.tesseract]
# Extract text from images using OCR (Optical Character Recognition)
# USAGE: tesseract <image.png> stdout
# SUPPORTS: PNG, JPEG, TIFF, BMP, GIF
# NOTE: Accuracy depends on image quality and language packs installed
enabled = true
timeout = 120
binary = "tesseract"

# =============================================================================
# EPUB TOOLS
# =============================================================================

[external.tools.ebook-convert]
# Convert ePub to text using Calibre (full-featured)
# USAGE: ebook-convert <file.epub> .txt
# NOTE: Creates <file>.txt in current directory
# NOTE: Requires calibre package (large install ~120MB)
enabled = true
timeout = 60
binary = "ebook-convert"

[external.tools.epub2txt]
# Lightweight ePub to text converter (fallback)
# USAGE: epub2txt <file.epub> -
# NOTE: Outputs to stdout, lighter alternative to calibre
enabled = true
timeout = 30
binary = "epub2txt"

# =============================================================================
# IMAGE TOOLS
# =============================================================================

[external.tools.exiftool]
# Read/write metadata from images, videos, and many other file types
# USAGE: exiftool <file.jpg>
# SUPPORTS: JPEG, PNG, TIFF, MP4, PDF, and 100+ other formats
enabled = true
timeout = 10
binary = "exiftool"

[external.tools.imagemagick]
# Image conversion and manipulation (resize, crop, format convert)
# USAGE: magick convert input.png -resize 50% output.jpg
# WARNING: Powerful tool - consider disabling if security is a concern
# NOTE: Binary is "magick" not "imagemagick" (v7 naming)
enabled = true
timeout = 60
binary = "magick"

# =============================================================================
# INSTALLATION NOTES
# =============================================================================
#
# Arch Linux:
#   sudo pacman -S poppler tesseract perl-image-exiftool imagemagick calibre
#   yay -S epub2txt  # optional lightweight ePub fallback
#
# Debian/Ubuntu:
#   sudo apt install poppler-utils tesseract-ocr libimage-exiftool-perl imagemagick calibre
#   pip install epub2txt  # optional lightweight ePub fallback
#
# Void Linux:
#   sudo xbps-install -S poppler tesseract exiftool imagemagick calibre epub2txt
#
# Alpine Linux (calibre in edge/testing only):
#   sudo apk add poppler tesseract exiftool imagemagick epub2txt
#   sudo apk add calibre --repository=http://dl-cdn.alpinelinux.org/alpine/edge/testing
#
# Fedora:
#   sudo dnf install poppler-utils tesseract perl-Image-ExifTool ImageMagick calibre
#   pip install epub2txt  # optional lightweight ePub fallback
#
# Termux (Android):
#   pkg install poppler tesseract exiftool imagemagick
#   # calibre and epub2txt not available in Termux
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_config() {
        let config = create_default_config();

        // Should have PDF tools
        assert!(config.tools.contains_key("pdftotext"));
        assert!(config.tools.contains_key("pdfinfo"));
        assert!(config.tools.contains_key("pdftoppm"));

        // Should have OCR tool
        assert!(config.tools.contains_key("tesseract"));

        // Should have ePub tools
        assert!(config.tools.contains_key("ebook-convert"));
        assert!(config.tools.contains_key("epub2txt"));

        // Should have image tools
        assert!(config.tools.contains_key("exiftool"));
        assert!(config.tools.contains_key("imagemagick"));

        // Default timeout
        assert_eq!(config.default_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_empty_toml() {
        let toml = ToolsToml::default();
        let config = parse_config(toml);

        // Should still have all default tools
        assert!(config.tools.contains_key("pdftotext"));
        assert_eq!(config.default_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_config_with_custom_timeout() {
        let content = r#"
[external]
default_timeout = 60
"#;
        let toml: ToolsToml = toml::from_str(content).unwrap();
        let config = parse_config(toml);

        assert_eq!(config.default_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_parse_config_disable_tool() {
        let content = r#"
[external]
default_timeout = 30

[external.tools.tesseract]
enabled = false
"#;
        let toml: ToolsToml = toml::from_str(content).unwrap();
        let config = parse_config(toml);

        // tesseract should be disabled
        let tesseract = config.tools.get("tesseract").unwrap();
        assert!(!tesseract.enabled);
        assert_eq!(tesseract.binary, "tesseract");

        // Other tools should still be enabled
        let pdftotext = config.tools.get("pdftotext").unwrap();
        assert!(pdftotext.enabled);
    }

    #[test]
    fn test_parse_config_custom_timeout() {
        let content = r#"
[external]

[external.tools.pdftotext]
timeout = 120
"#;
        let toml: ToolsToml = toml::from_str(content).unwrap();
        let config = parse_config(toml);

        let pdftotext = config.tools.get("pdftotext").unwrap();
        assert_eq!(pdftotext.timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_parse_config_custom_binary() {
        let content = r#"
[external]

[external.tools.imagemagick]
binary = "convert"
"#;
        let toml: ToolsToml = toml::from_str(content).unwrap();
        let config = parse_config(toml);

        let imagemagick = config.tools.get("imagemagick").unwrap();
        assert_eq!(imagemagick.binary, "convert");
    }

    #[test]
    fn test_parse_config_new_tool() {
        let content = r#"
[external]

[external.tools.ffmpeg]
enabled = true
timeout = 300
binary = "ffmpeg"
"#;
        let toml: ToolsToml = toml::from_str(content).unwrap();
        let config = parse_config(toml);

        // ffmpeg should be added
        assert!(config.tools.contains_key("ffmpeg"));
        let ffmpeg = config.tools.get("ffmpeg").unwrap();
        assert!(ffmpeg.enabled);
        assert_eq!(ffmpeg.timeout, Duration::from_secs(300));
        assert_eq!(ffmpeg.binary, "ffmpeg");
    }

    #[test]
    fn test_parse_config_full_example() {
        let content = r#"
[external]
default_timeout = 60

[external.tools.pdftotext]
enabled = true
timeout = 30

[external.tools.tesseract]
enabled = false
timeout = 120

[external.tools.ffmpeg]
enabled = true
timeout = 300
binary = "ffmpeg"
"#;
        let toml: ToolsToml = toml::from_str(content).unwrap();
        let config = parse_config(toml);

        // Global settings
        assert_eq!(config.default_timeout, Duration::from_secs(60));

        // pdftotext: default timeout overridden
        let pdftotext = config.tools.get("pdftotext").unwrap();
        assert!(pdftotext.enabled);
        assert_eq!(pdftotext.timeout, Duration::from_secs(30));

        // tesseract: disabled
        let tesseract = config.tools.get("tesseract").unwrap();
        assert!(!tesseract.enabled);

        // ffmpeg: new tool
        let ffmpeg = config.tools.get("ffmpeg").unwrap();
        assert!(ffmpeg.enabled);
        assert_eq!(ffmpeg.timeout, Duration::from_secs(300));

        // Other tools should still be at defaults
        let exiftool = config.tools.get("exiftool").unwrap();
        assert!(exiftool.enabled);
        assert_eq!(exiftool.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_generate_default_toml() {
        let toml = generate_default_toml();

        // Should contain all sections
        assert!(toml.contains("[external]"));
        assert!(toml.contains("[external.tools.pdftotext]"));
        assert!(toml.contains("[external.tools.tesseract]"));
        assert!(toml.contains("[external.tools.exiftool]"));
        assert!(toml.contains("enabled = true"));
    }
}
