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
use crate::capabilities::ModelCapabilities;

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
}

// ── Key input handler ─────────────────────────────────────────────────

/// Process a submitted line of user input.
///
/// Handles slash commands (starting with `/`) by dispatching to the
/// appropriate handler, or sends the line as a user message to the LLM.
///
/// Returns `LoopAction::Quit` if the user requested exit, `Continue` otherwise.
pub async fn handle_key_line(
    line: &str,
    state: &mut ReplState,
    view: &mut RatatuiView,
    capabilities: &ModelCapabilities,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
    cancel_token: &mut Option<CancellationToken>,
) -> LoopAction {
    let line = line.trim().to_string();
    if line.is_empty() {
        return LoopAction::Continue;
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
                        spawn_compact_task(state, llm_tx, llm_rx);
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

                if outputs.iter().any(|o| matches!(o, CommandOutput::Quit)) {
                    let _ = view.app_mut().save_history();
                    if !state.session.anonymous {
                        let _ = state.session.save_sqlite();
                    }
                    return LoopAction::Quit;
                }
                return LoopAction::Continue;
            }
            Some(Err(e)) => {
                view.show_error(&e.to_string());
                return LoopAction::Continue;
            }
            None => {}
        }
    }

    // Send as user message to LLM (non-blocking)
    // Reset round counter — each user prompt starts a fresh LLM cycle.
    view.app_mut().reset_round();
    view.set_llm_state(LlmState::Thinking);
    spawn_llm_task(&line, state, llm_tx, llm_rx, cancel_token);
    LoopAction::Continue
}

/// Handle Ctrl+C (interrupt) — cancel running LLM task or show warning.
///
/// If an LLM task is running, cancels it via the cancellation token.
/// If compaction is running, shows a warning (compaction is not cancellable).
pub fn handle_interrupt(
    _state: &ReplState,
    view: &mut RatatuiView,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    if let Some(token) = cancel_token.take() {
        token.cancel();
        view.app_mut()
            .add_message(ChatMessage::system("Cancelled.".to_string()));
        view.app_mut().reset_round();
        view.set_llm_state(LlmState::Idle);
        *llm_tx = None;
        *llm_rx = None;
    } else if view.app_mut().llm_state() == LlmState::Compacting {
        // Compaction is not cancellable — ignore Ctrl+C
        view.app_mut().add_message(ChatMessage::system(
            "Compaction in progress, please wait...".to_string(),
        ));
    }
}

/// Handle Ctrl+D (quit) — save session.
///
/// Embedding recovery is NOT performed on exit — it runs on next startup
/// via the background recovery pipeline (see `repl_tui.rs`). This avoids
/// blocking the application exit while hundreds of embeddings are generated.
///
/// Handle Ctrl+D (EOF) — save session and exit.
///
/// No embedding flush on exit — see `handle_quit()` for rationale.
/// Missing embeddings are recovered by the background pipeline on next startup.
///
/// Note: The caller is responsible for calling `view.restore()` after
/// this function returns, since `restore()` consumes `self`.
pub async fn handle_eof(state: &mut ReplState, view: &mut RatatuiView) {
    let _ = view.app_mut().save_history();
    if !state.session.anonymous {
        let _ = state.session.save_sqlite();
    }
}

// ── LLM event handler ─────────────────────────────────────────────────

/// Process an LLM event (streaming token, completion, error, etc.).
///
/// Updates the view and state based on the event type.
/// Clears LLM task state (cancel_token, llm_tx, llm_rx) on terminal events
/// (Complete, Error, Cancelled, CompactStreamDone).
pub fn handle_llm_event(
    llm_event: LlmEvent,
    state: &mut ReplState,
    view: &mut RatatuiView,
    cancel_token: &mut Option<CancellationToken>,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
) {
    match llm_event {
        LlmEvent::ViewAction(action) => {
            apply_view_action(view, action);
        }
        LlmEvent::StreamToken(token) => {
            // Append token to the current live turn
            view.app_mut().append_stream_token(&token);

            // W2 #121: throttle status bar updates during streaming.
            let bucket = state.status_bar_bucket();
            if bucket != state.last_status_token_bucket {
                state.last_status_token_bucket = bucket;
                if let Some((used, max, pct)) = state.estimate_status_bar() {
                    view.update_status_tokens(used, max, pct);
                }
            }
        }
        LlmEvent::StreamThinking(thinking_token) => {
            // Append thinking token to the current live turn
            view.app_mut().append_stream_thinking(&thinking_token);
        }
        LlmEvent::StreamDone {
            content,
            thinking,
            metrics,
        } => {
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
        LlmEvent::Complete {
            session,
            used_tokens,
            max_tokens,
            percent,
        } => {
            // Update the session with the one from the LLM task
            state.session = *session;
            view.update_status_tokens(used_tokens, max_tokens, percent);
            view.set_llm_state(LlmState::Idle);
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
        LlmEvent::Error(error) => {
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
            *cancel_token = None;
            *llm_tx = None;
            *llm_rx = None;
        }
        LlmEvent::Cancelled => {
            // LLM was cancelled — already handled by Ctrl+C branch
            view.app_mut().reset_round();
            *cancel_token = None;
            *llm_tx = None;
            *llm_rx = None;
        }
        // LlmEvent::InterToolText removed in W2 #122 — all ReAct turns stream
        // and post-tool text arrives via StreamToken/StreamThinking.
        LlmEvent::ToolCallStarted => {
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
        LlmEvent::ToolCallPreview {
            tool_call_id,
            name,
            args,
        } => {
            view.app_mut()
                .upsert_tool_preview_direct(tool_call_id, name, args);
        }
        LlmEvent::ToolExecutionStarted {
            tool_call_id,
            name,
            args,
        } => {
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
        LlmEvent::ToolExecutionFinished {
            tool_call_id,
            result,
            is_error,
        } => {
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
        LlmEvent::ProviderRetryStarted {
            attempt,
            max_attempts,
            delay_ms,
            reason,
        } => {
            // W2 #122 Fase 6c: render transient retry warning in the status bar.
            let overlay = format!("⚠ Retry {attempt}/{max_attempts} in {delay_ms}ms: {reason}");
            view.set_status_overlay(Some(overlay));
        }
        LlmEvent::ProviderRetryFinished { success, .. } => {
            // Clear the retry overlay once the retry finishes.
            if success {
                view.set_status_overlay(None);
            }
        }
        LlmEvent::TurnMetrics {
            prompt_tokens,
            completion_tokens: _,
        } => {
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
        LlmEvent::CompactStreamToken(token) => {
            // Compaction is streaming — display as assistant streaming
            view.stream_token(&token);
        }
        LlmEvent::CompactInfo { message } => {
            // System-level info during compaction (chunk count, truncation warnings)
            // Display as a dim system message, separate from the streaming summary
            view.app_mut().add_message(ChatMessage::system(message));
        }
        LlmEvent::CompactStreamDone { summary, range } => {
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

            if !state.session.anonymous {
                let _ = state.session.save_sqlite();
                if let Some(db) = state.session.db.as_ref() {
                    let _ = db.clear_conversation_prompt_tokens(&state.session.id);
                }
            }

            view.set_llm_state(LlmState::Idle);
            *cancel_token = None;
            *llm_tx = None;
            *llm_rx = None;
        }
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
) {
    // Create channels for LLM events
    let (task_llm_tx, new_llm_rx) = tokio::sync::mpsc::channel(LLM_VIEW_CHANNEL_CAPACITY);
    *llm_tx = Some(task_llm_tx.clone());
    *llm_rx = Some(new_llm_rx);

    // Create cancellation token
    let token = CancellationToken::new();
    *cancel_token = Some(token.clone());

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
            let _ = task_llm_tx.send(LlmEvent::Cancelled).await;
            return;
        }

        // Use the streaming handler which sends tokens through the LlmEvent channel
        super::repl::handle_user_message_stream(
            &line_owned,
            &mut task_state,
            &mut channel_view as &mut dyn ChatView,
            task_llm_tx.clone(),
            token.clone(),
        )
        .await;

        // Check if cancelled after the call
        if token.is_cancelled() {
            let _ = task_llm_tx.send(LlmEvent::Cancelled).await;
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
/// `llm_tx`/`llm_rx` channel pair. The background task calls
/// `compact_conversation()` which streams `CompactStreamToken` events.
/// On completion, sends `CompactStreamDone` with the summary and range
/// (or `Error` on failure). The event loop handles finalization.
///
/// Unlike LLM tasks, compaction is NOT cancellable — Ctrl+C is ignored.
fn spawn_compact_task(
    state: &mut ReplState,
    llm_tx: &mut Option<tokio::sync::mpsc::Sender<LlmEvent>>,
    llm_rx: &mut Option<tokio::sync::mpsc::Receiver<LlmEvent>>,
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

    // Clone the state for the compaction task
    let task_state = state.clone();

    tokio::spawn(async move {
        match super::core::compact_conversation(
            &task_state.ollama,
            &task_state.model_config,
            &task_state.session,
            &task_state.settings,
            task_state.agents_md.as_deref(),
            task_llm_tx.clone(),
        )
        .await
        {
            Ok((summary, range)) => {
                let _ = task_llm_tx
                    .send(LlmEvent::CompactStreamDone { summary, range })
                    .await;
            }
            Err(e) => {
                let _ = task_llm_tx.send(LlmEvent::Error(e.to_string())).await;
            }
        }
    });
}

// ── Mouse event handler ────────────────────────────────────────────────

/// Number of lines to scroll per mouse wheel tick.
const MOUSE_SCROLL_LINES: u16 = 3;

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
