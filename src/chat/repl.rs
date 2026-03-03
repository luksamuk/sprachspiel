//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use std::sync::Arc;

use ollama_rs::generation::chat::ChatMessage;
use rustyline::Config;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use termimad::print_text;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::context_overflow::{check_context_overflow, DEFAULT_OVERFLOW_THRESHOLD};
use crate::debug_tools::{enable_debug, log_debug};
use crate::prompts::builder::{build_system_prompt, PromptConfig, PromptType};
use crate::query::ChatContext;
use crate::retrieval::{build_context, update_retrieval_time, RetrievalConfig};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::tokens::calculate_context_metrics;
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

    // Get chat-specific configuration for model/thinking/tools defaults
    let (config_model, config_thinking, config_tools) = settings.get_subcommand_config("chat");

    // Resolve model from CLI args or ChatArgs, falling back to chat config
    let model_override = cli_model.or(args.model.as_deref());
    let default_model = if !config_model.is_empty() {
        &config_model
    } else {
        &settings.model.default
    };

    // Load or create session
    let mut session = if args.anonymous {
        // Anonymous mode: never load history, always start fresh
        if use_debug {
            log_debug("Anonymous mode: starting fresh session without history");
        }
        ChatSession::new(
            model_override
                .unwrap_or(default_model)
                .to_string(),
            None, // No project_id for anonymous
            true,  // anonymous = true
        )
    } else if let Some(ref session_name) = args.load {
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
                        .unwrap_or(default_model)
                        .to_string(),
                    project_id.clone(),
                    false,
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
                            .unwrap_or(default_model)
                            .to_string(),
                        project_id.clone(),
                        false,
                    )
                }
            }
        } else {
            ChatSession::new(
                model_override
                    .unwrap_or(default_model)
                    .to_string(),
                project_id.clone(),
                false,
            )
        }
    };

    // Apply CLI flags (CLI takes precedence over args)
    let ignore_agents = cli_ignore_agents || args.ignore_agents;

    // CLI model override takes precedence over saved session model
    // Validate model exists before applying
    if let Some(ref model) = model_override {
        if crate::user_models::is_model_valid(model) {
            session.set_model(model.to_string());
        } else {
            eprintln!(
                "Error: Unknown model '{}'. Use --list to see available models.",
                model
            );
            return Ok(());
        }
    } else {
        // Validate session model exists (may have been deleted)
        if !crate::user_models::is_model_valid(&session.model) {
            eprintln!(
                "Warning: Saved model '{}' no longer exists. Using default '{}'.",
                session.model, default_model
            );
            session.set_model(default_model.to_string());
        }
    }

    let mut current_model_name = session.model.clone();
    let mut model_config = crate::user_models::resolve_model_config(&current_model_name);

    let ollama = settings.ollama_client();
    let mut capabilities = ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;
    
    // Initialize database and embedding client for message persistence
    let db: Option<Arc<crate::db::Database>> = if !session.anonymous {
        match crate::db::Database::new() {
            Ok(database) => {
                if use_debug {
                    log_debug("Database initialized for message persistence");
                }
                Some(Arc::new(database))
            }
            Err(e) => {
                if use_debug {
                    log_debug(&format!("Warning: Could not initialize database: {}", e));
                }
                None
            }
        }
    } else {
        None
    };
    
    // Create embedding client
    let embedding_client: Option<Arc<crate::embeddings::EmbeddingClient>> = 
        if db.is_some() {
            Some(Arc::new(crate::embeddings::EmbeddingClient::new(ollama.clone())))
        } else {
            None
        };
    
    // Attach database to session
    if let (Some(db), Some(client)) = (&db, &embedding_client) {
        session.attach_db(Arc::clone(db), Arc::clone(client));
        
        // Recover any missing embeddings from previous session
        let recovered = crate::embeddings::recover_missing_embeddings(db, client, &session.id).await;
        if recovered > 0 {
            log_debug(&format!("Recovered {} missing embedding(s)", recovered));
        }
    }

    // Thinking mode priority:
    // 1. Model capability check (can't enable if not supported)
    // 2. CLI flags (-t/--think) - user override
    // 3. Chat-specific config (model.chat.thinking)
    // 4. Global config (model.thinking)
    // 5. Model default (from models.toml or built-in config)
    let cli_think_flag = cli_think || args.think;
    let model_default_thinking = model_config.thinking;

    // Determine thinking mode
    let think_enabled = if cli_think_flag {
        // User explicitly requested thinking via CLI
        if !capabilities.thinking {
            eprintln!(
                "Warning: Model '{}' does not support think mode. Ignoring -t/--think flag.",
                model_config.model_id
            );
            false
        } else {
            true
        }
    } else {
        // Use config preference, respecting model capability
        let requested_thinking = config_thinking || model_default_thinking;
        if requested_thinking && !capabilities.thinking {
            // Config says yes, but model says no - warn and respect model
            eprintln!(
                "Warning: Model '{}' does not support think mode. Disabled for this session.",
                model_config.model_id
            );
            false
        } else {
            requested_thinking
        }
    };

    // Tools mode priority: CLI -> config -> default
    let cli_tools_flag = cli_tools || args.tools;
    let tools_enabled = if cli_tools_flag {
        true
    } else {
        config_tools
    };

    session.think = think_enabled;
    session.tools = tools_enabled;
    session.tool_output_level = args.tools_output;

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
                                match super::model_switch::switch_model(
                                    name,
                                    &ollama,
                                    &capabilities,
                                    session.think,
                                    session.tools,
                                ).await {
                                    Ok(result) => {
                                        session.set_model(result.model_name.clone());
                                        session.think = result.think_active;
                                        session.tools = result.tools_active;
                                        
                                        current_model_name = result.model_name.clone();
                                        model_config = result.model_config;
                                        capabilities = result.capabilities;
                                        tools_active = result.tools_active;

                                        for warning in &result.warnings {
                                            eprintln!("{}", warning);
                                        }

                                        println!(
                                            "Model switched to: {} ({})",
                                            result.model_name, model_config.model_id
                                        );

                                        if !session.anonymous {
                                            let _ = session.save(&storage);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("{}", e);
                                    }
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
                                        Ok((summary, range)) => {
                                            let (first_preserved, last_preserved_start) =
                                                range.unwrap_or((0, session.messages.len()));
                                            let compacted_count =
                                                last_preserved_start - first_preserved;

                                            session
                                                .set_compacted_summary_with_range(summary.clone(), range);

                                            if first_preserved > 0
                                                || last_preserved_start < session.messages.len()
                                            {
                                                // Middle compaction
                                                println!(
                                                    "Compacted {} messages (preserved {} first, {} last).",
                                                    compacted_count,
                                                    first_preserved,
                                                    session.messages.len() - last_preserved_start
                                                );
                                            } else {
                                                // Full compaction (backward compatible)
                                                println!(
                                                    "Compacted all {} messages.",
                                                    compacted_count
                                                );
                                            }

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
                                CommandResult::RetrievalToggled(new_state) => {
                                    if new_state {
                                        println!("Semantic retrieval enabled. Messages will be retrieved from history for context.");
                                        if session.messages.len() < 20 {
                                            println!("Note: Retrieval activates after 20 messages (current: {})", session.messages.len());
                                        }
                                    } else {
                                        println!("Semantic retrieval disabled.");
                                    }
                                    continue;
                                }
                                CommandResult::Context => {
                                    print_context_info(
                                        &session,
                                        &model_config,
                                        tools_active,
                                        agents_md.as_deref(),
                                        settings,
                                    );
                                    continue;
                                }
                                CommandResult::Retry => {
                                    // Remove last assistant messages
                                    let removed = session.remove_last_assistant_messages();
                                    if removed > 0 {
                                        println!("Removed {} assistant message(s). Ready to retry.", removed);
                                    } else {
                                        println!("No assistant messages to remove.");
                                    }

                                    // Get the last user message
                                    if let Some(user_msg) = session.get_last_user_message() {
                                        let user_content = user_msg.content.clone();
                                        println!("Retrying: {}", user_content);

                                        // Send the message again
                                        let think_enabled = session.think;
                                        match send_message(
                                            &ollama,
                                            &model_config,
                                            &mut session,
                                            &user_content,
                                            tools_active,
                                            think_enabled,
                                            settings,
                                            agents_md.as_deref(),
                                            use_debug,
                                            db.as_ref(),
                                            embedding_client.as_ref(),
                                        )
                                        .await
                                        {
                                            Ok((response, metrics)) => {
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
                                    } else {
                                        println!("No user message to retry.");
                                    }
                                    continue;
                                }
                                CommandResult::Undo => {
                                    // Remove last assistant messages
                                    let removed = session.remove_last_assistant_messages();
                                    if removed > 0 {
                                        println!("Removed {} assistant message(s).", removed);
                                    } else {
                                        println!("No assistant messages to remove.");
                                    }

                                    // Get and display the last user message
                                    if let Some(user_msg) = session.get_last_user_message() {
                                        println!("Last message: \"{}\"", user_msg.content);
                                        println!("(Press \u{2191} to retrieve and edit, or type a new message)");
                                    } else {
                                        println!("No user message to show.");
                                    }
                                    continue;
                                }
                                CommandResult::Search { query, limit } => {
                                    // Get the database
                                    let db = match crate::db::Database::new() {
                                        Ok(db) => db,
                                        Err(e) => {
                                            eprintln!("Error: Failed to open database: {}", e);
                                            continue;
                                        }
                                    };

                                    // Search in current conversation
                                    let conversation_id = session.id.clone();
                                    
                                    if use_debug {
                                        log_debug(&format!("Searching in conversation: {}", conversation_id));
                                    }

                                    // Run search
                                    crate::retrieval::run_search(
                                        &db,
                                        &ollama,
                                        &query,
                                        Some(&conversation_id),
                                        limit,
                                    ).await;
                                    continue;
                                }
                                CommandResult::Migrate { session_id } => {
                                    // Check if database is available
                                    let db = match &db {
                                        Some(d) => Arc::clone(d),
                                        None => {
                                            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
                                            continue;
                                        }
                                    };
                                    
                                    let embedding_client = crate::embeddings::EmbeddingClient::new(ollama.clone());
                                    let embedding_client = Arc::new(embedding_client);
                                    
                                    if let Some(sid) = session_id {
                                        // Migrate specific session
                                        println!("Migrating session: {}", sid);
                                        match ChatSession::load(&storage, &session.project_id, &sid) {
                                            Ok(sess) => {
                                                match crate::db::migrate_session(&sess, &db, &embedding_client).await {
                                                    Ok(stats) => {
                                                        println!(
                                                            "Migration complete: {} messages, {} embeddings",
                                                            stats.messages_migrated,
                                                            stats.embeddings_generated
                                                        );
                                                        if !stats.errors.is_empty() {
                                                            eprintln!("Errors:");
                                                            for e in stats.errors {
                                                                eprintln!("  - {}", e);
                                                            }
                                                        }
                                                    }
                                                    Err(e) => eprintln!("Error: {}", e),
                                                }
                                            }
                                            Err(e) => eprintln!("Error loading session: {}", e),
                                        }
                                    } else {
                                        // Migrate all sessions for project
                                        match crate::db::migrate_project(&storage, &session.project_id, &db, &embedding_client).await {
                                            Ok(stats) => {
                                                println!(
                                                    "Migration complete: {} sessions, {} messages, {} embeddings",
                                                    stats.sessions_migrated,
                                                    stats.messages_migrated,
                                                    stats.embeddings_generated
                                                );
                                                if !stats.errors.is_empty() {
                                                    eprintln!("Errors:");
                                                    for e in stats.errors {
                                                        eprintln!("  - {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => eprintln!("Error: {}", e),
                                        }
                                    }
                                    continue;
                                }
                                CommandResult::Reindex { conversation_id } => {
                                    // Check if database is available
                                    let db = match &db {
                                        Some(d) => Arc::clone(d),
                                        None => {
                                            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
                                            continue;
                                        }
                                    };
                                    
                                    let embedding_client = crate::embeddings::EmbeddingClient::new(ollama.clone());
                                    let embedding_client = Arc::new(embedding_client);
                                    
                                    let conv_id = conversation_id.unwrap_or_else(|| session.id.clone());
                                    
                                    println!("Reindexing conversation: {}", conv_id);
                                    match crate::db::reindex_conversation(&db, &embedding_client, &conv_id).await {
                                        Ok(stats) => {
                                            println!(
                                                "Reindex complete: {} messages, {} embeddings",
                                                stats.messages_migrated,
                                                stats.embeddings_generated
                                            );
                                            if !stats.errors.is_empty() {
                                                eprintln!("Errors:");
                                                for e in stats.errors {
                                                    eprintln!("  - {}", e);
                                                }
                                            }
                                        }
                                        Err(e) => eprintln!("Error: {}", e),
                                    }
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

                // Save user message immediately before sending
                session.add_user_message(line.to_string());
                if !session.anonymous
                    && let Err(e) = session.save(&storage)
                    && use_debug
                {
                    log_debug(&format!("Warning: Could not save session: {}", e));
                }

                let think_enabled = session.think;
                match send_message(
                    &ollama,
                    &model_config,
                    &mut session,
                    line,
                    tools_active,
                    think_enabled,
                    settings,
                    agents_md.as_deref(),
                    use_debug,
                    db.as_ref(),
                    embedding_client.as_ref(),
                )
                .await
                {
                    Ok((response, metrics)) => {
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
    session: &mut ChatSession,
    user_input: &str,
    tools_enabled: bool,
    think_enabled: bool,
    settings: &Settings,
    agents_md: Option<&str>,
    use_debug: bool,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
) -> AppResult<(String, TokenMetrics)> {
    let model_options = model_config.build_model_options();

    let blacklist_set = settings.blacklist_set();

    let system_prompt = if let Some(ref custom_prompt) = session.system_prompt {
        custom_prompt.clone()
    } else {
        let prompt_type = if tools_enabled {
            PromptType::ToolUser
        } else {
            PromptType::Default
        };
        build_system_prompt(
            PromptConfig::new(prompt_type)
                .with_model_id(Some(&model_config.model_id))
                .with_blacklist(Some(&blacklist_set))
                .with_agents_md(agents_md)
                .with_tools(tools_enabled),
        )
    };

    // Check context overflow
    let context_window = model_config.num_ctx as usize;
    let overflow_status = check_context_overflow(session, &system_prompt, context_window, DEFAULT_OVERFLOW_THRESHOLD);
    
    if overflow_status.needs_compaction() {
        eprintln!(
            "\x1B[33m⚠ Context {}% full. Consider using /compact to summarize old messages.\x1B[0m",
            overflow_status.usage_percent()
        );
    } else if use_debug {
        log_debug(&format!(
            "Context usage: {} / {} tokens ({:.1}%)",
            overflow_status.total_tokens(),
            context_window,
            overflow_status.usage_percent() as f32
        ));
    }

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

    // Build context with retrieval if enabled and available
    let retrieval_config = if session.retrieval_enabled {
        RetrievalConfig::default()
    } else {
        RetrievalConfig {
            enabled: false,
            ..RetrievalConfig::default()
        }
    };

    let context_result = build_context(
        session,
        db,
        embedding_client,
        user_input,
        &system_prompt,
        &retrieval_config,
    ).await;
    
    // Update last_retrieval_time if retrieval was performed
    if context_result.retrieval_performed {
        update_retrieval_time(session);
        if use_debug {
            log_debug(&format!(
                "Retrieved {} relevant messages",
                context_result.retrieved_count
            ));
        }
    }
    
    let mut messages = context_result.messages;
    
    // Add current user query at the end
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
) -> AppResult<(String, Option<(usize, usize)>)> {
    use crate::context_overflow::get_compaction_range_default;

    if session.messages.is_empty() {
        return Err("No messages to compact.".into());
    }

    // Determine which messages to summarize (middle compaction)
    let (messages_to_summarize, range) = match get_compaction_range_default(session) {
        Some(suggestion) => {
            // Middle compaction: preserve first N + last N, summarize middle
            let middle: Vec<_> = session.messages[suggestion.middle_indices.clone()].to_vec();
            let range = Some((
                suggestion.keep_first,
                session.messages.len() - suggestion.keep_last,
            ));
            (middle, range)
        }
        None => {
            // Not enough messages for middle compaction, summarize all
            let all = session.messages.clone();
            let range = Some((0, session.messages.len()));
            (all, range)
        }
    };

    // Build conversation text for summarization
    let mut conversation_text = String::new();
    for msg in &messages_to_summarize {
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
            Ok((summary, range))
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

fn print_context_info(
    session: &ChatSession,
    model_config: &ModelConfig,
    tools_enabled: bool,
    agents_md: Option<&str>,
    settings: &Settings,
) {
    let blacklist_set = settings.blacklist_set();
    
    let prompt_type = if tools_enabled {
        PromptType::ToolUser
    } else {
        PromptType::Default
    };
    
    let system_prompt = build_system_prompt(
        PromptConfig::new(prompt_type)
            .with_model_id(Some(&model_config.model_id))
            .with_blacklist(Some(&blacklist_set))
            .with_agents_md(agents_md)
            .with_tools(tools_enabled),
    );
    
    let history_messages = session.get_messages_for_llm(&system_prompt);
    let context_window = model_config.num_ctx as usize;
    
    let tool_count = if tools_enabled {
        get_available_tool_names(settings).len()
    } else {
        0
    };
    
    let tools_tokens = if tools_enabled && tool_count > 0 {
        tool_count * 20
    } else {
        0
    };
    
    let metrics = calculate_context_metrics(
        &history_messages,
        context_window,
        &system_prompt,
        tools_tokens,
    );
    
    let context_window_k = context_window / 1024;
    
    println!();
    println!("Context Information:");
    println!("  Model:          {} ({}K context)", model_config.model_id, context_window_k);
    println!();
    println!("  Token Breakdown:");
    println!("    System prompt:  ~{} tokens", metrics.system_tokens);
    if tools_enabled && tool_count > 0 {
        println!("    Tool definitions: ~{} tokens ({} tools)", metrics.tools_tokens, tool_count);
    }
    println!("    Conversation:    ~{} tokens ({} messages)", metrics.history_tokens, session.messages.len());
    println!("    {}", "─".repeat(40));
    println!("    Total used:       ~{} tokens", metrics.total_tokens);
    println!("    Available:        ~{} tokens", metrics.available());
    println!("    Utilization:      {:.1}%", metrics.utilization * 100.0);
    println!();
    
    if session.has_compacted_messages() {
        println!("  Session:");
        println!("    Compacted:       {} messages summarized", session.compacted_message_count());
        println!("    Active:          {} messages", session.messages.len() - session.compacted_message_count());
    }
    println!("    Total:           {} messages", session.messages.len());
    println!();
}
