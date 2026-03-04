use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Default model name when not specified in config
pub const DEFAULT_MODEL: &str = "llama3.1";

/// Default Ollama host
pub const DEFAULT_OLLAMA_HOST: &str = "127.0.0.1";

/// Default Ollama port
pub const DEFAULT_OLLAMA_PORT: u16 = 11434;

/// Normalize host string to ensure it has a scheme (http:// or https://)
/// This handles cases where users configure just an IP address like "192.168.1.100"
pub fn normalize_host(host: &str) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{}", trimmed)
    }
}

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
    /// LED control configuration
    #[serde(default)]
    pub led: LedSettings,
}

/// Model-related settings with per-subcommand configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Default model preset name (used by query subcommand if not specified)
    #[serde(default = "default_model")]
    pub default: String,
    /// Ollama host address
    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,
    /// Ollama port
    #[serde(default = "default_ollama_port")]
    pub ollama_port: u16,
    /// Global default for thinking mode (used as fallback for all subcommands)
    #[serde(default)]
    pub thinking: Option<bool>,
    /// Per-subcommand model configurations
    #[serde(default)]
    pub query: SubcommandModelConfig,
    #[serde(default)]
    pub chat: SubcommandModelConfig,
    #[serde(default)]
    pub summarize: SubcommandModelConfig,
    #[serde(default)]
    pub code: SubcommandModelConfig,
    #[serde(default)]
    pub vision: SubcommandModelConfig,
    /// Translate subcommand configuration
    /// Falls back to "translategemma" if model not specified
    #[serde(default)]
    pub translate: SubcommandModelConfig,
}

/// Model configuration for a specific subcommand
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubcommandModelConfig {
    /// Model preset name for this subcommand
    #[serde(default)]
    pub model: Option<String>,
    /// Enable thinking mode for this subcommand
    #[serde(default)]
    pub thinking: Option<bool>,
    /// Enable tools for this subcommand
    #[serde(default)]
    pub tools: Option<bool>,
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

/// LED control settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LedSettings {
    /// IP address of the LED device (Raspberry Pi Pico W)
    /// If not set, LED tools will be disabled
    #[serde(default)]
    pub ip: Option<String>,
    /// HTTP port for LED device (default: 80)
    #[serde(default = "default_led_port")]
    pub port: u16,
}

fn default_led_port() -> u16 {
    80
}

impl Default for ModelSettings {
    fn default() -> Self {
        ModelSettings {
            default: default_model(),
            ollama_host: default_ollama_host(),
            ollama_port: default_ollama_port(),
            thinking: None,
            query: SubcommandModelConfig::default(),
            chat: SubcommandModelConfig::default(),
            summarize: SubcommandModelConfig::default(),
            code: SubcommandModelConfig::default(),
            vision: SubcommandModelConfig::default(),
            translate: SubcommandModelConfig::default(),
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
    Vec::new()
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
    pub fn blacklist_set(&self) -> HashSet<&str> {
        self.tools.blacklist.iter().map(|s| s.as_str()).collect()
    }

    /// Check if a tool is blacklisted
    pub fn is_tool_blacklisted(&self, tool_name: &str) -> bool {
        self.tools.blacklist.iter().any(|b| b == tool_name)
    }

    /// Check if LED is configured (IP address set)
    ///
    /// Note: Used only when `led-tools` feature is enabled.
    #[allow(dead_code)]
    pub fn is_led_configured(&self) -> bool {
        self.led.ip.is_some()
    }

    /// Get LED endpoint URL (returns None if not configured)
    ///
    /// Note: Used only when `led-tools` feature is enabled.
    #[allow(dead_code)]
    pub fn led_endpoint(&self) -> Option<String> {
        self.led.ip.as_ref().map(|ip| {
            format!("http://{}:{}", ip, self.led.port)
        })
    }

    /// Get model configuration for a specific subcommand
    /// Returns (model_name, thinking_enabled, tools_enabled)
    ///
    /// Priority for thinking:
    /// 1. Subcommand-specific config (model.query.thinking, model.chat.thinking, etc.)
    /// 2. Global config (model.thinking)
    /// 3. Model default (from model config in models.toml or built-in)
    /// 4. Hardcoded default (false for most, true for query)
    pub fn get_subcommand_config(&self, subcommand: &str) -> (String, bool, bool) {
        let subcommand_config = match subcommand {
            "query" => &self.model.query,
            "chat" => &self.model.chat,
            "summarize" => &self.model.summarize,
            "code" => &self.model.code,
            "vision" => &self.model.vision,
            "translate" => &self.model.translate,
            _ => &SubcommandModelConfig::default(),
        };

        // Get model: subcommand specific -> global default
        let model = subcommand_config
            .model
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.model.default.clone());

        // Get thinking: subcommand specific -> global -> model default
        // Note: This returns the config preference; model capability check happens elsewhere
        let thinking = subcommand_config
            .thinking
            .or(self.model.thinking)
            .unwrap_or({
                // Fall back to subcommand-specific defaults
                match subcommand {
                    "query" => true,  // Query benefits from thinking by default
                    _ => false,        // Chat, summarize, code, vision default to no thinking
                }
            });

        // Get tools: subcommand specific -> default by subcommand
        let default_tools = match subcommand {
            "query" => true,
            "chat" => true,
            "code" => true,
            "summarize" => false,
            "translate" => false,
            "vision" => false,
            _ => true,
        };
        let tools = subcommand_config.tools.unwrap_or(default_tools);

        (model, thinking, tools)
    }

    /// Create an Ollama client using the configured host and port
    pub fn ollama_client(&self) -> Ollama {
        if self.model.ollama_host != DEFAULT_OLLAMA_HOST
            || self.model.ollama_port != DEFAULT_OLLAMA_PORT
        {
            Ollama::new(
                normalize_host(&self.model.ollama_host),
                self.model.ollama_port,
            )
        } else {
            Ollama::default()
        }
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
# Location: ~/.config/ask-ai/config.toml
# 
# This is a complete example configuration showing all available options.
# Lines starting with '#' are comments and are ignored.
# Remove the '#' to enable an option, or modify values as needed.
# 
# After editing, the configuration takes effect immediately on the next run.

# =============================================================================
# MODEL CONFIGURATION
# =============================================================================
# Configure which AI models to use for different tasks.

[model]

# The default model preset to use for general queries.
# See all available models with: ask-ai --list-models
# Recommended: "llama3.1" (built-in) or "ministral" (from models.toml)
# Default: "llama3.1"
default = "llama3.1"

# Global default for thinking mode.
# This is used as a fallback for all subcommands that don't have their own setting.
# Subcommand-specific settings (model.query.thinking, model.chat.thinking, etc.) override this.
# Model capability takes precedence: if the model doesn't support thinking, this is ignored.
# If not specified, subcommand defaults are used (true for query, false for others).
# thinking = false

# Ollama server connection settings.
# Change these if your Ollama server is not running on the default localhost.
# The host can be an IP address (e.g., "192.168.1.100") or a URL (e.g., "http://192.168.1.100").
# Default: "127.0.0.1"
ollama_host = "127.0.0.1"
# Default: 11434
ollama_port = 11434

# -----------------------------------------------------------------------------
# PER-SUBCOMMAND MODEL OVERRIDES (Optional)
# -----------------------------------------------------------------------------
# You can use different models for different subcommands.
# This allows you to use lightweight models for simple tasks and 
# powerful models for complex ones, optimizing for speed and cost.
#
# Priority for thinking mode:
# 1. Model capability (can't enable if model doesn't support it)
# 2. Subcommand-specific setting (e.g., model.query.thinking)
# 3. Global setting (model.thinking)
# 4. Model default (from models.toml or built-in config)
# 5. Subcommand hardcoded default (true for query, false for others)

# --- QUERY SUBCOMMAND ---
[model.query]
# The model to use for 'ask query' or 'ask q'.
# If not specified, falls back to the global [model] default.
# model = "llama3.1"

# Enable thinking mode for queries. Some models show their reasoning process.
# If not specified, defaults to: true for query
# thinking = true

# Enable tool calling for queries (weather, file operations, etc.).
# If not specified, defaults to: true for query
# tools = true

# --- CHAT SUBCOMMAND ---
[model.chat]
# The model to use for 'ask chat'.
# If not specified, falls back to the global [model] default.
# model = "llama3.1"

# Enable thinking mode for chat. Some models show their reasoning process.
# If not specified, defaults to: false for chat
# thinking = false

# Enable tool calling for chat (weather, file operations, etc.).
# If not specified, defaults to: true for chat
# tools = true

# --- SUMMARIZE SUBCOMMAND ---
[model.summarize]
# The model to use for 'ask summarize'.
# Recommended: a lightweight model like qwen3 for speed and thinking.
# If not specified, falls back to the global [model] default.
# model = "qwen3"

# Summarization typically doesn't need thinking mode.
# If not specified, defaults to: false for summarize
# thinking = false

# Summarization doesn't use external tools.
# If not specified, defaults to: false for summarize
# tools = false

# --- TRANSLATE SUBCOMMAND ---
[model.translate]
# The model to use for 'ask translate'.
# Built-in: "translategemma" (optimized for translation)
# If not specified, uses "translategemma" by default.
# model = "translategemma"

# Translation typically doesn't need thinking mode.
# If not specified, defaults to: false for translate
# thinking = false

# Translation doesn't use tools.
# If not specified, defaults to: false for translate
# tools = false

# --- CODE MODE ---
[model.code]
# The model to use when the code flag (-c) is active.
# Recommended: a code-optimized model like deepseek-coder-v2 or qwen3-coder.
# If not specified, falls back to the global [model] default.
# model = "deepseek-coder-v2"

# Code generation typically doesn't need thinking mode.
# If not specified, defaults to: false for code
# thinking = false

# Enable tool calling in code mode. This allows the model to inspect 
# your project files (read_file, list_directory, search_files) before 
# generating code, leading to more accurate suggestions.
# If not specified, defaults to: true for code
# tools = true

# =============================================================================
# TOOLS CONFIGURATION
# =============================================================================
# Control which AI tools are available and how they behave.

[tools]

# A list of tools to disable (blacklist).
# Blacklisted tools won't be available to the AI, saving context window space.
#
# Available tools include:
#   - get_current_datetime, get_project_context (System information)
#   - get_weather, get_current_weather, get_weather_forecast (Weather)
#   - read_file, list_directory, search_files (File operations)
#   - fetch_pokemon, fetch_pokemon_stats, etc. (Pokémon data)
#   - serper_search, serper_search_news (Serper API web search - requires SERPER_API_KEY)
#   - web_search, web_search_news, web_instant_answer (DuckDuckGo - may fail due to CAPTCHA)
#
# Note: DuckDuckGo tools may be blocked by CAPTCHA. Use Serper tools for reliable web search.
# Default: [] (all tools enabled)
blacklist = []

# Enable file operation sandboxing for security.
# When true, file tools (read_file, list_directory, search_files) can only 
# access files within the current working directory and its subdirectories.
# This prevents the AI from accessing sensitive system files.
# WARNING: Disable only if you fully trust the AI and understand the risks.
# Default: true
file_sandbox = true

# =============================================================================
# OUTPUT CONFIGURATION
# =============================================================================
# Control how responses are displayed.

[output]

# Use plain text output by default, disabling markdown rendering.
# If true, responses will be plain text instead of formatted markdown.
# Default: false
plain_default = false

# Enable debug output by default.
# If true, shows detailed logs including tool calls, model parameters,
# and raw responses. Useful for troubleshooting.
# Default: false
debug_default = false

# =============================================================================
# DISPLAY CONFIGURATION
# =============================================================================
# Customize the terminal appearance.

[display]

# The color theme for markdown rendering.
# Options: "dark", "light", or "mono" (for terminals without color)
# Default: "dark"
skin = "dark"

# =============================================================================
# LED CONTROL CONFIGURATION (Optional)
# =============================================================================
# Control NeoPixel LED strips via Raspberry Pi Pico W HTTP server.
# LED tools are disabled by default and require configuration to activate.
# 
# Build with LED tools: cargo build --release --features led-tools

[led]
# IP address of your Raspberry Pi Pico W LED server.
# Required to enable LED tools. If not set, LED tools are disabled.
# ip = "192.168.1.100"

# HTTP port for the LED server.
# Default: 80
# port = 80
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
        assert_eq!(settings.model.default, "llama3.1");
        assert_eq!(settings.model.ollama_host, "127.0.0.1");
        assert_eq!(settings.model.ollama_port, 11434);
        assert!(settings.tools.file_sandbox);
        assert_eq!(settings.display.skin, "dark");
        // These should be false by default
        assert!(!settings.output.plain_default);
        assert!(!settings.output.debug_default);
        // Translate model defaults to None (uses builtin "translategemma")
        assert!(settings.model.translate.model.is_none());
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
default = "qwen3-coder"
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
        assert_eq!(settings.model.default, "qwen3-coder");
        assert_eq!(settings.model.ollama_host, "192.168.1.100");
        assert_eq!(settings.model.ollama_port, 8080);
        assert!(settings.is_tool_blacklisted("web_search"));
        assert!(!settings.tools.file_sandbox);
        assert!(settings.output.plain_default);
        assert!(settings.output.debug_default);
        assert_eq!(settings.display.skin, "light");
    }

    #[test]
    fn test_translate_model_override() {
        let sample = r#"
[model.translate]
model = "qwen3"
"#;

        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.model.translate.model, Some("qwen3".to_string()));
    }

    #[test]
    fn test_translate_model_default() {
        let settings = Settings::default();
        // Translate defaults to None, code should use "translategemma" as fallback
        assert!(settings.model.translate.model.is_none());
        assert!(settings.model.translate.thinking.is_none());
        assert!(settings.model.translate.tools.is_none());
    }
}
