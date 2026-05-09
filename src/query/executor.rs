//! Query execution with retry logic
//!
//! Provides execute_query_with_retry to handle Ollama errors with retry logic.

use std::sync::Arc;

use indicatif::ProgressBar;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;

use crate::chat::coordinator::{
    MAX_RETRIES, classify_ollama_error, format_recovery_message, is_ollama_error_recoverable,
};
use crate::chat::custom_coordinator::CustomCoordinator;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::settings::Settings;
use crate::tools::context::{with_full_context, with_tool_context};

/// Execute a query with retry logic.
///
/// Handles recoverable Ollama errors by retrying up to MAX_RETRIES times.
/// Automatically wraps with full context if available for the remember tool
/// and agent spawning tools.
#[expect(clippy::too_many_arguments)]
pub async fn execute_query_with_retry(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    db: Option<Arc<Database>>,
    embedding_client: Option<Arc<EmbeddingClient>>,
    ollama: Ollama,
    settings: Arc<Settings>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    if let (Some(db), Some(embedding)) = (&db, &embedding_client) {
        execute_with_context(
            coordinator,
            messages,
            db.clone(),
            embedding.clone(),
            ollama,
            settings,
            tool_names,
            spinner,
        )
        .await
    } else {
        execute_without_context(coordinator, messages, ollama, settings, tool_names, spinner).await
    }
}

/// Execute with DB context (for remember tool support).
#[expect(clippy::too_many_arguments)]
async fn execute_with_context(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    db: Arc<Database>,
    embedding: Arc<EmbeddingClient>,
    ollama: Ollama,
    settings: Arc<Settings>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    with_full_context(db, embedding, ollama, settings, async {
        execute_retry_loop(coordinator, messages, tool_names, spinner).await
    })
    .await
}

/// Execute without DB context (code mode or anonymous).
async fn execute_without_context(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    ollama: Ollama,
    settings: Arc<Settings>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    with_tool_context(ollama, settings, async {
        execute_retry_loop(coordinator, messages, tool_names, spinner).await
    })
    .await
}

/// Core retry loop shared by both execution paths.
async fn execute_retry_loop(
    mut coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    let mut attempts = 0;
    let mut messages = messages;

    loop {
        let current_result = coordinator.chat(messages.clone()).await;

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                if is_ollama_error_recoverable(&e) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, tool_names);
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
                        crate::spinner::finish_spinner(spinner.clone());
                        eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                    }

                    continue;
                } else {
                    break Err(e.to_string());
                }
            }
        }
    }
}
