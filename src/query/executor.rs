//! Query execution with retry logic
//!
//! Provides execute_query_with_retry to handle Ollama errors with retry logic.

use std::sync::Arc;

use indicatif::ProgressBar;
use ollama_rs::generation::chat::ChatMessage;

use crate::chat::coordinator::{
    classify_ollama_error, format_recovery_message, is_ollama_error_recoverable, MAX_RETRIES,
};
use crate::chat::custom_coordinator::CustomCoordinator;
use crate::db::Database;
use crate::debug_tools::log_debug;
use crate::embeddings::EmbeddingClient;

/// Execute a query with retry logic.
///
/// Handles recoverable Ollama errors by retrying up to MAX_RETRIES times.
/// Automatically wraps with DB context if available for the remember tool.
pub async fn execute_query_with_retry(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    db: Option<Arc<Database>>,
    embedding_client: Option<Arc<EmbeddingClient>>,
    tool_names: &[String],
    spinner: ProgressBar,
    use_debug: bool,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    if let (Some(db), Some(embedding)) = (&db, &embedding_client) {
        execute_with_context(
            coordinator,
            messages,
            db.clone(),
            embedding.clone(),
            tool_names,
            spinner,
            use_debug,
        )
        .await
    } else {
        execute_without_context(coordinator, messages, tool_names, spinner, use_debug).await
    }
}

/// Execute with DB context (for remember tool support).
async fn execute_with_context(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    db: Arc<Database>,
    embedding: Arc<EmbeddingClient>,
    tool_names: &[String],
    spinner: ProgressBar,
    use_debug: bool,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    crate::tools::context::with_context(db, embedding, async {
        execute_retry_loop(coordinator, messages, tool_names, spinner, use_debug).await
    })
    .await
}

/// Execute without DB context (code mode or anonymous).
async fn execute_without_context(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    tool_names: &[String],
    spinner: ProgressBar,
    use_debug: bool,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    execute_retry_loop(coordinator, messages, tool_names, spinner, use_debug).await
}

/// Core retry loop shared by both execution paths.
async fn execute_retry_loop(
    mut coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    tool_names: &[String],
    spinner: ProgressBar,
    use_debug: bool,
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

                    if use_debug {
                        log_debug(&format!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            MAX_RETRIES,
                            recovery_err.description()
                        ));
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