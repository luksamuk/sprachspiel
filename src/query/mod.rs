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
use crate::debug_tools::{enable_debug, log_debug};
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
    pub debug: bool,
    pub plain: bool,
}

impl OutputFlags {
    pub fn resolve(debug: Option<bool>, plain: Option<bool>, settings: &Settings) -> Self {
        Self {
            debug: debug.unwrap_or(settings.output.debug_default),
            plain: plain.unwrap_or(settings.output.plain_default),
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
    pub use_debug: bool,
    pub use_plain: bool,
    pub context_window: Option<usize>,
    pub system_prompt: Option<String>,
}

impl ChatContext {
    pub fn build_coordinator(self) -> CustomCoordinator<Vec<ChatMessage>> {
        let use_think = self.use_think;
        let use_plain = self.use_plain;
        let use_debug = self.use_debug;

        let mut coordinator = CustomCoordinator::new(self.ollama, self.model_id, vec![])
            .options(self.model_options)
            .think(use_think)
            .debug(use_debug)
            .on_event(move |event| {
                handle_chat_event(event, use_think, use_plain, use_debug);
            });

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
pub fn handle_chat_event(event: ChatEvent, use_think: bool, use_plain: bool, use_debug: bool) {
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
            if !use_debug {
                suspend_for_print(|| {
                    let preview = crate::utils::truncate_chars(&result, 100);
                    eprintln!("\x1B[90m✓ Result: {}\x1B[0m", preview.replace('\n', " "));
                });
            }
        }
        ChatEvent::ContextNearLimit {
            tool_name,
            tokens_used,
            context_window,
        } => {
            if use_debug {
                eprintln!(
                    "\x1B[33m[INFO] Context at {:.0}% after tool '{}' ({} / {} tokens)\x1B[0m",
                    (tokens_used * 100) / context_window,
                    tool_name,
                    tokens_used,
                    context_window
                );
            }
        }
        ChatEvent::ContextTruncated {
            tool_name,
            original_tokens,
            new_tokens,
            context_window,
        } => {
            eprintln!(
                "\x1B[33m[WARN] Tool '{}' result truncated ({} → {} tokens) to fit context ({} tokens max)\x1B[0m",
                tool_name, original_tokens, new_tokens, context_window
            );
        }
        ChatEvent::ContextNeedsCompaction {
            tokens_used,
            context_window,
            tools_executed,
        } => {
            if use_debug {
                eprintln!(
                    "\x1B[33m[INFO] Context needs compaction: {}K / {}K tokens ({} tools executed)\x1B[0m",
                    tokens_used / 1000,
                    context_window / 1000,
                    tools_executed.len()
                );
            }
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
    println!("Debug Mode - Configuration:");
    println!("==========================");
    println!("Model ID:          {}", model_config.model_id);
    if model_config.num_ctx > 0 {
        println!("Context Window:    {}K tokens", model_config.num_ctx / 1024);
    } else {
        println!("Context Window:    auto");
    }
    println!("Temperature:       {}", model_config.temperature);
    if let Some(top_k) = model_config.top_k {
        println!("Top K:             {}", top_k);
    }
    if let Some(top_p) = model_config.top_p {
        println!("Top P:             {}", top_p);
    }
    if let Some(rp) = model_config.repeat_penalty {
        println!("Repeat Penalty:    {}", rp);
    }
    println!();
    println!("Detected Capabilities:");
    println!("  Tools:      {}", capabilities.tools);
    println!("  Vision:     {}", capabilities.vision);
    println!("  Completion: {}", capabilities.completion);
    println!("  Thinking:   {}", capabilities.thinking);
    println!();
    println!("Active Configuration:");
    println!("  Tools Enabled:   {}", use_tools);
    println!("  Think Mode:      {}", use_think);
    println!("  Prompt Mode:     {}", prompt_name);
    println!();
    println!("Query: {}", query);
    println!("==========================");
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
    debug: Option<bool>,
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
        .debug(debug)
        .plain(plain)
        .build(settings)
        .await;

    validate_prompt_type(ctx.prompt_type, cli_prompt)?;

    if ctx.output_flags.debug {
        enable_debug();
        log_debug("Debug mode enabled - will log all tool calls and results");
        print_debug_info(
            &ctx.model_config,
            &ctx.capabilities,
            ctx.use_tools,
            ctx.use_think,
            &query,
            &ctx.prompt_name,
        );
        eprintln!("\n🚀 Executing with debug logging enabled...\n");
    }

    if ctx.output_flags.debug && ctx.agents_md.is_some() {
        eprintln!("📄 [AGENTS.md] Context injected from current directory");
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
        ctx.output_flags.debug,
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
        ctx.output_flags.debug,
    )
    .await;

    let response = match result {
        Ok(resp) => resp,
        Err(e) => {
            finish_spinner(spinner);
            if crate::debug_tools::is_debug_enabled() {
                eprintln!("\n❌ Tool execution failed (RAW):\n{:#?}\n", e);
            } else {
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
