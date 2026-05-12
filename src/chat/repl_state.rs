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
use crate::chat::view::StatusBarInfo;
use crate::config::ModelConfig;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::settings::Settings;
use crate::tokens::ContextMetrics;

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
    pub ollama: ollama_rs::Ollama,
    pub db: Option<Arc<Database>>,
    pub embedding_client: Option<Arc<EmbeddingClient>>,

    // Settings reference (immutable after init)
    pub settings: Settings,

    // UI state
    pub last_assistant_message_id: Option<i64>,
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
    ollama: Option<ollama_rs::Ollama>,
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

    pub fn ollama(mut self, ollama: ollama_rs::Ollama) -> Self {
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
        let ollama = self.ollama.ok_or("ollama client is required")?;
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
        })
    }
}

impl ReplState {
    /// Get status bar info from context metrics
    #[allow(dead_code)] // PR3: Will be used for TUI status bar
    pub fn get_status_bar_info(&self, metrics: ContextMetrics) -> StatusBarInfo {
        let percent = if metrics.context_window > 0 {
            ((metrics.total_tokens as f64 / metrics.context_window as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };

        StatusBarInfo {
            model_name: self.current_model_name.clone(),
            used_tokens: metrics.total_tokens,
            max_tokens: metrics.context_window,
            percent,
            think_enabled: self.session.think && self.capabilities.thinking,
            tools_enabled: self.tools_active,
        }
    }
}

impl Default for ReplStateBuilder {
    fn default() -> Self {
        Self::new()
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
        use ollama_rs::Ollama;

        let session = ChatSession::new("test-model".to_string(), None, false);
        let model_config = ModelConfig::get_default();
        let capabilities = ModelCapabilities::default();
        let ollama = Ollama::new("http://localhost".to_string(), 11434);

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
