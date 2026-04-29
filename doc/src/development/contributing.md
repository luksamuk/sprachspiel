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
cd ask-ai

# Install dependencies
cargo build

# Run tests
cargo test

# Install documentation tools
cargo install mdbook
cargo install mdbook-mermaid
```

## Development Workflow

Ask-AI uses a structured PR workflow. **See `doc/src/development/PR-PROCESS.md` for the complete process.**

Quick summary:
1. Create a branch with conventional prefix (`feat/`, `fix/`, `refactor/`, `docs/`, `test/`)
2. Update documentation BEFORE writing code (CHANGELOG, IMPLEMENTATION.md)
3. Create a DRAFT PR, then implement
4. Run quality gates before each commit and PR (load `quality-gates` skill for details)
5. Mark PR ready for review after passing all checks
6. Respond to review comments individually
7. Manual testing via Hermes Agent before merge

For the full step-by-step workflow, load the `pr-workflow` skill.

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

## Adding a New Tool

**For the complete guide, load the `project-conventions` skill.**

Quick summary:
1. Add feature flag to `Cargo.toml`
2. Create tool module in `src/tools/my_tool.rs`
3. Register in `src/tools/registry.rs`
4. Add tool prompts in `src/prompts/tools.rs`
5. Test with `cargo test --features my-tools`

**For detailed tool development guidelines (error handling, parameter types, logging, etc.), load the `tool-guidelines` skill.**

## Adding a New Command

1. Create module in `src/`
2. Add CLI struct with `clap`
3. Implement handler
4. Add to main router
5. Write tests
6. Update documentation
7. Update man page

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

# Test with all features
cargo test --features all-tools
```

### Manual Testing

Before merging, PRs must pass manual testing via the Hermes Agent. See `doc/src/development/MANUAL-TEST-TEMPLATE.md` for the template.

**Load the `pr-testing` skill for the complete manual testing workflow.**

## Documentation

### When to Update

- **New commands** → Add page in `doc/src/commands/`
- **New flags/options** → Update relevant command documentation and man page
- **New models** → Update `doc/src/models.md`
- **New tools** → Update `doc/src/tools.md`
- **Behavior changes** → Update relevant documentation with migration notes

**For the complete documentation checklist and update process, load the `project-conventions` skill.**

### Building Documentation

```bash
cd doc
mdbook serve     # Serve locally for testing
mdbook build      # Build static site → doc/book/
```

## Pull Request Checklist

- [ ] Code builds without warnings (`cargo check --all-features`)
- [ ] Tests pass (`cargo test --all-features`)
- [ ] Clippy clean (`cargo clippy --all-features -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] No bare `#[allow(dead_code)]` without justification
- [ ] Documentation updated (CHANGELOG, relevant doc pages)
- [ ] Man page updated (if CLI changed)
- [ ] Commit messages follow conventional commits (`feat:`, `fix:`, `docs:`, etc.)
- [ ] PR created as DRAFT first
- [ ] Quality gates passed (load `quality-gates` skill for details)

## Getting Help

- GitHub Issues: Bug reports, feature requests
- Discussions: General questions
- Documentation: Check `doc/src/development/`
- AGENTS.md: Development guidelines and project conventions

## See Also

- [PR Process](./PR-PROCESS.md) — Mandatory workflow for all PRs
- [Architecture](./architecture.md) — Technical details
- [Roadmap](./roadmap.md) — Future plans
- [AGENTS.md](../../AGENTS.md) — Development guidelines and coding rules