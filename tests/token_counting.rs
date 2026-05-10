//! Comprehensive tests for the tokens module
//!
//! Tests token estimation, message overhead, and context metrics.

use ollama_rs::generation::chat::ChatMessage;
use sprachspiel::tokens::{
    ContextMetrics, MESSAGE_OVERHEAD, calculate_context_metrics, count_messages_tokens,
    estimate_tokens, estimate_tokens_code,
};

#[test]
fn test_estimate_tokens_empty() {
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn test_estimate_tokens_single_word() {
    let tokens = estimate_tokens("hello");
    assert!(
        (1..=3).contains(&tokens),
        "Expected 1-3 tokens for single word, got {}",
        tokens
    );
}

#[test]
fn test_estimate_tokens_sentence() {
    let tokens = estimate_tokens("Hello world this is a test");
    assert!(
        (5..=10).contains(&tokens),
        "Expected 5-10 tokens for sentence, got {}",
        tokens
    );
}

#[test]
fn test_estimate_tokens_code() {
    let code = "fn main() { println!(\"hello\"); }";
    let tokens = estimate_tokens_code(code);
    assert!(
        tokens > 0,
        "Code should have positive token count, got {}",
        tokens
    );
    assert!(
        tokens >= estimate_tokens(code),
        "Code estimation should be >= text estimation"
    );
}

#[test]
fn test_message_overhead() {
    assert_eq!(MESSAGE_OVERHEAD, 4);
}

#[test]
fn test_count_messages_tokens() {
    let messages = vec![
        ChatMessage::user("Hello".to_string()),
        ChatMessage::assistant("Hi there".to_string()),
    ];
    let total = count_messages_tokens(&messages);
    let expected_per_msg = MESSAGE_OVERHEAD;
    let user_tokens = estimate_tokens("Hello") + expected_per_msg;
    let assistant_tokens = estimate_tokens("Hi there") + expected_per_msg;
    assert_eq!(total, user_tokens + assistant_tokens);
}

#[test]
fn test_context_metrics() {
    let metrics = ContextMetrics {
        system_tokens: 100,
        tools_tokens: 50,
        history_tokens: 200,
        total_tokens: 350,
        context_window: 4096,
        utilization: 0.085,
    };
    assert_eq!(metrics.system_tokens, 100);
    assert_eq!(metrics.tools_tokens, 50);
    assert_eq!(metrics.history_tokens, 200);
    assert_eq!(metrics.total_tokens, 350);
    assert_eq!(metrics.context_window, 4096);
}

#[test]
fn test_context_metrics_utilization() {
    let metrics = ContextMetrics {
        system_tokens: 1000,
        tools_tokens: 500,
        history_tokens: 2596,
        total_tokens: 4096,
        context_window: 4096,
        utilization: 1.0,
    };
    assert!((metrics.utilization - 1.0).abs() < 0.001);
}

#[test]
fn test_context_metrics_available() {
    let metrics = ContextMetrics {
        system_tokens: 100,
        tools_tokens: 50,
        history_tokens: 200,
        total_tokens: 350,
        context_window: 4096,
        utilization: 0.085,
    };
    assert_eq!(metrics.available(), 3746);
}

#[test]
fn test_context_metrics_available_overflow() {
    let metrics = ContextMetrics {
        system_tokens: 1000,
        tools_tokens: 500,
        history_tokens: 4000,
        total_tokens: 5500,
        context_window: 4096,
        utilization: 1.0,
    };
    assert_eq!(metrics.available(), 0);
}

#[test]
fn test_calculate_context_metrics_basic() {
    let messages = vec![
        ChatMessage::user("hello world".to_string()),
        ChatMessage::assistant("hi there".to_string()),
    ];
    let metrics = calculate_context_metrics(&messages, 4096, "You are helpful.", 100, None);
    assert!(metrics.system_tokens > 0);
    assert!(metrics.tools_tokens > 0);
    assert!(metrics.history_tokens > 0);
    assert!(metrics.total_tokens > 0);
    assert_eq!(metrics.context_window, 4096);
}

#[test]
fn test_estimate_tokens_multiple_spaces() {
    let tokens = estimate_tokens("hello    world   test");
    assert!(
        (3..=6).contains(&tokens),
        "Expected 3-6 tokens, got {}",
        tokens
    );
}

#[test]
fn test_estimate_tokens_code_empty() {
    assert_eq!(estimate_tokens_code(""), 0);
}

#[test]
fn test_estimate_tokens_code_complex() {
    let code = r#"
fn calculate(x: i32) -> i32 {
    if x > 0 { x * 2 } else { x / 2 }
}
"#;
    let tokens = estimate_tokens_code(code);
    assert!(tokens > 0, "Complex code should have positive token count");
}

#[test]
fn test_count_messages_tokens_empty() {
    let messages: Vec<ChatMessage> = Vec::new();
    assert_eq!(count_messages_tokens(&messages), 0);
}

#[test]
fn test_count_messages_tokens_single() {
    let messages = vec![ChatMessage::user("hello world".to_string())];
    let tokens = count_messages_tokens(&messages);
    assert!(tokens >= MESSAGE_OVERHEAD);
}

#[test]
fn test_utilization_calculation() {
    let metrics = calculate_context_metrics(&[], 1000, "test", 0, None);
    let ratio = metrics.total_tokens as f32 / metrics.context_window as f32;
    assert!((metrics.utilization - ratio).abs() < 0.001);
}

#[test]
fn test_available_tokens_calculation() {
    let messages = vec![ChatMessage::user("test".to_string())];
    let metrics = calculate_context_metrics(&messages, 4096, "system prompt", 0, None);
    assert_eq!(
        metrics.available(),
        metrics.context_window.saturating_sub(metrics.total_tokens)
    );
}
