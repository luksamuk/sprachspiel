//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use ollama_rs::coordinator::Coordinator;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::models::ModelOptions;
use rustyline::Config;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use termimad::print_text;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::debug_tools::{enable_debug, log_debug};
use crate::prompts::get_prompt_with_blacklist;
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::tool_robustness::format_tool_error;
use crate::tools::*;

use super::commands::{CommandResult, execute_command, parse_command};
use super::completion::ChatCompleter;
use super::history::{ConversationStorage, get_project_id};
use super::session::ChatSession;
use super::thinking::{process_thinking, strip_thinking_tags};

/// Type alias for common Result type
type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Build model options from config, only setting parameters if specified
fn build_model_options(config: &ModelConfig) -> ModelOptions {
    let mut opts = ModelOptions::default()
        .temperature(config.temperature)
        .repeat_penalty(config.repeat_penalty.unwrap_or(1.1));

    if config.num_ctx > 0 {
        opts = opts.num_ctx(config.num_ctx as u64);
    }

    if let Some(top_k) = config.top_k {
        opts = opts.top_k(top_k);
    }

    if let Some(top_p) = config.top_p {
        opts = opts.top_p(top_p);
    }

    opts
}

/// Run the interactive chat REPL
pub async fn run_chat_repl(settings: &Settings, args: &super::ChatArgs) -> AppResult<()> {
    let use_debug = settings.output.debug_default;

    if use_debug {
        enable_debug();
        log_debug("Debug mode enabled for chat session");
    }

    // Get project identifier
    let project_id = if args.anonymous {
        None
    } else {
        get_project_id()
    };

    if use_debug {
        if let Some(ref pid) = project_id {
            log_debug(&format!("Project ID: {}", pid));
        } else {
            log_debug("Running in anonymous mode (no persistence)");
        }
    }

    // Initialize storage
    let storage = ConversationStorage::new();

    // Load or create session
    let mut session = if let Some(ref session_name) = args.load {
        match ChatSession::load(&storage, &project_id, session_name) {
            Ok(s) => {
                println!(
                    "Loaded session: {} ({} messages)",
                    session_name,
                    s.messages.len()
                );
                s
            }
            Err(e) => {
                eprintln!("Warning: Could not load session '{}': {}", session_name, e);
                println!("Starting new session...");
                ChatSession::new(
                    args.model
                        .clone()
                        .unwrap_or_else(|| settings.model.default.clone()),
                    project_id.clone(),
                    args.anonymous,
                )
            }
        }
    } else {
        let default_id = ConversationStorage::default_session_id();
        if storage.session_exists(&project_id, &default_id) {
            match ChatSession::load(&storage, &project_id, &default_id) {
                Ok(s) => {
                    println!(
                        "Resumed session: {} ({} messages)",
                        default_id,
                        s.messages.len()
                    );
                    s
                }
                Err(e) => {
                    eprintln!("Warning: Could not load default session: {}", e);
                    println!("Starting new session...");
                    ChatSession::new(
                        args.model
                            .clone()
                            .unwrap_or_else(|| settings.model.default.clone()),
                        project_id.clone(),
                        args.anonymous,
                    )
                }
            }
        } else {
            ChatSession::new(
                args.model
                    .clone()
                    .unwrap_or_else(|| settings.model.default.clone()),
                project_id.clone(),
                args.anonymous,
            )
        }
    };

    // Apply CLI flags
    session.think = args.think;
    session.tools = args.tools || settings.get_subcommand_config("query").2;
    session.tool_output_level = args.tools_output;

    // Get initial model configuration - this is the actual state we use
    let mut current_model_name = session.model.clone();
    let mut model_config = match crate::user_models::get_model_config(&current_model_name) {
        Some(cfg) => cfg,
        None => {
            eprintln!(
                "Error: Unknown model '{}'. Use --list to see available models.",
                session.model
            );
            return Err(format!("Unknown model: {}", session.model).into());
        }
    };

    // Initialize Ollama client
    let ollama = settings.ollama_client();

    // Detect model capabilities
    let mut capabilities = match ModelCapabilities::detect(&ollama, &model_config.model_id).await {
        Ok(caps) => caps,
        Err(e) => {
            eprintln!("Warning: Could not detect model capabilities: {}", e);
            eprintln!("Continuing without capability detection...");
            ModelCapabilities {
                tools: false,
                vision: false,
                completion: true,
                thinking: false,
            }
        }
    };

    // Check think mode compatibility
    if session.think && !capabilities.thinking {
        eprintln!(
            "Warning: Model '{}' does not support think mode. Ignoring -t flag.",
            model_config.model_id
        );
        session.think = false;
    }

    // Load AGENTS.md ONCE at startup (not on every message)
    let agents_md = if !args.ignore_agents {
        let md = crate::context::load_agents_md();
        if md.is_some() {
            println!("Loaded AGENTS.md context from current directory.");
        }
        md
    } else {
        None
    };

    // Print session info
    print_welcome(&session, &model_config, &capabilities);

    // Get tools setting
    let mut tools_enabled = session.tools && capabilities.tools;

    // Warn if tools are enabled but model doesn't support them
    if session.tools && !capabilities.tools {
        eprintln!(
            "Warning: Tools are enabled but model '{}' does not support tool calling.",
            model_config.model_id
        );
        eprintln!("         Tools have been disabled for this session. Use /tools to toggle.");
    }

    // Initialize readline with completer
    let config = Config::default();

    // Get model list for completion
    let model_names: Vec<String> = crate::user_models::list_all_model_names();
    let completer = ChatCompleter::new(model_names);

    let mut rl: rustyline::Editor<ChatCompleter, DefaultHistory> =
        rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(completer));
    let _ = rl.load_history(&history_path());

    // Main REPL loop
    loop {
        // Build prompt with mode indicators
        let mut prompt = current_model_name.clone();
        if session.think && capabilities.thinking {
            prompt.push_str("[t]");
        }
        if tools_enabled {
            prompt.push_str("[T]");
        }
        prompt.push_str("> ");

        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line.to_string());

                // Check if it's a command
                if line.starts_with('/') {
                    match parse_command(line) {
                        Some(Ok(cmd)) => {
                            // Handle model switch specially (needs async)
                            if let super::commands::ChatCommand::Model { name } = &cmd {
                                if !crate::user_models::is_model_valid(name) {
                                    eprintln!(
                                        "Unknown model: '{}'. Use --list to see available models.",
                                        name
                                    );
                                    continue;
                                }

                                // Update session
                                session.set_model(name.clone());
                                current_model_name = name.clone();

                                // Load new config
                                let new_config = match crate::user_models::get_model_config(name) {
                                    Some(cfg) => cfg,
                                    None => {
                                        eprintln!("Error loading model config.");
                                        continue;
                                    }
                                };

                                // Detect new capabilities
                                let new_caps =
                                    match ModelCapabilities::detect(&ollama, &new_config.model_id)
                                        .await
                                    {
                                        Ok(c) => c,
                                        Err(_) => {
                                            eprintln!("Warning: Could not detect capabilities.");
                                            capabilities.clone()
                                        }
                                    };

                                // Update state
                                model_config = new_config;
                                capabilities = new_caps;
                                tools_enabled = session.tools && capabilities.tools;

                                // Warn about capabilities
                                if session.think && !capabilities.thinking {
                                    eprintln!("Note: '{}' does not support think mode.", name);
                                }
                                if session.tools && !capabilities.tools {
                                    eprintln!(
                                        "Warning: Tools are enabled but '{}' does not support tool calling.",
                                        name
                                    );
                                    eprintln!(
                                        "         Tools have been disabled. Use /tools to toggle."
                                    );
                                }

                                println!("Model switched to: {} ({})", name, model_config.model_id);

                                if !session.anonymous {
                                    let _ = session.save(&storage);
                                }
                                continue;
                            }

                            match execute_command(cmd, &mut session, &storage) {
                                CommandResult::Continue => continue,
                                CommandResult::Exit => {
                                    let _ = rl.save_history(&history_path());
                                    if !session.anonymous {
                                        let _ = session.save(&storage);
                                    }
                                    return Ok(());
                                }
                                CommandResult::Error(e) => {
                                    eprintln!("Error: {}", e);
                                    continue;
                                }
                                CommandResult::ThinkToggled(new_state) => {
                                    // Check if model supports think mode
                                    if new_state && !capabilities.thinking {
                                        eprintln!(
                                            "Warning: Model '{}' does not support think mode.",
                                            model_config.model_id
                                        );
                                        session.think = false;
                                    } else {
                                        println!(
                                            "Think mode: {}",
                                            if new_state { "enabled" } else { "disabled" }
                                        );
                                        tools_enabled = session.tools && capabilities.tools;
                                    }
                                    continue;
                                }
                                CommandResult::ToolsToggled(new_state) => {
                                    // Check if model supports tools
                                    if new_state && !capabilities.tools {
                                        eprintln!(
                                            "Warning: Model '{}' does not support tools.",
                                            model_config.model_id
                                        );
                                        session.tools = false;
                                        tools_enabled = false;
                                    } else {
                                        println!(
                                            "Tools: {}",
                                            if new_state { "enabled" } else { "disabled" }
                                        );
                                        tools_enabled = new_state && capabilities.tools;
                                    }
                                    continue;
                                }
                                CommandResult::Compact => {
                                    // Compact needs async handling - do it here
                                    if session.messages.is_empty() {
                                        println!("No messages to compact.");
                                        continue;
                                    }

                                    println!("Compacting {} messages...", session.messages.len());

                                    match compact_conversation(
                                        &ollama,
                                        &model_config,
                                        &session,
                                        settings,
                                        agents_md.as_deref(),
                                    )
                                    .await
                                    {
                                        Ok(summary) => {
                                            let compacted_count = session.messages.len();
                                            session.set_compacted_summary(summary.clone());
                                            println!(
                                                "Compacted {} messages into summary.",
                                                compacted_count
                                            );
                                            println!();
                                            println!("\x1B[90m--- Summary ---\x1B[0m");
                                            println!("{}", summary);
                                            println!("\x1B[90m---------------\x1B[0m");

                                            if !session.anonymous {
                                                let _ = session.save(&storage);
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Error compacting conversation: {}", e);
                                        }
                                    }
                                    continue;
                                }
                                CommandResult::ToolOutputChanged(level) => {
                                    println!("Tool output level: {}", level);
                                    continue;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            eprintln!("{}", e);
                            continue;
                        }
                        None => {}
                    }
                }

                // Regular message - send to model
                match send_message(
                    &ollama,
                    &model_config,
                    &session,
                    line,
                    tools_enabled,
                    session.think,
                    settings,
                    agents_md.as_deref(),
                    use_debug,
                )
                .await
                {
                    Ok((response, metrics)) => {
                        session.add_user_message(line.to_string());
                        session.add_assistant_message(response);

                        // Display token metrics
                        if metrics.total_tokens > 0 {
                            eprintln!(
                                "\n\x1B[90m[Tokens: {} prompt + {} response = {} total]\x1B[0m",
                                metrics.prompt_tokens,
                                metrics.response_tokens,
                                metrics.total_tokens
                            );
                        }

                        // Auto-save after each message
                        if !session.anonymous
                            && let Err(e) = session.save(&storage)
                            && use_debug
                        {
                            log_debug(&format!("Warning: Could not save session: {}", e));
                        }
                    }
                    Err(e) => {
                        let error_msg = format_tool_error(&e.to_string());
                        eprintln!("\nError: {}", error_msg);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("^D");
                let _ = rl.save_history(&history_path());
                if !session.anonymous {
                    let _ = session.save(&storage);
                }
                return Ok(());
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                break;
            }
        }
    }

    let _ = rl.save_history(&history_path());
    Ok(())
}

/// Token usage metrics from Ollama response
#[derive(Debug, Clone, Default)]
pub struct TokenMetrics {
    /// Number of tokens in the prompt
    pub prompt_tokens: u64,
    /// Number of tokens in the response
    pub response_tokens: u64,
    /// Total tokens (prompt + response)
    pub total_tokens: u64,
}

/// Send a message to the model
#[allow(clippy::too_many_arguments)]
async fn send_message(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &ChatSession,
    user_input: &str,
    tools_enabled: bool,
    think_enabled: bool,
    settings: &Settings,
    agents_md: Option<&str>,
    use_debug: bool,
) -> AppResult<(String, TokenMetrics)> {
    let model_options = build_model_options(model_config);

    // Get system prompt - AGENTS.md passed from REPL startup
    let prompt_name = if tools_enabled {
        "tool_user"
    } else {
        "default"
    };
    let blacklist_set = settings.blacklist_set();

    // Use session's custom prompt or default
    let system_prompt = if let Some(ref custom_prompt) = session.system_prompt {
        custom_prompt.clone()
    } else {
        get_prompt_with_blacklist(
            prompt_name,
            Some(&model_config.model_id),
            Some(&blacklist_set),
            agents_md,
        )
        .unwrap_or_else(|| "You are a helpful assistant.".to_string())
    };

    // Build coordinator
    let mut coordinator: Coordinator<Vec<ChatMessage>> =
        Coordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
            .options(model_options)
            .think(think_enabled);

    // Add tools if enabled
    if tools_enabled {
        let is_tool_allowed = |name: &str| !settings.is_tool_blacklisted(name);
        let mut tool_count = 0;

        #[cfg(feature = "pokemon-tools")]
        {
            if is_tool_allowed("fetch_pokemon") {
                coordinator = coordinator.add_tool(fetch_pokemon);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_pokemon_basic") {
                coordinator = coordinator.add_tool(fetch_pokemon_basic);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_pokemon_stats") {
                coordinator = coordinator.add_tool(fetch_pokemon_stats);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_pokemon_moves") {
                coordinator = coordinator.add_tool(fetch_pokemon_moves);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_pokemon_evolution") {
                coordinator = coordinator.add_tool(fetch_pokemon_evolution);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_ability_details") {
                coordinator = coordinator.add_tool(fetch_ability_details);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_type_effectiveness") {
                coordinator = coordinator.add_tool(fetch_type_effectiveness);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_pokemon_by_type") {
                coordinator = coordinator.add_tool(fetch_pokemon_by_type);
                tool_count += 1;
            }
            if is_tool_allowed("fetch_move_details") {
                coordinator = coordinator.add_tool(fetch_move_details);
                tool_count += 1;
            }
        }

        #[cfg(feature = "weather-tools")]
        {
            if is_tool_allowed("get_weather") {
                coordinator = coordinator.add_tool(get_weather);
                tool_count += 1;
            }
            if is_tool_allowed("get_current_weather") {
                coordinator = coordinator.add_tool(get_current_weather);
                tool_count += 1;
            }
            if is_tool_allowed("get_weather_forecast") {
                coordinator = coordinator.add_tool(get_weather_forecast);
                tool_count += 1;
            }
        }

        #[cfg(feature = "calc-tools")]
        {
            if is_tool_allowed("calculate") {
                coordinator = coordinator.add_tool(calculate);
                tool_count += 1;
            }
        }

        #[cfg(feature = "serper-tools")]
        {
            if crate::tools::serper::is_serper_available() {
                if is_tool_allowed("web_search") {
                    coordinator = coordinator.add_tool(crate::tools::serper::web_search);
                    tool_count += 1;
                }
                if is_tool_allowed("web_search_news") {
                    coordinator = coordinator.add_tool(crate::tools::serper::web_search_news);
                    tool_count += 1;
                }
            }
        }

        #[cfg(feature = "system-tools")]
        {
            if is_tool_allowed("get_current_datetime") {
                coordinator = coordinator.add_tool(get_current_datetime);
                tool_count += 1;
            }
            if is_tool_allowed("get_project_context") {
                coordinator = coordinator.add_tool(get_project_context);
                tool_count += 1;
            }
        }

        #[cfg(feature = "file-tools")]
        {
            if is_tool_allowed("read_file") {
                coordinator = coordinator.add_tool(read_file);
                tool_count += 1;
            }
            if is_tool_allowed("read_file_segment") {
                coordinator = coordinator.add_tool(read_file_segment);
                tool_count += 1;
            }
            if is_tool_allowed("count_lines") {
                coordinator = coordinator.add_tool(count_lines);
                tool_count += 1;
            }
            if is_tool_allowed("list_directory") {
                coordinator = coordinator.add_tool(list_directory);
                tool_count += 1;
            }
            if is_tool_allowed("search_files") {
                coordinator = coordinator.add_tool(search_files);
                tool_count += 1;
            }
        }

        if use_debug {
            log_debug(&format!("{} tools active", tool_count));
        }
    }

    // Build messages with history (using compacted summary if available)
    let mut messages = session.get_messages_for_llm(&system_prompt);

    // Add current user message
    messages.push(ChatMessage::user(user_input.to_string()));

    if use_debug {
        log_debug(&format!("Sending {} messages to model", messages.len()));
        if session.has_compacted_messages() {
            log_debug(&format!(
                "(includes compacted summary of {} messages)",
                session.compacted_message_count()
            ));
        }
    }

    // Show spinner
    let spinner = create_spinner("Thinking...");

    // Send request
    let result = coordinator.chat(messages.clone()).await;

    finish_spinner(spinner);

    match result {
        Ok(response) => {
            let raw_content = response.message.content.clone();
            let processed = process_thinking(&raw_content);

            // Extract token metrics from final_data
            let metrics = if let Some(ref final_data) = response.final_data {
                TokenMetrics {
                    prompt_tokens: final_data.prompt_eval_count,
                    response_tokens: final_data.eval_count,
                    total_tokens: final_data.prompt_eval_count + final_data.eval_count,
                }
            } else {
                TokenMetrics::default()
            };

            // Show thinking content in gray/dim if present and think mode is enabled
            if think_enabled
                && let Some(ref thinking) = processed.thinking
            {
                eprintln!("\x1B[2m\x1B[90m[Thinking]\x1B[0m");
                for line in thinking.lines() {
                    eprintln!("\x1B[2m\x1B[90m  {}\x1B[0m", line);
                }
                eprintln!();
            }

            print_text(&processed.content);
            Ok((processed.content, metrics))
        }
        Err(e) => {
            let error_str = e.to_string();
            let error_msg = format_tool_error(&error_str);
            eprintln!("\n{}", error_msg);
            Err(error_str.into())
        }
    }
}

/// Compact the conversation by summarizing it
async fn compact_conversation(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &ChatSession,
    _settings: &Settings,
    _agents_md: Option<&str>,
) -> AppResult<String> {
    // Build the compact prompt
    let mut conversation_text = String::new();
    for msg in &session.messages {
        match msg.role {
            super::session::MessageRole::User => {
                conversation_text.push_str(&format!("User: {}\n", msg.content));
            }
            super::session::MessageRole::Assistant => {
                conversation_text.push_str(&format!("Assistant: {}\n", msg.content));
            }
            super::session::MessageRole::System => {}
        }
    }

    let compact_prompt = format!(
        r#"Summarize the following conversation concisely, preserving:
1. Key topics discussed
2. Important decisions or conclusions
3. Any code or technical details mentioned
4. Action items or pending questions

Conversation:
{}

Provide a clear, structured summary that captures the essential context."#,
        conversation_text
    );

    // Build coordinator for compact
    let mut model_cfg = model_config.clone();
    model_cfg.temperature = 0.3;
    model_cfg.top_p = Some(0.9);
    let model_options = build_model_options(&model_cfg);

    let mut coordinator: Coordinator<Vec<ChatMessage>> =
        Coordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
            .options(model_options);

    let messages = vec![
        ChatMessage::system("You are a helpful assistant that summarizes conversations concisely while preserving key information.".to_string()),
        ChatMessage::user(compact_prompt),
    ];

    let spinner = create_spinner("Compacting...");
    let result = coordinator.chat(messages).await;
    finish_spinner(spinner);

    match result {
        Ok(response) => {
            let summary = strip_thinking_tags(&response.message.content);
            Ok(summary)
        }
        Err(e) => Err(format!("Failed to compact: {}", e).into()),
    }
}

/// Print welcome message
fn print_welcome(
    session: &ChatSession,
    model_config: &ModelConfig,
    capabilities: &ModelCapabilities,
) {
    let project = session.project_id.as_deref().unwrap_or("anonymous");
    let session_display = if session.anonymous {
        "anonymous (no persistence)"
    } else {
        session.name.as_deref().unwrap_or(&session.id)
    };

    println!();
    println!("+==============================================================+");
    println!("|  Ask-AI Chat                                                 |");
    println!("+==============================================================+");
    println!("|  Model: {:52} |", model_config.model_id);

    // Only show Tools if model supports it
    if capabilities.tools {
        println!(
            "|  Tools: {:52} |",
            if session.tools { "enabled" } else { "disabled" }
        );
    }

    // Only show Think if model supports it
    if capabilities.thinking {
        println!(
            "|  Think: {:52} |",
            if session.think { "enabled" } else { "disabled" }
        );
    }

    println!("|  Project: {:50} |", truncate_str(project, 50));
    println!("|  Session: {:49} |", truncate_str(session_display, 49));
    println!("+==============================================================+");
    println!("|  Type /help for commands, /quit to exit                      |");
    println!("+==============================================================+");
    println!();
}

/// Truncate a string to fit in a display
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Get the history file path
fn history_path() -> std::path::PathBuf {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        let path = std::path::PathBuf::from(data_home).join("ask-ai");
        let _ = std::fs::create_dir_all(&path);
        path.join("chat_history.txt")
    } else if let Some(home_dir) = dirs::home_dir() {
        let path = home_dir.join(".local").join("share").join("ask-ai");
        let _ = std::fs::create_dir_all(&path);
        path.join("chat_history.txt")
    } else {
        std::path::PathBuf::from(".chat_history.txt")
    }
}
