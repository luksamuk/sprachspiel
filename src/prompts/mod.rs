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
//! let config = PromptConfig {
//!     prompt_type: PromptType::ToolUser,
//!     model_id: Some("llama3.2:3b"),
//!     blacklist: Some(&blacklist),
//!     agents_md: Some(agents_content),
//!     tools_enabled: true,
//! };
//!
//! let prompt = build_system_prompt(config);
//! ```

pub mod base;
pub mod builder;
pub mod examples;
pub mod personality;
pub mod tools;

// Re-export commonly used items
pub use base::{SYSTEM_PROMPT_BASE, SYSTEM_PROMPT_CODE, SYSTEM_PROMPT_SUMMARIZE};
pub use builder::{
    build_code_with_tools_prompt, build_system_prompt, build_tool_user_prompt, PromptConfig,
    PromptType,
};
pub use personality::{get_personality_prefix, is_pepe_model, PERSONALITY_PEPE};
pub use tools::build_tool_context;

// Re-export legacy functions for backward compatibility
use std::collections::HashSet;

/// Legacy function - use build_system_prompt instead
#[deprecated(note = "Use build_system_prompt from builder module instead")]
pub fn get_prompt(name: &str, model_id: Option<&str>) -> Option<String> {
    get_prompt_with_blacklist(name, model_id, None, None)
}

/// Legacy function - use build_system_prompt instead
#[deprecated(note = "Use build_system_prompt from builder module instead")]
pub fn get_prompt_with_blacklist(
    name: &str,
    model_id: Option<&str>,
    blacklist: Option<&HashSet<&str>>,
    agents_md: Option<&str>,
) -> Option<String> {
    let prompt_type = match name {
        "default" | "tool_user" => PromptType::ToolUser,
        "code" => PromptType::Code,
        "code_with_tools" => PromptType::CodeWithTools,
        "summarize" => PromptType::Summarize,
        _ => return None,
    };

    let config = PromptConfig {
        prompt_type,
        model_id,
        blacklist,
        agents_md,
        tools_enabled: matches!(
            prompt_type,
            PromptType::ToolUser | PromptType::CodeWithTools
        ),
    };

    Some(build_system_prompt(config))
}

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
