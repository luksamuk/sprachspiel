# Prompt Refactoring Plan

**Created:** 2026-02-28
**Status:** Planned
**Priority:** HIGH

## Overview

This document outlines a comprehensive refactoring of the system prompts used in sprachspiel. The goal is to improve LLM behavior through better prompt engineering techniques.

## Current Problems

### 1. Negative Instructions

Current prompts use many negative instructions which LLMs often ignore or misinterpret:

```
"NEVER ask follow-up questions"
"DO NOT assume everything is Pokémon-related"
"DO NOT end your responses with conversation continuation hooks"
```

**Problem:** Models tend to focus on the action described rather than the negation.

### 2. Overly Long Tool User Prompt

- ~430 lines of dynamic code
- Mixes tool instructions with system context
- Repetitive examples for each tool category
- No clear section delimiters
- Emoji ⚠️ may tokenize poorly in some models

### 3. Malformatted Few-shot Examples

```
"Who is Sonic the Hedgehog?" → CALL web_search
```

**Problem:** Not real few-shot prompting. No input/output trajectory, just arrow notation.

### 4. Hardcoded Platform

```rust
"You are a helpful agent invoked through a command-line script on Arch Linux."
```

**Problem:** Not accurate for Termux, Ubuntu, macOS, or other platforms.

### 5. AGENTS.md Injection Position

AGENTS.md is injected in the middle of the prompt without clear hierarchy:

```
[Pepe personality]
[Base prompt]
[AGENTS.md context]
```

### 6. Token Waste

Current prompts use ~1700 tokens. With proper engineering, we can reduce this by ~65%.

---

## Recommended Practices (from Prompt Engineering Guide)

### Specificity Over Negation

| Negative | Positive Alternative |
|----------|---------------------|
| "Do NOT end with follow-up questions" | "End your response with the complete answer" |
| "Do NOT assume everything is Pokémon" | "Use web_search for general queries, Pokémon tools only for Pokémon content" |

### Clear Structure with Delimiters

```
### ROLE
[Identity definition]

### CONTEXT
[System information]

### TOOLS
[Available tools]

### EXAMPLES
[Few-shot trajectories]

### INSTRUCTIONS
[Behavior guidelines]
```

### Few-shot with Real Trajectories

Use ReAct-style examples:

```
User: What's the weather in Tokyo?
Action: get_weather(location="Tokyo")
Response: **Tokyo Weather**: Currently 23°C...

---

User: Compare Pikachu and Charizard stats
Action: fetch_pokemon_stats(pokemon_name="Pikachu")
Action: fetch_pokemon_stats(pokemon_name="Charizard")
Response: **Stat Comparison**: [table]
```

### Function Calling Best Practices

Tool descriptions are more important than prompt text for tool selection. The prompt should:

1. List available tools briefly
2. Provide categorical guidance (when to use which category)
3. Let tool definitions do the heavy lifting

---

## Decisions Made

| Decision | Choice |
|----------|--------|
| **Approach** | Moderate - Hierarchical structure with externalized tool descriptions |
| **Personality (Pepe)** | Keep separate - Prefix injected at prompt start |
| **Few-shot examples** | 5+ examples using ReAct-style trajectories |
| **Validation** | Automated benchmark tests |
| **Platform detection** | Detect actual OS/distro (Linux, Termux, macOS, etc.) |
| **AGENTS.md** | Keep + improve formatting |

---

## File Structure

```
src/
├── platform.rs              # NEW - Platform detection
├── context.rs               # MODIFY - Use PlatformInfo
├── prompts/                 # NEW DIRECTORY
│   ├── mod.rs               # Re-exports and public API
│   ├── base.rs              # SYSTEM_PROMPT_BASE, CODE, SUMMARIZE
│   ├── tools.rs             # build_tool_context()
│   ├── examples.rs          # TOOL_EXAMPLES (5+ examples)
│   ├── personality.rs       # PERSONALITY_PEPE, is_pepe_model()
│   └── builder.rs           # build_system_prompt() main function
└── prompts.rs               # DEPRECATED - stub with re-exports

tests/
└── prompt_benchmark.rs      # NEW - Comparison tests

benches/
└── prompt_comparison.rs     # NEW - Performance benchmarks
```

---

## Implementation

### Phase 1: Platform Detection

Create `src/platform.rs`:

```rust
pub enum Platform {
    Linux,
    Termux,
    MacOS,
    Windows,
    Other,
}

pub enum LinuxDistro {
    Arch,
    Ubuntu,
    Debian,
    Fedora,
    OpenSuse,
    Gentoo,
    Alpine,
    NixOS,
    Unknown,
}

pub struct PlatformInfo {
    pub platform: Platform,
    pub linux_distro: Option<LinuxDistro>,
    pub is_android: bool,
}

impl PlatformInfo {
    pub fn detect() -> Self { /* ... */ }
    pub fn prompt_string(&self) -> String { /* ... */ }
}
```

**Detection methods:**
- Termux: Check `TERMUX_VERSION` env var or `PREFIX` containing "com.termux"
- Linux distro: Read `/etc/os-release` or check `/etc/arch-release`
- macOS/Windows: Use `cfg!(target_os = "...")` compile-time detection
- Android: `cfg!(target_os = "android")`

### Phase 2: New Prompts

#### `src/prompts/base.rs`

```rust
pub const SYSTEM_PROMPT_BASE: &str = r#"
### ROLE
You are a helpful CLI assistant.

### BEHAVIOR
- Respond in the user's language
- Use available tools for current information
- Provide complete answers in a single response
- Format output in markdown

### TOOL USAGE
When you need current data:
1. Analyze what information you need
2. Call the appropriate tool
3. Use tool results to form your answer
"#;

pub const SYSTEM_PROMPT_CODE: &str = r#"
### ROLE
You are a senior developer assistant.

### BEHAVIOR
- Provide working code solutions
- Use markdown code blocks with language identifier
- Include only essential code
- Add explanations only when explicitly requested

### OUTPUT FORMAT
```language
code here
```

Return the solution directly. Brief explanation only if requested.
"#;

pub const SYSTEM_PROMPT_SUMMARIZE: &str = r#"
### ROLE
You are a professional summarization assistant.

### BEHAVIOR
- Extract main points and essential information
- Preserve technical details and proper nouns
- Use paragraphs or bullet points as appropriate
- Maintain the original language

### OUTPUT
Provide the summary directly without preamble.
"#;
```

#### `src/prompts/tools.rs`

Minimal tool context - detailed descriptions live in tool definitions:

```rust
pub fn build_tool_context(blacklist: &HashSet<&str>) -> String {
    // List available tools by category
    // Brief usage guidance per category
    // Let tool definitions handle specifics
}
```

#### `src/prompts/examples.rs`

5+ ReAct-style examples:

```rust
pub const TOOL_EXAMPLES: &str = r#"
### EXAMPLES

User: What's the weather in Tokyo?
Action: get_weather(location="Tokyo")
Response: **Tokyo Weather**: Currently 23°C, partly cloudy...

---

User: Compare Pikachu and Charizard base stats
Action: fetch_pokemon_stats(pokemon_name="Pikachu")
Action: fetch_pokemon_stats(pokemon_name="Charizard")
Response: **Stat Comparison**: [comparison table]

---

User: What is quantum computing?
Action: web_search(query="quantum computing explanation")
Response: **Quantum Computing**: [summary from results]

---

User: Show me the project structure
Action: list_directory(path=".")
Response: **Project Structure**: [file listing]

---

User: What type is Pikachu weak against?
Action: fetch_type_effectiveness(type_name="electric")
Response: **Electric Type Weaknesses**: Ground: 2x damage.

---

User: Read lines 10-20 of config.yaml
Action: count_lines(path="config.yaml")
Action: read_file_segment(path="config.yaml", start_line="10", num_lines="11")
Response: [content of lines 10-20]
"#;
```

#### `src/prompts/personality.rs`

```rust
pub const PERSONALITY_PEPE: &str = r#"
### PERSONALITY
You are Pepe - a helpful but sarcastic assistant. Help users while making light-hearted jokes about their questions. Be concise, helpful, and slightly snarky.

"#;

pub fn is_pepe_model(model_id: &str) -> bool {
    model_id.to_lowercase().contains("pepe")
}
```

#### `src/prompts/builder.rs`

Main orchestration:

```rust
pub fn build_system_prompt(config: PromptConfig) -> String {
    let mut prompt = String::new();
    
    // 1. Personality (if Pepe) - FIRST
    // 2. Role definition
    // 3. CONTEXT section (platform + system info + AGENTS.md)
    // 4. TOOLS section (if enabled)
    // 5. EXAMPLES
    // 6. FINAL INSTRUCTION
    
    prompt
}
```

### Phase 3: Tests

#### `tests/prompt_benchmark.rs`

```rust
// Token count comparison
#[test]
fn test_token_count_reduction() { /* ... */ }

// No negative instructions
#[test]
fn test_no_negative_instructions() { /* ... */ }

// Structure hierarchy
#[test]
fn test_structure_hierarchy() { /* ... */ }

// Examples present
#[test]
fn test_examples_present() { /* ... */ }

// Platform detection
#[test]
fn test_platform_detection() { /* ... */ }
```

### Phase 4: Migration

1. Create `src/platform.rs`
2. Modify `src/context.rs` - use PlatformInfo
3. Create `src/prompts/` directory with all modules
4. Modify `src/main.rs` - register modules
5. Update callers: `src/query.rs`, `src/chat/repl.rs`
6. Create `src/prompts.rs` stub for backward compatibility
7. Create tests and benchmarks
8. Run `cargo test --all`
9. Manual validation with different models

---

## Removed Items

| Item | Status |
|------|--------|
| `"Arch Linux"` hardcoded | REMOVED - Dynamic detection |
| Negative instructions (DO NOT, NEVER) | REMOVED |
| `"⚠️ CRITICAL RULES"` section | REMOVED |
| `"Question → CALL tool"` examples | REMOVED |
| `SYSTEM_PROMPT_TOOL_USER_PLACEHOLDER` | REMOVED |

---

## Expected Improvements

| Metric | Current | Target | Change |
|--------|---------|--------|--------|
| Tokens (tool_user) | ~1700 | ~600 | -65% |
| Negative instructions | 15+ | 0 | -100% |
| Few-shot examples | 0 (malformatted) | 5+ | Real examples |
| Platform accuracy | Fixed "Arch Linux" | Dynamic | Correct |

---

## Test Instructions

### Before Implementation

```bash
# See current prompt structure
cargo test --test prompt_benchmark -- --nocapture

# Specific tests
cargo test --test prompt_benchmark test_token_count_tool_user_prompt -- --nocapture
cargo test --test prompt_benchmark test_no_negative_instructions_in_new_prompts -- --nocapture
cargo test --test prompt_benchmark test_full_prompt_comparison -- --nocapture
```

### After Implementation

```bash
# All tests
cargo test --test prompt_benchmark -- --nocapture

# Verify improvements
cargo test --test prompt_benchmark test_token_count_tool_user_prompt -- --nocapture
cargo test --test prompt_benchmark test_no_negative_instructions_in_new_prompts -- --nocapture
cargo test --test prompt_benchmark test_new_prompt_structure_hierarchy -- --nocapture
cargo test --test prompt_benchmark test_few_shot_examples_present -- --nocapture
cargo test --test prompt_benchmark test_platform_detection -- --nocapture
```

### Manual Testing

```bash
# Build
cargo build

# Test tool selection
./target/debug/sprachspiel -m <model> "Tell me about Pikachu" -vv 2>&1 | grep -i "fetch_pokemon"

# Test weather
./target/debug/sprachspiel -m <model> "Weather in Paris" -vv 2>&1 | grep -i "weather"

# Test web search
./target/debug/sprachspiel -m <model> "Latest Rust news" -vv 2>&1 | grep -i "search"

# Test AGENTS.md
echo "# Test Project\nTest context." > AGENTS.md
./target/debug/sprachspiel -m <model> "What is this project?" -vv
rm AGENTS.md
```

### Termux Testing

```bash
# Verify platform detection
echo $TERMUX_VERSION    # Should return Termux version
echo $PREFIX            # Should contain /data/data/com.termux/

cargo test --test prompt_benchmark test_platform_detection -- --nocapture

# Expected output:
# Detected platform: Termux
# Is Android: true
# Prompt string: "Termux on Android"
```

---

## Checklist

- [ ] Create `src/platform.rs`
- [ ] Modify `src/context.rs`
- [ ] Create `src/prompts/mod.rs`
- [ ] Create `src/prompts/base.rs`
- [ ] Create `src/prompts/tools.rs`
- [ ] Create `src/prompts/examples.rs`
- [ ] Create `src/prompts/personality.rs`
- [ ] Create `src/prompts/builder.rs`
- [ ] Update callers in `src/query.rs`
- [ ] Update callers in `src/chat/repl.rs`
- [ ] Create `tests/prompt_benchmark.rs`
- [ ] Create backward compat stub in `src/prompts.rs`
- [ ] Run all tests: `cargo test --all`
- [ ] Manual validation with multiple models
- [ ] Update AGENTS.md if needed

---

## References

- [Prompt Engineering Guide](https://www.promptingguide.ai/)
- [Few-shot Prompting](https://www.promptingguide.ai/techniques/fewshot)
- [ReAct Prompting](https://www.promptingguide.ai/techniques/react)
- [Function Calling](https://www.promptingguide.ai/agents/function-calling)
- [General Tips for Designing Prompts](https://www.promptingguide.ai/introduction/tips)