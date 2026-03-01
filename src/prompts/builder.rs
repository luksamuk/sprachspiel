//! Prompt builder - main orchestration module
//!
//! Builds complete system prompts by combining:
//! 1. Personality prefix (if applicable)
//! 2. Role definition
//! 3. Context section (platform, date, cwd, git, AGENTS.md)
//! 4. Tools section (if enabled)
//! 5. Examples (if tools enabled)
//! 6. Final instruction

use std::collections::HashSet;

use super::base::{SYSTEM_PROMPT_BASE, SYSTEM_PROMPT_CODE, SYSTEM_PROMPT_SUMMARIZE};
use super::examples::TOOL_EXAMPLES;
use super::personality::get_personality_prefix;
use super::tools::build_tool_context;
use crate::context::get_system_context;
use crate::platform::PlatformInfo;

/// Prompt type determines which base prompt to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptType {
    /// Default assistant with tools
    Default,
    /// Tool-enabled assistant
    ToolUser,
    /// Code-focused assistant (minimal explanation)
    Code,
    /// Code-focused assistant with file tools
    CodeWithTools,
    /// Summarization assistant (tools disabled)
    Summarize,
}

/// Configuration for building a system prompt
pub struct PromptConfig<'a> {
    /// Type of prompt to generate
    pub prompt_type: PromptType,
    /// Model ID (for personality detection)
    pub model_id: Option<&'a str>,
    /// Tools to exclude from the prompt
    pub blacklist: Option<&'a HashSet<&'a str>>,
    /// Optional AGENTS.md content to inject
    pub agents_md: Option<&'a str>,
    /// Whether tools are enabled for this prompt
    pub tools_enabled: bool,
}

impl<'a> PromptConfig<'a> {
    /// Create a new prompt configuration
    pub fn new(prompt_type: PromptType) -> Self {
        Self {
            prompt_type,
            model_id: None,
            blacklist: None,
            agents_md: None,
            tools_enabled: matches!(
                prompt_type,
                PromptType::ToolUser | PromptType::CodeWithTools
            ),
        }
    }

    /// Set the model ID
    pub fn with_model_id(mut self, model_id: Option<&'a str>) -> Self {
        self.model_id = model_id;
        self
    }

    /// Set the tool blacklist
    pub fn with_blacklist(mut self, blacklist: Option<&'a HashSet<&'a str>>) -> Self {
        self.blacklist = blacklist;
        self
    }

    /// Set the AGENTS.md content
    pub fn with_agents_md(mut self, agents_md: Option<&'a str>) -> Self {
        self.agents_md = agents_md;
        self
    }

    /// Set whether tools are enabled
    pub fn with_tools(mut self, tools_enabled: bool) -> Self {
        self.tools_enabled = tools_enabled;
        self
    }
}

/// Build a complete system prompt
///
/// This is the main entry point for generating system prompts.
/// It assembles all components in the correct order:
///
/// 1. Personality prefix (e.g., Pepe) - if applicable
/// 2. Role definition (from base prompt)
/// 3. Context section:
///    - Platform info (detected dynamically)
///    - System info (date, cwd, git)
///    - Project guidelines (AGENTS.md) - if provided
/// 4. Tools section - if enabled
/// 5. Examples - if tools enabled
/// 6. Final instruction
///
/// # Arguments
/// * `config` - Prompt configuration
///
/// # Returns
/// Complete system prompt string
pub fn build_system_prompt(config: PromptConfig) -> String {
    let mut prompt = String::new();

    // 1. Personality prefix (if applicable) - FIRST
    if let Some(model_id) = config.model_id {
        let personality = get_personality_prefix(Some(model_id));
        if !personality.is_empty() {
            prompt.push_str(personality);
            // Don't add newline - personality already ends with one
        }
    }

    // 2. Role definition
    let base = match config.prompt_type {
        PromptType::Summarize => SYSTEM_PROMPT_SUMMARIZE,
        PromptType::Code | PromptType::CodeWithTools => SYSTEM_PROMPT_CODE,
        PromptType::Default | PromptType::ToolUser => SYSTEM_PROMPT_BASE,
    };
    prompt.push_str(base);

    // For code_with_tools, add file tools context after role
    if config.prompt_type == PromptType::CodeWithTools {
        prompt.push_str("\n### FILE TOOLS\n");
        prompt.push_str("You have file tools to inspect the codebase:\n");
        let blacklist = config.blacklist.map(|b| b.clone()).unwrap_or_default();
        prompt.push_str(&build_file_tools_context(&blacklist));
    }

    // 3. Context section (skip for summarize and code prompts)
    if !matches!(config.prompt_type, PromptType::Summarize | PromptType::Code) {
        prompt.push_str("\n### CONTEXT\n\n");

        // 3a. Platform info
        let platform = PlatformInfo::detect();
        prompt.push_str(&format!("Platform: {}\n", platform.prompt_string()));

        // 3b. System context (date, cwd, git)
        let system_ctx = get_system_context();
        if !system_ctx.is_empty() {
            prompt.push_str(&system_ctx);
            prompt.push('\n');
        }

        // 3c. Project guidelines (AGENTS.md)
        if let Some(agents) = config.agents_md {
            prompt.push_str("\n#### Project Guidelines\n");
            prompt.push_str(agents);
            prompt.push('\n');
        }
    }

    // 4. Tools section (if enabled)
    if config.tools_enabled {
        let blacklist = config.blacklist.map(|b| b.clone()).unwrap_or_default();
        let tool_context = build_tool_context(&blacklist);
        if !tool_context.is_empty() {
            prompt.push('\n');
            prompt.push_str(&tool_context);
        }
    }

    // 5. Examples (if tools enabled)
    if config.tools_enabled {
        prompt.push_str("\n\n");
        prompt.push_str(TOOL_EXAMPLES);
    }

    // 6. Final instruction
    if !matches!(config.prompt_type, PromptType::Summarize) {
        prompt.push_str("\n### FINAL INSTRUCTION\n");
        prompt.push_str("Provide a complete response in the user's language. End when finished.\n");
    }

    prompt
}

/// Build file tools context for code_with_tools prompt
fn build_file_tools_context(blacklist: &HashSet<&str>) -> String {
    let mut lines = Vec::new();

    let file_tools = [
        ("read_file", "Read file contents"),
        (
            "read_file_segment",
            "Read file segment (requires start_line and num_lines)",
        ),
        (
            "count_lines",
            "Count lines in a file (use before reading large files)",
        ),
        ("list_directory", "List files and directories"),
        ("search_files", "Search file contents with regex"),
    ];

    for (tool, desc) in file_tools {
        if !blacklist.contains(tool) {
            lines.push(format!("- {}: {}", tool, desc));
        }
    }

    lines.join("\n")
}

// ============================================================================
// Backward compatibility layer - mirrors old prompts.rs functions
// ============================================================================

use std::collections::HashSet as OldHashSet;

/// Legacy function - builds tool_user prompt for backward compatibility
///
/// This function is used by existing code during migration.
/// Prefer `build_system_prompt` for new code.
pub fn build_tool_user_prompt(blacklist: &OldHashSet<&str>) -> String {
    build_system_prompt(
        PromptConfig::new(PromptType::ToolUser)
            .with_blacklist(Some(blacklist))
            .with_tools(true),
    )
}

/// Legacy function - builds code_with_tools prompt for backward compatibility
pub fn build_code_with_tools_prompt(blacklist: &OldHashSet<&str>) -> String {
    build_system_prompt(
        PromptConfig::new(PromptType::CodeWithTools)
            .with_blacklist(Some(blacklist))
            .with_tools(true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tool_user_prompt() {
        let blacklist = HashSet::new();
        let prompt = build_tool_user_prompt(&blacklist);

        // Should have role
        assert!(prompt.contains("### ROLE"), "Missing ROLE section");

        // Should have context
        assert!(prompt.contains("### CONTEXT"), "Missing CONTEXT section");

        // Should have platform info (dynamically detected)
        assert!(prompt.contains("Platform:"), "Missing Platform info");

        // Should have a detected platform (ANY of these is valid)
        let detected_platforms = [
            "Arch Linux",
            "Ubuntu",
            "Debian",
            "Fedora",
            "Linux",
            "Termux on Android",
            "macOS",
            "Windows",
        ];
        let has_detected_platform = detected_platforms.iter().any(|p| prompt.contains(p));
        assert!(has_detected_platform, "Should have a detected platform");

        // Should have tools
        #[cfg(feature = "weather-tools")]
        assert!(prompt.contains("WEATHER TOOLS"), "Missing WEATHER TOOLS");

        // Should have examples
        assert!(prompt.contains("### EXAMPLES"), "Missing EXAMPLES section");

        // Should have final instruction
        assert!(
            prompt.contains("### FINAL INSTRUCTION"),
            "Missing FINAL INSTRUCTION"
        );
    }

    #[test]
    fn test_build_code_prompt() {
        let prompt = build_system_prompt(PromptConfig::new(PromptType::Code));

        // Should have role
        assert!(prompt.contains("### ROLE"), "Missing ROLE section");

        // Should be concise
        assert!(
            prompt.contains("Provide working code"),
            "Missing code behavior"
        );

        // Should NOT have context (code prompts are minimal)
        assert!(
            !prompt.contains("### CONTEXT"),
            "Code prompt should not have CONTEXT"
        );
    }

    #[test]
    fn test_build_summarize_prompt() {
        let prompt = build_system_prompt(PromptConfig::new(PromptType::Summarize));

        // Should have role
        assert!(prompt.contains("### ROLE"), "Missing ROLE section");

        // Should have summarization behavior
        assert!(
            prompt.contains("Extract main points"),
            "Missing summarization behavior"
        );

        // Should NOT have tools
        assert!(
            !prompt.contains("### TOOLS"),
            "Summarize prompt should not have TOOLS"
        );

        // Should NOT have examples
        assert!(
            !prompt.contains("### EXAMPLES"),
            "Summarize prompt should not have EXAMPLES"
        );
    }

    #[test]
    fn test_no_negative_instructions() {
        let blacklist = HashSet::new();
        let prompt = build_tool_user_prompt(&blacklist);

        // Should NOT have negative instructions
        assert!(!prompt.contains("DO NOT"), "Should not contain 'DO NOT'");
        assert!(!prompt.contains("NEVER"), "Should not contain 'NEVER'");
        assert!(!prompt.contains("DON'T"), "Should not contain 'DON'T'");
    }

    #[test]
    fn test_pepe_personality() {
        let prompt = build_system_prompt(
            PromptConfig::new(PromptType::ToolUser).with_model_id(Some("pepe:8b-64k")),
        );

        // Should have Pepe personality at start
        assert!(
            prompt.starts_with("### PERSONALITY"),
            "Pepe personality should be at start"
        );
        assert!(prompt.contains("Pepe"), "Should contain Pepe");
    }

    #[test]
    fn test_agents_md_injection() {
        let agents = "Test project context\nBuild: cargo build";
        let prompt = build_system_prompt(
            PromptConfig::new(PromptType::ToolUser).with_agents_md(Some(agents)),
        );

        // Should have AGENTS.md content
        assert!(
            prompt.contains("Test project context"),
            "Missing AGENTS.md content"
        );
        assert!(
            prompt.contains("Project Guidelines"),
            "Missing Project Guidelines header"
        );
    }

    #[test]
    fn test_examples_count() {
        let prompt = build_system_prompt(PromptConfig::new(PromptType::ToolUser).with_tools(true));

        // Count example separators
        let count = prompt.matches("---").count();
        assert!(count >= 5, "Expected at least 5 examples, found {}", count);
    }
}
