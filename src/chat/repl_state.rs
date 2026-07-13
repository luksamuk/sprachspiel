//! REPL state management
//!
//! This module provides `ReplState`, a struct that consolidates all mutable state
//! in the chat REPL loop. This separation enables:
//!
//! 1. Clearer state management (all state in one place)
//! 2. Easier testing (state can be mocked)
//! 3. Future TUI compatibility (state separated from I/O)
//!
//! # Architecture
//!
//! ```text
//! Layer 3 (State): repl_state.rs
//!     ↓ uses
//! Layer 1 (Session): session.rs
//! Layer 0 (Base): capabilities, config
//! ```

use std::sync::Arc;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::settings::Settings;

use super::session::ChatSession;

/// Consolidated state for the chat REPL
///
/// This struct holds all mutable state that changes during the REPL loop,
/// separating it from I/O concerns (input/output) and business logic.
///
/// # State Categories
///
/// - **Session**: chat history, messages, persistence
/// - **Model**: current model, capabilities, config
/// - **Tools**: tools active, think mode
/// - **Context**: debug mode, agents.md, etc.
#[derive(Clone)]
pub struct ReplState {
    // Session state
    pub session: ChatSession,

    // Model state
    pub current_model_name: String,
    pub model_config: ModelConfig,
    pub capabilities: ModelCapabilities,

    // Tool/toggle state
    pub tools_active: bool,

    // Context state
    pub agents_md: Option<String>,

    // Command flags (immutable after init)
    pub cli_code: bool,
    pub cli_soulless: bool,

    // External clients (immutable after init)
    pub ollama: crate::provider::OpenAICompatibleProvider,
    pub db: Option<Arc<Database>>,
    pub embedding_client: Option<Arc<EmbeddingClient>>,

    // Settings reference (immutable after init)
    pub settings: Settings,

    // UI state
    pub last_assistant_message_id: Option<i64>,

    /// Throttle bucket for status bar updates during streaming.
    ///
    /// The real `prompt_eval_count` only arrives in the final streaming
    /// chunk (with `usage`), so we estimate the prompt size during streaming
    /// via `ContextUsage::from_session_estimate`. To avoid re-rendering the
    /// status bar on every character, we only re-render when the estimated
    /// total crosses a 50-token boundary. This field tracks the last bucket
    /// we rendered.
    pub last_status_token_bucket: u64,
}

/// Builder for ReplState
///
/// Provides a fluent interface for constructing ReplState,
/// useful during REPL initialization.
pub struct ReplStateBuilder {
    session: Option<ChatSession>,
    model_config: Option<ModelConfig>,
    capabilities: Option<ModelCapabilities>,
    tools_active: bool,
    agents_md: Option<String>,
    cli_code: bool,
    cli_soulless: bool,
    ollama: Option<crate::provider::OpenAICompatibleProvider>,
    db: Option<Arc<Database>>,
    embedding_client: Option<Arc<EmbeddingClient>>,
    settings: Option<Settings>,
    last_assistant_message_id: Option<i64>,
}

impl ReplStateBuilder {
    pub fn new() -> Self {
        Self {
            session: None,
            model_config: None,
            capabilities: None,
            tools_active: false,
            agents_md: None,
            cli_code: false,
            cli_soulless: false,
            ollama: None,
            db: None,
            embedding_client: None,
            settings: None,
            last_assistant_message_id: None,
        }
    }

    pub fn session(mut self, session: ChatSession) -> Self {
        self.session = Some(session);
        self
    }

    pub fn model_config(mut self, config: ModelConfig) -> Self {
        self.model_config = Some(config);
        self
    }

    pub fn capabilities(mut self, caps: ModelCapabilities) -> Self {
        self.capabilities = Some(caps);
        self
    }

    pub fn tools_active(mut self, active: bool) -> Self {
        self.tools_active = active;
        self
    }

    pub fn agents_md(mut self, md: Option<String>) -> Self {
        self.agents_md = md;
        self
    }

    pub fn cli_code(mut self, code: bool) -> Self {
        self.cli_code = code;
        self
    }

    pub fn cli_soulless(mut self, soulless: bool) -> Self {
        self.cli_soulless = soulless;
        self
    }

    pub fn ollama(mut self, ollama: crate::provider::OpenAICompatibleProvider) -> Self {
        self.ollama = Some(ollama);
        self
    }

    pub fn db(mut self, db: Option<Arc<Database>>) -> Self {
        self.db = db;
        self
    }

    pub fn embedding_client(mut self, client: Option<Arc<EmbeddingClient>>) -> Self {
        self.embedding_client = client;
        self
    }

    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn build(self) -> Result<ReplState, String> {
        let session = self.session.ok_or("session is required")?;
        let model_config = self.model_config.ok_or("model_config is required")?;
        let capabilities = self.capabilities.ok_or("capabilities is required")?;
        let ollama = self.ollama.ok_or("LLM client is required")?;
        let settings = self.settings.ok_or("settings is required")?;

        let current_model_name = session.model.clone();

        Ok(ReplState {
            session,
            current_model_name,
            model_config,
            capabilities,
            tools_active: self.tools_active,
            agents_md: self.agents_md,
            cli_code: self.cli_code,
            cli_soulless: self.cli_soulless,
            ollama,
            db: self.db,
            embedding_client: self.embedding_client,
            settings,
            last_assistant_message_id: self.last_assistant_message_id,
            last_status_token_bucket: 0,
        })
    }
}

impl Default for ReplStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplState {
    /// Return session entries for the tab completer.
    ///
    /// Each entry is `(display_label, id)` where `display_label` shows
    /// the session name (or ID if unnamed), suitable for `/session forget`
    /// tab completion.
    pub fn session_entries_for_completer(&self) -> Vec<(String, String)> {
        let Some(db) = &self.db else {
            return Vec::new();
        };
        match db.list_sessions(self.session.project_id.as_deref()) {
            Ok(sessions) => sessions
                .into_iter()
                .map(|s| {
                    let label = s.name.unwrap_or_else(|| s.id.clone());
                    (label, s.id)
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Estimate current context usage for status bar display during streaming.
    ///
    /// The real `prompt_eval_count` only arrives in the FINAL streaming
    /// chunk (with `usage`). To keep the status bar from going stale during
    /// streaming, we use `ContextUsage::from_session_estimate` to get an
    /// approximate prompt size based on session state.
    ///
    /// The estimate has a 30-50% undercount bias vs real tokenizers (see
    /// the TODO in src/tokens.rs), but that's acceptable for a status bar
    /// — the user wants to see the TREND, not the exact number.
    ///
    /// **Bug fix:** Previously this used `String::new()` as the system prompt,
    /// which meant system prompt tokens (~3.5K) and tool definition tokens
    /// (~2.9K) were NOT counted, producing a misleadingly low total (e.g.,
    /// 277 tokens instead of ~6.4K). Now we build the real system prompt via
    /// `build_pre_tool_prompt` so the estimate includes system + tools.
    ///
    /// Returns `(used_tokens, max_tokens, percent)` suitable for
    /// `view.update_status_tokens`. Returns `None` if `num_ctx` is unset.
    pub fn estimate_status_bar(&self) -> Option<(usize, usize, u8)> {
        let ctx_window = self.model_config.num_ctx as usize;
        if ctx_window == 0 {
            return None;
        }
        // Build the real system prompt so system + tool tokens are counted.
        // This uses the same builder as the pre-tool check in repl.rs.
        let system_prompt = super::continuation::build_pre_tool_prompt(self);
        let usage = crate::tokens::ContextUsage::from_session_estimate(
            &self.session,
            &system_prompt,
            self.tools_active,
        );
        // from_session_estimate sets tools_tokens to 0 — add the real count
        // so tool definitions are reflected in the total.
        let tools_tokens = if self.tools_active {
            let tool_count = crate::tools::get_available_tool_names(&self.settings).len();
            tool_count * crate::tokens::TOKENS_PER_TOOL
        } else {
            0
        };
        let total = usage.total_tokens + tools_tokens;
        let percent = if ctx_window > 0 {
            ((total as f64 / ctx_window as f64) * 100.0) as u8
        } else {
            0
        };
        Some((total, ctx_window, percent.min(100)))
    }

    /// Bucket for throttling status bar updates.
    ///
    /// Returns `total_tokens / STATUS_BAR_BUCKET_TOKENS` (rounded down).
    /// The event loop compares this against the previous bucket; only
    /// re-render the status bar when the bucket changes.
    pub fn status_bar_bucket(&self) -> u64 {
        const STATUS_BAR_BUCKET_TOKENS: u64 = 50;
        match self.estimate_status_bar() {
            Some((used, _, _)) => (used as u64) / STATUS_BAR_BUCKET_TOKENS,
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests require mock objects for ChatSession, ModelConfig, etc.
    // These will be added in Phase 9 (unit tests for refactored modules).

    #[test]
    fn test_replstate_builder_requires_session() {
        let result = ReplStateBuilder::new().build();
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.contains("session"));
        }
    }

    #[test]
    fn test_replstate_builder_requires_model_config() {
        let session = ChatSession::new("test-model".to_string(), None, false);
        let result = ReplStateBuilder::new().session(session).build();
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.contains("model_config"));
        }
    }

    #[test]
    fn test_replstate_builder_requires_settings() {
        use crate::capabilities::ModelCapabilities;
        use crate::config::ModelConfig;
        use crate::provider::OpenAICompatibleProvider;

        let session = ChatSession::new("test-model".to_string(), None, false);
        let model_config = ModelConfig::get_default();
        let capabilities = ModelCapabilities::default();
        let ollama = OpenAICompatibleProvider::new_local("http://localhost".to_string(), 11434);

        let result = ReplStateBuilder::new()
            .session(session)
            .model_config(model_config)
            .capabilities(capabilities)
            .ollama(ollama)
            .build();
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.contains("settings"));
        }
    }
}
