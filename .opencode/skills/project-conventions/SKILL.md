---
name: project-conventions
description: Add new tools with feature flags and registry, and update user/development documentation. Covers the full lifecycle from tool code to documentation.
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: conventions
---

## What I do

I guide two related workflows:
1. **Adding new tools** — feature flags, module structure, registry, prompts, Cargo.toml
2. **Updating documentation** — user docs, man page, CHANGELOG, IMPLEMENTATION.md

## When to use me

- Load me when adding a new tool or feature flag
- Load me when updating documentation after implementation
- Load me when asked to update docs, man page, or CHANGELOG

---

# Part 1: Adding New Tools

## Step 1: Add Feature Flag to Cargo.toml

```toml
[features]
default = [..., "my-tools"]
my-tools = ["dep:required-crate"]
all-tools = [..., "my-tools"]
```

- Add to `default` if enabled by default
- Add to `all-tools` always
- Add dependencies if needed

## Step 2: Create Tool Module

Create `src/tools/my_tool.rs`:

```rust
#[cfg(feature = "my-tools")]
use crate::debug_tools::{log_tool_call, log_tool_result};

/// Tool description for the LLM.
///
/// Extended description of what the tool does and when to use it.
///
/// # Arguments
/// * `param` - Description. Default: X. Example: "Y".
///
/// # Returns
/// Description of successful output.
#[cfg(feature = "my-tools")]
#[ollama_rs::function]
pub async fn my_tool(param: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("my_tool", &[("param".to_string(), param.clone())]);

    // IMPORTANT: Use String/Option<String> for numeric params
    // IMPORTANT: Never use ? operator — return Ok(error_msg) instead
    // IMPORTANT: Always log_tool_result before every return

    let result = "...";
    log_tool_result("my_tool", &result);
    Ok(result)
}
```

## Step 3: Register in Module Exports

In `src/tools/mod.rs`:

```rust
#[cfg(feature = "my-tools")]
pub mod my_tool;

#[cfg(feature = "my-tools")]
pub use my_tool::*;
```

## Step 4: Register in Tool Registry

In `src/tools/registry.rs`, add to the appropriate helper functions:

1. **`register_my_tools()`** — Create helper if not exists
2. **`get_my_tools_names()`** — Return tool names
3. **Call from `register_tools()`** with `#[cfg(feature = "my-tools")]`
4. **Call from `get_available_tool_names()`** with `#[cfg(feature = "my-tools")]`

```rust
#[cfg(feature = "my-tools")]
fn register_my_tools(coordinator: &mut Coordinator, blacklist: &[String]) -> usize {
    let mut count = 0;
    if is_tool_allowed("my_tool", blacklist) {
        coordinator = coordinator.add_tool(my_tool);
        count += 1;
    }
    count
}
```

## Step 5: Add Tool Prompts

In `src/prompts/tools.rs`, add the tool description for the system prompt. This tells the LLM when and how to use the tool.

Gate with `#[cfg(feature = "my-tools")]`.

## Step 6: Add Configuration (if needed)

In `src/external/types.rs`, add config struct:

```rust
pub struct MyToolsConfig {
    pub enabled: bool,
}
```

In `src/external/config.rs`, parse from `tools.toml`.

## Step 7: Test

```bash
cargo build --features my-tools
cargo test --features my-tools
cargo clippy --features my-tools -- -D warnings
```

---

# Part 2: Updating Documentation

## User Documentation (`doc/src/`)

### New Commands

Add a new page in `doc/src/commands/`:
- Follow existing command doc structure
- Include synopsis, description, options, and examples
- Add entry to `doc/src/SUMMARY.md`

### New Options/Flags

Update the relevant command documentation:
- Add option to the options table
- Include example usage
- Update the man page if applicable

### New Models

Update `doc/src/models.md`:
- Add to model table
- Include configuration details
- Document best use cases

### New Tools

Update `doc/src/tools.md`:
- Document the tool function and arguments
- Include usage examples
- Note any limitations or known issues

### Behavior Changes

Update relevant documentation:
- Change descriptions if behavior differs
- Update examples to reflect new behavior
- Add migration notes if breaking change

## Man Page (`man/sprach.1`)

Update when CLI interface changes:
- New commands
- New flags/options
- New models
- New examples

## Development Documentation

Update when architecture changes:
- `doc/src/development/architecture.md` — Design decisions
- `doc/src/development/roadmap.md` — Planned features
- `doc/src/CHANGELOG.md` — Version history

## CHANGELOG.md Updates

Add entries under appropriate version section:

```markdown
## [Unreleased]

### Added
- New feature description

### Changed
- Change description

### Fixed
- Bug fix description

### Removed
- Removal description
```

## IMPLEMENTATION.md Updates

After completing ANY roadmap item:

1. Find the relevant section
2. Update status markers:
   - `❌ NOT STARTED` → `✅ COMPLETED` or `📋 IN PROGRESS`
   - `🟡 PLANNED` → `🟢 ACTIVE` (when work begins)
3. Add implementation summary:
   - Key files modified
   - Commits (with short hash)
   - Any deviations from original plan

## Documentation Checklist

Before committing:
- [ ] Updated relevant doc pages in `doc/src/`
- [ ] Updated man page (if CLI changed)
- [ ] Updated CHANGELOG.md
- [ ] Ran `mdbook serve` and verified rendering
- [ ] All internal links work
- [ ] Examples tested and working

## Building Documentation

```bash
cd doc
mdbook serve     # Serve locally for testing
mdbook build      # Build static site → doc/book/
```