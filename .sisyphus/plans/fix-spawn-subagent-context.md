# Fix: spawn_subagent Ollama Context Not Available

## TL;DR

> **Quick Summary**: Fix the bug where `spawn_subagent` fails with "Ollama client not available in tool context" by switching from `with_context()` to `with_full_context()` in both chat and query modes, and adding `with_tool_context()` for sessions without database.
>
> **Deliverables**:
> - Fix chat mode tool context (`src/chat/core.rs`)
> - Fix query mode tool context (`src/query/executor.rs` + `mod.rs`)
> - Add `with_tool_context()` to `src/tools/context.rs`
> - Update manual test script (`~/MANUAL-TEST-P4.md`)
>
> **Estimated Effort**: Quick
> **Parallel Execution**: YES - 2 waves
> **Critical Path**: Task 1 → Task 2+3 (parallel) → Task 4

---

## Context

### Original Request
User tested `/ocr` in chat → LLM called `spawn_subagent` → got error: "Error: Ollama client not available in tool context."

### Root Cause
`spawn_subagent` uses `get_ollama()` and `get_settings()` which read from `tokio::task_local!` variables (`TOOL_OLLAMA` and `TOOL_SETTINGS`). These are never set because:

1. **Chat mode** (`src/chat/core.rs:494-503`): Uses `with_context()` which only scopes `REMEMBER_DB` + `REMEMBER_EMBEDDING`. Does NOT scope `TOOL_OLLAMA` or `TOOL_SETTINGS`.
2. **Query mode** (`src/query/executor.rs:53`): Same — uses `with_context()` only.
3. **No-DB path** (`core.rs:501-503` and `executor.rs:39-41`): Skips context entirely — `coordinator.chat()` is called bare with no task-locals at all.

The `with_full_context()` function exists in `src/tools/context.rs:87-110` but is marked `#[allow(dead_code)]` because it was never integrated.

### Interview Summary
**Key Discussions**:
- User confirmed subagents should NOT receive conversation history (they are one-shot, already correct)
- User wants agent principal to stay on-hold while subagent works (already the case via `.await`)
- Chat commands (`/ocr`, `/vision`, `/translate`, `/summarize`) work fine — they get ollama/settings directly from `ReplState`

### Metis Review
**Identified Gaps** (addressed):
- **No-DB path**: Anonymous sessions also need `TOOL_OLLAMA`/`TOOL_SETTINGS` — requires a `with_tool_context()` that only scopes ollama+settings (Option B from Metis)
- **Query parameter threading**: `execute_query_with_retry` lacks `ollama` and `settings` params — they must be added
- **`Arc<Settings>` vs `&Settings` mismatch**: Callers have `&Settings`, but `with_full_context` expects `Arc<Settings>` — clone+Arc at call site

---

## Work Objectives

### Core Objective
Make `spawn_subagent` work by ensuring `TOOL_OLLAMA` and `TOOL_SETTINGS` are always set in the task-local context when tools execute, regardless of whether DB/embedding are available.

### Concrete Deliverables
- `spawn_subagent` succeeds from chat mode
- `spawn_subagent` succeeds from query mode
- `spawn_subagent` succeeds from anonymous/no-DB sessions
- Manual test script updated with tilde expansion, session context, and feature flag corrections

### Definition of Done
- [x] `cargo test --all-features` passes
- [x] `cargo clippy --all-features -- -D warnings` passes
- [x] Chat mode: LLM can call `spawn_subagent` successfully
- [x] Query mode: LLM can call `spawn_subagent` successfully
- [x] Anonymous session: LLM can call `spawn_subagent` successfully

### Must Have
- `TOOL_OLLAMA` and `TOOL_SETTINGS` scoped in ALL execution paths (chat, query, no-DB)
- `with_tool_context()` for no-DB paths (only scopes TOOL_OLLAMA + TOOL_SETTINGS)
- `with_full_context()` used when DB is available (all 4 task-locals)
- `#[allow(dead_code)]` removed from `with_full_context()`

### Must NOT Have (Guardrails)
- Do NOT modify CLI subcommand code (G2)
- Do NOT give database access to subagents (G3)
- Do NOT add `spawn_subagent` to subagent tool whitelists (G5)
- Do NOT change `SubagentRunner` structure or behavior
- Do NOT modify `compact_conversation` in core.rs (it doesn't use tools)
- Do NOT create a new public function in executor.rs when extending existing ones suffices
- Do NOT refactor `QueryContext` to store `Arc<Settings>` (too invasive for a bug fix)

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** - ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: Tests-after (add regression test for context availability)
- **Framework**: cargo test

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Backend**: Use Bash (cargo test, cargo clippy) — Build, lint, test
- **Integration**: Use Bash (cargo run with specific args) — Verify spawn_subagent works

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately - context module):
└── Task 1: Add with_tool_context() + clean up dead_code [quick]

Wave 2 (After Wave 1 - fix both modes in parallel):
├── Task 2: Fix chat mode — switch to with_full_context/with_tool_context [unspecified-low]
├── Task 3: Fix query mode — thread params + switch context [unspecified-low]

Wave 3 (After Wave 2 - test + doc updates):
├── Task 4: Add unit test for context availability [quick]
└── Task 5: Update manual test script [quick]

Wave FINAL (After ALL tasks — verify):
├── F1: Plan compliance audit (oracle)
├── F2: Code quality review (unspecified-low)
├── F3: Real manual QA (unspecified-low)
└── F4: Scope fidelity check (deep)
```

### Dependency Matrix

| Task | Depends On | Blocks | Wave |
|------|-----------|--------|------|
| 1 | — | 2, 3, 4 | 1 |
| 2 | 1 | F1-F4 | 2 |
| 3 | 1 | F1-F4 | 2 |
| 4 | 1 | F1-F4 | 3 |
| 5 | 2, 3 | F1-F4 | 3 |

### Agent Dispatch Summary

- **Wave 1**: 1 task - T1 → `quick`
- **Wave 2**: 2 tasks - T2, T3 → `unspecified-low`
- **Wave 3**: 2 tasks - T4, T5 → `quick`
- **FINAL**: 4 tasks - F1 → `oracle`, F2 → `unspecified-low`, F3 → `unspecified-low`, F4 → `deep`

---

## TODOs

- [x] 1. Add `with_tool_context()` and remove `#[allow(dead_code)]` from `with_full_context()`

  **What to do**:
  - In `src/tools/context.rs`, add a new `with_tool_context()` function that only scopes `TOOL_OLLAMA` + `TOOL_SETTINGS` (for no-DB paths like anonymous sessions)
  - Refactor `with_full_context()` to call `with_tool_context()` internally (nest the scopes)
  - Remove `#[allow(dead_code)]` from `with_full_context()` (line 86)
  - Keep `#[allow(clippy::redundant_async_block)]` (it's a style suppression, not dead code)

  **Must NOT do**:
  - Do NOT change the signature of `with_full_context()` — it already takes all 4 params
  - Do NOT remove `with_context()` — it's still valid for tools that only need DB+embedding
  - Do NOT modify `get_db()`, `get_embedding()`, `get_ollama()`, or `get_settings()`

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (foundation for tasks 2-4)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 2, 3, 4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/tools/context.rs:69-80` — `with_context()` pattern to follow for `with_tool_context()`
  - `src/tools/context.rs:87-110` — `with_full_context()` to refactor (remove dead_code, make it use with_tool_context internally)

  **API/Type References**:
  - `src/tools/context.rs:18-27` — Task-local declarations (`TOOL_OLLAMA`, `TOOL_SETTINGS`)

  **Acceptance Criteria**:

  - [x] `with_tool_context(ollama, settings, f)` function exists in `src/tools/context.rs`
  - [x] `with_tool_context()` scopes `TOOL_OLLAMA` and `TOOL_SETTINGS`
  - [x] `with_full_context()` internally calls `with_tool_context()` (or nests its scopes)
  - [x] `#[allow(dead_code)]` removed from `with_full_context()`
  - [x] `cargo check --all-features` passes
  - [x] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: with_tool_context provides Ollama and Settings to tools
    Tool: Bash (cargo test)
    Preconditions: Changes to context.rs compiled
    Steps:
      1. cargo test --lib context -- --nocapture
      2. Verify no compilation errors
    Expected Result: All existing tests pass, no new warnings
    Evidence: .sisyphus/evidence/task-1-context-compile.txt

  Scenario: Dead code annotation removed
    Tool: Bash (grep)
    Preconditions: Edit complete
    Steps:
      1. grep -n "dead_code" src/tools/context.rs
    Expected Result: No "dead_code" on with_full_context line
    Evidence: .sisyphus/evidence/task-1-no-dead-code.txt
  ```

  **Commit**: YES
  - Message: `fix(context): add with_tool_context() and remove dead_code from with_full_context()`
  - Files: `src/tools/context.rs`
  - Pre-commit: `cargo check --all-features`

---

- [x] 2. Fix chat mode — use `with_full_context()` / `with_tool_context()`

  **What to do**:
  - In `src/chat/core.rs`, modify the execution block at lines 494-503:
    - **When DB+embedding are available**: Use `with_full_context(db, embedding, ollama.clone(), Arc::new(settings.clone()), coordinator.chat(messages.clone()))`
    - **When DB or embedding is missing**: Use `with_tool_context(ollama.clone(), Arc::new(settings.clone()), coordinator.chat(messages.clone()))`
  - `ollama` is already available as `&Ollama` (parameter `ollama: &ollama_rs::Ollama` at line 331) — call `.clone()` to get owned `Ollama`
  - `settings` is already available as `&Settings` (parameter at line 338) — create `Arc::new(settings.clone())`
  - Import `with_full_context` and `with_tool_context` from `crate::tools::context`

  **Must NOT do**:
  - Do NOT modify `compact_conversation` (line ~686) — it doesn't use tools
  - Do NOT change `send_message` signature
  - Do NOT add `Arc<Settings>` as a new parameter — clone internally from `&Settings`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 3)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 5
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/chat/core.rs:494-503` — Current `with_context()` call to replace
  - `src/chat/core.rs:330-344` — `send_message` signature showing `ollama: &Ollama` and `settings: &Settings` params

  **API/Type References**:
  - `src/tools/context.rs:87-110` — `with_full_context()` signature: `(db: Arc<Database>, embedding: Arc<EmbeddingClient>, ollama: Ollama, settings: Arc<Settings>, f: F) -> T`
  - `src/tools/context.rs` (new) — `with_tool_context()` signature: `(ollama: Ollama, settings: Arc<Settings>, f: F) -> T`

  **WHY Each Reference Matters**:
  - `core.rs:494-503`: This is the exact code block to modify — the entire `if let` / `else` for context wrapping
  - `core.rs:330-344`: Confirms `ollama` and `settings` are already in scope at the call site
  - `context.rs:87-110`: Shows the exact parameter types needed — `Ollama` (owned) and `Arc<Settings>`

  **Acceptance Criteria**:

  - [x] Chat mode with DB: `with_full_context()` called with all 4 context parameters
  - [x] Chat mode without DB: `with_tool_context()` called with ollama + settings
  - [x] `spawn_subagent` no longer returns "Ollama client not available" from chat
  - [x] `cargo clippy --all-features -- -D warnings` passes
  - [x] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: spawn_subagent works in chat mode with DB
    Tool: Bash (cargo build + cargo test)
    Preconditions: Chat mode fix applied, Ollama running, models available
    Steps:
      1. cargo build --features all-tools
      2. cargo test --all-features
    Expected Result: Build succeeds, tests pass
    Evidence: .sisyphus/evidence/task-2-chat-build.txt

  Scenario: No-DB session still allows spawn_subagent
    Tool: Bash (cargo clippy)
    Preconditions: Fix applied
    Steps:
      1. cargo clippy --all-features -- -D warnings
    Expected Result: Zero warnings
    Evidence: .sisyphus/evidence/task-2-clippy.txt
  ```

  **Commit**: YES
  - Message: `fix(chat): use with_full_context/with_tool_context for tool execution`
  - Files: `src/chat/core.rs`
  - Pre-commit: `cargo check --all-features`

---

- [x] 3. Fix query mode — thread `ollama` and `settings` + switch context

  **What to do**:
  - In `src/query/executor.rs`:
    - Add `ollama: Ollama` and `settings: Arc<Settings>` parameters to `execute_query_with_retry()`
    - Add same parameters to `execute_with_context()` and `execute_without_context()`
    - Replace `with_context()` call in `execute_with_context()` (line 53) with `with_full_context()`
    - Replace bare `coordinator.chat()` in `execute_without_context()` (line 66) with `with_tool_context()`
  - In `src/query/mod.rs` (caller at line 289-297):
    - Pass `ctx.ollama.clone()` as the `ollama` parameter
    - Pass `Arc::new(settings.clone())` as the `settings` parameter (settings is `&Settings` in `run_query` scope at line 234)
  - Import `with_full_context` and `with_tool_context` from `crate::tools::context`

  **Must NOT do**:
  - Do NOT refactor `QueryContext` to store `Arc<Settings>` — too invasive for a bug fix
  - Do NOT create a new public function — extend `execute_query_with_retry`
  - Do NOT change `execute_retry_loop()` — it doesn't need context, it's called inside the scoped closure

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 2)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 5
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/query/executor.rs:21-42` — `execute_query_with_retry()` current signature and dispatch
  - `src/query/executor.rs:44-57` — `execute_with_context()` to modify
  - `src/query/executor.rs:59-67` — `execute_without_context()` to modify
  - `src/query/mod.rs:289-297` — Call site to add parameters

  **API/Type References**:
  - `src/query/context.rs:20-35` — `QueryContext` struct has `ollama: Ollama` at line 34
  - `src/query/mod.rs:224-235` — `run_query()` has `settings: &Settings` parameter at line 234

  **WHY Each Reference Matters**:
  - `executor.rs:21-42`: This is the function whose signature changes — all 3 inner functions need new params
  - `mod.rs:289-297`: This is the ONLY caller — must pass `ctx.ollama.clone()` and `Arc::new(settings.clone())`
  - `context.rs:34`: Confirms `QueryContext` already owns an `Ollama` instance
  - `mod.rs:234`: Confirms `settings` is available as `&Settings` in the caller scope

  **Acceptance Criteria**:

  - [x] `execute_query_with_retry()` accepts `ollama: Ollama` and `settings: Arc<Settings>` parameters
  - [x] `execute_with_context()` calls `with_full_context()` instead of `with_context()`
  - [x] `execute_without_context()` calls `with_tool_context()` instead of bare `coordinator.chat()`
  - [x] `spawn_subagent` no longer returns "Ollama client not available" from query mode
  - [x] `cargo clippy --all-features -- -D warnings` passes
  - [x] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: spawn_subagent works in query mode
    Tool: Bash (cargo build + cargo test)
    Preconditions: Query mode fix applied
    Steps:
      1. cargo build --features all-tools
      2. cargo test --all-features
    Expected Result: Build succeeds, all tests pass
    Evidence: .sisyphus/evidence/task-3-query-build.txt

  Scenario: Code query mode (no DB) allows spawn_subagent
    Tool: Bash (cargo clippy)
    Preconditions: Fix applied
    Steps:
      1. cargo clippy --all-features -- -D warnings
    Expected Result: Zero warnings, no unused parameter warnings
    Evidence: .sisyphus/evidence/task-3-clippy.txt
  ```

  **Commit**: YES
  - Message: `fix(query): thread ollama+settings to executor and use with_full_context/with_tool_context`
  - Files: `src/query/executor.rs`, `src/query/mod.rs`
  - Pre-commit: `cargo check --all-features`

---

- [x] 4. Add unit test for context availability

  **What to do**:
  - Add a test in `src/tools/context.rs` (or a new test file) that verifies:
    - `with_full_context()` scopes all 4 task-locals correctly
    - `with_tool_context()` scopes `TOOL_OLLAMA` and `TOOL_SETTINGS` correctly
    - `get_ollama()` and `get_settings()` return `Some` inside `with_tool_context()`
    - `get_ollama()` returns `None` outside any context scope
  - Test should be an async test using `#[tokio::test]`

  **Must NOT do**:
  - Do NOT test with real Ollama server — use mock/dummy values
  - Do NOT test tool behavior — only test context scoping

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 5)
  - **Parallel Group**: Wave 3
  - **Blocks**: F1-F4
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/tools/context.rs` — Module where the test should live

  **Acceptance Criteria**:

  - [x] Test `with_tool_context_scopes_ollama_and_settings` passes
  - [x] Test `with_full_context_scopes_all_four` passes
  - [x] Test `get_ollama_returns_none_outside_scope` passes
  - [x] `cargo test --lib context` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Context tests pass
    Tool: Bash (cargo test)
    Preconditions: Tests written
    Steps:
      1. cargo test --lib context -- --nocapture
    Expected Result: 3 tests pass, 0 failures
    Evidence: .sisyphus/evidence/task-4-context-tests.txt
  ```

  **Commit**: YES
  - Message: `test(context): add unit tests for task-local context scoping`
  - Files: `src/tools/context.rs`
  - Pre-commit: `cargo test --lib context`

---

- [x] 5. Update manual test script

  **What to do**:
  - Update `~/MANUAL-TEST-P4.md` with the following additions:
    1. **Section 1 (/ocr)**: Add tests for tilde expansion (`~/path`) and session context (LLM references OCR result)
    2. **Section 2 (/vision)**: Add tests for tilde expansion and session context
    3. **Section 3 (/translate)**: Add test for session context
    4. **Section 4 (/summarize)**: Add test for session context
    5. **Section 5 (spawn_subagent)**: Add tests for query mode (not just chat), anonymous session, and tilde expansion in file_path
    6. **Section 9 (Feature Flag)**: Fix incorrect statement — chat commands are always available (NOT feature-gated). Only `spawn_subagent` LLM tool is feature-gated.
    7. **New Section 13**: Test that `spawn_subagent` fails gracefully when built without `subagent-tools` feature

  **Specific corrections for Section 9**:
  - CURRENT (WRONG): "Without feature: `/ocr`, `/vision`, `/translate`, `/summarize` commands are NOT available in chat"
  - CORRECT: "Chat commands `/ocr`, `/vision`, `/translate`, `/summarize` are ALWAYS available (not feature-gated). Only the `spawn_subagent` LLM tool is gated by `subagent-tools`."

  **Must NOT do**:
  - Do NOT rewrite existing test sections — only ADD new items
  - Do NOT commit this file to git (it's outside the repo)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 4)
  - **Parallel Group**: Wave 3
  - **Blocks**: F1-F4
  - **Blocked By**: Tasks 2, 3

  **References**:

  **Pattern References**:
  - `~/MANUAL-TEST-P4.md` — Existing manual test script to update

  **Acceptance Criteria**:

  - [x] Tilde expansion test items added to sections 1 and 2
  - [x] Session context test items added to sections 1-4
  - [x] Section 5 has query mode, anonymous session, and tilde expansion tests
  - [x] Section 9 corrected: chat commands always available, only spawn_subagent is feature-gated
  - [x] New section 13 added for no-feature behavior test

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Manual test script updated
    Tool: Bash (grep)
    Preconditions: File updated
    Steps:
      1. grep -c "expand_tilde_path\|tilde" ~/MANUAL-TEST-P4.md  (expect >= 4)
      2. grep -c "session context\|add_user_message\|add_assistant_message" ~/MANUAL-TEST-P4.md  (expect >= 4)
      3. grep "always available" ~/MANUAL-TEST-P4.md  (expect in Section 9)
    Expected Result: All grep counts match expectations
    Evidence: .sisyphus/evidence/task-5-test-script-updates.txt
  ```

  **Commit**: NO (file is outside repo at ~/MANUAL-TEST-P4.md)

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, grep for pattern). For each "Must NOT Have": search codebase for forbidden patterns. Check evidence files exist in .sisyphus/evidence/. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-low`
  Run `cargo clippy --all-features -- -D warnings` + `cargo test --all-features`. Review all changed files for: `as any`/`@ts-ignore`, empty catches, console.log in prod, commented-out code, unused imports.
  Output: `Build [PASS/FAIL] | Lint [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [x] F3. **Real Manual QA** — `unspecified-low`
  Start from clean state. Execute EVERY QA scenario from EVERY task — follow exact steps, capture evidence. Test cross-task integration. Test edge cases. Save to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff (git log/diff). Verify 1:1 — everything in spec was built, nothing beyond spec was built. Check "Must NOT do" compliance. Detect cross-task contamination.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

- **Task 1**: `fix(context): add with_tool_context() and remove dead_code from with_full_context()` — src/tools/context.rs
- **Task 2**: `fix(chat): use with_full_context/with_tool_context for tool execution` — src/chat/core.rs
- **Task 3**: `fix(query): thread ollama+settings to executor and use with_full_context/with_tool_context` — src/query/executor.rs, src/query/mod.rs
- **Task 4**: `test(context): add unit tests for task-local context scoping` — src/tools/context.rs
- **Task 5**: No commit (file outside repo)

---

## Success Criteria

### Verification Commands
```bash
cargo build --features all-tools         # Expected: success
cargo clippy --all-features -- -D warnings  # Expected: 0 warnings
cargo test --all-features                # Expected: all pass (623+ tests)
cargo run --features all-tools -- chat    # Then: LLM calls spawn_subagent → success
```

### Final Checklist
- [x] `TOOL_OLLAMA` scoped in chat mode (with DB and without DB)
- [x] `TOOL_SETTINGS` scoped in chat mode (with DB and without DB)
- [x] `TOOL_OLLAMA` scoped in query mode (with DB and without DB)
- [x] `TOOL_SETTINGS` scoped in query mode (with DB and without DB)
- [x] `spawn_subagent` works from chat mode
- [x] `spawn_subagent` works from query mode
- [x] `spawn_subagent` works from anonymous session
- [x] No regressions in existing tests
- [x] Manual test script updated