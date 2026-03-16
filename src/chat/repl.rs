//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use std::sync::Arc;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::debug_tools::{enable_debug, log_debug};
use crate::settings::Settings;
use crate::tool_robustness::format_tool_error;

use super::commands::{ChatCommand, execute_command, parse_command};
use super::command_handlers::{
    handle_command_result, HandleResult,
    handle_model_switch,
};
use super::continuation::{
    handle_overflow_error,
    check_and_compact_before_tool, build_pre_tool_prompt,
    process_send_result, ProcessResult,
};
use super::core::send_message;
use super::input::{InputBackend, InputResult, RustylineInput};
use super::session::ChatSession;
use super::view::TerminalView;
use crate::project::get_project_id;
use crate::facts::db::DecayStats;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize database and embedding client.
fn init_database(
    args: &super::ChatArgs,
    use_debug: bool,
    settings: &Settings,
) -> (Option<Arc<crate::db::Database>>, Option<Arc<crate::embeddings::EmbeddingClient>>, ollama_rs::Ollama) {
    let ollama = settings.ollama_client();
    let db = if !args.anonymous {
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

    let embedding_client = db.as_ref().map(|_| {
        Arc::new(crate::embeddings::EmbeddingClient::new(ollama.clone()))
    });

    (db, embedding_client, ollama)
}

/// Run startup tasks (migration and decay cycle).
async fn run_startup_tasks(
    db: &Option<Arc<crate::db::Database>>,
    embedding_client: &Option<Arc<crate::embeddings::EmbeddingClient>>,
    anonymous: bool,
) {
    if let (Some(db_ref), Some(client)) = (db, embedding_client)
        && !anonymous
    {
        let migration_stats = crate::db::migrate_all_legacy_sessions(db_ref, client).await;
        if migration_stats.sessions_migrated > 0 {
            log_debug(&format!(
                "Migrated {} session(s) from JSON to SQLite",
                migration_stats.sessions_migrated
            ));
        }
    }

    if let Some(db_ref) = db
        && !anonymous
    {
        match db_ref.run_decay_cycle() {
            Ok(DecayStats { pruned, remaining }) => {
                if pruned > 0 {
                    log_debug(&format!(
                        "Facts decay: pruned {} old facts, {} remaining",
                        pruned, remaining
                    ));
                }
            }
            Err(e) => {
                log_debug(&format!("Warning: Facts decay cycle failed: {}", e));
            }
        }
    }
}

/// Handle user input that's not a command.
async fn handle_user_message(
    line: &str,
    state: &mut super::repl_state::ReplState,
) {
    let user_message_id = state.session.add_user_message(line.to_string());
    if !state.session.anonymous
        && let Err(e) = state.session.save_sqlite()
        && state.use_debug
    {
        log_debug(&format!("Warning: Could not save session: {}", e));
    }

    let context_window = state.model_config.num_ctx as usize;
    let system_prompt_for_check = build_pre_tool_prompt(state);
    check_and_compact_before_tool(state, &system_prompt_for_check, context_window).await;

    let think_enabled = state.session.think;
    match send_message(
        &state.ollama,
        &state.model_config,
        &mut state.session,
        line,
        state.tools_active,
        think_enabled,
        state.cli_code,
        &state.settings,
        state.agents_md.as_deref(),
        state.use_debug,
        state.db.as_ref(),
        state.embedding_client.as_ref(),
        state.cli_soulless,
        None,
    )
    .await
    {
        Ok(result) => {
            match process_send_result(state, result, user_message_id).await {
                ProcessResult::Success => {}
                ProcessResult::ContinuationError(e) => {
                    eprintln!("\x1B[31mContinuation failed: {}\x1B[0m", e);
                }
            }
        }
        Err(e) => {
            let error_str = e.to_string();
            if !handle_overflow_error(state, &error_str).await {
                eprintln!("\x1B[31m{}\x1B[0m", format_tool_error(&error_str));
            }
        }
    }
}

/// Load or create a chat session based on args.
fn create_session(
    args: &super::ChatArgs,
    db: &Option<Arc<crate::db::Database>>,
    project_id: &Option<String>,
    model_override: Option<&str>,
    default_model: &str,
    use_debug: bool,
) -> ChatSession {
    if args.anonymous {
        if use_debug {
            log_debug("Anonymous mode: starting fresh session without history");
        }
        return ChatSession::new(
            model_override.unwrap_or(default_model).to_string(),
            None,
            true,
        );
    }

    if let Some(session_name) = &args.load {
        if let Some(db_ref) = db {
            match ChatSession::load_sqlite(db_ref, session_name) {
                Ok(s) => {
                    println!(
                        "Loaded session: {} ({} messages)",
                        session_name,
                        s.messages.len()
                    );
                    return s;
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
                    return new_session;
                }
            }
        }
        return ChatSession::new(
            model_override.unwrap_or(default_model).to_string(),
            project_id.clone(),
            false,
        );
    }

    if let Some(db_ref) = db {
        let default_id = "default";
        if let Ok(true) = db_ref.conversation_exists(default_id) {
            match ChatSession::load_sqlite(db_ref, default_id) {
                Ok(s) => {
                    println!(
                        "Resumed session: {} ({} messages)",
                        default_id,
                        s.messages.len()
                    );
                    return s;
                }
                Err(e) => {
                    eprintln!("Warning: Could not load default session: {}", e);
                    println!("Starting new session...");
                    return ChatSession::new(
                        model_override.unwrap_or(default_model).to_string(),
                        project_id.clone(),
                        false,
                    );
                }
            }
        }
    }

    ChatSession::new(
        model_override.unwrap_or(default_model).to_string(),
        project_id.clone(),
        false,
    )
}

/// Validate and set the model for a session.
fn resolve_session_model(
    session: &mut ChatSession,
    model_override: Option<&str>,
    default_model: &str,
) -> bool {
    if let Some(model) = model_override {
        if crate::user_models::is_model_valid(model) {
            session.set_model(model.to_string());
            return true;
        }
        eprintln!(
            "Error: Unknown model '{}'. Use --list to see available models.",
            model
        );
        return false;
    }

    if !crate::user_models::is_model_valid(&session.model) {
        eprintln!(
            "Warning: Saved model '{}' no longer exists. Using default '{}'.",
            session.model, default_model
        );
        session.set_model(default_model.to_string());
    }
    true
}

/// Determine thinking mode from CLI flags, config, and model capabilities.
fn resolve_thinking_mode(
    cli_think: bool,
    config_thinking: bool,
    model_config: &ModelConfig,
    capabilities: &ModelCapabilities,
) -> bool {
    let cli_think_flag = cli_think;
    let model_default_thinking = model_config.thinking;

    if cli_think_flag {
        if !capabilities.thinking {
            eprintln!(
                "Warning: Model '{}' does not support think mode. Ignoring -t/--think flag.",
                model_config.model_id
            );
            return false;
        }
        return true;
    }

    let requested_thinking = config_thinking || model_default_thinking;
    if requested_thinking && !capabilities.thinking {
        eprintln!(
            "Warning: Model '{}' does not support think mode. Disabled for this session.",
            model_config.model_id
        );
        return false;
    }
    requested_thinking
}

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

    let (config_model, config_thinking, config_tools) = settings.get_subcommand_config("chat");

    let model_override = cli_model.or(args.model.as_deref());
    let default_model = if !config_model.is_empty() {
        &config_model
    } else {
        &settings.model.default
    };

    let (db, embedding_client, ollama) = init_database(args, use_debug, settings);
    run_startup_tasks(&db, &embedding_client, args.anonymous).await;

    // Load or create session
    let mut session = create_session(
        args,
        &db,
        &project_id,
        model_override,
        default_model,
        use_debug,
    );

    // Apply CLI flags (CLI takes precedence over args)
    let ignore_agents = cli_ignore_agents || args.ignore_agents;

    // Validate and set model
    if !resolve_session_model(&mut session, model_override, default_model) {
        return Ok(());
    }

    let current_model_name = session.model.clone();
    let model_config = crate::user_models::resolve_model_config(&current_model_name);

    let capabilities =
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
    let think_enabled = resolve_thinking_mode(
        cli_think_flag,
        config_thinking,
        &model_config,
        &capabilities,
    );

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

    let tools_active = session.tools && capabilities.tools;

    if session.tools && !capabilities.tools {
        eprintln!(
            "Warning: Tools are enabled but model '{}' does not support tool calling.",
            model_config.model_id
        );
        eprintln!("         Tools have been disabled for this session. Use /tools to toggle.");
    }

    // Phase 8: Create ReplState to consolidate mutable state
    // Created AFTER initialization, right before the loop
    // We pass cloned/copied values for incremental migration.
    // Immutable values (use_debug, cli_code, cli_soulless, agents_md) are accessed via state.
    // Mutable values (session, model_config, capabilities, tools_active) currently have
    // BOTH local variables AND state fields - migration is in progress.
    let mut state = super::repl_state::ReplStateBuilder::new()
        .session(session.clone()) // Clone for state; session var still primary
        .model_config(model_config.clone()) // Clone for state
        .capabilities(capabilities.clone()) // Clone for state
        .tools_active(tools_active) // Copy (bool) - now accessible as state.tools_active
        .agents_md(agents_md.clone()) // Clone for state
        .use_debug(use_debug) // Copy (bool) - now accessible as state.use_debug
        .cli_code(cli_code) // Copy (bool) - now accessible as state.cli_code
        .cli_soulless(cli_soulless) // Copy (bool) - now accessible as state.cli_soulless
        .ollama(ollama.clone()) // Clone for state
        .db(db.clone()) // Arc clone (cheap)
        .embedding_client(embedding_client.clone()) // Arc clone (cheap)
        .settings(settings.clone()) // Clone for state
        .build()?;

    // Initialize global todo state from session
    crate::tools::todo::load_from_session(&state.session.todos);

    // Initialize input backend using RustylineInput abstraction
    let model_names: Vec<String> = crate::user_models::list_all_model_names();
    let mut input = RustylineInput::new(model_names);

    loop {
        let mut prompt = state.current_model_name.clone();
        if state.session.think && state.capabilities.thinking {
            prompt.push_str("[t]");
        }
        if state.tools_active {
            prompt.push_str("[T]");
        }
        prompt.push_str("> ");

        let readline = input.read_line(&prompt);

        match readline {
            InputResult::Line(ref line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                input.add_history(line);

                if line.starts_with('/') {
                    match parse_command(line) {
                        Some(Ok(cmd)) => {
                            if let ChatCommand::Model { name } = &cmd {
                                let _ = handle_model_switch(&mut state, name, &capabilities).await;
                                continue;
                            }

                            let result = execute_command(cmd, &mut state.session);
                            match handle_command_result(result, &mut state, &mut input).await {
                                HandleResult::Continue => continue,
                                HandleResult::Exit => return Ok(()),
                            }
                        }
                        Some(Err(e)) => {
                            eprintln!("{}", e);
                            continue;
                        }
                        None => {}
                    }
                }

                handle_user_message(line, &mut state).await;
            }
            InputResult::Interrupted => {
                println!("^C");
                continue;
            }
            InputResult::Eof => {
                println!("^D");
                let _ = input.save_history();
                if !state.session.anonymous {
                    let _ = state.session.save_sqlite();
                }
                return Ok(());
            }
            InputResult::Error(err) => {
                eprintln!("Error: {}", err);
                break;
            }
        }
    }

    let _ = input.save_history();
    Ok(())
}

// Helper functions (REPL-specific, not moved to core.rs)

fn print_welcome(
    session: &ChatSession,
    model_config: &ModelConfig,
    capabilities: &ModelCapabilities,
) {
    let project = session.project_id.as_deref().unwrap_or("anonymous");
    let session_name = session.name.as_deref().unwrap_or(&session.id);
    let sandbox_status = crate::external::get_sandbox_status();

    let mut view = TerminalView::new();
    view.show_welcome(
        &model_config.model_id,
        session.tools && capabilities.tools,
        session.think && capabilities.thinking,
        sandbox_status,
        project,
        session_name,
        session.anonymous,
    );
}
