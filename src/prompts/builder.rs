//! Prompt builder - main orchestration module
//!
//! Builds complete system prompts by combining:
//! 1. SOUL LAYER: User-defined personality from SOUL.md or PERSONALITY_DEFAULT
//! 2. OPERATION LAYER: Role definition and behavior
//! 3. CONTEXT LAYER: Platform, date, cwd, git, AGENTS.md
//! 4. CAPABILITY LAYER: Tools, memory, examples
//! 5. FINAL INSTRUCTION

use std::collections::HashSet;

use super::base::{
    CONTEXT_MANAGEMENT_INSTRUCTION, PERSONALITY_DEFAULT, SYSTEM_PROMPT_BASE, SYSTEM_PROMPT_CODE,
    SYSTEM_PROMPT_SUMMARIZE,
};
use super::examples::TOOL_EXAMPLES;
use super::tools::build_tool_context;
use crate::context::get_system_context;
use crate::context_overflow::ContextStatus;
use crate::platform::PlatformInfo;
use crate::soul::load_soul;

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
///
/// Use the builder pattern to construct:
/// ```ignore
/// let prompt = build_system_prompt(
///     PromptConfig::new(PromptType::ToolUser)
///         .with_model_id(Some("llama3.2"))
///         .with_tools(true)
/// );
/// ```
pub struct PromptConfig<'a> {
    /// Type of prompt to generate
    pub prompt_type: PromptType,
    /// Model ID (for personality detection) - retained for backward compatibility
    pub model_id: Option<&'a str>,
    /// Tools to exclude from the prompt
    pub blacklist: Option<&'a HashSet<&'a str>>,
    /// Optional AGENTS.md content to inject
    pub agents_md: Option<&'a str>,
    /// Whether tools are enabled for this prompt
    pub tools_enabled: bool,
    /// Whether retrieval is enabled (adds MEMORY section)
    pub retrieval_enabled: bool,
    /// Whether to skip personality (SOUL.md and PERSONALITY_DEFAULT)
    pub soulless: bool,
    /// Context status for awareness (injects usage % into prompt)
    pub context_status: Option<ContextStatus>,
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
            retrieval_enabled: false,
            soulless: false,
            context_status: None,
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

    /// Set whether retrieval is enabled
    pub fn with_retrieval(mut self, retrieval_enabled: bool) -> Self {
        self.retrieval_enabled = retrieval_enabled;
        self
    }

    /// Set whether to skip personality (SOUL.md)
    pub fn with_soulless(mut self, soulless: bool) -> Self {
        self.soulless = soulless;
        self
    }

    /// Set context status for awareness injection
    pub fn with_context_status(mut self, context_status: Option<ContextStatus>) -> Self {
        self.context_status = context_status;
        self
    }
}

/// Build a complete system prompt
///
/// This is the main entry point for generating system prompts.
/// It assembles all components in the correct order:
///
/// 1. SOUL layer (if not soulless and applicable prompt type)
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

    // 1. SOUL LAYER - Personality (if not soulless and applicable prompt type)
    // Only Default and ToolUser use SOUL.md
    let uses_soul = matches!(
        config.prompt_type,
        PromptType::Default | PromptType::ToolUser
    );
    if uses_soul && !config.soulless {
        let soul = load_soul().unwrap_or_else(|| PERSONALITY_DEFAULT.to_string());
        if !soul.is_empty() {
            prompt.push_str(&soul);
            prompt.push_str("\n\n");
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
        let blacklist = config.blacklist.cloned().unwrap_or_default();
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
        let blacklist = config.blacklist.cloned().unwrap_or_default();
        let tool_context = build_tool_context(&blacklist);
        if !tool_context.is_empty() {
            prompt.push('\n');
            prompt.push_str(&tool_context);
        }
    }

    // 4b. Memory section (if retrieval is enabled)
    if config.retrieval_enabled {
        prompt.push_str("\n### MEMORY\n");
        prompt.push_str("When <retrieved_context> appears in our conversation, ");
        prompt.push_str("it contains messages from our prior conversation. ");
        prompt.push_str("Reference them when the user asks about topics we discussed earlier.\n");
    }

    // 4c. Memory tools section (if retrieval and tools are both enabled)
    if config.retrieval_enabled && config.tools_enabled {
        prompt.push_str("\n### MEMORY TOOLS\n");
        prompt.push_str("You have access to your conversation history via the remember tool:\n\n");
        prompt.push_str("- **remember(id=\"N\")**: Get full message by ID\n");
        prompt.push_str("  Use when you see a relevant but truncated message in context.\n\n");
        prompt.push_str("- **remember(query=\"topic\")**: Search past discussions\n");
        prompt.push_str("  Use when the user asks about something not in current context.\n\n");
        prompt.push_str("**Navigation:**\n");
        prompt.push_str("Each message may include navigation fields:\n");
        prompt.push_str(
            "- `previous_message_id`: ID of the preceding message (for assistant messages)\n",
        );
        prompt.push_str("- `subsequent_messages`: Messages that follow a user question\n");
        prompt.push_str("Use these to walk through conversation history contextually.\n\n");
        prompt.push_str("**Example:**\n");
        prompt.push_str(
            "Context shows: <message id='42'><content>What about...</content></message>\n",
        );
        prompt.push_str("You think: This looks relevant but incomplete.\n");
        prompt.push_str("You call: remember(id=\"42\")\n");
        prompt.push_str("You receive: Full message content with navigation fields\n");
    }

    // 4d. Context status (if status provided and needs compaction)
    // Injected before examples so LLM is aware of context pressure
    if let Some(ref status) = config.context_status
        && status.needs_compaction()
    {
        prompt.push_str("\n### CONTEXT STATUS\n\n");
        prompt.push_str(&format!(
            "Context usage: {}% ({:.1}K / {:.0}K tokens)\n\n",
            status.usage_percent(),
            status.total_tokens() as f64 / 1000.0,
            status.max_tokens() as f64 / 1000.0
        ));

        if status.is_overflow() {
            prompt.push_str("⚠️ CRITICAL: Context window is nearly full.\n");
            prompt.push_str("If you need to perform lengthy reasoning or multiple tool calls, ");
            prompt.push_str("first inform the user, then use the continuation protocol.\n");
        } else if status.is_warning() {
            prompt.push_str("⚠️ WARNING: Context is approaching capacity.\n");
            prompt.push_str("Consider completing current tasks before starting new ones.\n");
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

        // Add context management instruction if context is critical
        if let Some(ref status) = config.context_status
            && status.is_overflow()
        {
            prompt.push('\n');
            prompt.push_str(CONTEXT_MANAGEMENT_INSTRUCTION);
        }
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

/// Legacy function - builds tool_user prompt for benchmark tests.
///
/// This function is kept for backward compatibility with the benchmark test suite
/// in `tests/prompt_benchmark.rs`. New code should use `build_system_prompt` directly.
///
/// # Why this exists
/// The benchmark tests need a simple way to generate tool_user prompts without
/// constructing a full PromptConfig. This function provides that convenience wrapper.
#[allow(dead_code)]
pub fn build_tool_user_prompt(blacklist: &OldHashSet<&str>) -> String {
    build_system_prompt(
        PromptConfig::new(PromptType::ToolUser)
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
    fn test_soul_default_or_user_soul() {
        // When SOUL.md exists, uses that; otherwise uses PERSONALITY_DEFAULT
        let prompt = build_system_prompt(PromptConfig::new(PromptType::ToolUser));

        // Should have some personality content - either SOUL.md or PERSONALITY_DEFAULT
        // PERSONALITY_DEFAULT has "### IDENTITY", user SOUL.md has "## Purpose", etc.
        let has_personality = prompt.contains("### IDENTITY")
            || prompt.contains("## Purpose")
            || prompt.contains("## Behavior");
        assert!(
            has_personality,
            "Should contain personality content from either SOUL.md or PERSONALITY_DEFAULT"
        );
    }

    #[test]
    fn test_soulless_skips_personality() {
        // When --soulless is set, no personality is added
        let prompt =
            build_system_prompt(PromptConfig::new(PromptType::ToolUser).with_soulless(true));

        // Should NOT start with personality section
        assert!(
            !prompt.contains("### IDENTITY"),
            "Soulless prompt should not have IDENTITY"
        );
        // Should start directly with ROLE
        assert!(prompt.contains("### ROLE"), "Should have ROLE section");
    }

    #[test]
    fn test_code_prompt_ignores_soul() {
        // Code prompts don't use SOUL regardless
        let prompt = build_system_prompt(PromptConfig::new(PromptType::Code));

        // Should NOT have personality
        assert!(
            !prompt.contains("### IDENTITY"),
            "Code prompt should not have IDENTITY"
        );
        // Should have ROLE directly
        assert!(prompt.contains("### ROLE"), "Should have ROLE section");
    }

    #[test]
    fn test_summarize_prompt_ignores_soul() {
        // Summarize prompts don't use SOUL regardless
        let prompt = build_system_prompt(PromptConfig::new(PromptType::Summarize));

        // Should NOT have personality
        assert!(
            !prompt.contains("### IDENTITY"),
            "Summarize prompt should not have IDENTITY"
        );
        // Should have ROLE directly
        assert!(prompt.contains("### ROLE"), "Should have ROLE section");
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
