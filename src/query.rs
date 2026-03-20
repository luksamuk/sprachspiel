//! Query execution module
//!
//! Consolidates common logic for query, legacy query, and chat message handling.

use std::sync::Arc;

use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;
use ollama_rs::Ollama;

use crate::capabilities::ModelCapabilities;
use crate::chat::{
    coordinator::{
        classify_ollama_error, format_recovery_message, is_ollama_error_recoverable, MAX_RETRIES,
    },
    custom_coordinator::{ChatEvent, CustomCoordinator},
    display_thinking, strip_thinking_tags,
};
use crate::config::ModelConfig;
use crate::db::Database;
use crate::debug_tools::{enable_debug, log_debug};
use crate::embeddings::EmbeddingClient;
use crate::markdown;
use crate::project::get_project_id;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::retrieval::{RetrievalConfig, build_query_context};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner, suspend_for_print};
use crate::tool_robustness::format_tool_error;
use crate::tools::{get_available_tool_names, register_tools};
use crate::user_models;

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
    /// Context window size for overflow detection
    pub context_window: Option<usize>,
    /// System prompt for token estimation
    pub system_prompt: Option<String>,
}

impl ChatContext {
    /// Build a coordinator with event callbacks for tool execution
    pub fn build_coordinator(self) -> CustomCoordinator<Vec<ChatMessage>> {
        let use_think = self.use_think;
        let use_plain = self.use_plain;
        let use_debug = self.use_debug;

        let mut coordinator = CustomCoordinator::new(self.ollama, self.model_id, vec![])
            .options(self.model_options)
            .think(use_think)
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
        ChatEvent::ToolCall { .. } => {
            // Tool call logging is handled by debug_tools.rs
        }
        ChatEvent::ToolResult { result, .. } => {
            if !use_debug {
                suspend_for_print(|| {
                    let preview = crate::utils::truncate_chars(&result, 100);
                    eprintln!("\x1B[90m✓ Result: {}\x1B[0m", preview.replace('\n', " "));
                });
            }
        }
        ChatEvent::ContextNearLimit { tool_name, tokens_used, context_window } => {
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
        ChatEvent::ContextTruncated { tool_name, original_tokens, new_tokens, context_window } => {
            eprintln!(
                "\x1B[33m[WARN] Tool '{}' result truncated ({} → {} tokens) to fit context ({} tokens max)\x1B[0m",
                tool_name,
                original_tokens,
                new_tokens,
                context_window
            );
        }
        ChatEvent::ContextNeedsCompaction { tokens_used, context_window, tools_executed } => {
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

    let output_flags = OutputFlags::resolve(debug, plain, settings);

    let config_name = if cli_code { "code" } else { "query" };
    let (subcommand_model, subcommand_thinking, subcommand_tools) =
        settings.get_subcommand_config(config_name);

    let model_name = if let Some(m) = cli_model {
        m.to_string()
    } else if !subcommand_model.is_empty() {
        subcommand_model
    } else {
        settings.model.default.clone()
    };

    let model_config = user_models::resolve_model_config(&model_name);
    let ollama = settings.ollama_client();
    let capabilities = ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

    let use_tools = cli_tools || (subcommand_tools && capabilities.tools);
    let use_think = user_models::resolve_think_mode(
        cli_think,
        subcommand_thinking,
        model_config.thinking,
        &model_config.model_id,
        capabilities.thinking,
    );

    let agents_md = if !cli_ignore_agents {
        crate::context::load_agents_md()
    } else {
        None
    };

    let prompt_name = if cli_code && use_tools {
        "code_with_tools"
    } else if cli_code {
        "code"
    } else if use_tools {
        "tool_user"
    } else {
        cli_prompt
    };

    let blacklist_set = settings.blacklist_set();

    // Determine prompt type for build_system_prompt
    let prompt_type = match prompt_name {
        "tool_user" => PromptType::ToolUser,
        "code_with_tools" => PromptType::CodeWithTools,
        "code" => PromptType::Code,
        "summarize" => PromptType::Summarize,
        _ => PromptType::Default,
    };

    // Skip DB/retrieval for --code mode (no project context needed)
    let project_id = if cli_code { None } else { get_project_id() };

    let (db, embedding_client) = if cli_code {
        (None, None)
    } else {
        match Database::new() {
            Ok(db) => {
                let embedding = Arc::new(EmbeddingClient::new(ollama.clone()));
                (Some(Arc::new(db)), Some(embedding))
            }
            Err(_) => (None, None),
        }
    };

    let retrieval_enabled = db.is_some() && embedding_client.is_some();

    let system_prompt = build_system_prompt(
        PromptConfig::new(prompt_type)
            .with_model_id(Some(&model_config.model_id))
            .with_blacklist(Some(&blacklist_set))
            .with_agents_md(agents_md.as_deref())
            .with_tools(use_tools)
            .with_retrieval(retrieval_enabled)
            .with_soulless(cli_soulless),
    );

    // Validate prompt type (only for legacy prompt names)
    if !matches!(
        prompt_type,
        PromptType::ToolUser
            | PromptType::CodeWithTools
            | PromptType::Code
            | PromptType::Summarize
            | PromptType::Default
    ) {
        eprintln!(
            "Error: Unknown prompt '{}'. Use --list to see available prompts.",
            cli_prompt
        );
        std::process::exit(1);
    }

    if output_flags.debug {
        enable_debug();
        log_debug("Debug mode enabled - will log all tool calls and results");
        print_debug_info(
            &model_config,
            &capabilities,
            use_tools,
            use_think,
            &query,
            prompt_name,
        );
        eprintln!("\n🚀 Executing with debug logging enabled...\n");
    }

    if output_flags.debug && agents_md.is_some() {
        eprintln!("📄 [AGENTS.md] Context injected from current directory");
    }

    let model_options = model_config.build_model_options();

    let coordinator = ChatContext {
        ollama,
        model_id: model_config.model_id.clone(),
        model_options,
        use_think,
        use_debug: output_flags.debug,
        use_plain: output_flags.plain,
        context_window: Some(model_config.num_ctx as usize),
        system_prompt: Some(system_prompt.clone()),
    }
    .build_coordinator();

    // Add tools if enabled
    let mut coordinator = coordinator;
    if use_tools {
        if output_flags.debug {
            eprintln!("🔧 [Tools] Tools enabled - will log when called");
        }
        let (coord_new, tool_count) = register_tools(coordinator, settings, output_flags.debug);
        coordinator = coord_new;
        if output_flags.debug {
            eprintln!("   -> {} tools active", tool_count);
        }
    } else if output_flags.debug {
        eprintln!("⚠️  [Tools] No tools enabled for this model");
    }

    let tool_names: Vec<String> = if use_tools {
        get_available_tool_names(settings)
    } else {
        vec![]
    };

    // Build messages with optional retrieval
    let retrieval_config = RetrievalConfig::default();

    let context_result = build_query_context(
        project_id.as_deref(),
        db.as_ref(),
        embedding_client.as_ref(),
        &query,
        &system_prompt,
        &retrieval_config,
        output_flags.debug,
    )
    .await;

    let messages = context_result.messages;

    // Execute with retry logic
    let spinner = create_spinner("Waiting for response...");

    let mut attempts = 0;
    let mut messages = messages;
    let result = if let (Some(db), Some(embedding)) = (&db, &embedding_client) {
        // Wrap with task-local context for remember tool
        crate::tools::context::with_context(db.clone(), embedding.clone(), async {
            let mut attempts = 0;
            loop {
                let current_result = coordinator.chat(messages.clone()).await;

                match current_result {
                    Ok(response) => break Ok(response),
                    Err(e) => {
                        if is_ollama_error_recoverable(&e) && attempts < MAX_RETRIES {
                            attempts += 1;

                            let recovery_err = classify_ollama_error(&e, &tool_names);
                            let error_msg = format_recovery_message(&recovery_err);

                            if output_flags.debug {
                                log_debug(&format!(
                                    "🔧 [Recovery] Attempt {}/{} - {}",
                                    attempts,
                                    MAX_RETRIES,
                                    recovery_err.description()
                                ));
                            }

                            messages.push(ChatMessage::tool(error_msg));

                            if attempts == 1 {
                                finish_spinner(spinner.clone());
                                eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                            }

                            continue;
                        } else {
                            let error_str = e.to_string();
                            break Err(error_str);
                        }
                    }
                }
            }
        })
        .await
    } else {
        // No DB context, run directly
        loop {
            let current_result = coordinator.chat(messages.clone()).await;

            match current_result {
                Ok(response) => break Ok(response),
                Err(e) => {
                    if is_ollama_error_recoverable(&e) && attempts < MAX_RETRIES {
                        attempts += 1;

                        let recovery_err = classify_ollama_error(&e, &tool_names);
                        let error_msg = format_recovery_message(&recovery_err);

                        if output_flags.debug {
                            log_debug(&format!(
                                "🔧 [Recovery] Attempt {}/{} - {}",
                                attempts,
                                MAX_RETRIES,
                                recovery_err.description()
                            ));
                        }

                        messages.push(ChatMessage::tool(error_msg));

                        if attempts == 1 {
                            finish_spinner(spinner.clone());
                            eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                        }

                        continue;
                    } else {
                        let error_str = e.to_string();
                        break Err(error_str);
                    }
                }
            }
        }
    };

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

    display_result(&result, use_think, output_flags.plain);

    Ok(())
}
