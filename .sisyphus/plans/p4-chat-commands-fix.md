# P4 Fix: Wire Up Chat Commands /ocr /vision /translate /summarize

## TL;DR

> **Quick Summary**: Add the 4 missing chat commands /ocr, /vision, /translate, /summarize to the REPL, and fix 3 pre-existing clippy warnings.
> 
> **Deliverables**:
> - 4 ChatCommand variants + parse logic + handlers + help text
> - 3 clippy fixes (manual_checked_ops, useless_conversion, dead_code)
> 
> **Estimated Effort**: Quick
> **Parallel Execution**: YES - 2 waves
> **Critical Path**: Task 1 → Task 2

---

## Context

### Original Request
User tried `/ocr` in chat and it didn't work. The 4 subagent chat commands were never wired into the ChatCommand enum, parse_command(), command_handlers.rs, or print_help(). Task 9 in P4 only added documentation to `doc/src/commands/chat.md`.

### Interview Summary
**Key Discussions**:
- Syntax: Match existing CLI syntax (simplified for chat context)
- Feature gate: Always available (NOT feature-gated under subagent-tools)
- `/ocr <path>` — single file, default mode=text
- `/vision <path> [prompt]` — single file + optional prompt
- `/translate <lang_pair> <text>` — e.g., `/translate en:pt Hello world`
- `/summarize <text>` — default format/style

**Research Findings**:
- `run_ocr()` takes `settings: &Settings` as separate param (unlike other run_* methods)
- `Ollama` and `Settings` both implement `Clone`
- `handle_command()` is already `async`, so awaiting subagent calls is fine
- `ReplState` has `ollama: Ollama` and `settings: Settings` — all needed data available

### Metis Review
**Identified Gaps** (addressed):
- `run_ocr` takes `settings: &Settings` as a separate parameter — handler must pass `&state.settings`
- `SubagentConfig` system prompt is ignored by OCR/Vision (they delegate to processors) — config can use placeholder prompt

---

## Work Objectives

### Core Objective
Wire the 4 subagent chat commands into the REPL and fix 3 clippy warnings.

### Concrete Deliverables
- `src/chat/commands.rs`: 4 new ChatCommand variants + parse logic + help text
- `src/chat/command_handlers.rs`: 4 handler implementations using SubagentRunner
- `src/chat/core.rs`: Fix `manual_checked_ops`
- `src/retrieval/context_builder.rs`: Fix `useless_conversion`
- `src/tools/context.rs`: Fix `dead_code` warning

### Definition of Done
- [ ] `cargo clippy --all-features -- -D warnings` passes cleanly
- [ ] `cargo test --lib` passes
- [ ] `/ocr`, `/vision`, `/translate`, `/summarize` parse correctly in chat
- [ ] `/help` shows the new commands
- [ ] Each command dispatches to SubagentRunner and prints result

### Must Have
- All 4 commands functional in chat REPL
- Matching CLI syntax (simplified for chat)
- Clippy clean with `-D warnings`
- No feature gating (always available)

### Must NOT Have (Guardrails)
- G1: Do NOT change SubagentRunner or its run_* methods
- G2: Do NOT change CLI subcommand handlers (main.rs handle_*)
- G3: Do NOT feature-gate these commands under subagent-tools
- G4: Do NOT add database access to the handlers (subagents are stateless)
- G5: Do NOT add new dependencies
- G6: Do NOT change the struct/behavior of existing commands
- G7: Do NOT modify OcrProcessor or VisionProcessor

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: Tests-after
- **Framework**: cargo test

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **CLI/TUI**: Use interactive_bash (tmux) — Run command, validate output
- **Build**: Use Bash — cargo clippy, cargo test

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — clippy fixes):
├── Task 1: Fix 3 clippy warnings [quick]
└── (sequential prerequisite for Task 2 — ensures clean build)

Wave 2 (After Wave 1 — command wiring):
├── Task 2: Add ChatCommand variants + parse logic + help text [quick]
├── Task 3: Add command handlers using SubagentRunner [unspecified-high]
└── (Task 2 and 3 are in same wave because Task 3 references variants from Task 2;
     they CAN be parallel if the agent writes both files, but let's keep sequential
     to avoid compile errors from missing variants)

Wave FINAL (After ALL tasks):
├── Task F1: Build + clippy verification [quick]
├── Task F2: Test suite verification [quick]
├── Task F3: Manual QA — command parsing [unspecified-high]
└── Task F4: Scope fidelity check [deep]
-> Present results -> Get explicit user okay

Critical Path: Task 1 → Task 2 → Task 3 → F1-F4 → user okay
Parallel Speedup: Limited (sequential due to compile dependencies)
Max Concurrent: 2 (Wave FINAL)
```

### Dependency Matrix

- **1**: - → 2
- **2**: 1 → 3
- **3**: 2 → F1-F4

### Agent Dispatch Summary

- **1**: 1 - T1 → `quick`
- **2**: 2 - T2 → `quick`, T3 → `unspecified-high`
- **FINAL**: 4 - F1 → `quick`, F2 → `quick`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [x] 1. Fix 3 Pre-Existing Clippy Warnings

  **What to do**:
  - Fix `src/chat/core.rs:171` — `manual_checked_ops`:
    Replace the 5-line if-else block (lines 171-175):
    ```rust
    // BEFORE:
    let percent = if context_window > 0 {
        (tokens_used * 100) / context_window
    } else {
        0
    };
    // AFTER:
    let percent = (tokens_used * 100).checked_div(context_window).unwrap_or(0);
    ```
  - Fix `src/retrieval/context_builder.rs:372` — `useless_conversion`:
    Remove `.into_iter()` from the call:
    ```rust
    // BEFORE:
    push_messages_as_chat_messages(&mut messages, recent_messages.into_iter());
    // AFTER:
    push_messages_as_chat_messages(&mut messages, recent_messages);
    ```
  - Fix `src/tools/context.rs:86` — `dead_code` warning for `with_full_context`:
    Change the attribute on line 86:
    ```rust
    // BEFORE:
    #[allow(clippy::redundant_async_block)]
    // AFTER:
    #[allow(dead_code, clippy::redundant_async_block)]
    ```

  **Must NOT do**:
  - Do NOT change any logic — these are syntax-only fixes
  - Do NOT fix other clippy warnings

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Trivial 1-line mechanical fixes, no design decisions
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (solo)
  - **Blocks**: Task 2
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References**:
  - `src/chat/core.rs:165-179` — Context compaction handler, the if-else block to replace
  - `src/retrieval/context_builder.rs:366-373` — The push_messages_as_chat_messages call with redundant .into_iter()
  - `src/tools/context.rs:82-99` — The with_full_context function needing dead_code allow

  **External References**:
  - https://rust-lang.github.io/rust-clippy/master/index.html#manual_checked_ops — clippy lint docs
  - https://rust-lang.github.io/rust-clippy/master/index.html#useless_conversion — clippy lint docs

  **WHY Each Reference Matters**:
  - `core.rs:165-179`: Provides exact context for the replacement — verify tokens_used and context_window are usize
  - `context_builder.rs:366-373`: Shows the function signature accepts IntoIterator so .into_iter() is redundant
  - `context.rs:82-99`: The function is intentionally public infrastructure, just needs dead_code allow

  **Acceptance Criteria**:
  - [ ] `cargo clippy --all-features -- -D warnings 2>&1 | grep error` returns empty
  - [ ] `cargo clippy --all-features -- -D warnings 2>&1 | grep warning` returns empty

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Clippy clean after fixes
    Tool: Bash
    Preconditions: All 3 files edited
    Steps:
      1. cargo clippy --all-features -- -D warnings 2>&1
      2. Check exit code is 0
    Expected Result: Zero errors, zero warnings, exit code 0
    Failure Indicators: Any "error:" or "warning:" lines in output
    Evidence: .sisyphus/evidence/task-1-clippy-clean.txt
  ```

  **Commit**: YES
  - Message: `fix: resolve 3 pre-existing clippy warnings`
  - Files: `src/chat/core.rs`, `src/retrieval/context_builder.rs`, `src/tools/context.rs`
  - Pre-commit: `cargo clippy --all-features -- -D warnings`

- [x] 2. Add ChatCommand Variants + Parse Logic + Help Text

  **What to do**:
  - Add 4 new variants to the `ChatCommand` enum in `src/chat/commands.rs`:
    ```rust
    /// Run OCR on an image file
    Ocr { path: String },
    /// Analyze image(s) with vision model
    Vision { paths: Vec<String>, prompt: Option<String> },
    /// Translate text between languages
    Translate { lang_pair: String, text: String },
    /// Summarize text
    Summarize { text: String },
    ```
  - Add parse entries in `parse_command()` (inside the `match *cmd` block, BEFORE the `_` catch-all):
    ```rust
    "ocr" => {
        if args.is_empty() {
            return Some(Err("Usage: /ocr <file>".to_string()));
        }
        ChatCommand::Ocr { path: args.trim().to_string() }
    }
    "vision" => {
        if args.is_empty() {
            return Some(Err("Usage: /vision <path> [prompt]".to_string()));
        }
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let path = parts.first().unwrap_or(&"").to_string();
        let prompt = parts.get(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        ChatCommand::Vision { paths: vec![path], prompt }
    }
    "translate" | "tr" => {
        if args.is_empty() {
            return Some(Err("Usage: /translate <source:target> <text>".to_string()));
        }
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let lang_pair = parts.first().unwrap_or(&"").to_string();
        let text = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
        if text.is_empty() {
            return Some(Err("Usage: /translate <source:target> <text>".to_string()));
        }
        ChatCommand::Translate { lang_pair, text }
    }
    "summarize" | "sum" => {
        if args.is_empty() {
            return Some(Err("Usage: /summarize <text>".to_string()));
        }
        ChatCommand::Summarize { text: args.trim().to_string() }
    }
    ```
  - Add help text to `print_help()`. Add a new `Subagents:` section before the `Shortcuts:` section:
    ```
    Subagents:
      /ocr <file>                 Extract text from an image using OCR
      /vision <path> [prompt]     Analyze image with vision model
      /translate <src:dst> <text>  Translate text between languages
      /summarize <text>           Summarize text

      Shortcuts: /tr = /translate, /sum = /summarize
    ```

  **Must NOT do**:
  - Do NOT add handlers in this task (that's Task 3)
  - Do NOT feature-gate any commands
  - Do NOT change existing command variants

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Straightforward enum + parse additions following existing patterns
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (Task 3 depends on new variants)
  - **Parallel Group**: Wave 2 (sequential with Task 3)
  - **Blocks**: Task 3
  - **Blocked By**: Task 1 (clippy must pass first)

  **References**:

  **Pattern References**:
  - `src/chat/commands.rs:30-160` — Existing ChatCommand enum variants — follow same doc-comment + variant pattern
  - `src/chat/commands.rs:784-975` — parse_command() match block — add new arms before the _ catch-all at line 970
  - `src/chat/commands.rs:982-1070` — print_help() function — add new section before Shortcuts: line (line 1060)
  - `src/chat/commands.rs:867-875` — /search command parsing — reference for multi-token argument handling

  **API/Type References**:
  - `src/chat/subagent.rs:38` — SubagentType enum
  - `src/chat/subagent.rs:278-282` — run_translate() takes lang_pair: &str, text: &str
  - `src/chat/subagent.rs:307-310` — run_summarize() takes text: &str
  - `src/chat/subagent.rs:352-356` — run_vision() takes paths: &[PathBuf], prompt: &str
  - `src/chat/subagent.rs:403-407` — run_ocr() takes path: &Path, mode: OcrMode, settings: &Settings

  **WHY Each Reference Matters**:
  - The enum pattern ensures consistency with existing variants
  - The parse_command position matters — must be BEFORE the _ catch-all to avoid Unknown command errors
  - The run_* signatures dictate what data the ChatCommand variants must carry

  **Acceptance Criteria**:
  - [ ] `cargo check` passes (no compile errors from new variants)

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Help text verification
    Tool: Bash (grep)
    Preconditions: Code compiled
    Steps:
      1. grep -n "Subagents" src/chat/commands.rs
      2. grep -n "/ocr" src/chat/commands.rs
    Expected Result: Subagents section found, /ocr documented in help
    Failure Indicators: No Subagents heading or no /ocr in help output
    Evidence: .sisyphus/evidence/task-2-help-text.txt
  ```

  **Commit**: NO (groups with Task 3)

- [x] 3. Add Command Handlers Using SubagentRunner

  **What to do**:
  - Add 4 handler functions in `src/chat/command_handlers.rs` (after existing handlers, around line 730+):
    ```rust
    async fn handle_subagent_ocr(state: &mut ReplState, path: String) {
        use crate::chat::subagent::{SubagentConfig, SubagentRunner};
        use crate::ocr::mode::OcrMode;
        use std::path::Path;

        let file_path = Path::new(&path);
        if !file_path.exists() {
            eprintln!("\x1B[31mError: File not found: {}\x1B[0m", path);
            return;
        }

        let (model, _, _) = state.settings.get_subcommand_config("ocr");
        let config = SubagentConfig::new(model, "OCR extraction");
        let runner = SubagentRunner::new(state.ollama.clone(), config, state.settings.clone());

        match runner.run_ocr(file_path, OcrMode::Text, &state.settings).await {
            Ok(result) => println!("{}", result),
            Err(e) => eprintln!("\x1B[31mError: {}\x1B[0m", e),
        }
    }
    ```
  - Similarly for handle_subagent_vision, handle_subagent_translate, handle_subagent_summarize
  - Add match arms in `handle_command()` (before the closing `}` of the match at line 409):
    ```rust
    ChatCommand::Ocr { path } => {
        handle_subagent_ocr(state, path).await;
        HandleResult::Continue
    }
    ChatCommand::Vision { paths, prompt } => {
        handle_subagent_vision(state, paths, prompt).await;
        HandleResult::Continue
    }
    ChatCommand::Translate { lang_pair, text } => {
        handle_subagent_translate(state, lang_pair, text).await;
        HandleResult::Continue
    }
    ChatCommand::Summarize { text } => {
        handle_subagent_summarize(state, text).await;
        HandleResult::Continue
    }
    ```

  **Key Implementation Details**:
  - `run_ocr()` takes `settings: &Settings` as separate param — pass `&state.settings`
  - `run_vision()` takes `paths: &[PathBuf]` — convert `Vec<String>` to `Vec<PathBuf>`
  - `run_translate()` takes `lang_pair: &str, text: &str` — pass directly
  - `run_summarize()` takes `text: &str` — pass directly
  - Use `state.ollama.clone()` (Ollama implements Clone)
  - Use `state.settings.clone()` (Settings implements Clone)
  - Resolve model from `state.settings.get_subcommand_config("ocr")` etc.
  - All handlers are async — handle_command() is already async

  **Must NOT do**:
  - Do NOT modify SubagentRunner or its run_* methods
  - Do NOT add database access to handlers
  - Do NOT change OcrProcessor or VisionProcessor

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Multiple handler implementations with understanding of SubagentRunner API and ReplState
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Task 2's new variants)
  - **Parallel Group**: Wave 2 (sequential after Task 2)
  - **Blocks**: F1-F4
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `src/chat/command_handlers.rs:83-409` — handle_command() match dispatch — add new arms before closing brace
  - `src/chat/command_handlers.rs:354-376` — DocumentImport and DocumentDelete handlers — similar async pattern

  **API/Type References**:
  - `src/chat/subagent.rs:122-147` — SubagentConfig::new() and default_model_options()
  - `src/chat/subagent.rs:154-168` — SubagentRunner::new(ollama, config, settings)
  - `src/chat/subagent.rs:278-293` — run_translate(&self, lang_pair, text) -> Result<String>
  - `src/chat/subagent.rs:307-338` — run_summarize(&self, text) -> Result<String>
  - `src/chat/subagent.rs:352-356` — run_vision(&self, paths: &[PathBuf], prompt: &str) -> Result<String>
  - `src/chat/subagent.rs:403-418` — run_ocr(&self, path: &Path, mode: OcrMode, settings: &Settings) -> Result<String>
  - `src/chat/repl_state.rs:42-68` — ReplState struct: ollama: Ollama, settings: Settings
  - `src/settings.rs` — get_subcommand_config(name) returns (model_name, thinking, tools)
  - `src/ocr/mode.rs:OcrMode` — OCR mode enum (Text, Table, Figure, Formula) — use OcrMode::Text for default

  **WHY Each Reference Matters**:
  - SubagentConfig::new() and SubagentRunner::new() are exact constructors needed
  - run_ocr has DIFFERENT signature from other methods (extra settings param) — must handle this
  - ReplState field names used directly: state.ollama, state.settings
  - get_subcommand_config resolves model name from config.toml or falls back to defaults

  **Acceptance Criteria**:
  - [ ] `cargo check` passes
  - [ ] `cargo test --lib` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Build passes after adding handlers
    Tool: Bash
    Preconditions: Task 2 and 3 code in place
    Steps:
      1. cargo check 2>&1
    Expected Result: Finished dev [unoptimized + debuginfo] — no errors
    Failure Indicators: Any error[E lines
    Evidence: .sisyphus/evidence/task-3-build.txt

  Scenario: Test suite passes
    Tool: Bash
    Preconditions: Code compiles
    Steps:
      1. cargo test --lib 2>&1 | tail -5
    Expected Result: "test result: ok. N passed; 0 failed"
    Failure Indicators: Any FAILED or 0 passed
    Evidence: .sisyphus/evidence/task-3-tests.txt
  ```

  **Commit**: YES (groups with Task 2)
  - Message: `feat(chat): add /ocr /vision /translate /summarize commands`
  - Files: `src/chat/commands.rs`, `src/chat/command_handlers.rs`
  - Pre-commit: `cargo test --lib`

## Final Verification Wave

- [x] F1. **Build + Clippy Verification** — `quick`
  Run `cargo clippy --all-features -- -D warnings 2>&1` — must produce zero warnings and zero errors.
  Run `cargo check --all-features 2>&1` — must succeed.
  Output: `Clippy [PASS/FAIL] | Check [PASS/FAIL] | VERDICT`

- [x] F2. **Test Suite Verification** — `quick`
  Run `cargo test --lib 2>&1` — must produce 0 failures.
  Compare test count with baseline (623 before changes).
  Output: `Tests [N pass / 0 fail] | Baseline [623] | VERDICT`

- [x] F3. **Manual QA — Command Parsing** — `unspecified-high`
  Build the binary with `cargo build`. Start the app in tmux with `cargo run -- chat`.
  Type each command and verify it's recognized (doesn't show "Unknown command"):
  1. `/help` → verify "Subagents:" section appears with /ocr /vision /translate /summarize
  2. `/ocr` → verify shows "Usage: /ocr <file>"
  3. `/vision` → verify shows "Usage: /vision <path> [prompt]"
  4. `/translate` → verify shows "Usage: /translate <source:target> <text>"
  5. `/summarize` → verify shows "Usage: /summarize <text>"
  6. `/tr en:pt Hello` → verify it dispatches (doesn't error with "Unknown command")
  7. `/sum This is text` → verify it dispatches
  Exit with `/quit`.
  Save tmux output as evidence.
  Evidence: `.sisyphus/evidence/task-F3-command-parsing.log`

- [x] F4. **Scope Fidelity Check** — `deep`
  Read git diff for this branch. Verify:
  1. Only the 4 command files (commands.rs, command_handlers.rs) + 3 clippy files (core.rs, context_builder.rs, context.rs) changed
  2. No new files created (all edits to existing files)
  3. No SubagentRunner changes
  4. No feature flag changes
  5. No CLI handler changes
  Output: `Files [N changed / 7 expected] | Scope [CLEAN/ISSUE] | VERDICT`

---

## Commit Strategy

- **1**: `fix: resolve 3 pre-existing clippy warnings` — `src/chat/core.rs`, `src/retrieval/context_builder.rs`, `src/tools/context.rs`
- **2+3**: `feat(chat): add /ocr /vision /translate /summarize commands` — `src/chat/commands.rs`, `src/chat/command_handlers.rs`

---

## Success Criteria

### Verification Commands
```bash
cargo clippy --all-features -- -D warnings  # Expected: 0 warnings, 0 errors
cargo test --lib                             # Expected: 623+ passed, 0 failed
cargo run -- chat  # Then type /help, /ocr, /vision, /translate, /summarize
```

### Final Checklist
- [ ] All 4 chat commands parse correctly
- [ ] Each command dispatches to SubagentRunner
- [ ] /help shows Subagents section
- [ ] Clippy clean with -D warnings
- [ ] All tests pass
- [ ] No SubagentRunner changes
- [ ] No feature gate changes