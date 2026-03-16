# Agent Guidelines for ask-ai

This is a Rust project that uses the ollama-rs library to interact with Ollama LLM models.

**IMPORTANT: See `IMPLEMENTATION.md` for the detailed feature roadmap and implementation phases.**

## ⚠️ Pull Request Process (MANDATORY)

**Read `doc/src/development/PR-PROCESS.md` for the complete workflow.**

### Critical Rules

1. **NEVER close issues before PR merge** - Issues close automatically when PR is merged
2. **NEVER move cards to "Done"** - Only the REVIEWER moves cards to Done after approval
3. **ALWAYS create PR as DRAFT first** - Implement, then mark "ready for review"
4. **ALWAYS move card to "In Review"** - After creating PR

### Workflow Summary

```
1. Create branch     → git checkout -b feat/feature-name
2. Move to In Progress → update GitHub Project card
3. Update docs        → CHANGELOG.md, IMPLEMENTATION.md (mark as IN PROGRESS)
4. Implement         → code changes
5. Test               → cargo test --all-features && cargo clippy
6. Commit             → conventional commits (feat:, fix:, refactor:)
7. Push               → git push -u origin branch
8. Create DRAFT PR    → gh pr create --draft
9. Move to In Review  → update GitHub Project card
10. Mark ready         → gh pr ready PR_NUMBER
11. Review iteration   → fetch ALL comments, respond to each, implement fixes
12. WAIT for review   → do not merge or close until approved
```

### Status Flow

```
Todo → In Progress → In Review → Done
           ↑            ↑          ↑
       (you start)  (PR created) (REVIEWER ONLY)
```

### Review Iteration Phase

**CRITICAL:** When responding to review comments:

1. **Fetch ALL unresolved threads** using `last: 50` (not `first: 30`)
2. **Respond to EACH thread individually** - not in a single summary comment
3. **Use proper prefixes** in responses:
   - ✅ **Resolvido** - Code fixed/removed
   - ✅ **Verificado** - Code is correct as-is
   - 📋 **Acknowledged, deferred** - Good suggestion, future work
   - ❌ **Declined** - With explanation
   - ❓ **Clarification needed** - Question about the comment

4. **If implementation changes needed:**
   - Create todo list overview
   - Wait for user confirmation
   - Implement approved changes
   - Update documentation
   - Push changes

5. **Loop until all comments resolved:**
   - Check for unresolved comments again
   - If found → respond and implement
   - If none → inform user and wait for approval

## Build Commands

```bash
# Build the project
cargo build

# Build release
cargo build --release

# Build with specific features (see Features section)
cargo build --release --features pokemon-tools
cargo build --release --features all-tools

# Run the application
cargo run

# Run a specific test
cargo test --test <test_name>
cargo test <pattern>

# Run all tests
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Lint with Clippy
cargo clippy -- -D warnings
```

## Compilation Features

Tools are organized into feature flags for modular compilation:

### Default Features

The following features are enabled by default:

- `weather-tools` - Weather lookup tools (3 tools)
- `file-tools` - File system operations (5 tools)
- `pokemon-tools` - Pokémon data tools (9 tools)
- `calc-tools` - Mathematical calculator (1 tool)
- `serper-tools` - Serper API web search (2 tools, requires `SERPER_API_KEY`)
- `system-tools` - System information tools (2 tools)

### Optional Features

- `search-tools` - DuckDuckGo web search (3 tools, disabled by default, may fail due to CAPTCHA)
- `finance-tools` - Stock quotes (planned, disabled by default)
- `all-tools` - Enable all tool categories

### Feature Usage in Code

When adding new tools, wrap them with feature flags:

```rust
// In src/tools/mod.rs
#[cfg(feature = "my-tools")]
pub mod my_tool;

#[cfg(feature = "my-tools")]
pub use my_tool::*;

// In src/tools/my_tool.rs
#[cfg(feature = "my-tools")]
#[ollama_rs::function]
pub async fn my_tool() -> Result<String, ...> {
    // implementation
}

// In src/main.rs when registering tools
#[cfg(feature = "my-tools")]
{
    if is_tool_allowed("my_tool") {
        coordinator = coordinator.add_tool(my_tool);
        tool_count += 1;
    }
}
```

### Feature Flags in Prompts

Prompts dynamically include only available tools:

```rust
// In src/prompts.rs
#[cfg(feature = "pokemon-tools")]
if !pokemon_enabled.is_empty() {
    prompt.push_str("Pokémon Tools section...");
}
```

This ensures the model only sees tools that are:
1. Compiled in (feature flag enabled)
2. Not blacklisted at runtime

### Testing with Features

```bash
# Test with all features
cargo test --features all-tools

# Test without optional features (default build)
cargo test

# Test specific feature combination
cargo test --features "weather-tools,file-tools"
```

## Code Style Guidelines

### Formatting
- Use `cargo fmt` before committing
- Run `cargo clippy` to catch common mistakes
- Maximum line length: 100 characters (enforced by rustfmt)

### Imports
- Group imports: std library, external crates, local modules
- Use crate-level imports rather than deep paths when possible
- Example:
  ```rust
  use std::collections::HashMap;
  use ollama_rs::Ollama;
  use ollama_rs::coordinator::Coordinator;
  ```

### Types
- Prefer type aliases for commonly used Result types:
  ```rust
  type AppResult<T> = Result<T, Box<dyn std::error::Error + Sync + Send>>;
  ```
- Use `async/await` for async operations
- Prefer owned types (String) over references for struct fields

### Naming Conventions
- **Functions/Variables**: snake_case
- **Types/Structs/Enums**: PascalCase
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case
- Boolean fields: use clear predicates (e.g., `tools`, `vision`, `ocr`)

### Error Handling
- Use the `?` operator for error propagation
- Use `AppResult<T>` type alias for function returns
- For user-facing errors, provide clear error messages

### Async Patterns
- Use `#[tokio::main]` for the main function
- Prefer async functions when dealing with I/O
- Use `.await?` for chaining async operations

### Struct Definitions
- Derive common traits: `Debug`, `Clone` as needed
- Public fields for data structures:
  ```rust
  #[derive(Debug, Clone)]
  struct ModelInfo {
      pub name: String,
      pub architecture: String,
      pub tools: bool,
      pub vision: bool,
      pub ocr: bool,
  }
  ```

### Function Style
- Use doc comments (`///`) for public functions
- Use closures for simple helper functions
- Keep functions focused on a single responsibility

### Ollama Integration
- Use the `Coordinator` pattern for chat sessions
- Add tools conditionally based on model capabilities
- Set appropriate model options (temperature, context window)

## Project Structure

```
ask-ai/
├── Cargo.toml      # Dependencies and package info
├── src/
│   └── main.rs     # Application entry point
└── target/         # Build output
```

## Dependencies

Key crates:
- `ollama-rs`: Ollama integration
- `tokio`: Async runtime
- `reqwest`: HTTP client
- `futures`: Async utilities

## Code Deduplication Guidelines

When implementing new features or modifying existing code, always check for duplication:

### Before Adding New Code

1. **Search for existing implementations** - Use `grep` or `rg` to find similar patterns
2. **Check shared utilities** - Look in `src/utils.rs` for common functions
3. **Review related modules** - Similar functionality may exist elsewhere

### Shared Utilities (`src/utils.rs`)

The following utilities are available and should be used instead of creating duplicates:

```rust
// Boolean parsing from strings
parse_bool(value: Option<&str>, default: bool) -> bool

// U32 parsing with fallback
parse_u32(value: Option<&str>, default: Option<u32>) -> Option<u32>

// Number parsing with bounds
parse_bounded_number(value: Option<&str>, default: usize, max: Option<usize>) -> usize

// Human-readable file sizes
format_size(bytes: u64) -> String  // Returns "512 B", "1 KB", "1.5 MB"

// Stdin reading
read_stdin() -> Result<String, String>

// String capitalization
capitalize(s: &str) -> String
```

### Tool Registration (`src/tools/registry.rs`)

All tool registration is centralized. When adding a new tool:
1. Add the tool function in the appropriate module (e.g., `src/tools/weather.rs`)
2. Register it in `register_tools()` in `src/tools/registry.rs`
3. Add the tool name to `get_available_tool_names()` if needed

### Common Patterns to Avoid Duplicating

1. **Model configuration building** - Use `ModelConfig::build_model_options()`
2. **Capability detection** - Use `ModelCapabilities::detect_or_default()`
3. **Thinking display** - Use `display_thinking()`
4. **Model resolution** - Use `resolve_model_config()`
5. **Think mode validation** - Use `resolve_think_mode()`
6. **Model switching** - Use `model_switch::switch_model()` (see below)

### Model Switching - SINGLE POINT OF FAILURE

**CRITICAL:** All model switching MUST go through `src/chat/model_switch.rs`.

The `switch_model()` function is the ONLY place that handles:
- Model validation
- Config resolution
- Capability detection
- Think/tools state adjustment
- Warning generation

```rust
// ✅ CORRECT - Use the centralized function
match super::model_switch::switch_model(
    name,
    &ollama,
    &capabilities,
    session.think,
    session.tools,
).await {
    Ok(result) => {
        session.set_model(result.model_name.clone());
        session.think = result.think_active;
        session.tools = result.tools_active;
        // ... update other state
    }
    Err(e) => eprintln!("{}", e),
}

// ❌ WRONG - Never duplicate this logic
if !user_models::is_model_valid(name) { ... }
let config = user_models::resolve_model_config(name);
let caps = ModelCapabilities::detect(...).await;
// ... etc
```

**Why this matters:**
- Prevents inconsistent state between `session.think`, `session.tools`, and `tools_active`
- Ensures capabilities are always detected and warnings are consistent
- Single place to fix bugs related to model switching

### When to Create New Shared Utilities

Create a new utility in `src/utils.rs` when:
- The same code appears 2+ times in different files
- The code is a pure function with no external dependencies
- The code could be useful in multiple contexts

### `#[allow(dead_code)]` Policy

Only use `#[allow(dead_code)]` with justification:

**Acceptable reasons:**
- JSON deserialization fields (required by serde)
- Error enum variants (for completeness/extensibility)
- Public API methods (for library completeness)
- Test-only code with `#[cfg(test)]`

**Not acceptable:**
- "Might be useful later" without concrete plan
- Dead code that should be removed
- "Preparation for future features" - implement when needed, not before

### TUI Preparation Code Policy

**CRITICAL: Do not declare unused code "for future TUI implementation".**

When implementing TUI (ratatui.rs) in the future:
1. Implement code when the feature is actually being worked on
2. The `InputBackend` and `ChatView` traits already exist as abstractions
3. Add methods to traits only when they are needed
4. Review and remove any dead code after TUI is implemented

**Current TUI abstractions:**
- `src/chat/input/mod.rs` - `InputBackend` trait for input handling
- `src/chat/view/mod.rs` - `ChatView` trait for output rendering
- `src/chat/input/rustyline.rs` - `RustylineInput` implementation (in use)
- `src/chat/view/terminal.rs` - `TerminalView` implementation (in use)

These abstractions are documented and ready for TUI migration. **Do not add more unused code.**

Before adding `#[allow(dead_code)]`, verify the code is truly unused:
```bash
cargo clippy 2>&1 | grep "never used\|never constructed"
```

## Constants and String Management

### CRITICAL: No Hardcoded String Duplicates

**String literals must NEVER be duplicated.** When a string value is used in multiple places, 
it MUST be defined once and referenced everywhere.

### String Constants Module

All string constants are centralized in `src/consts/`:

- `src/consts/roles.rs` - Message role constants (`ROLE_USER`, `ROLE_ASSISTANT`, etc.)
- `src/consts/api.rs` - API URLs (`OPEN_METEO_BASE`, `SERPER_API_URL`, etc.)

### Categories of Duplicated Strings

#### 1. Source Type Prefixes

**NEVER hardcode source type prefixes like `"msg"`, `"doc"`, `"note"`, `"web"`.**

```rust
// ❌ WRONG - Hardcoded prefix
format!("msg:{}", id)
text.push_str("Use remember(id=\"msg:N\")...\n");

// ✅ CORRECT - Use SourceType::prefix()
let prefix = SourceType::Conversation.prefix();
format!("{}:{}", prefix, id)
text.push_str(&format!("Use remember(id=\"{}:N\")...\n", prefix));
```

**Source of truth:** `src/db/operations.rs` - `SourceType::prefix()` and `SourceType::from_prefix()`

#### 2. Message Roles

**NEVER hardcode role strings like `"user"`, `"assistant"`, `"system"`, `"tool"`.**

```rust
// ❌ WRONG - Hardcoded role
if msg.role == "user" { ... }
db.insert_message(&id, "assistant", &content, now);

// ✅ CORRECT - Use constants from src/consts/roles.rs
use crate::consts::roles::*;
if msg.role == ROLE_USER { ... }
db.insert_message(&id, ROLE_ASSISTANT, &content, now);

// ✅ CORRECT - Use MessageRole enum when type-safe
use crate::chat::session::MessageRole;
match role {
    MessageRole::User => { ... }
    MessageRole::Assistant => { ... }
}
```

#### 3. Role Display Labels

**NEVER duplicate role display labels.** Use the central functions.

```rust
// ❌ WRONG - Duplicated in multiple files
let role_label = match msg.role.as_str() {
    "user" => "👤 User",
    "assistant" => "🤖 Assistant",
    // ...
};

// ✅ CORRECT - Use central function from src/consts/roles.rs
use crate::consts::roles::format_role_label;
let role_label = format_role_label(&msg.role);

// For Markdown bold format
use crate::consts::roles::format_role_label_md;
let label = format_role_label_md("user"); // "👤 **User**"
```

#### 4. API URLs and Endpoints

**NEVER hardcode API URLs directly in code.**

```rust
// ❌ WRONG - Hardcoded URL
const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";

// ✅ CORRECT - Use centralized constants from src/consts/api.rs
use crate::consts::api::OPEN_METEO_GEOCODING;
```

### String Constants Location

When adding new constants:

1. **Source type prefixes** → Add to `SourceType` enum in `src/db/operations.rs`
2. **Message roles** → Add to `src/consts/roles.rs`
3. **API URLs** → Add to `src/consts/api.rs`
4. **Other strings** → Add new submodule to `src/consts/` or use existing constants

### When to Use Constants vs. Functions vs. Enums

**Use constants for:**
- String literals that are pure values (URLs, identifiers, labels)
- Values that don't change based on context

**Use functions for:**
- Formatted strings that depend on input parameters
- Display logic (like `format_role_label()`)

**Use enums for:**
- Values with associated data or methods
- State that needs pattern matching (like `SourceType`, `MessageRole`)

### Checklist Before Adding String Literals

Before committing code with string literals, verify:

1. **Search for duplicates:** `grep -r "literal_string" src/`
2. **Check if enum/method exists:** Does `SourceType`, `MessageRole`, etc. already provide this?
3. **Check consts module:** Is there already a constant in `src/consts/` for this?
4. **Consider future use:** Will this string be needed elsewhere? Create a constant.

### `#[allow(dead_code)]` Rejection on Constants

**REJECT `#[allow(dead_code)]` on newly created constants without explicit justification.**

If you create a constant for consistency but it's currently unused:
1. **Don't create it yet** - YAGNI (You Ain't Gonna Need It)
2. **Wait until it's needed in multiple places**
3. **Then create it and use it everywhere**

The only exception: constants that are part of a documented enum pattern (like `SourceType::prefix()`) where unused variants exist for API completeness.

### Enforcement

Before any PR merge, run these checks:
```bash
# Check for duplicated role strings
rg '"user"|"assistant"|"system"|"tool"' src/ --type rust

# Check for duplicated source prefixes
rg '"msg"|"doc"|"note"' src/ --type rust | grep -v 'const\|prefix()'

# Check for dead_code annotations on constants
rg '#\[allow\(dead_code\)\]' src/consts/
```

If duplicates found: refactor before merging.

## Tool Development Guidelines

When creating or modifying tools, follow these principles:

### Error Handling Philosophy

**Tools must NEVER crash the application.** Instead, return informative error messages as strings that help the LLM understand what went wrong and recover.

### CRITICAL: No `?` operator or `Err()` returns in tools

The `?` operator and `Err()` returns will propagate errors and crash the entire tool execution. **This must NEVER happen.**

```rust
// ❌ NEVER DO THIS - Will crash on error
let metadata = std::fs::metadata(&path)?;
let content = std::fs::read_to_string(&path)?;
let parsed = some_str.parse::<u32>()?;

// ✅ ALWAYS DO THIS - Returns helpful error to LLM
let metadata = match std::fs::metadata(&path) {
    Ok(m) => m,
    Err(e) => {
        let err_msg = format!("Error: Cannot read file metadata: {}", e);
        log_tool_result("my_tool", &err_msg);
        return Ok(err_msg);
    }
};

let content = match std::fs::read_to_string(&path) {
    Ok(c) => c,
    Err(e) => {
        let err_msg = format!("Error: Cannot read file: {}", e);
        log_tool_result("my_tool", &err_msg);
        return Ok(err_msg);
    }
};

let parsed = match some_str.parse::<u32>() {
    Ok(n) => n,
    Err(_) => {
        let err_msg = format!("Error: '{}' is not a valid number.", some_str);
        log_tool_result("my_tool", &err_msg);
        return Ok(err_msg);
    }
};
```

### When can errors crash?

Only truly catastrophic errors that should stop the ENTIRE APPLICATION (not just the tool) should use `?` or `Err()`. Examples:
- Application startup failures
- Configuration loading errors
- Database connection failures in the main app

**Tools are NOT the place for catastrophic error handling.** Tools should ALWAYS return `Ok(String)` with either success or error message.

### Tool Error Categories

1. **User input errors** - File not found, invalid regex, invalid parameters
   - Return helpful message with suggestions
   - Include examples of correct usage when possible

2. **API/Network errors** - Timeout, rate limit, service unavailable
   - Return error with "try again later" message
   - Don't retry automatically in tool code

3. **System errors** - Permission denied, out of memory
   - Return error with context
   - These are rare but should still be handled gracefully

### Error Example

```rust
// ❌ BAD - Crashes on error
if !canonical_path.exists() {
    return Err(format!("File not found: {}", path).into());
}

// ✅ GOOD - Returns helpful error message
if !canonical_path.exists() {
    let err_msg = format!(
        "Error: File not found: {}. Please check if the file exists or try a different file name (e.g., README.org instead of README.md).",
        path
    );
    log_tool_result("read_file", &err_msg);
    return Ok(err_msg);
}
```

### Logging Debug Output

Always log tool calls and results for debug mode:

```rust
use crate::debug_tools::{log_tool_call, log_tool_result};

pub async fn my_tool(param: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("my_tool", &[("param".to_string(), param.clone())]);
    
    // ... do work ...
    
    let result = "...";
    log_tool_result("my_tool", &result);
    Ok(result)
}
```

### Optional Parameters

Show "all" or default values in debug output, not empty strings:

```rust
log_tool_call(
    "read_file",
    &[
        ("path".to_string(), path.clone()),
        ("max_lines".to_string(), max_lines.map(|l| l.to_string()).unwrap_or_else(|| "all".to_string())),
    ],
);
```

### CRITICAL: Parameter Types for LLM Tools

**LLMs often send parameters as strings instead of proper JSON types.** This causes deserialization failures that crash tools.

#### The Problem

When an LLM generates tool calls, it may send:
- `"5"` instead of `5` (string instead of number)
- `"null"` instead of `null` (string literal instead of JSON null)
- `""` instead of `null` (empty string instead of omitting the parameter)

#### WRONG: Numeric Types in Tool Parameters

```rust
// ❌ NEVER USE numeric types for optional parameters
#[function]
pub async fn my_tool(
    path: String,
    max_lines: Option<usize>,      // ❌ Will fail if LLM sends "100" or "null"
    timeout: Option<u32>,          // ❌ Will fail if LLM sends "30" or "null"
) -> Result<String, ...>
```

**Why this fails:**
- `"100"` (string) cannot deserialize to `Option<usize>` → serde error → tool crashes
- `"null"` (string) cannot deserialize to `Option<u32>` → serde error → tool crashes
- The LLM sees "Error calling tool" with no useful feedback

#### CORRECT: String Types with Internal Parsing

```rust
// ✅ ALWAYS USE String for optional parameters, parse internally
#[function]
pub async fn my_tool(
    path: String,
    max_lines: Option<String>,      // ✅ Accepts "100", 100, "null", null
    timeout: Option<String>,       // ✅ Accepts "30", 30, "null", null
) -> Result<String, ...> {
    // Parse strings safely - returns None for invalid input
    let max_lines_val: Option<usize> = max_lines.as_deref().and_then(|m| m.parse().ok());
    let timeout_val: Option<u32> = timeout.as_deref().and_then(|t| t.parse().ok());
    
    // Use parsed values
    let lines = max_lines_val.unwrap_or(usize::MAX);
    // ...
}
```

#### Required Numeric Parameters

For parameters that MUST be provided and MUST be numeric, still use `String` but validate early:

```rust
// ✅ String type with validation
#[function]
pub async fn my_tool(
    path: String,
    start_line: String,     // Required, but String to accept LLM variations
    num_lines: String,      // Required, but String to accept LLM variations
) -> Result<String, ...> {
    // Validate required parameters early
    let start: usize = start_line.parse()
        .map_err(|_| format!("Error: Invalid start_line '{}'. Must be a positive number.", start_line))?;
    
    if start == 0 {
        let err_msg = "Error: start_line must be 1 or greater. Line numbers start at 1.".to_string();
        log_tool_result("my_tool", &err_msg);
        return Ok(err_msg);
    }
    // ...
}
```

#### Existing Tools Using This Pattern

| Tool | File | Parameters |
|------|------|------------|
| `web_search` | `src/tools/serper.rs` | `num_results: Option<String>` |
| `get_current_weather` | `src/tools/weather.rs` | `latitude: Option<String>`, `longitude: Option<String>` |
| `run_command` | `src/tools/run_cmd.rs` | `head: Option<String>`, `tail: Option<String>`, `timeout_seconds: Option<String>` |

#### NEVER Use These Types in Tool Parameters

| Type | Why It's Dangerous | Use Instead |
|------|-------------------|-------------|
| `Option<usize>` | Fails on `"100"` or `"null"` | `Option<String>` |
| `Option<u32>` | Fails on `"30"` or `"null"` | `Option<String>` |
| `Option<i32>` | Fails on `"-1"` or `"null"` | `Option<String>` |
| `usize` (required) | Fails on `"5"` or empty string | `String` + validate |
| `bool` (optional) | May work, but `Option<String>` is safer | `Option<String>` → `parse_bool()` |

#### Summary

1. **Always use `String` or `Option<String>` for numeric parameters**
2. **Parse internally with `.parse().ok()` or utility functions**
3. **Validate required parameters early and return helpful errors**
4. **Use existing patterns from `web_search`, `run_command`, etc.**

### Required Parameters

For parameters that MUST be provided (like `read_file_segment`'s `start_line` and `num_lines`):

```rust
// Validate required parameters early
let start_line_parsed = parse_u32(Some(start_line.clone()), None)
    .ok_or_else(|| format!("Error: Invalid start_line '{}'. Must be a positive number.", start_line))?;

if start_line_parsed == 0 {
    let err_msg = "Error: start_line must be 1 or greater. Line numbers start at 1.".to_string();
    log_tool_result("read_file_segment", &err_msg);
    return Ok(err_msg);
}
```

### File Size Output

Always show file sizes in human-readable format (KB/MB), not raw bytes. LLMs think in lines and file sizes, not byte counts:

```rust
let kb = metadata.len() as f64 / 1024.0;
let size_info = if kb >= 1024.0 {
    format!(" ({:.1} MB)", kb / 1024.0)
} else {
    format!(" ({:.0} KB)", kb)
};
```

### API Response Structs

When deserializing API responses, ALWAYS make fields optional with `#[serde(default)]`:

```rust
// ❌ BAD - Will crash if API doesn't return this field
#[derive(Deserialize)]
struct ApiResponse {
    data: Vec<Item>,
    metadata: Metadata,
}

// ✅ GOOD - Handles missing fields gracefully
#[derive(Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    data: Vec<Item>,
    #[serde(default)]
    metadata: Metadata,
}
```

**Why?** Different API endpoints return different fields. For example:
- `get_current_weather` doesn't return `daily` data
- `get_weather_forecast` doesn't return `current` data
- Using a shared struct requires all fields to be optional

### Network Requests in Tools

Always wrap network requests with proper error handling:

```rust
// ❌ BAD - Will crash on network error
let response = client.get(&url).send().await?;
let data: MyStruct = response.json().await?;

// ✅ GOOD - Returns helpful error message
let response = match client.get(&url).send().await {
    Ok(r) => r,
    Err(e) => {
        let err = format!("Network error: {}. Please try again later.", e);
        log_tool_result("my_tool", &err);
        return Ok(err);
    }
};

let data: MyStruct = match response.json().await {
    Ok(d) => d,
    Err(e) => {
        let err = format!("Error parsing response: {}. Please try again later.", e);
        log_tool_result("my_tool", &err);
        return Ok(err);
    }
};
```

### Common Tool Bugs Checklist

When reviewing or creating tools, check for these common issues:

1. **Missing `log_tool_call`** at the start of the function
2. **Missing `log_tool_result`** before every return
3. **Using `?` operator** instead of match for error handling
4. **Using `Err()` returns** instead of `Ok(error_message)`
5. **Non-optional struct fields** for API responses
6. **Missing error handling** for network requests
7. **Missing error handling** for JSON parsing
8. **Missing error handling** for file operations

### Tool Output Language

**All tool output must be in English.** This ensures consistency with the rest of the application and makes the LLM's job easier. Error messages, result formatting, and descriptions should all be in English.

```rust
// ✅ GOOD - English output
let result = format!("**Weather in {}**\nTemperature: {}°C", location_name, temp);

// ❌ BAD - Localized output
let result = format!("**Clima em {}**\nTemperatura: {}°C", location_name, temp);
```

### Tool Documentation (Docstrings)

**Well-documented tools help LLMs understand what each tool does and how to use it correctly.** Poor documentation leads to incorrect tool calls and wasted tokens.

#### Docstring Structure

Every tool function MUST have a docstring with:
1. **One-line summary** - What the tool does (imperative mood)
2. **Extended description** - When to use, what it returns
3. **Parameter documentation** - Name, type, description, examples

#### Example: Good Docstring

```rust
/// Search the web using Google via Serper.dev API.
///
/// Returns search results with title, URL, and snippet for each result.
/// Use this tool when you need to find current information on the internet.
///
/// # Arguments
/// * `query` - The search query (what to search for). Be specific for better results.
///   - Example: "Rust async programming best practices" instead of just "rust async"
/// * `num_results` - Number of results to return (default: 5, max: 10). Optional.
///
/// # Returns
/// Formatted search results with titles, URLs, and snippets.
/// Returns error message if SERPER_API_KEY is not set or if the API fails.
///
/// # Example
/// ```ignore
/// web_search("Python pandas dataframe merge".to_string(), Some("3".to_string()))
/// ```
#[function]
pub async fn web_search(query: String, num_results: Option<String>) -> Result<String, ...>
```

#### Example: Bad Docstring

```rust
/// Fetch basic information about a Pokémon.
///
/// * pokemon_name - The name of the Pokémon in lowercase.
#[function]
pub async fn fetch_pokemon_basic(pokemon_name: String) -> Result<String, ...>
```

#### Docstring Guidelines

1. **First line**: Capital letter, period at end, imperative mood ("Search the web", not "Searches the web")
2. **When to use**: Mention specific use cases in the extended description
3. **Parameters**: Document each parameter with:
   - What it accepts (type constraints, format)
   - Default value if optional
   - Example values
4. **Returns**: Describe the format of successful output
5. **Errors**: Mention common error conditions
6. **Examples**: Show realistic function call examples
7. **Keep it concise**: LLMs need to read this every time they consider using the tool

#### Parameter Documentation Format

```rust
/// * `parameter_name` - Brief description. More details if needed.
///   - Accepts: "value1", "value2", or "value3"
///   - Default: value1
///   - Example: "value2"
```

#### Current Tool Documentation Status

| Tool Category | Tools | Documentation Quality |
|--------------|-------|----------------------|
| weather-tools | 3 tools | ⚠️ Needs improvement |
| file-tools | 8 tools | ✅ Good (write tools well documented) |
| pokemon-tools | 9 tools | ⚠️ Minimal docstrings |
| calc-tools | 1 tool | ⚠️ Needs improvement |
| serper-tools | 2 tools | ✅ Good |
| system-tools | 2 tools | ⚠️ Needs improvement |
| search-tools | 3 tools | ⚠️ Needs improvement |

### File Write Tools Security

File write operations have additional security requirements that MUST be followed:

**Mandatory Security Measures:**

1. **Always sandboxed** - `sandbox=false` parameter is IGNORED for write operations
2. **Blocked patterns** - Sensitive files are always blocked, regardless of configuration
3. **Size limits** - Maximum 5MB per write operation
4. **UTF-8 only** - Binary content is rejected
5. **Atomic writes** - Use temp file + rename pattern to prevent corruption

**Blocked patterns (always blocked):**

```
# Environment files
".env", ".env.local", ".env.development", ".env.production", ".env.staging"

# Secrets and credentials
"secrets", "credentials", "secrets.json", "credentials.json", "secrets.yaml"

# SSH keys
"id_rsa", "id_dsa", "id_ed25519", "id_ecdsa"

# Certificates and keys
".pem", ".key"

# SSH directory
".ssh", "authorized_keys", "known_hosts"

# GPG
".gnupg"

# Cloud credentials
"credentials.json", "service-account.json"
```

**Implementation requirements for write tools:**

```rust
// MUST call validate_write_path() before ANY write operation
fn validate_write_path(path: &Path, sandbox: bool) -> Result<PathBuf, String> {
    // 1. Canonicalize path (resolve symlinks)
    // 2. Enforce sandbox (ignore sandbox=false for writes)
    // 3. Check is_blocked_path()
    // 4. Verify parent directory exists
    // 5. Verify write permissions
}

// MUST check blocked patterns after path resolution
fn is_blocked_path(path: &Path) -> bool {
    // Check against DEFAULT_BLOCKED_PATTERNS
    // Check against configured blocked_patterns
    // Case-insensitive matching
}

// MUST use atomic write pattern
fn atomic_write(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, content)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
}
```

**When implementing new write tools:**

- Always call `validate_write_path()` before operations
- Check `is_blocked_path()` after path resolution
- Implement atomic writes to prevent corruption
- Return meaningful error messages following AGENTS.md guidelines
- Log operations in debug mode with `log_tool_call()` and `log_tool_result()`

**Error message guidelines for write tools:**

```rust
// ✅ GOOD - Clear, actionable error
let err_msg = format!(
    "Error: '{}' matches a blocked pattern and cannot be written to. \
     Blocked patterns protect sensitive files like secrets, keys, and credentials.",
    filename
);

// ✅ GOOD - Helpful guidance
let err_msg = format!(
    "Error: File '{}' already exists. Use overwrite=true to replace it.",
    path
);

// ❌ BAD - Vague error
let err_msg = format!("Error: Cannot write file: {}", e);

// ❌ BAD - Leaks sensitive information
let err_msg = format!("Error: Cannot write to {}: {}", path, detailed_io_error);
```

## Notes

- The project is a CLI tool for interacting with local Ollama models
- Model capabilities (tools, vision, ocr) are detected at runtime
- Supports custom tools defined via `#[ollama_rs::function]` macro

## Documentation Guidelines

When implementing new features or modifying behavior, always update the documentation:

### User Documentation (doc/src/)

1. **New Commands**: Add a new page in `doc/src/commands/`
   - Follow the structure of existing command docs
   - Include synopsis, description, options, and examples
   - Add entry to `doc/src/SUMMARY.md`

2. **New Options/Flags**: Update the relevant command documentation
   - Add option to the options table
   - Include example usage
   - Update the man page if applicable

3. **New Models**: Update `doc/src/models.md`
   - Add to model table
   - Include configuration details
   - Document best use cases

4. **New Tools**: Update `doc/src/tools.md`
   - Document the tool function and arguments
   - Include usage examples
   - Note any limitations or known issues

5. **Behavior Changes**: Update relevant documentation
   - Change descriptions if behavior differs
   - Update examples to reflect new behavior
   - Add migration notes if breaking change

### Man Page (man/ask-ai.1)

Update when CLI interface changes:
- New commands
- New flags/options
- New models
- New examples

### Development Documentation

Update when architecture changes:
- `doc/src/development/architecture.md` - Design decisions
- `doc/src/development/roadmap.md` - Planned features
- `doc/src/CHANGELOG.md` - Version history

### Documentation Checklist

Before committing changes:

- [ ] Updated relevant doc pages in `doc/src/`
- [ ] Updated man page (if CLI changed)
- [ ] Updated CHANGELOG.md
- [ ] Ran `mdbook serve` and verified rendering
- [ ] All internal links work
- [ ] Examples tested and working

### Building Documentation

```bash
# Navigate to doc directory
cd doc

# Serve locally for testing
mdbook serve

# Build static site
mdbook build

# Output will be in doc/book/
```

### Writing Guidelines

- **User-facing docs**: Focus on functionality, not implementation
- **Keep it concise**: Clear and to the point
- **Include examples**: Show, don't just tell
- **Use Mermaid**: For diagrams and flows
- **Test examples**: Ensure they actually work

## Release Process

### Creating a Release

1. **Update version** in:
   - `Cargo.toml` - `version` field
   - `man/ask-ai.1` - `.TH` line (version number)
   - `doc/src/CHANGELOG.md` - Add new version section

2. **Update CHANGELOG** with all changes since last release

3. **Commit and push**:
   ```bash
   git add Cargo.toml Cargo.lock man/ask-ai.1 doc/src/CHANGELOG.md
   git commit -m "chore: bump version to X.Y.Z"
   git push origin master
   ```

4. **Create tarballs**:
   ```bash
   make all-tarballs
   ```
   
   This creates:
   - `dist/ask-ai-X.Y.Z-linux-x86_64.tar.gz` - Default features
   - `dist/ask-ai-X.Y.Z-linux-x86_64-all-tools.tar.gz` - All features
   - `dist/ask-ai-X.Y.Z-termux-aarch64-linux-android.tar.gz` - Termux default
   - `dist/ask-ai-X.Y.Z-termux-aarch64-linux-android-all-tools.tar.gz` - Termux all tools

5. **Create tag and release**:
   ```bash
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   git push origin vX.Y.Z
   
   gh release create vX.Y.Z \
     --title "vX.Y.Z" \
     --notes "## Changes..." \
     dist/ask-ai-X.Y.Z-linux-x86_64.tar.gz \
     dist/ask-ai-X.Y.Z-linux-x86_64-all-tools.tar.gz \
     dist/ask-ai-X.Y.Z-termux-aarch64-linux-android.tar.gz \
     dist/ask-ai-X.Y.Z-termux-aarch64-linux-android-all-tools.tar.gz
   ```

### Release Tarball Contents

Each tarball includes:
- **Linux**: Binary, man page (`ask-ai.1`), `README.md`, `LICENSE.txt`
- **Termux**: Binary, `README-TERMUX.txt` with installation instructions

### Documentation Philosophy

Documentation should:
- Be **functional** (what it does, not how it works internally)
- Be **discoverable** (clear organization)
- Be **complete** (cover all features)
- Be **maintained** (updated with code changes)
- Include **examples** (practical usage)

## Project Management

### GitHub Project Board

The project uses a GitHub Project board for task tracking. When working on tasks:

**When starting a task:**
1. Find the issue on the Project board
2. Update the issue's status to "In Progress"
3. Assign yourself to the issue if appropriate

**When completing a task:**
1. Update the issue's status to "Done"
2. Close the issue with a reference to the commit/PR
3. Update `IMPLEMENTATION.md` status for the feature

**When a task is blocked:**
1. Update the issue's status to reflect current state
2. Add a comment explaining the blocker
3. Add the `status:blocked` label if not already present

### Updating Roadmap

**CRITICAL:** After completing ANY roadmap item, update `IMPLEMENTATION.md`:

1. **Find the relevant section** in `IMPLEMENTATION.md`
2. **Update status markers:**
   - `❌ NOT STARTED` → `✅ COMPLETED` or `📋 IN PROGRESS`
   - `🟡 PLANNED` → `🟢 ACTIVE` (when work begins)
   - Update version number for completed work
3. **For completed phases:** Add implementation summary with:
   - Key files modified
   - Commits (with short hash)
   - Any deviations from original plan
4. **Move completed items** from "Priority Roadmap" to appropriate version history

**Example status update:**

```markdown
### ✅ PRIORITY 1: Feature Name (COMPLETED)

**Status:** ✅ COMPLETED (v0.32.0)

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Implementation | ✅ Done |
| 2 | Testing | ✅ Done |

**Implementation:**
- Created `src/tools/new_tool.rs`
- Updated `src/tools/mod.rs`
- Commits: `abc1234 feat: add new tool`
```

### Issue Management Guidelines

**Creating new issues:**
- Use descriptive titles with priority tag: `[P#] Feature Name`
- Add appropriate labels:
  - `priority:critical`, `priority:high`, `priority:medium`, `priority:low`
  - `status:planned`, `status:in-progress`, `status:blocked`
  - `enhancement` or `bug`
  - Phase labels if applicable: `phase:1`, `phase:2`, `phase:3`
- Reference related issues in description
- Reference `IMPLEMENTATION.md` section when applicable

**Closing issues:**
- Reference the commit that resolves the issue: `Fixes #123`
- Update roadmap status before closing
- Ensure documentation is updated

### Board Columns (Scrum Status)

| Column | Meaning |
|--------|---------|
| Backlog | Not yet prioritized, needs review |
| Ready | Prioritized, ready to start |
| In Progress | Currently being worked on |
| In Review | PR submitted or awaiting review |
| Done | Completed and verified |

## Pull Request Review Process

### Before Responding to Reviews

1. **Fetch the latest code state** - Ensure local code matches the PR branch
2. **Read the changed files** - Understand context before responding
3. **Verify the comments match the code** - Line numbers may shift between commits

### Getting All Review Threads

**CRITICAL:** Always use `last: 50` (not `first: 30`) to get ALL threads. Using `first` may miss newer comments.

```bash
# Get ALL review thread IDs (use 'last' not 'first')
gh api graphql -f query='
query {
  repository(owner: "OWNER", name: "REPO") {
    pullRequest(number: PR_NUMBER) {
      reviewThreads(last: 50) {
        totalCount
        nodes {
          id
          path
          line
          comments(first: 1) { nodes { body } }
        }
      }
    }
  }
}'
```

**Verify thread count:** Check that `totalCount` matches the number of nodes returned. If `first: 30` was used and there are 35 threads, 5 would be missed!

### Responding to Each Thread

**Always reply to each thread individually**, not in a single summary comment.

**Why this matters:**
- Each comment needs its own reply for the reviewer to mark as "resolved"
- A single summary comment cannot be marked as resolved per-thread
- Thread-specific replies keep the review organized and actionable

### Response Types

Use the appropriate prefix based on the disposition:

| Prefix | Meaning | When to Use |
|--------|---------|-------------|
| ✅ Resolvido | Code fixed/removed | Changed code to address the comment |
| ✅ Verificado | Code is correct as-is | Confirmed the code behavior is intentional |
| 📋 | Acknowledged, deferred | Good suggestion, will address in future PR |
| ❌ | Declined | Suggestion not applicable, with explanation |
| ❓ | Clarification needed | Question about the comment |

### Reply Command

```bash
# Reply to a specific thread
gh api graphql -f query='
mutation {
  addPullRequestReviewThreadReply(input: {
    pullRequestReviewThreadId: "THREAD_ID",
    body: "✅ Resolvido. Explicação da resolução..."
  }) {
    comment { id }
  }
}'
```

### Example Responses

```
✅ Resolvido. O campo `agents_md` é usado em `repl.rs` via `state.agents_md.as_deref()`.

✅ Resolvido. Este método foi removido no commit bf99ecc - era código morto (YAGNI).

✅ Verificado. `#[allow(clippy::too_many_arguments)]` é necessário - a função tem 8 parâmetros e o limite do Clippy é 7.

📋 Boa sugestão! Mover para `prompts.rs` é uma melhoria de organização. Pode ser feito em uma refatoração subsequente.

❌ Não aplicável. Este padrão `const { Cell::new(false) }` é válido desde Rust 1.79+ - é a forma recomendada para thread_local.

❓ Você pode elaborar? Não entendi bem o que está sugerindo aqui.
```

### After Responding to All Threads

1. **Verify count:** Ensure all `totalCount` threads have been replied to
2. **Check for missed files:** Reviewers may comment on files you didn't check
3. **Commit changes if needed:** If code was modified, commit and push
4. **Inform the user:** Let them know all threads have been responded to

### Common Review Comment Categories

| Category | Response Type | Example |
|----------|---------------|---------|
| `#[allow(dead_code)]` removal | ✅ Resolvido | "Code removed/found usage" |
| Code simplification | ✅ Resolvido or 📋 | "Refactored" or "Deferred to future PR" |
| Architecture improvement | 📋 | "Good suggestion, will address separately" |
| Bug fix | ✅ Resolvido | "Fixed in commit abc1234" |
| Question/clarification | ❓ or ✅ Verificado | "Answer is..." or "Verified behavior is correct" |

## Never Leave Things for Later

**CRITICAL RULE:** If you cannot complete something now, you MUST document it somewhere:

1. **Todo list** - Use the todowrite tool for immediate tasks
2. **Roadmap** - Update `IMPLEMENTATION.md` for larger features
3. **Code comments** - If leaving TODO/FIXME, add issue reference or context
4. **Changelog** - Note incomplete work in version notes
5. **GitHub Issue** - Create/update issue on the Project board

**Why this matters:** The user cannot read your mind. If you "leave something for later" without documenting it, it will be forgotten and may cause confusion, bugs, or security issues.

**Examples of what to document:**

```rust
// ❌ BAD - Will be forgotten
fn parse_config() {
    // TODO: implement per-tool parsing later
}

// ✅ GOOD - Tracked and explained
fn parse_config() {
    // FIXME: Per-tool TOML parsing not implemented yet.
    // See IMPLEMENTATION.md Phase 1.2 for details.
    // Currently uses hardcoded defaults regardless of TOML content.
}
```

**When you realize something is incomplete:**
1. Stop and assess: Is this critical for current task?
2. If yes → Implement it now
3. If no → Document immediately in todo list AND roadmap
4. Never proceed silently with incomplete work

**In conversation context:**
If you tell the user "I'll do X later", you have failed. Either do it now, or explicitly ask if it should be deferred and then document it in a visible place.
