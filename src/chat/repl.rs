//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles session initialization, model detection, and delegates to
//! the TUI event loop (`repl_tui`) for interactive chat.
//!
//! The terminal REPL loop (rustyline) has been replaced by the ratatui-based
//! TUI in PR2. This module now handles pre-TUI setup
//! only (database, session, model detection) and then calls
//! `run_chat_repl_tui()` for the interactive loop.

#![expect(clippy::print_stderr)] // Pre-TUI session warnings/errors to stderr

use std::path::PathBuf;
use std::sync::Arc;

use crate::capabilities::ModelCapabilities;
use crate::capabilities::check_server_health;
use crate::config::ModelConfig;
use crate::settings::Settings;

use super::continuation::{
    OverflowHandleResult, ProcessResult, build_inter_tool_compaction_prompt, build_pre_tool_prompt,
    check_and_compact_before_tool, handle_overflow_error, process_send_result,
};
use super::core::send_message_stream;
use super::llm_event::LlmEvent;
use super::session::{ChatSession, MessageRole};
use super::view::ChatView;

use crate::facts::db::DecayStats;
use crate::facts::extract::extract_and_insert_facts;
use crate::project::get_project_id;
use crate::tool_robustness::format_tool_error;

/// Maximum compaction cycles per message to prevent infinite loops
const MAX_COMPACTION_CYCLES: usize = 3;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Initialize database and embedding client for chat mode.
/// Returns (db, embedding_client, ollama, error_message).
/// error_message is Some when database initialization fails for non-anonymous sessions.
#[expect(clippy::type_complexity)]
fn init_chat_database(
    args: &super::ChatArgs,
    settings: &Settings,
    db_path: Option<PathBuf>,
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

    let result = crate::db::init_database_core(ollama.clone(), false, false, db_path);

    let error_detail = if result.db.is_none() {
        // Error already logged and formatted in init_database_core
        if let Some(ref detail) = result.error_detail {
            eprintln!("\x1B[31m{}\x1B[0m", detail);
            Some(detail.clone())
        } else {
            None
        }
    } else {
        None
    };

    // NOTE: normalize_inline_thinking() is called in the background spawn
    // (repl_tui.rs), not here. It runs before embedding recovery to ensure
    // normalized items (has_embedding=0) are picked up for regeneration.

    (result.db, result.embedding, ollama, error_detail)
}

/// Run startup tasks (decay cycles).
async fn run_startup_tasks(
    db: &Option<Arc<crate::db::Database>>,
    _embedding_client: &Option<Arc<crate::embeddings::EmbeddingClient>>,
    anonymous: bool,
    settings: &Settings,
) {
    if let Some(db_ref) = db
        && !anonymous
    {
        match db_ref.run_decay_cycle() {
            Ok(DecayStats { pruned, remaining }) => {
                if pruned > 0 {
                    log::debug!(
                        "Facts decay: pruned {} old facts, {} remaining",
                        pruned,
                        remaining
                    );
                }
            }
            Err(e) => {
                log::debug!("Warning: Facts decay cycle failed: {}", e);
            }
        }

        // Content decay cycle (gated by settings.feedback.content_decay)
        if settings.feedback.content_decay {
            match db_ref.with_connection(|conn| {
                crate::db::content_decay_ops::run_content_decay_cycle(conn).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
                })
            }) {
                Ok(crate::content::decay::ContentDecayStats {
                    pruned,
                    remaining,
                    avg_retention,
                }) => {
                    if pruned > 0 {
                        log::debug!(
                            "Content decay: pruned {} items, {} remaining (avg retention: {:.2})",
                            pruned,
                            remaining,
                            avg_retention
                        );
                    }
                }
                Err(e) => {
                    log::debug!("Warning: Content decay cycle failed: {}", e);
                }
            }
        }
    }
}

/// Handle user input with streaming token display.
///
/// This is the streaming equivalent of `handle_user_message()`. Instead of
/// waiting for the full LLM response before displaying it, tokens are streamed
/// through the `LlmEvent` channel for incremental display in the TUI.
///
/// The `llm_tx` sender is used for:
/// - `LlmEvent::StreamToken(token)` — each content token chunk
/// - `LlmEvent::StreamThinking(token)` — each thinking token chunk
/// - `LlmEvent::StreamDone` — final content and metrics after stream completes
/// - `LlmEvent::ViewAction(action)` — view events from coordinator callbacks
///
/// The `cancel_token` allows aborting the stream on Ctrl+C.
pub async fn handle_user_message_stream(
    line: &str,
    state: &mut super::repl_state::ReplState,
    view: &mut dyn ChatView,
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
    cancel_token: tokio_util::sync::CancellationToken,
) {
    let user_message_id = state.session.add_user_message(line.to_string());
    if !state.session.anonymous
        && let Err(e) = state.session.save_sqlite()
    {
        log::debug!("Warning: Could not save session: {}", e);
    }

    let context_window = state.model_config.num_ctx as usize;
    let system_prompt_for_check = build_pre_tool_prompt(state);
    check_and_compact_before_tool(
        state,
        &system_prompt_for_check,
        context_window,
        view,
        llm_tx.clone(),
    )
    .await;

    let think_enabled = state.session.think;
    let mut compaction_cycles = 0;
    let mut current_input = line.to_string();

    loop {
        match send_message_stream(
            &state.ollama,
            &state.model_config,
            &mut state.session,
            &current_input,
            state.tools_active,
            think_enabled,
            state.cli_code,
            &state.settings,
            state.agents_md.as_deref(),
            state.db.as_ref(),
            state.embedding_client.as_ref(),
            state.cli_soulless,
            None,
            view,
            llm_tx.clone(),
            Some(cancel_token.clone()),
        )
        .await
        {
            Ok(result) => {
                match process_send_result(state, result, user_message_id, view, llm_tx.clone())
                    .await
                {
                    ProcessResult::Success => {
                        // Auto-extract facts from recent user messages (autoDream-lite)
                        try_auto_extract_facts(state, view).await;
                    }
                    ProcessResult::ContinuationError(e) => {
                        view.show_error(&format!("Continuation failed: {}", e));
                    }
                }
                break;
            }
            Err(e) => {
                let error_str = e.to_string();

                // Cancellation from tool loop (Ctrl+C during multi-tool execution)
                // is user-initiated — don't show an error message, just stop.
                // The Ctrl+C handler in the event loop already showed "Cancelled."
                if error_str == super::custom_coordinator::CANCELLED_BY_USER {
                    log::debug!("LLM task cancelled during tool execution");
                    break;
                }

                match handle_overflow_error(state, &error_str, view, llm_tx.clone()).await {
                    OverflowHandleResult::NotOverflow => {
                        view.show_error(&format_tool_error(&error_str));
                        break;
                    }
                    OverflowHandleResult::HandledContinue => {
                        view.show_warning("Please retry your message.");
                        break;
                    }
                    OverflowHandleResult::InterToolCompaction { tools_executed } => {
                        compaction_cycles += 1;

                        if compaction_cycles > MAX_COMPACTION_CYCLES {
                            view.show_warning(&format!(
                                "Maximum compaction cycles reached ({}). Please continue manually.",
                                MAX_COMPACTION_CYCLES
                            ));
                            break;
                        }

                        let remaining_cycles = MAX_COMPACTION_CYCLES - compaction_cycles;
                        log::debug!(
                            "[Inter-tool Compaction] Cycle {}/{} ({} tools executed before pause)",
                            compaction_cycles,
                            MAX_COMPACTION_CYCLES,
                            tools_executed.len()
                        );
                        if remaining_cycles > 0 {
                            log::debug!(
                                "[Inter-tool Compaction] {} compaction(s) remaining before manual intervention",
                                remaining_cycles
                            );
                        }

                        view.show_progress("Continuing...");

                        current_input = build_inter_tool_compaction_prompt(&tools_executed);
                        continue;
                    }
                }
            }
        }
    }
}

/// Attempt auto-extraction of facts from recent user messages (autoDream-lite).
///
/// This is called synchronously after each successful response. Extraction is:
/// - Disabled for anonymous sessions (no database)
/// - Gated by `settings.facts.auto_extract`
/// - Limited to `settings.facts.max_facts` per response
/// - Notification gated by `settings.facts.auto_extract_notify`
///
/// See ADR-E1 (heuristic-only), ADR-E2 (always Global), ADR-E5 (synchronous).
async fn try_auto_extract_facts(state: &mut super::repl_state::ReplState, view: &mut dyn ChatView) {
    // Guard: auto_extract must be enabled
    if !state.settings.facts.auto_extract {
        return;
    }

    // Guard: anonymous sessions have no database
    if state.session.anonymous {
        return;
    }

    // Guard: database must be available
    let Some(db) = &state.db else {
        return;
    };

    // Collect recent user messages (up to MAX_MESSAGES_TO_SCAN)
    let user_messages: Vec<&str> = state
        .session
        .messages
        .iter()
        .rev()
        .filter(|m| m.role == MessageRole::User)
        .take(5)
        .map(|m| m.content.as_str())
        .collect();

    if user_messages.is_empty() {
        return;
    }

    let max_facts = state.settings.facts.max_facts as usize;
    let project_id = state.session.project_id.as_deref();

    let result = extract_and_insert_facts(
        db,
        &user_messages,
        project_id,
        max_facts,
        state.embedding_client.as_ref(),
    )
    .await;

    // Log the extraction result
    if result.inserted > 0 || result.updated > 0 {
        log::debug!(
            "Auto-extract: {} inserted, {} updated, {} skipped",
            result.inserted,
            result.updated,
            result.skipped
        );
        for detail in &result.details {
            log::debug!(
                "Auto-extract detail: {} [{:?}] — {}",
                detail.action,
                detail.category,
                detail.content
            );
        }
    }

    // Show notification if configured and any facts were extracted
    if state.settings.facts.auto_extract_notify {
        let total = result.inserted + result.updated;
        if total > 0 {
            view.show_system(&format!("[Auto-extracted: {} fact(s)]", total));
        }
    }

    // Generate embeddings for newly inserted facts (eager, fire-and-forget).
    // Semantic dedup (Layer 3.5) already generates embeddings for preference facts.
    // This covers the remaining cases: identity facts, facts added without an
    // embedding client, and facts where Layer 3.5 was skipped.
    if (result.inserted > 0 || result.updated > 0)
        && let (Some(db_ref), Some(client)) = (&state.db, &state.embedding_client)
    {
        let db_clone = Arc::clone(db_ref);
        let client_clone = Arc::clone(client);
        tokio::spawn(async move {
            crate::facts::recovery::recover_missing_fact_embeddings(&db_clone, &client_clone, None)
                .await;
        });
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
) -> SessionLoadResult {
    if args.anonymous {
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
                    log::warn!("Could not load session '{}': {}", session_name, e);
                    eprintln!(
                        "\x1B[33m⚠️ Could not load session '{}': {}\x1B[0m",
                        session_name, e
                    );
                    eprintln!("Starting new session...");
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
                    log::warn!("Could not load most recent session '{}': {}", last_id, e);
                    eprintln!(
                        "\x1B[33m⚠️ Could not load session '{}': {}\x1B[0m",
                        last_id, e
                    );
                    eprintln!("Starting new session...");
                }
            },
            Ok(None) => {
                // No sessions exist - create new session (not persisted yet)
            }
            Err(e) => {
                log::warn!("Could not query sessions: {}", e);
                eprintln!("\x1B[33m⚠️ Could not query sessions: {}\x1B[0m", e);
                eprintln!("Starting new session...");
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
///
/// Prints warnings/errors to stderr before the TUI takes over the terminal.
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
        log::error!("Unknown model specified: '{}'", model);
        eprintln!(
            "\x1B[31mUnknown model '{}'. Use --list to see available models.\x1B[0m",
            model
        );
        return false;
    }

    if !crate::user_models::is_model_valid(&session.model) {
        log::warn!(
            "Saved model '{}' no longer exists. Using default '{}'.",
            session.model,
            default_model
        );
        eprintln!(
            "\x1B[33m⚠️ Saved model '{}' no longer exists. Using default '{}'.\x1B[0m",
            session.model, default_model
        );
        session.set_model(default_model.to_string());
    }
    true
}

/// Determine thinking mode from CLI flags, config, and model capabilities.
///
/// Prints warnings to stderr before the TUI takes over the terminal.
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
            log::warn!(
                "Model '{}' does not support think mode. Ignoring -t/--think flag.",
                model_config.model_id
            );
            eprintln!(
                "\x1B[33m⚠️ Model '{}' does not support think mode. Ignoring -t/--think flag.\x1B[0m",
                model_config.model_id
            );
            return false;
        }
        return true;
    }

    let requested_thinking = config_thinking || model_default_thinking;
    if requested_thinking && !capabilities.thinking {
        log::warn!(
            "Model '{}' does not support think mode. Disabled for this session.",
            model_config.model_id
        );
        eprintln!(
            "\x1B[33m⚠️ Model '{}' does not support think mode. Disabled for this session.\x1B[0m",
            model_config.model_id
        );
        return false;
    }
    requested_thinking
}

/// Run the interactive chat REPL
#[expect(clippy::too_many_arguments)]
pub async fn run_chat_repl(
    settings: &Settings,
    args: &super::ChatArgs,
    cli_model: Option<&str>,
    cli_think: bool,
    cli_tools: bool,
    cli_code: bool,
    cli_ignore_agents: bool,
    cli_soulless: bool,
    db_path: Option<PathBuf>,
) -> AppResult<()> {
    let project_id = if args.anonymous {
        None
    } else {
        get_project_id()
    };

    if log::log_enabled!(log::Level::Debug) {
        if let Some(ref pid) = project_id {
            log::debug!("Project ID: {}", pid);
        } else {
            log::debug!("Running in anonymous mode (no persistence)");
        }
    }

    let (config_model, config_thinking, config_tools) = settings.get_subcommand_config("chat");

    let model_override = cli_model.or(args.model.as_deref());
    let default_model = if !config_model.is_empty() {
        &config_model
    } else {
        &settings.model.default
    };

    // W2 Wave Context (#116): Health check the Ollama server BEFORE
    // initializing the database. This prevents the startup hang reported
    // during manual testing: when Ollama is unreachable, the heavy
    // init_chat_database path could hang indefinitely because ollama-rs
    // does not expose a configurable request timeout. The health check
    // has a 3s timeout and fails fast with a clear error message.
    // Resolved permanently by #120 (OllamaProvider reqwest direct).
    let pre_init_ollama = settings.ollama_client();
    if let Err(e) = check_server_health(&pre_init_ollama).await {
        log::error!("Ollama health check failed: {e}");
        eprintln!("\x1B[31mError: {e}\x1B[0m");
        eprintln!(
            "\x1B[33mHint: Start Ollama with `ollama serve` in another terminal, then retry.\x1B[0m"
        );
        return Ok(());
    }

    let (db, embedding_client, ollama, db_error) = init_chat_database(args, settings, db_path);

    // FAIL FAST: Cannot continue without database for non-anonymous session
    if !args.anonymous && db.is_none() {
        if db_error.is_some() {
            // Error already printed in init_database
            log::error!("Cannot start chat session without database.");
            eprintln!("\x1B[31mCannot start chat session without database.\x1B[0m");
            eprintln!("Either fix the database issue or use --anonymous mode.");
        }
        return Ok(());
    }

    run_startup_tasks(&db, &embedding_client, args.anonymous, settings).await;

    // Load or create session (returns info without printing yet)
    let session_load_result = create_session(args, &db, &project_id, model_override, default_model);
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

    // Attach database to session (needed before TUI for embedding recovery)
    if let (Some(db_ref), Some(client)) = (&db, &embedding_client) {
        session.attach_db(Arc::clone(db_ref), Arc::clone(client));
    }

    let agents_md = if !ignore_agents {
        crate::context::load_agents_md()
    } else {
        None
    };

    let tools_active = session.tools && capabilities.tools;

    // Build ReplState — all display (welcome, resume, DB messages, etc.)
    // is handled by run_chat_repl_tui() inside the TUI.
    let mut state = super::repl_state::ReplStateBuilder::new()
        .session(session.clone())
        .model_config(model_config.clone())
        .capabilities(capabilities.clone())
        .tools_active(tools_active)
        .agents_md(agents_md.clone())
        .cli_code(cli_code)
        .cli_soulless(cli_soulless)
        .ollama(ollama.clone())
        .db(db.clone())
        .embedding_client(embedding_client.clone())
        .settings(settings.clone())
        .build()?;

    // Initialize global todo state from session
    crate::tools::todo::load_from_session(&state.session.todos);

    // Enter TUI mode — all display is handled by RatatuiView from here on
    super::repl_tui::run_chat_repl_tui(&mut state, resume_message).await
}

// Helper functions (REPL-specific, not moved to core.rs)
