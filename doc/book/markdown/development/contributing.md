# Contributing to Ask-AI

Thank you for your interest in contributing to Ask-AI! This guide will help you get started.

## Getting Started

### Prerequisites

- Rust toolchain (latest stable)
- Git
- Ollama (for testing)
- mdbook and mdbook-mermaid (for documentation)

### Setup

```bash
# Clone the repository
git clone <your-repo-url>
cd ask-ollama-rs

# Install dependencies
cargo build

# Run tests
cargo test

# Install documentation tools
cargo install mdbook
cargo install mdbook-mermaid
```

## Development Workflow

### 1. Create a Branch

```bash
git checkout -b feature/my-feature
```

### 2. Make Changes

- Follow the code style guidelines (see AGENTS.md)
- Write tests for new features
- Update documentation

### 3. Test

```bash
# Run all tests
cargo test

# Run clippy
cargo clippy -- -D warnings

# Format code
cargo fmt

# Build
cargo build --release
```

### 4. Update Documentation

When adding features, update:

1. **User documentation** in `doc/src/`
   - Relevant command documentation
   - Examples and use cases
   - Troubleshooting if applicable

2. **Man page** in `man/ask-ai.1`
   - New flags/options
   - New commands
   - Examples

3. **Development docs** in `doc/src/development/`
   - Architecture changes
   - Roadmap updates

### 5. Commit

```bash
git add .
git commit -m "feat: add new feature"
```

Follow conventional commits:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `refactor:` Code refactoring
- `test:` Tests
- `chore:` Maintenance

### 6. Push and Create PR

```bash
git push origin feature/my-feature
```

Then create a Pull Request on GitHub.

## Code Style

See [AGENTS.md](../../AGENTS.md) for detailed guidelines.

Quick reference:

```rust
// Naming
const MAX_SIZE: usize = 100;
fn process_data(input: String) -> AppResult<String> {
    let result = transform(input)?;
    Ok(result)
}

// Error handling
use_tool().await?;

// Async
async fn fetch_data() -> AppResult<Data> {
    // async code
}
```

## Documentation Guidelines

### User Documentation

- Keep it functional, not technical
- Include examples
- Use clear, concise language
- Add diagrams where helpful (Mermaid)

### Development Documentation

- Explain architecture decisions
- Document APIs
- Add troubleshooting for developers

### When to Update

Update documentation when:

- Adding new commands
- Adding new flags/options
- Changing behavior
- Adding new models
- Adding new tools

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        assert_eq!(2 + 2, 4);
    }
}
```

### Integration Tests

```bash
# Test specific command
cargo test query
cargo test translate

# Test all
cargo test
```

### Manual Testing

```bash
# Build and run
cargo build --release
./target/release/ask-ollama "Test query"

# Test subcommands
./target/release/ask-ollama translate en:pt "Hello"
./target/release/ask-ollama ocr image.png
./target/release/ask-ollama summarize "Text"
```

## Adding a New Command

1. Create module in `src/`
2. Add CLI struct with `clap`
3. Implement handler
4. Add to main router
5. Write tests
6. Update documentation
7. Update man page

Example structure:

```rust
// src/mycommand/cli.rs
use clap::Args;

#[derive(Args, Debug)]
pub struct MyCommandArgs {
    #[arg(short, long)]
    pub option: String,
}

// src/mycommand/mod.rs
pub mod cli;
pub mod processor;

// src/mycommand/processor.rs
use crate::AppResult;

pub async fn process(args: cli::MyCommandArgs) -> AppResult<()> {
    // Implementation
    Ok(())
}
```

## Adding a New Tool

1. Define function in `src/tools/`
2. Use `#[ollama_rs::function]` macro
3. Add to coordinator in `main.rs`
4. Update tools documentation
5. Test with capable model

Example:

```rust
// src/tools/my_tool.rs
use ollama_rs::function;

#[function]
pub async fn my_tool(param: String) -> Result<String, Error> {
    // Implementation
    Ok(result)
}
```

## Adding a New Model

1. Add to `src/config.rs`
2. Update `ask-ai --list` output
3. Document in `doc/src/models.md`
4. Update man page

## Documentation Checklist

Before submitting PR:

- [ ] Updated relevant doc pages in `doc/src/`
- [ ] Updated man page if CLI changed
- [ ] Updated README.md if needed
- [ ] Added examples to documentation
- [ ] Tested with `mdbook serve`
- [ ] All links work

## Pull Request Checklist

- [ ] Code builds without warnings
- [ ] Tests pass
- [ ] Clippy clean
- [ ] Code formatted
- [ ] Documentation updated
- [ ] Man page updated (if needed)
- [ ] Commit messages follow conventions

## Getting Help

- GitHub Issues: Bug reports, feature requests
- Discussions: General questions
- Documentation: Check `doc/src/development/`

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Acknowledgments

Thank you to all contributors who make Ask-AI better!

## See Also

- [Architecture](./architecture.md) - Technical details
- [Roadmap](./roadmap.md) - Future plans
- AGENTS.md - Development guidelines
