//! Platform and environment detection
//!
//! Detects the operating system, Linux distribution, and special environments
//! like Termux on Android. This information is used to customize system prompts
//! with accurate platform information instead of hardcoded values.

use std::env;

/// Detected platform type
///
/// Variants represent possible platforms - not all will be constructed
/// on every system, but all are needed for completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Platform {
    /// Standard Linux desktop (Arch, Ubuntu, Fedora, etc.)
    Linux,
    /// Android via Termux
    Termux,
    /// macOS
    MacOS,
    /// Windows
    Windows,
    /// Unknown/Other
    Other,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Linux => write!(f, "Linux"),
            Platform::Termux => write!(f, "Termux"),
            Platform::MacOS => write!(f, "macOS"),
            Platform::Windows => write!(f, "Windows"),
            Platform::Other => write!(f, "Unknown"),
        }
    }
}

/// Detected Linux distribution (Linux only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LinuxDistro {
    Arch,
    Ubuntu,
    Debian,
    Fedora,
    OpenSuse,
    Gentoo,
    Alpine,
    NixOS,
    Unknown,
}

impl std::fmt::Display for LinuxDistro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinuxDistro::Arch => write!(f, "Arch Linux"),
            LinuxDistro::Ubuntu => write!(f, "Ubuntu"),
            LinuxDistro::Debian => write!(f, "Debian"),
            LinuxDistro::Fedora => write!(f, "Fedora"),
            LinuxDistro::OpenSuse => write!(f, "openSUSE"),
            LinuxDistro::Gentoo => write!(f, "Gentoo"),
            LinuxDistro::Alpine => write!(f, "Alpine"),
            LinuxDistro::NixOS => write!(f, "NixOS"),
            LinuxDistro::Unknown => write!(f, "Linux"),
        }
    }
}

/// Platform information for prompts
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Detected platform type
    pub platform: Platform,
    /// Linux distribution (only set if platform is Linux)
    pub linux_distro: Option<LinuxDistro>,
    /// Whether running on Android (stored for potential future use)
    #[allow(dead_code)]
    pub is_android: bool,
}

impl PlatformInfo {
    /// Detect current platform
    pub fn detect() -> Self {
        let platform = Self::detect_platform();
        let is_android = platform == Platform::Termux;

        let linux_distro = if platform == Platform::Linux {
            Self::detect_linux_distro()
        } else {
            None
        };

        Self {
            platform,
            linux_distro,
            is_android,
        }
    }

    /// Get human-readable platform string for prompts
    ///
    /// Returns strings like:
    /// - "Termux on Android"
    /// - "Arch Linux"
    /// - "Ubuntu Linux"
    /// - "Linux" (unknown distro)
    /// - "macOS"
    /// - "Windows"
    pub fn prompt_string(&self) -> String {
        match self.platform {
            Platform::Termux => "Termux on Android".to_string(),
            Platform::Linux => self
                .linux_distro
                .map(|d| d.to_string())
                .unwrap_or_else(|| "Linux".to_string()),
            Platform::MacOS => "macOS".to_string(),
            Platform::Windows => "Windows".to_string(),
            Platform::Other => "unknown platform".to_string(),
        }
    }

    /// Detect the current platform
    fn detect_platform() -> Platform {
        // Check for Termux first (has TERMUX_VERSION env var)
        if env::var("TERMUX_VERSION").is_ok() {
            return Platform::Termux;
        }

        // Check for Termux via PREFIX path
        if let Ok(prefix) = env::var("PREFIX") {
            if prefix.contains("com.termux") {
                return Platform::Termux;
            }
        }

        // Use compile-time target detection
        #[cfg(target_os = "linux")]
        {
            Platform::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Platform::MacOS
        }
        #[cfg(target_os = "windows")]
        {
            Platform::Windows
        }
        #[cfg(target_os = "android")]
        {
            Platform::Termux
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
            target_os = "android"
        )))]
        {
            Platform::Other
        }
    }

    /// Detect Linux distribution by reading /etc/os-release
    fn detect_linux_distro() -> Option<LinuxDistro> {
        // Check /etc/os-release first (standard on most modern distros)
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            let content_lower = content.to_lowercase();

            if content_lower.contains("arch") || content_lower.contains("archlinux") {
                return Some(LinuxDistro::Arch);
            }
            if content_lower.contains("ubuntu") {
                return Some(LinuxDistro::Ubuntu);
            }
            if content_lower.contains("debian") {
                return Some(LinuxDistro::Debian);
            }
            if content_lower.contains("fedora") {
                return Some(LinuxDistro::Fedora);
            }
            if content_lower.contains("opensuse") || content_lower.contains("suse") {
                return Some(LinuxDistro::OpenSuse);
            }
            if content_lower.contains("gentoo") {
                return Some(LinuxDistro::Gentoo);
            }
            if content_lower.contains("alpine") {
                return Some(LinuxDistro::Alpine);
            }
            if content_lower.contains("nixos") {
                return Some(LinuxDistro::NixOS);
            }
        }

        // Fallback: check for Arch-specific files
        if std::path::Path::new("/etc/arch-release").exists() {
            return Some(LinuxDistro::Arch);
        }

        // Fallback: check for Debian-specific files
        if std::path::Path::new("/etc/debian_version").exists() {
            return Some(LinuxDistro::Debian);
        }

        // Fallback: check for Fedora-specific files
        if std::path::Path::new("/etc/fedora-release").exists() {
            return Some(LinuxDistro::Fedora);
        }

        None
    }
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detect() {
        let info = PlatformInfo::detect();

        // Should always succeed
        println!("Detected platform: {:?}", info.platform);
        println!("Linux distro: {:?}", info.linux_distro);
        println!("Is Android: {}", info.is_android);
        println!("Prompt string: {}", info.prompt_string());

        // Prompt string should not be empty
        assert!(!info.prompt_string().is_empty());
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(Platform::Linux.to_string(), "Linux");
        assert_eq!(Platform::Termux.to_string(), "Termux");
        assert_eq!(Platform::MacOS.to_string(), "macOS");
        assert_eq!(Platform::Windows.to_string(), "Windows");
    }

    #[test]
    fn test_linux_distro_display() {
        assert_eq!(LinuxDistro::Arch.to_string(), "Arch Linux");
        assert_eq!(LinuxDistro::Ubuntu.to_string(), "Ubuntu");
        assert_eq!(LinuxDistro::Debian.to_string(), "Debian");
        assert_eq!(LinuxDistro::Fedora.to_string(), "Fedora");
        assert_eq!(LinuxDistro::Unknown.to_string(), "Linux");
    }

    #[test]
    fn test_prompt_string_format() {
        // Test Termux
        let termux = PlatformInfo {
            platform: Platform::Termux,
            linux_distro: None,
            is_android: true,
        };
        assert_eq!(termux.prompt_string(), "Termux on Android");

        // Test Arch Linux
        let arch = PlatformInfo {
            platform: Platform::Linux,
            linux_distro: Some(LinuxDistro::Arch),
            is_android: false,
        };
        assert_eq!(arch.prompt_string(), "Arch Linux");

        // Test unknown Linux
        let unknown_linux = PlatformInfo {
            platform: Platform::Linux,
            linux_distro: None,
            is_android: false,
        };
        assert_eq!(unknown_linux.prompt_string(), "Linux");

        // Test macOS
        let macos = PlatformInfo {
            platform: Platform::MacOS,
            linux_distro: None,
            is_android: false,
        };
        assert_eq!(macos.prompt_string(), "macOS");

        // Test Windows
        let windows = PlatformInfo {
            platform: Platform::Windows,
            linux_distro: None,
            is_android: false,
        };
        assert_eq!(windows.prompt_string(), "Windows");
    }
}
