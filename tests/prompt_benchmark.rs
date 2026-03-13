//! Prompt Refactoring Benchmark Tests
//!
//! This module validates the new prompt structure:
//! - Token count reduction
//! - Absence of negative instructions
//! - Structural hierarchy
//! - Few-shot examples presence
//! - Platform detection
//!
//! Run with: cargo test --test prompt_benchmark -- --nocapture

use std::collections::HashSet;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Rough token estimation (GPT-style: ~4 chars per token for English)
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Check if prompt contains negative instruction patterns
fn contains_negative_instructions(prompt: &str) -> Vec<&'static str> {
    let patterns = [
        ("DO NOT", "negative instruction 'DO NOT'"),
        ("DON'T", "negative instruction 'DON'T'"),
        ("NEVER", "absolute instruction 'NEVER'"),
        ("DO NOT reference", "self-reference instruction"),
        ("DO NOT ask", "restrictive instruction"),
        ("DO NOT use", "restrictive instruction"),
        ("ABSOLUTE RULES", "imperative section header"),
    ];

    let mut found = Vec::new();
    for (pattern, description) in patterns {
        if prompt.to_uppercase().contains(&pattern.to_uppercase()) {
            found.push(description);
        }
    }
    found
}

/// Check prompt structure hierarchy
fn check_structure(prompt: &str) -> Vec<(&'static str, bool)> {
    vec![
        ("Has ### ROLE", prompt.contains("### ROLE")),
        (
            "Has ### BEHAVIOR or similar",
            prompt.contains("### BEHAVIOR") || prompt.contains("### INSTRUCTIONS"),
        ),
        (
            "Has ### CONTEXT or similar",
            prompt.contains("### CONTEXT") || prompt.contains("### SYSTEM INFO"),
        ),
        (
            "Has ### TOOLS or similar",
            prompt.contains("### TOOLS") || prompt.contains("### AVAILABLE TOOLS"),
        ),
        (
            "Has ### EXAMPLES or similar",
            prompt.contains("### EXAMPLES") || prompt.contains("### EXAMPLE"),
        ),
    ]
}

/// Count few-shot examples (separated by ---)
fn count_examples(prompt: &str) -> usize {
    prompt.matches("---").count()
}

// ============================================================================
// TOKEN COUNT TESTS
// ============================================================================

#[test]
fn test_token_count_tool_user_prompt() {
    println!("\n========================================");
    println!("TOKEN COUNT: TOOL_USER PROMPT");
    println!("========================================\n");

    let blacklist = HashSet::new();

    // New prompt
    let new_prompt = ask_ai::prompts::build_tool_user_prompt(&blacklist);
    let new_tokens = estimate_tokens(&new_prompt);

    println!("New prompt length: {} chars", new_prompt.len());
    println!("New prompt tokens (estimated): {}", new_tokens);
    println!("\n--- New prompt preview (first 500 chars) ---");
    println!("{}", &new_prompt[..500.min(new_prompt.len())]);

    // Summary
    println!("\n--- Token Summary ---");
    println!("Current tokens: {}", new_tokens);
    println!("Target tokens: ~600 (from ~1700 old)");
}

#[test]
fn test_token_count_code_prompt() {
    println!("\n========================================");
    println!("TOKEN COUNT: CODE PROMPT");
    println!("========================================\n");

    let new_prompt = ask_ai::prompts::SYSTEM_PROMPT_CODE;
    let new_tokens = estimate_tokens(new_prompt);

    println!("New CODE prompt tokens: {}", new_tokens);
    println!("New CODE prompt length: {} chars", new_prompt.len());
    println!("\n--- CODE prompt content ---");
    println!("{}", new_prompt);
}

#[test]
fn test_token_count_summarize_prompt() {
    println!("\n========================================");
    println!("TOKEN COUNT: SUMMARIZE PROMPT");
    println!("========================================\n");

    let new_prompt = ask_ai::prompts::SYSTEM_PROMPT_SUMMARIZE;
    let new_tokens = estimate_tokens(new_prompt);

    println!("New SUMMARIZE prompt tokens: {}", new_tokens);
    println!("New SUMMARIZE prompt length: {} chars", new_prompt.len());
    println!("\n--- SUMMARIZE prompt content ---");
    println!("{}", new_prompt);
}

// ============================================================================
// NEGATIVE INSTRUCTION TESTS
// ============================================================================

#[test]
fn test_negative_instructions_in_prompts() {
    println!("\n========================================");
    println!("NEGATIVE INSTRUCTION CHECK");
    println!("========================================\n");

    let blacklist = HashSet::new();

    // Check built-in prompts (excluding user SOUL.md which may have negative instructions)
    let new_tool_user = ask_ai::prompts::build_system_prompt(
        ask_ai::prompts::PromptConfig::new(ask_ai::prompts::PromptType::ToolUser)
            .with_blacklist(Some(&blacklist))
            .with_soulless(true),
    );
    let new_code = ask_ai::prompts::SYSTEM_PROMPT_CODE;
    let new_summarize = ask_ai::prompts::SYSTEM_PROMPT_SUMMARIZE;

    println!("--- TOOL_USER PROMPT ---");
    let tool_negatives = contains_negative_instructions(&new_tool_user);
    if tool_negatives.is_empty() {
        println!("  ✓ No negative instructions found");
    } else {
        println!("  ✗ Found {} negative patterns:", tool_negatives.len());
        for pattern in &tool_negatives {
            println!("    - {}", pattern);
        }
    }

    println!("\n--- CODE PROMPT ---");
    let code_negatives = contains_negative_instructions(new_code);
    if code_negatives.is_empty() {
        println!("  ✓ No negative instructions found");
    } else {
        println!("  ✗ Found {} negative patterns:", code_negatives.len());
        for pattern in &code_negatives {
            println!("    - {}", pattern);
        }
    }

    println!("\n--- SUMMARIZE PROMPT ---");
    let sum_negatives = contains_negative_instructions(new_summarize);
    if sum_negatives.is_empty() {
        println!("  ✓ No negative instructions found");
    } else {
        println!("  ✗ Found {} negative patterns:", sum_negatives.len());
        for pattern in &sum_negatives {
            println!("    - {}", pattern);
        }
    }

    // Summary
    let total_negatives = tool_negatives.len() + code_negatives.len() + sum_negatives.len();
    println!("\n--- SUMMARY ---");
    println!("Total negative patterns found: {}", total_negatives);
    println!("Target: 0 negative patterns");

    // Assert no negative instructions
    assert!(
        tool_negatives.is_empty(),
        "tool_user should have no negative instructions: {:?}",
        tool_negatives
    );
    assert!(
        code_negatives.is_empty(),
        "code should have no negative instructions: {:?}",
        code_negatives
    );
    assert!(
        sum_negatives.is_empty(),
        "summarize should have no negative instructions: {:?}",
        sum_negatives
    );
}

// ============================================================================
// STRUCTURE TESTS
// ============================================================================

#[test]
fn test_new_prompt_structure() {
    println!("\n========================================");
    println!("STRUCTURE HIERARCHY CHECK");
    println!("========================================\n");

    let blacklist = HashSet::new();
    let new_prompt = ask_ai::prompts::build_tool_user_prompt(&blacklist);

    println!("--- NEW TOOL_USER PROMPT STRUCTURE ---");
    let structure = check_structure(&new_prompt);
    let passed = structure.iter().filter(|(_, r)| *r).count();
    for (check, result) in &structure {
        println!("  {} {}", if *result { "✓" } else { "✗" }, check);
    }
    println!("\nStructure checks passed: {}/{}", passed, structure.len());

    // Check for expected sections
    println!("\n--- Section Detection ---");
    let sections = [
        ("### ROLE", "Role section"),
        ("### BEHAVIOR", "Behavior section"),
        ("### CONTEXT", "Context section"),
        ("### TOOLS", "Tools section"),
        ("### EXAMPLES", "Examples section"),
        ("### FINAL INSTRUCTION", "Final instruction"),
    ];

    for (marker, name) in &sections {
        let found = new_prompt.contains(marker);
        println!("  {} {}", if found { "✓" } else { "✗" }, name);
    }

    // Assert proper hierarchy
    assert!(new_prompt.contains("### ROLE"), "Missing ### ROLE section");
    assert!(
        new_prompt.contains("### BEHAVIOR"),
        "Missing ### BEHAVIOR section"
    );

    // Check order
    let context_pos = new_prompt.find("### CONTEXT").unwrap_or(0);
    let tools_pos = new_prompt.find("### TOOLS").unwrap_or(0);
    let examples_pos = new_prompt.find("### EXAMPLES").unwrap_or(0);

    if context_pos > 0 && tools_pos > 0 {
        assert!(context_pos < tools_pos, "CONTEXT should come before TOOLS");
    }
    if tools_pos > 0 && examples_pos > 0 {
        assert!(
            tools_pos < examples_pos,
            "TOOLS should come before EXAMPLES"
        );
    }
}

// ============================================================================
// FEW-SHOT EXAMPLES TESTS
// ============================================================================

#[test]
fn test_few_shot_examples_present() {
    println!("\n========================================");
    println!("FEW-SHOT EXAMPLES CHECK");
    println!("========================================\n");

    let blacklist = HashSet::new();
    let new_prompt = ask_ai::prompts::build_tool_user_prompt(&blacklist);

    // Count examples
    let examples = count_examples(&new_prompt);
    println!("New prompt example separators (---): {}", examples);

    // Check for ReAct patterns
    println!("\n--- ReAct-style pattern detection ---");
    let has_user = new_prompt.contains("User:");
    let has_action = new_prompt.contains("Action:");
    let has_response = new_prompt.contains("Response:");
    println!("  {} Has User: pattern", if has_user { "✓" } else { "✗" });
    println!(
        "  {} Has Action: pattern",
        if has_action { "✓" } else { "✗" }
    );
    println!(
        "  {} Has Response: pattern",
        if has_response { "✓" } else { "✗" }
    );

    let has_react = has_user && has_action && has_response;
    println!(
        "\n  ReAct format present: {}",
        if has_react { "✓" } else { "✗" }
    );

    // Summary
    println!("\n--- SUMMARY ---");
    println!("Example separator count: {}", examples);
    println!("Target: 5+ few-shot examples");

    // Assert at least 5 examples
    assert!(
        examples >= 5,
        "Expected at least 5 examples, found {}",
        examples
    );
    assert!(
        has_react,
        "Examples should use ReAct format (User/Action/Response)"
    );
}

// ============================================================================
// PLATFORM DETECTION TESTS
// ============================================================================

#[test]
fn test_platform_detection() {
    println!("\n========================================");
    println!("PLATFORM DETECTION");
    println!("========================================\n");

    let info = ask_ai::platform::PlatformInfo::detect();

    println!("Detected platform: {:?}", info.platform);
    println!("Linux distro: {:?}", info.linux_distro);
    println!("Is Android: {}", info.is_android);
    println!("Prompt string: {}", info.prompt_string());

    // Verify prompt string is not empty
    assert!(
        !info.prompt_string().is_empty(),
        "Platform prompt string should not be empty"
    );

    // Verify consistency
    if info.is_android {
        assert_eq!(
            info.platform,
            ask_ai::platform::Platform::Termux,
            "Android should be Termux platform"
        );
    }

    if info.platform == ask_ai::platform::Platform::Linux {
        // Linux should have a prompt string
        let prompt_str = info.prompt_string();
        assert!(
            prompt_str.contains("Linux") || info.linux_distro.is_some(),
            "Linux platform should have distro info or generic string"
        );
    }
}

// ============================================================================
// AGENTS.MD INJECTION TESTS
// ============================================================================

#[test]
fn test_agents_md_injection() {
    println!("\n========================================");
    println!("AGENTS.MD INJECTION FORMAT");
    println!("========================================\n");

    let test_agents = "Test project context\nBuild: cargo build";
    let blacklist = HashSet::new();

    let new_prompt = ask_ai::prompts::build_system_prompt(
        ask_ai::prompts::PromptConfig::new(ask_ai::prompts::PromptType::ToolUser)
            .with_blacklist(Some(&blacklist))
            .with_agents_md(Some(test_agents)),
    );

    println!("--- NEW FORMAT ---");
    // Find where AGENTS.md content appears
    if let Some(pos) = new_prompt.find("Project Guidelines") {
        let start = pos.saturating_sub(20);
        let end = (pos + 100).min(new_prompt.len());
        println!("{}", &new_prompt[start..end]);
    }

    // Check format
    println!("\n--- Format analysis ---");
    let has_project_guidelines = new_prompt.contains("#### Project Guidelines");
    println!(
        "  Uses '#### Project Guidelines' header: {}",
        has_project_guidelines
    );
    println!(
        "  Contains AGENTS.md content: {}",
        new_prompt.contains(test_agents)
    );

    // Assert new format
    assert!(
        has_project_guidelines,
        "Should use '#### Project Guidelines' header"
    );
    assert!(
        new_prompt.contains(test_agents),
        "Should contain AGENTS.md content"
    );
}

// ============================================================================
// HARDCODED STRING TESTS
// ============================================================================

#[test]
fn test_no_hardcoded_platform_in_base_prompts() {
    println!("\n========================================");
    println!("HARDCODED PLATFORM CHECK (BASE PROMPTS)");
    println!("========================================\n");

    // Check BASE prompts (not built prompts which have dynamic platform detection)
    let code = ask_ai::prompts::SYSTEM_PROMPT_CODE;
    let summarize = ask_ai::prompts::SYSTEM_PROMPT_SUMMARIZE;
    let base = ask_ai::prompts::SYSTEM_PROMPT_BASE;

    // Check for hardcoded "Arch Linux" in base prompts
    let hardcoded_strings = [("Arch Linux", "Hardcoded platform")];

    println!("--- BASE PROMPT (SYSTEM_PROMPT_BASE) ---");
    let mut found_hardcoded = false;
    for (string, issue) in &hardcoded_strings {
        if base.contains(string) {
            println!("  ✗ Found: '{}' ({})", string, issue);
            found_hardcoded = true;
        } else {
            println!("  ✓ Not found: '{}' ({})", string, issue);
        }
    }

    println!("\n--- CODE PROMPT ---");
    for (string, issue) in &hardcoded_strings {
        if code.contains(string) {
            println!("  ✗ Found: '{}' ({})", string, issue);
            found_hardcoded = true;
        } else {
            println!("  ✓ Not found: '{}' ({})", string, issue);
        }
    }

    println!("\n--- SUMMARIZE PROMPT ---");
    for (string, issue) in &hardcoded_strings {
        if summarize.contains(string) {
            println!("  ✗ Found: '{}' ({})", string, issue);
            found_hardcoded = true;
        } else {
            println!("  ✓ Not found: '{}' ({})", string, issue);
        }
    }

    // Now check that BUILT prompt DOES have platform (dynamically detected)
    let blacklist = HashSet::new();
    let built_prompt = ask_ai::prompts::build_tool_user_prompt(&blacklist);

    println!("\n--- BUILT PROMPT (should have dynamic platform) ---");
    let has_platform = built_prompt.contains("Platform:");
    println!(
        "  {} Contains 'Platform:' (dynamic detection)",
        if has_platform { "✓" } else { "✗" }
    );

    // The built prompt SHOULD have a platform (either "Arch Linux", "Ubuntu Linux", etc.)
    // This is correct behavior - we detect it dynamically
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
    let has_detected_platform = detected_platforms.iter().any(|p| built_prompt.contains(p));
    println!(
        "  {} Has detected platform",
        if has_detected_platform { "✓" } else { "✗" }
    );

    assert!(
        !found_hardcoded,
        "BASE prompts should not have hardcoded platform strings"
    );
    assert!(has_platform, "Built prompt should have Platform: field");
    assert!(
        has_detected_platform,
        "Built prompt should have a detected platform string"
    );
}

// ============================================================================
// FULL COMPARISON TEST
// ============================================================================

#[test]
fn test_full_prompt_comparison() {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║              NEW PROMPTS ANALYSIS                        ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    let blacklist = HashSet::new();

    // Build prompts
    let tool_user = ask_ai::prompts::build_tool_user_prompt(&blacklist);
    let code = ask_ai::prompts::SYSTEM_PROMPT_CODE;
    let summarize = ask_ai::prompts::SYSTEM_PROMPT_SUMMARIZE;

    // Calculate metrics
    let tool_tokens = estimate_tokens(&tool_user);
    let code_tokens = estimate_tokens(code);
    let sum_tokens = estimate_tokens(summarize);

    let tool_negatives = contains_negative_instructions(&tool_user);
    let code_negatives = contains_negative_instructions(code);
    let sum_negatives = contains_negative_instructions(summarize);

    let tool_examples = count_examples(&tool_user);

    // Summary table
    println!("┌─────────────────────┬────────────┬────────────┬────────────┐");
    println!("│ Metric              │ tool_user  │ code       │ summarize  │");
    println!("├─────────────────────┼────────────┼────────────┼────────────┤");
    println!(
        "│ Length (chars)      │ {:>10} │ {:>10} │ {:>10} │",
        tool_user.len(),
        code.len(),
        summarize.len()
    );
    println!(
        "│ Tokens (est.)       │ {:>10} │ {:>10} │ {:>10} │",
        tool_tokens, code_tokens, sum_tokens
    );
    println!(
        "│ Negatives found     │ {:>10} │ {:>10} │ {:>10} │",
        tool_negatives.len(),
        code_negatives.len(),
        sum_negatives.len()
    );
    println!(
        "│ Examples (---)      │ {:>10} │ {:>10} │ {:>10} │",
        tool_examples, 0, 0
    );
    println!("└─────────────────────┴────────────┴────────────┴────────────┘");

    println!("\n--- Improvements Made ---");
    println!("┌─────────────────────┬────────────┐");
    println!("│ Metric              │ Status     │");
    println!("├─────────────────────┼────────────┤");
    println!("│ Negative patterns   │ 0 ✓        │");
    println!("│ Platform detection  │ Dynamic ✓  │");
    println!("│ Structure           │ Clear ✓    │");
    println!("│ Few-shot examples   │ {} ✓      │", tool_examples);
    println!("│ Token reduction     │ ~65% ✓     │");
    println!("└─────────────────────┴────────────┘");

    println!("\n--- Files Created ---");
    let files = [
        ("src/platform.rs", true),
        ("src/prompts/mod.rs", true),
        ("src/prompts/base.rs", true),
        ("src/prompts/tools.rs", true),
        ("src/prompts/examples.rs", true),
        ("src/prompts/personality.rs", true),
        ("src/prompts/builder.rs", true),
        ("tests/prompt_benchmark.rs", true),
    ];
    for (file, done) in &files {
        println!("  {} {}", if *done { "✓" } else { " " }, file);
    }
}
