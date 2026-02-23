# Agent Guidelines for ask-ai

This is a Rust project that uses the ollama-rs library to interact with Ollama LLM models.

**IMPORTANT: See `IMPLEMENTATION.md` for the detailed feature roadmap and implementation phases.**

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

Before adding `#[allow(dead_code)]`, verify the code is truly unused:
```bash
cargo clippy 2>&1 | grep "never used\|never constructed"
```

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
