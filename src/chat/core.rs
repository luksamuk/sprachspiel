//! Core chat functionality
//!
//! This module provides `ChatCore`, encapsulating the core logic for sending
//! messages and managing chat conversations. This enables:
//!
//! 1. Clear separation of concerns (I/O vs business logic)
//! 2. Easier testing (core logic in isolation)
//! 3. Future TUI compatibility (stateless processing)
//!
//! # Architecture
//!
//! ```text
//! Layer 4 (Core): core.rs
//!     ↓ uses
//! Layer 3 (State): repl_state.rs
//! Layer 1 (Session): session.rs
//! Layer 0 (Base): capabilities, config
//! ```
//!
//! # View Event Channel
//!
//! The coordinator event callback (`setup_coordinator`) sends events through
//! an `mpsc` channel (`ViewEventSender`) instead of printing directly.
//! The `ViewEventReceiver` is drained into `ChatView` after the coordinator
//! call completes, ensuring all output goes through the view layer.

use std::sync::Arc;

use ollama_rs::generation::chat::ChatMessage;

use crate::config::ModelConfig;
use crate::context_overflow::{
    MAX_SUMMARY_TOKENS, check_context_overflow, needs_buffered_compaction,
};
use crate::facts::prompt::build_facts_section;
use crate::prompts::builder::{
    PromptConfig, PromptType, build_compaction_prompt, build_continuation_prompt,
    build_system_prompt,
};
use crate::retrieval::{RetrievalConfig, build_context, update_retrieval_time};
use crate::settings::Settings;
use crate::spinner::finish_spinner;
use crate::tokens::estimate_tokens;
use crate::tools::context::{with_full_context, with_tool_context};
use crate::tools::{get_available_tool_names, register_tools};
use crate::utils::truncate_to_budget;

use super::coordinator::{
    MAX_RETRIES, classify_ollama_error, format_recovery_message, is_ollama_error_recoverable,
};
use super::custom_coordinator::CustomCoordinator;
use super::llm_event::LlmEvent;
use super::session::ChatSession;
use super::thinking::{extract_thinking, strip_thinking_tags};
use super::view::ChatView;
use super::{ContinuationTag, parse_continuation_tag};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Token usage metrics for a chat response
#[derive(Debug, Clone, Default)]
pub struct TokenMetrics {
    pub prompt_tokens: u64,
    pub response_tokens: u64,
    pub total_tokens: u64,
}

/// Result of sending a message to the LLM
///
/// Contains the response, any pre-tool content (for tool calls),
/// token metrics, and continuation information.
pub struct SendMessageResult {
    pub response: String,
    pub pre_tool_content: Option<String>,
    pub pre_tool_thinking: Option<String>,
    pub metrics: TokenMetrics,
    pub context_window: usize,
    pub system_prompt: String,
    /// Parsed continuation tag if LLM requested to continue after compaction
    pub continuation_needed: Option<ContinuationTag>,
}

/// Build system prompt for the session
#[expect(clippy::too_many_arguments)]
pub fn build_session_system_prompt(
    session: &ChatSession,
    tools_enabled: bool,
    cli_code: bool,
    cli_soulless: bool,
    model_config: &ModelConfig,
    blacklist_set: &std::collections::HashSet<&str>,
    agents_md: Option<&str>,
    facts_section: Option<&str>,
    todos_section: Option<&str>,
) -> String {
    if let Some(ref custom_prompt) = session.system_prompt {
        return custom_prompt.clone();
    }

    let prompt_type = if cli_code && tools_enabled {
        PromptType::CodeWithTools
    } else if cli_code {
        PromptType::Code
    } else if tools_enabled {
        PromptType::ToolUser
    } else {
        PromptType::Default
    };

    let ctx_window = model_config.num_ctx as usize;
    let ctx_status = check_context_overflow(session, "", ctx_window);

    build_system_prompt(
        PromptConfig::new(prompt_type)
            .with_model_id(Some(&model_config.model_id))
            .with_blacklist(Some(blacklist_set))
            .with_agents_md(agents_md)
            .with_tools(tools_enabled)
            .with_retrieval(session.retrieval_enabled && !cli_code)
            .with_soulless(cli_soulless)
            .with_context_status(if ctx_status.needs_compaction() {
                Some(ctx_status.clone())
            } else {
                None
            })
            .with_facts_section(facts_section)
            .with_anonymous(session.anonymous)
            .with_todos(todos_section)
            .with_active_skill(session.active_skill.as_ref().map(|s| s.content.as_str())),
    )
}

/// Setup coordinator with optional tools
///
/// Events from tool execution (pre-tool content, context warnings) are sent
/// via `view_event_sender` to a channel, and drained into the `ChatView`
/// after the coordinator call completes.
#[expect(clippy::too_many_arguments)]
pub fn setup_coordinator(
    ollama: ollama_rs::Ollama,
    model_config: &ModelConfig,
    model_options: ollama_rs::models::ModelOptions,
    think_enabled: bool,
    tools_enabled: bool,
    settings: &Settings,
    system_prompt: String,
    real_history_tokens: Option<usize>,
    view_event_sender: super::view::ViewEventSender,
    llm_tx: Option<tokio::sync::mpsc::Sender<super::llm_event::LlmEvent>>,
) -> CustomCoordinator<Vec<ChatMessage>> {
    let coordinator = crate::query::ChatContext {
        ollama,
        model_id: model_config.model_id.clone(),
        model_options,
        use_think: think_enabled,
        context_window: Some(model_config.num_ctx as usize),
        system_prompt: Some(system_prompt),
    }
    .build_coordinator()
    .on_event(move |event| {
        // Send view events through the channel for later rendering.
        // This avoids requiring the callback closure to hold a mutable
        // reference to ChatView, which is not possible with 'static closures.
        //
        // For streaming TUI mode: PreToolContent from inter-tool rounds
        // is sent DIRECTLY to the LLM event channel via LlmEvent::InterToolText
        // so the event loop can process it in real-time. Without this, the
        // content would be batched via ViewEvents and either (a) dropped due
        // to streaming deduplication, or (b) appear in the wrong order.
        let has_llm_tx = llm_tx.is_some();
        match event {
            crate::chat::custom_coordinator::ChatEvent::PreToolContent { content, thinking } => {
                if has_llm_tx {
                    // Streaming mode: emit directly to LLM channel for real-time
                    // event loop processing. This avoids batching and deduplication.
                    let _ = llm_tx.as_ref().unwrap().try_send(
                        super::llm_event::LlmEvent::InterToolText {
                            content,
                            metrics: None,
                        },
                    );
                } else {
                    // Terminal mode: emit as ViewEvent for batch processing
                    let cleaned = strip_thinking_tags(&content);
                    if thinking.is_some() || !cleaned.trim().is_empty() {
                        view_event_sender.send(super::view::ViewEvent::PreToolContent {
                            content: cleaned,
                            thinking,
                        });
                    }
                }
            }
            crate::chat::custom_coordinator::ChatEvent::ContextNeedsCompaction {
                tokens_used,
                context_window,
                ..
            } => {
                let percent = (tokens_used * 100).checked_div(context_window).unwrap_or(0);
                view_event_sender.send(super::view::ViewEvent::ContextNeedsCompaction {
                    percent: percent as u64,
                });
            }
            crate::chat::custom_coordinator::ChatEvent::ContextTruncated { .. } => {
                // Already logged via log::warn! — no view event needed
                // (this is informational, not user-facing)
            }
            _ => {
                // Other events (ToolCall, ToolResult, ContextNearLimit)
                // are handled by log_tool_call/log_tool_result/log::debug
            }
        }
    });

    let mut coordinator = coordinator;

    // Set real token count for accurate overflow detection
    if let Some(tokens) = real_history_tokens {
        coordinator = coordinator.real_history_tokens(tokens);
    }

    if tools_enabled {
        let (coord_new, tool_count) = register_tools(coordinator, settings);
        coordinator = coord_new;
        if log::log_enabled!(log::Level::Debug) {
            log::debug!("{} tools active", tool_count);
        }
    }
    coordinator
}

/// Prepare messages with retrieval and optional continuation
#[allow(clippy::too_many_arguments)]
pub async fn prepare_messages(
    session: &mut ChatSession,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
    user_input: &str,
    system_prompt: &str,
    coordinator: &mut CustomCoordinator<Vec<ChatMessage>>,
    continuation_tag: Option<&ContinuationTag>,
) -> Vec<ChatMessage> {
    let retrieval_config = if session.retrieval_enabled {
        RetrievalConfig::default()
    } else {
        RetrievalConfig {
            enabled: false,
            ..RetrievalConfig::default()
        }
    };

    let context_result = build_context(
        session,
        db,
        embedding_client,
        user_input,
        system_prompt,
        &retrieval_config,
    )
    .await;

    if context_result.retrieval_performed {
        update_retrieval_time(session);
        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "Retrieved {} relevant messages",
                context_result.retrieved_count
            );
        }
    }

    let mut messages = context_result.messages;
    messages.push(ChatMessage::user(user_input.to_string()));

    if let Some(tag) = continuation_tag {
        let continuation_prompt = build_continuation_prompt(&tag.paused_at, &tag.next_step);
        coordinator.push_ephemeral(ChatMessage::user(continuation_prompt));
        if log::log_enabled!(log::Level::Debug) {
            log::debug!("Injected continuation prompt as ephemeral message");
        }
    }

    messages
}

/// Process chat response into SendMessageResult
///
/// Renders thinking content and the main response via the provided view.
pub fn process_chat_response(
    response: ollama_rs::generation::chat::ChatMessageResponse,
    think_enabled: bool,
    coordinator: &mut CustomCoordinator<Vec<ChatMessage>>,
    context_window: usize,
    system_prompt: String,
    view: &mut dyn ChatView,
) -> SendMessageResult {
    let content = response.message.content.clone();

    let metrics = if let Some(ref final_data) = response.final_data {
        TokenMetrics {
            prompt_tokens: final_data.prompt_eval_count,
            response_tokens: final_data.eval_count,
            total_tokens: final_data.prompt_eval_count + final_data.eval_count,
        }
    } else {
        TokenMetrics::default()
    };

    // Extract and render thinking content via view
    if think_enabled {
        let thinking = extract_thinking(&content, response.message.thinking.as_ref());
        if let Some(ref thinking_content) = thinking {
            view.show_thinking(thinking_content);
        }
    }

    let display_content = strip_thinking_tags(&content);
    view.show_assistant_response(&display_content, None);

    let pre_tool = coordinator.take_pre_tool_content();
    let (pre_tool_content, pre_tool_thinking) = match pre_tool {
        Some(ptc) => (Some(ptc.content), ptc.thinking),
        None => (None, None),
    };

    let (cleaned_response, continuation_needed) = parse_continuation_tag(&display_content);

    if continuation_needed.is_some() {
        view.clear_continuation_line();
        view.show_assistant_response(&cleaned_response, None);
    }

    SendMessageResult {
        response: cleaned_response,
        pre_tool_content,
        pre_tool_thinking,
        metrics,
        context_window,
        system_prompt,
        continuation_needed,
    }
}

/// Send a message to the LLM and process the response
///
/// This is the core function for chat interaction, handling:
/// - System prompt building
/// - Context overflow checking
/// - Tool registration
/// - Message preparation with retrieval
/// - Retry logic for recoverable errors
/// - Response processing
///
/// All output rendering is delegated to the provided `ChatView`.
#[expect(clippy::too_many_arguments)]
pub async fn send_message(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    user_input: &str,
    tools_enabled: bool,
    think_enabled: bool,
    cli_code: bool,
    settings: &Settings,
    agents_md: Option<&str>,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
    cli_soulless: bool,
    continuation_tag: Option<&ContinuationTag>,
    view: &mut dyn ChatView,
) -> AppResult<SendMessageResult> {
    let model_options = model_config.build_model_options();
    let blacklist_set = settings.blacklist_set();

    // Load facts from Factual Memory System
    let facts_section = if let Some(db_ref) = db {
        match db_ref.get_facts_for_prompt(session.project_id.as_deref()) {
            Ok(facts) if !facts.is_empty() => {
                let section = build_facts_section(&facts);
                if log::log_enabled!(log::Level::Debug) && !section.is_empty() {
                    log::debug!("Loaded {} facts for prompt", facts.len());
                }
                Some(section)
            }
            Ok(_) => None,
            Err(e) => {
                if log::log_enabled!(log::Level::Debug) {
                    log::debug!("Warning: Failed to load facts: {}", e);
                }
                None
            }
        }
    } else {
        None
    };

    // Build system prompt
    // Get todos section from global state
    let todos_section = crate::tools::todo::format_todos_for_prompt();

    let system_prompt = build_session_system_prompt(
        session,
        tools_enabled,
        cli_code,
        cli_soulless,
        model_config,
        &blacklist_set,
        agents_md,
        facts_section.as_deref(),
        todos_section.as_deref(),
    );

    // Check context overflow
    let context_window = model_config.num_ctx as usize;
    let overflow_status = check_context_overflow(session, &system_prompt, context_window);

    // Show context warning only if tools are disabled.
    // When tools are enabled, check_and_compact_before_tool in continuation.rs
    // will show a more informative warning with remaining tokens.
    if overflow_status.needs_compaction() && !tools_enabled {
        view.show_context_warning(
            overflow_status.usage_percent(),
            "Consider using /compact to summarize old messages.",
        );
    } else if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "Context usage: {} / {} tokens ({:.1}%)",
            overflow_status.total_tokens(),
            context_window,
            overflow_status.usage_percent() as f32
        );
    }

    // Setup coordinator with optional tools
    // Get real token count from session for accurate overflow detection
    let real_history_tokens = session.history_real_tokens();

    if log::log_enabled!(log::Level::Debug) {
        // Collect prompt_tokens state for debugging
        let prompt_tokens_state: Vec<(usize, Option<u64>)> = session
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| (i, m.prompt_tokens))
            .take(10) // First 10
            .collect();
        let has_nonzero_tokens = session
            .messages
            .iter()
            .any(|m| m.prompt_tokens.map(|t| t > 0).unwrap_or(false));

        log::debug!(
            "[setup_coordinator] real_history_tokens={} messages={} has_compacted={} messages_sent_to_llm={}",
            real_history_tokens,
            session.messages.len(),
            session.has_compacted_messages(),
            session.messages_sent_to_llm
        );
        log::debug!(
            "[setup_coordinator] has_nonzero_prompt_tokens={} first_10_prompt_tokens={:?}",
            has_nonzero_tokens,
            prompt_tokens_state
        );
        if session.has_compacted_messages() {
            log::debug!(
                "[setup_coordinator] summary_len={} compacted_range={:?}",
                session
                    .compacted_summary
                    .as_ref()
                    .map(|s| s.len())
                    .unwrap_or(0),
                session.compacted_range
            );
        }
    }

    // Create view event channel for coordinator callback
    let (view_event_sender, view_event_receiver) = super::view::create_view_event_channel();

    let mut coordinator = setup_coordinator(
        ollama.clone(),
        model_config,
        model_options,
        think_enabled,
        tools_enabled,
        settings,
        system_prompt.clone(),
        Some(real_history_tokens),
        view_event_sender,
        None,
    );

    // Prepare messages with retrieval and continuation
    let mut messages = prepare_messages(
        session,
        db,
        embedding_client,
        user_input,
        &system_prompt,
        &mut coordinator,
        continuation_tag,
    )
    .await;

    if log::log_enabled!(log::Level::Debug) {
        log::debug!("Sending {} messages to model", messages.len());
        if session.has_compacted_messages() {
            log::debug!(
                "(includes compacted summary of {} messages)",
                session.compacted_message_count()
            );
        }
    }

    let spinner =
        crate::spinner::create_spinner_suppressed("Thinking...", view.suppress_progress_spinner());

    let tool_names: Vec<String> = if tools_enabled {
        get_available_tool_names(settings)
    } else {
        vec![]
    };

    // Execute with retry logic
    let mut attempts = 0;
    let result = loop {
        let current_result = if let (Some(db), Some(embedding)) = (db, embedding_client) {
            with_full_context(
                db.clone(),
                embedding.clone(),
                ollama.clone(),
                Arc::new(settings.clone()),
                coordinator.chat(messages.clone()),
            )
            .await
        } else {
            with_tool_context(
                ollama.clone(),
                Arc::new(settings.clone()),
                coordinator.chat(messages.clone()),
            )
            .await
        };

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                if is_ollama_error_recoverable(&e) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            MAX_RETRIES,
                            recovery_err.description()
                        );
                    }

                    messages.push(ChatMessage::tool(error_msg));

                    if attempts == 1 {
                        finish_spinner(spinner.clone());
                        view.show_system("  Retrying after error...");
                    }

                    continue;
                } else {
                    let error_str = e.to_string();
                    break Err(error_str);
                }
            }
        }
    };

    finish_spinner(spinner);

    // Drain view events accumulated during coordinator chat (pre-tool content,
    // context warnings) into the ChatView for rendering
    view_event_receiver.drain_into(view);

    // Process response and display it
    let processed_result = match result {
        Ok(response) => process_chat_response(
            response,
            think_enabled,
            &mut coordinator,
            context_window,
            system_prompt,
            view,
        ),
        Err(e) => return Err(e.into()),
    };

    Ok(processed_result)
}

/// Send a message to the LLM with streaming token display.
///
/// This is the streaming equivalent of `send_message()`. Instead of waiting
/// for the full response, it streams token chunks through the provided
/// `LlmEvent` sender. When the stream completes:
/// - Sends `LlmEvent::StreamDone` with the full content and metrics
/// - Tool calls are handled by the non-streaming coordinator path after streaming
///
/// The `llm_tx` sender is used for:
/// - `LlmEvent::StreamToken(token)` — each content token chunk
/// - `LlmEvent::StreamThinking(token)` — each thinking token chunk
/// - `LlmEvent::ViewAction(action)` — view events from tool calls
///
/// All non-streaming view output (context warnings, tool results, etc.) is
/// delegated to the provided `ChatView`.
#[expect(clippy::too_many_arguments)]
pub async fn send_message_stream(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    user_input: &str,
    tools_enabled: bool,
    think_enabled: bool,
    cli_code: bool,
    settings: &Settings,
    agents_md: Option<&str>,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
    cli_soulless: bool,
    continuation_tag: Option<&ContinuationTag>,
    view: &mut dyn ChatView,
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
) -> AppResult<SendMessageResult> {
    let model_options = model_config.build_model_options();
    let blacklist_set = settings.blacklist_set();

    // Load facts from Factual Memory System
    let facts_section = if let Some(db_ref) = db {
        match db_ref.get_facts_for_prompt(session.project_id.as_deref()) {
            Ok(facts) if !facts.is_empty() => {
                let section = build_facts_section(&facts);
                if log::log_enabled!(log::Level::Debug) && !section.is_empty() {
                    log::debug!("Loaded {} facts for prompt", facts.len());
                }
                Some(section)
            }
            Ok(_) => None,
            Err(e) => {
                if log::log_enabled!(log::Level::Debug) {
                    log::debug!("Warning: Failed to load facts: {}", e);
                }
                None
            }
        }
    } else {
        None
    };

    // Build system prompt
    let todos_section = crate::tools::todo::format_todos_for_prompt();

    let system_prompt = build_session_system_prompt(
        session,
        tools_enabled,
        cli_code,
        cli_soulless,
        model_config,
        &blacklist_set,
        agents_md,
        facts_section.as_deref(),
        todos_section.as_deref(),
    );

    // Check context overflow
    let context_window = model_config.num_ctx as usize;
    let overflow_status = check_context_overflow(session, &system_prompt, context_window);

    if overflow_status.needs_compaction() && !tools_enabled {
        view.show_context_warning(
            overflow_status.usage_percent(),
            "Consider using /compact to summarize old messages.",
        );
    }

    // Setup coordinator with optional tools
    let real_history_tokens = session.history_real_tokens();

    // Create view event channel for coordinator callback
    let (view_event_sender, view_event_receiver) = super::view::create_view_event_channel();

    let mut coordinator = setup_coordinator(
        ollama.clone(),
        model_config,
        model_options,
        think_enabled,
        tools_enabled,
        settings,
        system_prompt.clone(),
        Some(real_history_tokens),
        view_event_sender,
        Some(llm_tx.clone()),
    );

    // Prepare messages with retrieval and continuation
    let mut messages = prepare_messages(
        session,
        db,
        embedding_client,
        user_input,
        &system_prompt,
        &mut coordinator,
        continuation_tag,
    )
    .await;

    if log::log_enabled!(log::Level::Debug) {
        log::debug!("Sending {} messages to model (streaming)", messages.len());
    }

    // No indicatif spinner in TUI mode
    let tool_names: Vec<String> = if tools_enabled {
        get_available_tool_names(settings)
    } else {
        vec![]
    };

    // Create streaming callbacks that send tokens through the LlmEvent channel
    let llm_tx_token = llm_tx.clone();
    let on_token = move |token: String| {
        let _ = llm_tx_token.try_send(LlmEvent::StreamToken(token));
    };

    let llm_tx_thinking = llm_tx.clone();
    let on_thinking = move |token: String| {
        let _ = llm_tx_thinking.try_send(LlmEvent::StreamThinking(token));
    };

    let llm_tx_tool = llm_tx.clone();
    let on_tool_call = move || {
        let _ = llm_tx_tool.try_send(LlmEvent::ToolCallStarted);
    };

    // Execute with retry logic using streaming
    let mut attempts = 0;
    let result = loop {
        let current_result = if let (Some(db), Some(embedding)) = (db, embedding_client) {
            with_full_context(
                db.clone(),
                embedding.clone(),
                ollama.clone(),
                Arc::new(settings.clone()),
                coordinator.chat_stream(
                    messages.clone(),
                    &on_token,
                    &on_thinking,
                    &on_tool_call,
                    cancel_token.clone(),
                ),
            )
            .await
        } else {
            with_tool_context(
                ollama.clone(),
                Arc::new(settings.clone()),
                coordinator.chat_stream(
                    messages.clone(),
                    &on_token,
                    &on_thinking,
                    &on_tool_call,
                    cancel_token.clone(),
                ),
            )
            .await
        };

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                if is_ollama_error_recoverable(&e) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            MAX_RETRIES,
                            recovery_err.description()
                        );
                    }

                    messages.push(ChatMessage::tool(error_msg));

                    if attempts == 1 {
                        view.show_system("  Retrying after error...");
                    }

                    continue;
                } else {
                    let error_str = e.to_string();
                    break Err(error_str);
                }
            }
        }
    };

    // Drain view events accumulated during coordinator chat directly into
    // the LLM event channel. This guarantees that ViewActions (pre-tool
    // content, context warnings) arrive as LlmEvent::ViewAction BEFORE
    // StreamDone, ensuring correct message ordering in the TUI event loop.
    // The previous approach (draining into ChannelView → async forwarding)
    // could reorder ViewActions relative to StreamDone.
    view_event_receiver.drain_into_llm_channel(&llm_tx);

    // Process response
    let processed_result = match result {
        Ok(response) => {
            let content = response.message.content.clone();

            let metrics = if let Some(ref final_data) = response.final_data {
                TokenMetrics {
                    prompt_tokens: final_data.prompt_eval_count,
                    response_tokens: final_data.eval_count,
                    total_tokens: final_data.prompt_eval_count + final_data.eval_count,
                }
            } else {
                TokenMetrics::default()
            };

            // In streaming mode, thinking was already displayed via StreamThinking events.
            // Extract thinking for the result struct (but don't display again via view).
            let thinking = if think_enabled {
                extract_thinking(&content, response.message.thinking.as_ref())
            } else {
                None
            };

            // Content is already displayed via StreamToken events.
            // Don't call view.show_assistant_response() — that would duplicate.

            let display_content = strip_thinking_tags(&content);
            let pre_tool = coordinator.take_pre_tool_content();
            let (pre_tool_content, pre_tool_thinking) = match pre_tool {
                Some(ptc) => (Some(ptc.content), ptc.thinking),
                None => (None, None),
            };

            let (cleaned_response, continuation_needed) = parse_continuation_tag(&display_content);

            // When tools interrupted streaming, pre-tool content is already
            // displayed via StreamToken events. Send StreamBlockDone to
            // finalize it as a STABLE Assistant+Thinking message, preventing
            // StreamDone from wiping it. StreamDone then adds ONLY the
            // post-tool content as a NEW message.
            let post_tool_content = if let Some(ref pre_tool) = pre_tool_content {
                let pre_tool_display = strip_thinking_tags(pre_tool);
                let _ = llm_tx.try_send(LlmEvent::StreamBlockDone {
                    content: pre_tool_display.clone(),
                    thinking: pre_tool_thinking.clone(),
                    metrics: None,
                });
                // Compute post-tool content: full response minus pre-tool.
                // The LLM may or may not include the pre-tool text in its
                // final response. If it does, remove the prefix to avoid
                // duplication. If it doesn't, the whole response is post-tool.
                if cleaned_response.starts_with(&pre_tool_display) {
                    cleaned_response[pre_tool_display.len()..]
                        .trim()
                        .to_string()
                } else {
                    cleaned_response.clone()
                }
            } else {
                cleaned_response.clone()
            };

            // StreamDone: post-tool content (or the only content if no tools)
            let _ = llm_tx.try_send(LlmEvent::StreamDone {
                content: post_tool_content,
                thinking: thinking.clone(),
                metrics: None,
            });

            SendMessageResult {
                response: cleaned_response,
                pre_tool_content,
                pre_tool_thinking,
                metrics,
                context_window,
                system_prompt,
                continuation_needed,
            }
        }
        Err(e) => return Err(e.into()),
    };

    Ok(processed_result)
}
/// Uses buffer-based approach (15K tokens remaining) for predictable overflow prevention.
///
/// All output rendering is delegated to the provided `ChatView`.
pub async fn auto_compact_if_needed(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
    context_window: usize,
    view: &mut dyn ChatView,
) {
    // Use buffer-based compaction trigger (more predictable than percentages)
    // Compacts when there are only COMPACTION_BUFFER tokens remaining
    if !needs_buffered_compaction(session, context_window) {
        return;
    }

    // Calculate usage percentage for display purposes
    let real_tokens = session.history_real_tokens();
    let usage_percent = ((real_tokens as f32 / context_window as f32) * 100.0).min(100.0) as u8;

    // Show indicator before starting compaction
    view.show_compact_progress(&format!(
        "Compacting context ({}% full, {}K remaining)...",
        usage_percent,
        (context_window.saturating_sub(real_tokens)) / 1000
    ));

    // Attempt auto-compaction
    let suppress_spinner = view.suppress_progress_spinner();
    match compact_conversation(
        ollama,
        model_config,
        session,
        settings,
        agents_md,
        suppress_spinner,
    )
    .await
    {
        Ok((summary, range)) => {
            session.set_compacted_summary_with_range(summary, range);

            // Get compacted count
            let (first_preserved, last_preserved_start) =
                range.unwrap_or((0, session.messages.len()));
            let compacted_count = last_preserved_start - first_preserved;
            let preserved_last = session.messages.len() - last_preserved_start;

            view.show_compact_complete(compacted_count, first_preserved, preserved_last);

            if !session.anonymous {
                let _ = session.save_sqlite();

                // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                if let Some(db) = session.db.as_ref() {
                    let _ = db.clear_conversation_prompt_tokens(&session.id);
                }
            }
        }
        Err(e) => {
            view.show_error(&format!("Auto-compaction failed: {}", e));
        }
    }
}

/// Compact conversation by summarizing old messages
///
/// When `suppress_spinner` is `true`, no indicatif progress spinner is created.
/// This is the case for TUI mode where the view has its own progress indication
/// and indicatif would corrupt the alternate screen buffer with ANSI escapes.
#[allow(clippy::too_many_arguments)]
pub async fn compact_conversation(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &ChatSession,
    _settings: &Settings,
    _agents_md: Option<&str>,
    suppress_spinner: bool,
) -> AppResult<(String, Option<(usize, usize)>)> {
    use crate::context_overflow::get_compaction_range_default;

    if session.messages.is_empty() {
        return Err("No messages to compact.".into());
    }

    // Determine which messages to summarize (middle compaction)
    let (messages_to_summarize, range) = match get_compaction_range_default(session) {
        Some(suggestion) => {
            // Middle compaction: preserve first N + last N, summarize middle
            let middle: Vec<_> = session.messages[suggestion.middle_indices.clone()].to_vec();
            let range = Some((
                suggestion.keep_first,
                session.messages.len() - suggestion.keep_last,
            ));
            (middle, range)
        }
        None => {
            // Not enough messages for middle compaction, summarize all
            let all = session.messages.clone();
            let range = Some((0, session.messages.len()));
            (all, range)
        }
    };

    // Build conversation text for summarization
    let mut conversation_text = String::new();
    for msg in &messages_to_summarize {
        match msg.role {
            super::session::MessageRole::User => {
                conversation_text.push_str(&format!("User: {}\n", msg.content));
            }
            super::session::MessageRole::Assistant => {
                conversation_text.push_str(&format!("Assistant: {}\n", msg.content));
            }
            super::session::MessageRole::System => {}
            super::session::MessageRole::Tool => {
                conversation_text.push_str(&format!("Tool call: {}\n", msg.content));
            }
        }
    }

    let compact_prompt = build_compaction_prompt(&conversation_text);

    let mut model_cfg = model_config.clone();
    model_cfg.temperature = 0.3;
    model_cfg.top_p = Some(0.9);
    let model_options = model_cfg.build_model_options();

    let mut coordinator =
        CustomCoordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
            .options(model_options);

    let messages = vec![
        ChatMessage::system("You are a helpful assistant that summarizes conversations in clean Markdown format. Always use headers, bullets, and formatting to make the summary readable and scannable.".to_string()),
        ChatMessage::user(compact_prompt),
    ];

    let spinner = crate::spinner::create_spinner_suppressed("Compacting...", suppress_spinner);
    let result = coordinator.chat(messages).await;
    finish_spinner(spinner);

    match result {
        Ok(response) => {
            let summary = strip_thinking_tags(&response.message.content);

            // Truncate summary if it exceeds MAX_SUMMARY_TOKENS
            // This prevents infinite compaction loops caused by oversized summaries
            let summary = if estimate_tokens(&summary) > MAX_SUMMARY_TOKENS {
                log::warn!(
                    "Summary exceeds {} tokens, truncating...",
                    MAX_SUMMARY_TOKENS
                );
                truncate_to_budget(&summary, MAX_SUMMARY_TOKENS)
            } else {
                summary
            };

            Ok((summary, range))
        }
        Err(e) => Err(format!("Failed to compact: {}", e).into()),
    }
}
