//! Query execution module
//!
//! Consolidates common logic for query, legacy query, and chat message handling.

mod context;
mod coordinator;
mod executor;

use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;

use crate::capabilities::ModelCapabilities;
use crate::chat::custom_coordinator::{ChatEvent, CustomCoordinator};
use crate::config::ModelConfig;
use crate::markdown;
use crate::prompts::builder::PromptType;
use crate::retrieval::build_query_context;
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner, suspend_for_print};
use crate::tool_robustness::format_tool_error;

pub use context::QueryContextBuilder;

/// Output flags resolved from CLI and config
#[derive(Debug, Clone, Copy)]
pub struct OutputFlags {
    pub plain: bool,
}

impl OutputFlags {
    pub fn resolve(plain: Option<bool>, _settings: &Settings) -> Self {
        Self {
            plain: plain.unwrap_or(false),
        }
    }
}

/// Result of a query execution
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub content: String,
    pub thinking: Option<String>,
}

/// Context for building a chat coordinator
pub struct ChatContext {
    pub ollama: Ollama,
    pub model_id: String,
    pub model_options: ModelOptions,
    pub use_think: bool,
    pub use_plain: bool,
    pub context_window: Option<usize>,
    pub system_prompt: Option<String>,
}

impl ChatContext {
    pub fn build_coordinator(self) -> CustomCoordinator<Vec<ChatMessage>> {
        let use_think = self.use_think;
        let _use_plain = self.use_plain;

        let mut coordinator = CustomCoordinator::new(self.ollama, self.model_id, vec![])
            .options(self.model_options)
            .think(use_think);

        if let Some(ctx_window) = self.context_window {
            coordinator = coordinator.context_window(ctx_window);
        }

        if let Some(prompt) = self.system_prompt {
            coordinator = coordinator.system_prompt(prompt);
        }

        coordinator
    }
}

/// Handle chat events (pre-tool content, tool calls, tool results)
pub fn handle_chat_event(event: ChatEvent, use_think: bool, use_plain: bool) {
    use crate::chat::{display_thinking, strip_thinking_tags};

    match event {
        ChatEvent::PreToolContent { content, thinking } => {
            suspend_for_print(|| {
                if use_think {
                    display_thinking(&content, thinking.as_ref(), !use_plain);
                }
                if !content.trim().is_empty() {
                    let cleaned = strip_thinking_tags(&content);
                    if !cleaned.trim().is_empty() {
                        if use_plain {
                            println!("{}", cleaned);
                        } else {
                            markdown::print_markdown(&cleaned);
                        }
                    }
                }
            });
        }
        ChatEvent::ToolCall { .. } => {}
        ChatEvent::ToolResult { result, .. } => {
            if !log::log_enabled!(log::Level::Debug) {
                suspend_for_print(|| {
                    let preview = crate::utils::truncate_chars(&result, 100);
                    eprintln!("✓ Result: {}", preview.replace('\n', " "));
                });
            }
        }
        ChatEvent::ContextNearLimit {
            tool_name,
            tokens_used,
            context_window,
        } => {
            log::debug!(
                "[INFO] Context at {:.0}% after tool '{}' ({} / {} tokens)",
                (tokens_used * 100) / context_window,
                tool_name,
                tokens_used,
                context_window
            );
        }
        ChatEvent::ContextTruncated {
            tool_name,
            original_tokens,
            new_tokens,
            context_window,
        } => {
            log::warn!(
                "[WARN] Tool '{}' result truncated ({} → {} tokens) to fit context ({} tokens max)",
                tool_name,
                original_tokens,
                new_tokens,
                context_window
            );
        }
        ChatEvent::ContextNeedsCompaction {
            tokens_used,
            context_window,
            tools_executed,
        } => {
            log::debug!(
                "[INFO] Context needs compaction: {}K / {}K tokens ({} tools executed)",
                tokens_used / 1000,
                context_window / 1000,
                tools_executed.len()
            );
        }
    }
}

/// Print debug information
pub fn print_debug_info(
    model_config: &ModelConfig,
    capabilities: &ModelCapabilities,
    use_tools: bool,
    use_think: bool,
    query: &str,
    prompt_name: &str,
) {
    log::debug!("Debug Mode - Configuration:");
    log::debug!("==========================");
    log::debug!("Model ID:          {}", model_config.model_id);
    if model_config.num_ctx > 0 {
        log::debug!("Context Window:    {}K tokens", model_config.num_ctx / 1024);
    } else {
        log::debug!("Context Window:    auto");
    }
    log::debug!("Temperature:       {}", model_config.temperature);
    if let Some(top_k) = model_config.top_k {
        log::debug!("Top K:             {}", top_k);
    }
    if let Some(top_p) = model_config.top_p {
        log::debug!("Top P:             {}", top_p);
    }
    if let Some(rp) = model_config.repeat_penalty {
        log::debug!("Repeat Penalty:    {}", rp);
    }
    log::debug!("Detected Capabilities:");
    log::debug!("  Tools:      {}", capabilities.tools);
    log::debug!("  Vision:     {}", capabilities.vision);
    log::debug!("  Completion: {}", capabilities.completion);
    log::debug!("  Thinking:   {}", capabilities.thinking);
    log::debug!("Active Configuration:");
    log::debug!("  Tools Enabled:   {}", use_tools);
    log::debug!("  Think Mode:      {}", use_think);
    log::debug!("  Prompt Mode:     {}", prompt_name);
    log::debug!("Query: {}", query);
    log::debug!("==========================");
}

/// Display query result with optional thinking and markdown
pub fn display_result(result: &QueryResult, use_think: bool, use_plain: bool) {
    use crate::chat::{display_thinking, strip_thinking_tags};

    if use_think {
        display_thinking(&result.content, result.thinking.as_ref(), !use_plain);
    }

    let content = strip_thinking_tags(&result.content);

    if use_plain {
        println!("{}", content);
    } else {
        markdown::print_markdown(&content);
    }
}

/// Validate prompt type
fn validate_prompt_type(prompt_type: PromptType, cli_prompt: &str) -> Result<(), String> {
    if matches!(
        prompt_type,
        PromptType::ToolUser
            | PromptType::CodeWithTools
            | PromptType::Code
            | PromptType::Summarize
            | PromptType::Default
    ) {
        Ok(())
    } else {
        Err(format!(
            "Error: Unknown prompt '{}'. Use --list to see available prompts.",
            cli_prompt
        ))
    }
}

/// Run a single query (handles both subcommand and legacy modes)
#[allow(clippy::too_many_arguments)]
pub async fn run_query(
    query: String,
    cli_model: Option<&str>,
    cli_think: bool,
    cli_tools: bool,
    cli_code: bool,
    cli_prompt: &str,
    cli_ignore_agents: bool,
    cli_soulless: bool,
    plain: Option<bool>,
    settings: &Settings,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        std::process::exit(1);
    }

    let ctx = QueryContextBuilder::new()
        .cli_model(cli_model.map(|s| s.to_string()))
        .cli_think(cli_think)
        .cli_tools(cli_tools)
        .cli_code(cli_code)
        .cli_prompt(cli_prompt)
        .cli_ignore_agents(cli_ignore_agents)
        .cli_soulless(cli_soulless)
        .plain(plain)
        .build(settings)
        .await;

    validate_prompt_type(ctx.prompt_type, cli_prompt)?;

    if log::log_enabled!(log::Level::Debug) {
        log::debug!("Debug mode enabled - will log all tool calls and results");
        print_debug_info(
            &ctx.model_config,
            &ctx.capabilities,
            ctx.use_tools,
            ctx.use_think,
            &query,
            &ctx.prompt_name,
        );
        log::debug!("🚀 Executing with debug logging enabled...");
    }

    if ctx.agents_md.is_some() {
        log::debug!("📄 [AGENTS.md] Context injected from current directory");
    }

    let coordinator = coordinator::build_query_coordinator(&ctx, settings);

    let retrieval_config = crate::retrieval::RetrievalConfig::default();
    let context_result = build_query_context(
        ctx.project_id.as_deref(),
        ctx.db.as_ref(),
        ctx.embedding_client.as_ref(),
        &query,
        &ctx.system_prompt,
        &retrieval_config,
        log::log_enabled!(log::Level::Debug),
    )
    .await;

    let messages = context_result.messages;

    let spinner = create_spinner("Waiting for response...");

    let result = executor::execute_query_with_retry(
        coordinator,
        messages,
        ctx.db,
        ctx.embedding_client,
        &ctx.tool_names,
        spinner.clone(),
    )
    .await;

    let response = match result {
        Ok(resp) => resp,
        Err(e) => {
            finish_spinner(spinner);
            log::debug!("❌ Tool execution failed (RAW):\n{:#?}", e);
            if !log::log_enabled!(log::Level::Debug) {
                let error_msg = format_tool_error(&e);
                eprintln!("\n❌ Tool execution failed: {}\n", error_msg);
            }
            return Err(e.into());
        }
    };

    finish_spinner(spinner);

    let result = QueryResult {
        content: response.message.content.clone(),
        thinking: response.message.thinking.clone(),
    };

    display_result(&result, ctx.use_think, ctx.output_flags.plain);

    Ok(())
}
