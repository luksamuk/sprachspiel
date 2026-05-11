# Agent Guidelines for sprachspiel

This is a Rust project that uses the ollama-rs library to interact with LLM models, with Ollama as the default backend (multi-backend support planned).

**IMPORTANT: See `IMPLEMENTATION.md` for the detailed feature roadmap and implementation phases.**

## ⚠️ BEFORE IMPLEMENTING ANYTHING

**READ `doc/src/development/PR-PROCESS.md` COMPLETELY.** This is MANDATORY.

The PR process document and related skills describe the exact workflow for branches, documentation, PRs, reviews, and merges. **DO NOT skip this step. DO NOT assume you know the workflow.**

For the full step-by-step workflow, load the `pr-workflow` skill.

## Next Demand Workflow

When the user asks "What's the next demand?" (or "Qual a próxima demanda?"), load the `next-demand` skill for the complete workflow.

## Build Commands

```bash
cargo build                              # Build the project
cargo build --release                    # Build release
cargo build --release --features all-tools  # Build with all tools
cargo run                                # Run the application
cargo test                               # Run all tests
cargo test --features all-tools          # Run tests with all features
cargo check                              # Check code without building
cargo fmt                                # Format code
cargo clippy -- -D warnings -A clippy::allow_attributes -A clippy::too_many_lines -A clippy::cognitive_complexity
```

## Compilation Features

- **Default:** weather-tools, file-tools, calc-tools, serper-tools, system-tools, skills-tools
- **Optional:** pokemon-tools, search-tools, finance-tools, led-tools
- **All:** `--features all-tools`

For details on adding new tools with feature flags, load the `project-conventions` skill.

## Code Style Guidelines

- Use `cargo fmt` before committing, `cargo clippy` to catch mistakes, max 100 chars line length
- Group imports: std → external crates → local modules
- Prefer `type AppResult<T> = Result<T, Box<dyn std::error::Error + Sync + Send>>;`
- Use `async/await` for async operations, owned types (String) for struct fields
- **Naming:** snake_case (functions/variables), PascalCase (types/structs/enums), SCREAMING_SNAKE_CASE (constants)
- Boolean fields: use clear predicates (e.g., `tools`, `vision`, `ocr`)
- Derive `Debug`, `Clone` as needed, public fields for data structures
- Use `Coordinator` pattern for chat sessions, add tools conditionally based on model capabilities

## Code Deduplication

### Shared Utilities (`src/utils.rs`)

Use these instead of creating duplicates: `parse_bool()`, `parse_u32()`, `parse_bounded_number()`, `format_size()`, `read_stdin()`, `capitalize()`

### Tool Registration (`src/tools/registry.rs`)

All tool registration is centralized. When adding a new tool: add function in module → register in `register_tools()` → add name to `get_available_tool_names()`

### Common Patterns — Use Centralized Implementations

These patterns have centralized implementations that MUST NOT be duplicated:

1. **Model switching** — Use `model_switch::switch_model()` (load `model-switching` skill for details)
2. **Model configuration** — Use `ModelConfig::build_model_options()`
3. **Capability detection** — Use `ModelCapabilities::detect_or_default()`
4. **Thinking display** — Use `display_thinking()`
5. **Model resolution** — Use `resolve_model_config()`
6. **Think mode validation** — Use `resolve_think_mode()`

### Model Switching — CRITICAL SAFETY RULE

**All model switching MUST go through `switch_model()` in `src/chat/model_switch.rs`.** Never duplicate validation, config resolution, capability detection, or state adjustment logic. Load the `model-switching` skill for details.

### `#[allow(dead_code)]` Policy

Only use `#[allow(dead_code)]` with justification:

**Acceptable:** JSON deserialization fields, error enum variants, public API methods (rare — prefer `#[cfg(test)]`)

**Not acceptable:** "Might be useful later", dead code that should be removed, "Preparation for future features"

**Prefer `#[cfg(test)]` for test-only code.** If a function is only called from tests, gate it with `#[cfg(test)]` instead of `#[allow(dead_code)]`. This makes the scope explicit and prevents accidental reliance in production code.

**Every `#[allow(dead_code)]` MUST have a justification comment on the same line:**

```rust
// GOOD: Public API contract, cannot gate behind #[cfg(test)]
#[allow(dead_code)] // Error enum variant — used by From implementation
fn from_error() -> Self { ... }

// GOOD: Test-only helper function
#[cfg(test)]
fn test_helper() { ... }

// BAD: Vague future promise
#[allow(dead_code)] // Might need this later
fn upcoming_feature() {}
```

Before adding `#[allow(dead_code)]`, verify: `cargo clippy 2>&1 | grep "never used\|never constructed"`

If clippy reports the item as unused and it's only called from tests, convert to `#[cfg(test)]`. If it's truly unused, remove it (YAGNI).

Load the `quality-gates` skill for the complete enforcement script.

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

All string constants are centralized in `src/consts/`:
- `src/consts/roles.rs` — Message role constants (`ROLE_USER`, `ROLE_ASSISTANT`, etc.) and `format_role_label()` / `format_role_label_md()`
- `src/consts/api.rs` — API URLs (`OPEN_METEO_BASE`, `SERPER_API_URL`, etc.)

**NEVER hardcode:** `"msg"`, `"doc"`, `"note"`, `"web"` (use `SourceType::prefix()`), `"user"`, `"assistant"`, `"system"`, `"tool"` (use role constants).

Enforcement before PR merge:
```bash
rg '"user"|"assistant"|"system"|"tool"' src/ --type rust   # Duplicated role strings
rg '"msg"|"doc"|"note"' src/ --type rust | grep -v 'const\|prefix()'  # Duplicated source prefixes
```

If duplicates found: refactor before merging.

## Tool Development

**For the complete guide to creating and modifying LLM tools, load the `tool-guidelines` skill.**

Critical rules (expanded in the skill): Tools must NEVER crash (always return `Ok(String)`), use `String`/`Option<String>` for numeric parameters, normalize empty strings, always log with `log_tool_call()`/`log_tool_result()`, API responses use `#[serde(default)]`, file writes must be sandboxed, all output in English, docstrings required.

## Documentation and Release

- **For updating documentation after implementation, load the `project-conventions` skill.**
- **For creating a release, load the `release-process` skill.**
- **For quality gates and sensor hierarchy, load the `quality-gates` skill.**

## Project Management

### GitHub Project Board

- **Starting a task:** Find issue → update status to "In Progress" → assign yourself
- **Completing a task:** Update status to "Done" → close with commit reference → update `IMPLEMENTATION.md`
- **Blocked task:** Update status → add comment → add `status:blocked` label

### Board Columns

| Column | Meaning |
|--------|---------|
| Backlog | Not yet prioritized, needs review |
| Ready | Prioritized, ready to start |
| In Progress | Currently being worked on |
| In Review | PR submitted or awaiting review |
| Done | Completed and verified |

### Updating Roadmap

**CRITICAL:** After completing ANY roadmap item, update `IMPLEMENTATION.md`: `❌ NOT STARTED` → `✅ COMPLETED` or `📋 IN PROGRESS`, with implementation summary.

### Issue Management

- **Creating:** Use `[P#] Feature Name` titles with priority labels
- **Closing:** Reference resolving commit (`Fixes #123`) → update roadmap → ensure docs updated

## Pull Request Review

**For the complete review workflow including review response format, load the `pr-workflow` skill.**

Critical rules for reviews:
- Always use `last: 50` (not `first: 30`) to get ALL review threads
- Respond to EACH thread individually (never a single summary comment)
- **NEVER create a single large comment addressing all review points.** Reply to each review thread inline. If inline replies are not possible, create ONE comment per review point with a blockquote of the original.
- Use response prefixes: ✅ Resolvido, ✅ Verificado, 📋, ❌, ❓

## Never Leave Things for Later

**CRITICAL RULE:** If you cannot complete something now, you MUST document it.

1. **Todo list** — Use the todowrite tool for immediate tasks
2. **Roadmap** — Update `IMPLEMENTATION.md` for larger features
3. **Code comments** — If leaving TODO/FIXME, add issue reference or context
4. **Changelog** — Note incomplete work in version notes
5. **GitHub Issue** — Create/update issue on the Project board

If you tell the user "I'll do X later", you have failed. Either do it now, or explicitly ask if it should be deferred and then document it in a visible place.

## Steering Rule — Bugs and Harness Failure

**Every bug that repeats MUST produce a guide or sensor.** This is the core principle of harness engineering:

- **One-off bugs** are acceptable — they happen, you fix them, move on.
- **Repeated bugs** are a harness failure — if the same type of bug happens twice, the harness (guides + sensors) was insufficient.

When a bug repeats:
1. **Add a computational sensor** that catches it automatically (test, linter rule, script check), OR
2. **Add a feedforward guide** that prevents it (rule in AGENTS.md, clippy configuration, documentation)

This rule applies to the development harness only. The product harness (SOUL.md, skills, facts) has its own feedback loop via the memory system.

**For the complete quality gates and enforcement scripts, load the `quality-gates` skill.**

### External References

- **rust-magic-linter** (vicnaum/rust-magic-linter) — Strict Clippy configs for AI-assisted Rust development
- **rust-skills** (leonardomso/rust-skills) — 179 Rust rules organized in 14 categories with examples
- **Harness Engineering for Coding Agent Users** (Martin Fowler, 2025) — Framework distinguishing feedforward from feedback, computational from inferential