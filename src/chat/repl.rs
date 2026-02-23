//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use ollama_rs::generation::chat::ChatMessage;
use rustyline::Config;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use termimad::print_text;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::debug_tools::{enable_debug, log_debug};
use crate::prompts::get_prompt_with_blacklist;
use crate::query::ChatContext;
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::tool_robustness::format_tool_error;
use crate::tools::{get_available_tool_names, register_tools};

use super::commands::{CommandResult, execute_command, parse_command};
use super::completion::ChatCompleter;
use super::coordinator::{classify_error_str, format_recovery_message, is_error_str_recoverable, MAX_RETRIES};
use super::custom_coordinator::CustomCoordinator;
use super::history::{ConversationStorage, get_project_id};
use super::session::ChatSession;
use super::thinking::{display_thinking, strip_thinking_tags};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Run the interactive chat REPL
#[allow(clippy::too_many_arguments)]
pub async fn run_chat_repl(
    settings: &Settings,
    args: &super::ChatArgs,
    cli_model: Option<&str>,
    cli_think: bool,
    cli_tools: bool,
    cli_ignore_agents: bool,
) -> AppResult<()> {
    let use_debug = settings.output.debug_default;

    if use_debug {
        enable_debug();
        log_debug("Debug mode enabled for chat session");
    }

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

    let storage = ConversationStorage::new();

    // Resolve model from CLI args or ChatArgs
    let model_override = cli_model.or(args.model.as_deref());

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
                    model_override
                        .unwrap_or(&settings.model.default)
                        .to_string(),
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
                        model_override
                            .unwrap_or(&settings.model.default)
                            .to_string(),
                        project_id.clone(),
                        args.anonymous,
                    )
                }
            }
        } else {
            ChatSession::new(
                model_override
                    .unwrap_or(&settings.model.default)
                    .to_string(),
                project_id.clone(),
                args.anonymous,
            )
        }
    };

    // Apply CLI flags (CLI takes precedence over args)
    let think_enabled = cli_think || args.think;
    let tools_enabled = cli_tools || args.tools || settings.get_subcommand_config("query").2;
    let ignore_agents = cli_ignore_agents || args.ignore_agents;

    session.think = think_enabled;
    session.tools = tools_enabled;
    session.tool_output_level = args.tools_output;

    let mut current_model_name = session.model.clone();
    let mut model_config = crate::user_models::resolve_model_config(&current_model_name);

    let ollama = settings.ollama_client();
    let mut capabilities = ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

    if session.think && !capabilities.thinking {
        eprintln!(
            "Warning: Model '{}' does not support think mode. Ignoring -t flag.",
            model_config.model_id
        );
        session.think = false;
    }

    let agents_md = if !ignore_agents {
        let md = crate::context::load_agents_md();
        if md.is_some() {
            println!("Loaded AGENTS.md context from current directory.");
        }
        md
    } else {
        None
    };

    print_welcome(&session, &model_config, &capabilities);

    let mut tools_active = session.tools && capabilities.tools;

    if session.tools && !capabilities.tools {
        eprintln!(
            "Warning: Tools are enabled but model '{}' does not support tool calling.",
            model_config.model_id
        );
        eprintln!("         Tools have been disabled for this session. Use /tools to toggle.");
    }

    let config = Config::default();
    let model_names: Vec<String> = crate::user_models::list_all_model_names();
    let completer = ChatCompleter::new(model_names);

    let mut rl: rustyline::Editor<ChatCompleter, DefaultHistory> =
        rustyline::Editor::with_config(config)?;
    rl.set_helper(Some(completer));
    let _ = rl.load_history(&history_path());

    loop {
        let mut prompt = current_model_name.clone();
        if session.think && capabilities.thinking {
            prompt.push_str("[t]");
        }
        if tools_active {
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

                if line.starts_with('/') {
                    match parse_command(line) {
                        Some(Ok(cmd)) => {
                            if let super::commands::ChatCommand::Model { name } = &cmd {
                                if !crate::user_models::is_model_valid(name) {
                                    eprintln!(
                                        "Unknown model: '{}'. Use --list to see available models.",
                                        name
                                    );
                                    continue;
                                }

                                session.set_model(name.clone());
                                current_model_name = name.clone();

                                let new_config = crate::user_models::resolve_model_config(name);

                                let new_caps =
                                    match ModelCapabilities::detect(&ollama, &new_config.model_id)
                                        .await
                                    {
                                        Ok(c) => c,
                                        Err(_) => {
                                            eprintln!("Warning: Could not detect capabilities, keeping previous.");
                                            capabilities.clone()
                                        }
                                    };

                                model_config = new_config;
                                capabilities = new_caps;
                                tools_active = session.tools && capabilities.tools;

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
                                        tools_active = session.tools && capabilities.tools;
                                    }
                                    continue;
                                }
                                CommandResult::ToolsToggled(new_state) => {
                                    if new_state && !capabilities.tools {
                                        eprintln!(
                                            "Warning: Model '{}' does not support tools.",
                                            model_config.model_id
                                        );
                                        session.tools = false;
                                        tools_active = false;
                                    } else {
                                        println!(
                                            "Tools: {}",
                                            if new_state { "enabled" } else { "disabled" }
                                        );
                                        tools_active = new_state && capabilities.tools;
                                    }
                                    continue;
                                }
                                CommandResult::Compact => {
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
                                CommandResult::DebugToggled(new_state) => {
                                    println!("Debug mode: {}", new_state);
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

                match send_message(
                    &ollama,
                    &model_config,
                    &session,
                    line,
                    tools_active,
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

                        if metrics.total_tokens > 0 {
                            eprintln!(
                                "\n\x1B[90m[Tokens: {} prompt + {} response = {} total]\x1B[0m",
                                metrics.prompt_tokens,
                                metrics.response_tokens,
                                metrics.total_tokens
                            );
                        }

                        if !session.anonymous
                            && let Err(e) = session.save(&storage)
                            && use_debug
                        {
                            log_debug(&format!("Warning: Could not save session: {}", e));
                        }
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        eprintln!("\x1B[31m{}\x1B[0m", format_tool_error(&error_str));
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

#[derive(Debug, Clone, Default)]
pub struct TokenMetrics {
    pub prompt_tokens: u64,
    pub response_tokens: u64,
    pub total_tokens: u64,
}

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
    let model_options = model_config.build_model_options();

    let prompt_name = if tools_enabled { "tool_user" } else { "default" };
    let blacklist_set = settings.blacklist_set();

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

    let coordinator = ChatContext {
        ollama: ollama.clone(),
        model_id: model_config.model_id.clone(),
        model_options,
        use_think: think_enabled,
        use_debug,
        use_plain: false,
    }
    .build_coordinator();

    let mut coordinator = coordinator;
    if tools_enabled {
        let (coord_new, tool_count) = register_tools(coordinator, settings, use_debug);
        coordinator = coord_new;
        if use_debug {
            log_debug(&format!("{} tools active", tool_count));
        }
    }

    let mut messages = session.get_messages_for_llm(&system_prompt);
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

    let spinner = create_spinner("Thinking...");

    let tool_names: Vec<String> = if tools_enabled {
        get_available_tool_names(settings)
    } else {
        vec![]
    };

    let mut attempts = 0;
    let mut messages = messages;
    let result = loop {
        let current_result = coordinator.chat(messages.clone()).await;

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                let error_str = e.to_string();

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

                    messages.push(ChatMessage::tool(error_msg));

                    if attempts == 1 {
                        finish_spinner(spinner.clone());
                        eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                    }

                    continue;
                } else {
                    break Err(error_str);
                }
            }
        }
    };

    finish_spinner(spinner);

    match result {
        Ok(response) => {
            let content = response.message.content.clone();

            let metrics = if let Some(ref final_data) = response.final_data {
                TokenMetrics {
                    prompt_tokens: final_data.prompt_eval_count,
                    response_tokens: final_data.eval_count,
                    total_tokens: final_data.prompt_eval_count + final_data.eval_count,
                }
            } else {
                TokenMetrics::default()
            };

            if think_enabled {
                display_thinking(&content, response.message.thinking.as_ref(), true);
            }

            let display_content = strip_thinking_tags(&content);
            print_text(&display_content);
            Ok((display_content, metrics))
        }
        Err(e) => {
            let error_msg = format_tool_error(&e);
            eprintln!("\n{}", error_msg);
            Err(e.into())
        }
    }
}

async fn compact_conversation(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &ChatSession,
    _settings: &Settings,
    _agents_md: Option<&str>,
) -> AppResult<String> {
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
            super::session::MessageRole::Tool => {
                conversation_text.push_str(&format!("Tool call: {}\n", msg.content));
            }
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

    let mut model_cfg = model_config.clone();
    model_cfg.temperature = 0.3;
    model_cfg.top_p = Some(0.9);
    let model_options = model_cfg.build_model_options();

    let mut coordinator = CustomCoordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
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

    if capabilities.tools {
        println!(
            "|  Tools: {:52} |",
            if session.tools { "enabled" } else { "disabled" }
        );
    }

    if capabilities.thinking {
        println!(
            "|  Think: {:52} |",
            if session.think { "enabled" } else { "disabled" }
        );
    }

    println!("|  Project: {:50} |", truncate_str(project, 50));
    println!("|  Session: {:50} |", truncate_str(session_display, 49));
    println!("+==============================================================+");
    println!("|  Type /help for commands, /quit to exit                      |");
    println!("+==============================================================+");
    println!();
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

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
