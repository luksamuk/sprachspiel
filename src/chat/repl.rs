//! Chat REPL - Interactive read-eval-print loop
//!
//! Handles the main chat loop, user input, and model interaction.

use std::sync::Arc;

use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::context_overflow::{PRE_TOOL_THRESHOLD, check_context_overflow, needs_pre_tool_compaction};
use crate::debug_tools::{enable_debug, log_debug};
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::settings::Settings;
use crate::tokens::{calculate_context_metrics, estimate_tokens};
use crate::tool_robustness::format_tool_error;
use crate::tools::get_available_tool_names;

use super::commands::{CommandResult, execute_command, parse_command};
use super::command_handlers::{
    handle_think_toggled, handle_tools_toggled, handle_retrieval_toggled,
    handle_tool_output_changed, handle_debug_toggled, handle_undo,
    handle_search, handle_restore, handle_reindex, handle_compact, handle_retry,
};
use super::core::{auto_compact_if_needed, send_message};
use super::input::{InputBackend, InputResult, RustylineInput};
use super::session::ChatSession;
use super::view::TerminalView;
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
                            if let super::commands::ChatCommand::Model { name } = &cmd {
                                match super::model_switch::switch_model(
                                    name,
                                    &state.ollama,
                                    &capabilities,
                                    state.session.think,
                                    state.session.tools,
                                )
                                .await
                                {
                                    Ok(result) => {
                                        state.session.set_model(result.model_name.clone());
                                        state.session.think = result.think_active;
                                        state.session.tools = result.tools_active;

                                        state.current_model_name = result.model_name.clone();
                                        state.model_config = result.model_config;
                                        state.capabilities = result.capabilities;
                                        state.tools_active = result.tools_active;

                                        for warning in &result.warnings {
                                            eprintln!("{}", warning);
                                        }

                                        println!(
                                            "Model switched to: {} ({})",
                                            result.model_name, state.model_config.model_id
                                        );

                                        if !state.session.anonymous {
                                            let _ = state.session.save_sqlite();
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("{}", e);
                                    }
                                }
                                continue;
                            }

                            match execute_command(cmd, &mut state.session) {
                                CommandResult::Continue => continue,
                                CommandResult::Exit => {
                                    let _ = input.save_history();
                                    if !state.session.anonymous {
                                        let _ = state.session.save_sqlite();
                                    }
                                    return Ok(());
                                }
                                CommandResult::Error(e) => {
                                    eprintln!("Error: {}", e);
                                    continue;
                                }
                                CommandResult::ThinkToggled(new_state) => {
                                    handle_think_toggled(&mut state, new_state);
                                    continue;
                                }
                                CommandResult::ToolsToggled(new_state) => {
                                    handle_tools_toggled(&mut state, new_state);
                                    continue;
                                }
                                CommandResult::Compact => {
                                    handle_compact(&mut state).await;
                                    continue;
                                }
                                CommandResult::ToolOutputChanged(level) => {
                                    handle_tool_output_changed(level);
                                    continue;
                                }
                                CommandResult::DebugToggled(new_state) => {
                                    handle_debug_toggled(new_state);
                                    continue;
                                }
                                CommandResult::RetrievalToggled(new_state) => {
                                    handle_retrieval_toggled(&state, new_state);
                                    continue;
                                }
                                CommandResult::Context => {
                                    print_context_info(
                                        &state.session,
                                        &state.model_config,
                                        state.tools_active,
                                        state.agents_md.as_deref(),
                                        &state.settings,
                                        state.cli_soulless,
                                    );
                                    continue;
                                }
                                CommandResult::Retry => {
                                    handle_retry(&mut state).await;
                                    continue;
                                }
                                CommandResult::Undo => {
                                    handle_undo(&mut state);
                                    continue;
                                }
                                CommandResult::Search { query, limit } => {
                                    handle_search(&state, query, limit).await;
                                    continue;
                                }
                                CommandResult::Restore { session_id } => {
                                    handle_restore(&mut state, session_id);
                                    continue;
                                }
                                CommandResult::Reindex { conversation_id } => {
                                    handle_reindex(&mut state, conversation_id).await;
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
                let user_message_id = state.session.add_user_message(line.to_string());
                if !state.session.anonymous
                    && let Err(e) = state.session.save_sqlite()
                    && state.use_debug
                {
                    log_debug(&format!("Warning: Could not save session: {}", e));
                }

                // Pre-tool context check: Auto-compact BEFORE tool execution if context is high
                // This prevents context exhaustion during multi-tool turns
                let context_window = state.model_config.num_ctx as usize;
                let system_prompt_for_check = build_system_prompt(
                    PromptConfig::new(PromptType::ToolUser)
                        .with_model_id(Some(&state.model_config.model_id))
                        .with_blacklist(Some(&state.settings.blacklist_set()))
                        .with_agents_md(state.agents_md.as_deref())
                        .with_tools(state.tools_active)
                        .with_retrieval(state.session.retrieval_enabled && !state.cli_code)
                        .with_soulless(state.cli_soulless),
                );

                if needs_pre_tool_compaction(&state.session, &system_prompt_for_check, context_window) {
                    let usage_pct = check_context_overflow(
                        &state.session,
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
                        &state.ollama,
                        &state.model_config,
                        &mut state.session,
                        &state.settings,
                        state.agents_md.as_deref(),
                        &system_prompt_for_check,
                        context_window,
                        state.use_debug,
                    )
                    .await;
                }

                let think_enabled = state.session.think;
                match send_message(
                    &state.ollama,
                    &state.model_config,
                    &mut state.session,
                    line,
                    state.tools_active,
                    think_enabled,
                    state.cli_code, // from function parameter
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
                        // Save pre-tool content before final response
                        if let Some(pre_content) = &result.pre_tool_content {
                            state.session.add_pre_tool_message(
                                pre_content.clone(),
                                result.pre_tool_thinking.clone(),
                                user_message_id,
                            );
                            if state.use_debug {
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
                            if state.use_debug {
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
                                &state.ollama,
                                &state.model_config,
                                &mut state.session,
                                &state.settings,
                                state.agents_md.as_deref(),
                                &continuation_system_prompt,
                                continuation_context_window,
                                state.use_debug,
                            )
                            .await;

                            // Continue with continuation prompt

                            // Send continuation request (empty user_input, continuation via ephemeral)
                            let continuation_result = send_message(
                                &state.ollama,
                                &state.model_config,
                                &mut state.session,
                                "", // empty user_input - continuation via ephemeral message
                                state.tools_active,
                                think_enabled,
                                state.cli_code,
                                &state.settings,
                                state.agents_md.as_deref(),
                                state.use_debug,
                                state.db.as_ref(),
                                state.embedding_client.as_ref(),
                                state.cli_soulless,
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
                                            &state.ollama,
                                            &state.model_config,
                                            &mut state.session,
                                            &state.settings,
                                            state.agents_md.as_deref(),
                                            &cont_result.system_prompt,
                                            cont_result.context_window,
                                            state.use_debug,
                                        )
                                        .await;

                                        let next_result = send_message(
                                            &state.ollama,
                                            &state.model_config,
                                            &mut state.session,
                                            "", // empty user_input - continuation via ephemeral
                                            state.tools_active,
                                            think_enabled,
                                            state.cli_code,
                                            &state.settings,
                                            state.agents_md.as_deref(),
                                            state.use_debug,
                                            state.db.as_ref(),
                                            state.embedding_client.as_ref(),
                                            state.cli_soulless,
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
                        state.session.add_assistant_message(
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
                            &state.ollama,
                            &state.model_config,
                            &mut state.session,
                            &state.settings,
                            state.agents_md.as_deref(),
                            &system_prompt,
                            context_window,
                            state.use_debug,
                        )
                        .await;

                        if !state.session.anonymous
                            && let Err(e) = state.session.save_sqlite()
                            && state.use_debug
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
                                state.session.remove_last_assistant_messages_with_content();
                            if state.use_debug {
                                log_debug(&format!(
                                    "Removed {} messages after overflow error",
                                    removed
                                ));
                            }

                            // Auto-compact to free space
                            let overflow_context_window = state.model_config.num_ctx as usize;
                            let overflow_system_prompt = build_system_prompt(
                                PromptConfig::new(PromptType::ToolUser)
                                    .with_model_id(Some(&state.model_config.model_id))
                                    .with_blacklist(Some(&state.settings.blacklist_set()))
                                    .with_agents_md(state.agents_md.as_deref())
                                    .with_tools(state.tools_active)
                                    .with_retrieval(state.session.retrieval_enabled && !state.cli_code)
                                    .with_soulless(state.cli_soulless),
                            );

                            eprintln!("\x1B[33m⏳ Auto-compacting after overflow error...\x1B[0m");
                            auto_compact_if_needed(
                                &state.ollama,
                                &state.model_config,
                                &mut state.session,
                                &state.settings,
                                state.agents_md.as_deref(),
                                &overflow_system_prompt,
                                overflow_context_window,
                                state.use_debug,
                            )
                            .await;

                            // Save session after compaction
                            if !state.session.anonymous
                                && let Err(save_err) = state.session.save_sqlite()
                                && state.use_debug
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
