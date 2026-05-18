//! TUI-based REPL loop for the chat
//!
//! This module provides `run_chat_repl_tui()`, which replaces the
//! blocking rustyline loop with a crossterm-based event loop that
//! renders via ratatui.
//!
//! # Architecture
//!
//! The event loop uses `tokio::select!` to handle three sources concurrently:
//!
//! 1. **Crossterm key events** — user input, tab completion, scroll
//! 2. **LLM events** — view actions and completion/cancellation from the
//!    background LLM task
//! 3. **Spinner tick** — periodic 100ms interval for spinner animation
//!
//! When the user submits a message, the LLM call is spawned on a
//! background tokio task. The LLM task uses `ChannelView` (a `ChatView`
//! proxy) to send view updates through an mpsc channel. The event loop
//! drains these and applies them to the real `RatatuiView`.
//!
//! Ctrl+C during LLM processing cancels the background task via
//! `CancellationToken`. The LLM result is discarded (not applied to state).

use crossterm::event::MouseButton;
use crossterm::event::{self, Event as CrosstermEvent, MouseEvent, MouseEventKind};
use tokio_util::sync::CancellationToken;

use std::sync::Arc;

use super::app::LlmState;
use super::channel_view::ChannelView;
use super::command_handlers;
use super::command_output::CommandOutput;
use super::commands::{ChatCommand, parse_command};
use super::input::InputResult;
use super::llm_event::{LlmEvent, ViewAction};
use super::repl_state::ReplState;
use super::tui::components::chat_area::ChatMessage;
use super::tui::components::chat_selection::mouse_to_visual_pos;
use super::view::ChatView;
use super::view::RatatuiView;

/// Channel capacity for LLM view actions.
///
/// Each `show_*` call during LLM processing sends one `ViewAction`.
/// A typical response may produce 5-10 view actions (content, thinking,
/// tokens, etc.). Tool calls may produce more. 128 is generous.
const LLM_VIEW_CHANNEL_CAPACITY: usize = 128;

/// Run the chat REPL using the TUI (ratatui + crossterm).
///
/// This replaces the blocking rustyline loop with an async event loop
/// that renders via ratatui. All view operations go through `RatatuiView`,
/// which implements `ChatView` and delegates to `App::add_message()`.
///
/// # Arguments
///
/// * `state` - The REPL state (session, model config, capabilities, etc.)
/// * `resume_message` - Optional message to display when resuming a session
///
/// # Errors
///
/// Returns an error if terminal setup fails or an irrecoverable error occurs.
pub async fn run_chat_repl_tui(
    state: &mut ReplState,
    resume_message: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let capabilities = state.capabilities.clone();
    let model_names = crate::user_models::list_all_model_names();
    let model_name = state.model_config.model_id.clone();
    let think_enabled = state.session.think;
    let tools_enabled = state.session.tools && capabilities.tools;
    let used_tokens = state.session.history_real_tokens();
    let max_tokens = state.model_config.num_ctx as usize;

    let theme = super::tui::markdown::MarkdownTheme::from_config(&state.settings.display.skin);

    // Create the TUI view (initializes terminal in raw mode, installs panic hook)
    let mut view = RatatuiView::new(theme, model_names);

    // Wire embedding progress channel to session for per-message progress reporting
    state.session.embedding_tx = Some(view.embedding_tx());

    // ── Startup Messages ─────────────────────────────────────────────
    // All startup messages are rendered through the TUI view so they
    // appear in the chat area (not as terminal prints).

    // Show welcome banner
    {
        let session = &state.session;
        let model_config = &state.model_config;
        let settings = &state.settings;

        let project = session.project_id.as_deref().unwrap_or("anonymous");
        let session_name = session.name.as_deref().unwrap_or(&session.id);
        let sandbox_status = crate::external::get_sandbox_status();
        let version = env!("CARGO_PKG_VERSION");
        let server_url = format!(
            "{}:{}",
            settings.model.ollama_host, settings.model.ollama_port
        );

        let (fact_count, note_count, doc_count) = if let Some(db_ref) = &state.db {
            (
                db_ref.count_facts().unwrap_or(0),
                db_ref.count_notes().unwrap_or(0),
                db_ref.count_documents().unwrap_or(0),
            )
        } else {
            (0, 0, 0)
        };

        let skill_count = if tools_enabled {
            crate::skills::load_skill_indexes().len()
        } else {
            0
        };

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
            skill_count,
        );
    }

    // Show resume message and recent context (when resuming a session)
    if let Some(msg) = resume_message {
        view.show_system(&msg);
        view.show_recent_context(&state.session);
    }

    // Show database recovery messages
    if let (Some(db_ref), Some(client)) = (&state.db, &state.embedding_client) {
        // Show indexing indicator while regenerating embeddings
        view.app_mut().set_embedding_progress(0, 1);
        view.render();

        // Embedding progress channel — reports current/total to the TUI status bar
        let tx = Some(view.embedding_tx());

        // Regenerate embeddings if needed (after schema migration)
        let stats =
            crate::embeddings::regenerate_all_embeddings(db_ref, client, true, tx.clone()).await;
        if stats.total_processed() > 0 {
            view.show_system(&format!(
                "Regenerated {} embedding(s) ({} items, {} chunks)",
                stats.total_processed(),
                stats.items_processed,
                stats.chunks_processed
            ));
            if stats.has_errors() {
                view.show_warning(&format!(
                    "{} embedding(s) failed to generate. They will be retried on next startup.",
                    stats.total_failed()
                ));
            }
        }

        // Recover any missing embeddings from previous session
        let recovered =
            crate::embeddings::recover_missing_embeddings(db_ref, client, true, tx.clone()).await;
        if recovered > 0 {
            view.show_system(&format!("Recovered {} missing embedding(s)", recovered));
        }

        // Recover missing fact embeddings and verify semantic dedup
        let fact_recovered =
            crate::facts::recovery::recover_missing_fact_embeddings(db_ref, client).await;
        if fact_recovered > 0 {
            log::debug!("Recovered {} fact embedding(s)", fact_recovered);
        }

        let stats = crate::facts::verify::verify_and_dedup_facts(db_ref, client).await;
        if stats.facts_checked > 0
            && (stats.duplicates_removed > 0
                || stats.contradictions_resolved > 0
                || stats.global_wins > 0)
        {
            log::debug!(
                "Fact verification: checked {}, removed {} duplicates, {} contradictions, {} global-wins",
                stats.facts_checked,
                stats.duplicates_removed,
                stats.contradictions_resolved,
                stats.global_wins
            );
        }

        // Clear embedding indicator
        view.app_mut().clear_embedding_progress();
        view.render();
    }

    // AGENTS.md loaded message
    if state.agents_md.is_some() {
        view.show_system("Loaded AGENTS.md context from current directory.");
    }

    // Show help line
    view.show_help_line();

    // Tools warning
    if state.session.tools && !capabilities.tools {
        view.show_warning(&format!(
            "Tools are enabled but model '{}' does not support tool calling.",
            state.model_config.model_id
        ));
        view.show_system("Tools have been disabled for this session. Use /tools to toggle.");
    }

    // ── Initial Status Bar ────────────────────────────────────────────
    let percent = if max_tokens > 0 {
        ((used_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
    } else {
        0
    };
    view.update_status_model(&model_name, think_enabled, tools_enabled);
    view.update_status_tokens(used_tokens, max_tokens, percent);

    // ── LLM task state ────────────────────────────────────────────────
    // Cancellation token for the running LLM task (if any)
    let mut cancel_token: Option<CancellationToken> = None;
    // Receiver for LLM events (view actions + completion/error)
    let mut llm_rx: Option<tokio::sync::mpsc::Receiver<LlmEvent>> = None;

    // ── Spinner tick interval ─────────────────────────────────────────
    // The spinner advances at a fixed cadence independent of streaming
    // tokens or key events. This ensures smooth, consistent animation.
    let spinner_interval = tokio::time::interval(std::time::Duration::from_millis(
        super::app::SPINNER_TICK_MS,
    ));
    // We need to pin the interval for use in tokio::select!
    tokio::pin!(spinner_interval);

    // ── Main Event Loop ──────────────────────────────────────────────
    loop {
        // Snapshot of LLM activity *at the start of this select iteration*.
        // Used to pick the right poll timeout without borrowing llm_rx in
        // the async block while the LLM branch holds &mut llm_rx.
        let has_llm_task = llm_rx.is_some();

        tokio::select! {
            // ── Crossterm key events ──────────────────────────────
            // Use a short (0ms) poll when the LLM is streaming so tokens
            // arrive without delay. Use a longer block when idle to
            // let the CPU sleep and avoid busy-waiting.
            crossterm_event = async {
                let poll_timeout = if has_llm_task {
                    // Streaming: non-blocking so the LLM events branch
                    // in tokio::select! is never starved.
                    std::time::Duration::from_millis(0)
                } else {
                    // Idle: block for up to the spinner tick interval.
                    // poll(120ms) lets the CPU sleep between events while
                    // still waking up often enough for spinner animation.
                    std::time::Duration::from_millis(super::app::SPINNER_TICK_MS)
                };
                if event::poll(poll_timeout).unwrap_or(false) {
                    event::read().ok()
                } else {
                    None
                }
            } => {
                if let Some(crossterm_event) = crossterm_event {
                    match crossterm_event {
                        CrosstermEvent::Key(key) => {
                            let result = view.app_mut().handle_key(key);

                            match result {
                                Some(InputResult::Line(line)) => {
                                    let line = line.trim().to_string();
                                    if line.is_empty() {
                                        continue;
                                    }

                                    // Add user message to chat area
                                    view.app_mut().add_message(ChatMessage::user(line.clone()));

                                    // Check for slash commands (only if first line starts with /)
                                    let first_line = line.lines().next().unwrap_or("");
                                    if first_line.starts_with('/') {
                                        match parse_command(&line) {
                                            Some(Ok(cmd)) => {
                                                // Handle model switch specially
                                                if let ChatCommand::Model { name } = &cmd {
                                                    let outputs = command_handlers::handle_model_switch(
                                                        state,
                                                        name,
                                                        &capabilities,
                                                    )
                                                    .await;
                                                    view.show_command_outputs(&outputs);

                                                    // Update the modeline with the new model name
                                                    let new_model_name =
                                                        state.model_config.model_id.clone();
                                                    let think_enabled =
                                                        state.session.think && state.capabilities.thinking;
                                                    let tools_enabled =
                                                        state.tools_active && state.capabilities.tools;
                                                    view.update_status_model(
                                                        &new_model_name,
                                                        think_enabled,
                                                        tools_enabled,
                                                    );

                                                    if outputs
                                                        .iter()
                                                        .any(|o| matches!(o, CommandOutput::Quit))
                                                    {
                                                        let _ = view.app_mut().save_history();
                                                        if !state.session.anonymous {
                                                            let _ = state.session.save_sqlite();
                                                        }
                                                        view.restore();
                                                        return Ok(());
                                                    }
                                                    continue;
                                                }

                                                // Handle other commands
                                                let mut dummy_input =
                                                    super::input::CrosstermInput::default();
                                                let outputs = command_handlers::handle_command(
                                                    cmd,
                                                    state,
                                                    &mut dummy_input,
                                                    &mut view as &mut dyn ChatView,
                                                )
                                                .await;

                                                view.show_command_outputs(&outputs);

                                                if outputs.iter().any(|o| matches!(o, CommandOutput::Quit))
                                                {
                                                    let _ = view.app_mut().save_history();
                                                    if !state.session.anonymous {
                                                        let _ = state.session.save_sqlite();
                                                    }
                                                    view.restore();
                                                    return Ok(());
                                                }
                                                continue;
                                            }
                                            Some(Err(e)) => {
                                                view.show_error(&e.to_string());
                                                continue;
                                            }
                                            None => {}
                                        }
                                    }

                                    // Send as user message to LLM (non-blocking)
                                    view.set_llm_state(LlmState::Thinking);
                                    spawn_llm_task(&line, state, &mut llm_rx, &mut cancel_token);
                                }
                                Some(InputResult::Interrupted) => {
                                    // Ctrl+C — cancel running LLM or ignore
                                    if let Some(token) = cancel_token.take() {
                                        token.cancel();
                                        view.app_mut().add_message(
                                            ChatMessage::system("Cancelled.".to_string()),
                                        );
                                        view.set_llm_state(LlmState::Idle);
                                        llm_rx = None;
                                    }
                                }
                                Some(InputResult::Eof) => {
                                    // Ctrl+D — quit: save session, history, and flush embeddings
                                    let _ = view.app_mut().save_history();
                                    if !state.session.anonymous {
                                        let _ = state.session.save_sqlite();

                                        // Flush pending embeddings before exit (same as /quit)
                                        if let (Some(db), Some(client)) =
                                            (&state.db, &state.embedding_client)
                                        {
                                            let progress_tx = state.session.embedding_tx.clone();
                                            command_handlers::flush_pending_embeddings(
                                                Arc::clone(db),
                                                Arc::clone(client),
                                                true, // suppress_spinner — avoid corrupting alternate screen
                                                progress_tx,
                                            )
                                            .await;
                                            crate::facts::recovery::flush_pending_fact_embeddings(
                                                db, client,
                                            )
                                            .await;
                                        }
                                    }
                                    view.restore();
                                    return Ok(());
                                }
                                Some(InputResult::Error(_)) => {
                                    // Should not happen in TUI mode
                                }
                                None => {
                                    // Other key event (buffer updated, cursor moved)
                                }
                            }
                        }
                        CrosstermEvent::Resize(_, _) => {
                            view.app_mut().scroll_to_bottom();
                        }
                        CrosstermEvent::Mouse(mouse) => {
                            handle_mouse_event(mouse, &mut view);
                        }
                        CrosstermEvent::Paste(text) => {
                            // Bracketed paste from terminal (Shift+Insert, middle-click, etc.)
                            view.app_mut().textarea_mut().insert_str(&text);
                        }
                        _ => {
                            // Ignore other events (focus gained/lost, etc.)
                        }
                    }
                }
            }

            // ── LLM events (when LLM is running) ──────────────────
            Some(llm_event) = async {
                if let Some(rx) = &mut llm_rx {
                    rx.recv().await
                } else {
                    // No LLM running — never complete this branch
                    std::future::pending().await
                }
            } => {
                match llm_event {
                    LlmEvent::ViewAction(action) => {
                        apply_view_action(&mut view, action);
                    }
                    LlmEvent::StreamToken(token) => {
                        // Append token to the current streaming message (or create one)
                        view.stream_token(&token);
                    }
                    LlmEvent::StreamThinking(thinking_token) => {
                        // Append thinking token to the current streaming thinking block
                        view.stream_thinking(&thinking_token);
                    }
                    LlmEvent::StreamBlockDone => {
                        // Pre-tool block complete — the streaming zone was
                        // already finalized by ToolCallStarted (which calls
                        // finalize_streaming_zone_as_is()). Do NOT call
                        // stream_done() here: that calls finalize_stream(),
                        // which would add a DUPLICATE Assistant message when
                        // no AssistantStreaming exists (already converted).
                        view.app_mut().block_finalized = true;
                        view.set_llm_state(LlmState::ToolCall);
                    }
                    LlmEvent::StreamDone {
                        content,
                        thinking,
                        metrics,
                    } => {
                        // Replace the streaming message with the final markdown version
                        view.stream_done(&content, thinking.as_deref(), metrics.as_ref());
                    }
                    LlmEvent::Complete { session, used_tokens, max_tokens, percent } => {
                        // Update the session with the one from the LLM task
                        state.session = *session;
                        view.update_status_tokens(used_tokens, max_tokens, percent);
                        view.set_llm_state(LlmState::Idle);
                        cancel_token = None;
                        llm_rx = None;
                    }
                    LlmEvent::Error(error) => {
                        view.app_mut().add_message(ChatMessage::error(error));
                        view.set_llm_state(LlmState::Idle);
                        cancel_token = None;
                        llm_rx = None;
                    }
                    LlmEvent::Cancelled => {
                        // LLM was cancelled — already handled by Ctrl+C branch
                        cancel_token = None;
                        llm_rx = None;
                    }
                    LlmEvent::InterToolText { content, .. } => {
                        // Inter-tool block arrived from process_next().
                        // Display immediately as a stable block before tools.
                        view.app_mut().add_message(ChatMessage::assistant_markdown(content));
                        view.app_mut().set_llm_state(LlmState::ToolCall);
                    }
                    LlmEvent::ToolCallStarted => {
                        // Tool calls detected — finalize streaming and transition
                        view.app_mut().finalize_streaming_zone_as_is();
                        view.set_llm_state(LlmState::ToolCall);
                    }
                }
            }

            // ── Spinner tick ─────────────────────────────────────
            _ = spinner_interval.tick() => {
                // Advance the spinner at a fixed cadence (~180ms).
                // This is the ONLY place tick_spinner() is called, ensuring
                // the animation speed is independent of streaming tokens or
                // key events. The event loop wakes up here even when there
                // are no other events, keeping the spinner alive.
                view.app_mut().tick_spinner();
            }
        }

        // Drain tool messages from the global callback and insert them
        // in the correct position (before streaming zone when LLM is active).
        // This must happen before render() so messages appear in order.
        for msg in view.drain_tool_messages() {
            if view.app().llm_state() != LlmState::Idle {
                view.app_mut()
                    .insert_before_streaming_zone(ChatMessage::tool(msg));
            } else {
                view.app_mut().add_message(ChatMessage::tool(msg));
            }
        }

        // Re-render after each event or tick
        view.app_mut().poll_embedding_progress();
        view.render();
    }
}

/// Spawn the LLM task in the background with streaming token display.
///
/// Creates a `ChannelView` that the LLM task uses to send view updates
/// through an mpsc channel. The event loop drains these as `LlmEvent::ViewAction`.
/// Streaming tokens are sent directly as `LlmEvent::StreamToken/StreamThinking`.
/// On completion, sends `LlmEvent::StreamDone` then `LlmEvent::Complete` with
/// the updated session and final token counts.
/// On error, sends `LlmEvent::Error`.
///
/// The `cancel_token` allows the event loop to cancel the task on Ctrl+C.
fn spawn_llm_task(
    line: &str,
    state: &mut ReplState,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    cancel_token: &mut Option<CancellationToken>,
) {
    // Create channels for LLM events
    let (llm_tx, new_llm_rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);
    *llm_rx = Some(new_llm_rx);

    // Create cancellation token
    let token = CancellationToken::new();
    *cancel_token = Some(token.clone());

    // Create the channel view proxy
    let (view_tx, view_rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);

    // Forward view actions from the ChannelView sender to the LLM event sender.
    // This runs in a small forwarding task.
    let llm_tx_clone = llm_tx.clone();
    tokio::spawn(async move {
        let mut rx = view_rx;
        while let Some(action) = rx.recv().await {
            if llm_tx_clone
                .send(LlmEvent::ViewAction(action))
                .await
                .is_err()
            {
                break; // Event loop dropped
            }
        }
    });

    // Create the ChannelView for the LLM task
    let mut channel_view = ChannelView::new(view_tx);

    // Clone the state for the LLM task
    let mut task_state = state.clone();

    // Spawn the LLM task with streaming
    let line_owned = line.to_string();
    tokio::spawn(async move {
        // Check for cancellation before starting
        if token.is_cancelled() {
            let _ = llm_tx.send(LlmEvent::Cancelled).await;
            return;
        }

        // Use the streaming handler which sends tokens through the LlmEvent channel
        super::repl::handle_user_message_stream(
            &line_owned,
            &mut task_state,
            &mut channel_view as &mut dyn ChatView,
            llm_tx.clone(),
            token.clone(),
        )
        .await;

        // Check if cancelled after the call
        if token.is_cancelled() {
            let _ = llm_tx.send(LlmEvent::Cancelled).await;
            return;
        }

        // Send the updated session and token counts back
        let used_tokens = task_state.session.history_real_tokens();
        let max_tokens = task_state.model_config.num_ctx as usize;
        let percent = if max_tokens > 0 {
            ((used_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };

        let _ = llm_tx
            .send(LlmEvent::Complete {
                session: Box::new(task_state.session),
                used_tokens,
                max_tokens,
                percent,
            })
            .await;
    });
}

/// Apply a `ViewAction` to the real `RatatuiView`.
///
/// This translates the channel-based view proxy calls into actual
/// rendering on the TUI view.
///
/// When the LLM is in `ToolCall` or `Streaming` state, content
/// ViewActions (ShowAssistantResponse, ShowThinking, ShowMarkdown)
/// are inserted before the streaming zone so they appear before
/// any in-progress streaming content. Other ViewActions (system
/// messages, errors, token metrics) always append at the end.
///
/// # Deduplication
///
/// After `StreamBlockDone` finalizes the pre-tool block and before
/// `StreamDone` adds the post-tool content, ViewActions that carry the
/// ALREADY-SHOWN pre-tool text (via `PreToolContent` in the coordinator)
/// will be drained by `drain_into_llm_channel()` and arrive as
/// `ShowThinking` + `ShowMarkdown` on the event loop. Because the
/// pre-tool content was already displayed by the `StreamToken`/
/// `StreamThinking` sequence that preceded `StreamBlockDone`,
/// `apply_view_action` skips those content messages to prevent
/// duplicating text on the screen.
fn apply_view_action(view: &mut RatatuiView, action: ViewAction) {
    let llm_state = view.app().llm_state();
    let has_streaming_zone = view.app().has_streaming_zone();

    match action {
        ViewAction::ShowSystem(msg) => {
            view.show_system(&msg);
        }
        ViewAction::ShowError(msg) => {
            view.show_error(&msg);
        }
        ViewAction::ShowAssistantResponse { content, thinking } => {
            // When LLM is active and content was already displayed via
            // StreamToken events (streaming zone exists), don't duplicate.
            // The streaming zone will be finalized by StreamDone.
            if has_streaming_zone {
                // Only show thinking if it's present — it may be from a
                // pre-tool round and should be preserved before the zone.
                if let Some(thinking_content) = thinking {
                    if llm_state != LlmState::Idle {
                        view.app_mut()
                            .insert_before_streaming_zone(ChatMessage::thinking(thinking_content));
                    } else {
                        view.app_mut()
                            .add_message(ChatMessage::thinking(thinking_content));
                    }
                    view.render();
                }
                // Skip adding the assistant message — it's already streaming.
            } else if llm_state != LlmState::Idle {
                // No streaming zone but LLM is active (tool call before
                // streaming starts) — insert before future streaming zone.
                if let Some(thinking_content) = thinking {
                    view.app_mut()
                        .insert_before_streaming_zone(ChatMessage::thinking(thinking_content));
                }
                view.app_mut()
                    .insert_before_streaming_zone(ChatMessage::assistant_markdown(content));
                view.render();
            } else {
                // LLM is idle — no streaming, safe to add normally.
                view.show_assistant_response(&content, thinking.as_deref());
            }
        }
        ViewAction::ShowTokenMetrics(metrics) => {
            view.show_token_metrics(&metrics);
        }
        ViewAction::ShowContextWarning { percent, message } => {
            view.show_context_warning(percent, &message);
        }
        ViewAction::ShowCompactProgress(msg) => {
            view.show_compact_progress(&msg);
        }
        ViewAction::ShowCompactComplete {
            count,
            preserved_first,
            preserved_last,
        } => {
            view.show_compact_complete(count, preserved_first, preserved_last);
        }
        ViewAction::ShowMarkdown(content) => {
            // Pre-tool content shown as markdown during tool calls.
            // When streaming zone exists, the content is already being
            // displayed via StreamToken events — don't duplicate.
            if has_streaming_zone {
                // Already streaming — skip duplicate markdown content.
            } else if llm_state != LlmState::Idle {
                // No streaming yet but LLM is active — insert before zone.
                view.app_mut()
                    .insert_before_streaming_zone(ChatMessage::assistant_markdown(content));
                view.render();
            } else {
                view.show_markdown(&content);
            }
        }
        ViewAction::ShowThinking(thinking) => {
            // Thinking content during tool calls should be inserted
            // before the streaming zone so it appears above streaming.
            // This is NOT a duplicate — it's from a previous round.
            if llm_state != LlmState::Idle {
                view.app_mut()
                    .insert_before_streaming_zone(ChatMessage::thinking(thinking));
                view.render();
            } else {
                view.show_thinking(&thinking);
            }
        }
        ViewAction::ClearContinuationLine => {
            view.clear_continuation_line();
        }
        ViewAction::ShowCommandOutput(output) => {
            view.show_command_output(&output);
        }
    }
}

/// Number of lines to scroll per mouse wheel tick.
const MOUSE_SCROLL_LINES: u16 = 3;

/// Handle mouse events for the TUI chat REPL.
///
/// Currently supports:
/// - Mouse wheel scroll up/down in the chat area (3 lines per tick)
/// - Left-click + drag to select text in the chat area
/// - Clicking in chat area clears textarea selection (bidirectional mutual exclusion)
/// - Clicking outside chat area clears chat selection
fn handle_mouse_event(mouse: MouseEvent, view: &mut RatatuiView) {
    let app = view.app_mut();
    let chat_area = app.chat_area_rect_cache();
    let scroll_from_top = app.scroll_from_top_cache();

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Check if click is within the chat area
            if let Some((visual_line, char_offset)) =
                mouse_to_visual_pos(mouse.column, mouse.row, chat_area, scroll_from_top)
            {
                // Start text selection in chat area — clear textarea selection (mutual exclusion)
                app.textarea_mut().cancel_selection();
                app.chat_selection_mut().begin(visual_line, char_offset);
            } else {
                // Click outside chat area — clear chat selection (mutual exclusion)
                app.chat_selection_mut().clear();
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Extend selection if we're in selection mode
            if app.chat_selection().is_dragging()
                && let Some((visual_line, char_offset)) =
                    mouse_to_visual_pos(mouse.column, mouse.row, chat_area, scroll_from_top)
            {
                app.chat_selection_mut().extend(visual_line, char_offset);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            // Finish selection if we're in selection mode
            if app.chat_selection().is_dragging()
                && let Some((visual_line, char_offset)) =
                    mouse_to_visual_pos(mouse.column, mouse.row, chat_area, scroll_from_top)
            {
                app.chat_selection_mut().finish(visual_line, char_offset);
            } else if app.chat_selection().is_dragging() {
                // Released outside chat area — clear selection
                app.chat_selection_mut().clear();
            }
        }
        MouseEventKind::ScrollUp => {
            app.scroll_state_mut().scroll_up(MOUSE_SCROLL_LINES);
        }
        MouseEventKind::ScrollDown => {
            app.scroll_state_mut().scroll_down(MOUSE_SCROLL_LINES);
        }
        _ => {}
    }
}
