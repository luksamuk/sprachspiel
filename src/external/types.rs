//! Types for external tool configuration and execution.

use std::collections::HashMap;
use std::time::Duration;

/// Platform identifiers for install hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Arch,
    Debian,
    Fedora,
    Termux,
    Other,
}

impl Platform {
    /// Detect the current platform.
    pub fn detect() -> Self {
        // Check for Termux first (Android)
        if std::env::var("TERMUX_VERSION").is_ok() {
            return Platform::Termux;
        }

        // Try to detect Linux distribution
        if let Ok(os_release) = std::fs::read_to_string("/etc/os-release") {
            let os_release = os_release.to_lowercase();

            if os_release.contains("arch") || os_release.contains("manjaro") {
                return Platform::Arch;
            }
            if os_release.contains("debian") || os_release.contains("ubuntu") {
                return Platform::Debian;
            }
            if os_release.contains("fedora") || os_release.contains("rhel") {
                return Platform::Fedora;
            }
        }

        Platform::Other
    }

    /// Get the package manager command for this platform.
    pub fn package_manager(&self) -> &'static str {
        match self {
            Platform::Arch => "pacman",
            Platform::Debian => "apt",
            Platform::Fedora => "dnf",
            Platform::Termux => "pkg",
            Platform::Other => "your package manager",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Arch => write!(f, "Arch Linux"),
            Platform::Debian => write!(f, "Debian/Ubuntu"),
            Platform::Fedora => write!(f, "Fedora/RHEL"),
            Platform::Termux => write!(f, "Termux (Android)"),
            Platform::Other => write!(f, "Other"),
        }
    }
}

/// Configuration for a single external tool.
#[derive(Debug, Clone)]
pub struct ExternalTool {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Execution timeout in seconds.
    pub timeout: Duration,
    /// Binary name to search in PATH.
    pub binary: String,
    /// Whether to sandbox the execution (future: landlock).
    pub sandbox: bool,
    /// Installation hints by platform.
    pub install_hints: HashMap<Platform, String>,
}

impl Default for ExternalTool {
    fn default() -> Self {
        ExternalTool {
            enabled: true,
            timeout: Duration::from_secs(30),
            binary: String::new(),
            sandbox: false,
            install_hints: HashMap::new(),
        }
    }
}

impl ExternalTool {
    /// Create a new tool configuration with a binary name.
    pub fn new(binary: impl Into<String>) -> Self {
        ExternalTool {
            binary: binary.into(),
            ..Default::default()
        }
    }

    /// Create a disabled tool configuration.
    pub fn disabled(binary: impl Into<String>) -> Self {
        ExternalTool {
            enabled: false,
            binary: binary.into(),
            ..Default::default()
        }
    }

    /// Set the timeout in seconds.
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = Duration::from_secs(seconds);
        self
    }

    /// Enable sandboxing for this tool.
    pub fn with_sandbox(mut self) -> Self {
        self.sandbox = true;
        self
    }

    /// Add an install hint for a platform.
    pub fn install_hint(mut self, platform: Platform, hint: impl Into<String>) -> Self {
        self.install_hints.insert(platform, hint.into());
        self
    }
}

/// Configuration for external tools parsed from tools.toml.
#[derive(Debug, Clone, Default)]
pub struct ExternalToolsConfig {
    /// Default timeout for all commands (seconds).
    pub default_timeout: Duration,
    /// Configured tools by name.
    pub tools: HashMap<String, ExternalTool>,
}

/// Configuration for file tools (read/write operations).
///
/// Loaded from `[file-tools]` section in tools.toml.
/// Controls blocked patterns and size limits for file operations.
#[derive(Debug, Clone)]
pub struct FileToolsConfig {
    /// Maximum file size for operations in bytes (default: 5MB).
    pub max_file_size: usize,
    /// Additional blocked patterns (added to defaults).
    pub blocked_patterns: Vec<String>,
    /// Whether to block read operations for sensitive files.
    pub block_read: bool,
    /// Whether to block list operations (hide filenames).
    pub block_list: bool,
}

impl Default for FileToolsConfig {
    fn default() -> Self {
        Self {
            max_file_size: 5_242_880, // 5MB
            blocked_patterns: Vec::new(),
            block_read: true,
            block_list: false,
        }
    }
}

impl ExternalToolsConfig {
    /// Create a new configuration with defaults.
    pub fn new() -> Self {
        ExternalToolsConfig {
            default_timeout: Duration::from_secs(30),
            tools: HashMap::new(),
        }
    }

    /// Create the default configuration with commonly used tools.
    pub fn with_defaults() -> Self {
        let mut config = Self::new();

        // PDF Tools
        config.tools.insert(
            "pdftotext".to_string(),
            ExternalTool::new("pdftotext")
                .with_timeout(30)
                .install_hint(Platform::Arch, "sudo pacman -S poppler")
                .install_hint(Platform::Debian, "sudo apt install poppler-utils")
                .install_hint(Platform::Fedora, "sudo dnf install poppler-utils")
                .install_hint(Platform::Termux, "pkg install poppler"),
        );

        config.tools.insert(
            "pdfinfo".to_string(),
            ExternalTool::new("pdfinfo")
                .with_timeout(5)
                .install_hint(Platform::Arch, "sudo pacman -S poppler")
                .install_hint(Platform::Debian, "sudo apt install poppler-utils")
                .install_hint(Platform::Fedora, "sudo dnf install poppler-utils")
                .install_hint(Platform::Termux, "pkg install poppler"),
        );

        config.tools.insert(
            "pdftoppm".to_string(),
            ExternalTool::new("pdftoppm")
                .with_timeout(60)
                .install_hint(Platform::Arch, "sudo pacman -S poppler")
                .install_hint(Platform::Debian, "sudo apt install poppler-utils")
                .install_hint(Platform::Fedora, "sudo dnf install poppler-utils")
                .install_hint(Platform::Termux, "pkg install poppler"),
        );

        // OCR Tools
        config.tools.insert(
            "tesseract".to_string(),
            ExternalTool::new("tesseract")
                .with_timeout(120)
                .install_hint(Platform::Arch, "sudo pacman -S tesseract")
                .install_hint(Platform::Debian, "sudo apt install tesseract-ocr")
                .install_hint(Platform::Fedora, "sudo dnf install tesseract")
                .install_hint(Platform::Termux, "pkg install tesseract"),
        );

        // ePub Tools
        config.tools.insert(
            "ebook-convert".to_string(),
            ExternalTool::new("ebook-convert")
                .with_timeout(60)
                .install_hint(Platform::Arch, "sudo pacman -S calibre")
                .install_hint(Platform::Debian, "sudo apt install calibre")
                .install_hint(Platform::Fedora, "sudo dnf install calibre"), // Termux: calibre not available
        );

        config.tools.insert(
            "epub2txt".to_string(),
            ExternalTool::new("epub2txt")
                .with_timeout(30)
                .install_hint(Platform::Arch, "yay -S epub2txt")
                .install_hint(Platform::Debian, "pip install epub2txt")
                .install_hint(Platform::Fedora, "pip install epub2txt"), // Termux: epub2txt not available
        );

        // Image Tools
        config.tools.insert(
            "exiftool".to_string(),
            ExternalTool::new("exiftool")
                .with_timeout(10)
                .install_hint(Platform::Arch, "sudo pacman -S perl-image-exiftool")
                .install_hint(Platform::Debian, "sudo apt install libimage-exiftool-perl")
                .install_hint(Platform::Fedora, "sudo dnf install perl-Image-ExifTool")
                .install_hint(Platform::Termux, "pkg install exiftool"),
        );

        config.tools.insert(
            "imagemagick".to_string(),
            ExternalTool::new("magick")
                .with_timeout(60)
                .install_hint(Platform::Arch, "sudo pacman -S imagemagick")
                .install_hint(Platform::Debian, "sudo apt install imagemagick")
                .install_hint(Platform::Fedora, "sudo dnf install ImageMagick")
                .install_hint(Platform::Termux, "pkg install imagemagick"),
        );

        // Search Tools
        config.tools.insert(
            "rg".to_string(),
            ExternalTool::new("rg")
                .with_timeout(30)
                .install_hint(Platform::Arch, "sudo pacman -S ripgrep")
                .install_hint(Platform::Debian, "sudo apt install ripgrep")
                .install_hint(Platform::Fedora, "sudo dnf install ripgrep")
                .install_hint(Platform::Termux, "pkg install ripgrep"),
        );

        config
    }

    /// Get a tool configuration by name.
    pub fn get(&self, name: &str) -> Option<&ExternalTool> {
        self.tools.get(name)
    }

    /// Check if a tool is configured and enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.tools.get(name).map(|t| t.enabled).unwrap_or(false)
    }
}

/// Output from command execution.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Exit code (None if killed by signal).
    pub exit_code: Option<i32>,
    /// Whether the command succeeded.
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detect() {
        let platform = Platform::detect();
        assert!(matches!(
            platform,
            Platform::Arch
                | Platform::Debian
                | Platform::Fedora
                | Platform::Termux
                | Platform::Other
        ));
    }

    #[test]
    fn test_platform_display() {
        assert!(!Platform::Arch.to_string().is_empty());
        assert!(!Platform::Debian.to_string().is_empty());
        assert!(!Platform::Fedora.to_string().is_empty());
        assert!(!Platform::Termux.to_string().is_empty());
    }

    #[test]
    fn test_external_tool_default() {
        let tool = ExternalTool::default();
        assert!(tool.enabled);
        assert_eq!(tool.timeout, Duration::from_secs(30));
        assert!(!tool.sandbox);
    }

    #[test]
    fn test_external_tool_new() {
        let tool = ExternalTool::new("pdftotext");
        assert_eq!(tool.binary, "pdftotext");
        assert!(tool.enabled);
    }

    #[test]
    fn test_external_tool_disabled() {
        let tool = ExternalTool::disabled("ffmpeg");
        assert!(!tool.enabled);
    }

    #[test]
    fn test_external_tool_builder() {
        let tool = ExternalTool::new("pdftotext")
            .with_timeout(60)
            .with_sandbox()
            .install_hint(Platform::Debian, "apt install poppler-utils");

        assert_eq!(tool.timeout, Duration::from_secs(60));
        assert!(tool.sandbox);
        assert_eq!(
            tool.install_hints.get(&Platform::Debian),
            Some(&"apt install poppler-utils".to_string())
        );
    }

    #[test]
    fn test_config_with_defaults() {
        let config = ExternalToolsConfig::with_defaults();

        assert!(config.tools.contains_key("pdftotext"));
        assert!(config.tools.contains_key("pdfinfo"));
        assert!(config.tools.contains_key("pdftoppm"));
        assert!(config.tools.contains_key("tesseract"));
        assert!(config.tools.contains_key("exiftool"));
        assert!(config.tools.contains_key("rg"));
    }

    #[test]
    fn test_command_output() {
        let output = CommandOutput {
            stdout: "test output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        };

        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
    }
}
