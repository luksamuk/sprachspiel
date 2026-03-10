//! Tool registry for external tool detection and management.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::config::load_tools_config;
use super::types::{CommandError, ExternalTool, ExternalToolsConfig, Platform, ToolAvailability};

/// Global registry instance.
static REGISTRY: OnceLock<ToolRegistry> = OnceLock::new();

/// Get or initialize the global registry.
pub fn get_registry() -> &'static ToolRegistry {
    REGISTRY.get_or_init(|| ToolRegistry::load())
}

/// Registry for external tool management.
///
/// Handles:
/// - Tool availability detection via `which`
/// - Configuration parsing from tools.toml
/// - Platform-specific installation hints
pub struct ToolRegistry {
    /// Configuration loaded from tools.toml.
    config: ExternalToolsConfig,
    /// Detected platform.
    platform: Platform,
    /// Cache of tool availability (name -> installed).
    availability_cache: HashMap<String, bool>,
}

impl ToolRegistry {
    /// Create a new registry with the given configuration.
    pub fn new(config: ExternalToolsConfig) -> Self {
        ToolRegistry {
            config,
            platform: Platform::detect(),
            availability_cache: HashMap::new(),
        }
    }

    /// Load the registry from configuration file.
    pub fn load() -> Self {
        let config = load_tools_config();
        Self::new(config)
    }

    /// Get the detected platform.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Check if a tool is available in PATH.
    ///
    /// This checks the `which` crate and caches the result.
    pub fn is_installed(&mut self, tool_name: &str) -> bool {
        if let Some(&available) = self.availability_cache.get(tool_name) {
            return available;
        }

        // Get binary name from config, or use tool_name as binary
        let binary = self
            .config
            .get(tool_name)
            .map(|t| t.binary.as_str())
            .unwrap_or(tool_name);

        let available = which::which(binary).is_ok();
        self.availability_cache
            .insert(tool_name.to_string(), available);
        available
    }

    /// Check if a tool is enabled in configuration.
    ///
    /// Returns true if the tool is explicitly enabled in tools.toml,
    /// or if it's part of the default tools and not explicitly disabled.
    pub fn is_enabled(&self, tool_name: &str) -> bool {
        self.config.is_enabled(tool_name)
    }

    /// Get the timeout for a tool.
    ///
    /// Returns the configured timeout, or the default timeout.
    pub fn timeout(&self, tool_name: &str) -> std::time::Duration {
        self.config
            .get(tool_name)
            .map(|t| t.timeout)
            .unwrap_or(self.config.default_timeout)
    }

    /// Check if a tool requires sandboxing.
    pub fn requires_sandbox(&self, tool_name: &str) -> bool {
        self.config
            .get(tool_name)
            .map(|t| t.sandbox)
            .unwrap_or(false)
    }

    /// Get the installation hint for a tool on the current platform.
    pub fn install_hint(&self, tool_name: &str) -> Option<&str> {
        self.config
            .get(tool_name)
            .and_then(|t| t.install_hints.get(&self.platform).map(|s| s.as_str()))
    }

    /// Get full availability information for a tool.
    pub fn get_availability(&mut self, tool_name: &str) -> ToolAvailability {
        let installed = self.is_installed(tool_name);
        let enabled = self.is_enabled(tool_name);
        let install_hint = self.install_hint(tool_name).map(|s| s.to_string());

        ToolAvailability {
            name: tool_name.to_string(),
            installed,
            enabled,
            install_hint,
        }
    }

    /// List all configured tools.
    pub fn list_tools(&self) -> Vec<&str> {
        self.config.tools.keys().map(|s| s.as_str()).collect()
    }

    /// List installed and enabled tools.
    pub fn list_available(&mut self) -> Vec<&str> {
        self.config
            .tools
            .keys()
            .filter(|name| {
                let enabled = self.is_enabled(name);
                if !enabled {
                    return false;
                }
                self.is_installed(name)
            })
            .map(|s| s.as_str())
            .collect()
    }

    /// Validate that a tool can be executed.
    ///
    /// Returns an error if the tool is disabled or not installed.
    pub fn validate(&mut self, tool_name: &str) -> Result<ExternalTool, CommandError> {
        // Check if tool is in configuration
        let tool = self.config.get(tool_name).cloned();

        if let Some(tool) = tool {
            // Check if enabled
            if !tool.enabled {
                return Err(CommandError::Disabled(tool_name.to_string()));
            }

            // Check if installed
            if !self.is_installed(tool_name) {
                return Err(CommandError::NotFound(tool_name.to_string()));
            }

            Ok(tool)
        } else {
            // Tool not in configuration - not whitelisted
            Err(CommandError::Disabled(format!(
                "{} (not in whitelist)",
                tool_name
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_registry_load() {
        let registry = ToolRegistry::load();

        // Should have default tools
        assert!(registry.config.tools.contains_key("pdftotext"));
        assert!(registry.config.tools.contains_key("tesseract"));
    }

    #[test]
    fn test_registry_platform() {
        let registry = ToolRegistry::load();
        let platform = registry.platform();

        // Should detect a valid platform
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
    fn test_registry_is_enabled() {
        let registry = ToolRegistry::load();

        // Default tools should be enabled
        assert!(registry.is_enabled("pdftotext"));
        assert!(registry.is_enabled("tesseract"));
        assert!(registry.is_enabled("exiftool"));

        // Unknown tools should be disabled
        assert!(!registry.is_enabled("unknown_tool_xyz"));
    }

    #[test]
    fn test_registry_timeout() {
        let registry = ToolRegistry::load();

        // tesseract has 120s timeout in defaults
        let timeout = registry.timeout("tesseract");
        assert_eq!(timeout, Duration::from_secs(120));

        // Default timeout for unknown tools
        let timeout = registry.timeout("unknown_tool_xyz");
        assert_eq!(timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_registry_install_hint() {
        let registry = ToolRegistry::load();
        let platform = registry.platform();

        // Should have install hints for known tools
        let hint = registry.install_hint("pdftotext");
        if matches!(platform, Platform::Arch) {
            assert!(hint.unwrap().contains("pacman"));
        } else if matches!(platform, Platform::Debian) {
            assert!(hint.unwrap().contains("apt"));
        }

        // Unknown tools should have no hint
        let hint = registry.install_hint("unknown_tool_xyz");
        assert!(hint.is_none());
    }

    #[test]
    fn test_registry_list_tools() {
        let registry = ToolRegistry::load();
        let tools = registry.list_tools();

        // Should have PDF, OCR, and image tools
        assert!(tools.contains(&"pdftotext"));
        assert!(tools.contains(&"tesseract"));
        assert!(tools.contains(&"exiftool"));
    }
}
