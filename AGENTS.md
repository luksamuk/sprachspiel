# Agent Guidelines for ask-ollama-rs

This is a Rust project that uses the ollama-rs library to interact with Ollama LLM models.

**IMPORTANT: See `IMPLEMENTATION.md` for the detailed feature roadmap and implementation phases.**

## Build Commands

```bash
# Build the project
cargo build

# Build release
cargo build --release

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

## Notes

- The project is a CLI tool for interacting with local Ollama models
- Model capabilities (tools, vision, ocr) are detected at runtime
- Supports custom tools defined via `#[ollama_rs::function]` macro
