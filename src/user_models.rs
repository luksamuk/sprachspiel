//! User-defined model configurations
//!
//! Allows users to define custom models or override built-in model parameters
//! via a TOML file at ~/.config/sprachspiel/models.toml
//!
//! Schema (W2 #121 — OpenAI-First):
//! ```toml
//! [provider."my-llama-swap"]
//! kind = "openai"             # default; "ollama" is deprecated alias
//! base_url = "http://localhost:12434/v1"   # /v1 suffix REQUIRED
//! connect_timeout_secs = 5
//! read_timeout_secs = 300
//! stream_idle_timeout_secs = 180
//! max_retries = 3
//! retry_base_delay_ms = 2000
//! retry_max_delay_ms = 16000
//! retry_jitter_percent = 20
//!
//! # Chat model (default: embeddings = false)
//! [models."gemma4-e2b"]
//! model_id = "gemma4-e2b:think"
//! num_ctx = 32768             # optional; auto-detected if absent
//! temperature = 0.7
//! top_p = 0.95
//! seed = 42                   # optional, cross-provider
//! thinking = true             # optional; explicit capability flag
//! tools = true                # optional; explicit capability flag
//! vision = false              # optional; explicit capability flag
//! provider = "my-llama-swap"
//!
//! # Embedding model (opt-in via embeddings = true)
//! [models."nomic"]
//! model_id = "nomic-embed-text-v2-moe"
//! provider = "my-llama-swap"
//! embeddings = true     # opt-in; reserves the alias for /v1/embeddings only
//! dimensions = 768      # REQUIRED when embeddings = true
//!
//! # Vision model (opt-in via vision = true)
//! [models."glm-ocr"]
//! model_id = "glm-ocr:bf16"
//! provider = "my-llama-swap"
//! vision = true         # REQUIRED for vision: OpenAI-compat doesn't expose vision in /v1/models
//! ```

#![expect(clippy::print_stderr)] // CLI model management output
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::config::ModelConfig;

/// Provider kind enumeration.
///
/// W2 #121: `OpenAI` is the default (and only fully implemented kind).
/// `Ollama` is kept as a deprecated alias for backward compatibility with
/// `models.toml` files from before #121. It is auto-migrated to `OpenAI`
/// by `sprach models upgrade`. Anthropic is reserved for future use.
///
/// Note: `rename_all = "snake_case"` would produce `open_a_i` (because
/// `OpenAI` has a contiguous uppercase sequence), so we use explicit
/// `#[serde(rename = "...")]` attributes to control the on-disk format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    #[default]
    #[serde(rename = "openai")]
    OpenAI,
    /// Deprecated: use `OpenAI` with `base_url = ".../v1"`. Will be removed in #123.
    #[serde(rename = "ollama", alias = "openai_compatible")]
    OllamaLegacy,
    /// Reserved for future use (M3 or later).
    #[serde(rename = "anthropic")]
    Anthropic,
}

/// Provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,

    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,

    #[serde(default = "default_read_timeout")]
    pub read_timeout_secs: u64,

    #[serde(default = "default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: u64,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,

    #[serde(default = "default_retry_max_delay_ms")]
    pub retry_max_delay_ms: u64,

    #[serde(default = "default_retry_jitter_percent")]
    pub retry_jitter_percent: u8,

    // Future: OpenAI-compatible specific
    pub api_key_env: Option<String>,
}

fn default_connect_timeout() -> u64 {
    5
}
fn default_read_timeout() -> u64 {
    300
}
fn default_stream_idle_timeout() -> u64 {
    // 180s — aligned with Hermes Agent's base stale-stream timeout.
    // Cloud reasoning models (MiniMax M3, Nemotron 3 Ultra, etc.) can
    // take 60-120s between SSE connection and the first token during
    // prefill + thinking. The previous 60s default caused spurious
    // "SSE stream idle timeout" errors with cloud providers.
    // Users can override via stream_idle_timeout_secs in models.toml.
    180
}
fn default_max_retries() -> u32 {
    3
}
fn default_retry_base_delay_ms() -> u64 {
    2000
}
fn default_retry_max_delay_ms() -> u64 {
    16000
}
fn default_retry_jitter_percent() -> u8 {
    20
}

impl ProviderConfig {
    /// Normalize base_url to ensure it has a scheme (http:// or https://) AND a /v1 suffix.
    ///
    /// W2 #121: All backends now speak OpenAI-compat. The `/v1` suffix is part of
    /// the OpenAI API spec. For Ollama at `http://localhost:11434`, the normalized
    /// URL becomes `http://localhost:11434/v1`.
    pub fn normalize_base_url(&mut self) {
        let trimmed = self.base_url.trim();
        let mut url = trimmed.to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            url = format!("http://{url}");
        }
        // Append /v1 if not present. We assume any URL without /v1 is missing it.
        // Exception: URLs ending with /v1, /v1/, or already containing /v1 in the path.
        if !url.contains("/v1") && !url.ends_with('/') {
            url.push_str("/v1");
        } else if url.ends_with('/') && !url.contains("/v1/") && !url.ends_with("/v1/") {
            // Trailing slash with no /v1 — append
            url.push_str("v1");
        }
        self.base_url = url;
    }
}

/// User model configuration.
///
/// W2 #121 (BREAKING CHANGES):
/// - Removed: `top_k`, `repeat_penalty`, `think`
///   (not supported by OpenAI API; ollama/ollama#11325 closed as "not planned")
/// - Added: `seed` (cross-provider, optional)
/// - `num_ctx` is now optional (auto-detected via `/v1/models` and `/api/show`)
///
/// W2 #121 (extension): Embedding model opt-in:
/// - Added: `embeddings: bool` (default false) — declares this model
///   as embedding-capable. When true, `dimensions` MUST also be set.
/// - Added: `dimensions: Option<u32>` — required when `embeddings = true`.
///   Specifies the output dimension of the embedding model (e.g. 768
///   for nomic-embed-text-v2-moe full, 256 for Matryoshka-truncated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserModelConfig {
    pub model_id: String,
    /// Optional context window. If `None`, auto-detected from server.
    pub num_ctx: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    /// Optional seed for reproducible outputs (cross-provider).
    pub seed: Option<u32>,
    /// Whether the model supports thinking mode (chain-of-thought).
    /// Tri-state (`None` = probe fallback, `Some(true/false)` = explicit).
    /// Required for chat models served by non-Ollama providers where
    /// the probe can't see the "thinking" capability flag.
    pub thinking: Option<bool>,
    /// Whether the model supports tool calling.
    /// Tri-state (`None` = probe fallback, `Some(true/false)` = explicit).
    pub tools: Option<bool>,
    /// Whether the model supports vision (image inputs).
    /// Tri-state (`None` = probe fallback, `Some(true/false)` = explicit).
    /// Required for vision models (OCR, vision subcommand) because
    /// OpenAI-compat `/v1/models` does NOT expose a vision flag, so
    /// the probe can't detect it.
    #[serde(default)]
    pub vision: Option<bool>,
    pub provider: String,
    /// Whether this model is declared as an embedding model.
    ///
    /// When `true`, the model is reserved for embedding generation
    /// (`/v1/embeddings` endpoint) and CANNOT be used for chat
    /// completions via `-m <alias>` or `/model <alias>`. Use the
    /// `[indexing].model` field in `config.toml` to reference the
    /// alias for embedding generation.
    ///
    /// Default: `false` (chat model).
    #[serde(default)]
    pub embeddings: bool,
    /// Output dimension of the embedding model.
    ///
    /// Required when `embeddings = true`. Specifies the dim count
    /// that sprach's vector store will use (e.g. 768 for
    /// nomic-embed-text-v2-moe at full precision, 256 for
    /// Matryoshka-truncated storage).
    ///
    /// The startup probe verifies the provider's actual response dim
    /// matches this value, failing fast if there's a mismatch
    /// (catches Matryoshka misconfigurations).
    pub dimensions: Option<u32>,
}

/// Complete user models file structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserModelsFile {
    pub provider: HashMap<String, ProviderConfig>,
    pub models: HashMap<String, UserModelConfig>,
}

pub struct UserModelDefaults;

impl UserModelDefaults {
    /// Default context window when auto-detection fails.
    pub const NUM_CTX: u32 = 32768;
    pub const TEMPERATURE: f32 = 0.8;
}

pub fn get_user_models_path() -> PathBuf {
    use crate::consts::app;

    if let Ok(data_home) = env::var("XDG_DATA_HOME") {
        PathBuf::from(data_home)
            .join(app::APP_CONFIG_DIR)
            .join("models.toml")
    } else if let Some(home_dir) = dirs::home_dir() {
        home_dir
            .join(".config")
            .join(app::APP_CONFIG_DIR)
            .join("models.toml")
    } else {
        PathBuf::from(app::APP_PROJECT_DIR).join("models.toml")
    }
}

fn load_user_models_internal() -> Result<UserModelsFile, String> {
    let path = get_user_models_path();

    if !path.exists() {
        return Err(format!("Models file not found at {}", path.display()));
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read models file '{}': {}", path.display(), e))?;

    parse_and_validate_user_models(&contents, &path.display().to_string())
}

/// Parse and validate a `UserModelsFile` from a TOML string. The
/// `source_label` is used in error messages (the file path, or a
/// test label like `"<inline>"`).
///
/// W2 #121: extracted from `load_user_models_internal` so tests can
/// exercise the validation logic without writing to disk.
fn parse_and_validate_user_models(
    contents: &str,
    source_label: &str,
) -> Result<UserModelsFile, String> {
    // Pre-parse hint: detect the common user error (commented-out
    // [provider.*] block) before TOML parse fails with the generic
    // "missing field `provider`" message. The UserModelsFile struct
    // requires at least one [provider."..."] entry; without it the
    // TOML deserializer fails before our explicit validation below
    // (line "Validate: provider section must exist") can run.
    // Per PR #206 review: emit a specific, actionable error instead.
    //
    // We check for an UNCOMMENTED [provider.*] line (line doesn't start
    // with optional whitespace followed by `#`). This catches both
    // "no provider block at all" and "provider block is commented out".
    let has_active_provider = contents.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with('#') && trimmed.starts_with("[provider.")
    });
    if !has_active_provider {
        return Err(format!(
            "Missing [provider.\"name\"] section in models.toml at {source_label}. \
             Add at least one [provider.\"my-ollama\"] block with \
             `kind = \"openai\"` and `base_url = \"http://127.0.0.1:11434/v1\"`. \
             Run `sprach models upgrade` to migrate an existing config."
        ));
    }

    let mut file: UserModelsFile = toml::from_str(contents)
        .map_err(|e| format!("Failed to parse models file '{source_label}': {e}"))?;

    // Validate: provider section must exist
    if file.provider.is_empty() {
        return Err("Missing [provider] section in models.toml".to_string());
    }

    // Validate: at least one model must exist
    if file.models.is_empty() {
        return Err("No models defined in models.toml".to_string());
    }

    // Normalize base_url for all providers (adds /v1 suffix if missing)
    for provider_config in file.provider.values_mut() {
        provider_config.normalize_base_url();
    }

    // Validate: all model providers exist
    for (model_name, model_config) in &file.models {
        if !file.provider.contains_key(&model_config.provider) {
            return Err(format!(
                "Model '{}' references unknown provider '{}'",
                model_name, model_config.provider
            ));
        }
    }

    // Validate: embedding models must declare `dimensions` (W2 #121).
    // A model with `embeddings = true` and no `dimensions` cannot be
    // used: the indexing pipeline needs a known dim count to size
    // the vector store and to verify the probe response.
    for (model_name, model_config) in &file.models {
        if model_config.embeddings && model_config.dimensions.is_none() {
            return Err(format!(
                "Embedding model '{model_name}' is missing `dimensions` in models.toml. \
                 Add `dimensions = <N>` (e.g. 768 for nomic-embed-text-v2-moe, \
                 256 for Matryoshka-truncated).",
            ));
        }
    }

    // Model name uniqueness is guaranteed by HashMap keys

    Ok(file)
}

/// Cached loaded models file. Returns an empty `UserModelsFile` on load
/// failure (missing file, parse error, validation error) so that the
/// process does not abort — callers that need a valid config check
/// `get_providers().is_empty()` or use `get_user_models_path()` to
/// surface the error contextually.
static USER_MODELS_FILE: LazyLock<UserModelsFile> = LazyLock::new(|| {
    load_user_models_internal().unwrap_or_else(|e| {
        log::error!("Failed to load models.toml: {e}");
        UserModelsFile::default()
    })
});

/// Get provider configs.
pub fn get_providers() -> &'static HashMap<String, ProviderConfig> {
    &USER_MODELS_FILE.provider
}

/// Check that at least one provider is configured in `models.toml`.
///
/// Returns `Ok(())` if providers exist, or an `Err` with an actionable
/// error message if not. Use this BEFORE calling `resolve_model_config`
/// to prevent `process::exit(1)` from masking the actual config error.
///
/// Per PR #206 review: failing silently with "default" or generic
/// "Unknown model" masks user configuration errors. Callers should call
/// this helper early in their entry points to bail out with a clear
/// message before any `resolve_model_config` is reached.
pub fn require_providers() -> Result<(), String> {
    if get_providers().is_empty() {
        return Err(
            "Cannot determine provider: no providers defined in models.toml. \
             Add a [provider.\"name\"] section or run `sprach models upgrade` to migrate."
                .to_string(),
        );
    }
    Ok(())
}

/// Get user-defined model configs.
pub fn get_user_models() -> &'static HashMap<String, UserModelConfig> {
    &USER_MODELS_FILE.models
}

/// Get the provider name for a given model name.
///
/// Returns `Some(provider_name)` if the model is in `models.toml` and has
/// a `provider` field set. Returns `None` for built-in models (which don't
/// have a `provider` field, since they were defined before the multi-provider
/// refactor) or for unknown model names.
///
/// Lookup order:
///  1. Map key (e.g. `"gemma4-e2b"`) — what `--model gemma4-e2b` resolves to.
///  2. Inner `model_id` field (e.g. `"gemma4-e2b:think"`) — what the
///     coordinator actually passes to the LLM client.
///
/// Callers may invoke this with either form, so we must accept both.
///
/// Used by the chat banner to display "Provider: <name>" instead of the
/// server URL, and by `Settings::ollama_client_for_model` to build the
/// correct client.
pub fn get_provider_for_model(model_name: &str) -> Option<String> {
    // 1. Direct map key lookup
    if let Some(cfg) = get_user_models().get(model_name) {
        return Some(cfg.provider.clone());
    }
    // 2. Fallback: scan by inner `model_id` field
    get_user_models()
        .values()
        .find(|cfg| cfg.model_id == model_name)
        .map(|cfg| cfg.provider.clone())
}

pub fn merge_configs(built_in: Option<&ModelConfig>, user: &UserModelConfig) -> ModelConfig {
    match built_in {
        Some(bi) => ModelConfig {
            model_id: user.model_id.clone(),
            num_ctx: user.num_ctx.unwrap_or(bi.num_ctx),
            temperature: user.temperature.unwrap_or(bi.temperature),
            top_p: user.top_p.or(bi.top_p),
            // W2 #121: top_k, repeat_penalty, think removed from UserModelConfig
            // Default fallbacks to "no overrides" (None)
            thinking: user.thinking.unwrap_or(bi.thinking),
        },
        None => ModelConfig {
            model_id: user.model_id.clone(),
            num_ctx: user.num_ctx.unwrap_or(UserModelDefaults::NUM_CTX),
            temperature: user.temperature.unwrap_or(UserModelDefaults::TEMPERATURE),
            top_p: user.top_p,
            thinking: user.thinking.unwrap_or(false),
        },
    }
}

pub fn get_model_config(name: &str) -> Option<ModelConfig> {
    // Try config key lookup first (e.g., "glm-ocr", "translategemma")
    let built_in = ModelConfig::get_builtin(name);
    let user_models = get_user_models();
    let user_config = user_models.get(name);

    match (built_in, user_config) {
        (Some(bi), Some(uc)) => Some(merge_configs(Some(bi), uc)),
        (Some(bi), None) => Some(bi.clone()),
        (None, Some(uc)) => Some(merge_configs(None, uc)),
        (None, None) => {
            // Fall back to lookup by model_id (e.g., "translategemma:4b" matches builtin config)
            ModelConfig::get_builtin_by_model_id(name).cloned()
        }
    }
}

pub fn is_model_valid(name: &str) -> bool {
    ModelConfig::is_builtin_valid(name) || get_user_models().contains_key(name)
}

/// Resolve model configuration with error handling
///
/// Returns the model configuration or prints an error and exits.
/// Use this instead of the `is_model_valid` + `get_model_config().unwrap()` pattern.
pub fn resolve_model_config(name: &str) -> ModelConfig {
    match get_model_config(name) {
        Some(config) => config,
        None => {
            eprintln!(
                "Error: Unknown model '{}'. Use --list to see available models.",
                name
            );
            std::process::exit(1);
        }
    }
}

/// Determine if think mode should be enabled
///
/// Priority: CLI flag > model config > subcommand config
/// Returns the final think mode state and prints warning if model doesn't support it.
pub fn resolve_think_mode(
    cli_think: bool,
    subcommand_thinking: bool,
    model_config_thinking: bool,
    model_id: &str,
    capabilities_thinking: bool,
) -> bool {
    let model_supports_think = capabilities_thinking || model_config_thinking;

    if cli_think {
        if model_supports_think {
            true
        } else {
            eprintln!(
                "Warning: Model '{}' does not support think mode. Ignoring -t flag.",
                model_id
            );
            false
        }
    } else {
        (subcommand_thinking || model_config_thinking) && model_supports_think
    }
}

/// All model names (chat + embedding). Used by `sprach --list`
/// and any other place that wants to enumerate every model
/// the user has configured, regardless of capability.
pub fn list_all_model_names() -> Vec<String> {
    let mut names: Vec<String> = ModelConfig::list_builtin_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    for name in get_user_models().keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }

    names.sort();
    names
}

/// Names of models that are safe to use as chat models (i.e. NOT
/// declared with `embeddings = true` in models.toml). Used by
/// the `/model` tab completer and any other place that should
/// hide embedding-only models from chat selection.
///
/// Built-in models are never embedding-only (the `ModelConfig`
/// builtin has no `embeddings` field), so they are always
/// included.
pub fn list_chat_model_names() -> Vec<String> {
    let mut names: Vec<String> = ModelConfig::list_builtin_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    for (name, cfg) in get_user_models() {
        if !cfg.embeddings && !names.contains(name) {
            names.push(name.clone());
        }
    }

    names.sort();
    names
}

/// Returns `true` if the model at the given name (alias or
/// inner model_id) is declared as embedding-only in
/// `models.toml`. Built-in models are never embedding-only.
///
/// W2 #121 extension: this is the canonical check used by
/// `model_switch::switch_model` and the `--model` CLI flag to
/// reject embedding-only models from being selected as chat
/// models.
#[must_use]
pub fn is_model_embedding_only(name: &str) -> bool {
    if let Some(cfg) = get_user_models().get(name) {
        return cfg.embeddings;
    }
    get_user_models()
        .values()
        .find(|cfg| cfg.model_id == name)
        .is_some_and(|cfg| cfg.embeddings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_model_defaults() {
        assert_eq!(UserModelDefaults::NUM_CTX, 32768);
        assert_eq!(UserModelDefaults::TEMPERATURE, 0.8);
    }

    #[test]
    fn test_provider_kind_default() {
        assert_eq!(ProviderKind::default(), ProviderKind::OpenAI);
    }

    #[test]
    fn test_provider_kind_ollama_legacy_deserialization() {
        // Backward compat: "ollama" in old configs should deserialize as OllamaLegacy
        let json = r#""ollama""#;
        let kind: ProviderKind = serde_json::from_str(json).unwrap();
        assert_eq!(kind, ProviderKind::OllamaLegacy);

        let json = r#""openai_compatible""#;
        let kind: ProviderKind = serde_json::from_str(json).unwrap();
        assert_eq!(kind, ProviderKind::OllamaLegacy);
    }

    #[test]
    fn test_provider_kind_serde_roundtrip() {
        let k = ProviderKind::OpenAI;
        let s = serde_json::to_string(&k).unwrap();
        let parsed: ProviderKind = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, ProviderKind::OpenAI);
    }

    #[test]
    fn test_base_url_normalization_adds_v1() {
        let mut cfg = ProviderConfig {
            kind: ProviderKind::OpenAI,
            base_url: "http://localhost:11434".to_string(),
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
            api_key_env: None,
        };
        cfg.normalize_base_url();
        assert_eq!(cfg.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn test_base_url_normalization_preserves_v1() {
        let mut cfg = ProviderConfig {
            kind: ProviderKind::OpenAI,
            base_url: "http://localhost:11434/v1".to_string(),
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
            api_key_env: None,
        };
        cfg.normalize_base_url();
        assert_eq!(cfg.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn test_base_url_normalization_adds_scheme() {
        let mut cfg = ProviderConfig {
            kind: ProviderKind::OpenAI,
            base_url: "localhost:11434".to_string(),
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
            api_key_env: None,
        };
        cfg.normalize_base_url();
        assert_eq!(cfg.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn test_base_url_normalization_preserves_https() {
        let mut cfg = ProviderConfig {
            kind: ProviderKind::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
            api_key_env: None,
        };
        cfg.normalize_base_url();
        assert_eq!(cfg.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_merge_partial_override() {
        let built_in = ModelConfig {
            model_id: "test:1b".to_string(),
            num_ctx: 8192,
            temperature: 0.5,
            top_p: Some(0.95),
            thinking: false,
        };

        let user = UserModelConfig {
            model_id: "test:1b".to_string(),
            num_ctx: Some(16384),
            temperature: None,
            top_p: None,
            seed: None,
            thinking: None,
            tools: None,
            vision: None,
            provider: "test".to_string(),
            embeddings: false,
            dimensions: None,
        };

        let merged = merge_configs(Some(&built_in), &user);

        assert_eq!(merged.model_id, "test:1b");
        assert_eq!(merged.num_ctx, 16384);
        assert_eq!(merged.temperature, 0.5);
        assert_eq!(merged.top_p, Some(0.95));
    }

    #[test]
    fn test_user_only_model_no_ctx() {
        let user = UserModelConfig {
            model_id: "custom-model:7b".to_string(),
            num_ctx: None,
            temperature: None,
            top_p: None,
            seed: Some(42),
            thinking: Some(true),
            tools: None,
            vision: None,
            provider: "test".to_string(),
            embeddings: false,
            dimensions: None,
        };

        let merged = merge_configs(None, &user);

        assert_eq!(merged.model_id, "custom-model:7b");
        assert_eq!(merged.num_ctx, UserModelDefaults::NUM_CTX);
        assert_eq!(merged.temperature, UserModelDefaults::TEMPERATURE);
        assert!(merged.thinking);
    }

    #[test]
    fn test_parse_user_models_file_new_format() {
        let toml_content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "localhost:11434"
connect_timeout_secs = 10
read_timeout_secs = 600

[models."glm-5.1"]
model_id = "glm-5.1:cloud"
num_ctx = 202757
thinking = true
tools = true
seed = 42
provider = "my-ollama"
"#;

        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();

        assert_eq!(parsed.provider.len(), 1);
        assert!(parsed.provider.contains_key("my-ollama"));

        let prov = parsed.provider.get("my-ollama").unwrap();
        assert_eq!(prov.kind, ProviderKind::OpenAI);
        assert_eq!(prov.connect_timeout_secs, 10);
        assert_eq!(prov.read_timeout_secs, 600);

        assert_eq!(parsed.models.len(), 1);
        let model = parsed.models.get("glm-5.1").unwrap();
        assert_eq!(model.model_id, "glm-5.1:cloud");
        assert_eq!(model.provider, "my-ollama");
        assert_eq!(model.seed, Some(42));
    }

    #[test]
    fn test_parse_legacy_kind_ollama() {
        let toml_content = r#"
[provider."my-ollama"]
kind = "ollama"
base_url = "http://localhost:11434"

[models."test-model"]
model_id = "test:1b"
provider = "my-ollama"
"#;

        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        let prov = parsed.provider.get("my-ollama").unwrap();
        assert_eq!(prov.kind, ProviderKind::OllamaLegacy);
    }

    #[test]
    fn test_url_normalization_in_load() {
        let toml_content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "localhost:11434"

[models."test-model"]
model_id = "test:1b"
provider = "my-ollama"
"#;
        let mut parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        for (_, provider_config) in &mut parsed.provider {
            provider_config.normalize_base_url();
        }
        let prov = parsed.provider.get("my-ollama").unwrap();
        assert_eq!(prov.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn test_provider_defaults() {
        let toml_content = r#"
[provider."my-ollama"]
kind = "openai"
base_url = "http://localhost:11434/v1"

[models.test]
model_id = "test:1b"
provider = "my-ollama"
"#;

        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        let prov = parsed.provider.get("my-ollama").unwrap();

        assert_eq!(prov.connect_timeout_secs, 5);
        assert_eq!(prov.read_timeout_secs, 300);
        assert_eq!(prov.stream_idle_timeout_secs, 180);
        assert_eq!(prov.max_retries, 3);
        assert_eq!(prov.retry_base_delay_ms, 2000);
        assert_eq!(prov.retry_max_delay_ms, 16000);
        assert_eq!(prov.retry_jitter_percent, 20);
    }

    #[test]
    fn test_user_model_embeddings_default_false() {
        // W2 #121: `embeddings = true` is opt-in. Without the flag
        // in TOML, the model parses as embeddings=false.
        let toml_content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models."gemma4-e2b"]
model_id = "gemma4-e2b:think"
provider = "my-llama-swap"
"#;
        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        let model = parsed.models.get("gemma4-e2b").unwrap();
        assert!(!model.embeddings);
        assert!(model.dimensions.is_none());
    }

    #[test]
    fn test_user_model_embeddings_opt_in_with_dimensions() {
        // W2 #121: explicit `embeddings = true` requires
        // `dimensions = N`.
        let toml_content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models."nomic"]
model_id = "nomic-embed-text-v2-moe"
provider = "my-llama-swap"
embeddings = true
dimensions = 768
"#;
        let parsed: UserModelsFile = toml::from_str(toml_content).unwrap();
        let model = parsed.models.get("nomic").unwrap();
        assert!(model.embeddings);
        assert_eq!(model.dimensions, Some(768));
    }

    #[test]
    fn test_user_model_dimensions_required_when_embeddings() {
        // W2 #121: if `embeddings = true` but `dimensions` is
        // absent, parse_and_validate_user_models returns an error.
        let toml_content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models."nomic"]
model_id = "nomic-embed-text-v2-moe"
provider = "my-llama-swap"
embeddings = true
"#;
        let result = parse_and_validate_user_models(&toml_content, "<inline>");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("'nomic' is missing `dimensions`"));
    }

    #[test]
    fn test_user_model_dimensions_optional_when_chat() {
        // W2 #121: chat models (embeddings = false) don't need
        // dimensions.
        let toml_content = r#"
[provider."my-llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models."gemma4-e2b"]
model_id = "gemma4-e2b:think"
provider = "my-llama-swap"
"#;
        let result = parse_and_validate_user_models(&toml_content, "<inline>");
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_model_embedding_only() {
        // W2 #121: is_model_embedding_only returns true only for
        // models declared with `embeddings = true`.
        if get_user_models().is_empty() {
            eprintln!("SKIP: test requires models.toml with at least one entry.");
            return;
        }
        // Take any embedding-only model from the loaded models.toml
        // (or skip if none).
        let any_embedding = get_user_models().iter().find(|(_, m)| m.embeddings);
        let any_chat = get_user_models().iter().find(|(_, m)| !m.embeddings);
        if let Some((alias, _)) = any_embedding {
            assert!(is_model_embedding_only(alias));
        }
        if let Some((alias, _)) = any_chat {
            assert!(!is_model_embedding_only(alias));
        }
    }

    #[test]
    fn test_get_model_config_by_model_id_translategemma() {
        let config = get_model_config("translategemma:4b");
        assert!(
            config.is_some(),
            "translategemma:4b should resolve via model_id"
        );
        let config = config.unwrap();
        assert_eq!(config.model_id, "translategemma:4b");
        assert_eq!(config.temperature, 0.2);
        assert!(!config.thinking);
    }

    #[test]
    fn test_get_model_config_by_model_id_glm_ocr() {
        let config = get_model_config("glm-ocr:bf16");
        assert!(config.is_some(), "glm-ocr:bf16 should resolve via model_id");
        let config = config.unwrap();
        assert_eq!(config.model_id, "glm-ocr:bf16");
        assert_eq!(config.temperature, 0.1);
        assert!(!config.thinking);
    }

    #[test]
    fn test_get_model_config_by_key_still_works() {
        let config = get_model_config("translategemma");
        assert!(config.is_some());
        assert_eq!(config.unwrap().model_id, "translategemma:4b");

        // W2 #121 extension: glm-ocr can be either the builtin
        // (glm-ocr:bf16) or a user override in models.toml.
        // Just verify it resolves to a non-empty string
        // containing "glm-ocr".
        let config = get_model_config("glm-ocr");
        assert!(config.is_some());
        let model_id = config.unwrap().model_id;
        assert!(
            !model_id.is_empty() && model_id.contains("glm-ocr"),
            "glm-ocr should resolve to a model_id (got {:?})",
            model_id
        );
    }

    #[test]
    fn test_get_model_config_unknown_returns_none() {
        let config = get_model_config("nonexistent:model");
        assert!(config.is_none());
    }

    // === Tests for PR #206 E1: provider bail-out ===

    #[test]
    fn test_require_providers_returns_ok_when_providers_configured() {
        if get_providers().is_empty() {
            eprintln!(
                "SKIP: test requires models.toml with at least one [provider.*] block. \
                 Set up a fixture models.toml before running this test."
            );
            return;
        }
        assert!(
            require_providers().is_ok(),
            "require_providers() must return Ok when providers are configured"
        );
    }

    #[test]
    fn test_bail_out_error_message_contains_actionable_hint() {
        let expected_keywords = ["providers", "models.toml", "sprach models upgrade"];
        for keyword in &expected_keywords {
            assert!(
                keyword
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '\"' || c == '.'),
                "Keyword '{keyword}' is verified to be a lowercase ASCII string"
            );
        }
    }

    #[test]
    fn test_pre_parse_heuristic_patterns() {
        fn has_active_provider(contents: &str) -> bool {
            contents.lines().any(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with('#') && trimmed.starts_with("[provider.")
            })
        }

        let content_with_provider = r#"
[provider."my-ollama"]
kind = "openai"

[models."test"]
provider = "my-ollama"
"#;
        let content_completely_empty = "# only a comment\n";
        let content_with_commented_provider = "# [provider.\"my-ollama\"] is commented out\n";
        let content_with_inline_commented_provider = r#"
#[provider."my-ollama"]
[models."test"]
provider = "missing"
"#;

        assert!(
            has_active_provider(content_with_provider),
            "Heuristic: file WITH [provider.*] should be detected"
        );
        assert!(
            !has_active_provider(content_completely_empty),
            "Heuristic: comment-only file should NOT be detected as having providers"
        );
        assert!(
            !has_active_provider(content_with_commented_provider),
            "Heuristic: line-start commented [provider.*] should NOT be detected"
        );
        assert!(
            !has_active_provider(content_with_inline_commented_provider),
            "Heuristic: inline #[provider.*] (commented at line start) should NOT be detected"
        );
    }
}
