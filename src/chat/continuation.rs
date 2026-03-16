//! Continuation handling for chat REPL
//!
//! This module provides functions to handle context continuation and overflow
//! recovery during chat interactions. These were extracted from `repl.rs` to
//! reduce cyclomatic complexity.
//!
//! # Architecture
//!
//! ```text
//! Layer 4 (Core): continuation.rs
//!     ↓ uses
//! Layer 3 (State): repl_state.rs
//! Layer 2 (Core): core.rs (send_message, auto_compact_if_needed)
//! ```

use super::core::{auto_compact_if_needed, send_message, SendMessageResult, TokenMetrics};
use super::repl_state::ReplState;
use crate::context_overflow::{check_context_overflow, needs_pre_tool_compaction, PRE_TOOL_THRESHOLD};
use crate::debug_tools::log_debug;
use crate::prompts::builder::{build_system_prompt, PromptConfig, PromptType};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Process a successful SendMessageResult
///
/// Handles:
/// - Pre-tool content saving
/// - Continuation handling (if needed)
/// - Final response and metrics extraction
/// - Auto-compaction after response
///
/// Returns the response and metrics (either from continuation or direct result)
pub async fn process_send_result(
    state: &mut ReplState,
    result: SendMessageResult,
    user_message_id: Option<i64>,
) -> ProcessResult {
    // Save pre-tool content before final response
    if let Some(pre_content) = &result.pre_tool_content {
        state.session.add_pre_tool_message(
            pre_content.clone(),
            result.pre_tool_thinking.clone(),
            user_message_id,
        );
        if state.use_debug {
            log_debug(&format!("Saved pre-tool content ({} chars)", pre_content.len()));
        }
    }

    // Handle continuation if LLM paused for compaction
    let (final_response, final_metrics, context_window, system_prompt) = 
        if result.continuation_needed.is_some() {
            match handle_continuation(state, result).await {
                Ok(cont_result) => (
                    cont_result.response,
                    cont_result.metrics,
                    cont_result.context_window,
                    cont_result.system_prompt,
                ),
                Err(e) => {
                    return ProcessResult::ContinuationError(e.to_string());
                }
            }
        } else {
            (
                result.response.clone(),
                result.metrics.clone(),
                result.context_window,
                result.system_prompt.clone(),
            )
        };

    // Save the final response (merged with continuations if any)
    state.session.add_assistant_message(
        final_response.clone(),
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

    ProcessResult::Success
}

/// Result of processing a send_message result
pub enum ProcessResult {
    Success,
    ContinuationError(String),
}

/// Check if pre-tool compaction is needed and perform it if necessary
///
/// Returns the system prompt that was built for the check.
pub async fn check_and_compact_before_tool(
    state: &mut ReplState,
    system_prompt: &str,
    context_window: usize,
) {
    if !needs_pre_tool_compaction(&state.session, system_prompt, context_window) {
        return;
    }

    let usage_pct = check_context_overflow(
        &state.session,
        system_prompt,
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
        system_prompt,
        context_window,
        state.use_debug,
    )
    .await;
}

/// Build system prompt for pre-tool check
pub fn build_pre_tool_prompt(state: &ReplState) -> String {
    build_system_prompt(
        PromptConfig::new(PromptType::ToolUser)
            .with_model_id(Some(&state.model_config.model_id))
            .with_blacklist(Some(&state.settings.blacklist_set()))
            .with_agents_md(state.agents_md.as_deref())
            .with_tools(state.tools_active)
            .with_retrieval(state.session.retrieval_enabled && !state.cli_code)
            .with_soulless(state.cli_soulless),
    )
}

/// Result of processing continuation(s) after context compaction
#[derive(Debug, Clone)]
pub struct ContinuationResult {
    pub response: String,
    pub metrics: TokenMetrics,
    pub context_window: usize,
    pub system_prompt: String,
}

/// Handle continuation after LLM pauses for context compaction
///
/// Processes nested continuations (max 3) when the LLM emits `<continuation_needed>`.
/// Each continuation involves compacting context and sending a follow-up request.
///
/// # Arguments
///
/// * `state` - Mutable reference to REPL state (contains session, ollama client, etc.)
/// * `initial_result` - The result from the initial `send_message` call that requested continuation
///
/// # Returns
///
/// * `Ok(ContinuationResult)` - Contains accumulated response and metrics
/// * `Err(...)` - If any continuation fails
pub async fn handle_continuation(
    state: &mut ReplState,
    initial_result: SendMessageResult,
) -> AppResult<ContinuationResult> {
    let mut final_response = initial_result.response.clone();
    let mut final_metrics = initial_result.metrics.clone();
    let mut continuation_count = 1; // Already counted first in repl.rs

    let continuation_tag = initial_result
        .continuation_needed
        .as_ref()
        .ok_or("No continuation tag found")?;

    if state.use_debug {
        log_debug(&format!(
            "Continuation requested: paused_at='{}', next_step='{}'",
            continuation_tag.paused_at, continuation_tag.next_step
        ));
    }
    eprintln!(
        "\n\x1B[33m⏳ Paused for context compaction, continuing...\x1B[0m"
    );

    // Compact context before first continuation
    let continuation_context_window = initial_result.context_window;
    let continuation_system_prompt = initial_result.system_prompt.clone();
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

    // Send first continuation request
    let think_enabled = state.session.think;
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
            final_metrics.response_tokens += cont_result.metrics.response_tokens;
            final_metrics.total_tokens += cont_result.metrics.total_tokens;

            eprintln!("\n\x1B[90m[Continuation complete]\x1B[0m");

            // Handle nested continuations (max 3)
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

                // Compact again before next continuation
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
                    "", // empty user_input
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
                        final_metrics.response_tokens += n_result.metrics.response_tokens;
                        final_metrics.total_tokens += n_result.metrics.total_tokens;

                        eprintln!("\n\x1B[90m[Continuation complete]\x1B[0m");

                        // Update for next iteration
                        cont_result = n_result;
                    }
                    Err(e) => {
                        eprintln!("\x1B[31mContinuation failed: {}\x1B[0m", e);
                        break;
                    }
                }
            }

            Ok(ContinuationResult {
                response: final_response,
                metrics: final_metrics,
                context_window: initial_result.context_window,
                system_prompt: initial_result.system_prompt,
            })
        }
        Err(e) => {
            eprintln!("\x1B[31mContinuation failed: {}\x1B[0m", e);
            Err(e)
        }
    }
}

/// Handle context overflow error during tool execution
///
/// Attempts recovery by removing failed messages and compacting context.
/// Returns `true` if overflow was handled (and caller should `continue`),
/// `false` if this was not an overflow error.
///
/// # Arguments
///
/// * `state` - Mutable reference to REPL state
/// * `error_str` - The error string from send_message
///
/// # Returns
///
/// * `true` - Overflow was handled, caller should `continue` the loop
/// * `false` - Not an overflow error, caller should handle differently
pub async fn handle_overflow_error(state: &mut ReplState, error_str: &str) -> bool {
    if !error_str.contains("Context overflow during tool execution") {
        return false;
    }

    eprintln!(
        "\x1B[31mContext overflow during tool execution. Attempting recovery...\x1B[0m"
    );

    // Remove the failed message
    let (removed, _) = state.session.remove_last_assistant_messages_with_content();
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

    true
}