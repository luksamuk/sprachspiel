//! Settings module — application configuration from `~/.config/sprachspiel/config.toml`.
//!
//! Provides [`Settings`] and its sub-structs for configuring models, tools, output,
//! display, LED, feedback, fact auto-extraction, and retrieval. Settings are loaded from TOML
//! at startup via [`Settings::load`], with sensible defaults when no config file exists.
//!
//! # Configuration hierarchy
//!
//! 1. CLI flags (highest priority)
//! 2. `config.toml` per-subcommand settings (e.g., `[model.chat]`)
//! 3. `config.toml` global settings
//! 4. Built-in defaults ([`DEFAULT_MODEL`], [`DEFAULT_CODE_MODEL`])
//!
//! # Key structs
//!
//! - [`ModelSettings`] — model, per-subcommand overrides
//! - [`ToolSettings`] — tool blacklist
//! - [`FeedbackSettings`] — RRF boost, LLM feedback weight, decay, reinforcement
//! - [`FactSettings`] — auto-extraction toggle, max facts per response, notification

#![expect(clippy::print_stderr)] // Settings output
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Default model name when not specified in config
pub const DEFAULT_MODEL: &str = "qwen3.5:4b";

/// Default model for code mode (optimized for coding with tools)
pub const DEFAULT_CODE_MODEL: &str = "qwen2.5-coder:7b";

/// Default semantic search threshold for fact deduplication (cosine similarity).
///
/// Used by Layer 3.5 in the dedup pipeline. Intentionally broad (0.70) to catch
/// contradictions that normalized match and FTS5 miss.
/// Separate from `SEMANTIC_DEDUP_THRESHOLD = 0.90` in verify.rs (startup pairwise dedup).
pub const DEFAULT_SEMANTIC_THRESHOLD: f32 = 0.70;

/// Default keyword weight for hybrid RRF retrieval (BM25).
pub const DEFAULT_KEYWORD_WEIGHT: f32 = 0.4;

/// Default semantic weight for hybrid RRF retrieval (vector similarity).
pub const DEFAULT_SEMANTIC_WEIGHT: f32 = 0.6;

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
    /// Thinking Trace Transform configuration
    #[serde(default)]
    pub thinking_trace: ThinkingTraceSettings,
    /// Indexing configuration.
    ///
    /// The `[indexing]` section in `config.toml` is **required** in
    /// production. It declares the alias (from `models.toml
    /// [models.*]`) used to generate vector embeddings, AND the
    /// hybrid RRF weights for retrieval. The two are merged into a
    /// single section because indexing and retrieval are two sides
    /// of the same concern.
    ///
    /// `#[serde(default)]` is used so the Settings type is still
    /// constructable for tests; the real validation happens in
    /// `init_chat_database` (which bails out on empty `model` or
    /// when the alias is missing `embeddings = true` in models.toml).
    ///
    /// See [IndexingSettings] for the full schema.
    #[serde(default)]
    pub indexing: IndexingSettings,
}

/// Model-related settings with per-subcommand configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Default model preset name (used by query subcommand if not specified)
    #[serde(default = "default_model")]
    pub default: String,
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
    /// Whether to show 🔧 tool call indicators during chat (compact format).
    /// Default: true. Set to false to hide tool call display entirely.
    /// Quiet mode (-q) overrides this to false regardless.
    #[serde(default = "default_true")]
    pub show_tool_calls: bool,
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

    /// Threshold for semantic search via embeddings (cosine similarity).
    /// Used for fact deduplication and conflict detection.
    /// Range: 0.0–1.0. Default: 0.70.
    /// Higher values (0.80+) are stricter — fewer false matches,
    /// but may miss genuine contradictions.
    /// Lower values (0.60-) are more permissive — catches more
    /// contradictions but risks false positives.
    #[serde(default = "default_semantic_threshold")]
    pub semantic_threshold: f32,
}

impl Default for FactSettings {
    fn default() -> Self {
        FactSettings {
            auto_extract: true,
            max_facts: 3,
            auto_extract_notify: true,
            semantic_threshold: DEFAULT_SEMANTIC_THRESHOLD,
        }
    }
}

fn default_max_facts() -> u32 {
    3
}

fn default_semantic_threshold() -> f32 {
    DEFAULT_SEMANTIC_THRESHOLD
}

// The keyword_weight and semantic_weight fields live in
// IndexingSettings (the [indexing] section), since indexing and
// retrieval are two sides of the same concern. Use the
// `default_keyword_weight()` / `default_semantic_weight()` helpers
// for the default values.

fn default_keyword_weight() -> f32 {
    DEFAULT_KEYWORD_WEIGHT
}

fn default_semantic_weight() -> f32 {
    DEFAULT_SEMANTIC_WEIGHT
}

fn default_embedding_prefix() -> String {
    "search_document: ".to_string()
}

/// Thinking Trace Transform configuration.
///
/// Controls whether reasoning traces from LLM responses are included in
/// the LLM context for improved reasoning (Arabzadeh et al. 2026, arXiv:2605.03344).
/// Thinking content is always preserved in the database regardless of this setting —
/// the flag only controls whether it's injected into the conversation context.
///
/// When enabled, assistant messages with thinking content will include their
/// reasoning traces in the `ChatMessage.thinking` field sent to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThinkingTraceSettings {
    /// Enable thinking trace injection into LLM context.
    /// Default: false — thinking is preserved in DB but not injected into context.
    /// When true, thinking traces are included in retrieval context for reasoning tasks.
    #[serde(default)]
    pub enabled: bool,
}

/// Indexing configuration.
///
/// The `[indexing]` section in `config.toml` merges two concerns:
/// (1) which model to use for vector embedding generation, and
/// (2) the hybrid RRF weights for retrieval. They live together
/// because indexing and retrieval are two sides of the same
/// concern (storage and lookup of vector embeddings).
///
/// # Schema
///
/// ```toml
/// [indexing]
/// model = "nomic"              # required; ALIAS from models.toml [models.*]
/// # The alias MUST be declared with `embeddings = true` in
/// # models.toml and have `dimensions = N` set. The provider is
/// # inferred from the alias's `provider = "..."` field.
/// probe = true                 # optional; default true
/// keyword_weight = 0.4         # optional; default 0.4 (moved from [retrieval])
/// semantic_weight = 0.6        # optional; default 0.6 (moved from [retrieval])
/// ```
///
/// # Model resolution
///
/// `model` is an ALIAS (the map key in `models.toml [models.*]`),
/// NOT the upstream `model_id`. The provider is INFERRED from the
/// alias — the user does NOT specify a provider in `[indexing]`.
/// Sprach bails out if:
/// - the alias doesn't exist in `models.toml`
/// - the alias exists but doesn't have `embeddings = true`
/// - the alias has `embeddings = true` but no `dimensions`
///
/// # Probe (opt-OUT, default ON)
///
/// When `probe = true` (default), sprach makes one POST
/// `/v1/embeddings` call at startup to verify the provider actually
/// serves the model. The probe does NOT pass `dimensions` in the
/// request body (adaptive — some providers reject it); the
/// response's vector dim count is compared against the alias's
/// declared `dimensions`. Mismatch is a fatal error.
///
/// Set `probe = false` to skip the probe and trust the configuration
/// strictly. Useful for cold-start scenarios where the model takes
/// 30-60s to load and the probe would time out before the model is
/// ready.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingSettings {
    /// Embedding model **alias** (the key in
    /// `models.toml [models.*]`). Required.
    ///
    /// The alias must be declared with `embeddings = true` and a
    /// `dimensions` value in `models.toml`. Sprach uses the alias
    /// to look up the upstream `model_id` and the provider.
    pub model: String,

    /// Whether to make a probe `/v1/embeddings` call at startup.
    /// Default: `true`. Set to `false` for cold-start scenarios
    /// where the model takes time to load.
    #[serde(default = "default_true")]
    pub probe: bool,

    /// Weight for keyword (BM25) search in hybrid RRF. Range: 0.0–1.0.
    /// Higher = more weight on keyword matches.
    /// Default: 0.4 (semantic is weighted higher because embeddings
    /// capture meaning). Moved from the old `[retrieval]` section.
    #[serde(default = "default_keyword_weight")]
    pub keyword_weight: f32,

    /// Weight for semantic (vector) search in hybrid RRF. Range: 0.0–1.0.
    /// Higher = more weight on semantic similarity.
    /// Default: 0.6 (embeddings capture meaning better than keyword
    /// overlap). Moved from the old `[retrieval]` section.
    #[serde(default = "default_semantic_weight")]
    pub semantic_weight: f32,

    /// Prefix prepended to each text before embedding (e.g.
    /// `"search_document: "` for nomic-embed-text models). Some
    /// models (BGE, GTE, mxbai) do not require a prefix — set to
    /// `""` for those. Default: `"search_document: "` (matches
    /// nomic-embed-text-v2-moe, the current default model).
    #[serde(default = "default_embedding_prefix")]
    pub prefix: String,
}

impl Default for IndexingSettings {
    /// Default for tests and `Settings::default()`: a sensible alias
    /// name (the literal alias the user must declare in
    /// `models.toml`). Real configuration MUST come from
    /// `config.toml`'s `[indexing]` section — sprach refuses to start
    /// without it.
    fn default() -> Self {
        Self {
            model: "nomic".to_string(),
            probe: true,
            keyword_weight: DEFAULT_KEYWORD_WEIGHT,
            semantic_weight: DEFAULT_SEMANTIC_WEIGHT,
            prefix: default_embedding_prefix(),
        }
    }
}

// Embedding settings live in IndexingSettings (defined above).

/// The complete sample configuration exposed as a static string.
///
/// This is the single source of truth for the `config.toml` format. The
/// `sprach config upgrade` subcommand parses this string to extract
/// doc-comments for new fields, so they always stay in sync with
/// `Settings::default()`. When adding a new field to any `Settings`
/// sub-struct, also add a documented entry here so that `config upgrade`
/// can insert it with proper documentation.
pub const SAMPLE_CONFIG: &str = r#"# Sprachspiel Configuration File
# Location: ~/.config/sprachspiel/config.toml
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
#
# NOTE: LLM provider configuration (host, port, timeouts) has moved to models.toml
# under the [provider] section. See ~/.config/sprachspiel/models.toml for the new format.

[model]

# The default model preset to use for general queries.
# See all available models with: sprach --list-models
# Recommended: "qwen3.5:4b" (built-in, multimodal) or "ministral" (from models.toml)
# Default: "qwen3.5:4b"
default = "qwen3.5:4b"

# Global default for thinking mode.
# This is used as a fallback for all subcommands that don't have their own setting.
# Subcommand-specific settings (model.query.thinking, model.chat.thinking, etc.) override this.
# Model capability takes precedence: if the model doesn't support thinking, this is ignored.
# If not specified, subcommand defaults are used (true for query, false for others).
# thinking = false

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
# The model to use for 'sprach query' or 'sprach q'.
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
# The model to use for 'sprach chat'.
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
# The model to use for 'sprach ocr'.
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
# The model to use for 'sprach document'.
# If not specified, falls back to the global [model] default.
# model = "qwen3.5:4b"

# Document operations typically don't need thinking mode.
# If not specified, defaults to: false for document
# thinking = false

# Enable tool calling for document operations. This allows the model to inspect
# your project files (read_file, list_directory) before
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
# your project files (read_file, list_directory) before
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
#   - read_file, list_directory (File operations)
#   - fetch_pokemon, fetch_pokemon_stats, etc. (Pokémon data)
#   - web_search, web_search_news, web_scrape (DuckDuckGo - may fail due to CAPTCHA)
#
# Note: DuckDuckGo tools may be blocked by CAPTCHA. MCP-based search is
# planned post-W2 (issue TBD); until then, this is the only web search option.
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

# Whether to show 🔧 tool call indicators during chat.
# When enabled, each tool call is displayed as a compact single line:
#   🔧 read_file(path=/tmp/test.txt)
# This is independent of the verbosity setting — tool calls are shown
# even in Normal mode. Set to false to hide tool call display entirely.
# Quiet mode (-q) overrides this to false regardless.
# Default: true
# show_tool_calls = true

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

# Semantic similarity threshold for fact deduplication (0.0–1.0).
# Cosine similarity at or above this value triggers semantic conflict detection.
# Higher = stricter matching (fewer false matches, may miss contradictions).
# Lower = more permissive (catches more contradictions, risks false positives).
# The default 0.70 is designed for nomic-embed-text-v2-moe at 256 dimensions.
# Adjust based on "sprach diagnostics" threshold recommendations.
# Default: 0.70
# semantic_threshold = 0.70

# The [retrieval] section was removed. The keyword_weight and
# semantic_weight fields live in the [indexing] section (above).
# Indexing and retrieval are two sides of the same concern.

# =============================================================================
# THINKING TRACE TRANSFORM CONFIGURATION (Optional)
# =============================================================================
# Control whether reasoning traces from LLM responses are included in context
# for improved reasoning (Arabzadeh et al. 2026, arXiv:2605.03344).
# Thinking content is always preserved in the database — this flag only
# controls whether it's injected into the LLM conversation context.

# [thinking_trace]

# Enable thinking trace injection into LLM context.
# Default: false — thinking is preserved in DB but not included in context.
# When true, assistant messages with thinking content include their reasoning
# traces in the ChatMessage sent to the LLM, improving reasoning performance.
# enabled = false

# =============================================================================
# INDEXING CONFIGURATION (REQUIRED)
# =============================================================================
# Configures the embedding model and the hybrid RRF weights used by
# /search and the indexing pipeline. This section is REQUIRED —
# sprach refuses to start if it is missing or `model` is empty.
#
# The `model` field is an ALIAS from models.toml [models.*], NOT the
# upstream model_id. The alias must be declared with
# `embeddings = true` and `dimensions = N` in models.toml. The
# provider is inferred from the alias's `provider` field — there is
# NO `provider` field in this section.
#
# Indexing and retrieval weights live in the same section because
# they are two sides of the same concern (storage and lookup of
# vector embeddings).

[indexing]

# Embedding model alias (key in models.toml [models.*]). Required.
# The alias must be declared with `embeddings = true` and
# `dimensions = N` in models.toml. The provider is inferred from
# the alias — no provider override here.
# Example: models.toml has
#   [models."nomic"]
#   model_id = "nomic-embed-text-v2-moe"
#   provider = "llama-swap"
#   embeddings = true
#   dimensions = 768
# so the alias is "nomic".
model = "nomic"

# Whether to probe the embedding endpoint at startup.
# When true (default), sprach makes 1 POST /v1/embeddings call to
# verify the provider actually serves the model. The probe does NOT
# pass `dimensions` in the request body (adaptive — some providers
# reject it); the response's vector dim count is compared against
# the alias's declared `dimensions`. Mismatch is a fatal error.
# Set to false for cold-start scenarios where the model takes
# 30-60s to load (the probe uses a 30s timeout).
# Default: true
# probe = true

# Weight for keyword (BM25) search in hybrid RRF. Range: 0.0–1.0.
# Higher = more weight on keyword matches.
# Default: 0.4 (semantic is weighted higher because embeddings
# capture meaning). Moved from the old [retrieval] section.
# keyword_weight = 0.4

# Weight for semantic (vector) search in hybrid RRF. Range: 0.0–1.0.
# Higher = more weight on semantic similarity.
# Default: 0.6 (embeddings capture meaning better than keyword
# overlap). Moved from the old [retrieval] section.
# semantic_weight = 0.6

# Prefix prepended to each text before embedding.
# nomic-embed-text models use "search_document: " (the default).
# Models that do not require a prefix (BGE, GTE, mxbai) should
# set this to "" (empty string).
# Default: "search_document: "
# prefix = "search_document: "
"#;

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
            show_tool_calls: true,
        }
    }
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
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
        use crate::consts::app;

        // Check XDG_CONFIG_HOME first
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg_config)
                .join(app::APP_CONFIG_DIR)
                .join("config.toml");
            if path.exists() {
                return Some(path);
            }
        }

        // Fall back to ~/.config/sprachspiel/config.toml
        if let Some(home_dir) = dirs::home_dir() {
            let path = home_dir
                .join(".config")
                .join(app::APP_CONFIG_DIR)
                .join("config.toml");
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Get the config directory path (for creating new config)
    pub fn config_dir() -> Option<PathBuf> {
        use crate::consts::app;

        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(xdg_config).join(app::APP_CONFIG_DIR));
        }

        if let Some(home_dir) = dirs::home_dir() {
            return Some(home_dir.join(".config").join(app::APP_CONFIG_DIR));
        }

        None
    }

    /// Get the configured indexing model alias.
    ///
    /// Returns `&self.indexing.model` (the value
    /// from `[indexing].model` in `config.toml`). This is an ALIAS
    /// from `models.toml [models.*]`, NOT the upstream `model_id`.
    /// The alias is resolved via `Settings::resolve_indexing_model`
    /// in a follow-up commit.
    pub fn indexing_model_alias(&self) -> &str {
        &self.indexing.model
    }

    /// Get the configured indexing model upstream `model_id` (after
    /// alias resolution).
    ///
    /// Whether the embedding endpoint should be probed at startup.
    pub fn indexing_probe_enabled(&self) -> bool {
        self.indexing.probe
    }

    // The alias-based resolver `Settings::resolve_indexing_model(alias)`
    // is defined below.

    /// Resolve the indexing configuration for the alias in
    /// `[indexing].model` of `config.toml`.
    ///
    /// Reads the alias (e.g. `"nomic"`) from `settings.indexing.model`,
    /// looks it up in `models.toml [models.*]`, and returns:
    /// - `&UserModelConfig` — the model config (with the upstream
    ///   `model_id`, `provider`, `dimensions`, etc.)
    /// - `&ProviderConfig` — the provider config (resolved via the
    ///   model config's `provider` field)
    /// - `&str` — the upstream `model_id` (the name passed verbatim
    ///   to `/v1/embeddings`)
    /// - `u32` — the `dimensions` value (guaranteed non-zero by the
    ///   models.toml validation)
    ///
    /// Returns `Err` with an actionable message if:
    /// - the alias is empty
    /// - the alias doesn't exist in `models.toml`
    /// - the alias exists but doesn't have `embeddings = true`
    /// - the alias has `embeddings = true` but no `dimensions`
    ///   (this should be caught at load time, but is checked here
    ///   defensively)
    pub fn resolve_indexing_model(
        &self,
    ) -> Result<
        (
            &crate::user_models::UserModelConfig,
            &crate::user_models::ProviderConfig,
            &str,
            u32,
        ),
        String,
    > {
        let alias = self.indexing_model_alias();
        if alias.trim().is_empty() {
            return Err("Error: [indexing].model is empty in config.toml. \
                 Add the alias of an embedding-capable model from \
                 models.toml [models.*], e.g.:\n\
                 \n\
                 [indexing]\n\
                 model = \"nomic\"\n\
                 \n\
                 The alias must be declared with `embeddings = true` \
                 and `dimensions = N` in models.toml."
                .to_string());
        }

        let models = crate::user_models::get_user_models();
        let model_cfg = models.get(alias).ok_or_else(|| {
            format!(
                "Error: Indexing alias '{alias}' not found in models.toml. \
                 Add a [models.\"{alias}\"] block with `embeddings = true` \
                 and `dimensions = N`, or change [indexing].model to an \
                 existing alias.",
            )
        })?;

        if !model_cfg.embeddings {
            return Err(format!(
                "Error: Model '{alias}' is not declared as an embedding model. \
                 Add `embeddings = true` and `dimensions = N` to \
                 [models.\"{alias}\"] in models.toml.",
            ));
        }

        // The models.toml loader guarantees dimensions is set when
        // embeddings = true, but we check defensively in case a
        // caller bypassed the loader.
        let dimensions = model_cfg.dimensions.ok_or_else(|| {
            format!(
                "Error: Embedding model '{alias}' is missing `dimensions` \
                 in models.toml. Add `dimensions = <N>` (e.g. 768 for \
                 nomic-embed-text-v2-moe, 256 for Matryoshka-truncated).",
            )
        })?;

        let providers = crate::user_models::get_providers();
        let provider_cfg = providers.get(&model_cfg.provider).ok_or_else(|| {
            format!(
                "Error: Provider '{}' referenced by embedding model '{alias}' \
                 not found in models.toml. Add a [provider.\"{}\"] block.",
                model_cfg.provider, model_cfg.provider,
            )
        })?;

        Ok((
            model_cfg,
            provider_cfg,
            model_cfg.model_id.as_str(),
            dimensions,
        ))
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
    #[cfg(feature = "led-tools")]
    pub fn is_led_configured(&self) -> bool {
        self.led.ip.is_some()
    }

    /// Get LED endpoint URL (returns None if not configured)
    ///
    /// Note: Used only when `led-tools` feature is enabled.
    #[cfg(feature = "led-tools")]
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
    ///
    /// Build a client for a named provider from `models.toml`.
    /// Each model declares its `provider = "<name>"` in `models.toml`; this
    /// function returns the appropriate `Ollama` (shim) for that provider.
    pub fn provider_for(&self, provider_name: &str) -> crate::provider::OpenAICompatibleProvider {
        let providers = crate::user_models::get_providers();
        if let Some(cfg) = providers.get(provider_name) {
            crate::provider::OpenAICompatibleProvider::from_provider_config(cfg)
        } else {
            // Fallback to default Ollama
            crate::provider::OpenAICompatibleProvider::new_local(
                "http://localhost".to_string(),
                11434,
            )
        }
    }

    /// Build a client for the provider of a given model name.
    /// Resolves the model's `provider = "<name>"` from `models.toml`
    /// and returns the appropriate `Ollama` shim.
    /// Falls back to default Ollama if model is not found or has no provider.
    pub fn provider_for_model(
        &self,
        model_name: &str,
    ) -> crate::provider::OpenAICompatibleProvider {
        if let Some(provider_name) = crate::user_models::get_provider_for_model(model_name) {
            return self.provider_for(&provider_name);
        }
        // Fallback: use the first OpenAI provider if available, else default Ollama
        let providers = crate::user_models::get_providers();
        for (_name, cfg) in providers.iter() {
            if matches!(cfg.kind, crate::user_models::ProviderKind::OpenAI) {
                return crate::provider::OpenAICompatibleProvider::from_provider_config(cfg);
            }
        }
        crate::provider::OpenAICompatibleProvider::new_local("http://localhost".to_string(), 11434)
    }

    /// Create a sample config file if it doesn't exist
    pub fn create_sample_config() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = Self::config_dir().ok_or("Could not determine config directory")?;
        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            return Ok(config_path);
        }

        std::fs::write(&config_path, SAMPLE_CONFIG)?;
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
        // Host/port config moved to models.toml [provider.*] section
        assert_eq!(settings.display.skin, "dark");
        assert!(settings.display.show_tool_calls);
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

[tools]
blacklist = ["web_search", "fetch_page"]

[output]
plain_default = true

[display]
skin = "light"
"#;

        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.model.default, "qwen3-coder");
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
        // Verify that config key "glm-ocr" resolves to a model_id
        // via get_model_config. The resolution can hit either the
        // builtin (`glm-ocr:bf16`) or the user's models.toml override. We just verify the
        // model_id is non-empty and contains "glm-ocr".
        use crate::user_models::get_model_config;
        let config = get_model_config("glm-ocr");
        assert!(config.is_some(), "glm-ocr should resolve via config key");
        let config = config.unwrap();
        assert!(
            !config.model_id.is_empty() && config.model_id.contains("glm-ocr"),
            "model_id should resolve (got {:?})",
            config.model_id
        );
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

    #[test]
    fn test_fact_settings_defaults() {
        let settings = Settings::default();
        assert!(settings.facts.auto_extract);
        assert_eq!(settings.facts.max_facts, 3);
        assert!(settings.facts.auto_extract_notify);
        assert!((settings.facts.semantic_threshold - 0.70).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fact_settings_parse_defaults() {
        // Empty config should yield all defaults
        let settings: Settings = toml::from_str("").unwrap();
        assert!(settings.facts.auto_extract);
        assert!((settings.facts.semantic_threshold - 0.70).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fact_settings_parse_overrides() {
        let sample = r#"
[facts]
auto_extract = false
max_facts = 5
semantic_threshold = 0.80
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert!(!settings.facts.auto_extract);
        assert_eq!(settings.facts.max_facts, 5);
        assert!((settings.facts.semantic_threshold - 0.80).abs() < f32::EPSILON);
        // Not overridden fields should keep defaults
        assert!(settings.facts.auto_extract_notify);
    }

    #[test]
    fn test_indexing_settings_defaults() {
        // keyword_weight and semantic_weight live in IndexingSettings,
        // not RetrievalSettings.
        let settings = Settings::default();
        assert!((settings.indexing.keyword_weight - 0.4).abs() < f32::EPSILON);
        assert!((settings.indexing.semantic_weight - 0.6).abs() < f32::EPSILON);
        assert_eq!(settings.indexing.prefix, "search_document: ");
    }

    #[test]
    fn test_indexing_settings_parse_defaults() {
        // Empty config should yield all defaults
        let settings: Settings = toml::from_str("").unwrap();
        assert!((settings.indexing.keyword_weight - 0.4).abs() < f32::EPSILON);
        assert!((settings.indexing.semantic_weight - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_indexing_settings_parse_overrides() {
        let sample = r#"
[indexing]
model = "nomic"
keyword_weight = 0.3
semantic_weight = 0.7
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert!((settings.indexing.keyword_weight - 0.3).abs() < f32::EPSILON);
        assert!((settings.indexing.semantic_weight - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_thinking_trace_settings_default_disabled() {
        let settings = Settings::default();
        assert!(
            !settings.thinking_trace.enabled,
            "thinking_trace should be disabled by default"
        );
    }

    #[test]
    fn test_thinking_trace_settings_parse_enabled() {
        let sample = r#"
[thinking_trace]
enabled = true
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert!(settings.thinking_trace.enabled);
    }

    #[test]
    fn test_indexing_settings_default_via_settings_default() {
        // Settings::default() yields a default IndexingSettings to
        // keep tests working; the real validation of the [indexing]
        // TOML section happens in init_chat_database, not in serde.
        let settings = Settings::default();
        assert_eq!(settings.indexing.model, "nomic");
        assert!(settings.indexing.probe);
        assert!((settings.indexing.keyword_weight - 0.4).abs() < f32::EPSILON);
        assert!((settings.indexing.semantic_weight - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_indexing_settings_parse_minimal() {
        let sample = r#"
[indexing]
model = "nomic"
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.indexing.model, "nomic");
        assert!(settings.indexing.probe);
    }

    #[test]
    fn test_indexing_settings_parse_full() {
        let sample = r#"
[indexing]
model = "bge"
probe = false
keyword_weight = 0.3
semantic_weight = 0.7
prefix = ""
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.indexing.model, "bge");
        assert!(!settings.indexing.probe);
        assert!((settings.indexing.keyword_weight - 0.3).abs() < f32::EPSILON);
        assert!((settings.indexing.semantic_weight - 0.7).abs() < f32::EPSILON);
        assert_eq!(settings.indexing.prefix, "");
    }

    #[test]
    fn test_indexing_settings_omitted_uses_default() {
        // serde(default) makes the [indexing] section optional at the
        // TOML level. The real check that settings.indexing.model is
        // non-empty happens in init_chat_database, where sprach bails
        // out with a clear error.
        let sample = r#"
[model]
default = "qwen3.5:4b"
"#;
        let settings: Settings = toml::from_str(sample).unwrap();
        assert_eq!(settings.indexing.model, "nomic");
    }

    #[test]
    fn test_resolve_indexing_model_empty_alias() {
        // [indexing].model is empty.
        let mut settings = Settings::default();
        settings.indexing.model = String::new();
        let result = settings.resolve_indexing_model();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("[indexing].model is empty"));
    }

    #[test]
    fn test_resolve_indexing_model_alias_not_found() {
        // [indexing].model = "missing" but no such alias in models.toml.
        let mut settings = Settings::default();
        settings.indexing.model = "nonexistent-alias-xyz".to_string();
        let result = settings.resolve_indexing_model();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("nonexistent-alias-xyz"));
        assert!(err.contains("not found in models.toml"));
    }

    #[test]
    fn test_resolve_indexing_model_alias_not_embedding_capable() {
        // Alias exists in models.toml but lacks `embeddings = true`.
        let models = crate::user_models::get_user_models();
        let any_chat = models.iter().find(|(_, m)| !m.embeddings);
        let Some((name, _)) = any_chat else {
            eprintln!("SKIP: test requires models.toml with at least one chat model.");
            return;
        };
        let mut settings = Settings::default();
        settings.indexing.model = name.clone();
        let result = settings.resolve_indexing_model();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains(name.as_str()));
        assert!(err.contains("not declared as an embedding model"));
    }

    #[test]
    fn test_resolve_indexing_model_alias_success() {
        // Pick an embedding-capable alias from models.toml and verify
        // resolve returns the expected tuple.
        let models = crate::user_models::get_user_models();
        let any_embedding = models.iter().find(|(_, m)| m.embeddings);
        let Some((name, model_cfg)) = any_embedding else {
            eprintln!(
                "SKIP: test requires models.toml with at least one [models.X] \
                 block having `embeddings = true`."
            );
            return;
        };
        let mut settings = Settings::default();
        settings.indexing.model = name.clone();
        let result = settings.resolve_indexing_model();
        assert!(
            result.is_ok(),
            "resolve_indexing_model should succeed for '{name}': {:?}",
            result.err()
        );
        let (resolved_model_cfg, resolved_provider_cfg, model_id, dimensions) = result.unwrap();
        assert_eq!(resolved_model_cfg.model_id, model_cfg.model_id);
        assert_eq!(model_id, model_cfg.model_id);
        assert_eq!(dimensions, model_cfg.dimensions.unwrap());
        // Provider is resolved via the model's provider field.
        let providers = crate::user_models::get_providers();
        assert_eq!(
            resolved_provider_cfg.base_url,
            providers.get(&model_cfg.provider).unwrap().base_url
        );
    }
}
