//! Integration tests for `provider::factory::build_provider`.
//!
//! These tests validate the factory function end-to-end without requiring
//! a running Ollama instance. The provider chain is exercised via the
//! provider's identifier, which does not perform network I/O.

use std::collections::HashMap;

use sprachspiel::provider::factory::build_provider;
use sprachspiel::user_models::{ProviderConfig, ProviderKind};

fn make_ollama_config(base_url: &str) -> ProviderConfig {
    ProviderConfig {
        kind: ProviderKind::Ollama,
        base_url: base_url.to_string(),
        connect_timeout_secs: 5,
        read_timeout_secs: 300,
        stream_idle_timeout_secs: 60,
        max_retries: 3,
        retry_base_delay_ms: 2000,
        retry_max_delay_ms: 16000,
        retry_jitter_percent: 20,
        api_key_env: None,
    }
}

#[test]
fn test_build_provider_ollama_returns_provider() {
    let mut providers = HashMap::new();
    providers.insert("my-ollama".to_string(), make_ollama_config("http://localhost:11434"));

    let provider = build_provider("my-ollama", &providers).expect("factory should succeed");

    assert_eq!(provider.provider_name(), "ollama");
}

#[test]
fn test_build_provider_unknown_name_errors() {
    let providers = HashMap::<String, ProviderConfig>::new();

    let result = build_provider("nonexistent", &providers);

    match result {
        Ok(_) => panic!("Unknown provider name should error"),
        Err(e) => {
            let err = e.to_string();
            assert!(
                err.contains("nonexistent"),
                "Error should mention the missing provider name; got: {err}"
            );
        }
    }
}

#[test]
fn test_build_provider_openai_compatible_returns_unsupported() {
    let mut providers = HashMap::new();
    providers.insert(
        "openai-cloud".to_string(),
        ProviderConfig {
            kind: ProviderKind::OpenAICompatible,
            base_url: "https://api.openai.com/v1".to_string(),
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
            api_key_env: Some("OPENAI_API_KEY".to_string()),
        },
    );

    let result = build_provider("openai-cloud", &providers);

    match result {
        Ok(_) => panic!("OpenAI-compatible should be unimplemented"),
        Err(e) => {
            let err = e.to_string();
            assert!(
                err.contains("OpenAICompatible") || err.contains("OpenAI"),
                "Error should mention OpenAI; got: {err}"
            );
        }
    }
}
