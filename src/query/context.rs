//! Query context for state management
//!
//! Provides QueryContext struct and builder for query execution.

use std::sync::Arc;


use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::settings::Settings;
use crate::user_models;

/// Context for query execution
///
/// Consolidates all state needed for query execution in one struct.
pub struct QueryContext {
    pub model_config: ModelConfig,
    pub capabilities: ModelCapabilities,
    pub use_tools: bool,
    pub use_think: bool,
    pub agents_md: Option<String>,
    pub db: Option<Arc<Database>>,
    pub embedding_client: Option<Arc<EmbeddingClient>>,
    pub project_id: Option<String>,
    pub tool_names: Vec<String>,
    pub output_flags: super::OutputFlags,
    pub prompt_type: PromptType,
    pub prompt_name: String,
    pub system_prompt: String,
    pub ollama: crate::provider::Ollama,
}

/// Builder for QueryContext
pub struct QueryContextBuilder {
    cli_model: Option<String>,
    cli_think: bool,
    cli_tools: bool,
    cli_code: bool,
    cli_prompt: String,
    cli_ignore_agents: bool,
    cli_soulless: bool,
    plain: Option<bool>,
}

impl QueryContextBuilder {
    pub fn new() -> Self {
        Self {
            cli_model: None,
            cli_think: false,
            cli_tools: false,
            cli_code: false,
            cli_prompt: String::new(),
            cli_ignore_agents: false,
            cli_soulless: false,
            plain: None,
        }
    }

    pub fn cli_model(mut self, model: impl Into<Option<String>>) -> Self {
        self.cli_model = model.into();
        self
    }

    pub fn cli_think(mut self, think: bool) -> Self {
        self.cli_think = think;
        self
    }

    pub fn cli_tools(mut self, tools: bool) -> Self {
        self.cli_tools = tools;
        self
    }

    pub fn cli_code(mut self, code: bool) -> Self {
        self.cli_code = code;
        self
    }

    pub fn cli_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.cli_prompt = prompt.into();
        self
    }

    pub fn cli_ignore_agents(mut self, ignore: bool) -> Self {
        self.cli_ignore_agents = ignore;
        self
    }

    pub fn cli_soulless(mut self, soulless: bool) -> Self {
        self.cli_soulless = soulless;
        self
    }

    pub fn plain(mut self, plain: Option<bool>) -> Self {
        self.plain = plain;
        self
    }

    /// Build the query context
    pub async fn build(self, settings: &Settings) -> QueryContext {
        let output_flags = super::OutputFlags::resolve(self.plain);

        let config_name = if self.cli_code { "code" } else { "query" };
        let (subcommand_model, subcommand_thinking, subcommand_tools) =
            settings.get_subcommand_config(config_name);

        let model_name = self.cli_model.unwrap_or_else(|| {
            if !subcommand_model.is_empty() {
                subcommand_model
            } else {
                settings.model.default.clone()
            }
        });

        let model_config = user_models::resolve_model_config(&model_name);
        #[allow(deprecated)] // ollama_client() removed in #121 (Consumer Migration)
        let ollama = settings.ollama_client_for_model(&model_config.model_id);
        let capabilities =
            ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

        let use_tools = self.cli_tools || (subcommand_tools && capabilities.tools);
        let use_think = user_models::resolve_think_mode(
            self.cli_think,
            subcommand_thinking,
            model_config.thinking,
            &model_config.model_id,
            capabilities.thinking,
        );

        let agents_md = if !self.cli_ignore_agents {
            crate::context::load_agents_md()
        } else {
            None
        };

        let prompt_name = if self.cli_code && use_tools {
            "code_with_tools".to_string()
        } else if self.cli_code {
            "code".to_string()
        } else if use_tools {
            "tool_user".to_string()
        } else {
            self.cli_prompt.clone()
        };

        let prompt_type = match prompt_name.as_str() {
            "tool_user" => PromptType::ToolUser,
            "code_with_tools" => PromptType::CodeWithTools,
            "code" => PromptType::Code,
            "summarize" => PromptType::Summarize,
            _ => PromptType::Default,
        };

        let project_id = if self.cli_code {
            None
        } else {
            crate::project::get_project_id()
        };

        let skip_persistence = self.cli_code;
        let result = crate::db::init_database_core(
            crate::db::EmbeddingInit {
                provider: ollama.clone(),
                model_name: settings.embedding_model_name().to_string(),
                probe: settings.embedding_probe_enabled(),
            },
            skip_persistence,
            log::log_enabled!(log::Level::Debug),
            None, // Use default database path
        );

        let db = result.db;
        let embedding_client = result.embedding;

        let retrieval_enabled = db.is_some() && embedding_client.is_some();

        let blacklist_set = settings.blacklist_set();

        let system_prompt = build_system_prompt(
            PromptConfig::new(prompt_type)
                .with_model_id(Some(&model_config.model_id))
                .with_blacklist(Some(&blacklist_set))
                .with_agents_md(agents_md.as_deref())
                .with_tools(use_tools)
                .with_retrieval(retrieval_enabled)
                .with_soulless(self.cli_soulless),
        );

        let tool_names = if use_tools {
            crate::tools::get_available_tool_names(settings)
        } else {
            vec![]
        };

        QueryContext {
            model_config,
            capabilities,
            use_tools,
            use_think,
            agents_md,
            db,
            embedding_client,
            project_id,
            tool_names,
            output_flags,
            prompt_type,
            prompt_name,
            system_prompt,
            ollama,
        }
    }
}

impl Default for QueryContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
