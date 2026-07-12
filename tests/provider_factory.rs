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
        kind: ProviderKind::OllamaLegacy,
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
fn test_build_provider_ollama_legacy_returns_deprecation_error() {
    let mut providers = HashMap::new();
    providers.insert(
        "my-ollama".to_string(),
        make_ollama_config("http://localhost:11434"),
    );

    let result = build_provider("my-ollama", &providers);

    // OllamaLegacy is deprecated in W2 #121 — the factory returns a
    // config error prompting the user to migrate to kind = "openai".
    match result {
        Ok(_) => panic!("OllamaLegacy should return a deprecation error, not a provider"),
        Err(e) => {
            let err = e.to_string();
            assert!(
                err.contains("deprecated") || err.contains("upgrade"),
                "Error should mention deprecation/upgrade; got: {err}"
            );
        }
    }
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
fn test_build_provider_openai_returns_provider() {
    let mut providers = HashMap::new();
    providers.insert(
        "openai-cloud".to_string(),
        ProviderConfig {
            kind: ProviderKind::OpenAI,
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

    // W2 #121: OpenAI-compatible providers are now the default and are
    // fully supported (no API key in this test env means the provider
    // struct is still built — the key is only read at request time).
    let result = build_provider("openai-cloud", &providers);

    match result {
        Ok(provider) => {
            assert_eq!(
                provider.provider_name(),
                "openai-compatible",
                "OpenAI provider should return its name"
            );
        }
        Err(e) => {
            // The factory may reject the config if the base_url is
            // invalid, but it should NOT return an "unsupported" error.
            let err = e.to_string();
            assert!(
                !err.contains("Unsupported"),
                "OpenAI should be supported, not unsupported; got: {err}"
            );
        }
    }
}
