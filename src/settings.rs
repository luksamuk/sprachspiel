use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Default model name when not specified in config
pub const DEFAULT_MODEL: &str = "lfm";

/// Default Ollama host
pub const DEFAULT_OLLAMA_HOST: &str = "127.0.0.1";

/// Default Ollama port
pub const DEFAULT_OLLAMA_PORT: u16 = 11434;

/// Application settings loaded from config file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Model configuration
    #[serde(default)]
    pub model: ModelSettings,
    /// Tools configuration
    #[serde(default)]
    pub tools: ToolSettings,
    /// Output configuration
    #[serde(default)]
    pub output: OutputSettings,
    /// Display configuration
    #[serde(default)]
    pub display: DisplaySettings,
}

/// Model-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Default model preset name
    #[serde(default = "default_model")]
    pub default: String,
    /// Ollama host address
    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,
    /// Ollama port
    #[serde(default = "default_ollama_port")]
    pub ollama_port: u16,
}

/// Tool-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSettings {
    /// List of tools to disable
    #[serde(default = "default_blacklist")]
    pub blacklist: Vec<String>,
    /// Whether to sandbox file operations to CWD
    #[serde(default = "default_true")]
    pub file_sandbox: bool,
}

impl Default for ToolSettings {
    fn default() -> Self {
        ToolSettings {
            blacklist: default_blacklist(),
            file_sandbox: true,
        }
    }
}

/// Output-related settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputSettings {
    /// Use plain output by default
    #[serde(default)]
    pub plain_default: bool,
    /// Enable debug mode by default
    #[serde(default)]
    pub debug_default: bool,
}

/// Display-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySettings {
    /// Terminal skin/theme
    #[serde(default = "default_skin")]
    pub skin: String,
}

impl Default for ModelSettings {
    fn default() -> Self {
        ModelSettings {
            default: default_model(),
            ollama_host: default_ollama_host(),
            ollama_port: default_ollama_port(),
        }
    }
}

impl Default for DisplaySettings {
    fn default() -> Self {
        DisplaySettings {
            skin: default_skin(),
        }
    }
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_ollama_host() -> String {
    DEFAULT_OLLAMA_HOST.to_string()
}

fn default_ollama_port() -> u16 {
    DEFAULT_OLLAMA_PORT
}

fn default_skin() -> String {
    "dark".to_string()
}

fn default_blacklist() -> Vec<String> {
    vec![
        "web_search".to_string(),
        "web_search_news".to_string(),
        "web_instant_answer".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

impl Settings {
    /// Load settings from config file or use defaults
    pub fn load() -> Self {
        if let Some(config_path) = Self::config_path()
            && config_path.exists()
        {
            match Self::load_from_file(&config_path) {
                Ok(settings) => return settings,
                Err(e) => eprintln!("Warning: Failed to load config file: {}", e),
            }
        }
        Settings::default()
    }

    /// Load settings from a specific file path
    fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        let settings: Settings = toml::from_str(&content)?;
        Ok(settings)
    }

    /// Get the config file path
    pub fn config_path() -> Option<PathBuf> {
        // Check XDG_CONFIG_HOME first
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg_config).join("ask-ai").join("config.toml");
            if path.exists() {
                return Some(path);
            }
        }

        // Fall back to ~/.config/ask-ai/config.toml
        if let Some(home_dir) = dirs::home_dir() {
            let path = home_dir.join(".config").join("ask-ai").join("config.toml");
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Get the config directory path (for creating new config)
    pub fn config_dir() -> Option<PathBuf> {
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(xdg_config).join("ask-ai"));
        }

        if let Some(home_dir) = dirs::home_dir() {
            return Some(home_dir.join(".config").join("ask-ai"));
        }

        None
    }

    /// Get blacklist as a HashSet for efficient lookups
    #[allow(dead_code)]
    pub fn blacklist_set(&self) -> HashSet<&str> {
        self.tools.blacklist.iter().map(|s| s.as_str()).collect()
    }

    /// Check if a tool is blacklisted
    pub fn is_tool_blacklisted(&self, tool_name: &str) -> bool {
        self.tools.blacklist.iter().any(|b| b == tool_name)
    }

    /// Create a sample config file if it doesn't exist
    pub fn create_sample_config() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = Self::config_dir().ok_or("Could not determine config directory")?;
        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            return Ok(config_path);
        }

        let sample_config = r#"# Ask-AI Configuration File
# Located at ~/.config/ask-ai/config.toml

[model]
# Default model preset to use
# See available models with: ask-ai --list-models
default = "lfm"

# Ollama server connection
ollama_host = "127.0.0.1"
ollama_port = 11434

[tools]
# Tools to disable (blacklist)
# Available tools: web_search, web_search_news, web_instant_answer,
#   get_weather, get_current_weather, get_weather_forecast,
#   fetch_pokemon, fetch_pokemon_basic, fetch_pokemon_stats,
#   fetch_pokemon_moves, fetch_pokemon_evolution, fetch_ability_details,
#   fetch_type_effectiveness, fetch_move_details,
#   read_file, list_directory, search_files
#
# ⚠️  WEB SEARCH TOOLS ARE DISABLED BY DEFAULT ⚠️
# The web search tools (web_search, web_search_news, web_instant_answer)
# are currently blocked by DuckDuckGo CAPTCHA and do not work reliably.
# They are disabled by default to avoid confusion.
# You can enable them by removing from this blacklist, but expect failures.
blacklist = ["web_search", "web_search_news", "web_instant_answer"]

# Sandboxing for file operations tools
# When enabled, file tools can only access files in the current directory
# and its subdirectories
file_sandbox = true

[output]
# Use plain text output by default (no markdown rendering)
plain_default = false

# Enable debug output by default
debug_default = false

[display]
# Terminal skin: dark, light, or mono
skin = "dark"
"#;

        std::fs::write(&config_path, sample_config)?;
        Ok(config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.model.default, "lfm");
        assert_eq!(settings.model.ollama_host, "127.0.0.1");
        assert_eq!(settings.model.ollama_port, 11434);
        assert!(settings.tools.file_sandbox);
        assert_eq!(settings.display.skin, "dark");
        // These should be false by default
        assert!(!settings.output.plain_default);
        assert!(!settings.output.debug_default);
    }

    #[test]
    fn test_blacklist_set() {
        let mut settings = Settings::default();
        settings.tools.blacklist = vec!["web_search".to_string(), "weather".to_string()];

        let blacklist = settings.blacklist_set();
        assert!(blacklist.contains("web_search"));
        assert!(blacklist.contains("weather"));
        assert!(!blacklist.contains("pokemon_lookup"));
    }

    #[test]
    fn test_is_tool_blacklisted() {
        let mut settings = Settings::default();
        settings.tools.blacklist = vec!["web_search".to_string()];

        assert!(settings.is_tool_blacklisted("web_search"));
        assert!(!settings.is_tool_blacklisted("weather"));
    }

    #[test]
    fn test_parse_sample_config() {
        let sample = r#"
[model]
default = "gpt-oss"
ollama_host = "192.168.1.100"
ollama_port = 8080

[tools]
blacklist = ["web_search", "fetch_page"]
file_sandbox = false

[output]
plain_default = true
debug_default = true

[display]
skin = "light"
"#;

        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.model.default, "gpt-oss");
        assert_eq!(settings.model.ollama_host, "192.168.1.100");
        assert_eq!(settings.model.ollama_port, 8080);
        assert!(settings.is_tool_blacklisted("web_search"));
        assert!(!settings.tools.file_sandbox);
        assert!(settings.output.plain_default);
        assert!(settings.output.debug_default);
        assert_eq!(settings.display.skin, "light");
    }
}
