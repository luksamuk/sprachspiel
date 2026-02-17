# Agent Guidelines for ask-ollama-rs

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
- `weather-tools` - Weather lookup (enabled by default)
- `web-search-tools` - Web search (enabled by default, currently broken)
- `file-tools` - File operations (enabled by default)

### Optional Features
- `pokemon-tools` - Pokémon data tools (disabled by default to save context)
- `all-tools` - Enable all tool categories

### Why Pokémon Tools Are Optional

Pokémon tools consume significant context window space with 8 specialized tool definitions. They're disabled by default because:
1. They pollute the context window when not needed
2. They increase token usage without benefit for general queries
3. Only users specifically querying Pokémon data need them

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
ask-ollama-rs/
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

## Tool Development Guidelines

When creating or modifying tools, follow these principles:

### Error Handling Philosophy

**Tools must never crash the application.** Instead, return informative error messages as strings that help the LLM understand what went wrong and recover.

```rust
// ❌ BAD - Crashes on error
if !canonical_path.exists() {
    return Err(format!("File not found: {}", path).into());
}

// ✅ GOOD - Returns helpful error message
if !canonical_path.exists() {
    let err_msg = format!(
        "Error: File not found: {}. Please check if the file exists or try a different file name.",
        path
    );
    log_tool_result("read_file", &err_msg);
    return Ok(err_msg);
}
```

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

### Documentation Philosophy

Documentation should:
- Be **functional** (what it does, not how it works internally)
- Be **discoverable** (clear organization)
- Be **complete** (cover all features)
- Be **maintained** (updated with code changes)
- Include **examples** (practical usage)
