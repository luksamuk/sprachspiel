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
use crate::context_overflow::{
    check_context_overflow, needs_buffered_compaction, needs_pre_tool_compaction,
};
use crate::debug_tools::log_debug;
use crate::prompts::builder::{build_system_prompt, PromptConfig, PromptType};
use crate::prompts::CONTINUATION_PROMPT_INTER_TOOL;
use std::time::Instant;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Parse inter-tool compaction error from OllamaError
///
/// Returns (tokens_used, context_window, tools_executed) if the error is
/// a context needs compaction error, None otherwise.
pub fn parse_inter_tool_compaction_error(error_str: &str) -> Option<(usize, usize, Vec<String>)> {
    let marker = "CONTEXT_NEEDS_COMPACT:";
    let rest = error_str.strip_prefix(marker)?;

    let mut parts = rest.splitn(3, ':');
    let tokens_used: usize = parts.next()?.parse().ok()?;
    let context_window: usize = parts.next()?.parse().ok()?;
    let tools_str = parts.next()?;
    let tools_executed: Vec<String> = if tools_str.is_empty() {
        Vec::new()
    } else {
        tools_str.split(',').map(|s| s.trim().to_string()).collect()
    };

    Some((tokens_used, context_window, tools_executed))
}

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

    // Sync global TODO state back to session before saving
    state.session.todos = crate::tools::todo::save_to_session();

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
    // First check if we need actual compaction (88% threshold)
    if needs_buffered_compaction(&state.session, context_window) {
        let ctx_status = check_context_overflow(
            &state.session,
            system_prompt,
            context_window,
        );

        let usage_pct = ctx_status.usage_percent();
        let total_tokens = ctx_status.total_tokens();
        let remaining = context_window.saturating_sub(total_tokens);

        eprintln!(
            "\x1B[33m⏳ Context {}% full ({}K remaining). Auto-compacting before tool execution...\x1B[0m",
            usage_pct,
            remaining / 1000
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
    } else if needs_pre_tool_compaction(&state.session, context_window) {
        // At 75%: just show warning, don't compact yet
        let ctx_status = check_context_overflow(
            &state.session,
            system_prompt,
            context_window,
        );

        let usage_pct = ctx_status.usage_percent();
        let total_tokens = ctx_status.total_tokens();
        let remaining = context_window.saturating_sub(total_tokens);

        eprintln!(
            "\x1B[33m⚠ Context {}% full ({}K remaining). Consider using /compact to summarize old messages.\x1B[0m",
            usage_pct,
            remaining / 1000
        );
    }
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

/// Check if error is an inter-tool compaction error
pub fn is_inter_tool_compaction_error(error_str: &str) -> bool {
    error_str.starts_with("CONTEXT_NEEDS_COMPACT:")
}

/// Result of handling an overflow error
#[derive(Debug)]
pub enum OverflowHandleResult {
    /// Error was not an overflow error, caller should handle
    NotOverflow,
    /// Overflow was handled, caller should continue the loop (no continuation)
    HandledContinue,
    /// Inter-tool compaction happened, caller should send continuation prompt
    InterToolCompaction { tools_executed: Vec<String> },
}

/// Handle context overflow error during tool execution
///
/// Attempts recovery by removing failed messages and compacting context.
/// Returns appropriate result for caller to determine next action.
///
/// # Arguments
///
/// * `state` - Mutable reference to REPL state
/// * `error_str` - The error string from send_message
///
/// # Returns
///
/// * `NotOverflow` - Not an overflow error, caller should handle differently
/// * `HandledContinue` - Overflow handled, caller should continue the loop
/// * `InterToolCompaction` - Inter-tool compaction, caller should send continuation
pub async fn handle_overflow_error(state: &mut ReplState, error_str: &str) -> OverflowHandleResult {
    if is_inter_tool_compaction_error(error_str) {
        return handle_inter_tool_compaction_error(state, error_str).await;
    }

    if !error_str.contains("Context overflow during tool execution") {
        return OverflowHandleResult::NotOverflow;
    }

    eprintln!(
        "\x1B[31mContext overflow during tool execution. Attempting recovery...\x1B[0m"
    );

    let (removed, _) = state.session.remove_last_assistant_messages_with_content();
    if state.use_debug {
        log_debug(&format!(
            "Removed {} messages after overflow error",
            removed
        ));
    }

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

    OverflowHandleResult::HandledContinue
}

/// Handle inter-tool compaction error during multi-tool execution
///
/// Compacts context and returns info for continuation.
async fn handle_inter_tool_compaction_error(
    state: &mut ReplState,
    error_str: &str,
) -> OverflowHandleResult {
    let Some((tokens_used, context_window, tools_executed)) =
        parse_inter_tool_compaction_error(error_str)
    else {
        return OverflowHandleResult::NotOverflow;
    };

    let start_time = Instant::now();
    let tokens_before = state.session.history_real_tokens();
    let messages_before = state.session.messages.len();

    eprintln!(
        "\x1B[33m⏳ Context limit reached during tool execution ({} tools executed). Compacting...\x1B[0m",
        tools_executed.len()
    );

    if state.use_debug {
        log_debug(&format!(
            "[Inter-tool Compaction] Starting: {}K/{}K tokens ({}%), {} messages in history",
            tokens_used / 1000,
            context_window / 1000,
            (tokens_used * 100) / context_window,
            messages_before
        ));
        log_debug(&format!(
            "[Inter-tool Compaction] Tools executed before pause: {}",
            tools_executed.join(", ")
        ));
    }

    let system_prompt = build_pre_tool_prompt(state);

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

    let tokens_after = state.session.history_real_tokens();
    let messages_after = state.session.messages.len();
    let elapsed = start_time.elapsed();

    if state.use_debug {
        let tokens_saved = tokens_before.saturating_sub(tokens_after);
        let _messages_removed = messages_before.saturating_sub(messages_after);
        
        log_debug(&format!(
            "[Inter-tool Compaction] Completed in {:.2}s: {}K → {}K tokens (saved {}K), {} → {} messages",
            elapsed.as_secs_f64(),
            tokens_before / 1000,
            tokens_after / 1000,
            tokens_saved / 1000,
            messages_before,
            messages_after
        ));
        
        if let Some(summary) = &state.session.compacted_summary {
            log_debug(&format!(
                "[Inter-tool Compaction] Summary length: {} chars",
                summary.len()
            ));
        }
    }

    OverflowHandleResult::InterToolCompaction {
        tools_executed: tools_executed.clone(),
    }
}

/// Build continuation prompt for inter-tool compaction
pub fn build_inter_tool_compaction_prompt(tools_executed: &[String]) -> String {
    if tools_executed.is_empty() {
        return CONTINUATION_PROMPT_INTER_TOOL.to_string();
    }

    format!(
        "{}\n\nTools already executed: {}.",
        CONTINUATION_PROMPT_INTER_TOOL,
        tools_executed.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_inter_tool_compaction_error_valid() {
        let error = "CONTEXT_NEEDS_COMPACT:30000:32000:read_file,calculate,write_file";
        let result = parse_inter_tool_compaction_error(error);
        
        assert!(result.is_some());
        let (tokens, window, tools) = result.unwrap();
        assert_eq!(tokens, 30000);
        assert_eq!(window, 32000);
        assert_eq!(tools, vec!["read_file", "calculate", "write_file"]);
    }

    #[test]
    fn test_parse_inter_tool_compaction_error_empty_tools() {
        let error = "CONTEXT_NEEDS_COMPACT:25000:32000:";
        let result = parse_inter_tool_compaction_error(error);
        
        assert!(result.is_some());
        let (tokens, window, tools) = result.unwrap();
        assert_eq!(tokens, 25000);
        assert_eq!(window, 32000);
        assert!(tools.is_empty());
    }

    #[test]
    fn test_parse_inter_tool_compaction_error_invalid_prefix() {
        let error = "OTHER_ERROR:something";
        let result = parse_inter_tool_compaction_error(error);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_inter_tool_compaction_error_missing_fields() {
        let error = "CONTEXT_NEEDS_COMPACT:30000";
        let result = parse_inter_tool_compaction_error(error);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_inter_tool_compaction_error_invalid_token() {
        let error = "CONTEXT_NEEDS_COMPACT:invalid:32000:tool";
        let result = parse_inter_tool_compaction_error(error);
        assert!(result.is_none());
    }

    #[test]
    fn test_is_inter_tool_compaction_error_true() {
        assert!(is_inter_tool_compaction_error("CONTEXT_NEEDS_COMPACT:30000:32000:tool"));
        assert!(is_inter_tool_compaction_error("CONTEXT_NEEDS_COMPACT:0:0:"));
    }

    #[test]
    fn test_is_inter_tool_compaction_error_false() {
        assert!(!is_inter_tool_compaction_error("Context overflow during tool execution"));
        assert!(!is_inter_tool_compaction_error("Network error"));
        assert!(!is_inter_tool_compaction_error(""));
    }

    #[test]
    fn test_build_inter_tool_compaction_prompt_empty() {
        let prompt = build_inter_tool_compaction_prompt(&[]);
        assert!(prompt.contains("Context was compacted during multi-tool execution"));
        assert!(!prompt.contains("Tools already executed"));
    }

    #[test]
    fn test_build_inter_tool_compaction_prompt_with_tools() {
        let tools = vec!["read_file".to_string(), "calculate".to_string()];
        let prompt = build_inter_tool_compaction_prompt(&tools);
        
        assert!(prompt.contains("Context was compacted during multi-tool execution"));
        assert!(prompt.contains("Tools already executed: read_file, calculate"));
    }

    #[test]
    fn test_build_inter_tool_compaction_prompt_single_tool() {
        let tools = vec!["search".to_string()];
        let prompt = build_inter_tool_compaction_prompt(&tools);
        
        assert!(prompt.contains("Tools already executed: search"));
    }
}