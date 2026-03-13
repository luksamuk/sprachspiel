//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use std::sync::Arc;

use ollama_rs::generation::chat::ChatMessage;
use rustyline::Config;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::context_overflow::{
    DEFAULT_OVERFLOW_THRESHOLD, PRE_TOOL_THRESHOLD, check_context_overflow,
    needs_pre_tool_compaction,
};
use crate::debug_tools::{enable_debug, log_debug};
use crate::markdown;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::query::ChatContext;
use crate::retrieval::{RetrievalConfig, build_context, update_retrieval_time};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::tokens::{calculate_context_metrics, estimate_tokens};
use crate::tool_robustness::format_tool_error;
use crate::tools::{get_available_tool_names, register_tools};

use super::commands::{CommandResult, execute_command, parse_command};
use super::completion::ChatCompleter;
use super::coordinator::{
    MAX_RETRIES, classify_error_str, format_recovery_message, is_error_str_recoverable,
};
use super::custom_coordinator::CustomCoordinator;
use super::session::ChatSession;
use super::thinking::{display_thinking, strip_thinking_tags};
use crate::project::get_project_id;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Run the interactive chat REPL
#[allow(clippy::too_many_arguments)]
pub async fn run_chat_repl(
    settings: &Settings,
    args: &super::ChatArgs,
    cli_model: Option<&str>,
    cli_think: bool,
    cli_tools: bool,
    cli_code: bool,
    cli_ignore_agents: bool,
    cli_soulless: bool,
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

    // Get chat-specific configuration for model/thinking/tools defaults
    let (config_model, config_thinking, config_tools) = settings.get_subcommand_config("chat");

    // Resolve model from CLI args or ChatArgs, falling back to chat config
    let model_override = cli_model.or(args.model.as_deref());
    let default_model = if !config_model.is_empty() {
        &config_model
    } else {
        &settings.model.default
    };

    // Initialize database first (needed for session loading and migration)
    let db: Option<Arc<crate::db::Database>> = if !args.anonymous {
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

    // Create ollama client early for embedding client (needed for migration)
    let ollama = settings.ollama_client();
    let embedding_client: Option<Arc<crate::embeddings::EmbeddingClient>> = if db.is_some() {
        Some(Arc::new(crate::embeddings::EmbeddingClient::new(
            ollama.clone(),
        )))
    } else {
        None
    };

    // Run ONE-TIME automatic migration from JSON to SQLite
    if let (Some(db_ref), Some(client)) = (&db, &embedding_client)
        && !args.anonymous
    {
        let migration_stats = crate::db::migrate_all_legacy_sessions(db_ref, client).await;
        if migration_stats.sessions_migrated > 0 {
            // Sessions were migrated, save the count for later display
            log_debug(&format!(
                "Migrated {} session(s) from JSON to SQLite",
                migration_stats.sessions_migrated
            ));
        }
    }

    // Load or create session
    let mut session = if args.anonymous {
        // Anonymous mode: never load history, always start fresh
        if use_debug {
            log_debug("Anonymous mode: starting fresh session without history");
        }
        ChatSession::new(
            model_override.unwrap_or(default_model).to_string(),
            None, // No project_id for anonymous
            true, // anonymous = true
        )
    } else if let Some(session_name) = &args.load {
        // Try loading from SQLite
        if let Some(db_ref) = &db {
            match ChatSession::load_sqlite(db_ref, session_name) {
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
                    let mut new_session = ChatSession::new(
                        model_override.unwrap_or(default_model).to_string(),
                        project_id.clone(),
                        false,
                    );
                    new_session.id = session_name.clone();
                    new_session
                }
            }
        } else {
            // No database, fallback
            ChatSession::new(
                model_override.unwrap_or(default_model).to_string(),
                project_id.clone(),
                false,
            )
        }
    } else if let Some(db_ref) = &db {
        // Try loading default session from SQLite
        let default_id = "default";
        match db_ref.conversation_exists(default_id) {
            Ok(true) => match ChatSession::load_sqlite(db_ref, default_id) {
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
                        model_override.unwrap_or(default_model).to_string(),
                        project_id.clone(),
                        false,
                    )
                }
            },
            _ => {
                // No default session in DB, create new
                ChatSession::new(
                    model_override.unwrap_or(default_model).to_string(),
                    project_id.clone(),
                    false,
                )
            }
        }
    } else {
        // No database available, create new session
        ChatSession::new(
            model_override.unwrap_or(default_model).to_string(),
            project_id.clone(),
            false,
        )
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

    let mut capabilities =
        ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

    // Attach database to session
    if let (Some(db_ref), Some(client)) = (&db, &embedding_client) {
        session.attach_db(Arc::clone(db_ref), Arc::clone(client));

        // Recover any missing embeddings from previous session
        let recovered =
            crate::embeddings::recover_missing_embeddings(db_ref, client, &session.id).await;
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
    let tools_enabled = if cli_tools_flag { true } else { config_tools };

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
                                )
                                .await
                                {
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
                                            let _ = session.save_sqlite();
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("{}", e);
                                    }
                                }
                                continue;
                            }

                            match execute_command(cmd, &mut session) {
                                CommandResult::Continue => continue,
                                CommandResult::Exit => {
                                    let _ = rl.save_history(&history_path());
                                    if !session.anonymous {
                                        let _ = session.save_sqlite();
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

                                    let msg_count = session.messages.len();
                                    println!(
                                        "\x1B[33m⏳ Compacting {} messages...\x1B[0m",
                                        msg_count
                                    );

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

                                            session.set_compacted_summary_with_range(
                                                summary.clone(),
                                                range,
                                            );

                                            if first_preserved > 0
                                                || last_preserved_start < session.messages.len()
                                            {
                                                // Middle compaction
                                                println!(
                                                    "\x1B[32m✓ Compacted {} messages\x1B[0m (preserved {} first, {} last).",
                                                    compacted_count,
                                                    first_preserved,
                                                    session.messages.len() - last_preserved_start
                                                );
                                            } else {
                                                // Full compaction (backward compatible)
                                                println!(
                                                    "\x1B[32m✓ Compacted all {} messages.\x1B[0m",
                                                    compacted_count
                                                );
                                            }

                                            println!();
                                            println!("\x1B[90m--- Summary ---\x1B[0m");
                                            markdown::print_markdown(&summary);
                                            println!("\x1B[90m---------------\x1B[0m");

                                            if !session.anonymous {
                                                let _ = session.save_sqlite();
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("\x1B[31m✗ Compaction failed: {}\x1B[0m", e);
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
                                        println!(
                                            "Semantic retrieval enabled. Messages will be retrieved from history for context."
                                        );
                                        if session.messages.len() < 20 {
                                            println!(
                                                "Note: Retrieval activates after 20 messages (current: {})",
                                                session.messages.len()
                                            );
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
                                        cli_soulless,
                                    );
                                    continue;
                                }
                                CommandResult::Retry => {
                                    // Remove last assistant messages
                                    let removed = session.remove_last_assistant_messages();
                                    if removed > 0 {
                                        println!(
                                            "Removed {} assistant message(s). Ready to retry.",
                                            removed
                                        );
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
                                            false, // cli_code: false for retry (use existing config)
                                            settings,
                                            agents_md.as_deref(),
                                            use_debug,
                                            db.as_ref(),
                                            embedding_client.as_ref(),
                                            cli_soulless,
                                            None,
                                        )
                                        .await
                                        {
                                            Ok(result) => {
                                                session.add_assistant_message(
                                                    result.response,
                                                    Some(result.metrics.prompt_tokens),
                                                );

                                                if result.metrics.total_tokens > 0 {
                                                    eprintln!(
                                                        "\n\x1B[90m[Tokens: {} prompt + {} response = {} total]\x1B[0m",
                                                        result.metrics.prompt_tokens,
                                                        result.metrics.response_tokens,
                                                        result.metrics.total_tokens
                                                    );
                                                }

                                                // Auto-compact if needed (after response, before next input)
                                                auto_compact_if_needed(
                                                    &ollama,
                                                    &model_config,
                                                    &mut session,
                                                    settings,
                                                    agents_md.as_deref(),
                                                    &result.system_prompt,
                                                    result.context_window,
                                                    use_debug,
                                                )
                                                .await;

                                                if !session.anonymous
                                                    && let Err(e) = session.save_sqlite()
                                                    && use_debug
                                                {
                                                    log_debug(&format!(
                                                        "Warning: Could not save session: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                            Err(e) => {
                                                let error_str = e.to_string();
                                                eprintln!(
                                                    "\x1B[31m{}\x1B[0m",
                                                    format_tool_error(&error_str)
                                                );
                                            }
                                        }
                                    } else {
                                        println!("No user message to retry.");
                                    }
                                    continue;
                                }
                                CommandResult::Undo => {
                                    // Remove last assistant messages (includes preceding user message)
                                    let (removed, _) =
                                        session.remove_last_assistant_messages_with_content();
                                    if removed > 0 {
                                        // Also delete from database if not anonymous
                                        if !session.anonymous
                                            && !session.id.is_empty()
                                            && let Ok(db) = crate::db::Database::new()
                                            && let Err(e) =
                                                db.delete_last_messages(&session.id, removed)
                                        {
                                            eprintln!(
                                                "Warning: Failed to delete from database: {}",
                                                e
                                            );
                                        }
                                        println!("Removed {} message(s) from session.", removed);
                                    } else {
                                        println!("No messages to remove.");
                                    }

                                    // Get and display the last user message
                                    if let Some(user_msg) = session.get_last_user_message() {
                                        println!("Last message: \"{}\"", user_msg.content);
                                        println!(
                                            "(Press \u{2191} to retrieve and edit, or type a new message)"
                                        );
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
                                        log_debug(&format!(
                                            "Searching in conversation: {}",
                                            conversation_id
                                        ));
                                    }

                                    // Run search
                                    crate::retrieval::run_search(
                                        &db,
                                        &ollama,
                                        &query,
                                        Some(&conversation_id),
                                        limit,
                                    )
                                    .await;
                                    continue;
                                }
                                CommandResult::Restore { session_id } => {
                                    // Check if database is available
                                    let db = match &db {
                                        Some(d) => Arc::clone(d),
                                        None => {
                                            eprintln!(
                                                "Error: Database not initialized. Run chat without --anonymous."
                                            );
                                            continue;
                                        }
                                    };

                                    println!("Restoring session: {}", session_id);
                                    match crate::db::restore_session(
                                        &db,
                                        &session.project_id,
                                        &session_id,
                                    ) {
                                        Ok(restored) => {
                                            println!(
                                                "Session restored: {} ({} messages)",
                                                session_id,
                                                restored.messages.len()
                                            );
                                            // Switch to the restored session
                                            session = restored;
                                        }
                                        Err(e) => eprintln!("Error: {}", e),
                                    }
                                    continue;
                                }
                                CommandResult::Reindex { conversation_id } => {
                                    // Check if database is available
                                    let db = match &db {
                                        Some(d) => Arc::clone(d),
                                        None => {
                                            eprintln!(
                                                "Error: Database not initialized. Run chat without --anonymous."
                                            );
                                            continue;
                                        }
                                    };

                                    let embedding_client =
                                        crate::embeddings::EmbeddingClient::new(ollama.clone());
                                    let embedding_client = Arc::new(embedding_client);

                                    let conv_id =
                                        conversation_id.unwrap_or_else(|| session.id.clone());

                                    println!("Reindexing conversation: {}", conv_id);
                                    match crate::db::reindex_conversation(
                                        &db,
                                        &embedding_client,
                                        &conv_id,
                                    )
                                    .await
                                    {
                                        Ok(stats) => {
                                            println!(
                                                "Reindex complete: {} messages, {} embeddings",
                                                stats.messages_migrated, stats.embeddings_generated
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
                // Capture message ID for linking pre-tool content
                let user_message_id = session.add_user_message(line.to_string());
                if !session.anonymous
                    && let Err(e) = session.save_sqlite()
                    && use_debug
                {
                    log_debug(&format!("Warning: Could not save session: {}", e));
                }

                // Pre-tool context check: Auto-compact BEFORE tool execution if context is high
                // This prevents context exhaustion during multi-tool turns
                let context_window = model_config.num_ctx as usize;
                let system_prompt_for_check = build_system_prompt(
                    PromptConfig::new(PromptType::ToolUser)
                        .with_model_id(Some(&model_config.model_id))
                        .with_blacklist(Some(&settings.blacklist_set()))
                        .with_agents_md(agents_md.as_deref())
                        .with_tools(tools_active)
                        .with_retrieval(session.retrieval_enabled && !cli_code)
                        .with_soulless(cli_soulless),
                );

                if needs_pre_tool_compaction(&session, &system_prompt_for_check, context_window) {
                    let usage_pct = check_context_overflow(
                        &session,
                        &system_prompt_for_check,
                        context_window,
                        PRE_TOOL_THRESHOLD,
                    )
                    .usage_percent();
                    eprintln!(
                        "\x1B[33m⏳ Context {}% full. Auto-compacting before tool execution...\x1B[0m",
                        usage_pct
                    );

                    auto_compact_if_needed(
                        &ollama,
                        &model_config,
                        &mut session,
                        settings,
                        agents_md.as_deref(),
                        &system_prompt_for_check,
                        context_window,
                        use_debug,
                    )
                    .await;
                }

                let think_enabled = session.think;
                match send_message(
                    &ollama,
                    &model_config,
                    &mut session,
                    line,
                    tools_active,
                    think_enabled,
                    cli_code, // from function parameter
                    settings,
                    agents_md.as_deref(),
                    use_debug,
                    db.as_ref(),
                    embedding_client.as_ref(),
                    cli_soulless,
                    None,
                )
                .await
                {
                    Ok(result) => {
                        // Save pre-tool content before final response
                        if let Some(pre_content) = &result.pre_tool_content {
                            session.add_pre_tool_message(
                                pre_content.clone(),
                                result.pre_tool_thinking.clone(),
                                user_message_id,
                            );
                            if use_debug {
                                log_debug(&format!(
                                    "Saved pre-tool content ({} chars)",
                                    pre_content.len()
                                ));
                            }
                        }

                        // Handle continuation if LLM paused for compaction
                        let mut final_response = result.response.clone();
                        let mut final_metrics = result.metrics.clone();
                        let mut continuation_count = 0;

                        if let Some(ref continuation_tag) = result.continuation_needed {
                            continuation_count += 1;
                            if use_debug {
                                log_debug(&format!(
                                    "Continuation requested: paused_at='{}', next_step='{}'",
                                    continuation_tag.paused_at, continuation_tag.next_step
                                ));
                            }
                            eprintln!(
                                "\n\x1B[33m⏳ Paused for context compaction, continuing...\x1B[0m"
                            );

                            // Compact the context now
                            let continuation_context_window = result.context_window;
                            let continuation_system_prompt = result.system_prompt.clone();
                            auto_compact_if_needed(
                                &ollama,
                                &model_config,
                                &mut session,
                                settings,
                                agents_md.as_deref(),
                                &continuation_system_prompt,
                                continuation_context_window,
                                use_debug,
                            )
                            .await;

                            // Continue with continuation prompt

                            // Send continuation request (empty user_input, continuation via ephemeral)
                            let continuation_result = send_message(
                                &ollama,
                                &model_config,
                                &mut session,
                                "", // empty user_input - continuation via ephemeral message
                                tools_active,
                                think_enabled,
                                cli_code,
                                settings,
                                agents_md.as_deref(),
                                use_debug,
                                db.as_ref(),
                                embedding_client.as_ref(),
                                cli_soulless,
                                Some(continuation_tag),
                            )
                            .await;

                            match continuation_result {
                                Ok(mut cont_result) => {
                                    // Append continuation response
                                    final_response.push_str("\n\n");
                                    final_response.push_str(&cont_result.response);

                                    // Update metrics
                                    final_metrics.response_tokens +=
                                        cont_result.metrics.response_tokens;
                                    final_metrics.total_tokens += cont_result.metrics.total_tokens;

                                    eprintln!("\n\x1B[90m[Continuation complete]\x1B[0m");

                                    // Handle nested continuations (limit to 3)
                                    while let Some(ref next_tag) = cont_result.continuation_needed {
                                        if continuation_count >= 3 {
                                            eprintln!(
                                                "\x1B[33mWarning: Maximum continuations reached. Please continue manually.\x1B[0m"
                                            );
                                            break;
                                        }

                                        continuation_count += 1;
                                        eprintln!(
                                            "\n\x1B[33m⏳ Paused again, continuing ({})...\x1B[0m",
                                            continuation_count
                                        );

                                        // Compact again
                                        auto_compact_if_needed(
                                            &ollama,
                                            &model_config,
                                            &mut session,
                                            settings,
                                            agents_md.as_deref(),
                                            &cont_result.system_prompt,
                                            cont_result.context_window,
                                            use_debug,
                                        )
                                        .await;

                                        let next_result = send_message(
                                            &ollama,
                                            &model_config,
                                            &mut session,
                                            "", // empty user_input - continuation via ephemeral
                                            tools_active,
                                            think_enabled,
                                            cli_code,
                                            settings,
                                            agents_md.as_deref(),
                                            use_debug,
                                            db.as_ref(),
                                            embedding_client.as_ref(),
                                            cli_soulless,
                                            Some(next_tag),
                                        )
                                        .await;

                                        match next_result {
                                            Ok(n_result) => {
                                                final_response.push_str("\n\n");
                                                final_response.push_str(&n_result.response);
                                                final_metrics.response_tokens +=
                                                    n_result.metrics.response_tokens;
                                                final_metrics.total_tokens +=
                                                    n_result.metrics.total_tokens;

                                                eprintln!(
                                                    "\n\x1B[90m[Continuation complete]\x1B[0m"
                                                );

                                                // Update cont_result for the while loop
                                                cont_result = n_result;
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "\x1B[31mContinuation failed: {}\x1B[0m",
                                                    e
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("\x1B[31mContinuation failed: {}\x1B[0m", e);
                                }
                            }
                        }

                        // Save the final response (merged with continuations if any)
                        session.add_assistant_message(
                            final_response,
                            Some(final_metrics.prompt_tokens),
                        );

                        if final_metrics.total_tokens > 0 {
                            eprintln!(
                                "\n\x1B[90m[Tokens: {} prompt + {} response = {} total]\x1B[0m",
                                final_metrics.prompt_tokens,
                                final_metrics.response_tokens,
                                final_metrics.total_tokens
                            );
                        }

                        // Auto-compact if needed (after response, before next input)
                        let context_window = result.context_window;
                        let system_prompt = result.system_prompt.clone();
                        auto_compact_if_needed(
                            &ollama,
                            &model_config,
                            &mut session,
                            settings,
                            agents_md.as_deref(),
                            &system_prompt,
                            context_window,
                            use_debug,
                        )
                        .await;

                        if !session.anonymous
                            && let Err(e) = session.save_sqlite()
                            && use_debug
                        {
                            log_debug(&format!("Warning: Could not save session: {}", e));
                        }
                    }
                    Err(e) => {
                        let error_str = e.to_string();

                        // Check if this is a context overflow error during tool execution
                        if error_str.contains("Context overflow during tool execution") {
                            eprintln!(
                                "\x1B[31mContext overflow during tool execution. Attempting recovery...\x1B[0m"
                            );

                            // Remove the failed message
                            let (removed, _) =
                                session.remove_last_assistant_messages_with_content();
                            if use_debug {
                                log_debug(&format!(
                                    "Removed {} messages after overflow error",
                                    removed
                                ));
                            }

                            // Auto-compact to free space
                            let overflow_context_window = model_config.num_ctx as usize;
                            let overflow_system_prompt = build_system_prompt(
                                PromptConfig::new(PromptType::ToolUser)
                                    .with_model_id(Some(&model_config.model_id))
                                    .with_blacklist(Some(&settings.blacklist_set()))
                                    .with_agents_md(agents_md.as_deref())
                                    .with_tools(tools_enabled)
                                    .with_retrieval(session.retrieval_enabled && !cli_code)
                                    .with_soulless(cli_soulless),
                            );

                            eprintln!("\x1B[33m⏳ Auto-compacting after overflow error...\x1B[0m");
                            auto_compact_if_needed(
                                &ollama,
                                &model_config,
                                &mut session,
                                settings,
                                agents_md.as_deref(),
                                &overflow_system_prompt,
                                overflow_context_window,
                                use_debug,
                            )
                            .await;

                            // Save session after compaction
                            if !session.anonymous
                                && let Err(save_err) = session.save_sqlite()
                                && use_debug
                            {
                                log_debug(&format!(
                                    "Warning: Could not save session after recovery: {}",
                                    save_err
                                ));
                            }

                            eprintln!(
                                "\x1B[33mPlease retry your message. Context has been compacted.\x1B[0m"
                            );
                            continue;
                        }

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
                    let _ = session.save_sqlite();
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

pub struct SendMessageResult {
    pub response: String,
    pub pre_tool_content: Option<String>,
    pub pre_tool_thinking: Option<String>,
    pub metrics: TokenMetrics,
    pub context_window: usize,
    pub system_prompt: String,
    /// Parsed continuation tag if LLM requested to continue after compaction
    pub continuation_needed: Option<crate::chat::ContinuationTag>,
}

/// Build a continuation prompt from a continuation tag
///
/// Creates a system message that tells the LLM to resume from where it paused
/// after context compaction.
fn build_continuation_prompt(tag: &crate::chat::ContinuationTag) -> String {
    format!(
        "<continuation_prompt>\n\
        Context has been compacted. Resume from the checkpoint.\n\
        \n\
        Reasoning paused at: {}\n\
        Next step: {}\n\
        \n\
        Continue naturally from where you left off. Do not repeat completed work.\n\
        </continuation_prompt>",
        tag.paused_at, tag.next_step
    )
}

#[allow(clippy::too_many_arguments)]
async fn send_message(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    user_input: &str,
    tools_enabled: bool,
    think_enabled: bool,
    cli_code: bool,
    settings: &Settings,
    agents_md: Option<&str>,
    use_debug: bool,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
    cli_soulless: bool,
    continuation_tag: Option<&crate::chat::ContinuationTag>,
) -> AppResult<SendMessageResult> {
    let model_options = model_config.build_model_options();

    let blacklist_set = settings.blacklist_set();

    let system_prompt = if let Some(ref custom_prompt) = session.system_prompt {
        custom_prompt.clone()
    } else {
        // Determine prompt type based on code mode and tools
        let prompt_type = if cli_code && tools_enabled {
            PromptType::CodeWithTools
        } else if cli_code {
            PromptType::Code
        } else if tools_enabled {
            PromptType::ToolUser
        } else {
            PromptType::Default
        };

        // Check context overflow for status injection
        let ctx_window = model_config.num_ctx as usize;
        let ctx_status = check_context_overflow(
            session,
            "", // system_prompt computed below, we just need status
            ctx_window,
            DEFAULT_OVERFLOW_THRESHOLD,
        );

        build_system_prompt(
            PromptConfig::new(prompt_type)
                .with_model_id(Some(&model_config.model_id))
                .with_blacklist(Some(&blacklist_set))
                .with_agents_md(agents_md)
                .with_tools(tools_enabled)
                .with_retrieval(session.retrieval_enabled && !cli_code) // Disable retrieval for code mode
                .with_soulless(cli_soulless)
                .with_context_status(if ctx_status.needs_compaction() {
                    Some(ctx_status.clone())
                } else {
                    None
                }),
        )
    };

    // Check context overflow
    let context_window = model_config.num_ctx as usize;
    let overflow_status = check_context_overflow(
        session,
        &system_prompt,
        context_window,
        DEFAULT_OVERFLOW_THRESHOLD,
    );

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
        context_window: Some(model_config.num_ctx as usize),
        system_prompt: Some(system_prompt.clone()),
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
        use_debug,
    )
    .await;

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

    // If this is a continuation, add ephemeral message to coordinator
    if let Some(tag) = continuation_tag {
        let continuation_prompt = build_continuation_prompt(tag);
        coordinator.push_ephemeral(ChatMessage::user(continuation_prompt));
        if use_debug {
            log_debug("Injected continuation prompt as ephemeral message");
        }
    }

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
        // Run chat with context if DB and embedding client are available
        let current_result = if let (Some(db), Some(embedding)) = (db, embedding_client) {
            crate::tools::context::with_context(
                db.clone(),
                embedding.clone(),
                coordinator.chat(messages.clone()),
            )
            .await
        } else {
            coordinator.chat(messages.clone()).await
        };

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
            markdown::print_markdown(&display_content);

            // Extract pre-tool content from coordinator
            let pre_tool = coordinator.take_pre_tool_content();
            let (pre_tool_content, pre_tool_thinking) = match pre_tool {
                Some(ptc) => (Some(ptc.content), ptc.thinking),
                None => (None, None),
            };

            // Parse continuation tag from response
            let (cleaned_response, continuation_needed) =
                crate::chat::parse_continuation_tag(&display_content);

            // If there was a continuation tag, re-print the cleaned content
            if continuation_needed.is_some() {
                // Clear previous output and reprint without the tag
                eprint!("\x1B[2K\r"); // Clear current line
                markdown::print_markdown(&cleaned_response);
            }

            Ok(SendMessageResult {
                response: cleaned_response,
                pre_tool_content,
                pre_tool_thinking,
                metrics,
                context_window,
                system_prompt,
                continuation_needed,
            })
        }
        Err(e) => {
            // Error will be formatted and printed by the caller (REPL loop)
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
        r#"Summarize the following conversation concisely in MARKDOWN format.

Use this structure:
**Key Topics:**
- Topic 1
- Topic 2

**Decisions Made:**
- Decision 1
- Decision 2

**Technical Details:**
- Important code/technical info

**Action Items:**
- [ ] Pending task 1
- [ ] Pending task 2

Conversation:
{}

Provide a structured markdown summary that captures the essential context."#,
        conversation_text
    );

    let mut model_cfg = model_config.clone();
    model_cfg.temperature = 0.3;
    model_cfg.top_p = Some(0.9);
    let model_options = model_cfg.build_model_options();

    let mut coordinator =
        CustomCoordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
            .options(model_options);

    let messages = vec![
        ChatMessage::system("You are a helpful assistant that summarizes conversations in clean Markdown format. Always use headers, bullets, and formatting to make the summary readable and scannable.".to_string()),
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

    // Get sandbox status
    let sandbox_status = crate::external::get_sandbox_status();

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

    // Show sandbox status if run_command tool is available
    {
        println!("|  Sandbox: {:51} |", sandbox_status);
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

#[allow(clippy::too_many_arguments)]
async fn auto_compact_if_needed(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
    system_prompt: &str,
    context_window: usize,
    use_debug: bool,
) {
    let status = check_context_overflow(
        session,
        system_prompt,
        context_window,
        DEFAULT_OVERFLOW_THRESHOLD,
    );

    if !status.needs_compaction() {
        return;
    }

    // Show indicator before starting compaction
    let urgency = if status.is_overflow() {
        "urgent"
    } else {
        "auto"
    };
    eprintln!(
        "\x1B[33m⏳ Compacting context ({}% full)...\x1B[0m",
        status.usage_percent()
    );

    // Attempt auto-compaction
    match compact_conversation(ollama, model_config, session, settings, agents_md).await {
        Ok((summary, range)) => {
            session.set_compacted_summary_with_range(summary, range);

            // Get compacted count
            let (first_preserved, last_preserved_start) =
                range.unwrap_or((0, session.messages.len()));
            let compacted_count = last_preserved_start - first_preserved;

            eprintln!(
                "\x1B[90m[{}-compacted: {} messages summarized]\x1B[0m",
                urgency, compacted_count
            );

            if !session.anonymous
                && let Err(e) = session.save_sqlite()
                && use_debug
            {
                log_debug(&format!(
                    "Warning: Could not save session after auto-compact: {}",
                    e
                ));
            }
        }
        Err(e) => {
            eprintln!("\x1B[31mAuto-compaction failed: {}\x1B[0m", e);
        }
    }
}

fn print_context_info(
    session: &ChatSession,
    model_config: &ModelConfig,
    tools_enabled: bool,
    agents_md: Option<&str>,
    settings: &Settings,
    soulless: bool,
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
            .with_tools(tools_enabled)
            .with_retrieval(session.retrieval_enabled)
            .with_soulless(soulless),
    );

    let history_messages = session.get_messages_for_llm(&system_prompt);
    let context_window = model_config.num_ctx as usize;

    let tool_count = if tools_enabled {
        get_available_tool_names(settings).len()
    } else {
        0
    };

    const TOKENS_PER_TOOL: usize = 50;
    let tools_tokens = if tools_enabled && tool_count > 0 {
        tool_count * TOKENS_PER_TOOL
    } else {
        0
    };

    // Get real token count from history (if available)
    let real_history_tokens = session.history_real_tokens();
    let real_tokens_opt = if real_history_tokens > 0 {
        Some(real_history_tokens)
    } else {
        None
    };

    let metrics = calculate_context_metrics(
        &history_messages,
        context_window,
        &system_prompt,
        tools_tokens,
        real_tokens_opt,
    );

    let context_window_k = context_window / 1024;

    // Calculate usage percentage
    let usage_percent = (metrics.utilization * 100.0) as u8;

    // Visual bar (20 chars wide)
    let bar_width = 20;
    let filled = ((usage_percent as usize).min(100) * bar_width) / 100;
    let empty = bar_width - filled;

    // Color code based on usage
    let (color_code, reset_code, status_text) = if usage_percent < 72 {
        ("\x1B[32m", "\x1B[0m", "OK") // Green
    } else if usage_percent < 80 {
        ("\x1B[33m", "\x1B[0m", "MODERATE") // Yellow
    } else {
        ("\x1B[31m", "\x1B[0m", "CRITICAL") // Red
    };

    println!();
    println!("Context Information:");
    println!(
        "  Model:          {} ({}K context)",
        model_config.model_id, context_window_k
    );
    println!();
    println!("  Context Utilization:");
    println!(
        "    {}{}{}{} {}{}",
        color_code,
        "█".repeat(filled),
        "░".repeat(empty),
        reset_code,
        color_code,
        usage_percent
    );
    println!(
        "    {}{} / {} tokens{}\x1B[0m",
        color_code, metrics.total_tokens, context_window, reset_code
    );
    println!();
    println!("  Status: {}", status_text);
    println!();
    println!("  Token Breakdown:");
    println!("    System prompt:    ~{} tokens", metrics.system_tokens);
    if tools_enabled && tool_count > 0 {
        println!(
            "    Tool definitions: ~{} tokens ({} tools)",
            metrics.tools_tokens, tool_count
        );
    }

    // Show correct message count (active messages, not all)
    let active_messages = if session.has_compacted_messages() {
        session.messages.len() - session.messages_sent_to_llm
    } else {
        session.messages.len()
    };

    // Show real token count if available (from Ollama's prompt_eval_count)
    if metrics.total_tokens > 0 {
        // When we have real tokens, total = system + tools + history
        // history_tokens is derived: total - system - tools
        println!("    History:          ~{} tokens", metrics.history_tokens);
        if session.has_compacted_messages() {
            println!(
                "                      ({} active messages + summary)",
                active_messages
            );
        } else {
            println!("                      ({} messages)", active_messages);
        }
    } else {
        // Fallback estimation
        if session.has_compacted_messages() {
            println!(
                "    Summary:          ~{} tokens",
                estimate_tokens(session.compacted_summary.as_deref().unwrap_or("")) + 4
            );
            println!(
                "    Conversation:     ~{} tokens ({} active messages)",
                metrics.history_tokens, active_messages
            );
        } else {
            println!(
                "    Conversation:     ~{} tokens ({} messages)",
                metrics.history_tokens, active_messages
            );
        }
    }

    println!("    {}", "─".repeat(40));
    println!("    Total used:       ~{} tokens", metrics.total_tokens);
    println!("    Available:        ~{} tokens", metrics.available());
    println!();

    if session.has_compacted_messages() {
        println!("  Session:");
        println!(
            "    Compacted:        {} messages summarized",
            session.compacted_message_count()
        );
        println!("    Active:           {} messages", active_messages);
        println!("    Total:            {} messages", session.messages.len());
    } else {
        println!("  Session:");
        println!("    Total:            {} messages", session.messages.len());
    }
    println!();
}
