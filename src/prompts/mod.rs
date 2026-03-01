//! Prompts module
//!
//! This module provides system prompts for different use cases.
//! The prompts are designed using prompt engineering best practices:
//!
//! - Clear hierarchical structure with ### delimiters
//! - Positive instructions instead of negative ones
//! - Few-shot examples using ReAct-style trajectories
//! - Dynamic platform detection instead of hardcoded values
//!
//! # Structure
//!
//! - `base` - Core system prompts (default, code, summarize)
//! - `tools` - Tool context builder
//! - `examples` - Few-shot examples for tool usage
//! - `personality` - Personality overlays (Pepe)
//! - `builder` - Main prompt builder function
//!
//! # Usage
//!
//! ```ignore
//! use ask_ai::prompts::builder::{build_system_prompt, PromptConfig, PromptType};
//!
//! let prompt = build_system_prompt(
//!     PromptConfig::new(PromptType::ToolUser)
//!         .with_model_id(Some("llama3.2:3b"))
//!         .with_blacklist(Some(&blacklist))
//!         .with_agents_md(Some(agents_content))
//!         .with_tools(true)
//! );
//! ```

pub mod base;
pub mod builder;
pub mod examples;
pub mod personality;
pub mod tools;

// Re-export commonly used items for external access (tests, etc.)
// These may trigger unused_imports warnings in the binary but are used by tests/prompt_benchmark.rs
#[allow(unused_imports)]
pub use base::{SYSTEM_PROMPT_BASE, SYSTEM_PROMPT_CODE, SYSTEM_PROMPT_SUMMARIZE};
#[allow(unused_imports)]
pub use builder::{build_system_prompt, build_tool_user_prompt, PromptConfig, PromptType};
#[allow(unused_imports)]
pub use personality::{get_personality_prefix, is_pepe_model, PERSONALITY_PEPE};
#[allow(unused_imports)]
pub use tools::build_tool_context;

/// List all available prompt names
pub fn list_prompts() -> Vec<&'static str> {
    vec![
        "default",
        "tool_user",
        "code",
        "code_with_tools",
        "summarize",
        "pepe",
    ]
}
