//! Event loop handlers for the TUI REPL
//!
//! This module extracts the main event handlers from the monolithic
//! `run_chat_repl_tui()` event loop into named functions:
//!
//! - `handle_key_line()`: processes user input (slash commands and LLM queries)
//! - `handle_interrupt()`: handles Ctrl+C cancellation
//! - `handle_eof()`: handles Ctrl+D quit (saves session, exits immediately)
//! - `handle_llm_event()`: processes streaming tokens, completion, errors
//!
//! Helper functions moved from `repl_tui.rs`:
//!
//! - `apply_view_action()`: translates `ViewAction` into `RatatuiView` calls
//! - `drain_and_add_tool_messages()`: drains tool messages into chat area
//! - `spawn_llm_task()`: spawns background LLM task with streaming
//! - `spawn_compact_task()`: spawns background compaction task
//! - `handle_mouse_event()`: processes mouse events for chat area

use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use tokio_util::sync::CancellationToken;

use super::app::App;
use super::app::LlmState;
use super::channel_view::ChannelView;
use super::command_handlers;
use super::command_output::CommandOutput;
use super::commands::{ChatCommand, parse_command};
use super::llm_event::{LlmEvent, ViewAction};
use super::repl_state::ReplState;
use super::tui::components::chat_area::ChatMessage;
use super::tui::components::chat_selection::mouse_to_visual_pos;
use super::view::ChatView;
use super::view::RatatuiView;
use super::view::TokenMetrics;
use crate::capabilities::ModelCapabilities;
use crate::chat::fact_tracker::{SharedFactTracker, lock_fact_mirror};
use crate::chat::session::ChatSession;

/// Channel capacity for LLM view actions.
///
/// Each `show_*` call during LLM processing sends one `ViewAction`.
/// A typical response may produce 5-10 view actions (content, thinking,
/// tokens, etc.). Tool calls may produce more. 128 is generous.
const LLM_VIEW_CHANNEL_CAPACITY: usize = 128;

/// Action returned by event handlers to signal the event loop.
///
/// Handlers return `LoopAction::Continue` to keep the loop running
/// or `LoopAction::Quit` to exit the REPL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopAction {
    /// Continue processing events.
    Continue,
    /// Exit the event loop (user requested quit).
    Quit,
    /// Wait for the cancelled task's terminal event, then exit. A bounded
    /// watchdog in the event loop falls back to the per-turn fact mirror if
    /// delivery never happens.
    QuitAfterDelivery,
}

/// True while a cancelled LLM task still owns the event channel.
///
/// The terminal `Cancelled` event carries the task's session; a new
/// submission, `/compact` or any channel-spawning command must wait until
/// `handle_cancelled` drains it, or that event — and the fact tracker it
/// preserves — would be dropped. Uses only existing state: the token is
/// present exactly while a task is still tracked, `is_cancelled` is the
/// cancellation-in-progress signal, and a still-alive `llm_rx` means the
/// terminal event has not been delivered yet.
pub(crate) fn cancellation_delivery_pending(
    cancel_token: &Option<CancellationToken>,
    llm_rx: &Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) -> bool {
    cancel_token
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
        && llm_rx.is_some()
}

/// Fold the per-turn mirror's completed-tool facts into the live session.
///
/// The mirror is a fallback, not a second authority: the terminal `Cancelled`
/// merge remains the authoritative path, and repeated full-tracker merges are
/// idempotent for visible facts and recency.
fn merge_fact_mirror(session: &mut ChatSession, mirror: Option<&SharedFactTracker>) {
    if let Some(mirror) = mirror {
        let mirrored = lock_fact_mirror(mirror);
        session.merge_fact_tracker(&mirrored);
    }
}

/// Persist the live session and terminal input history.
fn persist_live_session(state: &mut ReplState, view: &mut RatatuiView) {
    let _ = view.app_mut().save_history();
    if !state.session.anonymous
        && let Err(err) = state.session.save_sqlite()
    {
        log::warn!("Failed to persist the live session: {err}");
    }
}

// ── Key input handler ─────────────────────────────────────────────────

/// Process a submitted line of user input.
///
/// Handles slash commands (starting with `/`) by dispatching to the
/// appropriate handler, or sends the line as a user message to the LLM.
///
/// Returns `LoopAction::Quit` if the user requested exit, `Continue` otherwise.
#[expect(clippy::too_many_arguments)] // one task-lifecycle set: channel, token and mirror travel together
pub async fn handle_key_line(
    line: &str,
    state: &mut ReplState,
    view: &mut RatatuiView,
    capabilities: &ModelCapabilities,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    cancel_token: &mut Option<CancellationToken>,
    fact_mirror: &mut Option<SharedFactTracker>,
) -> LoopAction {
    let line = line.trim().to_string();
    if line.is_empty() {
        return LoopAction::Continue;
    }

    let first_line = line.lines().next().unwrap_or("");
    let parsed_command = if first_line.starts_with('/') {
        parse_command(&line)
    } else {
        None
    };

    // While a cancelled LLM task still owns the channel, its terminal
    // `Cancelled` event (with the session facts of the aborted turn) has
    // not been delivered yet. The window ends only when the task returns —
    // or when every sender drops, handled by `handle_llm_channel_closed`.
    // Most input is refused so the terminal event cannot be replaced; quit
    // and `/new` keep fact-preserving exits so this wait is never a trap.
    if cancellation_delivery_pending(cancel_token, llm_rx) {
        match parsed_command.as_ref() {
            Some(Ok(ChatCommand::Quit)) => {
                merge_fact_mirror(&mut state.session, fact_mirror.as_ref());
                persist_live_session(state, view);
                view.app_mut().add_message(ChatMessage::system(
                    "Finishing the cancelled turn before exit...".to_string(),
                ));
                return LoopAction::QuitAfterDelivery;
            }
            Some(Ok(ChatCommand::New)) => {
                // A new session may not open while the old tool still holds
                // the turn: it would keep producing effects into the new
                // conversation. The wait is real; the terminal frees it.
                if cancel_token.is_some() {
                    view.app_mut().add_message(ChatMessage::system(
                        "A tool is still executing; wait for it to reach a safe point \
                         before starting a new session."
                            .to_string(),
                    ));
                    return LoopAction::Continue;
                }
                merge_fact_mirror(&mut state.session, fact_mirror.as_ref());
                persist_live_session(state, view);
                *cancel_token = None;
                *llm_tx = None;
                *llm_rx = None;
                *fact_mirror = None;
                view.app_mut().add_message(ChatMessage::system(
                    "Saved completed tool facts before starting a new session.".to_string(),
                ));
                view.app_mut().set_compaction_paused(false);
            }
            _ => {
                view.app_mut().add_message(ChatMessage::system(
                    "Cancellation in progress. Wait a moment, then try again, or exit.".to_string(),
                ));
                return LoopAction::Continue;
            }
        }
    }

    // Add user message to chat area
    view.app_mut().add_message(ChatMessage::user(line.clone()));

    if let Some(parse_result) = parsed_command {
        match parse_result {
            Ok(cmd) => {
                // Handle model switch specially
                if let ChatCommand::Model { name } = &cmd {
                    let outputs =
                        command_handlers::handle_model_switch(state, name, capabilities).await;
                    view.show_command_outputs(&outputs);

                    // Update the modeline with the new model name
                    let new_model_name = state.model_config.model_id.clone();
                    let think_enabled = state.session.think && state.capabilities.thinking;
                    let tools_enabled = state.tools_active && state.capabilities.tools;
                    view.update_status_model(&new_model_name, think_enabled, tools_enabled);

                    if outputs.iter().any(|o| matches!(o, CommandOutput::Quit)) {
                        let _ = view.app_mut().save_history();
                        if !state.session.anonymous {
                            let _ = state.session.save_sqlite();
                        }
                        return LoopAction::Quit;
                    }
                    return LoopAction::Continue;
                }

                // Handle toggle style specially (needs App access)
                if let ChatCommand::ToggleStyle = &cmd {
                    let app = view.app_mut();
                    app.toggle_style();
                    let label = if app.style_enabled() { "on" } else { "off" };
                    let output = CommandOutput::Info(format!("Style rendering: {label}"));
                    view.show_command_outputs(&[output]);
                    return LoopAction::Continue;
                }

                // Handle /compact specially — spawn background task
                // to avoid freezing the TUI during compaction.
                // Guard: refuse to compact while an LLM task is
                // already running, as spawn_compact_task replaces
                // the channel unconditionally.
                if let ChatCommand::Compact = &cmd {
                    if llm_tx.is_some() {
                        view.show_command_outputs(&[
                            CommandOutput::info(
                                "Cannot compact while LLM is busy. Wait for the current response to finish.",
                            ),
                        ]);
                    } else if state.session.messages.is_empty() {
                        view.show_command_outputs(&[CommandOutput::info(
                            "No messages to compact.",
                        )]);
                    } else {
                        let msg_count = state.session.messages.len();
                        view.show_command_outputs(&[CommandOutput::progress(format!(
                            "Compacting {} messages...",
                            msg_count
                        ))]);
                        view.set_llm_state(LlmState::Compacting);
                        spawn_compact_task(state, llm_tx, llm_rx, cancel_token);
                    }
                    return LoopAction::Continue;
                }

                // Handle other commands
                let mut dummy_input = super::input::CrosstermInput::default();

                // Track whether this command changes status bar
                // indicators (🧠 think, 🔧 tools) so we can
                // update the modeline after execution.
                let updates_status = matches!(cmd, ChatCommand::Think { .. } | ChatCommand::Tools);
                // Track whether this command changes the session list
                // (save, load, new, session forget) so we can refresh
                // the completer's session entries afterward.
                let refreshes_sessions = matches!(
                    cmd,
                    ChatCommand::Save { .. }
                        | ChatCommand::Load { .. }
                        | ChatCommand::New
                        | ChatCommand::SessionForget { .. }
                );
                // For commands that need llm_tx (e.g., /compact,
                // /retry), create a channel or reuse existing.
                // Track whether we had an active LLM task before
                // the command, so we don't leak a stale channel.
                let had_llm_task = llm_tx.is_some();
                let cmd_llm_tx = if let Some(tx) = llm_tx.as_ref() {
                    tx.clone()
                } else {
                    // No active LLM task — create a temporary
                    // channel for the command. After the command
                    // returns, we MUST clear llm_rx to avoid
                    // busy-waiting (has_llm_task = true with 0ms
                    // poll timeout).
                    let (tx, rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);
                    *llm_rx = Some(rx);
                    tx
                };
                let outputs = command_handlers::handle_command(
                    cmd,
                    state,
                    &mut dummy_input,
                    &mut *view as &mut dyn ChatView,
                    cmd_llm_tx,
                )
                .await;

                // If there was no active LLM task before
                // the command and none was spawned during it
                // (e.g., /help, /context), clear the temporary
                // channel to prevent busy-waiting with 0ms poll.
                if !had_llm_task && llm_tx.is_some() {
                    // A command (e.g., /compact) spawned a task
                    // and set llm_tx — keep the channel alive
                    // so we receive streaming events.
                } else if !had_llm_task {
                    // No task was running before, and no task
                    // was spawned — clear the temp channel.
                    *llm_tx = None;
                    *llm_rx = None;
                }

                view.show_command_outputs(&outputs);

                // Refresh session entries in the completer after
                // commands that change the session list, but only
                // if no error occurred (preview-only or failed commands
                // don't change the DB).
                if refreshes_sessions
                    && !outputs.iter().any(|o| matches!(o, CommandOutput::Error(_)))
                {
                    let entries = state.session_entries_for_completer();
                    view.app_mut().refresh_session_entries(entries);
                }

                // Commands that change status bar indicators
                // (🧠 think, 🔧 tools) must update the modeline.
                // Model and ToggleStyle are handled in their own
                // blocks above; Think and Tools fall through here.
                if updates_status {
                    let think_enabled = state.session.think && state.capabilities.thinking;
                    let tools_enabled = state.tools_active && state.capabilities.tools;
                    view.update_status_model(
                        &state.model_config.model_id,
                        think_enabled,
                        tools_enabled,
                    );
                }

                // The pause indicator mirrors the governor wherever a
                // command could have lifted or reset it (/compact, /new,
                // /session forget); the governor stays the source of truth.
                view.app_mut()
                    .set_compaction_paused(state.session.governor.is_paused());

                if outputs.iter().any(|o| matches!(o, CommandOutput::Quit)) {
                    let _ = view.app_mut().save_history();
                    if !state.session.anonymous {
                        let _ = state.session.save_sqlite();
                    }
                    return LoopAction::Quit;
                }
                return LoopAction::Continue;
            }
            Err(e) => {
                view.show_error(&e.to_string());
                return LoopAction::Continue;
            }
        }
    }

    // Send as user message to LLM (non-blocking)
    // Reset round counter — each user prompt starts a fresh LLM cycle.
    view.app_mut().reset_round();
    view.set_llm_state(LlmState::Thinking);
    spawn_llm_task(&line, state, llm_tx, llm_rx, cancel_token, fact_mirror);
    LoopAction::Continue
}

/// Handle Ctrl+C (interrupt) — cancel running LLM task or show warning.
///
/// If an LLM task is running, cancels it via a borrowed cancellation token;
/// the token and receiver stay in place as the cancellation-in-progress
/// markers until the terminal `Cancelled` event is handled. The loop's own
/// Sender is dropped immediately: the task and its forwarding task still
/// own senders, so terminal delivery is unaffected, while a dying producer
/// can now close the channel and reach the `None` recovery branch. If
/// compaction is running, shows a warning (compaction is not cancellable).
pub fn handle_interrupt(
    _state: &ReplState,
    app: &mut App,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    _llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    if let Some(token) = cancel_token.as_ref() {
        token.cancel();
        // A running tool owns its own safe point: the request stays pending
        // and visible until the tool's terminal, instead of pretending an
        // immediate stop that never happened.
        if app.llm_state() == LlmState::ToolCall {
            app.add_message(ChatMessage::system(
                crate::chat::coordinator::CANCELLATION_REQUESTED_MESSAGE.to_string(),
            ));
            log::info!(
                "Cancellation requested while a tool runs; waiting for the safe point"
            );
            app.set_llm_state(LlmState::Cancelling);
        } else {
            app.add_message(ChatMessage::system("Cancelled.".to_string()));
            app.reset_round();
            app.set_llm_state(LlmState::Idle);
        }
        // The token and the receiver stay in place: the task's terminal
        // `Cancelled` event (with its session) is still in flight and
        // `handle_cancelled` performs the actual cleanup. Dropping the
        // loop's Sender clone is what makes `recv() == None` reachable if
        // every producer dies before sending that event.
        *llm_tx = None;
    }
    // Compaction now cancels through the same channel as LLM tasks: while
    // it runs, this slot holds the compaction task's token. With neither
    // a token nor a running task, Ctrl+C has nothing to cancel.
}

/// Handle Ctrl+D (quit) — merge completed-tool facts, save, then exit.
///
/// Embedding recovery is NOT performed on exit; the background pipeline
/// recovers missing embeddings on the next startup.
/// While a cancelled task is still delivering, the first request waits for the
/// terminal event instead of exiting (the event loop bounds that wait); a
/// second request forces the exit through the mirror fallback.
///
/// Note: The caller is responsible for calling `view.restore()` after
/// this function returns `LoopAction::Quit`, since `restore()` consumes
/// `self`.
pub async fn handle_eof(
    state: &mut ReplState,
    view: &mut RatatuiView,
    cancel_token: &mut Option<CancellationToken>,
    _llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    fact_mirror: &mut Option<SharedFactTracker>,
    force: bool,
) -> LoopAction {
    merge_fact_mirror(&mut state.session, fact_mirror.as_ref());
    persist_live_session(state, view);

    if !force && cancellation_delivery_pending(cancel_token, llm_rx) {
        view.app_mut().add_message(ChatMessage::system(
            "Exit requested during cancellation; waiting briefly for the cancelled turn..."
                .to_string(),
        ));
        return LoopAction::QuitAfterDelivery;
    }
    LoopAction::Quit
}

/// The recovery message must claim fact preservation only when the mirror
/// actually exists. Compact-task channels have no tool-fact mirror, and a
/// false preservation claim would undermine trust in the real guarantee.
fn channel_closed_system_message(fact_mirror_present: bool) -> String {
    if fact_mirror_present {
        "LLM task ended unexpectedly. Completed tool facts were preserved.".to_string()
    } else {
        "LLM task ended unexpectedly.".to_string()
    }
}

/// Recover from every LLM event producer dying before a terminal event.
///
/// A task panic drops all senders, so `recv()` yields `None`. The completed-tool
/// mirror still exists, so fold it in, persist, and clear the task state;
/// without this the pending-delivery latch would never release.
pub fn handle_llm_channel_closed(
    state: &mut ReplState,
    view: &mut RatatuiView,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    fact_mirror: &mut Option<SharedFactTracker>,
) {
    merge_fact_mirror(&mut state.session, fact_mirror.as_ref());
    *fact_mirror = None;
    persist_live_session(state, view);
    view.app_mut()
        .add_message(ChatMessage::system(channel_closed_system_message(
            fact_mirror.is_some(),
        )));
    view.app_mut().reset_round();
    view.set_llm_state(LlmState::Idle);
    *cancel_token = None;
    *llm_tx = None;
    *llm_rx = None;
}

// ── LLM event handler ─────────────────────────────────────────────────

/// Process an LLM event (streaming token, completion, error, etc.).
///
/// Updates the view and state based on the event type.
/// Clears LLM task state (cancel_token, llm_tx, llm_rx) on terminal events
/// (Complete, Error, Cancelled, CompactStreamDone).
fn handle_stream_token(token: String, state: &mut ReplState, view: &mut RatatuiView) {
    // Append token to the current live turn
    view.app_mut().append_stream_token(&token);

    // Throttle status bar updates during streaming.
    let bucket = state.status_bar_bucket();
    if bucket != state.last_status_token_bucket {
        state.last_status_token_bucket = bucket;
        if let Some((used, max, pct)) = state.estimate_status_bar() {
            view.update_status_tokens(used, max, pct);
        }
    }
}

fn handle_stream_thinking(thinking_token: String, view: &mut RatatuiView) {
    // Append thinking token to the current live turn
    view.app_mut().append_stream_thinking(&thinking_token);
}

fn handle_stream_done(
    content: String,
    thinking: Option<String>,
    metrics: Option<TokenMetrics>,
    view: &mut RatatuiView,
) {
    // Finalize the live turn and commit it to the message history.
    let thinking_desc = thinking.as_ref().map_or_else(
        || "None".to_string(),
        |t| format!("Some({} chars)", t.len()),
    );
    log::debug!(
        "StreamDone: content_len={}, thinking={}, messages_before={}",
        content.len(),
        thinking_desc,
        view.app().message_count(),
    );
    view.app_mut()
        .finalize_stream(&content, thinking.as_deref());
    if let Some(m) = metrics
        && m.total_tokens > 0
    {
        view.show_token_metrics(&m);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_complete(
    session: Box<crate::chat::session::ChatSession>,
    used_tokens: usize,
    max_tokens: usize,
    percent: u8,
    state: &mut ReplState,
    view: &mut RatatuiView,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    // Update the session with the one from the LLM task
    state.session = *session;
    view.update_status_tokens(used_tokens, max_tokens, percent);
    view.set_llm_state(LlmState::Idle);
    // The task clone may have tripped the pause during trailing compaction;
    // the governor now lives in the session above, so mirror it to the bar.
    view.app_mut()
        .set_compaction_paused(state.session.governor.is_paused());
    log::debug!(
        "Complete: messages_count={}, current_round={}",
        view.app().message_count(),
        view.app().current_round(),
    );
    view.app_mut().reset_round();
    *cancel_token = None;
    *llm_tx = None;
    *llm_rx = None;
}

/// Preserve the task state when Error is followed by Cancelled on the same FIFO
/// producer; the latter still owns the fact merge and terminal cleanup.
fn clear_task_after_error(
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    if cancellation_delivery_pending(cancel_token, llm_rx) {
        return;
    }
    *cancel_token = None;
    *llm_tx = None;
    *llm_rx = None;
}

fn handle_error(
    error: String,
    state: &mut ReplState,
    view: &mut RatatuiView,
    fact_mirror: &mut Option<SharedFactTracker>,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    // Finalize any pending live_turn blocks (tool calls, streaming
    // text) BEFORE adding the error message. Without this,
    // render_messages puts self.messages (with the error) before
    // live_turn (with tool calls), causing the error to appear
    // out of order — before the tool calls instead of after.
    //
    // This is safe because LlmEvent::Error is only sent for fatal
    // system errors (timeout, connection lost, etc.) where the
    // ReAct loop has already been interrupted. Tool call errors
    // (invalid args, tool failure) are handled inside the coordinator
    // and never reach this handler — they use ToolExecutionFinished
    // with is_error=true instead.
    //
    // Only finalize if there's an active live_turn with content;
    // calling finalize_stream("", None) when there's no live_turn
    // would add an empty assistant message to the chat.
    if view.app().has_streaming_zone() {
        view.app_mut().finalize_stream("", None);
    }
    view.show_error(&error);
    log::debug!(
        "Complete (after error): messages_count={}, current_round={}",
        view.app().message_count(),
        view.app().current_round(),
    );
    view.app_mut().reset_round();
    view.set_llm_state(LlmState::Idle);
    // A fatal error is terminal: the task session clone never reaches the
    // live state, so fold the mirror's completed-tool facts before cleanup.
    merge_fact_mirror(&mut state.session, fact_mirror.as_ref());
    clear_task_after_error(cancel_token, llm_tx, llm_rx);
}

/// Fold the cancelled task session's fact tracker into the live session
/// and persist the live session, without adopting any other state from the
/// cancelled copy.
///
/// Core flushes the coordinator's facts into the task's session clone
/// before returning `CANCELLED_BY_USER`, so this fold is the last chance
/// to keep the aborted turn's file and command records. The clone's
/// messages/model/other fields are the task's snapshot, not the live
/// truth, and must not replace the live session.
fn merge_cancelled_facts(cancelled_session: &ChatSession, state: &mut ReplState) {
    state
        .session
        .merge_fact_tracker(&cancelled_session.fact_tracker);
    if !state.session.anonymous
        && let Err(err) = state.session.save_sqlite()
    {
        log::warn!("Failed to persist the session after a cancelled LLM turn: {err}");
    }
}

/// Terminal cleanup of a cancelled LLM turn.
///
/// Receives the session carried by the terminal `Cancelled` event; only
/// its fact tracker is folded into the live session. The round is reset
/// and the token and channels are cleared, resuming normal behavior for
/// the pending-delivery guard.
fn handle_cancelled(
    cancelled_session: Box<ChatSession>,
    state: &mut ReplState,
    app: &mut App,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    merge_cancelled_facts(&cancelled_session, state);
    // The clone is discarded, but a pause it tripped during the turn comes
    // from genuinely completed low-yield passes — cancelling the turn does
    // not un-observe them. Lift and re-engage run through their own paths.
    if cancelled_session.governor.is_paused() && !state.session.governor.is_paused() {
        state.session.governor = cancelled_session.governor.clone();
        app.set_compaction_paused(true);
    }
    app.reset_round();
    *cancel_token = None;
    *llm_tx = None;
    *llm_rx = None;
}

fn handle_tool_call_started(view: &mut RatatuiView) {
    // Tool calls detected — finalize the current content block and
    // transition to the next round. The visual transition
    // (`LlmState::ToolCall`) happens here so the user gets immediate
    // feedback that tool calls are coming.
    //
    // Don't freeze previews yet: at the first `ToolCallStart` the
    // provider may not have sent `argument_delta`, so freezing now
    // would commit empty args. Freezing happens later in
    // `ToolExecutionStarted`, after the stream ends and all previews
    // are fully populated.
    view.app_mut().finalize_streaming_zone_as_is();
    view.app_mut().increment_round();
    view.set_llm_state(LlmState::ToolCall);
}

fn handle_tool_call_preview(
    tool_call_id: String,
    name: String,
    args: serde_json::Value,
    view: &mut RatatuiView,
) {
    view.app_mut()
        .upsert_tool_preview_direct(tool_call_id, name, args);
}

fn handle_tool_execution_started(
    tool_call_id: String,
    name: String,
    args: serde_json::Value,
    view: &mut RatatuiView,
) {
    log::debug!(
        "ToolExecutionStarted: id={} name={} args={}",
        tool_call_id,
        name,
        args
    );
    // Freeze all previews now that the stream has ended and every
    // preview is fully populated. This promotes previews to
    // `TurnBlock::ToolCall` blocks keyed by the stream id, so
    // `freeze_tool_preview_by_name` can match by id and backfill
    // empty args from the execution args.
    view.app_mut().freeze_all_tool_previews();
    let has_live_turn = view.app().has_streaming_zone();
    if let Some(turn) = view.app_mut().live_turn_mut() {
        log::debug!(
            "freeze_tool_preview_by_name: live_turn exists={}, \
             blocks={}, previews={}, tool_call_id={}",
            has_live_turn,
            turn.blocks.len(),
            turn.tool_previews.len(),
            tool_call_id
        );
        turn.freeze_tool_preview_by_name(&tool_call_id, &name, &args);
    } else {
        log::warn!(
            "ToolExecutionStarted: no live_turn when trying to freeze \
             tool_call_id={} — block will be created by set_tool_result \
             with empty args (BUG-1)",
            tool_call_id
        );
    }
}

fn handle_tool_execution_finished(
    tool_call_id: String,
    result: String,
    is_error: bool,
    view: &mut RatatuiView,
) {
    log::debug!(
        "ToolExecutionFinished: id={} is_error={} result_len={}",
        tool_call_id,
        is_error,
        result.len()
    );
    if let Some(turn) = view.app_mut().live_turn_mut() {
        turn.set_tool_result(&tool_call_id, result, is_error);
    }
}

fn handle_provider_retry_started(
    attempt: u32,
    max_attempts: u32,
    delay_ms: u64,
    reason: String,
    view: &mut RatatuiView,
) {
    // Render transient retry warning in the status bar.
    let overlay = format!("⚠ Retry {attempt}/{max_attempts} in {delay_ms}ms: {reason}");
    view.set_status_overlay(Some(overlay));
}

fn handle_turn_metrics(prompt_tokens: u32, _completion_tokens: u32, view: &mut RatatuiView) {
    // Intermediate context update from a completed ReAct round.
    // The provider reports the real prompt_tokens for this round's
    // prompt (system + tools + all accumulated history). This keeps
    // the status bar current during multi-round tool loops instead
    // of freezing until the final Complete event.
    let max_tokens = view.app().status_bar().max_tokens;
    let percent = if max_tokens > 0 {
        ((prompt_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
    } else {
        0
    };
    view.update_status_tokens(prompt_tokens as usize, max_tokens, percent);
}

fn handle_compact_event(event: CompactEvent, view: &mut RatatuiView, state: &mut ReplState) {
    match event {
        CompactEvent::StreamToken(token) => {
            // Compaction is streaming — display as assistant streaming
            view.stream_token(&token);
        }
        CompactEvent::Info { message } => {
            // System-level info during compaction (chunk count, truncation warnings)
            // Display as a dim system message, separate from the streaming summary
            view.app_mut().add_message(ChatMessage::system(message));
        }
        CompactEvent::StreamDone { summary, range } => {
            // Compaction finished — apply the summary to the session
            let first_preserved = range.map(|(f, _)| f).unwrap_or(0);
            let last_preserved_start = range
                .map(|(_, l)| l)
                .unwrap_or(state.session.messages.len());
            let compacted_count = last_preserved_start - first_preserved;
            let preserved_last = state.session.messages.len() - last_preserved_start;

            state
                .session
                .set_compacted_summary_with_range(summary.clone(), range);

            // In the two-buffer model, the compaction summary was streamed into
            // the live turn via CompactStreamToken. Add the separator to committed
            // history BEFORE finalizing so the separator appears between progress
            // messages and the summary, not after it.
            view.app_mut().add_message(ChatMessage::separator());

            // Finalize the streaming content — just the summary, no artificial header/footer
            view.stream_done(&summary, None, None);

            // Add completion message after the summary, with a closing separator
            if compacted_count > 0 {
                view.app_mut().add_message(ChatMessage::system(format!(
                    "✓ Compacted {} messages (preserved {} first, {} last).",
                    compacted_count, first_preserved, preserved_last
                )));
                view.app_mut().add_message(ChatMessage::separator());
            }

            // Update status bar with new token count after compaction
            let used_tokens = state.session.history_real_tokens();
            let max_tokens = state.model_config.num_ctx as usize;
            let percent = if max_tokens > 0 {
                ((used_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
            } else {
                0
            };
            view.update_status_tokens(used_tokens, max_tokens, percent);
        }
    }
}

/// Internal variants of the compact events, used by `handle_llm_event` after
/// extracting the payload from `LlmEvent`.
enum CompactEvent {
    StreamToken(String),
    Info {
        message: String,
    },
    StreamDone {
        summary: String,
        range: Option<(usize, usize)>,
    },
}

pub fn handle_llm_event(
    llm_event: LlmEvent,
    state: &mut ReplState,
    view: &mut RatatuiView,
    fact_mirror: &mut Option<SharedFactTracker>,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    let terminal_event = matches!(
        llm_event,
        LlmEvent::Complete { .. } | LlmEvent::Cancelled { .. } | LlmEvent::CompactStreamDone { .. }
    );
    let error_event = matches!(llm_event, LlmEvent::Error(_));
    match llm_event {
        LlmEvent::ViewAction(action) => {
            apply_view_action(view, action);
        }
        LlmEvent::StreamToken(token) => {
            handle_stream_token(token, state, view);
        }
        LlmEvent::StreamThinking(thinking_token) => {
            handle_stream_thinking(thinking_token, view);
        }
        LlmEvent::StreamDone {
            content,
            thinking,
            metrics,
        } => {
            handle_stream_done(content, thinking, metrics, view);
        }
        LlmEvent::Complete {
            session,
            used_tokens,
            max_tokens,
            percent,
        } => {
            handle_complete(
                session,
                used_tokens,
                max_tokens,
                percent,
                state,
                view,
                cancel_token,
                llm_tx,
                llm_rx,
            );
        }
        LlmEvent::Error(error) => {
            handle_error(
                error,
                state,
                view,
                fact_mirror,
                cancel_token,
                llm_tx,
                llm_rx,
            );
        }
        LlmEvent::Cancelled { session } => {
            handle_cancelled(session, state, view.app_mut(), cancel_token, llm_tx, llm_rx);
        }
        // LlmEvent::InterToolText was removed — all ReAct turns stream and
        // post-tool text arrives via StreamToken/StreamThinking.
        LlmEvent::ToolCallStarted => {
            handle_tool_call_started(view);
        }
        LlmEvent::ToolCallPreview {
            tool_call_id,
            name,
            args,
        } => {
            handle_tool_call_preview(tool_call_id, name, args, view);
        }
        LlmEvent::ToolExecutionStarted {
            tool_call_id,
            name,
            args,
        } => {
            handle_tool_execution_started(tool_call_id, name, args, view);
        }
        LlmEvent::ToolExecutionFinished {
            tool_call_id,
            result,
            is_error,
        } => {
            handle_tool_execution_finished(tool_call_id, result, is_error, view);
        }
        LlmEvent::ProviderRetryStarted {
            attempt,
            max_attempts,
            delay_ms,
            reason,
        } => {
            handle_provider_retry_started(attempt, max_attempts, delay_ms, reason, view);
        }
        LlmEvent::ProviderRetryFinished { success, .. } => {
            // Clear the retry overlay once the retry finishes.
            if success {
                view.set_status_overlay(None);
            }
        }
        LlmEvent::TurnMetrics {
            prompt_tokens,
            completion_tokens,
        } => {
            handle_turn_metrics(prompt_tokens, completion_tokens, view);
        }
        LlmEvent::CompactStreamToken(token) => {
            handle_compact_event(CompactEvent::StreamToken(token), view, state);
        }
        LlmEvent::CompactInfo { message } => {
            handle_compact_event(CompactEvent::Info { message }, view, state);
        }
        LlmEvent::CompactStreamDone { summary, range } => {
            handle_compact_event(CompactEvent::StreamDone { summary, range }, view, state);
            // A manual compaction that ran to completion is exactly the
            // recovery that lifts the pause; a failed or cancelled one
            // never reaches this arm, so it cannot resume.
            state.session.governor.resume();
            view.set_compaction_paused(false);
            view.set_llm_state(LlmState::Idle);
            *cancel_token = None;
            *llm_tx = None;
            *llm_rx = None;
        }
    }

    // The mirror is per-turn: once a terminal event (or a non-pending Error)
    // has been consumed, a later exit must not merge the old turn's facts
    // into whatever session is live by then.
    if terminal_event || (error_event && !cancellation_delivery_pending(cancel_token, llm_rx)) {
        *fact_mirror = None;
    }
}

// ── View action and tool message helpers ─────────────────────────────────

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
/// Pre-tool content from the non-streaming fallback path arrives as
/// `ShowThinking` + `ShowMarkdown` view actions while the LLM is active.
/// When a streaming zone exists, the pre-tool text is already being
/// displayed via `StreamToken`/`StreamThinking`, so `apply_view_action`
/// skips those content messages to prevent duplication.
fn apply_view_action(view: &mut RatatuiView, action: ViewAction) {
    // In the two-buffer model, all streaming content goes through
    // StreamToken/StreamThinking events into LiveTurn. View actions that
    // carry content are only relevant for the terminal/CLI fallback path
    // or when the LLM is idle. When a live turn exists, skip content
    // view actions to avoid duplicating what is already being streamed.
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
            if has_streaming_zone {
                // Already streaming — content will be finalized by StreamDone.
                // Ignore to prevent duplication.
                let _ = (content, thinking);
            } else if llm_state != LlmState::Idle {
                // No live turn yet but LLM active — show as stable message
                // before any future streaming zone.
                if let Some(thinking_content) = thinking {
                    view.app_mut()
                        .add_message(ChatMessage::thinking(thinking_content));
                }
                view.app_mut()
                    .add_message(ChatMessage::assistant_markdown(content));
                view.render();
            } else {
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
        ViewAction::SetCompactionPaused(paused) => {
            view.set_compaction_paused(paused);
        }
        ViewAction::ShowCompactComplete {
            count,
            preserved_first,
            preserved_last,
        } => {
            view.show_compact_complete(count, preserved_first, preserved_last);
        }
        ViewAction::ShowMarkdown(content) => {
            if has_streaming_zone {
                // Already streaming — skip duplicate markdown content.
                let _ = content;
            } else if llm_state != LlmState::Idle {
                view.app_mut()
                    .add_message(ChatMessage::assistant_markdown(content));
                view.render();
            } else {
                view.show_markdown(&content);
            }
        }
        ViewAction::ShowThinking(thinking) => {
            if has_streaming_zone {
                // Already streaming — thinking will be finalized by StreamDone.
                let _ = thinking;
            } else if llm_state != LlmState::Idle {
                view.app_mut().add_message(ChatMessage::thinking(thinking));
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

// ── Task spawning ──────────────────────────────────────────────────────

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
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    cancel_token: &mut Option<CancellationToken>,
    fact_mirror: &mut Option<SharedFactTracker>,
) {
    // Create channels for LLM events
    let (task_llm_tx, new_llm_rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);
    *llm_tx = Some(task_llm_tx.clone());
    *llm_rx = Some(new_llm_rx);

    // Create cancellation token
    let token = CancellationToken::new();
    *cancel_token = Some(token.clone());

    // Per-turn fallback mirror for completed-tool facts. It stays with the
    // event loop while the task holds a clone, so exit paths can preserve
    // facts even if the task never sends its terminal event.
    let mirror: SharedFactTracker = std::sync::Arc::new(std::sync::Mutex::new(
        crate::chat::fact_tracker::SessionFactTracker::default(),
    ));
    *fact_mirror = Some(std::sync::Arc::clone(&mirror));

    // Create the channel view proxy
    let (view_tx, view_rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);

    // Forward view actions from the ChannelView sender to the LLM event sender.
    // This runs in a small forwarding task.
    let forward_tx = task_llm_tx.clone();
    tokio::spawn(async move {
        let mut rx = view_rx;
        while let Some(action) = rx.recv().await {
            if forward_tx.send(LlmEvent::ViewAction(action)).await.is_err() {
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
            let _ = task_llm_tx
                .send(LlmEvent::Cancelled {
                    // Pre-start: the clone was never touched by the turn, so
                    // its tracker carries only the live session's prior facts.
                    session: Box::new(task_state.session),
                })
                .await;
            return;
        }

        // The handler decides the terminal: `Complete` — including a
        // response preserved across a trailing compaction cancel — commits
        // this clone to the event loop; `Cancelled` (turn aborted before a
        // final response existed) discards it. The outcome is read from the
        // terminal, never re-derived from the token, because a cancellation
        // after the response is a completed turn.
        let turn_terminal = super::repl::handle_user_message_stream(
            &line_owned,
            &mut task_state,
            &mut channel_view as &mut dyn ChatView,
            task_llm_tx.clone(),
            token.clone(),
            mirror,
        )
        .await;

        // Error and Cancelled are sent sequentially by this task on one channel.
        // handle_error therefore defers cleanup when this token is cancelled.
        if turn_terminal == super::repl::TurnTerminal::Cancelled {
            let _ = task_llm_tx
                .send(LlmEvent::Cancelled {
                    // Core flushed the coordinator facts into the task's
                    // session clone before returning CANCELLED_BY_USER, so
                    // this clone is the turn's final fact tracker.
                    session: Box::new(task_state.session),
                })
                .await;
            return;
        }

        // Send the updated session and token counts back.
        // Bug B fix: when the server didn't return valid prompt_tokens (0 or
        // None — common in the streaming path), history_real_tokens falls
        // back to an estimate that excludes system prompt + tool definitions.
        // We add the overhead so the status bar shows a realistic total.
        let system_prompt = super::continuation::build_pre_tool_prompt(&task_state);
        let system_tokens =
            crate::tokens::estimate_tokens(&system_prompt) + crate::tokens::MESSAGE_OVERHEAD;
        let tools_tokens = if task_state.tools_active {
            let tool_count = crate::tools::get_available_tool_names(&task_state.settings).len();
            tool_count * crate::tokens::TOKENS_PER_TOOL
        } else {
            0
        };
        let used_tokens = task_state
            .session
            .history_real_tokens_with_overhead(system_tokens + tools_tokens);
        let max_tokens = task_state.model_config.num_ctx as usize;
        let percent = if max_tokens > 0 {
            ((used_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };

        let _ = task_llm_tx
            .send(LlmEvent::Complete {
                session: Box::new(task_state.session),
                used_tokens,
                max_tokens,
                percent,
            })
            .await;
    });
}

/// Spawn a compaction task in the background with streaming summary display.
///
/// Analogous to `spawn_llm_task` but for `/compact`. Creates a dedicated
/// `llm_tx`/`llm_rx` channel pair and registers the same turn
/// `cancel_token` the LLM tasks use, so Ctrl+C during compaction cancels
/// it the same way. The background task calls `compact_conversation()`
/// which streams `CompactStreamToken` events. On completion, sends
/// `CompactStreamDone` with the summary and range (or `Error` on
/// failure). A cancelled compaction sends the terminal `Cancelled` event
/// with the untouched clone: the summary never reaches the session.
fn spawn_compact_task(
    state: &mut ReplState,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    cancel_token: &mut Option<CancellationToken>,
) {
    // Precondition: must not be called while an LLM task is already running,
    // as this would clobber the active channel. Callers must guard against this.
    debug_assert!(
        llm_tx.is_none(),
        "spawn_compact_task called while LLM task is already running — channel would be clobbered"
    );
    // Create channels for compaction events
    let (task_llm_tx, new_llm_rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);
    *llm_tx = Some(task_llm_tx.clone());
    *llm_rx = Some(new_llm_rx);

    // Compaction shares the LLM tasks' cancellation channel: Ctrl+C while
    // the summary is being written stops the model calls and discards the
    // candidate summary.
    let token = CancellationToken::new();
    *cancel_token = Some(token.clone());

    // Clone the state for the compaction task
    let task_state = state.clone();

    tokio::spawn(async move {
        match super::core::compact_conversation(
            &task_state.provider,
            &task_state.model_config,
            &task_state.session,
            &task_state.settings,
            task_state.agents_md.as_deref(),
            task_llm_tx.clone(),
            Some(token.clone()),
        )
        .await
        {
            Ok((summary, range)) => {
                let _ = task_llm_tx
                    .send(LlmEvent::CompactStreamDone { summary, range })
                    .await;
            }
            Err(e) => {
                // The cancelled path keeps the summary out of the session:
                // the terminal Cancelled event discards this clone whole.
                if e.is_cancelled_or_token(&token) {
                    let _ = task_llm_tx
                        .send(LlmEvent::Cancelled {
                            session: Box::new(task_state.session),
                        })
                        .await;
                } else {
                    let _ = task_llm_tx.send(LlmEvent::Error(e.to_string())).await;
                }
            }
        }
    });
}

impl super::compaction::CompactionFailure {
    /// Whether this failure is cancellation — checked against the owning
    /// token, never against error text. A Model failure still counts when
    /// the token was cancelled mid-call: the provider error that raced the
    /// interrupt must not replace the terminal Cancelled event.
    fn is_cancelled_or_token(&self, token: &CancellationToken) -> bool {
        self.is_cancelled() || token.is_cancelled()
    }
}

// ── Mouse event handler ────────────────────────────────────────────────

/// Number of lines to scroll per mouse wheel tick.
const MOUSE_SCROLL_LINES: u16 = 3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::app::App;
    use crate::chat::fact_tracker::{SessionFactTracker, normalize_path};
    use crate::chat::repl_state::ReplStateBuilder;
    use crate::chat::session::ChatSession;
    use crate::chat::tui::markdown::MarkdownTheme;
    use crate::config::ModelConfig;
    use crate::provider::OpenAICompatibleProvider;
    use crate::settings::Settings;

    fn test_app() -> App {
        let (app, _embedding_tx, _async_message_tx) =
            App::with_embedding_channel(MarkdownTheme::Dark, vec!["test-model".to_string()]);
        app
    }

    /// A live session as the event loop would hold it, with one committed
    /// user turn so history replacement can be observed.
    fn live_session(model: &str) -> ChatSession {
        let mut session = ChatSession::new(model.to_string(), None, true);
        session.add_user_message("keep me".to_string());
        session
    }

    /// One turn's worth of tool facts, the same shape the coordinator
    /// records while the LLM executes tools.
    fn sample_turn_facts() -> SessionFactTracker {
        let mut turn = SessionFactTracker::default();
        turn.record_tool(
            "write_file",
            r#"{"path":"src/real.rs"}"#,
            "Successfully created",
            false,
        );
        turn.record_tool(
            "read_file",
            r#"{"path":"src/read_only.rs"}"#,
            "1|hello",
            false,
        );
        turn.record_tool(
            "run_command",
            r#"{"command_line":"make lint"}"#,
            "ok",
            false,
        );
        turn
    }

    fn test_repl_state(session: ChatSession) -> ReplState {
        ReplStateBuilder::new()
            .session(session)
            .model_config(ModelConfig::get_default())
            .capabilities(ModelCapabilities::default())
            .provider(OpenAICompatibleProvider::new_local(
                "http://localhost".to_string(),
                11434,
            ))
            .settings(Settings::default())
            .build()
            .expect("valid test ReplState")
    }

    type EventChannel = (
        tokio::sync::mpsc::Sender<LlmEvent>,
        tokio::sync::mpsc::Receiver<LlmEvent>,
    );

    fn test_channel() -> EventChannel {
        tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY)
    }

    fn cancelled_token() -> CancellationToken {
        let token = CancellationToken::new();
        token.cancel();
        token
    }

    #[test]
    fn error_cleanup_waits_for_the_cancelled_event() {
        let mut token = Some(cancelled_token());
        let (tx, rx) = test_channel();
        let mut tx = Some(tx);
        let mut rx = Some(rx);

        clear_task_after_error(&mut token, &mut tx, &mut rx);

        assert!(token.is_some() && tx.is_some() && rx.is_some());
        assert!(cancellation_delivery_pending(&token, &rx));
    }

    #[test]
    fn error_without_cancellation_still_clears_the_task() {
        let mut token = Some(CancellationToken::new());
        let (tx, rx) = test_channel();
        let mut tx = Some(tx);
        let mut rx = Some(rx);

        clear_task_after_error(&mut token, &mut tx, &mut rx);

        assert!(token.is_none() && tx.is_none() && rx.is_none());
    }

    #[test]
    fn fatal_error_folds_the_mirror_before_cleanup() {
        let source = include_str!("event_loop.rs");
        let start = source
            .find("fn handle_error(")
            .expect("handle_error exists");
        let end = source[start..]
            .find("fn merge_cancelled_facts")
            .map(|offset| start + offset)
            .expect("merge_cancelled_facts follows handle_error");
        let body = &source[start..end];

        let merge = body
            .find("merge_fact_mirror")
            .expect("a fatal error must fold completed-tool facts into the live session");
        let cleanup = body
            .find("clear_task_after_error")
            .expect("the error path still clears the task");
        assert!(
            merge < cleanup,
            "facts must be folded while the mirror is still available"
        );
    }

    #[test]
    fn handle_error_uses_the_cancellation_aware_cleanup() {
        let source = include_str!("event_loop.rs");
        let start = source
            .find("fn handle_error(")
            .expect("handle_error exists");
        let end = source[start..]
            .find("fn merge_cancelled_facts")
            .map(|offset| start + offset)
            .expect("handle_error body ends before cancellation merge");
        let body = &source[start..end];
        let cleanup = ["clear_task_after_error", "(cancel_token, llm_tx, llm_rx);"].concat();
        assert!(
            body.contains(&cleanup),
            "Error must not bypass the cleanup that preserves a pending Cancelled event"
        );
    }

    /// Covers both a cancelled tool turn and the pre-start case where the clone
    /// already carries prior session facts: all of them merge into the live
    /// session before terminal cleanup.
    #[test]
    fn cancelled_session_facts_merge_into_the_live_session() {
        let cancelled = {
            let mut session = ChatSession::new("test-model".to_string(), None, true);
            session.add_user_message("stale question".to_string());
            session.merge_fact_tracker(&sample_turn_facts());
            session
        };

        let mut state = test_repl_state(live_session("live-model"));
        let mut app = test_app();
        app.increment_round();
        let mut cancel_token = Some(cancelled_token());
        let (tx, rx) = test_channel();
        let mut llm_tx = Some(tx);
        let mut llm_rx = Some(rx);

        handle_cancelled(
            Box::new(cancelled),
            &mut state,
            &mut app,
            &mut cancel_token,
            &mut llm_tx,
            &mut llm_rx,
        );

        let facts = &state.session.fact_tracker;
        assert_eq!(
            facts.modified_files(),
            vec![normalize_path("src/real.rs")],
            "the modified path must land in the live session's tracker"
        );
        assert_eq!(
            facts.read_files(),
            vec![normalize_path("src/read_only.rs")],
            "the read-only path must land in the live session's tracker"
        );
        let run = facts
            .last_run()
            .expect("the last-run command must be preserved");
        assert_eq!(run.command, "make lint");
        assert!(run.passed);

        // Terminal cleanup: no pending-delivery state and a reset round.
        assert!(cancel_token.is_none());
        assert!(llm_tx.is_none());
        assert!(llm_rx.is_none());
        assert!(
            !cancellation_delivery_pending(&cancel_token, &llm_rx),
            "after the terminal event the guard must read as normal behavior again"
        );
        assert_eq!(app.current_round(), 0);
    }

    /// The cancelled turn's session must only donate its fact tracker:
    /// its own history and model are the task's clone, not the live truth.
    #[test]
    fn cancelled_session_must_not_replace_live_messages_or_model() {
        let cancelled = {
            let mut session = ChatSession::new("cancelled-model".to_string(), None, true);
            session.add_user_message("first stale".to_string());
            session.add_user_message("second stale".to_string());
            session
        };

        let live = live_session("live-model");
        let live_message_count = live.messages.len();
        let mut state = test_repl_state(live);
        let mut app = test_app();
        let mut cancel_token = Some(cancelled_token());
        let (tx, rx) = test_channel();
        let mut llm_tx = Some(tx);
        let mut llm_rx = Some(rx);

        handle_cancelled(
            Box::new(cancelled),
            &mut state,
            &mut app,
            &mut cancel_token,
            &mut llm_tx,
            &mut llm_rx,
        );

        assert_eq!(
            state.session.model, "live-model",
            "the live session's model must be untouched"
        );
        assert_eq!(
            state.session.messages.len(),
            live_message_count,
            "the live session's history must not be replaced by the cancelled copy"
        );
        assert_eq!(
            state
                .session
                .messages
                .first()
                .expect("live history kept")
                .content,
            "keep me",
            "the stale cancelled messages must not appear in the live history"
        );
    }

    /// A pre-start cancellation carries a session whose tracker was never
    /// touched; merging it must not disturb facts the live session already
    /// holds.
    #[test]
    fn pre_start_cancellation_with_empty_tracker_leaves_live_facts_untouched() {
        let mut live = live_session("live-model");
        live.merge_fact_tracker(&sample_turn_facts());
        let modified_before = live.fact_tracker.modified_files();
        let run_before = live.fact_tracker.last_run().map(|r| r.command.clone());

        let cancelled = ChatSession::new("test-model".to_string(), None, true);
        assert!(
            cancelled.fact_tracker.modified_files().is_empty()
                && cancelled.fact_tracker.read_files().is_empty()
                && cancelled.fact_tracker.last_run().is_none()
        );

        let mut state = test_repl_state(live);
        let mut app = test_app();
        let mut cancel_token = Some(cancelled_token());
        let (tx, rx) = test_channel();
        let mut llm_tx = Some(tx);
        let mut llm_rx = Some(rx);

        handle_cancelled(
            Box::new(cancelled),
            &mut state,
            &mut app,
            &mut cancel_token,
            &mut llm_tx,
            &mut llm_rx,
        );

        assert_eq!(
            state.session.fact_tracker.modified_files(),
            modified_before,
            "an empty cancelled tracker must not drop live modified paths"
        );
        assert_eq!(
            state
                .session
                .fact_tracker
                .last_run()
                .map(|r| r.command.clone()),
            run_before,
            "an empty cancelled tracker must not drop the live last-run record"
        );
    }

    /// Ctrl+C must cancel the token and release the event loop's own Sender
    /// while keeping the receiver open. The task and its forwarding task still
    /// own senders, so terminal delivery is unaffected — but once the task
    /// dies, no loop-held Sender can keep the channel artificially open.
    #[test]
    fn interrupt_cancels_the_token_and_releases_the_loop_sender() {
        let state = test_repl_state(live_session("live-model"));
        let mut app = test_app();
        app.set_llm_state(LlmState::Thinking);
        app.increment_round();
        let mut cancel_token = Some(CancellationToken::new());
        let (tx, rx) = test_channel();
        let mut llm_tx = Some(tx);
        let mut llm_rx = Some(rx);

        handle_interrupt(
            &state,
            &mut app,
            &mut cancel_token,
            &mut llm_tx,
            &mut llm_rx,
        );

        let token = cancel_token
            .as_ref()
            .expect("the token stays as the cancellation-in-progress marker");
        assert!(token.is_cancelled());
        assert!(
            llm_tx.is_none(),
            "the loop must drop its own Sender so producer death can close the channel"
        );
        assert!(
            llm_rx.is_some(),
            "dropping the receiver would discard the terminal Cancelled event"
        );

        // Cancellation UX is unchanged: idle status, round reset, marker
        // message.
        assert_eq!(app.llm_state(), LlmState::Idle);
        assert_eq!(app.current_round(), 0);
        assert!(
            app.render_messages()
                .iter()
                .any(|m| m.content == "Cancelled."),
            "the interrupt must keep showing the Cancelled marker"
        );
    }

    /// With the loop Sender released, the only remaining producers are the
    /// task and its forwarding task. If both die without a terminal event,
    /// `recv()` must observe closure instead of pending forever.
    #[tokio::test]
    async fn producer_death_closes_the_receiver_after_interrupt_releases_the_loop_sender() {
        let state = test_repl_state(live_session("live-model"));
        let mut app = test_app();
        let mut cancel_token = Some(CancellationToken::new());
        let (task_tx, rx) = tokio::sync::mpsc::channel::<LlmEvent>(1);
        let mut llm_tx = Some(task_tx.clone());
        let mut llm_rx = Some(rx);

        handle_interrupt(
            &state,
            &mut app,
            &mut cancel_token,
            &mut llm_tx,
            &mut llm_rx,
        );

        // Simulate the spawned task and its forwarding task dying: every
        // remaining producer Sender is gone.
        drop(task_tx);

        let event = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            llm_rx.as_mut().expect("receiver stays open").recv(),
        )
        .await
        .expect("a closed channel must resolve, not hang");
        assert!(
            event.is_none(),
            "with every producer gone the receiver must observe closure"
        );
    }

    /// Ctrl+C without an LLM task and without compaction must stay a no-op:
    /// no token is fabricated and no channel is created or cleared.
    #[test]
    fn interrupt_without_a_task_touches_nothing() {
        let state = test_repl_state(live_session("live-model"));
        let mut app = test_app();
        let mut cancel_token: Option<CancellationToken> = None;
        let (tx, rx) = test_channel();
        let mut llm_tx = Some(tx);
        let mut llm_rx = Some(rx);

        handle_interrupt(
            &state,
            &mut app,
            &mut cancel_token,
            &mut llm_tx,
            &mut llm_rx,
        );

        assert!(cancel_token.is_none());
        assert!(llm_tx.is_some());
        assert!(llm_rx.is_some());
        assert_eq!(app.llm_state(), LlmState::Idle);
        assert!(!cancellation_delivery_pending(&cancel_token, &llm_rx));
    }

    /// The pending-delivery window is exactly: a task token that is
    /// cancelled AND a receiver that is still alive. Anything else reads as
    /// normal operation.
    #[test]
    fn pending_delivery_needs_a_cancelled_token_and_a_live_receiver() {
        let cancelled = cancelled_token();
        let fresh = CancellationToken::new();
        let (tx, rx) = test_channel();
        drop(tx);
        let rx = Some(rx);
        let no_rx: Option<tokio::sync::mpsc::Receiver<LlmEvent>> = None;

        assert!(cancellation_delivery_pending(&Some(cancelled.clone()), &rx));
        assert!(
            !cancellation_delivery_pending(&Some(fresh), &rx),
            "an uncancelled task is streaming, not pending delivery"
        );
        assert!(
            !cancellation_delivery_pending(&Some(cancelled.clone()), &no_rx),
            "without a receiver there is no terminal event left to protect"
        );
        assert!(
            !cancellation_delivery_pending(&None, &rx),
            "no token means no task is in flight"
        );
        assert!(
            !cancellation_delivery_pending(&None, &no_rx),
            "the clean state must read as normal behavior"
        );
    }

    /// The fallback mirror holds only completed-tool facts. Merging it into
    /// the live session must add those facts without touching anything else,
    /// and a missing mirror must leave the session untouched.
    #[test]
    fn fact_mirror_merges_completed_turn_facts() {
        let mirror: crate::chat::fact_tracker::SharedFactTracker =
            std::sync::Arc::new(std::sync::Mutex::new(sample_turn_facts()));
        let mut state = test_repl_state(live_session("live-model"));
        let before = state.session.messages.len();

        merge_fact_mirror(&mut state.session, Some(&mirror));

        let facts = &state.session.fact_tracker;
        assert_eq!(
            facts.modified_files(),
            vec![normalize_path("src/real.rs")],
            "the mirror's modified paths must reach the live session"
        );
        assert_eq!(
            facts.read_files(),
            vec![normalize_path("src/read_only.rs")],
            "the mirror's read paths must reach the live session"
        );
        assert_eq!(facts.last_run().expect("mirrored run").command, "make lint");
        assert_eq!(
            state.session.messages.len(),
            before,
            "the mirror donates facts only, never messages"
        );

        merge_fact_mirror(&mut state.session, None);
        assert_eq!(
            state.session.fact_tracker.modified_files(),
            vec![normalize_path("src/real.rs")],
            "no mirror means no fallback merge, not a cleared tracker"
        );
    }

    /// The recovery message must not claim fact preservation when no mirror
    /// exists (e.g. a compact-task channel closing has no tool-fact mirror).
    #[test]
    fn channel_closed_message_does_not_claim_facts_without_a_mirror() {
        assert_eq!(
            channel_closed_system_message(false),
            "LLM task ended unexpectedly.",
            "without a mirror there is nothing to claim preservation for"
        );
        assert_eq!(
            channel_closed_system_message(true),
            "LLM task ended unexpectedly. Completed tool facts were preserved."
        );

        let source = include_str!("event_loop.rs");
        let marker = ["ChatMessage::system(", "channel_closed_system_message("].concat();
        assert!(
            source.contains(&marker),
            "handle_llm_channel_closed must derive its message from the actual mirror state"
        );
    }

    /// Cancellation is resolved at the core boundary, before the success
    /// path can send StreamDone or start auto-compaction. The behavior lives
    /// in `provider_success_cancellation_tests`; this guard pins the
    /// production call site so the helper cannot become test-only.
    #[test]
    fn cancelled_provider_success_never_enters_the_success_path() {
        let source = include_str!("core.rs");
        let marker = ["Ok(response) => break ", "resolve_provider_success("].concat();
        let call = source
            .find(&marker)
            .expect("the provider success arm must resolve through the cancellation boundary");
        let flush = source[call..]
            .find("flush_turn_facts(session, &coordinator);")
            .map(|offset| call + offset)
            .expect("the fact flush follows the provider loop");
        assert!(
            call < flush,
            "cancellation must be resolved before the success path, while the flush still runs afterward"
        );
    }

    /// A pending-delivery quit must not exit before the cancelled turn's
    /// completed facts are folded in, and a closed channel must be handled
    /// instead of silently disabling the LLM select arm. A `RatatuiView`
    /// cannot be built under the test harness, so both wiring invariants are
    /// pinned as source guards.
    #[test]
    fn eof_merges_completed_facts_before_saving_and_waits_when_pending() {
        let source = include_str!("event_loop.rs");
        let start = source
            .find("pub async fn handle_eof")
            .expect("handle_eof exists");
        let end = source[start..]
            .find("pub fn handle_llm_event")
            .map(|offset| start + offset)
            .expect("handle_llm_event follows handle_eof");
        let body = &source[start..end];

        let merge = body
            .find("merge_fact_mirror")
            .expect("EOF must fold the per-turn mirror into the live session");
        let persist = body
            .find("persist_live_session")
            .expect("EOF persists through the shared helper");
        assert!(
            merge < persist,
            "completed-tool facts must be merged before persistence"
        );

        let helper = source
            .find("fn persist_live_session")
            .expect("persist_live_session exists");
        let helper_body = &source[helper..helper + 500];
        assert!(
            helper_body.contains("save_history") && helper_body.contains("save_sqlite"),
            "the persistence helper must save both terminal history and the session"
        );
        assert!(
            body.contains("LoopAction::QuitAfterDelivery")
                && body.contains("cancellation_delivery_pending"),
            "a pending cancellation must request a bounded exit instead of exiting immediately"
        );
    }

    #[test]
    fn the_llm_select_arm_survives_a_closed_channel() {
        let source = include_str!("repl_tui.rs");
        assert!(
            !source.contains("Some(llm_event) = async"),
            "matching Some(event) silently disables the arm when every sender drops"
        );
        let arm = source
            .find("llm_event = async")
            .expect("the LLM branch binds the Option returned by recv()");
        let branch = &source[arm..arm + 2_000];
        assert!(
            branch.contains("None =>"),
            "recv() returning None must have an explicit handler"
        );
        assert!(
            branch.contains("handle_llm_channel_closed"),
            "a closed channel must recover the turn's facts and clear the task state"
        );
    }

    #[test]
    fn pending_delivery_keeps_quit_and_new_as_fact_preserving_exits() {
        let source = include_str!("event_loop.rs");
        let start = source
            .find("pub async fn handle_key_line")
            .expect("handle_key_line exists");
        let end = source[start..]
            .find("pub fn handle_interrupt")
            .map(|offset| start + offset)
            .expect("handle_interrupt follows handle_key_line");
        let body = &source[start..end];

        assert!(
            body.contains("ChatCommand::Quit"),
            "/quit during pending delivery needs its own fact-preserving path"
        );
        assert!(
            body.contains("LoopAction::QuitAfterDelivery"),
            "the pending quit must be bounded instead of immediate"
        );
        assert!(
            body.contains("ChatCommand::New"),
            "/new during pending delivery needs its own fact-preserving path"
        );
    }

    #[test]
    fn pre_turn_compaction_does_not_launch_a_new_request_after_cancellation() {
        let source = include_str!("repl.rs");
        let compaction = source
            .find("    check_and_compact_before_tool(")
            .expect("the pre-turn compaction call exists");
        let send = source
            .find("match send_message_stream(")
            .expect("the provider request follows the pre-turn compaction");
        let between = &source[compaction..send];
        assert!(
            between.contains("cancel_token.is_cancelled") && compaction < send,
            "a cancellation received during pre-turn compaction must stop before another provider request"
        );
    }

    #[test]
    fn a_stale_quit_deadline_cannot_exit_a_recovered_session() {
        let source = include_str!("repl_tui.rs");
        let watchdog = source
            .find("Bounded exit while cancellation delivery is pending")
            .expect("the bounded-exit branch exists");
        let spinner = source[watchdog..]
            .find("Spinner tick")
            .map(|offset| watchdog + offset)
            .expect("the spinner branch follows the bounded-exit branch");
        let body = &source[watchdog..spinner];
        assert!(
            body.contains("cancellation_delivery_pending") && body.contains("quit_deadline = None"),
            "the exit deadline must be re-checked against the pending window and cleared once it is gone"
        );

        let line_branch = source
            .find("Some(InputResult::Line(line))")
            .expect("the line branch exists");
        let interrupt = source[line_branch..]
            .find("Some(InputResult::Interrupted)")
            .map(|offset| line_branch + offset)
            .expect("the interrupt branch follows the line branch");
        let line_body = &source[line_branch..interrupt];
        assert!(
            line_body.contains("if quit_deadline.is_some()")
                && line_body.contains("LoopAction::QuitAfterDelivery"),
            "a second quit request must force the mirror-backed exit instead of restarting the deadline"
        );
    }

    /// Construction guard for the reentrancy seam: the pending-delivery
    /// refusal must sit BEFORE the user line is added to the view and
    /// BEFORE slash commands are dispatched, and it must return instead of
    /// falling through. A `RatatuiView` cannot be built under the test
    /// harness (no TTY), so the placement — and with it "refused without
    /// spawning/replacing the channel" — is pinned here.
    #[test]
    fn pending_delivery_refusal_precedes_view_mutation_and_dispatch() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/chat/event_loop.rs"
        ))
        .expect("event_loop.rs is readable");
        let start = source
            .find("pub async fn handle_key_line")
            .expect("handle_key_line exists in event_loop.rs");
        let end = source[start..]
            .find("pub fn handle_interrupt")
            .map(|offset| start + offset)
            .expect("handle_interrupt follows handle_key_line in event_loop.rs");
        let body = &source[start..end];

        let guard = body
            .find("cancellation_delivery_pending")
            .unwrap_or_else(|| {
                panic!(
                    "handle_key_line must consult the pending-delivery guard: a submission \
                 while the terminal Cancelled event is in flight would replace the \
                 channel and drop the session it carries"
                )
            });
        let add_user = body
            .find("add_message(ChatMessage::user")
            .expect("handle_key_line adds the user line to the view");
        // Parsing before the guard is only a read; dispatching is the state
        // change the guard exists to prevent, so pin against the real call.
        let dispatch = body
            .find("command_handlers::handle_command")
            .expect("handle_key_line dispatches slash commands via handle_command");

        assert!(
            guard < add_user,
            "the pending-delivery guard must run BEFORE the user line is added to the view"
        );
        assert!(
            guard < dispatch,
            "the pending-delivery guard must run BEFORE slash commands are dispatched"
        );

        let sender_clone = body
            .find("cmd_llm_tx")
            .expect("handle_key_line clones the event sender for command handlers");
        assert!(
            guard < sender_clone,
            "no command handler may obtain a usable event Sender after cancellation"
        );

        let refusal_block = &body[guard..add_user];
        assert!(
            refusal_block.contains("return LoopAction::Continue"),
            "the pending-delivery refusal must return instead of dispatching commands or spawning a task (would replace the channel)"
        );
    }
}

/// Handle mouse events for the TUI chat REPL.
///
/// Currently supports:
/// - Mouse wheel scroll up/down in the chat area (3 lines per tick)
/// - Left-click + drag to select text in the chat area
/// - Clicking in chat area clears textarea selection (bidirectional mutual exclusion)
/// - Clicking outside chat area clears chat selection
pub fn handle_mouse_event(mouse: MouseEvent, view: &mut RatatuiView) {
    let app = view.app_mut();
    let chat_area = app.chat_area_rect_cache();
    let scroll_from_top = app.scroll_from_top_cache();

    match mouse.kind {
        crossterm::event::MouseEventKind::Down(MouseButton::Left) => {
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
        crossterm::event::MouseEventKind::Drag(MouseButton::Left) => {
            // Extend selection if we're in selection mode
            if app.chat_selection().is_dragging()
                && let Some((visual_line, char_offset)) =
                    mouse_to_visual_pos(mouse.column, mouse.row, chat_area, scroll_from_top)
            {
                app.chat_selection_mut().extend(visual_line, char_offset);
            }
        }
        crossterm::event::MouseEventKind::Up(MouseButton::Left) => {
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
        crossterm::event::MouseEventKind::ScrollUp => {
            app.scroll_state_mut().scroll_up(MOUSE_SCROLL_LINES);
        }
        crossterm::event::MouseEventKind::ScrollDown => {
            app.scroll_state_mut().scroll_down(MOUSE_SCROLL_LINES);
        }
        _ => {}
    }
}
