//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use std::sync::Arc;

use termimad::terminal_size;
use unicode_width::UnicodeWidthStr;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::debug_tools::{enable_debug, log_debug};
use crate::settings::Settings;
use crate::tokens::calculate_context_metrics;
use crate::tool_robustness::format_tool_error;
use crate::tools::get_available_tool_names;

use super::command_handlers::{HandleResult, handle_command_result, handle_model_switch};
use super::commands::{ChatCommand, execute_command, parse_command};
use super::continuation::{
    OverflowHandleResult, ProcessResult, build_inter_tool_compaction_prompt, build_pre_tool_prompt,
    check_and_compact_before_tool, handle_overflow_error, process_send_result,
};
use super::core::send_message;
use super::input::{InputBackend, InputResult, RustylineInput};
use super::session::ChatSession;
use super::view::TerminalView;
use crate::facts::db::DecayStats;
use crate::project::get_project_id;

/// Token overhead for each tool definition (approximate)
const TOKENS_PER_TOOL: usize = 50;

/// Number of lines in the status bar (separator, content, separator)
const STATUS_BAR_LINES: usize = 3;

/// Prompt prefix displayed before user input
const PROMPT_PREFIX: &str = ">>> ";

/// Maximum compaction cycles per message to prevent infinite loops
const MAX_COMPACTION_CYCLES: usize = 3;

/// Calculate the number of visual lines a string will occupy in the terminal
///
/// Uses Unicode-aware width calculation to handle CJK and other wide characters.
/// Returns at least 1 line.
fn calculate_visual_lines(input: &str, prompt_len: usize, terminal_width: usize) -> usize {
    if terminal_width == 0 {
        return 1; // Fallback: assume single line (visual artifacts acceptable)
    }

    // Unicode-aware width calculation
    let input_width = input.width();
    let total_width = prompt_len + input_width;

    // Ceiling division: how many lines does the input occupy?
    total_width.div_ceil(terminal_width).max(1)
}

/// Build ANSI escape code to clear status bar and input lines
fn build_clear_code(visual_lines: usize) -> String {
    let lines_to_clear = STATUS_BAR_LINES + visual_lines;
    format!("\x1B[{}A\x1B[J", lines_to_clear)
}

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize database and embedding client for chat mode.
/// Returns (db, embedding_client, ollama, error_message).
/// error_message is Some when database initialization fails for non-anonymous sessions.
#[allow(clippy::type_complexity)]
fn init_chat_database(
    args: &super::ChatArgs,
    use_debug: bool,
    settings: &Settings,
) -> (
    Option<Arc<crate::db::Database>>,
    Option<Arc<crate::embeddings::EmbeddingClient>>,
    ollama_rs::Ollama,
    Option<String>,
) {
    let ollama = settings.ollama_client();

    if args.anonymous {
        return (None, None, ollama, None);
    }

    let (db, embedding) = crate::db::init_database_core(ollama.clone(), false, use_debug);

    let error_detail = if db.is_none() {
        let storage_path = crate::db::Database::get_storage_path();
        let error_msg = format!(
            "\n\
             ══════════════════════════════════════════════════════════════\n\
             DATABASE INITIALIZATION FAILED\n\
             ══════════════════════════════════════════════════════════════\n\
             \n\
             Storage path: {}\n\
             \n\
             Possible causes:\n\
             1. sqlite-vec extension not loaded (check Ollama installation)\n\
             2. Permission denied for storage directory\n\
             3. Corrupted database file (try deleting and restarting)\n\
             4. Disk full or I/O error\n\
             \n\
             To diagnose:\n\
             - Check if Ollama is running: ollama list\n\
             - Check directory permissions: ls -la ~/.local/share/ask-ai/\n\
             - Run with --debug for more information\n\
             \n\
             Use --anonymous for anonymous mode without database persistence.\n\
             ══════════════════════════════════════════════════════════════",
            storage_path.display()
        );
        eprintln!("{}", error_msg);
        Some(error_msg)
    } else {
        None
    };

    (db, embedding, ollama, error_detail)
}

/// Run startup tasks (decay cycle).
async fn run_startup_tasks(
    db: &Option<Arc<crate::db::Database>>,
    _embedding_client: &Option<Arc<crate::embeddings::EmbeddingClient>>,
    anonymous: bool,
) {
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
async fn handle_user_message(line: &str, state: &mut super::repl_state::ReplState) {
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
    let mut compaction_cycles = 0;
    let mut current_input = line.to_string();

    loop {
        match send_message(
            &state.ollama,
            &state.model_config,
            &mut state.session,
            &current_input,
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
                break;
            }
            Err(e) => {
                let error_str = e.to_string();
                match handle_overflow_error(state, &error_str).await {
                    OverflowHandleResult::NotOverflow => {
                        eprintln!("\x1B[31m{}\x1B[0m", format_tool_error(&error_str));
                        break;
                    }
                    OverflowHandleResult::HandledContinue => {
                        eprintln!("\x1B[33mPlease retry your message.\x1B[0m");
                        break;
                    }
                    OverflowHandleResult::InterToolCompaction { tools_executed } => {
                        compaction_cycles += 1;

                        if compaction_cycles > MAX_COMPACTION_CYCLES {
                            eprintln!(
                                "\x1B[33mMaximum compaction cycles reached ({}). Please continue manually.\x1B[0m",
                                MAX_COMPACTION_CYCLES
                            );
                            break;
                        }

                        if state.use_debug {
                            let remaining_cycles = MAX_COMPACTION_CYCLES - compaction_cycles;
                            log_debug(&format!(
                                "[Inter-tool Compaction] Cycle {}/{} ({} tools executed before pause)",
                                compaction_cycles,
                                MAX_COMPACTION_CYCLES,
                                tools_executed.len()
                            ));
                            if remaining_cycles > 0 {
                                log_debug(&format!(
                                    "[Inter-tool Compaction] {} compaction(s) remaining before manual intervention",
                                    remaining_cycles
                                ));
                            }
                        }

                        eprintln!("\x1B[33m\x1B[33mContinuing...\x1B[0m");

                        current_input = build_inter_tool_compaction_prompt(&tools_executed);
                        continue;
                    }
                }
            }
        }
    }
}

/// Information about loaded session (for display after banner)
pub struct SessionLoadResult {
    pub session: ChatSession,
    pub resume_message: Option<String>,
}

/// Load or create a chat session based on args.
fn create_session(
    args: &super::ChatArgs,
    db: &Option<Arc<crate::db::Database>>,
    project_id: &Option<String>,
    model_override: Option<&str>,
    default_model: &str,
    use_debug: bool,
) -> SessionLoadResult {
    if args.anonymous {
        if use_debug {
            log_debug("Anonymous mode: starting fresh session without history");
        }
        return SessionLoadResult {
            session: ChatSession::new(
                model_override.unwrap_or(default_model).to_string(),
                None,
                true,
            ),
            resume_message: None,
        };
    }

    if let Some(session_name) = &args.load {
        if let Some(db_ref) = db {
            match ChatSession::load_sqlite(db_ref, session_name) {
                Ok(s) => {
                    let msg_count = s.messages.len();
                    return SessionLoadResult {
                        session: s,
                        resume_message: Some(format!(
                            "Loaded session: {} ({} messages)",
                            session_name, msg_count
                        )),
                    };
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
                    return SessionLoadResult {
                        session: new_session,
                        resume_message: None,
                    };
                }
            }
        }
        return SessionLoadResult {
            session: ChatSession::new(
                model_override.unwrap_or(default_model).to_string(),
                project_id.clone(),
                false,
            ),
            resume_message: None,
        };
    }

    // Try to load the most recent session by updated_at
    if let Some(db_ref) = db {
        match db_ref.get_last_session_id(project_id.as_deref()) {
            Ok(Some(last_id)) => match ChatSession::load_sqlite(db_ref, &last_id) {
                Ok(s) => {
                    let display_name = s.name.as_deref().unwrap_or(&s.id).to_string();
                    let msg_count = s.messages.len();
                    return SessionLoadResult {
                        session: s,
                        resume_message: Some(format!(
                            "Resumed session: {} ({} messages)",
                            display_name, msg_count
                        )),
                    };
                }
                Err(e) => {
                    eprintln!("Warning: Could not load session '{}': {}", last_id, e);
                    println!("Starting new session...");
                }
            },
            Ok(None) => {
                // No sessions exist - create new session (not persisted yet)
                if use_debug {
                    log_debug("No existing sessions found, creating new session");
                }
            }
            Err(e) => {
                eprintln!("Warning: Could not query sessions: {}", e);
                println!("Starting new session...");
            }
        }
    }

    // Create new session (not persisted until first message)
    SessionLoadResult {
        session: ChatSession::new(
            model_override.unwrap_or(default_model).to_string(),
            project_id.clone(),
            false,
        ),
        resume_message: None,
    }
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

    let (db, embedding_client, ollama, db_error) = init_chat_database(args, use_debug, settings);

    // FAIL FAST: Cannot continue without database for non-anonymous session
    if !args.anonymous && db.is_none() {
        if db_error.is_some() {
            // Error already printed in init_database
            eprintln!("\nFATAL: Cannot start chat session without database.");
            eprintln!("Either fix the database issue or use --anonymous mode.\n");
        }
        return Ok(());
    }

    run_startup_tasks(&db, &embedding_client, args.anonymous).await;

    // Load or create session (returns info without printing yet)
    let session_load_result = create_session(
        args,
        &db,
        &project_id,
        model_override,
        default_model,
        use_debug,
    );
    let mut session = session_load_result.session;
    let resume_message = session_load_result.resume_message;

    // Apply CLI flags (CLI takes precedence over args)
    let ignore_agents = cli_ignore_agents || args.ignore_agents;

    // Validate and set model
    if !resolve_session_model(&mut session, model_override, default_model) {
        return Ok(());
    }

    let current_model_name = session.model.clone();
    let model_config = crate::user_models::resolve_model_config(&current_model_name);

    let capabilities = ModelCapabilities::detect_or_default(&ollama, &model_config.model_id).await;

    // PRINT BANNER FIRST (before any other output)
    let (fact_count, note_count, doc_count) = if let Some(db_ref) = &db {
        (
            db_ref.count_facts().unwrap_or(0),
            db_ref.count_notes().unwrap_or(0),
            db_ref.count_documents().unwrap_or(0),
        )
    } else {
        (0, 0, 0)
    };
    print_welcome(
        &session,
        &model_config,
        &capabilities,
        settings,
        fact_count,
        note_count,
        doc_count,
    );

    // Print session info (if any)
    if let Some(msg) = resume_message {
        println!("{}", msg);
    }

    // Attach database to session
    if let (Some(db_ref), Some(client)) = (&db, &embedding_client) {
        session.attach_db(Arc::clone(db_ref), Arc::clone(client));

        // Regenerate embeddings if needed (after schema migration)
        // This runs once after v6→v7 migration to rebuild embeddings from content
        let stats = crate::embeddings::regenerate_all_embeddings(db_ref, client).await;
        if stats.total_processed() > 0 {
            println!(
                "Regenerated {} embedding(s) ({} items, {} chunks)",
                stats.total_processed(),
                stats.items_processed,
                stats.chunks_processed
            );
            if stats.has_errors() {
                println!(
                    "Warning: {} embedding(s) failed to generate. They will be retried on next startup.",
                    stats.total_failed()
                );
            }
        }

        // Recover any missing embeddings from previous session
        let recovered = crate::embeddings::recover_missing_embeddings(db_ref, client).await;
        if recovered > 0 {
            println!("Recovered {} missing embedding(s)", recovered);
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

    // Print help line AFTER all startup messages
    print!("{}", super::view::WelcomeInfo::help_line());

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
        // Build status bar info
        let status_bar = build_status_bar(&state);

        // Print status bar (3 lines total)
        print!("{}", status_bar);

        // Simple prompt with just ">>> "
        let readline = input.read_line(">>> ");

        match readline {
            InputResult::Line(ref line) => {
                let line = line.trim();
                if line.is_empty() {
                    // Clear status bar only (no input to clear)
                    print!("\x1B[{}A\x1B[J", STATUS_BAR_LINES);
                    continue;
                }

                input.add_history(line);

                // Calculate visual lines for proper ANSI clearing
                let prompt_len = PROMPT_PREFIX.len();
                let (cols, _) = terminal_size();
                let visual_lines = if cols > 0 {
                    calculate_visual_lines(line, prompt_len, cols as usize)
                } else {
                    1 // Fallback: assume 1 line
                };

                // Clear status bar and input lines
                print!("{}", build_clear_code(visual_lines));
                println!(">>> {}", line);

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
                // Clear status bar only
                print!("\x1B[{}A\x1B[J", STATUS_BAR_LINES);
                println!("^C");
                continue;
            }
            InputResult::Eof => {
                // Clear status bar only
                print!("\x1B[{}A\x1B[J", STATUS_BAR_LINES);
                println!("^D");
                let _ = input.save_history();
                if !state.session.anonymous {
                    let _ = state.session.save_sqlite();
                }
                return Ok(());
            }
            InputResult::Error(err) => {
                // Clear status bar only
                print!("\x1B[{}A\x1B[J", STATUS_BAR_LINES);
                eprintln!("Error: {}", err);
                break;
            }
        }
    }

    let _ = input.save_history();
    Ok(())
}

// Helper functions (REPL-specific, not moved to core.rs)

/// Build the status bar string for display above prompt
///
/// Includes model name, context usage, progress bar, and indicators.
fn build_status_bar(state: &super::repl_state::ReplState) -> String {
    use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};

    let blacklist_set = state.settings.blacklist_set();

    let prompt_type = if state.tools_active {
        PromptType::ToolUser
    } else {
        PromptType::Default
    };

    let system_prompt = build_system_prompt(
        PromptConfig::new(prompt_type)
            .with_model_id(Some(&state.model_config.model_id))
            .with_blacklist(Some(&blacklist_set))
            .with_agents_md(state.agents_md.as_deref())
            .with_tools(state.tools_active)
            .with_retrieval(state.session.retrieval_enabled)
            .with_soulless(state.cli_soulless),
    );

    let context_window = state.model_config.num_ctx as usize;

    // Get tool count
    let tool_count = if state.tools_active {
        get_available_tool_names(&state.settings).len()
    } else {
        0
    };

    let tools_tokens = if state.tools_active && tool_count > 0 {
        tool_count * TOKENS_PER_TOOL
    } else {
        0
    };

    // Get real tokens if available, or use estimate
    let real_history_tokens = state.session.history_real_tokens();
    let real_tokens_opt = if real_history_tokens > 0 {
        Some(real_history_tokens)
    } else {
        None
    };

    let history_messages = state.session.get_messages_for_llm(&system_prompt);

    let metrics = calculate_context_metrics(
        &history_messages,
        context_window,
        &system_prompt,
        tools_tokens,
        real_tokens_opt,
    );

    // Update cache
    let info = state.get_status_bar_info(metrics);
    info.format_status_bar()
}

fn print_welcome(
    session: &ChatSession,
    model_config: &ModelConfig,
    capabilities: &ModelCapabilities,
    settings: &crate::settings::Settings,
    fact_count: i64,
    note_count: i64,
    doc_count: i64,
) {
    let project = session.project_id.as_deref().unwrap_or("anonymous");
    let session_name = session.name.as_deref().unwrap_or(&session.id);
    let sandbox_status = crate::external::get_sandbox_status();
    let version = env!("CARGO_PKG_VERSION");
    let server_url = format!(
        "{}:{}",
        settings.model.ollama_host, settings.model.ollama_port
    );

    let mut view = TerminalView::new();
    view.show_welcome(
        &model_config.model_id,
        session.tools && capabilities.tools,
        session.think && capabilities.thinking,
        capabilities.vision,
        sandbox_status,
        project,
        session_name,
        session.anonymous,
        version,
        &server_url,
        fact_count,
        note_count,
        doc_count,
    );
}
