//! Ask-Ollama: A CLI tool for querying Ollama LLM models
//!
//! This is an evolution of the Python ask-ai.py script, rewritten in Rust
//! with enhanced features including markdown rendering, tool support,
//! model capability detection, and translation support.

mod capabilities;
mod chat;
mod config;
mod context;
mod debug_tools;
mod ocr;
mod prompts;
mod settings;
mod spinner;
mod summarize;
mod tool_robustness;
mod tools;
mod translate;
mod user_models;
mod utils;

use clap::Parser;
use ollama_rs::generation::chat::ChatMessage;
use termimad::print_text;

use crate::capabilities::ModelCapabilities;
use crate::chat::{
    display_thinking, strip_thinking_tags, CustomCoordinator, ChatEvent,
    coordinator::{classify_error_str, format_recovery_message, is_error_str_recoverable, MAX_RETRIES},
};
use crate::config::ModelConfig;
use crate::debug_tools::{enable_debug, log_debug};
use crate::ocr::{OcrArgs, OcrProcessor, print_results};
use crate::prompts::get_prompt_with_blacklist;
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner, suspend_for_print};
use crate::summarize::{SummarizeArgs, SummarizeProcessor};
use crate::tool_robustness::format_tool_error;
use crate::translate::{
    Commands, CompletionArgs, LanguageMapper, QueryArgs, Shell, TranslateArgs, TranslationStyle,
    build_translation_prompt, parse_language_pair,
};

/// Type alias for common Result type
type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// CLI for ask-ai
#[derive(Parser, Debug)]
#[command(
    name = "ask-ai",
    bin_name = "ask-ai",
    about = "CLI tool for querying Ollama LLM models and translating text",
    version,
    subcommand_required = false,
    arg_required_else_help = false
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    // Legacy args for backward compatibility when no subcommand is used
    /// The query to send to the model (used when no subcommand specified)
    #[arg(value_name = "QUERY")]
    query: Option<String>,

    /// Model preset to use (lfm is the default)
    #[arg(short, long, value_name = "MODEL")]
    model: Option<String>,

    /// System prompt mode (default, tool_user)
    #[arg(short, long, default_value = "default", value_name = "PROMPT")]
    prompt: String,

    /// Enable think mode for models that support it
    #[arg(short, long)]
    think: bool,

    /// Output plain text without markdown formatting
    #[arg(long, action = clap::ArgAction::SetTrue)]
    plain: Option<bool>,

    /// Dry-run mode: print config without executing
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    debug: Option<bool>,

    /// List available models and prompts
    #[arg(short, long)]
    list: bool,

    /// Force enable tools even if model doesn't advertise tool support
    #[arg(long)]
    tools: bool,

    /// Code mode: optimize response for code output (minimal explanations)
    #[arg(short, long)]
    code: bool,

    /// Ignore AGENTS.md file if present in current directory
    #[arg(long)]
    ignore_agents: bool,

    /// Initialize/create sample configuration file
    #[arg(long)]
    init_config: bool,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();

    // Handle --init-config before anything else
    if cli.init_config {
        match Settings::create_sample_config() {
            Ok(path) => {
                println!("Configuration file created at: {}", path.display());
                println!("\nEdit this file to customize your Ask-AI settings.");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error creating configuration file: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Load settings from config file
    let settings = Settings::load();

    // Handle subcommands if present
    if let Some(ref command) = cli.command {
        match command {
            Commands::Translate(args) => {
                return handle_translate(args.clone(), &cli, &settings).await;
            }
            Commands::Query(args) => return handle_query(args.clone(), &cli, &settings).await,
            Commands::Ocr(args) => return handle_ocr(args.clone(), &cli, &settings).await,
            Commands::Summarize(args) => {
                return handle_summarize(args.clone(), &cli, &settings).await;
            }
            Commands::Chat(args) => return handle_chat(args.clone(), &settings).await,
            Commands::Completion(args) => return handle_completion(args.clone(), &settings),
        }
    }

    // No subcommand - handle as legacy query mode for backward compatibility
    handle_legacy_query(cli, &settings).await
}

/// Handle translate subcommand
async fn handle_translate(args: TranslateArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Use global CLI flags
    let use_debug = cli.debug.unwrap_or(settings.output.debug_default);
    let use_plain = cli.plain.unwrap_or(settings.output.plain_default);

    if use_debug {
        enable_debug();
        eprintln!("Debug Mode - Translation Configuration:");
        eprintln!("==========================");
        if let Some(lang) = &args.language {
            eprintln!("Language:          {}", lang);
        }
        if let Some(text) = &args.text {
            let preview = if text.len() > 50 {
                format!("{}...", &text[..50])
            } else {
                text.clone()
            };
            eprintln!("Text:              {}", preview);
        }
        eprintln!("==========================");
        eprintln!("\n🚀 Executing translation with debug logging enabled...\n");
    }

    let mapper = LanguageMapper::new();

    // Handle --list
    if let Some(filter) = args.list {
        print_supported_languages(&mapper, filter.as_deref());
        return Ok(());
    }

    // Parse language pair
    let language_str = args.language.as_ref().unwrap();
    let (source, target) = match parse_language_pair(language_str, &mapper) {
        Ok((src, tgt)) => (src, tgt),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Get text to translate
    let text = if let Some(text) = args.text {
        text
    } else {
        // Read from stdin
        match crate::utils::read_stdin() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("Usage: ask translate LANGUAGE \"text to translate\"");
                eprintln!("   or: echo \"text\" | ask translate LANGUAGE");
                std::process::exit(1);
            }
        }
    };

    if text.is_empty() {
        eprintln!("Error: No text provided for translation.");
        eprintln!("Usage: ask translate LANGUAGE \"text to translate\"");
        eprintln!("   or: echo \"text\" | ask translate LANGUAGE");
        std::process::exit(1);
    }

    // Parse style if provided
    let style = args.prompt.as_ref().map(|s| TranslationStyle::from_str(s));

    // Build translation prompt
    let prompt = build_translation_prompt(source.as_ref(), &target, &text, style.as_ref());

    // Get translate model config
    let model_config = match user_models::get_model_config("translate") {
        Some(cfg) => cfg,
        None => {
            eprintln!("Error: Translate model configuration not found.");
            std::process::exit(1);
        }
    };

    // Initialize Ollama client with settings
    let ollama = settings.ollama_client();

    let model_options = model_config.build_model_options();

    // Build coordinator - no tools for translation
    let mut coordinator =
        CustomCoordinator::new(ollama, model_config.model_id.clone(), vec![]).options(model_options);

    // Create messages - use system prompt for translation instructions
    let system_message = ChatMessage::system(prompt);
    let user_message = ChatMessage::user("".to_string()); // Empty user message since text is in system

    // Show spinner
    let spinner = create_spinner("Translating...");

    // Send request
    let response = coordinator
        .chat(vec![system_message, user_message])
        .await
        .map_err(|e| format!("Failed to get translation: {}", e))?;

    // Clear spinner
    finish_spinner(spinner);

    // Get translated text
    let translated = response.message.content.trim();

    // Output - respect --plain flag for markdown rendering
    if use_plain {
        println!("{}", translated);
    } else {
        print_text(translated);
    }

    Ok(())
}

/// Handle query subcommand
async fn handle_query(args: QueryArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    let query = args.get_query()?;

    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        std::process::exit(1);
    }

    // Use global CLI flags
    let code_mode = cli.code;
    let use_tools = cli.tools;
    let use_think = cli.think;
    let ignore_agents = cli.ignore_agents;

    // Determine which subcommand config to use - "code" if --code flag is set
    let config_name = if code_mode { "code" } else { "query" };

    // Get subcommand configuration from settings
    let (subcommand_model, subcommand_thinking, subcommand_tools) =
        settings.get_subcommand_config(config_name);

    // Get model configuration - priority: global CLI > subcommand config > global default
    let model_name = if let Some(ref m) = cli.model {
        m.clone()
    } else if !subcommand_model.is_empty() {
        subcommand_model
    } else {
        settings.model.default.clone()
    };

    let model_config = user_models::resolve_model_config(&model_name);

    // Initialize Ollama client with settings
    let ollama = settings.ollama_client();

    // Detect model capabilities (query command)
    let capabilities = ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

    // Determine if tools should be enabled
    let use_tools_final = use_tools || (subcommand_tools && capabilities.tools);

    // Determine if think mode should be enabled
    let use_think_final = user_models::resolve_think_mode(
        use_think,
        subcommand_thinking,
        model_config.thinking,
        &model_config.model_id,
        capabilities.thinking,
    );

    // Load AGENTS.md context if available and not ignored
    let agents_md = if !ignore_agents {
        crate::context::load_agents_md()
    } else {
        None
    };

    // Use global CLI flags
    let use_debug = cli.debug.unwrap_or(settings.output.debug_default);
    let use_plain = cli.plain.unwrap_or(settings.output.plain_default);

    if use_debug && agents_md.is_some() {
        eprintln!("📄 [AGENTS.md] Context injected from current directory");
    }

    // Get system prompt with blacklist filtering
    // Default is now tool_user, code mode can also use tools
    let prompt_name = if code_mode && use_tools_final {
        "code_with_tools"
    } else if code_mode {
        "code"
    } else if use_tools_final {
        "tool_user"
    } else {
        &cli.prompt
    };

    // Get the blacklist set to filter tools from the prompt
    let blacklist_set = settings.blacklist_set();

    let system_prompt = match get_prompt_with_blacklist(
        prompt_name,
        Some(&model_config.model_id),
        Some(&blacklist_set),
        agents_md.as_deref(),
    ) {
        Some(prompt) => prompt,
        None => {
            eprintln!(
                "Error: Unknown prompt '{}'. Use --list to see available prompts.",
                cli.prompt
            );
            std::process::exit(1);
        }
    };

    // Handle debug mode - now executes with full logging instead of dry-run
    if use_debug {
        enable_debug();
        log_debug("Debug mode enabled - will log all tool calls and results");
        print_debug_info(
            &model_config,
            &capabilities,
            use_tools_final,
            use_think_final,
            &query,
            prompt_name,
        );
        eprintln!("\n🚀 Executing with debug logging enabled...\n");
        // Don't return - continue with execution
    }

    let model_options = model_config.build_model_options();

    // Build coordinator with event callback for pre-tool content
    let use_plain_final = use_plain;
    // Note: use_think_final is already defined above
    let mut coordinator = CustomCoordinator::new(ollama, model_config.model_id.clone(), vec![])
        .options(model_options)
        .think(use_think_final)
        .on_event(move |event| {
            match event {
                ChatEvent::PreToolContent { content, thinking } => {
                    // Show pre-tool content (thinking/intro text before tool calls)
                    suspend_for_print(|| {
                        if use_think_final {
                            display_thinking(&content, thinking.as_ref(), !use_plain_final);
                        }
                        if !content.trim().is_empty() {
                            let cleaned = strip_thinking_tags(&content);
                            if !cleaned.trim().is_empty() {
                                if use_plain_final {
                                    println!("{}", cleaned);
                                } else {
                                    print_text(&cleaned);
                                }
                            }
                        }
                    });
                }
                ChatEvent::ToolCall { .. } => {
                    // Tool call logging is handled by debug_tools.rs
                    // which shows the tool name with parameters
                }
                ChatEvent::ToolResult { result, .. } => {
                    // In normal mode, show abbreviated result
                    // In debug mode, debug_tools.rs shows detailed result
                    if !use_debug {
                        suspend_for_print(|| {
                            let preview = if result.len() > 100 {
                                format!("{}...", &result[..100])
                            } else {
                                result.clone()
                            };
                            eprintln!("\x1B[90m✓ Result: {}\x1B[0m", preview.replace('\n', " "));
                        });
                    }
                }
                ChatEvent::FinalResponse(_) => {
                    // Final response is handled after coordinator.chat() returns
                }
            }
        });

    // Add tools if enabled
    if use_tools_final {
        eprintln!("🔧 [Tools] Tools enabled - will log when called");
        let (coordinator_new, tool_count) = crate::tools::register_tools(coordinator, settings, use_debug);
        coordinator = coordinator_new;
        if use_debug {
            eprintln!("   -> {} tools active", tool_count);
        }
    } else {
        eprintln!("⚠️  [Tools] No tools enabled for this model");
    }

    // Get available tool names for error messages
    let tool_names: Vec<String> = if use_tools_final {
        crate::tools::get_available_tool_names(settings)
    } else {
        vec![]
    };

    // Create messages
    let system_message = ChatMessage::system(system_prompt.to_string());
    let user_message = ChatMessage::user(query);

    // Show spinner
    let spinner = create_spinner("Waiting for response...");

    // Send request with retry logic for recoverable errors
    let mut attempts = 0;
    let mut messages = vec![system_message, user_message];
    let result = loop {
        let current_result = coordinator.chat(messages.clone()).await;

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                let error_str = e.to_string();

                // Check if error is recoverable using string matching
                if is_error_str_recoverable(&error_str) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_error_str(&error_str, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if use_debug {
                        log_debug(&format!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts, MAX_RETRIES, recovery_err.description()
                        ));
                    }

                    // Add error as tool message and retry
                    messages.push(ChatMessage::tool(error_msg));

                    // Show brief retry indicator
                    if attempts == 1 {
                        finish_spinner(spinner.clone());
                        eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                    }

                    continue;
                } else {
                    // Max retries or non-recoverable
                    break Err(error_str);
                }
            }
        }
    };

    // Handle result with better error messages
    let response = match result {
        Ok(resp) => resp,
        Err(e) => {
            finish_spinner(spinner);
            // In debug mode, show raw error with pretty printing
            if crate::debug_tools::is_debug_enabled() {
                eprintln!("\n❌ Tool execution failed (RAW):\n{:#?}\n", e);
            } else {
                let error_msg = format_tool_error(&e);
                eprintln!("\n❌ Tool execution failed: {}\n", error_msg);
            }
            std::process::exit(1);
        }
    };

    finish_spinner(spinner);

    // Show thinking if present and think mode is enabled
    if use_think {
        display_thinking(&response.message.content, response.message.thinking.as_ref(), !use_plain);
    }

    // Strip thinking tags from content
    let content = strip_thinking_tags(&response.message.content);

    // Render output
    if use_plain {
        println!("{}", content);
    } else {
        print_text(&content);
    }

    Ok(())
}

/// Handle legacy query mode (backward compatibility)
async fn handle_legacy_query(cli: Cli, settings: &Settings) -> AppResult<()> {
    // Handle --list flag
    if cli.list {
        print_available_options();
        return Ok(());
    }

    // Get query from args or stdin
    let query = get_query_legacy(&cli)?;
    if query.is_empty() {
        eprintln!("Error: No query provided. Use positional argument or pipe input.");
        eprintln!("Try 'ask-ai --help' for usage information.");
        std::process::exit(1);
    }

    // Determine which subcommand config to use - "code" if --code flag is set
    let config_name = if cli.code { "code" } else { "query" };

    // Get subcommand configuration from settings
    let (subcommand_model, subcommand_thinking, subcommand_tools) =
        settings.get_subcommand_config(config_name);

    // Get model configuration - priority: CLI > subcommand config > global default > built-in
    let model_name = if let Some(ref m) = cli.model {
        m.clone()
    } else if !subcommand_model.is_empty() {
        subcommand_model
    } else {
        settings.model.default.clone()
    };

    let model_config = user_models::resolve_model_config(&model_name);

    // Initialize Ollama client with settings
    let ollama = settings.ollama_client();

    // Detect model capabilities (code/summarize commands)
    let capabilities = ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

    // Determine if tools should be enabled
    let use_tools = cli.tools || (subcommand_tools && capabilities.tools);

    // Determine if think mode should be enabled
    let use_think = user_models::resolve_think_mode(
        cli.think,
        subcommand_thinking,
        model_config.thinking,
        &model_config.model_id,
        capabilities.thinking,
    );

    // Get system prompt with blacklist filtering
    // Default is now tool_user, code mode can also use tools
    let prompt_name = if cli.code && use_tools {
        "code_with_tools"
    } else if cli.code {
        "code"
    } else if use_tools {
        "tool_user"
    } else {
        &cli.prompt
    };

    // Get the blacklist set to filter tools from the prompt
    let blacklist_set = settings.blacklist_set();

    // Use CLI if specified, otherwise use config setting
    let use_debug = cli.debug.unwrap_or(settings.output.debug_default);
    let use_plain = cli.plain.unwrap_or(settings.output.plain_default);

    // Load AGENTS.md context if available and not ignored (legacy mode)
    let agents_md = if !cli.ignore_agents {
        crate::context::load_agents_md()
    } else {
        None
    };

    if use_debug && agents_md.is_some() {
        eprintln!("📄 [AGENTS.md] Context injected from current directory");
    }

    let system_prompt = match get_prompt_with_blacklist(
        prompt_name,
        Some(&model_config.model_id),
        Some(&blacklist_set),
        agents_md.as_deref(),
    ) {
        Some(prompt) => prompt,
        None => {
            eprintln!(
                "Error: Unknown prompt '{}'. Use --list to see available prompts.",
                cli.prompt
            );
            std::process::exit(1);
        }
    };

    // Handle debug mode - now executes with full logging instead of dry-run
    if use_debug {
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
        // Don't return - continue with execution
    }

    let model_options = model_config.build_model_options();

    // Build coordinator with event callback for pre-tool content
    let use_plain_final = use_plain;
    let use_think_final = use_think;
    let mut coordinator = CustomCoordinator::new(ollama, model_config.model_id.clone(), vec![])
        .options(model_options)
        .think(use_think_final)
        .on_event(move |event| {
            match event {
                ChatEvent::PreToolContent { content, thinking } => {
                    // Show pre-tool content (thinking/intro text before tool calls)
                    suspend_for_print(|| {
                        if use_think_final {
                            display_thinking(&content, thinking.as_ref(), !use_plain_final);
                        }
                        if !content.trim().is_empty() {
                            let cleaned = strip_thinking_tags(&content);
                            if !cleaned.trim().is_empty() {
                                if use_plain_final {
                                    println!("{}", cleaned);
                                } else {
                                    print_text(&cleaned);
                                }
                            }
                        }
                    });
                }
                ChatEvent::ToolCall { .. } => {
                    // Tool call logging is handled by debug_tools.rs
                    // which shows the tool name with parameters
                }
                ChatEvent::ToolResult { result, .. } => {
                    // In normal mode, show abbreviated result
                    // In debug mode, debug_tools.rs shows detailed result
                    if !use_debug {
                        suspend_for_print(|| {
                            let preview = if result.len() > 100 {
                                format!("{}...", &result[..100])
                            } else {
                                result.clone()
                            };
                            eprintln!("\x1B[90m✓ Result: {}\x1B[0m", preview.replace('\n', " "));
                        });
                    }
                }
                ChatEvent::FinalResponse(_) => {
                    // Final response is handled after coordinator.chat() returns
                }
            }
        });

    // Add tools if enabled
    if use_tools {
        // Only show in debug mode
        if use_debug {
            eprintln!("🔧 [Tools] Tools enabled - will log when called");
        }
        let (coordinator_new, tool_count) = crate::tools::register_tools(coordinator, settings, use_debug);
        coordinator = coordinator_new;
        if use_debug {
            eprintln!("   -> {} tools active", tool_count);
        }
    } else if use_debug {
        eprintln!("⚠️  [Tools] No tools enabled for this model");
    }

    // Get available tool names for error messages
    let tool_names: Vec<String> = if use_tools {
        crate::tools::get_available_tool_names(settings)
    } else {
        vec![]
    };

    // Create messages
    let system_message = ChatMessage::system(system_prompt.to_string());
    let user_message = ChatMessage::user(query);

    // Show spinner
    let spinner = create_spinner("Waiting for response...");

    // Send request with retry logic for recoverable errors
    let mut attempts = 0;
    let mut messages = vec![system_message, user_message];
    let result = loop {
        let current_result = coordinator.chat(messages.clone()).await;

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                let error_str = e.to_string();

                // Check if error is recoverable using string matching
                if is_error_str_recoverable(&error_str) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_error_str(&error_str, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if use_debug {
                        log_debug(&format!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts, MAX_RETRIES, recovery_err.description()
                        ));
                    }

                    // Add error as tool message and retry
                    messages.push(ChatMessage::tool(error_msg));

                    // Show brief retry indicator
                    if attempts == 1 {
                        finish_spinner(spinner.clone());
                        eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                    }

                    continue;
                } else {
                    // Max retries or non-recoverable
                    break Err(error_str);
                }
            }
        }
    };

    // Handle result with better error messages
    let response = match result {
        Ok(resp) => resp,
        Err(e) => {
            finish_spinner(spinner);
            // In debug mode, show raw error with pretty printing
            if crate::debug_tools::is_debug_enabled() {
                eprintln!("\n❌ Tool execution failed (RAW):\n{:#?}\n", e);
            } else {
                let error_msg = format_tool_error(&e);
                eprintln!("\n❌ Tool execution failed: {}\n", error_msg);
            }
            std::process::exit(1);
        }
    };

    finish_spinner(spinner);

    // Show thinking if present and think mode is enabled
    if use_think {
        display_thinking(&response.message.content, response.message.thinking.as_ref(), !use_plain);
    }

    // Strip thinking tags from content
    let content = strip_thinking_tags(&response.message.content);

    // Render output
    if use_plain {
        println!("{}", content);
    } else {
        print_text(&content);
    }

    Ok(())
}

/// Print supported languages for translation
fn print_supported_languages(mapper: &LanguageMapper, filter: Option<&str>) {
    let languages = mapper.list(filter);

    if languages.is_empty() {
        if let Some(f) = filter {
            println!("No languages found matching '{}'", f);
        } else {
            println!("No languages available.");
        }
        return;
    }

    if let Some(f) = filter {
        println!("Languages matching '{}':", f);
    } else {
        println!("Supported languages (use code or name):");
    }
    println!();

    // Group by language family for better display
    let mut current_family = String::new();

    for lang in languages {
        // Simple grouping by first two letters of code
        let family = lang
            .code
            .split('-')
            .next()
            .unwrap_or(&lang.code)
            .to_string();

        if family != current_family {
            if !current_family.is_empty() {
                println!();
            }
            current_family = family;
        }

        let aliases_str = if lang.aliases.is_empty() {
            String::new()
        } else {
            format!(" [aliases: {}]", lang.aliases.join(", "))
        };

        println!("  {:<15} - {}{}", lang.code, lang.name, aliases_str);
    }

    println!();
    println!("Usage examples:");
    println!("  ask translate en:pt \"Hello\"        # English to Portuguese");
    println!("  ask translate :pt \"Hello\"          # Auto-detect to Portuguese");
    println!("  ask translate pt \"Hello\"           # Auto-detect to Portuguese");
    println!("  ask translate he:en \"שלום\"        # Hebrew to English");
    println!("  ask translate en:br \"Hello\"        # English to Brazilian Portuguese");
    println!();
    println!("Tip: Use ambiguous codes like 'zh' or 'pt' for specific variants:");
    println!("  zh-Hans = Chinese Simplified, zh-Hant = Chinese Traditional");
    println!("  pt-BR = Brazilian Portuguese, pt-PT = European Portuguese");
}

/// Print available models and prompts
fn print_available_options() {
    println!("Available models:");
    for name in user_models::list_all_model_names() {
        if let Some(config) = user_models::get_model_config(&name) {
            let default_marker = if name == "llama3.1" { " (default)" } else { "" };
            let user_marker = if !ModelConfig::is_builtin_valid(&name) {
                " [user]"
            } else {
                ""
            };
            println!(
                "  {:20} - {} ({}K context){}{}",
                name,
                config.model_id,
                config.num_ctx / 1024,
                default_marker,
                user_marker
            );
        }
    }

    println!("\nAvailable prompts:");
    for name in prompts::list_prompts() {
        let default_marker = if name == "default" { " (default)" } else { "" };
        let special_marker = if name == "pepe" {
            " (Easter egg: Pepe personality)"
        } else {
            ""
        };
        println!("  {:20}{}{}", name, default_marker, special_marker);
    }

    println!("\nSubcommands:");
    println!("  translate [en:pt] TEXT  Translate text between languages");
    println!("  query QUERY             Query an LLM (default if no subcommand)");
    println!();
    println!("Examples:");
    println!("  ask \"What is Rust?\"");
    println!("  ask translate en:pt \"Hello world\"");
    println!("  ask -m lfm \"Explain async/await\"");
    println!("  ask translate --list");
    println!("  ask translate --list port");
}

/// Get query from CLI args or stdin (legacy mode)
fn get_query_legacy(cli: &Cli) -> AppResult<String> {
    // First check if query was provided as positional argument
    if let Some(ref query) = cli.query {
        return Ok(query.trim().to_string());
    }

    // Otherwise, try to read from stdin
    use std::io::{self, Read};
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    Ok(input.trim().to_string())
}

/// Print debug information
fn print_debug_info(
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

/// Handle OCR subcommand
async fn handle_ocr(args: OcrArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    // Validate arguments
    if let Err(e) = args.validate() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Use global CLI flags
    let use_debug = cli.debug.unwrap_or(settings.output.debug_default);

    if use_debug {
        enable_debug();
        eprintln!("Debug Mode - OCR Configuration:");
        eprintln!("==========================");
        eprintln!("Model ID:          glm-ocr:bf16");
        eprintln!("Mode:              {:?}", args.mode);
        eprintln!("Max Tokens:        {}", args.max_tokens);
        eprintln!("JSON Output:       {}", args.json);
        eprintln!("Files:             {:?}", args.files);
        eprintln!("==========================");
        eprintln!("\n🚀 Executing OCR with debug logging enabled...\n");
    }

    let processor = OcrProcessor::new();

    // Process files
    let results = match processor.process_batch(&args, settings).await {
        Ok(results) => results,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Print results
    print_results(&results, args.json);

    Ok(())
}

/// Handle chat subcommand
async fn handle_chat(args: chat::ChatArgs, settings: &Settings) -> AppResult<()> {
    chat::run_chat_repl(settings, &args).await
}

/// Handle summarize subcommand
async fn handle_summarize(args: SummarizeArgs, cli: &Cli, settings: &Settings) -> AppResult<()> {
    // Get subcommand configuration from settings
    let (subcommand_model, _subcommand_thinking, _subcommand_tools) =
        settings.get_subcommand_config("summarize");

    // Determine model to use following precedence:
    // 1. Global CLI argument
    // 2. Subcommand-specific config from settings
    // 3. Global default from settings
    let model_id = if let Some(ref m) = cli.model {
        // User specified model via global flag
        m.clone()
    } else if !subcommand_model.is_empty() {
        // Use subcommand-specific model from config
        subcommand_model
    } else {
        // Use global default from settings (or built-in default)
        settings.model.default.clone()
    };

    // Use global CLI flags
    let use_debug = cli.debug.unwrap_or(settings.output.debug_default);
    let use_plain = cli.plain.unwrap_or(settings.output.plain_default);

    if use_debug {
        enable_debug();
        eprintln!("Debug Mode - Summarize Configuration:");
        eprintln!("==========================");
        eprintln!("Model ID:          {}", model_id);
        eprintln!("Max Length:        {} words", args.max_length);
        eprintln!("Format:            {:?}", args.format);
        eprintln!("Style:             {:?}", args.style);
        eprintln!("Plain Output:      {}", use_plain);
        eprintln!("==========================");
        eprintln!("\n🚀 Executing summarization with debug logging enabled...\n");
    }

    // Get text from args or stdin (read once here)
    let text = if let Some(ref text) = args.text {
        text.clone()
    } else {
        // Read from stdin
        match crate::utils::read_stdin() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("Usage: ask summarize [OPTIONS] <TEXT>");
                eprintln!("   or: echo \"text\" | ask summarize");
                std::process::exit(1);
            }
        }
    };

    let processor = SummarizeProcessor::new();

    // Process summarization with the text already loaded, passing the determined model_id
    match processor.summarize(&args, &text, &model_id, settings).await {
        Ok(summary) => {
            // Render output with markdown if not --plain
            if use_plain {
                println!("{}", summary);
            } else {
                print_text(&summary);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Handle completion subcommand
fn handle_completion(args: CompletionArgs, _settings: &Settings) -> AppResult<()> {
    use clap::CommandFactory;
    use std::io::stdout;

    let cmd = Cli::command();
    let name = cmd.get_name().to_string();

    match args.shell {
        Shell::Bash => clap_complete::generate(
            clap_complete::Shell::Bash,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::Zsh => clap_complete::generate(
            clap_complete::Shell::Zsh,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::Fish => clap_complete::generate(
            clap_complete::Shell::Fish,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::PowerShell => clap_complete::generate(
            clap_complete::Shell::PowerShell,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
        Shell::Elvish => clap_complete::generate(
            clap_complete::Shell::Elvish,
            &mut cmd.clone(),
            &name,
            &mut stdout(),
        ),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_thinking_tags() {
        let input = "<think>This is thinking</think>Response.";
        let expected = "Response.";
        assert_eq!(strip_thinking_tags(input), expected);

        let input_no_think = "Just a normal response.";
        assert_eq!(
            strip_thinking_tags(input_no_think),
            "Just a normal response."
        );

        let input_multiline = "<think>\nThinking...\n</think>\n\nFinal answer.";
        let expected_multiline = "Final answer.";
        assert_eq!(strip_thinking_tags(input_multiline), expected_multiline);

        let input_upper = "<THINK>Thinking...</THINK>Response.";
        let expected_upper = "Response.";
        assert_eq!(strip_thinking_tags(input_upper), expected_upper);

        let input_multi = "<think>First</think>Part 1. <think>Second</think>Part 2.";
        let expected_multi = "Part 1. Part 2.";
        assert_eq!(strip_thinking_tags(input_multi), expected_multi);
    }
}
