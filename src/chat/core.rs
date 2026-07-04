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
use ollama_rs::models::ModelOptions;

use crate::config::ModelConfig;
use crate::context_overflow::{
    COMPACTION_PROMPT_OVERHEAD, DEFAULT_KEEP_FIRST, MAX_RECURSION_DEPTH, TRUNCATION_TARGET_RATIO,
    check_context_overflow, estimate_compaction_tokens, fallback_truncate, fits_in_context,
    is_prompt_too_long_error, max_chunk_tokens, pre_prune_messages, split_into_chunks,
};
use crate::facts::prompt::build_facts_section;
use crate::prompts::builder::{
    PromptConfig, PromptType, build_compaction_prompt, build_continuation_prompt,
    build_system_prompt,
};
use crate::provider::types::ProviderOptions;
use crate::retrieval::{RetrievalConfig, build_context, update_retrieval_time};
use crate::settings::Settings;
use crate::spinner::finish_spinner;
use crate::tools::context::{with_full_context, with_tool_context};
use crate::tools::{get_available_tool_names, register_tools};

use super::coordinator::{classify_ollama_error, format_recovery_message};
use super::custom_coordinator::CustomCoordinator;
use super::llm_event::LlmEvent;
use super::recovery::push_tool_result;
use super::session::ChatSession;
use super::thinking::{extract_thinking, process_thinking, strip_thinking_tags};
use super::view::ChatView;
use super::{ContinuationTag, parse_continuation_tag};
use crate::retry::{classify_for_retry, retry_delay, sleep_or_cancel};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Convert `ProviderOptions` (agnostic) to legacy `ModelOptions` (ollama-rs).
/// The CustomCoordinator still uses `ModelOptions` for now; this is removed
/// in the P6.0e.4 commit that migrates `custom_coordinator.rs` to LlmProvider.
pub fn convert_provider_to_model(opts: &ProviderOptions) -> ModelOptions {
    let mut out = ModelOptions::default();
    if let Some(t) = opts.temperature {
        out = out.temperature(t);
    }
    if let Some(p) = opts.top_p {
        out = out.top_p(p);
    }
    if let Some(n) = opts.num_predict {
        out = out.num_predict(n);
    }
    out
}

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
    /// Thinking content from the LLM response (preserved for storage)
    pub thinking: Option<String>,
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
    ollama: crate::provider::Ollama,
    model_config: &ModelConfig,
    model_options: ollama_rs::models::ModelOptions,
    think_enabled: bool,
    tools_enabled: bool,
    settings: &Settings,
    system_prompt: String,
    real_history_tokens: Option<usize>,
    view_event_sender: super::view::ViewEventSender,
    llm_tx: Option<tokio::sync::mpsc::Sender<super::llm_event::LlmEvent>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
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
        match event {
            crate::chat::custom_coordinator::ChatEvent::PreToolContent { content, thinking } => {
                if llm_tx.is_some() {
                    // TUI streaming path: pre-tool content is already on screen via
                    // StreamToken/StreamThinking and will be finalized by ToolCallStarted.
                    // Forwarding it again would duplicate the text.
                    let _ = (content, thinking);
                } else {
                    // Terminal mode: emit as ViewEvent for batch processing.
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
            crate::chat::custom_coordinator::ChatEvent::ToolExecutionStarted {
                tool_call_id,
                name,
                args,
            } => {
                if let Some(ref tx) = llm_tx {
                    let _ = tx.try_send(super::llm_event::LlmEvent::ToolExecutionStarted {
                        tool_call_id: tool_call_id.clone(),
                        name: name.clone(),
                        args: args.clone(),
                    });
                }
            }
            crate::chat::custom_coordinator::ChatEvent::ToolExecutionFinished {
                tool_call_id,
                result,
                is_error,
            } => {
                if let Some(ref tx) = llm_tx {
                    let _ = tx.try_send(super::llm_event::LlmEvent::ToolExecutionFinished {
                        tool_call_id: tool_call_id.clone(),
                        result: result.clone(),
                        is_error,
                    });
                }
            }
            _ => {
                // Other events (ContextNearLimit) are handled by
                // log_tool_call/log_tool_result/log::debug
            }
        }
    });

    let mut coordinator = coordinator;

    // Set cancellation token for interrupting tool execution mid-loop
    if let Some(ct) = cancel_token {
        coordinator = coordinator.cancel_token(ct);
    }

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
    let settings = crate::settings::Settings::load();
    let retrieval_config = if session.retrieval_enabled {
        RetrievalConfig {
            keyword_weight: settings.indexing.keyword_weight,
            semantic_weight: settings.indexing.semantic_weight,
            ..RetrievalConfig::default()
        }
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

    // Only add a user message if there is actual content.
    // The continuation path passes user_input="" and injects its own
    // ephemeral user message via coordinator.push_ephemeral() below.
    // Adding an empty user message confuses the LLM and wastes tokens.
    if !user_input.is_empty() {
        messages.push(ChatMessage::user(user_input.to_string()));
    }

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

    let processed = process_thinking(&content);
    let display_content = processed.content.clone();
    let thinking = processed.thinking;
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
        thinking,
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
    ollama: &crate::provider::Ollama,
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
    let provider_options = model_config.build_provider_options();
    // Bridge to legacy ModelOptions for CustomCoordinator.
    let model_options = convert_provider_to_model(&provider_options);
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
                // W2 Wave Context (#116): retry classification is in place,
                // but it only mitigates errors that ollama-rs RETURNS. When
                // Ollama hangs (kill -STOP, packet drop, server stopped),
                // ollama-rs does not return an error — the request hangs
                // indefinitely and the user never sees the retry messages.
                // TODO(#120): when OllamaProvider uses reqwest directly,
                // configure explicit timeouts and propagate HTTP errors
                // through ProviderError. Then this retry loop becomes
                // effective for the ServerRetry (5s/10s/15s) and
                // NetworkRetry (100ms→1.6s) scenarios from MANUAL_TEST_116.
                // Acceptance criteria for #120 are documented in
                // IMPLEMENTATION.md under W2 Wave Context.
                let category = classify_for_retry(&e);
                if category.is_retryable() && attempts < category.max_attempts() {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            category.max_attempts(),
                            recovery_err.description()
                        );
                    }

                    push_tool_result(&mut messages, error_msg);

                    if attempts == 1 {
                        finish_spinner(spinner.clone());
                        let delay = retry_delay(&category, attempts);
                        if delay > std::time::Duration::ZERO {
                            view.show_system(&format!("  Retrying in {}s...", delay.as_secs()));
                        } else {
                            view.show_system("  Retrying after error...");
                        }
                    }

                    // Cancel-aware sleep (non-streaming: no cancel token)
                    let _completed = sleep_or_cancel(retry_delay(&category, attempts), None).await;

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
    ollama: &crate::provider::Ollama,
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
    let provider_options = model_config.build_provider_options();
    // Bridge to legacy ModelOptions for CustomCoordinator.
    let model_options = convert_provider_to_model(&provider_options);
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

    // Clone the cancel_token so both the coordinator (tool loop cancellation)
    // and chat_stream() (stream cancellation) can check the same token.
    let coordinator_cancel = cancel_token.clone();

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
        coordinator_cancel,
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

    // Create streaming callbacks that send tokens through the LlmEvent channel.
    // These closures are moved into the coordinator and stored as boxed
    // callbacks so they can be reused across ReAct rounds.
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

    let llm_tx_preview = llm_tx.clone();
    let on_tool_call_preview =
        move |tool_call_id: String, name: String, args: serde_json::Value| {
            let _ = llm_tx_preview.try_send(LlmEvent::ToolCallPreview {
                tool_call_id,
                name,
                args,
            });
        };

    let llm_tx_provider_event = llm_tx.clone();
    let on_provider_event = move |event: LlmEvent| {
        let _ = llm_tx_provider_event.try_send(event);
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
                    on_token.clone(),
                    on_thinking.clone(),
                    on_tool_call.clone(),
                    on_tool_call_preview.clone(),
                    on_provider_event.clone(),
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
                    on_token.clone(),
                    on_thinking.clone(),
                    on_tool_call.clone(),
                    on_tool_call_preview.clone(),
                    on_provider_event.clone(),
                    cancel_token.clone(),
                ),
            )
            .await
        };

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                // W2 Wave Context (#116): retry classification is in place,
                // but it only mitigates errors that ollama-rs RETURNS. When
                // Ollama hangs (kill -STOP, packet drop, server stopped),
                // ollama-rs does not return an error — the request hangs
                // indefinitely and the user never sees the retry messages.
                // TODO(#120): when OllamaProvider uses reqwest directly,
                // configure explicit timeouts and propagate HTTP errors
                // through ProviderError. Then this retry loop becomes
                // effective for the ServerRetry (5s/10s/15s) and
                // NetworkRetry (100ms→1.6s) scenarios from MANUAL_TEST_116.
                // Acceptance criteria for #120 are documented in
                // IMPLEMENTATION.md under W2 Wave Context.
                let category = classify_for_retry(&e);
                if category.is_retryable() && attempts < category.max_attempts() {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            category.max_attempts(),
                            recovery_err.description()
                        );
                    }

                    push_tool_result(&mut messages, error_msg);

                    if attempts == 1 {
                        let delay = retry_delay(&category, attempts);
                        if delay > std::time::Duration::ZERO {
                            view.show_system(&format!("  Retrying in {}s...", delay.as_secs()));
                        } else {
                            view.show_system("  Retrying after error...");
                        }
                    }

                    // Cancel-aware sleep: aborts immediately on Ctrl+C
                    let _completed =
                        sleep_or_cancel(retry_delay(&category, attempts), cancel_token.as_ref())
                            .await;

                    continue;
                } else {
                    // Non-retryable error — propagate to the caller.
                    // Note: "invalid tool call arguments" (HTTP 400) is now
                    // handled inside the coordinator's process_next_stream
                    // (custom_coordinator.rs), which sanitizes invalid
                    // tool_calls and retries without breaking the ReAct loop.
                    // If it reaches here, the coordinator has already exhausted
                    // its internal retries (3 attempts) — propagate as fatal.
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

            // In streaming mode, thinking was already displayed via StreamThinking
            // events — no need to display again. But we still need to extract it
            // for storage. Use extract_thinking() so API-native thinking fields
            // (e.g. R1, Kimi) are respected before falling back to regex parsing.

            // Content is already displayed via StreamToken events.
            // Don't call view.show_assistant_response() — that would duplicate.

            let thinking = extract_thinking(&content, response.message.thinking.as_ref());
            let display_content = process_thinking(&content).content;
            let pre_tool = coordinator.take_pre_tool_content();
            let (pre_tool_content, pre_tool_thinking) = match pre_tool {
                Some(ptc) => (Some(ptc.content), ptc.thinking),
                None => (None, None),
            };

            let (cleaned_response, continuation_needed) = parse_continuation_tag(&display_content);

            // When tools interrupted streaming, pre-tool content is already
            // displayed via StreamToken events and finalized by
            // ToolCallStarted (which calls finalize_streaming_zone_as_is).
            // ToolCallStarted also transitions LlmState to ToolCall, so no
            // StreamBlockDone event is needed anymore.
            // StreamDone then carries ONLY the post-tool content.
            let post_tool_content = if let Some(ref pre_tool) = pre_tool_content {
                let pre_tool_display = strip_thinking_tags(pre_tool);
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
                thinking,
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

/// Compact conversation by summarizing old messages with streaming.
///
/// Uses a 3-layer progressive overflow strategy:
///
/// **Layer 1: Pre-pruning** — Strips long tool outputs from the middle
/// section before constructing the compaction prompt. Tool results exceeding
/// `PRUNE_TOOL_RESULT_THRESHOLD_TOKENS` estimated tokens are truncated to
/// their first `PRUNE_TOOL_RESULT_KEEP_TOKENS` estimated tokens plus a
/// truncation notice. This often reduces the prompt enough to fit the
/// model's window.
///
/// The threshold was previously `PRUNE_TOOL_RESULT_THRESHOLD = 500` chars and
/// `PRUNE_TOOL_RESULT_KEEP_CHARS = 100` chars. The threshold/keep are now
/// expressed in tokens (via `estimate_tokens` + `chars_for_tokens`) so the
/// compaction budget is honored regardless of content density.
///
/// **Layer 2: Chunked recursive summarization** — If the pre-pruned prompt
/// still exceeds the model's context window, splits the middle section into
/// chunks that each fit within `COMPACTION_MAX_CONTEXT_RATIO * context_window`.
/// Summarizes each chunk independently, then combines the summaries. If the
/// combined summaries still exceed the window, recurses (up to
/// `MAX_RECURSION_DEPTH`). Each chunk has a small overlap with the previous
/// one for coherence at boundaries.
///
/// **Layer 3: Fallback truncation** — If recursive summarization fails
/// (model unavailable, max recursion exceeded, etc.), hard-truncates oldest
/// middle messages to `context_window * TRUNCATION_TARGET_RATIO`. Always
/// preserves first `DEFAULT_KEEP_FIRST` and last `DEFAULT_KEEP_LAST`
/// messages. Logs a warning that context was forcibly truncated.
///
/// No token/character limits are imposed on the summary. The LLM is
/// instructed to preserve all relevant context via the `COMPACTION_PROMPT`.
#[allow(clippy::too_many_arguments)]
pub async fn compact_conversation(
    ollama: &crate::provider::Ollama,
    model_config: &ModelConfig,
    session: &ChatSession,
    _settings: &Settings,
    _agents_md: Option<&str>,
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
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

    // ── Layer 1: Pre-Compaction Pruning ─────────────────────────────
    //
    // Strip long tool outputs before sending to the LLM. Tool results
    // (file reads, shell outputs, search results) are often verbose and
    // low-information-density for summarization purposes.
    let pruned_messages = pre_prune_messages(&messages_to_summarize);
    let pruned_count = messages_to_summarize
        .iter()
        .zip(pruned_messages.iter())
        .filter(|(orig, pruned)| orig.content.len() != pruned.content.len())
        .count();

    if pruned_count > 0 {
        log::debug!(
            "Layer 1 (pre-pruning): truncated {}/{} tool results",
            pruned_count,
            messages_to_summarize.len()
        );
    }

    // Token overhead for compaction: system prompt, instructions, and response allowance.
    // See COMPACTION_PROMPT_OVERHEAD in context_overflow.rs for the breakdown.
    let context_window = model_config.num_ctx as usize;

    // Check if pre-pruned messages fit in context — if yes, single-pass compaction.
    // If the LLM rejects the prompt as "too long" despite our estimate, fall through
    // to Layer 2 (chunked summarization) — defense in depth against estimation errors.
    if fits_in_context(&pruned_messages, context_window, COMPACTION_PROMPT_OVERHEAD) {
        let conversation_text = build_conversation_text(&pruned_messages);
        let compact_prompt = build_compaction_prompt(&conversation_text);
        match compact_with_llm(ollama, model_config, compact_prompt, llm_tx.clone(), true).await {
            Ok(summary) => return Ok((summary, range)),
            Err(e) if is_prompt_too_long_error(&e.to_string()) => {
                log::warn!(
                    "Layer 1 single-pass compaction failed (prompt too long: {}). \
                     Estimated {} tokens with overhead {} but model rejected. \
                     Falling back to chunked summarization.",
                    e,
                    estimate_compaction_tokens(&pruned_messages),
                    COMPACTION_PROMPT_OVERHEAD,
                );
                // Fall through to Layer 2
            }
            Err(e) => return Err(e), // Non-overflow errors propagate immediately
        }
    }

    // ── Layer 2: Chunked Recursive Summarization ─────────────────────
    //
    // The pre-pruned messages don't fit in a single LLM call.
    // Split them into chunks that each fit, summarize each chunk,
    // and combine the summaries.
    log::info!(
        "Pre-pruned messages exceed context window ({} tokens > {} available). \
         Attempting chunked recursive summarization.",
        estimate_compaction_tokens(&pruned_messages),
        context_window.saturating_sub(COMPACTION_PROMPT_OVERHEAD),
    );

    let chunk_budget = max_chunk_tokens(context_window);
    let chunks = split_into_chunks(&pruned_messages, chunk_budget);

    log::debug!(
        "Layer 2 (chunked summarization): split into {} chunk(s), budget {} tokens/chunk",
        chunks.len(),
        chunk_budget,
    );

    // Report progress to the TUI as a separate system message
    let _ = llm_tx.try_send(LlmEvent::CompactInfo {
        message: format!("⚙ Compacting in {} chunk(s)...", chunks.len()),
    });

    match compact_recursive(ollama, model_config, &chunks, llm_tx.clone(), 0).await {
        Ok(summary) => {
            log::info!(
                "Layer 2 (chunked summarization): succeeded with {} chunks",
                chunks.len()
            );
            return Ok((summary, range));
        }
        Err(e) => {
            log::warn!(
                "Layer 2 (chunked summarization) failed: {}. Falling back to truncation.",
                e
            );
        }
    }

    // ── Layer 3: Fallback Truncation ────────────────────────────────
    //
    // Chunked summarization failed (model unavailable, max recursion
    // exceeded, etc.). Hard-truncate oldest middle messages to fit
    // within context_window * TRUNCATION_TARGET_RATIO.
    let truncation = fallback_truncate(
        &pruned_messages,
        context_window,
        DEFAULT_KEEP_FIRST.min(DEFAULT_KEEP_FIRST),
        0, // don't preserve last within middle (they're in "keep_last")
    );

    if truncation.dropped_count > 0 {
        log::warn!(
            "Layer 3 (fallback truncation): dropped {} oldest middle messages \
             to fit context window ({}/{:.0}% remaining).",
            truncation.dropped_count,
            truncation.remaining_tokens,
            (truncation.remaining_tokens as f32 / context_window as f32) * 100.0,
        );
        let _ = llm_tx.try_send(LlmEvent::CompactInfo {
            message: format!(
                "⚠ Truncation applied: dropped {} oldest messages to fit context window.",
                truncation.dropped_count
            ),
        });
    }

    let conversation_text = build_conversation_text(&truncation.remaining_messages);
    let compact_prompt = build_compaction_prompt(&conversation_text);

    // Layer 3 is the last resort. If even truncation fails to fit the prompt,
    // compaction is truly impossible — return a clear error with diagnostics.
    match compact_with_llm(ollama, model_config, compact_prompt, llm_tx, true).await {
        Ok(summary) => Ok((summary, range)),
        Err(e) if is_prompt_too_long_error(&e.to_string()) => {
            log::error!(
                "Layer 3 fallback truncation STILL exceeded context window. \
                 Estimated {} tokens (truncated from {}), context window {}, overhead {}. \
                 This should never happen — truncation targets {:.0}% of the window.",
                estimate_compaction_tokens(&truncation.remaining_messages),
                estimate_compaction_tokens(&pruned_messages),
                context_window,
                COMPACTION_PROMPT_OVERHEAD,
                TRUNCATION_TARGET_RATIO * 100.0,
            );
            Err(format!(
                "Compaction failed: even after truncation to {:.0}% of context, \
                 the prompt still exceeds the model's window. \
                 Original estimate: {} tokens, truncated estimate: {} tokens, \
                 context window: {} tokens. Error: {}",
                TRUNCATION_TARGET_RATIO * 100.0,
                estimate_compaction_tokens(&pruned_messages),
                estimate_compaction_tokens(&truncation.remaining_messages),
                context_window,
                e
            )
            .into())
        }
        Err(e) => Err(e),
    }
}

/// Recursively summarize message chunks.
///
/// Summarizes each chunk independently, then combines the summaries.
/// If the combined summaries still exceed the context window, recurses
/// (up to `MAX_RECURSION_DEPTH`). If recursion fails, returns an error
/// and the caller falls back to truncation.
///
/// Uses `Box::pin` for the recursive call because Rust requires
/// indirection for recursive async functions (the future size would
/// otherwise be infinite).
fn compact_recursive<'a>(
    ollama: &'a crate::provider::Ollama,
    model_config: &'a ModelConfig,
    chunks: &'a [crate::context_overflow::MessageChunk],
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
    depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<String>> + Send + 'a>> {
    Box::pin(async move {
        if depth >= MAX_RECURSION_DEPTH {
            return Err(format!(
                "Max recursion depth ({}) reached in chunked summarization",
                MAX_RECURSION_DEPTH
            )
            .into());
        }

        let mut summaries = Vec::with_capacity(chunks.len());

        for (i, chunk) in chunks.iter().enumerate() {
            let conversation_text = build_conversation_text(&chunk.messages);

            // Use a slightly different prompt for sub-summaries to encourage conciseness
            let chunk_prompt = if chunks.len() > 1 {
                format!(
                    "This is part {}/{} of a longer conversation. Summarize this section concisely.\n\n{}",
                    i + 1,
                    chunks.len(),
                    build_compaction_prompt(&conversation_text)
                )
            } else {
                build_compaction_prompt(&conversation_text)
            };

            log::debug!(
                "Layer 2: summarizing chunk {}/{} ({} tokens)",
                i + 1,
                chunks.len(),
                chunk.token_count
            );

            // Report per-chunk progress to the TUI
            let _ = llm_tx.try_send(LlmEvent::CompactInfo {
                message: format!("⚙ Compacting chunk {}/{}...", i + 1, chunks.len()),
            });

            // Intermediate chunk summaries are processed silently (stream=false).
            // Only the final consolidation pass streams to the TUI.
            match compact_with_llm(ollama, model_config, chunk_prompt, llm_tx.clone(), false).await
            {
                Ok(summary) => summaries.push(summary),
                Err(e) => {
                    log::warn!(
                        "Failed to summarize chunk {}/{}: {}",
                        i + 1,
                        chunks.len(),
                        e
                    );
                    return Err(e);
                }
            }
        }

        // If we only had one chunk, return its summary directly.
        // The summary was obtained silently (stream=false); the caller
        // (compact_conversation) will send it via CompactStreamDone
        // which renders the final text via stream_done().
        if summaries.len() == 1 {
            return Ok(summaries.swap_remove(0));
        }

        // Combine summaries and check if they fit in context
        let combined = summaries.join("\n\n---\n\n");
        let combined_tokens = crate::tokens::estimate_tokens(&combined);
        let context_window = model_config.num_ctx as usize;

        if combined_tokens + COMPACTION_PROMPT_OVERHEAD <= context_window {
            // Combined summaries fit — do a final summarization pass.
            // Stream the final consolidation so the user sees progress.
            let final_prompt = build_compaction_prompt(&combined);
            compact_with_llm(ollama, model_config, final_prompt, llm_tx, true).await
        } else {
            // Combined summaries still too large — recurse
            log::debug!(
                "Layer 2: combined summaries ({} tokens) still exceed context window, recursing (depth {})",
                combined_tokens,
                depth + 1
            );

            // Create synthetic messages from summaries for next recursion level
            let summary_messages: Vec<super::session::SavedMessage> = summaries
                .iter()
                .map(|s| super::session::SavedMessage {
                    role: super::session::MessageRole::Assistant,
                    content: s.clone(),
                    timestamp: chrono::Utc::now(),
                    ..Default::default()
                })
                .collect();

            let chunk_budget = max_chunk_tokens(context_window);
            let sub_chunks = split_into_chunks(&summary_messages, chunk_budget);

            compact_recursive(ollama, model_config, &sub_chunks, llm_tx, depth + 1).await
        }
    })
}

/// Build formatted conversation text from messages for the compaction prompt.
///
/// Formats messages as "User: ...\n", "Assistant: ...\n", etc.
/// System messages are skipped (they don't contain conversation content
/// relevant for summarization).
fn build_conversation_text(messages: &[super::session::SavedMessage]) -> String {
    let mut conversation_text = String::new();
    for msg in messages {
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
    conversation_text
}

/// Send a compaction prompt to the LLM and stream the summary back.
///
/// This is the core LLM call shared by all compaction paths (single-pass,
/// pre-pruned, truncated, and each chunk in recursive summarization).
/// Returns only the summary text; the range is determined by the caller.
/// Send a compaction prompt to the LLM and optionally stream the summary.
///
/// When `stream` is true, tokens are streamed to the TUI via `CompactStreamToken`
/// events (used for the final consolidation pass that the user sees).
/// When `stream` is false, tokens are silently discarded — the LLM still runs
/// and the summary is returned, but the TUI shows no intermediate content
/// (used for intermediate chunk summarization in recursive compaction).
async fn compact_with_llm(
    ollama: &crate::provider::Ollama,
    model_config: &ModelConfig,
    compact_prompt: String,
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
    stream: bool,
) -> AppResult<String> {
    let mut model_cfg = model_config.clone();
    model_cfg.temperature = 0.3;
    model_cfg.top_p = Some(0.9);
    let provider_options = model_cfg.build_provider_options();
    let model_options = convert_provider_to_model(&provider_options);

    let mut coordinator =
        CustomCoordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
            .options(model_options);

    let messages = vec![
        ChatMessage::system("You are a helpful assistant that summarizes conversations in clean Markdown format. Always use headers, bullets, and formatting to make the summary readable and scannable.".to_string()),
        ChatMessage::user(compact_prompt),
    ];

    // Only stream tokens to TUI for the final consolidaton pass.
    // Intermediate chunk summaries are processed silently.
    let llm_tx_token = llm_tx.clone();
    let result = coordinator
        .chat_stream(
            messages,
            move |token| {
                if stream {
                    let _ = llm_tx_token.try_send(LlmEvent::CompactStreamToken(token));
                }
                // If !stream, tokens are silently discarded — the summary
                // is still returned via Ok(summary) below.
            },
            |_thinking_token| {
                // Thinking tokens from compaction are not displayed in the chat area.
                // Compaction is an internal operation — the user only sees the summary.
            },
            || {},
            |_tool_call_id, _name, _args| {
                // Compaction does not use tools.
            },
            |_event| {
                // Compaction does not surface provider events.
            },
            None,
        )
        .await;

    match result {
        Ok(response) => {
            let summary = strip_thinking_tags(&response.message.content);
            Ok(summary)
        }
        Err(e) => Err(format!("Failed to compact: {}", e).into()),
    }
}
