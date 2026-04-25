use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Default model name when not specified in config
pub const DEFAULT_MODEL: &str = "qwen3.5:4b";

/// Default model for code mode (optimized for coding with tools)
pub const DEFAULT_CODE_MODEL: &str = "qwen2.5-coder:7b";

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
    /// Feedback system configuration
    #[serde(default)]
    pub feedback: FeedbackSettings,
    /// Factual memory auto-extraction configuration
    #[serde(default)]
    pub facts: FactSettings,
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
    /// OCR subcommand configuration
    #[serde(default)]
    pub ocr: SubcommandModelConfig,
    /// Document subcommand configuration
    #[serde(default)]
    pub document: SubcommandModelConfig,
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
}

impl Default for ToolSettings {
    fn default() -> Self {
        ToolSettings {
            blacklist: default_blacklist(),
        }
    }
}

/// Output-related settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputSettings {
    /// Use plain output by default
    #[serde(default)]
    pub plain_default: bool,
    /// Verbosity level for logging: "quiet", "normal", "verbose", or "trace"
    /// Default: "normal" (info level — shows tool calls)
    /// Priority: CLI flags (-v/-q) > RUST_LOG env var > this setting > default
    #[serde(default)]
    pub verbosity: Option<crate::logging::Verbosity>,
    /// Deprecated: use `verbosity` instead.
    /// This field exists solely for backwards compatibility with old config files.
    /// It is ignored — `verbosity` takes precedence.
    #[serde(default, rename = "debug_default")]
    pub debug_default: Option<bool>,
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

/// Feedback system settings for managing how user feedback affects memory scoring.
/// Controls RRF boost, LLM feedback weight, Ebbinghaus decay, content aging,
/// access reinforcement, and content pruning.
/// See ADR-004 (LLM feedback weight), ADR-008 (content decay), ADR-009 (access reinforcement).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackSettings {
    /// Whether the feedback system is enabled.
    /// When enabled, RRF boost and LLM feedback tools are active.
    /// This does NOT gate the `/feedback` command (that always works).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether implicit (non-explicit) feedback signals are captured.
    /// Reserved for Phase 2 — currently stored but not used in scoring.
    #[serde(default = "default_true")]
    pub implicit_capture: bool,

    /// Weight of LLM-provided feedback relative to explicit user feedback.
    /// See ADR-004. Range: 0.0–1.0.
    #[serde(default = "default_llm_feedback_weight")]
    pub llm_feedback_weight: f32,

    /// Half-life (in days) for decay of positively-rated content.
    /// Higher = good memories decay slower.
    #[serde(default = "default_decay_half_life_good")]
    pub decay_half_life_good: f32,

    /// Half-life (in days) for decay of negatively-rated content.
    /// Lower = bad memories decay faster.
    #[serde(default = "default_decay_half_life_bad")]
    pub decay_half_life_bad: f32,

    /// Half-life (in days) for decay of corrections.
    /// Between good and bad — corrections age at a moderate rate.
    #[serde(default = "default_decay_half_life_correction")]
    pub decay_half_life_correction: f32,

    /// Whether to apply time-based decay to content relevance scores.
    /// See ADR-008.
    #[serde(default = "default_true")]
    pub content_decay: bool,

    /// Whether to apply a small reinforcement boost each time content is accessed.
    /// See ADR-009.
    #[serde(default = "default_true")]
    pub access_reinforcement: bool,

    /// Per-access reinforcement boost amount.
    /// Applied each time content is retrieved, not per 10 accesses.
    #[serde(default = "default_access_reinforcement_boost")]
    pub access_reinforcement_boost: f32,

    /// Threshold below which content is pruned from the knowledge base.
    /// Content with a score below this value may be removed during maintenance.
    #[serde(default = "default_content_prune_threshold")]
    pub content_prune_threshold: f32,
}
impl Default for FeedbackSettings {
    fn default() -> Self {
        FeedbackSettings {
            enabled: true,
            implicit_capture: true,
            llm_feedback_weight: 0.3,
            decay_half_life_good: 30.0,
            decay_half_life_bad: 7.0,
            decay_half_life_correction: 14.0,
            content_decay: true,
            access_reinforcement: true,
            access_reinforcement_boost: 0.001,
            content_prune_threshold: 0.05,
        }
    }
}

/// Fact auto-extraction settings (autoDream-lite).
/// Controls heuristic extraction of preferences and identity facts from user messages.
/// See ADR-E1 (heuristic-only), ADR-E2 (always Global), ADR-E5 (synchronous).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactSettings {
    /// Whether auto-extraction of facts from user messages is enabled.
    /// When enabled, the system scans recent user messages for preference and
    /// identity patterns after each response and inserts discovered facts.
    #[serde(default = "default_true")]
    pub auto_extract: bool,

    /// Maximum number of facts to extract per response.
    /// Limits noise from over-extraction. Default: 3.
    #[serde(default = "default_max_facts")]
    pub max_facts: u32,

    /// Whether to show a notification when facts are auto-extracted.
    /// Displays `[Auto-extracted: N fact(s)]` in gray after token metrics.
    /// Suppressed in Quiet mode regardless of this setting.
    #[serde(default = "default_true")]
    pub auto_extract_notify: bool,
}

impl Default for FactSettings {
    fn default() -> Self {
        FactSettings {
            auto_extract: true,
            max_facts: 3,
            auto_extract_notify: true,
        }
    }
}

fn default_max_facts() -> u32 {
    3
}

fn default_led_port() -> u16 {
    80
}

fn default_true() -> bool {
    true
}

fn default_llm_feedback_weight() -> f32 {
    0.3
}

fn default_decay_half_life_good() -> f32 {
    30.0
}

fn default_decay_half_life_bad() -> f32 {
    7.0
}

fn default_decay_half_life_correction() -> f32 {
    14.0
}

fn default_access_reinforcement_boost() -> f32 {
    0.001
}

fn default_content_prune_threshold() -> f32 {
    0.05
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
            ocr: SubcommandModelConfig::default(),
            document: SubcommandModelConfig::default(),
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
        self.led
            .ip
            .as_ref()
            .map(|ip| format!("http://{}:{}", ip, self.led.port))
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
            "translate" => &self.model.translate,
            "ocr" => &self.model.ocr,
            "document" => &self.model.document,
            "vision" => &self.model.vision,
            "code" => &self.model.code,
            _ => &SubcommandModelConfig::default(),
        };

        // Get model: subcommand specific > code default > global default
        // Get model: subcommand specific -> code default -> global default
        let model = subcommand_config
            .model
            .as_ref()
            .cloned()
            .unwrap_or_else(|| {
                // Code subcommand has its own default model
                if subcommand == "code" {
                    DEFAULT_CODE_MODEL.to_string()
                } else if subcommand == "translate" {
                    "translategemma".to_string()
                } else if subcommand == "ocr" {
                    "glm-ocr".to_string()
                } else {
                    self.model.default.clone()
                }
            });
        // Get thinking: subcommand specific -> global -> model default
        // Note: This returns the config preference; model capability check happens elsewhere
        let thinking = subcommand_config
            .thinking
            .or(self.model.thinking)
            .unwrap_or({
                // Fall back to subcommand-specific defaults
                match subcommand {
                    "query" => true, // Query benefits from thinking by default
                    _ => false,      // Chat, summarize, code, vision default to no thinking
                }
            });

        // Get tools: subcommand specific -> default by subcommand
        let default_tools = match subcommand {
            "query" => true,
            "chat" => true,
            "code" => true,
            "translate" => false,
            "vision" => false,
            "ocr" => false,
            "document" => true,
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
# Recommended: "qwen3.5:4b" (built-in, multimodal) or "ministral" (from models.toml)
# Default: "qwen3.5:4b"
default = "qwen3.5:4b"

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
# model = "qwen3.5:4b"

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
# model = "qwen3.5:4b"

# Enable thinking mode for chat. Some models show their reasoning process.
# If not specified, defaults to: false for chat
# thinking = false

# Enable tool calling for chat (weather, file operations, etc.).
# If not specified, defaults to: true for chat
# tools = true

# --- SUMMARIZE SUBCOMMAND ---
# tools = false

# --- OCR SUBCOMMAND ---
[model.ocr]
# The model to use for 'ask ocr'.
# Built-in: "glm-ocr:bf16" (optimized for OCR tasks)
# If not specified, uses "glm-ocr:bf16" by default.
# model = "glm-ocr:bf16"

# OCR typically doesn't need thinking mode.
# If not specified, defaults to: false for ocr
# thinking = false

# OCR doesn't use external tools.
# If not specified, defaults to: false for ocr
# tools = false

# --- DOCUMENT SUBCOMMAND ---
[model.document]
# The model to use for 'ask document'.
# If not specified, falls back to the global [model] default.
# model = "qwen3.5:4b"

# Document operations typically don't need thinking mode.
# If not specified, defaults to: false for document
# thinking = false

# Enable tool calling for document operations. This allows the model to inspect
# your project files (read_file, list_directory, search_files) before
# performing document operations.
# If not specified, defaults to: true for document
# tools = true

# --- CODE MODE ---

# --- CODE MODE ---
[model.code]
# The model to use when the code flag (-c) is active.
# Default: "qwen2.5-coder:7b" (optimized for coding with function calling)
# If not specified, falls back to the code default (qwen2.5-coder:7b).
# model = "qwen2.5-coder:7b"

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

# =============================================================================
# OUTPUT CONFIGURATION
# =============================================================================
# Control how responses are displayed.

[output]

# Use plain text output by default, disabling markdown rendering.
# If true, responses will be plain text instead of formatted markdown.
# Default: false
plain_default = false

# Verbosity level for log output.
# Controls how much diagnostic information is shown alongside the LLM response.
#
# Options:
#   "quiet"   — Errors only. No spinner, no tool calls. Ideal for scripting/pipes.
#   "normal"  — Tool calls (compact), warnings, errors. Good default for interactive use.
#   "verbose" — Detailed tool calls with full parameters and results. For debugging.
#   "trace"   — Everything including embedding internals, token budgets. Maximum info.
#
# Priority: CLI flags (-v/-q) > RUST_LOG env var > this setting > default
# Default: "normal" (info level)
# verbosity = "normal"

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

# =============================================================================
# FEEDBACK CONFIGURATION (Optional)
# =============================================================================
# Control how user and LLM feedback affects memory scoring.
# These settings govern Ebbinghaus decay, access reinforcement,
# content pruning, and LLM feedback weight.
# See ADR-004 (LLM feedback weight), ADR-008 (content decay), ADR-009 (access reinforcement).

# [feedback]

# Whether the feedback system is enabled.
# When enabled, RRF boost and LLM feedback tools are active.
# This does NOT gate the /feedback command (that always works).
# Default: true
# enabled = true

# Whether implicit (non-explicit) feedback signals are captured.
# Reserved for Phase 2 — currently stored but not used in scoring.
# Default: true
# implicit_capture = true

# Weight of LLM-provided feedback relative to explicit user feedback.
# See ADR-004. Range: 0.0–1.0.
# Default: 0.3
# llm_feedback_weight = 0.3

# Half-life (in days) for decay of positively-rated content.
# Higher = good memories decay slower.
# Default: 30
# decay_half_life_good = 30

# Half-life (in days) for decay of negatively-rated content.
# Lower = bad memories decay faster.
# Default: 7
# decay_half_life_bad = 7

# Half-life (in days) for decay of corrections.
# Between good and bad — corrections age at a moderate rate.
# Default: 14
# decay_half_life_correction = 14

# Whether to apply time-based decay to content relevance scores.
# See ADR-008.
# Default: true
# content_decay = true

# Whether to apply a small reinforcement boost each time content is accessed.
# See ADR-009.
# Default: true
# access_reinforcement = true

# Per-access reinforcement boost amount.
# Applied each time content is retrieved, not per 10 accesses.
# Default: 0.001
# access_reinforcement_boost = 0.001

# Threshold below which content is pruned from the knowledge base.
# Content with a score below this value may be removed during maintenance.
# Default: 0.05
# content_prune_threshold = 0.05

# =============================================================================
# FACT AUTO-EXTRACTION CONFIGURATION (Optional)
# =============================================================================
# Control how facts are automatically extracted from user messages.
# When enabled, the system scans recent user messages after each response
# and extracts preferences and identity facts using heuristic patterns.
# Extracted facts are deduplicated against existing facts via FTS5 search.
# See P6.1 (autoDream-lite) for design details.

# [facts]

# Whether auto-extraction of facts from user messages is enabled.
# When enabled, the system extracts facts like "I prefer dark mode" or
# "My name is Lucas" after each response and stores them as facts.
# Default: true
# auto_extract = true

# Maximum number of facts to extract per response.
# Limits noise from over-extraction. Increase for longer conversations.
# Default: 3
# max_facts = 3

# Whether to show a notification when facts are auto-extracted.
# Displays "[Auto-extracted: N fact(s)]" in gray after token metrics.
# Suppressed in Quiet mode regardless of this setting.
# Default: true
# auto_extract_notify = true
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
        assert_eq!(settings.model.default, "qwen3.5:4b");
        assert_eq!(settings.model.ollama_host, "127.0.0.1");
        assert_eq!(settings.model.ollama_port, 11434);
        assert_eq!(settings.display.skin, "dark");
        // These should be false by default
        assert!(!settings.output.plain_default);
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

[output]
plain_default = true

[display]
skin = "light"
"#;

        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.model.default, "qwen3-coder");
        assert_eq!(settings.model.ollama_host, "192.168.1.100");
        assert_eq!(settings.model.ollama_port, 8080);
        assert!(settings.is_tool_blacklisted("web_search"));
        assert!(settings.output.plain_default);
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

    #[test]
    fn test_ocr_model_default() {
        let settings = Settings::default();
        // OCR defaults to None, code should use "glm-ocr:bf16" as fallback
        assert!(settings.model.ocr.model.is_none());
        assert!(settings.model.ocr.thinking.is_none());
        assert!(settings.model.ocr.tools.is_none());
    }

    #[test]
    fn test_ocr_model_override() {
        let sample = r#"
[model.ocr]
model = "custom-ocr:latest"
thinking = true
tools = false
"#;

        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(
            settings.model.ocr.model,
            Some("custom-ocr:latest".to_string())
        );
        assert_eq!(settings.model.ocr.thinking, Some(true));
        assert_eq!(settings.model.ocr.tools, Some(false));
    }

    #[test]
    fn test_vision_model_default() {
        let settings = Settings::default();
        // Vision defaults to None (uses global default from subcommand config)
        assert!(settings.model.vision.model.is_none());
        assert!(settings.model.vision.thinking.is_none());
        assert!(settings.model.vision.tools.is_none());
    }

    #[test]
    fn test_summarize_model_default() {
        let settings = Settings::default();
        // Summarize defaults to None (uses global default from subcommand config)
        assert!(settings.model.summarize.model.is_none());
        assert!(settings.model.summarize.thinking.is_none());
        assert!(settings.model.summarize.tools.is_none());
    }

    #[test]
    fn test_document_model_default() {
        let settings = Settings::default();
        // Document defaults to None (uses global default from subcommand config)
        assert!(settings.model.document.model.is_none());
        assert!(settings.model.document.thinking.is_none());
        assert!(settings.model.document.tools.is_none());
    }

    #[test]
    fn test_get_subcommand_config_translate_default_model() {
        let settings = Settings::default();
        let (model, thinking, tools) = settings.get_subcommand_config("translate");
        // Default translate model is config key "translategemma" (resolved to model_id by SubagentConfig)
        assert_eq!(model, "translategemma");
        // Translate defaults to no thinking
        assert!(!thinking);
        // Translate defaults to no tools
        assert!(!tools);
    }

    #[test]
    fn test_get_subcommand_config_ocr_default_model() {
        let settings = Settings::default();
        let (model, thinking, tools) = settings.get_subcommand_config("ocr");
        // Default OCR model is config key "glm-ocr" (resolved to model_id by SubagentConfig)
        assert_eq!(model, "glm-ocr");
        // OCR defaults to no thinking
        assert!(!thinking);
        // OCR defaults to no tools
        assert!(!tools);
    }

    #[test]
    fn test_get_subcommand_config_translate_model_resolution() {
        // Verify that config key "translategemma" resolves to model_id via get_model_config
        use crate::user_models::get_model_config;
        let config = get_model_config("translategemma");
        assert!(
            config.is_some(),
            "translategemma should resolve via config key"
        );
        let config = config.unwrap();
        assert_eq!(config.model_id, "translategemma:4b");
        // The builtin translategemma has temperature 0.2
        assert_eq!(config.temperature, 0.2);
    }

    #[test]
    fn test_get_subcommand_config_ocr_model_resolution() {
        // Verify that config key "glm-ocr" resolves to model_id via get_model_config
        use crate::user_models::get_model_config;
        let config = get_model_config("glm-ocr");
        assert!(config.is_some(), "glm-ocr should resolve via config key");
        let config = config.unwrap();
        assert_eq!(config.model_id, "glm-ocr:bf16");
        // The builtin glm-ocr has temperature 0.1
        assert_eq!(config.temperature, 0.1);
    }

    #[test]
    fn test_feedback_settings_defaults() {
        let settings = Settings::default();
        assert!(settings.feedback.enabled);
        assert!(settings.feedback.implicit_capture);
        assert!((settings.feedback.llm_feedback_weight - 0.3).abs() < f32::EPSILON);
        assert!((settings.feedback.decay_half_life_good - 30.0).abs() < f32::EPSILON);
        assert!((settings.feedback.decay_half_life_bad - 7.0).abs() < f32::EPSILON);
        assert!((settings.feedback.decay_half_life_correction - 14.0).abs() < f32::EPSILON);
        assert!(settings.feedback.content_decay);
        assert!(settings.feedback.access_reinforcement);
        assert!((settings.feedback.access_reinforcement_boost - 0.001).abs() < f32::EPSILON);
        assert!((settings.feedback.content_prune_threshold - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn test_feedback_settings_parse_defaults() {
        // Empty config should yield all defaults
        let settings: Settings = toml::from_str("").unwrap();
        assert!(settings.feedback.enabled);
        assert!((settings.feedback.llm_feedback_weight - 0.3).abs() < f32::EPSILON);
        assert!((settings.feedback.access_reinforcement_boost - 0.001).abs() < f32::EPSILON);
    }

    #[test]
    fn test_feedback_settings_parse_overrides() {
        let sample = r#"
[feedback]
enabled = false
llm_feedback_weight = 0.5
decay_half_life_good = 60
decay_half_life_bad = 3
content_prune_threshold = 0.1
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert!(!settings.feedback.enabled);
        assert!((settings.feedback.llm_feedback_weight - 0.5).abs() < f32::EPSILON);
        assert!((settings.feedback.decay_half_life_good - 60.0).abs() < f32::EPSILON);
        assert!((settings.feedback.decay_half_life_bad - 3.0).abs() < f32::EPSILON);
        // Not overridden fields should keep defaults
        assert!(settings.feedback.implicit_capture);
        assert!((settings.feedback.decay_half_life_correction - 14.0).abs() < f32::EPSILON);
        assert!((settings.feedback.content_prune_threshold - 0.1).abs() < f32::EPSILON);
    }
}
