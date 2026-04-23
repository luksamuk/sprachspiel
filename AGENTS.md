# Agent Guidelines for ask-ai

This is a Rust project that uses the ollama-rs library to interact with Ollama LLM models.

**IMPORTANT: See `IMPLEMENTATION.md` for the detailed feature roadmap and implementation phases.**

## ⚠️ BEFORE IMPLEMENTING ANYTHING

**READ `doc/src/development/PR-PROCESS.md` COMPLETELY.** This is MANDATORY.

The PR process document and related skills describe the exact workflow for branches, documentation, PRs, reviews, and merges. **DO NOT skip this step. DO NOT assume you know the workflow.**

For the full step-by-step workflow, load the `pr-workflow` skill.

## Next Demand Workflow

When the user asks "What's the next demand?" (or "Qual a próxima demanda?"), load the `next-demand` skill for the complete workflow.

**Summary:** Read `IMPLEMENTATION.md` and `roadmap.md` → identify next priority → re-read PR-PROCESS.md → present plan → wait for approval.

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
- `calc-tools` - Mathematical calculator (1 tool)
- `serper-tools` - Serper API web search (2 tools, requires `SERPER_API_KEY`)
- `system-tools` - System information tools (2 tools)
- `skills-tools` - AI behavior skills (2 tools)

### Optional Features

- `pokemon-tools` - Pokémon data tools (9 tools, disabled by default, opt-in)
- `search-tools` - DuckDuckGo web search (3 tools, disabled by default, may fail due to CAPTCHA)
- `finance-tools` - Stock quotes (disabled by default)
- `led-tools` - NeoPixel LED control (5 tools, disabled by default, requires hardware)
- `all-tools` - Enable all tool categories

### Adding New Tools

When adding new tools, wrap them with feature flags and register them. Load the `project-conventions` skill for the complete guide.

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
```

Prompts dynamically include only available tools (feature flag enabled + not blacklisted at runtime).

### Testing with Features

```bash
cargo test --features all-tools
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

### Types
- Prefer type aliases for commonly used Result types: `type AppResult<T> = Result<T, Box<dyn std::error::Error + Sync + Send>>;`
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

### Struct Definitions
- Derive common traits: `Debug`, `Clone` as needed
- Public fields for data structures

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

Key crates: `ollama-rs`, `tokio`, `reqwest`, `futures`

## Code Deduplication

### Shared Utilities (`src/utils.rs`)

Use these instead of creating duplicates:

```rust
parse_bool(value: Option<&str>, default: bool) -> bool
parse_u32(value: Option<&str>, default: Option<u32>) -> Option<u32>
parse_bounded_number(value: Option<&str>, default: usize, max: Option<usize>) -> usize
format_size(bytes: u64) -> String   // Returns "512 B", "1 KB", "1.5 MB"
read_stdin() -> Result<String, String>
capitalize(s: &str) -> String
```

### Tool Registration (`src/tools/registry.rs`)

All tool registration is centralized. When adding a new tool:
1. Add the tool function in the appropriate module
2. Register it in `register_tools()` in `src/tools/registry.rs`
3. Add the tool name to `get_available_tool_names()` if needed

### Common Patterns to Avoid Duplicating

1. **Model configuration building** — Use `ModelConfig::build_model_options()`
2. **Capability detection** — Use `ModelCapabilities::detect_or_default()`
3. **Thinking display** — Use `display_thinking()`
4. **Model resolution** — Use `resolve_model_config()`
5. **Think mode validation** — Use `resolve_think_mode()`
6. **Model switching** — Use `model_switch::switch_model()` (see below)

### Model Switching — SINGLE POINT OF FAILURE

**CRITICAL:** All model switching MUST go through `src/chat/model_switch.rs`. Never duplicate validation, config resolution, capability detection, or state adjustment logic. Load the `model-switching` skill for details.

### `#[allow(dead_code)]` Policy

Only use `#[allow(dead_code)]` with justification:

**Acceptable:** JSON deserialization fields, error enum variants, public API methods, test-only code with `#[cfg(test)]`

**Not acceptable:** "Might be useful later", dead code that should be removed, "Preparation for future features"

Before adding `#[allow(dead_code)]`, verify: `cargo clippy 2>&1 | grep "never used\|never constructed"`

### TUI Preparation Code Policy

**CRITICAL: Do not declare unused code "for future TUI implementation".**

Current TUI abstractions (documented and ready for TUI migration):
- `src/chat/input/mod.rs` — `InputBackend` trait
- `src/chat/view/mod.rs` — `ChatView` trait
- `src/chat/input/rustyline.rs` — `RustylineInput` (in use)
- `src/chat/view/terminal.rs` — `TerminalView` (in use)

**Do not add more unused code.**

## Constants and String Management

### CRITICAL: No Hardcoded String Duplicates

String literals must NEVER be duplicated. When a string value is used in multiple places, it MUST be defined once and referenced everywhere.

### String Constants Module

All string constants are centralized in `src/consts/`:
- `src/consts/roles.rs` — Message role constants (`ROLE_USER`, `ROLE_ASSISTANT`, etc.)
- `src/consts/api.rs` — API URLs (`OPEN_METEO_BASE`, `SERPER_API_URL`, etc.)

### Categories

1. **Source Type Prefixes** — **NEVER hardcode** `"msg"`, `"doc"`, `"note"`, `"web"`. Use `SourceType::prefix()` from `src/db/operations.rs`.
2. **Message Roles** — **NEVER hardcode** `"user"`, `"assistant"`, `"system"`, `"tool"`. Use constants from `src/consts/roles.rs` or `MessageRole` enum.
3. **Role Display Labels** — **NEVER duplicate**. Use `format_role_label()` and `format_role_label_md()` from `src/consts/roles.rs`.
4. **API URLs** — **NEVER hardcode**. Use constants from `src/consts/api.rs`.

### Where to Add New Constants

- Source type prefixes → `SourceType` enum in `src/db/operations.rs`
- Message roles → `src/consts/roles.rs`
- API URLs → `src/consts/api.rs`
- Other strings → New submodule in `src/consts/` or use existing constants

### Enforcement

Before any PR merge:
```bash
rg '"user"|"assistant"|"system"|"tool"' src/ --type rust   # Duplicated role strings
rg '"msg"|"doc"|"note"' src/ --type rust | grep -v 'const\|prefix()'  # Duplicated source prefixes
rg '#\[allow\(dead_code\)\]' src/consts/  # Dead code on constants
```

If duplicates found: refactor before merging.

## Tool Development

**For the complete guide to creating and modifying LLM tools, load the `tool-guidelines` skill.**

Critical rules (expanded in the skill):

1. **Tools must NEVER crash** — Always return `Ok(String)`, never use `?` or `Err()` in tool code
2. **Use `String`/`Option<String>` for numeric parameters** — LLMs send strings instead of JSON types
3. **Normalize empty strings** — Use `.filter(|s| !s.is_empty())` for truly optional text parameters
4. **Always log** — Use `log_tool_call()` at start, `log_tool_result()` before every return
5. **API responses use `#[serde(default)]`** — Always make struct fields optional
6. **File writes must be sandboxed** — Use `validate_write_path()` and atomic writes
7. **All output in English** — No localized tool output
8. **Docstrings required** — Summary + description + parameters + returns + examples

## Documentation and Release

**For updating documentation after implementation, load the `project-conventions` skill.**

**For creating a release, load the `release-process` skill.**

### Quick Reference

Documentation checklist before committing:
- [ ] Updated relevant doc pages in `doc/src/`
- [ ] Updated man page (if CLI changed)
- [ ] Updated CHANGELOG.md
- [ ] Verified rendering with `mdbook serve`

### Documentation Philosophy

Documentation should be: functional (what it does), discoverable (clear organization), complete (cover all features), maintained (updated with code changes), and include examples.

## Project Management

### GitHub Project Board

- **Starting a task:** Find issue → update status to "In Progress" → assign yourself
- **Completing a task:** Update status to "Done" → close with commit reference → update `IMPLEMENTATION.md`
- **Blocked task:** Update status → add comment → add `status:blocked` label

### Updating Roadmap

**CRITICAL:** After completing ANY roadmap item, update `IMPLEMENTATION.md`:
- `❌ NOT STARTED` → `✅ COMPLETED` or `📋 IN PROGRESS`
- Add implementation summary with key files and commits

### Issue Management

- **Creating:** Use `[P#] Feature Name` titles with priority labels
- **Closing:** Reference resolving commit (`Fixes #123`) → update roadmap → ensure docs updated

### Board Columns

| Column | Meaning |
|--------|---------|
| Backlog | Not yet prioritized, needs review |
| Ready | Prioritized, ready to start |
| In Progress | Currently being worked on |
| In Review | PR submitted or awaiting review |
| Done | Completed and verified |

## Pull Request Review

**For the complete review workflow including review response format, load the `pr-workflow` skill.**

Critical rules for reviews:
- Always use `last: 50` (not `first: 30`) to get ALL review threads
- Respond to EACH thread individually (never a single summary comment)
- Use response prefixes: ✅ Resolvido, ✅ Verificado, 📋, ❌, ❓

## Never Leave Things for Later

**CRITICAL RULE:** If you cannot complete something now, you MUST document it.

1. **Todo list** — Use the todowrite tool for immediate tasks
2. **Roadmap** — Update `IMPLEMENTATION.md` for larger features
3. **Code comments** — If leaving TODO/FIXME, add issue reference or context
4. **Changelog** — Note incomplete work in version notes
5. **GitHub Issue** — Create/update issue on the Project board

If you tell the user "I'll do X later", you have failed. Either do it now, or explicitly ask if it should be deferred and then document it in a visible place.