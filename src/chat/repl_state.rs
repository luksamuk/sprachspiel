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
///
/// # Example
///
/// ```ignore
/// let mut state = ReplState::new(
///     session,
///     model_config,
///     capabilities,
///     settings,
/// );
///
/// // State changes during REPL loop
/// state.switch_model("llama3.1".to_string());
/// state.toggle_think();
/// ```
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
    #[allow(dead_code)] // Used via state.agents_md in command handlers
    pub agents_md: Option<String>,
    pub use_debug: bool,

    // Command flags (immutable after init)
    pub cli_code: bool,
    pub cli_soulless: bool,

    // External clients (immutable after init)
    pub ollama: ollama_rs::Ollama,
    pub db: Option<Arc<Database>>,
    pub embedding_client: Option<Arc<EmbeddingClient>>,

    // Settings reference (immutable after init)
    pub settings: Settings,
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
    use_debug: bool,
    cli_code: bool,
    cli_soulless: bool,
    ollama: Option<ollama_rs::Ollama>,
    db: Option<Arc<Database>>,
    embedding_client: Option<Arc<EmbeddingClient>>,
    settings: Option<Settings>,
}

impl ReplStateBuilder {
    pub fn new() -> Self {
        Self {
            session: None,
            model_config: None,
            capabilities: None,
            tools_active: false,
            agents_md: None,
            use_debug: false,
            cli_code: false,
            cli_soulless: false,
            ollama: None,
            db: None,
            embedding_client: None,
            settings: None,
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

    pub fn use_debug(mut self, debug: bool) -> Self {
        self.use_debug = debug;
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
            use_debug: self.use_debug,
            cli_code: self.cli_code,
            cli_soulless: self.cli_soulless,
            ollama,
            db: self.db,
            embedding_client: self.embedding_client,
            settings,
        })
    }
}

impl Default for ReplStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplState {
    /// Create a builder for ReplState
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn builder() -> ReplStateBuilder {
        ReplStateBuilder::new()
    }

    /// Build the prompt string for the REPL
    ///
    /// Format: `model_name[t][T]> ` where:
    /// - `[t]` appears if think mode is enabled and model supports it
    /// - `[T]` appears if tools are active
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn build_prompt(&self) -> String {
        let mut prompt = self.current_model_name.clone();
        if self.session.think && self.capabilities.thinking {
            prompt.push_str("[t]");
        }
        if self.tools_active {
            prompt.push_str("[T]");
        }
        prompt.push_str("> ");
        prompt
    }

    /// Switch to a different model
    ///
    /// Updates the session and all model-related state.
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn switch_model(
        &mut self,
        model_name: String,
        model_config: ModelConfig,
        capabilities: ModelCapabilities,
    ) {
        self.session.set_model(model_name.clone());
        self.current_model_name = model_name;
        self.model_config = model_config;
        self.capabilities = capabilities;
    }

    /// Toggle think mode
    ///
    /// Returns the new state, or None if the model doesn't support thinking.
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn toggle_think(&mut self) -> Option<bool> {
        if !self.capabilities.thinking {
            return None;
        }
        self.session.think = !self.session.think;
        Some(self.session.think)
    }

    /// Toggle tools
    ///
    /// Returns the new state, or None if the model doesn't support tools.
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn toggle_tools(&mut self) -> Option<bool> {
        if !self.capabilities.tools {
            return None;
        }
        self.session.tools = !self.session.tools;
        self.tools_active = self.session.tools;
        Some(self.tools_active)
    }

    /// Check if this is an anonymous session
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn is_anonymous(&self) -> bool {
        self.session.anonymous
    }

    /// Save session to database (if not anonymous)
    #[allow(dead_code)] // Will be used in Phase 7 (handler extraction)
    pub fn save_session(&self) -> Result<(), String> {
        if self.session.anonymous {
            return Ok(());
        }
        self.session.save_sqlite().map_err(|e| e.to_string())
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
