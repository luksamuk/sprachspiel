//! Command handlers for the chat REPL
//!
//! This module provides command handlers that operate on `ReplState`.
//! Each handler returns `Vec<CommandOutput>` instead of printing directly.
//! The REPL loop renders outputs via `ChatView::show_command_outputs()`.
//!
//! # Architecture (W6-PR1, Issue #145)
//!
//! ```text
//! command_handlers.rs
//!     ↓ returns
//! Vec<CommandOutput>  (semantic data, no ANSI codes)
//!     ↓ consumed by
//! ChatView::show_command_outputs()
//!     ↓ implemented by
//! RatatuiView (TUI chat) ─── standalone renderer (query/translate/summarize/OCR)
//! ```
//!
//! # Handler Pattern
//!
//! Handlers return `Vec<CommandOutput>` and modify `ReplState` directly.
//! The `CommandOutput::Quit` variant signals REPL exit (replaces HandleResult::Exit).
//! All output styling is applied by the view layer, not by handlers.

use std::sync::Arc;

use super::command_output::{
    CommandOutput, CompactData, ContentPruneData, ContextData, DocumentEntry, DocumentListData,
    ExportData, ExportFormat, FactListData, FactListScopeData, FactRemoveResult, FactSearchData,
    FactSearchResult, GcData, NoteAddResult, NoteListData, ReindexData, SearchData, SessionEntry,
    SessionListData, SkillEntry, SkillListData, TodoListData,
};
use super::commands::{ChatCommand, FactListScope};
use super::repl_state::ReplState;
use super::session::ToolOutputLevel;

/// Approximate token overhead per tool definition in the system prompt.
const TOKENS_PER_TOOL: usize = 50;
use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::embeddings::{DEFAULT_CONTEXT_LENGTH, EmbedItemContext, embed_item_with_fallback};
use crate::settings::Settings;
use crate::tokens::{calculate_context_metrics, estimate_tokens};

pub use super::session::ChatSession;

/// Handle a chat command in the REPL loop.
///
/// Dispatches to the appropriate handler based on the command type.
/// Returns `Vec<CommandOutput>` for rendering via `ChatView`.
///
/// `CommandOutput::Quit` signals REPL exit (replaces former `HandleResult::Exit`).
/// An empty vec means no output (replaces `HandleResult::Continue` with no message).
#[allow(clippy::too_many_lines)] // Dispatch table: each arm is a trivial handler call.
pub async fn handle_command(
    cmd: ChatCommand,
    state: &mut ReplState,
    input: &mut (dyn super::input::InputBackend + Send),
    view: &mut dyn super::view::ChatView,
    llm_tx: tokio::sync::mpsc::Sender<super::llm_event::LlmEvent>,
) -> Vec<CommandOutput> {
    match cmd {
        ChatCommand::Quit => handle_quit(state, input, view.suppress_progress_spinner()).await,
        ChatCommand::Forget { confirmed } => handle_forget_cmd(state, confirmed),
        ChatCommand::New => handle_new(state),
        ChatCommand::Help => {
            let help_text = super::commands::format_help();
            vec![CommandOutput::HelpText(help_text)]
        }
        // Note: Model switching is handled directly in repl.rs via model_switch module
        ChatCommand::Model { name: _ } => vec![],
        ChatCommand::System { prompt } => {
            state.session.set_system_prompt(prompt);
            vec![CommandOutput::info("System prompt updated.")]
        }
        ChatCommand::Save { name } => handle_save_cmd(state, name),
        ChatCommand::Load { name } => handle_load_cmd(state, name),
        ChatCommand::Export { format, file } => handle_export(&state.session, format, file),
        ChatCommand::List => handle_list(state),
        ChatCommand::Info => {
            let info = super::commands::format_session_info(&state.session, None);
            vec![CommandOutput::Info(info)]
        }
        ChatCommand::Context => {
            let info = format_context_info(
                &state.session,
                &state.model_config,
                state.tools_active,
                state.agents_md.as_deref(),
                &state.settings,
                state.cli_soulless,
                state.db.as_ref(),
            );
            vec![CommandOutput::ContextInfo(ContextData { formatted: info })]
        }
        ChatCommand::Think { enabled } => match enabled {
            Some(on) => {
                state.session.think = on;
                vec![handle_think_toggled(state, on)]
            }
            None => {
                state.session.think = !state.session.think;
                vec![handle_think_toggled(state, state.session.think)]
            }
        },
        ChatCommand::Tools => {
            state.session.tools = !state.session.tools;
            vec![handle_tools_toggled(state, state.session.tools)]
        }
        ChatCommand::Compact => handle_compact(state, view, llm_tx).await,
        ChatCommand::ToolsOutput { level } => {
            state.session.tool_output_level = level;
            vec![handle_tool_output_changed(level)]
        }
        ChatCommand::Debug => handle_debug_toggle(),
        ChatCommand::Retry => handle_retry(state, view, llm_tx).await,
        ChatCommand::Undo => handle_undo(state),
        ChatCommand::Search { query, limit } => handle_search(state, query, limit).await,
        ChatCommand::Reindex { confirmed } => handle_reindex_cmd(state, confirmed).await,
        ChatCommand::Retrieval => {
            state.session.retrieval_enabled = !state.session.retrieval_enabled;
            handle_retrieval_toggled(state, state.session.retrieval_enabled)
        }
        ChatCommand::FactPrune => handle_fact_prune(state),
        ChatCommand::Gc => handle_gc(state),
        ChatCommand::FactAdd { content, global } => handle_fact_add(state, content, global).await,
        ChatCommand::FactList { scope } => handle_fact_list(state, scope),
        ChatCommand::FactRemove { id } => handle_fact_remove(state, id),
        ChatCommand::FactSearch {
            query,
            global,
            limit,
        } => handle_fact_search(state, query, global, limit),
        ChatCommand::TodoAdd {
            description,
            priority,
            tags,
        } => handle_todo_add(description, priority, tags, &mut state.session),
        ChatCommand::TodoList { filter } => handle_todo_list(filter),
        ChatCommand::TodoUpdate { id, status } => {
            handle_todo_update(id, status, &mut state.session)
        }
        ChatCommand::TodoGet { id } => handle_todo_get(id),
        ChatCommand::TodoEdit {
            id,
            description,
            priority,
            tags,
        } => handle_todo_edit(id, description, priority, tags, &mut state.session),
        ChatCommand::TodoDelete { id } => handle_todo_delete(id, &mut state.session),
        ChatCommand::TodoClearDone => handle_todo_clear_done(&mut state.session),
        ChatCommand::TodoClearAll => handle_todo_clear_all(&mut state.session),
        ChatCommand::NoteAdd {
            content,
            title,
            global,
        } => handle_note_add(state, content, title, global),
        ChatCommand::NoteList { global, page } => handle_note_list(state, global, page),
        ChatCommand::NoteShow { id } => handle_note_show(state, id),
        ChatCommand::NoteEdit { id, title, content } => handle_note_edit(state, id, title, content),
        ChatCommand::NoteDelete { id } => handle_note_delete(state, id),
        ChatCommand::NoteSearch {
            query,
            global,
            limit,
        } => handle_note_search(state, query, global, limit),
        ChatCommand::DocumentImport {
            path,
            global,
            nowait,
        } => handle_document_import(state, path, global, nowait),
        ChatCommand::DocumentList { global } => handle_document_list(state, global),
        ChatCommand::DocumentShow { id } => handle_document_show(state, id),
        ChatCommand::DocumentDelete { id } => handle_document_delete(state, id),
        ChatCommand::Skill { name } => handle_skill_cmd(state, name),
        ChatCommand::SkillList => handle_skill_list_cmd(),
        ChatCommand::Ocr { path, mode } => handle_subagent_ocr(state, path, mode).await,
        ChatCommand::Vision { paths, prompt } => handle_subagent_vision(state, paths, prompt).await,
        ChatCommand::Translate { lang_pair, text } => {
            handle_subagent_translate(state, lang_pair, text).await
        }
        ChatCommand::Summarize { text } => handle_subagent_summarize(state, text).await,
        ChatCommand::Feedback {
            signal_type,
            item_id,
            correction_text,
        } => handle_feedback(state, signal_type, item_id, correction_text),
        ChatCommand::ContentPrune => handle_content_prune(state),
        // ToggleStyle is handled directly in repl_tui.rs (needs App access).
        // This arm exists for match exhaustiveness; it is never reached
        // from the TUI because the command is intercepted before dispatch.
        ChatCommand::ToggleStyle => {
            vec![CommandOutput::Info(
                "Style rendering: (no change)".to_string(),
            )]
        }
    }
}

/// Handle /quit command — save session and exit.
///
/// Embedding recovery is NOT performed on exit — it runs on next startup
/// via the background recovery pipeline. This avoids blocking exit while
/// hundreds of embeddings are generated synchronously.
///
/// No embedding flush is needed on exit because:
/// - Insert-time embedding generation is fire-and-forget (`tokio::spawn`).
///   If Ollama is online during the chat, embeddings are generated eagerly.
/// - If Ollama was offline during insertion, `has_embedding` stays 0 and the
///   startup recovery pipeline retries on next boot.
/// - The previous synchronous flush could block exit for minutes with hundreds
///   of pending embeddings, which was the bug this design change fixed.
async fn handle_quit(
    state: &mut ReplState,
    input: &mut (dyn super::input::InputBackend + Send),
    _suppress_spinner: bool,
) -> Vec<CommandOutput> {
    let _ = input.save_history();
    if !state.session.anonymous {
        let _ = state.session.save_sqlite();
    }
    vec![CommandOutput::info("Goodbye!"), CommandOutput::quit()]
}

/// Handle /forget command — requires confirmation flag.
fn handle_forget_cmd(state: &mut ReplState, confirmed: bool) -> Vec<CommandOutput> {
    if !confirmed {
        return vec![
            CommandOutput::warning("/forget will permanently delete this conversation."),
            CommandOutput::warning("   Use /forget --yes to confirm."),
        ];
    }
    handle_forget(state)
}

/// Handle /save command — with error display wrapper.
fn handle_save_cmd(state: &mut ReplState, name: Option<String>) -> Vec<CommandOutput> {
    match handle_save(state, name) {
        Ok(outputs) => outputs,
        Err(e) => vec![CommandOutput::error(e)],
    }
}

/// Handle /load command — with error display wrapper.
fn handle_load_cmd(state: &mut ReplState, name: String) -> Vec<CommandOutput> {
    match handle_load(state, name) {
        Ok(outputs) => outputs,
        Err(e) => vec![CommandOutput::error(e)],
    }
}

/// Handle /debug command — toggle debug mode and print status.
fn handle_debug_toggle() -> Vec<CommandOutput> {
    let verbosity = crate::debug_tools::toggle_debug();
    let msg = match verbosity {
        crate::logging::Verbosity::Normal => "Debug mode: OFF (log level: info)".to_string(),
        crate::logging::Verbosity::Trace => "Debug mode: ON (log level: trace)".to_string(),
        _ => format!(
            "Debug mode: {} (log level: {:?})",
            verbosity,
            verbosity.to_level_filter()
        ),
    };
    vec![CommandOutput::info(msg)]
}

/// Handle /skill command — activate a skill by name.
fn handle_skill_cmd(state: &mut ReplState, name: String) -> Vec<CommandOutput> {
    let skill = crate::skills::get_skill_content(&name);
    match skill {
        Some(skill) => {
            handle_skill_activated(state, skill.name, skill.content);
            vec![]
        }
        None => {
            vec![CommandOutput::error(format!(
                "Skill '{}' not found. Use one of: {}",
                name,
                crate::skills::get_available_skill_names().join(", ")
            ))]
        }
    }
}

/// Handle /skilllist command — list available skills.
fn handle_skill_list_cmd() -> Vec<CommandOutput> {
    let skills = crate::skills::load_skill_indexes();
    if skills.is_empty() {
        return vec![CommandOutput::Info("No skills available.".into())];
    }
    let entries: Vec<SkillEntry> = skills
        .into_iter()
        .map(|s| SkillEntry {
            name: s.name,
            description: s.description,
        })
        .collect();
    vec![CommandOutput::SkillList(SkillListData { skills: entries })]
}

/// Handle /new command — start a new conversation session.
fn handle_new(state: &mut ReplState) -> Vec<CommandOutput> {
    // Check if there are searchable messages in database (any conversation)
    let has_searchable_messages = if let Some(ref db) = state.session.db {
        db.count_all_content_items()
            .map(|count| count > 0)
            .unwrap_or(false)
    } else {
        false
    };

    // Clear session state
    state.session.compacted_summary = None;
    state.session.messages.clear();
    state.session.messages_sent_to_llm = 0;
    state.session.compacted_range = None;
    state.session.name = None;

    // Generate new session ID
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    state.session.id = format!("session-{}", timestamp);

    // Reset timestamps
    let now = chrono::Utc::now();
    state.session.created_at = now;
    state.session.updated_at = now;

    let mut outputs = vec![CommandOutput::info("New session started.")];
    if has_searchable_messages {
        outputs.push(CommandOutput::info(
            "[i] Previous conversations remain searchable via /search or remember().",
        ));
    }
    outputs
}

/// Handle /forget command — delete conversation completely and start fresh.
fn handle_forget(state: &mut ReplState) -> Vec<CommandOutput> {
    state.session.forget_session();

    let mut outputs = Vec::new();

    if let Some(ref db) = state.session.db
        && !state.session.anonymous
        && !state.session.id.is_empty()
    {
        match db.delete_conversation(&state.session.id) {
            Ok(_) => outputs.push(CommandOutput::progress(
                "Removing conversation from database...",
            )),
            Err(e) => outputs.push(CommandOutput::warning(format!(
                "Could not delete conversation: {}",
                e
            ))),
        }
    }

    if !state.session.anonymous {
        // Generate new session ID using timestamp
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        state.session.id = format!("session-{}", timestamp);
        if let Err(e) = state.session.save_sqlite() {
            outputs.push(CommandOutput::warning(format!(
                "Could not save new session: {}",
                e
            )));
        }
    }

    outputs.push(CommandOutput::info(
        "Session forgotten. Starting fresh conversation.",
    ));
    outputs
}

/// Handle /save command — save current session.
fn handle_save(state: &mut ReplState, name: Option<String>) -> Result<Vec<CommandOutput>, String> {
    if state.session.anonymous {
        return Err(
            "Cannot save anonymous session. Use /save without --anonymous flag.".to_string(),
        );
    }

    if let Some(n) = name {
        state.session.rename(n);
    }

    match state.session.save_sqlite() {
        Ok(()) => {
            let session_name = state.session.name.as_deref().unwrap_or(&state.session.id);
            Ok(vec![CommandOutput::info(format!(
                "Session saved: {}",
                session_name
            ))])
        }
        Err(e) => Err(format!("Failed to save session: {}", e)),
    }
}

/// Handle /load command — load a saved session.
fn handle_load(state: &mut ReplState, name: String) -> Result<Vec<CommandOutput>, String> {
    let db = match &state.session.db {
        Some(d) => Arc::clone(d),
        None => {
            return Err("Cannot load session: database not initialized.".to_string());
        }
    };

    // Save current session if it has messages
    if !state.session.anonymous
        && !state.session.messages.is_empty()
        && let Err(e) = state.session.save_sqlite()
    {
        log::warn!("Could not save current session: {}", e);
    }

    match ChatSession::load_sqlite(&db, &name) {
        Ok(loaded) => {
            state.session = loaded;
            let display_name = state.session.name.as_deref().unwrap_or(&state.session.id);
            Ok(vec![CommandOutput::info(format!(
                "Loaded session: {} ({} messages)",
                display_name,
                state.session.messages.len()
            ))])
        }
        Err(e) => Err(format!("Failed to load session: {}", e)),
    }
}

/// Handle /export command — export conversation to file or stdout.
fn handle_export(
    session: &ChatSession,
    format: super::commands::ExportFormat,
    file: Option<String>,
) -> Vec<CommandOutput> {
    let content = match format {
        super::commands::ExportFormat::Markdown => export_markdown(session),
        super::commands::ExportFormat::Json => export_json(session),
    };

    let export_format = match format {
        super::commands::ExportFormat::Markdown => ExportFormat::Markdown,
        super::commands::ExportFormat::Json => ExportFormat::Json,
    };

    match file {
        Some(path) => {
            let expanded_path = crate::utils::expand_tilde_path(&path);
            match std::fs::write(&expanded_path, &content) {
                Ok(()) => vec![CommandOutput::ExportResult(ExportData {
                    content,
                    format: export_format,
                    file_path: Some(path),
                })],
                Err(e) => vec![CommandOutput::error(format!("Failed to write file: {}", e))],
            }
        }
        None => vec![CommandOutput::ExportResult(ExportData {
            content,
            format: export_format,
            file_path: None,
        })],
    }
}

/// Handle /list command — list saved sessions.
fn handle_list(state: &ReplState) -> Vec<CommandOutput> {
    let db = match &state.session.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Cannot list sessions: database not initialized.",
            )];
        }
    };

    match db.list_sessions(state.session.project_id.as_deref()) {
        Ok(sessions) => {
            if sessions.is_empty() {
                vec![CommandOutput::Info(
                    "No saved sessions for this project.".into(),
                )]
            } else {
                let entries: Vec<SessionEntry> = sessions
                    .into_iter()
                    .map(|info| {
                        let is_current = info.id == state.session.id;
                        let age_days = (chrono::Utc::now() - info.updated_at).num_days();
                        let updated_at = if age_days == 0 {
                            "today".to_string()
                        } else if age_days == 1 {
                            "yesterday".to_string()
                        } else {
                            format!("{}d ago", age_days)
                        };
                        SessionEntry {
                            name: info.name.unwrap_or_else(|| info.id.clone()),
                            message_count: info.message_count,
                            is_current,
                            updated_at: Some(updated_at),
                        }
                    })
                    .collect();
                vec![CommandOutput::SessionList(SessionListData {
                    sessions: entries,
                    is_empty: false,
                })]
            }
        }
        Err(e) => vec![CommandOutput::warning(format!(
            "Could not list sessions: {}",
            e
        ))],
    }
}

/// Export session as markdown
fn export_markdown(session: &ChatSession) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "# Chat Session: {}\n\n",
        session.name.as_deref().unwrap_or(&session.id)
    ));
    output.push_str(&format!("- **Model:** {}\n", session.model));
    output.push_str(&format!(
        "- **Created:** {}\n",
        session.created_at.format("%Y-%m-%d %H:%M")
    ));
    output.push_str(&format!("- **Messages:** {}\n\n", session.messages.len()));
    output.push_str("---\n\n");

    for msg in &session.messages {
        match msg.role {
            super::session::MessageRole::User => {
                output.push_str(&format!("**User:** {}\n\n", msg.content));
            }
            super::session::MessageRole::Assistant => {
                output.push_str(&format!("**Assistant:**\n\n{}\n\n", msg.content));
            }
            super::session::MessageRole::System => {
                output.push_str(&format!("**System:** {}\n\n", msg.content));
            }
            super::session::MessageRole::Tool => {
                output.push_str(&format!("**Tool:** {}\n\n", msg.content));
            }
        }
    }

    output
}

/// Export session as JSON
fn export_json(session: &ChatSession) -> String {
    serde_json::to_string_pretty(session).unwrap_or_else(|_| "{}".to_string())
}

/// Handle think mode toggle
///
/// Updates state based on the new toggle value. Prints warnings if
/// the model doesn't support thinking.
pub fn handle_think_toggled(state: &mut ReplState, new_state: bool) -> CommandOutput {
    if new_state && !state.capabilities.thinking {
        state.session.think = false;
        CommandOutput::warning(format!(
            "Model '{}' does not support think mode.",
            state.model_config.model_id
        ))
    } else {
        state.tools_active = state.session.tools && state.capabilities.tools;
        CommandOutput::info(format!(
            "Think mode: {}",
            if new_state { "enabled" } else { "disabled" }
        ))
    }
}

/// Handle tools toggle
///
/// Updates state based on the new toggle value. Prints warnings if
/// the model doesn't support tools.
pub fn handle_tools_toggled(state: &mut ReplState, new_state: bool) -> CommandOutput {
    if new_state && !state.capabilities.tools {
        state.session.tools = false;
        state.tools_active = false;
        CommandOutput::warning(format!(
            "Model '{}' does not support tools.",
            state.model_config.model_id
        ))
    } else {
        state.tools_active = new_state && state.capabilities.tools;
        CommandOutput::info(format!(
            "Tools: {}",
            if new_state { "enabled" } else { "disabled" }
        ))
    }
}

/// Handle retrieval mode toggle
///
/// Prints status message about the new retrieval state.
pub fn handle_retrieval_toggled(state: &ReplState, new_state: bool) -> Vec<CommandOutput> {
    if new_state {
        let mut outputs = vec![CommandOutput::info(
            "Semantic retrieval enabled. Messages will be retrieved from history for context.",
        )];
        if state.session.messages.len() < 20 {
            outputs.push(CommandOutput::info(format!(
                "Note: Retrieval activates after 20 messages (current: {})",
                state.session.messages.len()
            )));
        }
        outputs
    } else {
        vec![CommandOutput::info("Semantic retrieval disabled.")]
    }
}

/// Handle tool output level change
///
/// Prints the new tool output level.
pub fn handle_tool_output_changed(level: ToolOutputLevel) -> CommandOutput {
    CommandOutput::info(format!("Tool output level: {}", level))
}

/// Handle undo command
///
/// Removes the last assistant messages (including preceding user message)
/// and displays the remaining last user message.
pub fn handle_undo(state: &mut ReplState) -> Vec<CommandOutput> {
    let (removed, _) = state.session.remove_last_assistant_messages_with_content();
    state.last_assistant_message_id = None;
    let mut outputs = Vec::new();

    if removed > 0 {
        if !state.session.anonymous
            && !state.session.id.is_empty()
            && let Ok(db) = crate::db::Database::new()
            && let Err(e) = db.delete_last_content_items(&state.session.id, removed)
        {
            outputs.push(CommandOutput::warning(format!(
                "Failed to delete from database: {}",
                e
            )));
        }
        outputs.push(CommandOutput::info(format!(
            "Removed {} message(s) from session.",
            removed
        )));
    } else {
        outputs.push(CommandOutput::info("No messages to remove."));
    }

    if let Some(user_msg) = state.session.get_last_user_message() {
        outputs.push(CommandOutput::info(format!(
            "Last message: \"{}\"",
            user_msg.content
        )));
        outputs.push(CommandOutput::info(
            "(Press \u{2191} to retrieve and edit, or type a new message)",
        ));
    } else {
        outputs.push(CommandOutput::info("No user message to show."));
    }

    outputs
}

/// Handle search command (async)
///
/// Searches conversation history for matching messages.
/// Returns `SearchOutcome` data, which is converted to `CommandOutput` here.
pub async fn handle_search(state: &ReplState, query: String, limit: usize) -> Vec<CommandOutput> {
    let db = match crate::db::Database::new() {
        Ok(db) => db,
        Err(e) => {
            return vec![CommandOutput::error(format!(
                "Failed to open database: {}",
                e
            ))];
        }
    };

    let conversation_id = state.session.id.clone();

    log::debug!("Searching in conversation: {}", conversation_id);

    use crate::retrieval::{SearchOutcome, format_results};

    match crate::retrieval::run_search(&db, &state.ollama, &query, Some(&conversation_id), limit)
        .await
    {
        SearchOutcome::Results(results) => {
            if results.is_empty() {
                vec![CommandOutput::info("No results found.")]
            } else {
                match format_results(&results) {
                    Some(formatted) => {
                        vec![CommandOutput::SearchResults(SearchData { formatted })]
                    }
                    None => vec![CommandOutput::info("No results found.")],
                }
            }
        }
        SearchOutcome::EmbeddingError(msg) => vec![CommandOutput::error(msg)],
        SearchOutcome::SearchError(msg) => vec![CommandOutput::error(msg)],
        SearchOutcome::EnrichmentWarning {
            partial_results,
            error,
        } => {
            let mut outputs = vec![CommandOutput::warning(format!(
                "Enrichment warning: {}",
                error
            ))];
            if !partial_results.is_empty()
                && let Some(formatted) = format_results(&partial_results)
            {
                outputs.push(CommandOutput::SearchResults(SearchData { formatted }));
            }
            outputs
        }
    }
}

/// Handle `/reindex` command — requires `--yes` confirmation flag.
///
/// Without `--yes`, shows a warning explaining that `/reindex` regenerates
/// ALL embeddings from scratch. With `--yes`, resets all `has_embedding`
/// flags, deletes all vec0 embeddings, and spawns a background task to
/// regenerate them (the TUI stays responsive during regeneration).
///
/// Prevents concurrent execution: if a `/reindex --yes` is already running,
/// returns a warning instead of starting a second one.
pub async fn handle_reindex_cmd(state: &mut ReplState, confirmed: bool) -> Vec<CommandOutput> {
    if !confirmed {
        return vec![
            CommandOutput::warning("/reindex will regenerate ALL embeddings from scratch."),
            CommandOutput::warning("   This may take time depending on the amount of content."),
            CommandOutput::warning("   Use /reindex --yes to confirm."),
        ];
    }

    // Prevent concurrent reindex — another /reindex may be running
    if state
        .session
        .is_reindexing
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return vec![CommandOutput::warning(
            "Embedding reindex is already in progress. Please wait for it to finish.",
        )];
    }

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    // Reset all embedding flags and delete vec0 embeddings so that
    // regenerate_all_embeddings() will re-process every item from scratch.
    // This is done synchronously because it's fast (just SQL statements).
    let reset_stats = match db.reset_all_embedding_flags() {
        Ok(stats) => stats,
        Err(e) => {
            return vec![CommandOutput::error(format!(
                "Failed to reset embedding flags: {e}"
            ))];
        }
    };

    // If there's nothing to re-index, report immediately
    if reset_stats.items == 0 && reset_stats.facts == 0 {
        return vec![CommandOutput::info("No content to re-index.")];
    }

    let embedding_client = crate::embeddings::EmbeddingClient::new(state.ollama.clone());
    let embedding_client = Arc::new(embedding_client);
    let progress_tx = state.session.embedding_tx.clone();

    // TUI mode: spawn in background so the UI stays responsive.
    // The completion message arrives via the async_message channel
    // and is displayed by poll_async_messages() in the event loop.
    // Terminal mode: run synchronously and return the result directly.
    let async_message_tx = state.session.async_message_tx.clone();
    if async_message_tx.is_some() {
        // --- TUI mode: background execution ---
        state
            .session
            .is_reindexing
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let is_reindexing = state.session.is_reindexing.clone();
        tokio::spawn(async move {
            let result = crate::embeddings::regenerate_all_embeddings(
                &db,
                &embedding_client,
                true,
                progress_tx,
            )
            .await;

            // Build completion message
            let msg = if result.has_errors() {
                format!(
                    "✓ Reindexed {} of {} embeddings. {} failed — will be retried on next startup.",
                    result.total_processed(),
                    result.items_processed + result.chunks_processed,
                    result.total_failed()
                )
            } else {
                format!(
                    "✓ Reindexed {} of {} embeddings.",
                    result.total_processed(),
                    result.items_processed + result.chunks_processed
                )
            };

            // Send completion message to the TUI chat area
            if let Some(tx) = async_message_tx.as_ref() {
                let _ = tx.send(msg);
            }

            // Clear reindexing flag
            is_reindexing.store(false, std::sync::atomic::Ordering::Relaxed);
        });

        vec![CommandOutput::info(
            "Reindexing started in the background. Progress shown in status bar (⚙).",
        )]
    } else {
        // --- Terminal mode: synchronous execution ---
        let stats = crate::embeddings::regenerate_all_embeddings(
            &db,
            &embedding_client,
            false,
            progress_tx,
        )
        .await;

        vec![CommandOutput::ReindexResult(ReindexData {
            regenerated: stats.total_processed(),
            total: stats.items_processed + stats.chunks_processed,
            success: true,
            error: None,
        })]
    }
}

/// Handle compact command (async)
///
/// Compacts conversation history by summarizing old messages.
///
/// **Note:** The `_view` parameter is present for signature compatibility with
/// `dispatch_command()` but is not used by this handler. In the TUI path,
/// `/compact` is intercepted by the event loop (`spawn_compact_task()`) and
/// never reaches this function. This handler is only called via the non-TUI
/// (standalone renderer) path.
pub async fn handle_compact(
    state: &mut ReplState,
    _view: &mut dyn super::view::ChatView,
    llm_tx: tokio::sync::mpsc::Sender<super::llm_event::LlmEvent>,
) -> Vec<CommandOutput> {
    if state.session.messages.is_empty() {
        return vec![CommandOutput::info("No messages to compact.")];
    }

    let msg_count = state.session.messages.len();

    let mut outputs = vec![CommandOutput::progress(format!(
        "Compacting {} messages...",
        msg_count
    ))];

    match super::core::compact_conversation(
        &state.ollama,
        &state.model_config,
        &state.session,
        &state.settings,
        state.agents_md.as_deref(),
        llm_tx,
    )
    .await
    {
        Ok((summary, range)) => {
            let (first_preserved, last_preserved_start) =
                range.unwrap_or((0, state.session.messages.len()));
            let compacted_count = last_preserved_start - first_preserved;

            state
                .session
                .set_compacted_summary_with_range(summary.clone(), range);

            if first_preserved > 0 || last_preserved_start < state.session.messages.len() {
                // Middle compaction
                outputs.push(CommandOutput::CompactResult(CompactData {
                    count: compacted_count,
                    preserved_first: first_preserved,
                    preserved_last: state.session.messages.len() - last_preserved_start,
                }));
            } else {
                // Full compaction (backward compatible)
                outputs.push(CommandOutput::CompactResult(CompactData {
                    count: compacted_count,
                    preserved_first: 0,
                    preserved_last: 0,
                }));
            }

            // Render summary as markdown (no artificial header/footer)
            outputs.push(CommandOutput::MarkdownContent(summary));

            if !state.session.anonymous {
                let _ = state.session.save_sqlite();

                // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                if let Some(db) = state.session.db.as_ref() {
                    let _ = db.clear_conversation_prompt_tokens(&state.session.id);
                }
            }
        }
        Err(e) => {
            outputs.push(CommandOutput::error(format!("Compaction failed: {}", e)));
        }
    }

    outputs
}

/// Handle retry command (async)
///
/// Removes last assistant messages and regenerates the response.
///
/// # Bug fix: user message restoration
///
/// Previously, `get_last_user_message()` was called AFTER removing the assistant
/// messages (which also removes the preceding user message). This returned the
/// WRONG user message (the one before the last exchange, not the one being retried).
///
/// Now we capture the user content BEFORE removal, then restore it with
/// `add_user_message()` so the session history remains intact for the retry.
pub async fn handle_retry(
    state: &mut ReplState,
    view: &mut dyn super::view::ChatView,
    llm_tx: tokio::sync::mpsc::Sender<super::llm_event::LlmEvent>,
) -> Vec<CommandOutput> {
    use crate::tool_robustness::format_tool_error;

    let mut outputs = Vec::new();

    // Capture the user message BEFORE removing anything.
    // remove_last_assistant_messages_with_content() removes assistant messages
    // AND the preceding user message, so we must read it first.
    let user_content = state
        .session
        .get_last_user_message()
        .map(|m| m.content.clone());

    // Remove last assistant messages (and the preceding user message)
    let removed = state.session.remove_last_assistant_messages();
    if removed > 0 {
        outputs.push(CommandOutput::info(format!(
            "Removed {} assistant message(s). Ready to retry.",
            removed
        )));
    } else {
        outputs.push(CommandOutput::info("No assistant messages to remove."));
        return outputs;
    }

    // Restore the user message to session history so the LLM sees the correct
    // context and build_context() can include it properly.
    let Some(user_content) = user_content else {
        outputs.push(CommandOutput::info("No user message to retry."));
        return outputs;
    };

    state.session.add_user_message(user_content.clone());
    if !state.session.anonymous
        && let Err(e) = state.session.save_sqlite()
    {
        log::debug!("Warning: Could not save session: {}", e);
    }

    outputs.push(CommandOutput::info(format!("Retrying: {}", user_content)));

    // Send the message again with the correct user content
    let think_enabled = state.session.think;
    match super::core::send_message(
        &state.ollama,
        &state.model_config,
        &mut state.session,
        &user_content,
        state.tools_active,
        think_enabled,
        false, // cli_code: false for retry (use existing config)
        &state.settings,
        state.agents_md.as_deref(),
        state.db.as_ref(),
        state.embedding_client.as_ref(),
        state.cli_soulless,
        None,
        view,
    )
    .await
    {
        Ok(result) => {
            state.last_assistant_message_id = state
                .session
                .add_assistant_message(result.response, Some(result.metrics.prompt_tokens));

            if result.metrics.total_tokens > 0 {
                outputs.push(CommandOutput::TokenDisplay {
                    prompt_tokens: result.metrics.prompt_tokens,
                    response_tokens: result.metrics.response_tokens,
                    total_tokens: result.metrics.total_tokens,
                });
            }

            // Auto-compact if needed (after response, before next input)
            super::compaction::CompactionContext {
                ollama: &state.ollama,
                model_config: &state.model_config,
                session: &mut state.session,
                settings: &state.settings,
                agents_md: state.agents_md.as_deref(),
                context_window: result.context_window,
                view,
                llm_tx: llm_tx.clone(),
            }
            .compact_if_needed()
            .await;

            if !state.session.anonymous
                && let Err(e) = state.session.save_sqlite()
            {
                log::debug!("Warning: Could not save session: {}", e);
            }
        }
        Err(e) => {
            let error_str = e.to_string();
            outputs.push(CommandOutput::error(format_tool_error(&error_str)));
        }
    }

    outputs
}

/// Handle fact prune command
///
/// Runs the decay cycle and prunes old facts.
pub fn handle_fact_prune(state: &ReplState) -> Vec<CommandOutput> {
    use crate::facts::db::DecayStats;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot prune facts in anonymous mode.",
        )];
    }

    match db.run_decay_cycle() {
        Ok(DecayStats { pruned, remaining }) => {
            let msg = if pruned > 0 {
                format!("Pruned {} old fact(s), {} remaining.", pruned, remaining)
            } else {
                format!("No facts to prune. {} fact(s) remaining.", remaining)
            };
            vec![CommandOutput::success(msg)]
        }
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to prune facts: {}",
            e
        ))],
    }
}

/// Handle content prune command
///
/// Runs the content decay cycle and prunes low-retention content items.
/// Items with importance >= 0.8 are never pruned.
pub fn handle_content_prune(state: &ReplState) -> Vec<CommandOutput> {
    use crate::db::content_decay_ops::run_content_decay_cycle;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            log::warn!("Cannot prune content: database not initialized (anonymous mode)");
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        log::warn!("Cannot prune content in anonymous mode");
        return vec![CommandOutput::error(
            "Cannot prune content in anonymous mode.",
        )];
    }

    match db.with_connection(|conn| {
        run_content_decay_cycle(conn).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
        })
    }) {
        Ok(stats) => {
            log::debug!(
                "Content prune completed: {} pruned, {} remaining (avg retention: {:.2})",
                stats.pruned,
                stats.remaining,
                stats.avg_retention
            );
            vec![CommandOutput::ContentPruneResult(ContentPruneData {
                pruned_count: stats.pruned,
                total_count: stats.remaining + stats.pruned,
                success: true,
                error: None,
            })]
        }
        Err(e) => {
            log::warn!("Failed to prune content: {}", e);
            vec![CommandOutput::ContentPruneResult(ContentPruneData {
                pruned_count: 0,
                total_count: 0,
                success: false,
                error: Some(e.to_string()),
            })]
        }
    }
}

/// Handle `/gc` command — garbage collect database artifacts.
///
/// Identifies and removes:
/// - Empty assistant messages (artifacts from Ctrl+C cancellation)
/// - Orphan chunks (chunks whose parent item no longer exists)
/// - Orphan content/chunk/fact embeddings (vec0 rows without parent record)
pub fn handle_gc(state: &ReplState) -> Vec<CommandOutput> {
    // Check anonymous mode FIRST — the DB is never initialized in anonymous
    // mode, so this check must come before the DB None check, otherwise the
    // generic "Database not initialized" message hides the anonymous-specific
    // explanation.
    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot run garbage collection in anonymous mode.",
        )];
    }

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            log::warn!("Cannot run garbage collection: database not initialized");
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    match db.garbage_collect() {
        Ok(stats) => {
            log::debug!(
                "Garbage collection: {} empty message(s), {} orphan chunk(s), \
                 {} orphan item embedding(s), {} orphan chunk embedding(s), {} orphan fact embedding(s)",
                stats.empty_messages_removed,
                stats.orphan_chunks_removed,
                stats.orphan_item_embeddings_removed,
                stats.orphan_chunk_embeddings_removed,
                stats.orphan_fact_embeddings_removed,
            );
            vec![CommandOutput::GcResult(GcData {
                empty_messages_removed: stats.empty_messages_removed,
                orphan_chunks_removed: stats.orphan_chunks_removed,
                orphan_item_embeddings_removed: stats.orphan_item_embeddings_removed,
                orphan_chunk_embeddings_removed: stats.orphan_chunk_embeddings_removed,
                orphan_fact_embeddings_removed: stats.orphan_fact_embeddings_removed,
                success: true,
                error: None,
            })]
        }
        Err(e) => {
            log::warn!("Failed to run garbage collection: {}", e);
            vec![CommandOutput::GcResult(GcData {
                empty_messages_removed: 0,
                orphan_chunks_removed: 0,
                orphan_item_embeddings_removed: 0,
                orphan_chunk_embeddings_removed: 0,
                orphan_fact_embeddings_removed: 0,
                success: false,
                error: Some(e.to_string()),
            })]
        }
    }
}

/// Handle fact add command
///
/// Adds a new fact to the database with full 6-layer dedup:
/// Normalization (ADR-E4), Layer 1 (exact match), Layer 2 (normalized match),
/// Layer 3.5 (semantic embedding + triple disambiguation, ≥0.70),
/// Layer 3 (FTS5 BM25, ≥0.75), plus Global-wins-project rule
/// and synchronous embedding generation.
pub async fn handle_fact_add(
    state: &mut ReplState,
    content: String,
    global: bool,
) -> Vec<CommandOutput> {
    use crate::facts::classify::classify_fact;
    use crate::facts::dedup::{DedupConfig, DedupResult, deduplicate_and_insert};
    use crate::facts::lang;
    use crate::facts::types::{Category, MAX_FACT_CONTENT_SIZE, Scope};

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error("Cannot add facts in anonymous mode.")];
    }

    // Validate content length
    if content.len() > MAX_FACT_CONTENT_SIZE {
        return vec![
            CommandOutput::error(format!(
                "Fact content exceeds {} character limit.",
                MAX_FACT_CONTENT_SIZE
            )),
            CommandOutput::info(format!("  Current length: {} characters", content.len())),
            CommandOutput::info("  Use shorter content or split into multiple facts."),
        ];
    }

    // Normalize to storage format (ADR-E4: third-person storage).
    let content = lang::normalize_to_storage_format(&content);

    // Classify the fact (after normalization so category is based on canonical form)
    let category = classify_fact(&content);

    // Determine scope and project_id
    let scope = if global {
        Scope::Global
    } else {
        Scope::Project
    };
    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    // Delegate to centralized dedup pipeline
    let config = DedupConfig::user();
    let semantic_threshold = crate::settings::Settings::load().facts.semantic_threshold;
    let result = deduplicate_and_insert(
        &db,
        &content,
        category,
        scope,
        project_id.as_deref(),
        &config,
        state.embedding_client.as_ref(),
        semantic_threshold,
    )
    .await;

    // Format result for CLI
    match result {
        DedupResult::Inserted {
            id,
            category,
            scope,
        } => {
            let scope_str = if scope == Scope::Global {
                "global"
            } else {
                "project"
            };
            let category_str = match category {
                Category::Preference => "preference",
                Category::Fact => "fact",
            };
            vec![
                CommandOutput::success(format!(
                    "Added {} fact #{} (scope: {}, category: {})",
                    category_str, id, scope_str, category_str
                )),
                CommandOutput::info(format!("  {}", content)),
            ]
        }
        DedupResult::ExactDuplicate {
            existing_id,
            existing_content,
        } => {
            vec![
                CommandOutput::warning(format!(
                    "Skipped: Exact duplicate already exists (#{})",
                    existing_id
                )),
                CommandOutput::info(format!("  Existing: {}", existing_content)),
                CommandOutput::info(format!("  New: {}", content)),
                CommandOutput::info(format!(
                    "\n  Use /fact remove {} first if you want to replace it.",
                    existing_id
                )),
            ]
        }
        DedupResult::NormalizedDuplicate {
            existing_id,
            existing_content,
        } => {
            vec![
                CommandOutput::warning(format!(
                    "Skipped: Similar fact already exists (#{})",
                    existing_id
                )),
                CommandOutput::info(format!("  Existing: {}", existing_content)),
                CommandOutput::info(format!("  New: {}", content)),
                CommandOutput::info(format!(
                    "\n  Use /fact remove {} first if you want to replace it.",
                    existing_id
                )),
            ]
        }
        DedupResult::SemanticDuplicate {
            existing_id,
            existing_content,
            score,
        } => {
            log::debug!("/fact add: Semantic duplicate (cosine={:.3})", score);
            vec![
                CommandOutput::warning(format!(
                    "Skipped: Similar fact already exists (#{})",
                    existing_id
                )),
                CommandOutput::info(format!("  Existing: {}", existing_content)),
                CommandOutput::info(format!("  New: {}", content)),
                CommandOutput::info(format!(
                    "\n  Use /fact remove {} first if you want to replace it.",
                    existing_id
                )),
            ]
        }
        DedupResult::Updated {
            id,
            old_content,
            reason,
            category,
            scope,
        } => {
            let scope_str = if scope == Scope::Global {
                "global"
            } else {
                "project"
            };
            let category_str = match category {
                Category::Preference => "preference",
                Category::Fact => "fact",
            };
            vec![
                CommandOutput::info(format!(
                    "Updated: '{}' replaces '{}' ({})",
                    content, old_content, reason
                )),
                CommandOutput::info(format!(
                    "→ New fact #{} (scope: {}, category: {})",
                    id, scope_str, category_str
                )),
            ]
        }
        DedupResult::Fts5Conflict {
            existing_id,
            existing_content,
            is_contradiction: _,
        } => {
            vec![
                CommandOutput::warning(format!(
                    "Skipped: Similar fact already exists (#{})",
                    existing_id
                )),
                CommandOutput::info(format!("  Existing: {}", existing_content)),
                CommandOutput::info(format!("  New: {}", content)),
                CommandOutput::info(format!(
                    "\n  Use /fact remove {} first if you want to replace it.",
                    existing_id
                )),
            ]
        }
        DedupResult::Error(e) => {
            vec![CommandOutput::error(e)]
        }
    }
}
/// Handle fact list command
///
/// Lists all facts for the current scope.
pub fn handle_fact_list(state: &ReplState, scope: FactListScope) -> Vec<CommandOutput> {
    use crate::facts::types::Scope;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error("Cannot list facts in anonymous mode.")];
    }

    let project_id = state.session.project_id.clone();

    let scope_data = match scope {
        FactListScope::All => FactListScopeData::All,
        FactListScope::Global => FactListScopeData::Global,
        FactListScope::Project => FactListScopeData::Project,
    };

    let global_facts = db
        .list_facts(Some(Scope::Global), None, None)
        .unwrap_or_default();
    let project_facts = db
        .list_facts(Some(Scope::Project), None, project_id.as_deref())
        .unwrap_or_default();

    vec![CommandOutput::FactList(FactListData {
        global_facts,
        project_facts,
        scope: scope_data,
    })]
}

/// Handle fact remove command
///
/// Removes a fact by ID.
pub fn handle_fact_remove(state: &ReplState, id: i64) -> Vec<CommandOutput> {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot remove facts in anonymous mode.",
        )];
    }

    match db.get_fact(id) {
        Ok(Some(fact)) => match db.delete_fact(id) {
            Ok(()) => vec![CommandOutput::FactRemoved(FactRemoveResult {
                id,
                content: Some(fact.content),
                success: true,
                error: None,
            })],
            Err(e) => vec![CommandOutput::FactRemoved(FactRemoveResult {
                id,
                content: None,
                success: false,
                error: Some(e.to_string()),
            })],
        },
        Ok(None) => vec![CommandOutput::FactRemoved(FactRemoveResult {
            id,
            content: None,
            success: false,
            error: Some(format!("Fact #{} not found.", id)),
        })],
        Err(e) => vec![CommandOutput::FactRemoved(FactRemoveResult {
            id,
            content: None,
            success: false,
            error: Some(format!("Error retrieving fact: {}", e)),
        })],
    }
}

/// Handle fact search command
///
/// Searches facts using FTS5.
pub fn handle_fact_search(
    state: &ReplState,
    query: String,
    global: bool,
    limit: usize,
) -> Vec<CommandOutput> {
    use crate::facts::types::Scope;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot search facts in anonymous mode.",
        )];
    }

    let scope = if global {
        Some(Scope::Global)
    } else {
        Some(Scope::Project)
    };

    match db.search_facts(&query, scope, limit) {
        Ok(results) => {
            let search_results: Vec<FactSearchResult> = results
                .iter()
                .map(|r| FactSearchResult {
                    id: r.fact.id,
                    content: r.fact.content.clone(),
                    category: r.fact.category,
                    score: r.score as f64,
                })
                .collect();
            let total = search_results.len();
            vec![CommandOutput::FactSearchResults(FactSearchData {
                query,
                results: search_results,
                total,
            })]
        }
        Err(e) => vec![CommandOutput::error(format!("Search failed: {}", e))],
    }
}

/// Handle todo add command
///
/// Adds a new task to the todo list.
pub fn handle_todo_add(
    description: String,
    priority: Option<String>,
    tags: Option<String>,
    session: &mut super::session::ChatSession,
) -> Vec<CommandOutput> {
    use crate::chat::todo_state::Priority;
    use crate::tools::todo;

    let priority_val = priority
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Priority::Medium);

    let tags_val: Vec<String> = tags
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let id = {
        let state = todo::get_todo_state();
        #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
        let mut guard = state.lock().expect("lock poisoned: todo state");
        guard.add_with_options(description.clone(), priority_val, tags_val.clone())
    };

    session.todos = todo::save_to_session();

    let mut msg = format!(
        "Added task {}: {} [pending] [{}]",
        id, description, priority_val
    );
    if !tags_val.is_empty() {
        msg.push_str(&format!(
            " {}",
            tags_val
                .iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }

    let mut outputs = vec![CommandOutput::success(msg)];

    if !session.anonymous
        && let Err(e) = session.save_sqlite()
    {
        outputs.push(CommandOutput::warning(format!(
            "Could not save session: {}",
            e
        )));
    }

    outputs
}

/// Handle todo list command
///
/// Lists all tasks in the todo list, optionally filtered.
pub fn handle_todo_list(filter: Option<String>) -> Vec<CommandOutput> {
    use crate::chat::todo_state::{Priority, TaskFilter, TaskStatus};
    use crate::tools::todo;

    let filter_val = filter.filter(|s| !s.is_empty());

    let task_filter = if let Some(ref f) = filter_val {
        if let Some(tag) = f.strip_prefix('#') {
            TaskFilter {
                tag: Some(tag.to_lowercase()),
                ..Default::default()
            }
        } else if let Ok(status) = f.parse::<TaskStatus>() {
            TaskFilter {
                status: Some(status),
                ..Default::default()
            }
        } else if let Ok(priority) = f.parse::<Priority>() {
            TaskFilter {
                priority: Some(priority),
                ..Default::default()
            }
        } else {
            TaskFilter {
                tag: Some(f.to_lowercase()),
                ..Default::default()
            }
        }
    } else {
        TaskFilter::default()
    };

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let guard = state.lock().expect("lock poisoned: todo state");
    let formatted = guard.format_list_filtered(&task_filter);
    let count = guard.count();

    vec![CommandOutput::TodoList(TodoListData {
        formatted_list: formatted,
        count,
    })]
}

/// Handle todo get command
///
/// Gets a single task by ID.
pub fn handle_todo_get(id: usize) -> Vec<CommandOutput> {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let guard = state.lock().expect("lock poisoned: todo state");

    match guard.get(id) {
        Some(task) => {
            let mut output = format!(
                "Task {}: {}\n  Status: {}\n  Priority: {}",
                task.id, task.description, task.status, task.priority
            );
            if !task.tags.is_empty() {
                output.push_str(&format!(
                    "\n  Tags: {}",
                    task.tags
                        .iter()
                        .map(|t| format!("#{}", t))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            vec![CommandOutput::info(output)]
        }
        None => vec![CommandOutput::error(format!("Task {} not found", id))],
    }
}

/// Handle todo edit command
///
/// Edits a task's description, priority, and/or tags.
pub fn handle_todo_edit(
    id: usize,
    description: Option<String>,
    priority: Option<String>,
    tags: Option<String>,
    session: &mut super::session::ChatSession,
) -> Vec<CommandOutput> {
    use crate::chat::todo_state::Priority;
    use crate::tools::todo;

    // Normalize empty strings to None
    let description = description.filter(|s| !s.is_empty());
    let priority = priority.filter(|s| !s.is_empty());
    let tags = tags.filter(|s| !s.is_empty());

    if description.is_none() && priority.is_none() && tags.is_none() {
        return vec![CommandOutput::error(
            "Provide at least one field to update (description, priority, or tags).",
        )];
    }

    let priority_val: Option<Priority> = priority.and_then(|s| s.parse().ok());
    let tags_val: Option<Vec<String>> = tags.map(|s| {
        s.split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect()
    });

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");

    match guard.edit(id, description, priority_val, tags_val) {
        Ok(()) => {
            #[expect(clippy::expect_used)] // task just edited successfully, guaranteed to exist
            let task = guard.get(id).expect("task just edited successfully");
            let mut msg = format!("Task {} updated:", id);
            msg.push_str(&format!("\n  Description: {}", task.description));
            msg.push_str(&format!("\n  Status: {}", task.status));
            msg.push_str(&format!("\n  Priority: {}", task.priority));
            if !task.tags.is_empty() {
                msg.push_str(&format!(
                    "\n  Tags: {}",
                    task.tags
                        .iter()
                        .map(|t| format!("#{}", t))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            drop(guard);
            session.todos = todo::save_to_session();

            let mut outputs = vec![CommandOutput::success(msg)];
            if !session.anonymous
                && let Err(e) = session.save_sqlite()
            {
                outputs.push(CommandOutput::warning(format!(
                    "Could not save session: {}",
                    e
                )));
            }
            outputs
        }
        Err(e) => vec![CommandOutput::error(e.to_string())],
    }
}

/// Handle todo delete command
///
/// Deletes a specific task by ID.
pub fn handle_todo_delete(
    id: usize,
    session: &mut super::session::ChatSession,
) -> Vec<CommandOutput> {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");

    let task_desc = guard.get(id).map(|t| t.description.clone());

    match guard.delete(id) {
        Ok(()) => {
            let msg = if let Some(desc) = task_desc {
                format!("Deleted task {}: {}", id, desc)
            } else {
                format!("Deleted task {}", id)
            };
            drop(guard);
            session.todos = todo::save_to_session();

            let mut outputs = vec![CommandOutput::success(msg)];
            if !session.anonymous
                && let Err(e) = session.save_sqlite()
            {
                outputs.push(CommandOutput::warning(format!(
                    "Could not save session: {}",
                    e
                )));
            }
            outputs
        }
        Err(e) => vec![CommandOutput::error(e.to_string())],
    }
}

/// Handle todo update command
///
/// Updates the status of a task.
pub fn handle_todo_update(
    id: usize,
    status: String,
    session: &mut super::session::ChatSession,
) -> Vec<CommandOutput> {
    use crate::chat::todo_state::TaskStatus;
    use crate::tools::todo;

    let new_status: TaskStatus = match status.parse() {
        Ok(s) => s,
        Err(e) => {
            return vec![CommandOutput::error(e.to_string())];
        }
    };

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");

    match guard.update_status(id, new_status) {
        Ok(()) => {
            let msg = format!("Task {} marked as {}", id, new_status);
            drop(guard);
            session.todos = todo::save_to_session();

            let mut outputs = vec![CommandOutput::success(msg)];
            if !session.anonymous
                && let Err(e) = session.save_sqlite()
            {
                outputs.push(CommandOutput::warning(format!(
                    "Could not save session: {}",
                    e
                )));
            }
            outputs
        }
        Err(e) => vec![CommandOutput::error(e.to_string())],
    }
}

/// Handle todo clear-done command
///
/// Clears all completed tasks from the list.
pub fn handle_todo_clear_done(session: &mut super::session::ChatSession) -> Vec<CommandOutput> {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");
    let removed = guard.clear_done();

    let msg = if removed == 0 {
        "No completed tasks to remove.".to_string()
    } else if removed == 1 {
        "Removed 1 completed task.".to_string()
    } else {
        format!("Removed {} completed tasks.", removed)
    };

    drop(guard);
    session.todos = todo::save_to_session();

    let mut outputs = vec![CommandOutput::info(msg)];
    if !session.anonymous
        && let Err(e) = session.save_sqlite()
    {
        outputs.push(CommandOutput::warning(format!(
            "Could not save session: {}",
            e
        )));
    }
    outputs
}

/// Handle todo clear-all command
///
/// Clears all tasks from the list.
pub fn handle_todo_clear_all(session: &mut super::session::ChatSession) -> Vec<CommandOutput> {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");
    let count = guard.clear_all();

    let msg = if count == 0 {
        "The task list was already empty.".to_string()
    } else if count == 1 {
        "Cleared 1 task from the list.".to_string()
    } else {
        format!("Cleared {} tasks from the list.", count)
    };

    drop(guard);
    session.todos = todo::save_to_session();

    let mut outputs = vec![CommandOutput::info(msg)];
    if !session.anonymous
        && let Err(e) = session.save_sqlite()
    {
        outputs.push(CommandOutput::warning(format!(
            "Could not save session: {}",
            e
        )));
    }
    outputs
}

/// Handle model switch command.
///
/// Uses the centralized `model_switch::switch_model` function to switch
/// to a new model and updates the REPL state accordingly.
pub async fn handle_model_switch(
    state: &mut ReplState,
    model_name: &str,
    current_capabilities: &ModelCapabilities,
) -> Vec<CommandOutput> {
    use super::model_switch::switch_model;

    let result = match switch_model(
        model_name,
        &state.ollama,
        current_capabilities,
        state.session.think,
        state.tools_active,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return vec![CommandOutput::error(e)];
        }
    };

    state.current_model_name = result.model_name.clone();
    state.session.set_model(result.model_name.clone());
    state.model_config = result.model_config;
    state.capabilities = result.capabilities.clone();
    state.session.think = result.think_active;
    state.tools_active = result.tools_active;

    let mut outputs = Vec::new();
    for warning in result.warnings {
        outputs.push(CommandOutput::warning(warning));
    }
    outputs.push(CommandOutput::info(format!(
        "Switched to model: {}",
        state.model_config.model_id
    )));
    outputs
}

/// Format context information as a string for `CommandOutput::ContextInfo`.
///
/// Shows token usage, message count, and context window utilization.
pub fn format_context_info(
    session: &ChatSession,
    model_config: &ModelConfig,
    tools_enabled: bool,
    agents_md: Option<&str>,
    settings: &Settings,
    soulless: bool,
    db: Option<&Arc<crate::db::Database>>,
) -> String {
    use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
    use crate::tools::get_available_tool_names;

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

    let tools_tokens = if tools_enabled && tool_count > 0 {
        tool_count * TOKENS_PER_TOOL
    } else {
        0
    };

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
    let usage_percent = metrics.utilization * 100.0;

    let bar_width = 20;
    let filled = (usage_percent.min(100.0) as usize * bar_width) / 100;
    let empty = bar_width - filled;

    // Calculate thresholds based on percentage of context window
    let remaining = context_window.saturating_sub(metrics.total_tokens);
    let (pre_tool, compaction, _, _) =
        crate::context_overflow::calculate_thresholds(context_window);

    let status_text = if remaining > pre_tool {
        "OK"
    } else if remaining > compaction {
        "MODERATE"
    } else {
        "CRITICAL"
    };

    let mut output = String::new();
    output.push('\n');
    output.push_str("Context Information:\n");
    output.push_str(&format!(
        "  Model:          {} ({}K context)\n",
        model_config.model_id, context_window_k
    ));
    output.push('\n');
    output.push_str("  Context Utilization:\n");
    output.push_str(&format!(
        "    {}{}{} {:.1}%\n",
        "█".repeat(filled),
        "░".repeat(empty),
        status_text,
        usage_percent
    ));
    output.push_str(&format!(
        "    {} / {} tokens\n",
        metrics.total_tokens, context_window
    ));
    output.push('\n');
    output.push_str(&format!("  Status: {}\n", status_text));
    output.push('\n');
    output.push_str("  Token Breakdown:\n");
    output.push_str(&format!(
        "    System prompt:    ~{} tokens\n",
        metrics.system_tokens
    ));
    if tools_enabled && tool_count > 0 {
        output.push_str(&format!(
            "    Tool definitions: ~{} tokens ({} tools)\n",
            metrics.tools_tokens, tool_count
        ));
    }

    let active_messages = if session.has_compacted_messages() {
        session.messages.len() - session.messages_sent_to_llm
    } else {
        session.messages.len()
    };

    if metrics.total_tokens > 0 {
        output.push_str(&format!(
            "    History:          ~{} tokens\n",
            metrics.history_tokens
        ));
        if session.has_compacted_messages() {
            output.push_str(&format!(
                "                      ({} active messages + summary)\n",
                active_messages
            ));
        } else {
            output.push_str(&format!(
                "                      ({} messages)\n",
                active_messages
            ));
        }
    } else if session.has_compacted_messages() {
        output.push_str(&format!(
            "    Summary:          ~{} tokens\n",
            estimate_tokens(session.compacted_summary.as_deref().unwrap_or("")) + 4
        ));
        output.push_str(&format!(
            "    Conversation:     ~{} tokens ({} active messages)\n",
            metrics.history_tokens, active_messages
        ));
    } else {
        output.push_str(&format!(
            "    Conversation:     ~{} tokens ({} messages)\n",
            metrics.history_tokens, active_messages
        ));
    }

    output.push_str(&format!("    {}\n", "─".repeat(40)));
    output.push_str(&format!(
        "    Total used:       ~{} tokens\n",
        metrics.total_tokens
    ));
    output.push_str(&format!(
        "    Available:        ~{} tokens\n",
        metrics.available()
    ));
    output.push('\n');

    if session.has_compacted_messages() {
        output.push_str("  Session:\n");
        output.push_str(&format!(
            "    Compacted:        {} messages summarized\n",
            session.compacted_message_count()
        ));
        output.push_str(&format!(
            "    Active:           {} messages\n",
            active_messages
        ));
        output.push_str(&format!(
            "    Total:            {} messages\n",
            session.messages.len()
        ));
    } else {
        output.push_str("  Session:\n");
        output.push_str(&format!(
            "    Total:            {} messages\n",
            session.messages.len()
        ));
    }

    // Content Memory section (if database is available)
    if let Some(db_ref) = db {
        use crate::db::content_decay_ops::get_content_decay_stats;

        match db_ref.with_connection(|conn| {
            get_content_decay_stats(conn).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
            })
        }) {
            Ok(stats) => {
                output.push_str("  Content Memory:\n");
                output.push_str(&format!("    Total items:      {}\n", stats.total_items));
                output.push_str(&format!(
                    "    Avg importance:    {:.2}\n",
                    stats.avg_importance
                ));
                if stats.items_at_risk > 0 {
                    output.push_str(&format!(
                        "    ⚠ Items at risk:   {} (low decay score)\n",
                        stats.items_at_risk
                    ));
                }
                output.push_str(&format!(
                    "    Feedback signals:  {}\n",
                    stats.total_feedback_signals
                ));
            }
            Err(_) => {
                // Silently skip — don't error out /context if stats fail
            }
        }
        output.push('\n');
    }

    output.push_str("  Tip: Use /content prune to prune low-retention content.\n");
    output.push('\n');
    output
}

/// Handle note add command
///
/// Adds a new note with the given content.
/// Generates embedding asynchronously for semantic search.
pub fn handle_note_add(
    state: &ReplState,
    content: String,
    title: Option<String>,
    global: bool,
) -> Vec<CommandOutput> {
    use crate::content::{ContentScope, ContentSource, MAX_NOTE_CONTENT_SIZE, Note};

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error("Cannot add notes in anonymous mode.")];
    }

    if content.len() > MAX_NOTE_CONTENT_SIZE {
        return vec![
            CommandOutput::error(format!(
                "Note content exceeds {} character limit.",
                MAX_NOTE_CONTENT_SIZE
            )),
            CommandOutput::info(format!("  Current length: {} characters", content.len())),
            CommandOutput::info("  Use shorter content or split into multiple notes."),
        ];
    }

    let scope = if global {
        ContentScope::Global
    } else {
        ContentScope::Project
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    let note = match Note::new(
        content.clone(),
        scope,
        project_id.clone(),
        ContentSource::User,
        title.clone(),
    ) {
        Ok(n) => n,
        Err(e) => {
            return vec![CommandOutput::error(format!(
                "Failed to create note: {}",
                e
            ))];
        }
    };

    match db.insert_note(&note) {
        Ok(id) => {
            let scope_str = if global { "global" } else { "project" };
            let msg = if let Some(t) = &title {
                format!("Added note #{} (scope: {}): {}", id, scope_str, t)
            } else {
                format!("Added note #{} (scope: {})", id, scope_str)
            };

            // Generate embedding asynchronously
            if let Some(ref embedding_client) = state.embedding_client {
                let client = Arc::clone(embedding_client);
                let db_clone = Arc::clone(&db);
                let pid = project_id.clone();
                let note_content = note.content.clone();

                tokio::spawn(async move {
                    let ctx =
                        EmbedItemContext::new(&note_content, id, "note", None, pid.as_deref());
                    if let Err(e) =
                        embed_item_with_fallback(ctx, &db_clone, &client, DEFAULT_CONTEXT_LENGTH)
                            .await
                    {
                        log::warn!("Failed to generate embedding for note: {}", e);
                    }
                });
            }

            vec![CommandOutput::NoteAdded(NoteAddResult {
                success: true,
                message: msg,
            })]
        }
        Err(e) => {
            vec![CommandOutput::NoteAdded(NoteAddResult {
                success: false,
                message: format!("Failed to store note: {}", e),
            })]
        }
    }
}

/// Handle note list command
///
/// Lists notes for the current scope with pagination (8 per page).
pub fn handle_note_list(
    state: &ReplState,
    global: bool,
    page: Option<usize>,
) -> Vec<CommandOutput> {
    use crate::content::ContentScope;

    const NOTES_PER_PAGE: usize = 8;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error("Cannot list notes in anonymous mode.")];
    }

    let scope = if global {
        Some(ContentScope::Global)
    } else {
        Some(ContentScope::Project)
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.list_notes(scope, project_id.as_deref()) {
        Ok(notes) => {
            let total_notes = notes.len();
            let total_pages = if total_notes == 0 {
                1
            } else {
                total_notes.div_ceil(NOTES_PER_PAGE)
            };

            // Validate page number
            let requested_page = page.unwrap_or(1);
            if requested_page < 1 {
                return vec![CommandOutput::error(
                    "Page must be >= 1. Use /note list 1 for first page.",
                )];
            }
            if requested_page > total_pages {
                return vec![CommandOutput::error(format!(
                    "Page {} does not exist. Total pages: {}. Use /note list {}.",
                    requested_page, total_pages, total_pages
                ))];
            }

            vec![CommandOutput::NoteList(NoteListData {
                notes,
                page: requested_page,
                total_pages,
                total_notes,
            })]
        }
        Err(e) => vec![CommandOutput::error(format!("Failed to list notes: {}", e))],
    }
}

/// Handle note show command
///
/// Shows a single note by ID.
pub fn handle_note_show(state: &ReplState, id: i64) -> Vec<CommandOutput> {
    use crate::content::{ContentScope, ContentSource};
    use chrono::Utc;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error("Cannot show notes in anonymous mode.")];
    }

    match db.get_note(id) {
        Ok(Some(note)) => {
            let scope_str = match note.scope {
                ContentScope::Global => "global",
                ContentScope::Project => "project",
            };
            let source_str = match note.source {
                ContentSource::User => "user",
                ContentSource::Llm => "llm",
            };
            let age_days = (Utc::now() - note.created_at).num_days();

            // Build header as plain text (view layer applies styling)
            let mut header = format!("## Note #{}\n\n", note.id);
            if let Some(t) = &note.title {
                header.push_str(&format!("**Title:** {}\n\n", t));
            }
            header.push_str(&format!(
                "**Scope:** {} | **Source:** {} | **Age:** {}d\n\n",
                scope_str, source_str, age_days
            ));
            if let Some(pid) = &note.project_id {
                header.push_str(&format!("**Project:** {}\n\n", pid));
            }
            header.push_str("---\n");

            // Return header and content as markdown for view layer to render
            let mut full_content = header;
            full_content.push_str(&note.content);
            vec![CommandOutput::MarkdownContent(full_content)]
        }
        Ok(None) => vec![CommandOutput::error(format!("Note #{} not found.", id))],
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to retrieve note: {}",
            e
        ))],
    }
}

/// Handle note edit command
///
/// Edits a note's title and/or content.
pub fn handle_note_edit(
    state: &ReplState,
    id: i64,
    title: Option<String>,
    content: Option<String>,
) -> Vec<CommandOutput> {
    use crate::content::MAX_NOTE_CONTENT_SIZE;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error("Cannot edit notes in anonymous mode.")];
    }

    if let Some(ref c) = content
        && c.len() > MAX_NOTE_CONTENT_SIZE
    {
        return vec![
            CommandOutput::error(format!(
                "Note content exceeds {} character limit.",
                MAX_NOTE_CONTENT_SIZE
            )),
            CommandOutput::info(format!("  Current length: {} characters", c.len())),
        ];
    }

    match db.get_note(id) {
        Ok(Some(_)) => match db.update_note(id, title.as_deref(), content.as_deref()) {
            Ok(()) => {
                let mut msg = format!("Updated note #{}", id);
                if let Some(t) = &title {
                    msg.push_str(&format!("\n  Title: {}", t));
                }
                if let Some(c) = &content {
                    msg.push_str(&format!(
                        "\n  Content: {}",
                        crate::chat::view::truncate_str(c, 80)
                    ));
                }
                vec![CommandOutput::success(msg)]
            }
            Err(e) => vec![CommandOutput::error(format!(
                "Failed to update note: {}",
                e
            ))],
        },
        Ok(None) => vec![CommandOutput::error(format!("Note #{} not found.", id))],
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to retrieve note: {}",
            e
        ))],
    }
}

/// Handle note delete command
///
/// Deletes a note by ID.
pub fn handle_note_delete(state: &ReplState, id: i64) -> Vec<CommandOutput> {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot delete notes in anonymous mode.",
        )];
    }

    match db.get_note(id) {
        Ok(Some(note)) => match db.delete_note(id) {
            Ok(()) => {
                let msg = if let Some(t) = &note.title {
                    format!("Deleted note #{}: {}", id, t)
                } else {
                    format!("Deleted note #{}", id)
                };
                vec![CommandOutput::success(msg)]
            }
            Err(e) => vec![CommandOutput::error(format!(
                "Failed to delete note: {}",
                e
            ))],
        },
        Ok(None) => vec![CommandOutput::error(format!("Note #{} not found.", id))],
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to retrieve note: {}",
            e
        ))],
    }
}

/// Handle note search command
///
/// Searches notes by keyword.
pub fn handle_note_search(
    state: &ReplState,
    query: String,
    global: bool,
    limit: usize,
) -> Vec<CommandOutput> {
    use crate::content::ContentScope;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot search notes in anonymous mode.",
        )];
    }

    let scope = if global {
        Some(ContentScope::Global)
    } else {
        Some(ContentScope::Project)
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.search_notes_keyword(&query, scope, project_id.as_deref(), limit) {
        Ok(results) => {
            let scope_str = if global { "global" } else { "project" };
            if results.is_empty() {
                return vec![CommandOutput::info(format!(
                    "No notes found for '{}' ({}).",
                    query, scope_str
                ))];
            }

            // TODO(issue: memory-architecture): Phase 3.5 — migrate to structured NoteSearchData
            // For now, format as text output
            let mut output = format!("Search results for \"{}\" ({}):\n", query, scope_str);
            for result in &results {
                if let Some(t) = &result.item.title {
                    output.push_str(&format!(
                        "  #{} {} (score: {:.2})\n",
                        result.item.id, t, result.score
                    ));
                } else {
                    output.push_str(&format!(
                        "  #{} (score: {:.2})\n",
                        result.item.id, result.score
                    ));
                }
                let preview = crate::chat::view::truncate_str(&result.item.content, 80);
                output.push_str(&format!("    {}\n", preview));
            }
            output.push_str(&format!("\n  Found: {} note(s)", results.len()));
            vec![CommandOutput::info(output)]
        }
        Err(e) => vec![CommandOutput::error(format!("Search failed: {}", e))],
    }
}

// ============================================================
// Document Command Handlers
// ============================================================

/// Handle document import command
#[cfg(feature = "document-tools")]
pub fn handle_document_import(
    state: &ReplState,
    path: String,
    global: bool,
    nowait: bool,
) -> Vec<CommandOutput> {
    use crate::content::{ContentScope, Document, MAX_DOCUMENT_SIZE, detect_file_type};
    use crate::utils::expand_tilde_path;
    use std::fs;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot import documents in anonymous mode.",
        )];
    }

    let file_path = expand_tilde_path(&path);
    if !file_path.exists() {
        return vec![CommandOutput::error(format!("File not found: {}", path))];
    }

    let metadata = match fs::metadata(&file_path) {
        Ok(m) => m,
        Err(e) => {
            return vec![CommandOutput::error(format!(
                "Cannot read file metadata: {}",
                e
            ))];
        }
    };

    if metadata.len() > MAX_DOCUMENT_SIZE as u64 {
        return vec![
            CommandOutput::error(format!(
                "File exceeds maximum size of {} bytes (got {} bytes).",
                MAX_DOCUMENT_SIZE,
                metadata.len()
            )),
            CommandOutput::info("  Consider splitting the document into smaller files."),
        ];
    }

    let file_type = match detect_file_type(&file_path) {
        Ok(ft) => ft,
        Err(e) => {
            return vec![CommandOutput::error(e.to_string())];
        }
    };

    // detect_file_type will reject PDF/EPUB with helpful error message,
    // so only TXT/MD/ORG reach here
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            return vec![CommandOutput::error(format!("Cannot read file: {}", e))];
        }
    };

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();

    let title = Document::extract_title(&content, &filename);

    let scope = if global {
        ContentScope::Global
    } else {
        ContentScope::Project
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    let document = match Document::new(
        content.clone(),
        title.clone(),
        filename.clone(),
        file_type,
        scope,
        project_id.clone(),
    ) {
        Ok(d) => d,
        Err(e) => {
            return vec![CommandOutput::error(format!(
                "Failed to create document: {}",
                e
            ))];
        }
    };

    match db.insert_document(&document) {
        Ok(id) => {
            let scope_str = if global { "global" } else { "project" };
            let mut outputs = vec![CommandOutput::success(format!(
                "Imported document #{} (scope: {}): {}",
                id, scope_str, title
            ))];
            outputs.push(CommandOutput::info(format!("  File: {}", filename)));
            outputs.push(CommandOutput::info(format!(
                "  Words: {}",
                document.word_count
            )));
            outputs.push(CommandOutput::info(format!(
                "  Type: {}",
                file_type.extension()
            )));

            if let Some(ref embedding_client) = state.embedding_client {
                if nowait {
                    // Async embedding in background
                    outputs.push(CommandOutput::info(
                        "  Indexing in background...".to_string(),
                    ));
                    let client = Arc::clone(embedding_client);
                    let db_clone = Arc::clone(&db);
                    let pid = project_id.clone();
                    let doc_content = document.content.clone();

                    tokio::spawn(async move {
                        let ctx = EmbedItemContext::new(
                            &doc_content,
                            id,
                            "document",
                            None,
                            pid.as_deref(),
                        );
                        if let Err(e) = embed_item_with_fallback(
                            ctx,
                            &db_clone,
                            &client,
                            DEFAULT_CONTEXT_LENGTH,
                        )
                        .await
                        {
                            log::warn!("Failed to generate embedding for document: {}", e);
                        }
                    });
                } else {
                    // Synchronous embedding with progress
                    outputs.push(CommandOutput::info("  Indexing document...".to_string()));

                    let ctx = EmbedItemContext::new(
                        &document.content,
                        id,
                        "document",
                        None,
                        project_id.as_deref(),
                    );

                    match tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            embed_item_with_fallback(
                                ctx,
                                &db,
                                embedding_client,
                                DEFAULT_CONTEXT_LENGTH,
                            )
                            .await
                        })
                    }) {
                        Ok(result) => {
                            let chunks = result.chunks_created.max(1);
                            outputs.push(CommandOutput::success(format!(
                                "  Document indexed ({} chunk{})",
                                chunks,
                                if chunks > 1 { "s" } else { "" }
                            )));
                        }
                        Err(e) => {
                            outputs.push(CommandOutput::warning(format!(
                                "  Failed to index document: {}",
                                e
                            )));
                            outputs.push(CommandOutput::info(
                                "  Run '/reindex' to regenerate embeddings.".to_string(),
                            ));
                        }
                    }
                }
            }
            outputs
        }
        Err(e) => {
            vec![CommandOutput::error(format!(
                "Failed to store document: {}",
                e
            ))]
        }
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_import(
    _state: &ReplState,
    _path: String,
    _global: bool,
    _nowait: bool,
) -> Vec<CommandOutput> {
    vec![
        CommandOutput::error("Document import requires 'document-tools' feature."),
        CommandOutput::info("  Recompile with: cargo build --features document-tools"),
    ]
}

/// Handle document list command
#[cfg(feature = "document-tools")]
pub fn handle_document_list(state: &ReplState, global: bool) -> Vec<CommandOutput> {
    use crate::content::ContentScope;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot list documents in anonymous mode.",
        )];
    }

    let scope = if global {
        Some(ContentScope::Global)
    } else {
        Some(ContentScope::Project)
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.list_documents(scope, project_id.as_deref()) {
        Ok(documents) => {
            let entries: Vec<DocumentEntry> = documents
                .iter()
                .map(|doc| DocumentEntry {
                    title: doc.title.clone(),
                    id: doc.id,
                    source_type: doc.file_type.extension().to_string(),
                    word_count: doc.word_count,
                    created_at: doc.created_at,
                })
                .collect();
            let is_empty = entries.is_empty();
            vec![CommandOutput::DocumentList(DocumentListData {
                documents: entries,
                is_empty,
            })]
        }
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to list documents: {}",
            e
        ))],
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_list(_state: &ReplState, _global: bool) -> Vec<CommandOutput> {
    vec![
        CommandOutput::error("Document listing requires 'document-tools' feature."),
        CommandOutput::info("  Recompile with: cargo build --features document-tools"),
    ]
}

/// Handle document show command
#[cfg(feature = "document-tools")]
pub fn handle_document_show(state: &ReplState, id: i64) -> Vec<CommandOutput> {
    use chrono::Utc;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot show document in anonymous mode.",
        )];
    }

    match db.get_document(id) {
        Ok(Some(doc)) => {
            let age_days = (Utc::now() - doc.created_at).num_days();
            let scope_str = match doc.scope {
                crate::content::ContentScope::Global => "global".to_string(),
                crate::content::ContentScope::Project => {
                    doc.project_id.as_deref().unwrap_or("project").to_string()
                }
            };

            // Build header as plain text (view layer applies styling)
            let mut header = format!("## Document #{}\n\n", doc.id);
            header.push_str(&format!("**Title:** {}\n\n", doc.title));
            header.push_str(&format!(
                "**File:** {} | **Type:** {} | **Words:** {} | **Age:** {}d | **Scope:** {}\n\n",
                doc.filename,
                doc.file_type.extension(),
                doc.word_count,
                age_days,
                scope_str
            ));
            header.push_str("---\n");

            // Return header and content as markdown for view layer to render
            let mut full_content = header;
            full_content.push_str(&doc.content);
            vec![CommandOutput::MarkdownContent(full_content)]
        }
        Ok(None) => vec![CommandOutput::error(format!("Document #{} not found.", id))],
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to retrieve document: {}",
            e
        ))],
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_show(_state: &ReplState, _id: i64) -> Vec<CommandOutput> {
    vec![
        CommandOutput::error("Document viewing requires 'document-tools' feature."),
        CommandOutput::info("  Recompile with: cargo build --features document-tools"),
    ]
}

/// Handle document delete command
#[cfg(feature = "document-tools")]
pub fn handle_document_delete(state: &ReplState, id: i64) -> Vec<CommandOutput> {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    if state.session.anonymous {
        return vec![CommandOutput::error(
            "Cannot delete document in anonymous mode.",
        )];
    }

    match db.get_document(id) {
        Ok(Some(doc)) => match db.delete_document(id) {
            Ok(()) => vec![CommandOutput::success(format!(
                "Deleted document #{}: {}",
                id, doc.title
            ))],
            Err(e) => vec![CommandOutput::error(format!(
                "Failed to delete document: {}",
                e
            ))],
        },
        Ok(None) => vec![CommandOutput::error(format!("Document #{} not found.", id))],
        Err(e) => vec![CommandOutput::error(format!(
            "Failed to retrieve document: {}",
            e
        ))],
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_delete(_state: &ReplState, _id: i64) -> Vec<CommandOutput> {
    vec![
        CommandOutput::error("Document deletion requires 'document-tools' feature."),
        CommandOutput::info("  Recompile with: cargo build --features document-tools"),
    ]
}

/// Handle skill activation command
///
/// Activates a skill for the current session by setting it in the session state.
/// The skill content will be injected into the system prompt.
pub fn handle_skill_activated(
    state: &mut ReplState,
    name: String,
    content: String,
) -> Vec<CommandOutput> {
    // Store the active skill in session
    state.session.active_skill = Some(super::session::ActiveSkill {
        name: name.clone(),
        content,
    });

    vec![
        CommandOutput::success(format!("Skill '{}' activated for this session.", name)),
        CommandOutput::info(
            "Skill instructions will be followed when relevant to the conversation.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::session::ChatSession;
    use super::*;
    use crate::capabilities::ModelCapabilities;
    use crate::config::ModelConfig;
    use crate::settings::Settings;
    use ollama_rs::Ollama;

    fn create_test_state() -> ReplState {
        let session = ChatSession::new("test-model".to_string(), None, false);
        let model_config = ModelConfig::get_default();
        let capabilities = ModelCapabilities::default();
        let ollama = Ollama::new("http://localhost".to_string(), 11434);
        let settings = Settings::default();

        ReplState {
            session,
            current_model_name: "test-model".to_string(),
            model_config,
            capabilities,
            tools_active: false,
            agents_md: None,
            cli_code: false,
            cli_soulless: false,
            ollama,
            db: None,
            embedding_client: None,
            settings,
            last_assistant_message_id: None,
        }
    }

    #[test]
    fn test_handle_think_toggled_unsupported() {
        let mut state = create_test_state();
        state.capabilities.thinking = false;

        handle_think_toggled(&mut state, true);

        assert!(!state.session.think);
    }

    #[test]
    fn test_handle_tools_toggled_unsupported() {
        let mut state = create_test_state();
        state.capabilities.tools = false;

        handle_tools_toggled(&mut state, true);

        assert!(!state.session.tools);
        assert!(!state.tools_active);
    }

    #[test]
    fn test_handle_tools_toggled_supported() {
        let mut state = create_test_state();
        state.capabilities.tools = true;

        handle_tools_toggled(&mut state, true);

        assert!(state.tools_active);
    }

    #[test]
    fn test_handle_tools_toggled_disables_when_false() {
        let mut state = create_test_state();
        state.capabilities.tools = true;
        state.session.tools = true;
        state.tools_active = true;

        handle_tools_toggled(&mut state, false);

        assert!(!state.tools_active);
    }

    #[test]
    fn test_handle_think_toggled_enabled() {
        let mut state = create_test_state();
        state.capabilities.thinking = true;

        handle_think_toggled(&mut state, true);

        // The handler prints "Think mode: enabled" but doesn't change tools_active
        // It's a simple toggle that just validates capability support
    }

    #[test]
    fn test_handle_retrieval_toggled_enabled() {
        let state = create_test_state();

        // Should not panic when retrieval is enabled
        handle_retrieval_toggled(&state, true);
    }

    #[test]
    fn test_handle_retrieval_toggled_disabled() {
        let state = create_test_state();

        // Should not panic when retrieval is disabled
        handle_retrieval_toggled(&state, false);
    }

    #[test]
    fn test_handle_tool_output_changed() {
        use super::super::session::ToolOutputLevel;

        // Just verifying it doesn't panic
        handle_tool_output_changed(ToolOutputLevel::Compact);
        handle_tool_output_changed(ToolOutputLevel::Full);
        handle_tool_output_changed(ToolOutputLevel::Hidden);
    }

    #[test]
    fn test_handle_undo_empty_session() {
        let mut state = create_test_state();
        state.session.messages.clear();

        handle_undo(&mut state);

        // Should print "No messages to remove" and not panic
    }
}

/// Handle /ocr command - extract text from an image
pub async fn handle_subagent_ocr(
    state: &mut ReplState,
    path: String,
    mode: Option<String>,
) -> Vec<CommandOutput> {
    use crate::chat::subagent::{SubagentConfig, SubagentRunner};
    use crate::ocr::mode::{OcrMode, parse_ocr_mode};
    use crate::utils::expand_tilde_path;

    // Parse the optional OCR mode
    let mode = match parse_ocr_mode(mode) {
        Ok(m) => m,
        Err(e) => {
            return vec![CommandOutput::error(e.to_string())];
        }
    };

    // Expand tilde in path (e.g., ~/photo.jpg → /home/user/photo.jpg)
    let file_path = expand_tilde_path(&path);

    // Validate path for security (sandbox + blocklist)
    if let Err(e) = crate::security::validate_subagent_path(&file_path) {
        return vec![CommandOutput::error(format!("Error: {}", e))];
    }

    // Save user command to conversation context
    let cmd_str = match mode {
        OcrMode::Text => format!("/ocr {}", path),
        _ => format!("/ocr {} {:?}", path, mode).to_lowercase(),
    };
    state.session.add_user_message(cmd_str);

    let (model, _, _) = state.settings.get_subcommand_config("ocr");
    let config = SubagentConfig::new(model, "OCR extraction").with_ocr_mode(mode);
    let runner = SubagentRunner::new(state.ollama.clone(), config);

    match runner.run_ocr(&file_path, mode).await {
        Ok(result) => {
            let _ = state.session.add_assistant_message(result.clone(), None);
            vec![CommandOutput::info(result)]
        }
        Err(e) => vec![CommandOutput::error(format!("Error: {}", e))],
    }
}

/// Handle /vision command - analyze image(s) with vision model
pub async fn handle_subagent_vision(
    state: &mut ReplState,
    paths: Vec<String>,
    prompt: Option<String>,
) -> Vec<CommandOutput> {
    use crate::chat::subagent::{SubagentConfig, SubagentRunner};
    use crate::utils::expand_tilde_path;
    use std::path::PathBuf;

    // Expand tilde in paths
    let path_bufs: Vec<PathBuf> = paths.iter().map(|p| expand_tilde_path(p)).collect();

    // Validate all paths for security (sandbox + blocklist)
    for path in &path_bufs {
        if let Err(e) = crate::security::validate_subagent_path(path) {
            return vec![CommandOutput::error(format!("Error: {}", e))];
        }
    }

    // Build command string for context
    let cmd_str = match &prompt {
        Some(p) => format!("/vision {} {}", paths.join(" "), p),
        None => format!("/vision {}", paths.join(" ")),
    };
    state.session.add_user_message(cmd_str);

    let (model, _, _) = state.settings.get_subcommand_config("vision");
    let config = SubagentConfig::new(model, "Vision analysis");
    let runner = SubagentRunner::new(state.ollama.clone(), config);

    let prompt_str = prompt
        .as_deref()
        .unwrap_or("Describe what you see in this image.");

    match runner.run_vision(&path_bufs, prompt_str).await {
        Ok(result) => {
            let _ = state.session.add_assistant_message(result.clone(), None);
            vec![CommandOutput::info(result)]
        }
        Err(e) => vec![CommandOutput::error(format!("Error: {}", e))],
    }
}

/// Handle /translate command - translate text between languages
pub async fn handle_subagent_translate(
    state: &mut ReplState,
    lang_pair: String,
    text: String,
) -> Vec<CommandOutput> {
    use crate::chat::subagent::{SubagentConfig, SubagentRunner};

    // Save user command to conversation context
    state
        .session
        .add_user_message(format!("/translate {} {}", lang_pair, text));

    let (model, _, _) = state.settings.get_subcommand_config("translate");
    let config = SubagentConfig::new(model, "Translation");
    let runner = SubagentRunner::new(state.ollama.clone(), config);

    match runner.run_translate(&lang_pair, &text).await {
        Ok(result) => {
            let _ = state.session.add_assistant_message(result.clone(), None);
            vec![CommandOutput::info(result)]
        }
        Err(e) => vec![CommandOutput::error(format!("Error: {}", e))],
    }
}

/// Handle /summarize command - summarize text
pub async fn handle_subagent_summarize(state: &mut ReplState, text: String) -> Vec<CommandOutput> {
    use crate::chat::subagent::{SubagentConfig, SubagentRunner};

    // Save user command to conversation context
    state
        .session
        .add_user_message(format!("/summarize {}", text));

    let (model, _, _) = state.settings.get_subcommand_config("summarize");
    let config = SubagentConfig::new(model, "Summarization");
    let runner = SubagentRunner::new(state.ollama.clone(), config);

    match runner.run_summarize(&text).await {
        Ok(result) => {
            let _ = state.session.add_assistant_message(result.clone(), None);
            vec![CommandOutput::info(result)]
        }
        Err(e) => vec![CommandOutput::error(format!("Error: {}", e))],
    }
}

/// Handle /feedback command
///
/// Records user feedback (good/bad/correction) on an assistant message.
///
/// Two-guard anonymous check:
/// 1. If db is None → error
/// 2. If session.anonymous → error
///
/// Target resolution:
/// - If item_id provided (msg:N) → use that
/// - If no item_id → use last_assistant_message_id
/// - If neither → error: "No assistant message to give feedback on"
///
/// Importance adjustment:
/// - Good: importance + 0.05 (capped at 1.0)
/// - Bad: importance - 0.1 (floored at 0.0)
/// - Correction: no importance change
pub fn handle_feedback(
    state: &mut ReplState,
    signal_type: crate::feedback::types::FeedbackSignalType,
    item_id: Option<i64>,
    correction_text: Option<String>,
) -> Vec<CommandOutput> {
    use crate::db::feedback_ops::insert_feedback_signal;
    use crate::feedback::types::{FeedbackSignal, FeedbackSignalType, FeedbackSource};

    // Anonymous block — first guard: db.is_none()
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            log::warn!("Cannot give feedback: database not initialized (anonymous mode)");
            return vec![CommandOutput::error(
                "Database not initialized. Run chat without --anonymous.",
            )];
        }
    };

    // Second guard: session.anonymous
    if state.session.anonymous {
        log::warn!("Cannot give feedback in anonymous mode");
        return vec![CommandOutput::error(
            "Cannot give feedback in anonymous mode.",
        )];
    }

    // Resolve target item_id
    let target_id = match item_id {
        Some(id) => id,
        None => match state.last_assistant_message_id {
            Some(id) => id,
            None => {
                return vec![CommandOutput::info(
                    "No assistant message to give feedback on.",
                )];
            }
        },
    };

    // Determine importance delta based on signal type
    let importance_delta: f32 = match signal_type {
        FeedbackSignalType::Good => 0.05,
        FeedbackSignalType::Bad => -0.1,
        FeedbackSignalType::Correction => 0.0,
    };

    // Create feedback signal
    let now_ts: i64 = chrono::Utc::now().timestamp();
    let signal = FeedbackSignal {
        item_id: target_id,
        session_id: Some(state.session.id.clone()),
        signal_type,
        base_value: signal_type.base_value(),
        correction_text,
        source: FeedbackSource::User,
        created_at: now_ts,
    };

    // Insert feedback signal via db.with_connection()
    let insert_result: Result<i64, String> = db
        .with_connection(|conn| {
            insert_feedback_signal(
                conn,
                signal.item_id,
                signal.session_id.as_deref(),
                signal.signal_type,
                signal.base_value,
                signal.correction_text.as_deref(),
                signal.source,
                signal.created_at,
            )
            .map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e)))
            })
        })
        .map_err(|e| format!("{}", e));

    match insert_result {
        Ok(_row_id) => {
            // Adjust importance (except for correction which has delta 0.0)
            if importance_delta != 0.0
                && let Err(e) = db.adjust_importance(target_id, importance_delta)
            {
                log::warn!("Could not adjust importance for item {}: {}", target_id, e);
            }

            log::debug!(
                "Feedback recorded: {} for msg:{} (delta: {:+.2})",
                signal.signal_type,
                target_id,
                importance_delta
            );

            // Get message excerpt for confirmation
            let excerpt: String = db
                .with_connection(|conn| {
                    conn.query_row(
                        "SELECT SUBSTR(content, 1, 80) FROM content_items WHERE id = ?1",
                        rusqlite::params![target_id],
                        |row| row.get::<_, String>(0),
                    )
                })
                .unwrap_or_else(|_| "(no content)".to_string());

            let signal_label = signal.signal_type.to_string();
            let mut outputs = vec![CommandOutput::success(format!(
                "{} feedback recorded for msg:{}",
                signal_label, target_id
            ))];
            outputs.push(CommandOutput::info(format!("  {}", excerpt)));

            if let Some(ref text) = signal.correction_text {
                outputs.push(CommandOutput::info(format!("  Correction: {}", text)));
            }

            if importance_delta > 0.0 {
                outputs.push(CommandOutput::info(format!(
                    "  Importance: +{:.2}",
                    importance_delta
                )));
            } else if importance_delta < 0.0 {
                outputs.push(CommandOutput::info(format!(
                    "  Importance: {:.2}",
                    importance_delta
                )));
            }

            outputs
        }
        Err(e) => {
            log::warn!("Failed to record feedback for item {}: {}", target_id, e);
            vec![CommandOutput::error(format!(
                "Failed to record feedback: {}",
                e
            ))]
        }
    }
}
