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
//!
//! # Event Handler Decomposition
//!
//! The main event handlers are extracted into `event_loop.rs`:
//!
//! - `handle_key_line()`: processes user input (slash commands and LLM queries)
//! - `handle_interrupt()`: handles Ctrl+C cancellation
//! - `handle_eof()`: handles Ctrl+D quit (saves session, exits immediately)
//! - `handle_llm_event()`: processes streaming tokens, completion, errors
//! - `apply_view_action()`: translates `ViewAction` into `RatatuiView` calls
//! - `spawn_llm_task()`: spawns background LLM task with streaming
//! - `spawn_compact_task()`: spawns background compaction task

use std::sync::Arc;

use crossterm::event::{self, Event as CrosstermEvent};
use tokio_util::sync::CancellationToken;

use super::app::{EmbeddingPhase, EmbeddingProgress};
use super::event_loop::{self, LoopAction};
use super::input::InputResult;
use super::llm_event::LlmEvent;
use super::repl_state::ReplState;
use super::view::ChatView;
use super::view::RatatuiView;

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

    // Create the TUI view (see RatatuiView::new() for initialization details)
    let mut view = RatatuiView::new(theme, model_names);

    // Wire embedding progress channel to session for per-message progress reporting
    state.session.embedding_tx = Some(view.embedding_tx());

    // Wire async message channel to session for background task notifications
    state.session.async_message_tx = Some(view.async_message_tx());

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
        // Get server URL from provider config (use first provider or "ollama" by name)
        let providers = crate::user_models::get_providers();
        let server_url = providers
            .values()
            .next()
            .map(|p| p.base_url.clone())
            .unwrap_or_else(|| "http://localhost:11434".to_string());

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

    // Show database recovery messages — run in background so the TUI is responsive.
    //
    // Design decision: recovery runs as a tokio::spawn background task instead of
    // blocking the TUI before the event loop. This was changed from synchronous to
    // background because (1) schema migration v11→v12 resets all has_embedding flags,
    // causing minutes of blocking; (2) the TUI was unusable during recovery.
    //
    // Concurrency safety: the Database struct uses Arc<Mutex<Connection>>, which
    // serializes all SQLite accesses. Concurrent RAG queries during recovery will not
    // cause "database is locked" errors — they are simply serialized. Items with
    // has_embedding = 0 are excluded from vector search results, so partial recovery
    // does not produce incorrect results, only temporarily incomplete ones.
    //
    // On exit (/quit, Ctrl+D), no embedding flush is performed. This is intentional:
    // the flush used to block exit for minutes. Missing embeddings are recovered on
    // next startup by this same background pipeline.
    if let (Some(db_ref), Some(client)) = (&state.db, &state.embedding_client) {
        // Show indexing indicator while regenerating embeddings
        view.app_mut()
            .set_embedding_progress(EmbeddingProgress::new(EmbeddingPhase::Content, 0, 1, 0, 1));
        view.render();

        // Embedding progress channel — reports current/total to the TUI status bar
        let tx = Some(view.embedding_tx());

        // Async message channel — for chat messages from background tasks
        let async_tx = view.async_message_tx();

        let db_clone = Arc::clone(db_ref);
        let client_clone = Arc::clone(client);

        // Spawn embedding recovery as a background task.
        // Previously this ran synchronously before the event loop, which blocked
        // the TUI for minutes when hundreds of embeddings needed regeneration
        // (e.g., after schema migration v11→v12 that resets all has_embedding flags).
        tokio::spawn(async move {
            // Step 1: Normalize inline thinking tags (v13→v14 data migration).
            // Must run BEFORE embedding recovery because normalize_inline_thinking()
            // sets has_embedding=0 for rows whose content was rewritten (thinking
            // removed). The recovery pipeline then regenerates these embeddings
            // from the cleaned content automatically.
            //
            // This runs synchronously inside the async spawn — typical DBs have
            // few pre-tool messages (<100), so it completes in <100ms. The slow
            // part (embedding regeneration) is handled by the recovery pipeline below.
            let split_fn = |content: &str| -> (Option<String>, String) {
                let processed = crate::chat::thinking::process_thinking(content);
                (processed.thinking, processed.content)
            };
            match db_clone.normalize_inline_thinking(split_fn) {
                Ok(count) if count > 0 => {
                    log::info!(
                        "Normalized {} pre-tool messages with inline thinking tags — \
                         embeddings will be regenerated",
                        count
                    );
                    // Brief progress signal to update the ⚙ indicator
                    if let Some(ref tx) = tx {
                        let _ = tx.send(EmbeddingProgress::new(
                            EmbeddingPhase::Content,
                            0,
                            count as usize,
                            0,
                            count as usize,
                        ));
                    }
                    // Chat message (Fix C) — sent via async channel so the
                    // TUI event loop picks it up and displays it
                    let msg = format!(
                        "💾 Migrated {} pre-tool message(s) — thinking preserved separately. \
                         Embeddings being regenerated\u{2026}",
                        count
                    );
                    let _ = async_tx.send(msg);
                }
                Ok(_) => { /* No rows to normalize — nothing to do */ }
                Err(e) => {
                    log::warn!("Failed to normalize inline thinking: {}", e);
                }
            }

            // Step 2: Regenerate embeddings if needed (after schema migration)
            let stats = crate::embeddings::regenerate_all_embeddings(
                &db_clone,
                &client_clone,
                true,
                tx.clone(),
            )
            .await;
            if stats.total_processed() > 0 {
                log::debug!(
                    "Regenerated {} embedding(s) ({} items, {} chunks)",
                    stats.total_processed(),
                    stats.items_processed,
                    stats.chunks_processed
                );
                if stats.has_errors() {
                    log::warn!(
                        "{} embedding(s) failed to generate. They will be retried on next startup.",
                        stats.total_failed()
                    );
                }
            }

            // Recover any missing embeddings from previous session
            let recovered = crate::embeddings::recover_missing_embeddings(
                &db_clone,
                &client_clone,
                true,
                tx.clone(),
            )
            .await;
            if recovered > 0 {
                log::debug!("Recovered {} missing embedding(s)", recovered);
            }

            // Recover missing fact embeddings and verify semantic dedup
            let fact_recovered = crate::facts::recovery::recover_missing_fact_embeddings(
                &db_clone,
                &client_clone,
                tx.clone(),
            )
            .await;
            if fact_recovered > 0 {
                log::debug!("Recovered {} fact embedding(s)", fact_recovered);
            }

            let stats =
                crate::facts::verify::verify_and_dedup_facts(&db_clone, &client_clone, tx.clone())
                    .await;
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

            // Signal completion to the TUI status bar.
            // All recovery functions now send their own progress via the channel,
            // but send a guaranteed final completion signal to ensure the indicator
            // is cleared even if any function returned early due to an error
            // without signaling completion.
            if let Some(ref tx) = tx {
                let _ = tx.send(EmbeddingProgress::completed());
            }
        });
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
    // Sender for LLM events — available in the event loop for command handlers
    // that need to trigger compaction streaming (e.g., /compact, /retry).
    let mut llm_tx: Option<tokio::sync::mpsc::Sender<LlmEvent>> = None;
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

        // Track whether any real event was processed this iteration.
        // During streaming, stream_token() and stream_thinking() already
        // call render() per token, so we skip the final render when only
        // the crossterm poll timed out with no actual key event.
        let mut needs_render = true;

        tokio::select! {
            // ── Crossterm key events ──────────────────────────────
            // Use a 5ms poll when the LLM is streaming — short enough
            // for responsive Ctrl+C (≤5ms latency), but long enough to
            // prevent the busy-wait spinlock that 0ms caused (100% CPU,
            // see Issue #193). Use a longer block when idle to let the
            // CPU sleep between events.
            crossterm_event = async {
                let poll_timeout = if has_llm_task {
                    // Streaming: 5ms poll balances responsiveness (Ctrl+C)
                    // with CPU efficiency (prevents busy-wait at 0ms).
                    std::time::Duration::from_millis(5)
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
                                    let action = event_loop::handle_key_line(
                                        &line,
                                        &mut *state,
                                        &mut view,
                                        &capabilities,
                                        &mut llm_tx,
                                        &mut llm_rx,
                                        &mut cancel_token,
                                    )
                                    .await;
                                    if action == LoopAction::Quit {
                                        view.restore();
                                        return Ok(());
                                    }
                                }
                                Some(InputResult::Interrupted) => {
                                    // Ctrl+C — cancel running LLM task
                                    event_loop::handle_interrupt(
                                        state,
                                        &mut view,
                                        &mut cancel_token,
                                        &mut llm_tx,
                                        &mut llm_rx,
                                    );
                                }
                                Some(InputResult::Eof) => {
                                    // Ctrl+D — quit: save session, history, and flush embeddings
                                    event_loop::handle_eof(&mut *state, &mut view).await;
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
                            event_loop::handle_mouse_event(mouse, &mut view);
                        }
                        CrosstermEvent::Paste(text) => {
                            // Bracketed paste from terminal (Shift+Insert, middle-click, etc.)
                            view.app_mut().textarea_mut().insert_str(&text);
                        }
                        _ => {
                            // Ignore other events (focus gained/lost, etc.)
                        }
                    }
                } else {
                    // No crossterm event — poll timed out. During streaming,
                    // stream_token()/stream_thinking() already render per
                    // token, so skip the redundant render at the bottom.
                    needs_render = has_llm_task;
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
                event_loop::handle_llm_event(
                    llm_event,
                    &mut *state,
                    &mut view,
                    &mut cancel_token,
                    &mut llm_tx,
                    &mut llm_rx,
                );
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
        // at the end of the message list. Use the current round index
        // since this is a catch-all drain on every event loop tick —
        // late-arriving messages should be grouped with the active round.
        let current_round = view.app().current_round();
        event_loop::drain_and_add_tool_messages(&mut view, current_round);

        // Re-render after each event or tick — but skip during streaming
        // when no real event was processed, since stream_token() and
        // stream_thinking() already render per token.
        if needs_render {
            view.app_mut().poll_embedding_progress();
            view.app_mut().poll_async_messages();
            view.render();
        }
    }
}
