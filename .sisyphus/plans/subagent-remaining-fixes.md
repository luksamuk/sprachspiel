# Fix: Subagent Remaining Bugs — Spinner, Model Re-read, Dead Param

## TL;DR

> **Quick Summary**: Fix 4 remaining bugs in the subagent system: (1) OCR spinner overlap in chat mode, (2) run_vision() re-reads settings instead of using self.config.model, (3) run_document() re-reads settings instead of using self.config.model, (4) run_ocr() has unused `_settings` parameter.
>
> **Deliverables**:
> - OcrProcessor::process_file() uses conditional spinner (context param) — shows in CLI, hides in subagent
> - run_vision() uses self.config.model instead of get_subcommand_config()
> - run_document() uses self.config.model instead of get_subcommand_config()
> - run_ocr() signature cleaned — no unused `_settings` param
>
> **Estimated Effort**: Quick
> **Parallel Execution**: NO — single wave, all changes are small and interdependent
> **Critical Path**: Task 1 → Task 2 → Task 3 → Build/Verify

---

## Context

### Original Request
Follow-up from plan `subagent-security-model-fix.md` — implementation was 90% complete but 4 bugs remained unfixed, identified by user and F4 scope fidelity review.

### Interview Summary
**Key Discussions**:
- Bugs 2, 3, 4 are straightforward one-line fixes with exact line numbers known
- Bug 1 (spinner overlap) has a **regression risk**: standalone `ask-ai ocr` CLI depends on the spinner inside `process_file()`. Simply removing it would make CLI OCR run silently.
- User chose **conditional spinner (context param)** approach: add `show_spinner: bool` param to `process_file()` and `process_batch()`. CLI path passes `true`, subagent path passes `false`.
- Metis confirmed: must update doc comments referencing the old `get_subcommand_config` pattern
- Metis discovered Bug 5 (double truncation in `run()`) and Bug 6 (command_handlers double-resolution) — both DEFERRED to keep scope tight

**Research Findings**:
- `process_file()` called from 2 paths: subagent (run_ocr) and CLI (main.rs:486 via process_batch)
- `self.settings` still needed by `run_vision()` for `VisionProcessor::process()` (uses `settings.ollama_client()`)
- `self.config.model` contains resolved model ID (e.g. "glm-ocr:bf16"), not config key — this is correct for passing to processors
- Doc comments on lines 372, 421, 493-494 reference old `get_subcommand_config` pattern and must be updated

### Metis Review
**Identified Gaps** (addressed):
- Spinner regression in standalone OCR CLI — resolved with conditional param per user choice
- Stale doc comments — will update as part of fixes
- `run_translate()` and `run_summarize()` already correct (use `self.config.model`) — no changes needed there
- Double truncation in `run()` — DEFERRED (functionally benign)
- command_handlers.rs double-resolution — DEFERRED (separate cleanup)

---

## Work Objectives

### Core Objective
Fix the 4 remaining bugs so all subagent methods use `self.config` consistently and the OCR spinner doesn't overlap in chat mode.

### Concrete Deliverables
- `src/ocr/processor.rs` — conditional spinner in `process_file()` and `process_batch()`
- `src/chat/subagent.rs` — fix model re-read in run_vision/run_document, remove dead `_settings` param in run_ocr, update doc comments
- `src/chat/command_handlers.rs` — update run_ocr call site (remove `&state.settings`)
- `src/main.rs` — pass `show_spinner: true` in CLI OCR path

### Definition of Done
- [ ] `cargo test --all-features` passes
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] No `get_subcommand_config` calls inside `SubagentRunner::run_vision()`, `run_ocr()`, or `run_document()`
- [ ] No spinner import in `src/ocr/processor.rs` that's unused
- [ ] `run_ocr` signature has no `Settings` type
- [ ] Standalone `ask-ai ocr` shows spinner (conditional param = true)
- [ ] Chat-mode OCR does NOT show overlapping spinner (conditional param = false)

### Must Have
- Conditional spinner in OcrProcessor (show_spinner: bool param)
- run_vision/run_document use self.config.model instead of re-reading settings
- run_ocr signature cleaned (no _settings param)
- Doc comments updated to reflect new patterns
- All callers updated (subagent.rs:216, command_handlers.rs:2872, main.rs:486)

### Must NOT Have (Guardrails)
- G1: Do NOT remove spinner entirely from process_file() — would break standalone OCR CLI UX
- G2: Do NOT change VisionProcessor::process() signature or internals
- G3: Do NOT remove `self.settings` field from `SubagentRunner` (still used by run_vision)
- G4: Do NOT fix double truncation in run() (Bug 5 — defer)
- G5: Do NOT fix command_handlers double-resolution pattern (Bug 6 — defer)
- G6: Do NOT change run_translate() or run_summarize() (already correct)
- G7: Do NOT add new dependencies or features
- G8: Do NOT touch process_batch() structure beyond adding show_spinner forwarding

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed.

### Test Decision
- **Infrastructure exists**: YES (cargo test)
- **Automated tests**: Tests-after (verify existing tests still pass after changes)
- **Framework**: cargo test + cargo clippy

### QA Policy
Every task includes agent-executed QA scenarios. Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Sequential — small interdependent changes):
├── Task 1: Conditional spinner in OcrProcessor [quick]
├── Task 2: Fix model re-read + dead param in SubagentRunner [quick]
└── Task 3: Build, test, clippy verification [quick]

Wave FINAL (After ALL tasks — 4 parallel reviews):
├── F1: Plan compliance audit (oracle)
├── F2: Code quality review (unspecified-high)
├── F3: Real manual QA (unspecified-high)
└── F4: Scope fidelity check (deep)
```

### Dependency Matrix

- **1**: - - 2, 3
- **2**: 1 - 3
- **3**: 1, 2 - F1-F4

### Agent Dispatch Summary

- **Wave 1**: 3 tasks — T1 → `quick`, T2 → `quick`, T3 → `quick`
- **FINAL**: 4 tasks — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [x] 1. Conditional Spinner in OcrProcessor

  **What to do**:
  - Add `show_spinner: bool` parameter to `OcrProcessor::process_file()` (line 30)
  - Add `show_spinner: bool` parameter to `OcrProcessor::process_batch()` (line 84)
  - Guard the spinner creation (lines 57-61) and finish (line 72) with `if show_spinner { ... }`
  - Forward `show_spinner` from `process_batch()` to `process_file()` calls (line 94)
  - Update `main.rs` line 486: pass `true` for `show_spinner` in CLI OCR path
  - Update `src/chat/subagent.rs` line 439 (inside run_ocr): pass `false` for `show_spinner` in subagent path (the caller already shows "Thinking..." spinner)

  **Must NOT do**:
  - Do NOT remove the spinner import `use crate::spinner::{create_spinner, finish_spinner};` — it's still used conditionally
  - Do NOT change process_file() or process_batch() signatures beyond adding show_spinner
  - Do NOT add spinner logic to main.rs — the spinner stays inside OcrProcessor

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Small, well-scoped parameter addition with clear line references
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (sequential, Task 1 first)
  - **Blocks**: Task 2 (subagent.rs callers need updated signature)
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/ocr/processor.rs:30-81` — `process_file()` current signature and spinner usage
  - `src/ocr/processor.rs:84-111` — `process_batch()` current signature and call to `process_file()`

  **API/Type References**:
  - `src/spinner.rs` — `create_spinner()` and `finish_spinner()` signatures (already imported on line 13)

  **Call Sites**:
  - `src/main.rs:486` — CLI OCR path (pass `show_spinner: true`)
  - `src/chat/subagent.rs:439` — Subagent OCR path (pass `show_spinner: false`)

  **WHY Each Reference Matters**:
  - `process_file()` and `process_batch()`: These are the functions being modified — understand current structure before changing
  - `main.rs:486`: This is the CLI caller that needs show_spinner=true — currently passes no such param
  - `subagent.rs:439`: This is the subagent caller that needs show_spinner=false — currently passes no such param

  **Acceptance Criteria**:
  - [ ] `process_file()` signature includes `show_spinner: bool` parameter
  - [ ] `process_batch()` signature includes `show_spinner: bool` parameter
  - [ ] Spinner create/finish calls are guarded with `if show_spinner { ... }`
  - [ ] `process_batch()` forwards `show_spinner` to `process_file()`
  - [ ] main.rs:486 passes `true` for show_spinner
  - [ ] subagent.rs:439 passes `false` for show_spinner
  - [ ] `cargo check` succeeds

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: CLI OCR path shows spinner (parameter validation)
    Tool: Bash (grep)
    Preconditions: Code changes applied
    Steps:
      1. grep -n "show_spinner" src/ocr/processor.rs
      2. Verify "show_spinner: bool" appears in both process_file and process_batch signatures
      3. Verify "if show_spinner" appears around spinner creation
      4. Verify "show_spinner" is forwarded in process_batch → process_file call
    Expected Result: show_spinner param in signatures, conditional spinner, forwarding in batch
    Failure Indicators: Missing param, missing conditional, missing forwarding
    Evidence: .sisyphus/evidence/task-1-conditional-spinner.txt

  Scenario: Callers pass correct show_spinner values
    Tool: Bash (grep)
    Preconditions: Code changes applied
    Steps:
      1. grep -n "show_spinner" src/main.rs
      2. Verify "true" is passed in CLI OCR path
      3. grep -n "show_spinner" src/chat/subagent.rs
      4. Verify "false" is passed in subagent OCR path
    Expected Result: CLI passes true, subagent passes false
    Failure Indicators: Wrong boolean values, missing param in call
    Evidence: .sisyphus/evidence/task-1-caller-values.txt

  Scenario: Build succeeds after spinner changes
    Tool: Bash (cargo)
    Preconditions: All code changes applied
    Steps:
      1. Run `cargo check`
      2. Verify zero errors
    Expected Result: Compiles successfully
    Failure Indicators: Any compilation error
    Evidence: .sisyphus/evidence/task-1-build.txt
  ```

  **Commit**: NO (groups with Task 2 in single commit)

---

- [x] 2. Fix Model Re-read + Dead Param in SubagentRunner

  **What to do**:
  - **Fix run_vision() line 389**: Replace `let (model, _thinking, _tools) = self.settings.get_subcommand_config("vision");` with `let model = self.config.model.clone();`
  - **Fix run_vision() doc comment ~line 372**: Update comment that says "The vision model is resolved from `settings.get_subcommand_config("vision")`" to reflect `self.config.model`
  - **Fix run_document() line 495**: Replace `let (doc_model, _thinking, _tools) = self.settings.get_subcommand_config("document");` with `let doc_model = self.config.model.clone();`
  - **Fix run_document() comment ~lines 493-494**: Update "Get the document model from settings" to reflect `self.config.model`
  - **Fix run_ocr() line 435**: Remove `_settings: &Settings,` parameter from signature
  - **Fix run_ocr() doc comment ~lines 418-421**: Remove references to `settings` parameter in the doc comment (update "# Arguments" section to remove the `settings` line)
  - **Fix run_ocr() caller in subagent.rs line 216**: Change `self.run_ocr(&file_paths[0], OcrMode::Text, &self.settings).await?` to `self.run_ocr(&file_paths[0], OcrMode::Text).await?`
  - **Fix run_ocr() caller in command_handlers.rs line 2872**: Change `runner.run_ocr(&file_path, OcrMode::Text, &state.settings).await` to `runner.run_ocr(&file_path, OcrMode::Text).await`

  **Must NOT do**:
  - Do NOT change line 402 (`processor.process(&args, &model, &self.settings)`) — VisionProcessor still needs settings for ollama_client()
  - Do NOT change VisionProcessor::process() signature
  - Do NOT remove `self.settings` from SubagentRunner struct — still used by run_vision
  - Do NOT change run_translate() or run_summarize() (already correct)
  - Do NOT fix double truncation in run() (Bug 5)
  - Do NOT fix command_handlers double-resolution (Bug 6)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Small, targeted changes to specific lines with exact before/after known
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Task 1 for OcrProcessor signature)
  - **Parallel Group**: Wave 1 (sequential, Task 2 after Task 1)
  - **Blocks**: Task 3 (verification)
  - **Blocked By**: Task 1 (OcrProcessor signature change affects run_ocr compilation)

  **References**:

  **Pattern References**:
  - `src/chat/subagent.rs:380-410` — run_vision() current implementation (bug at line 389)
  - `src/chat/subagent.rs:431-447` — run_ocr() current signature (bug at line 435)
  - `src/chat/subagent.rs:473-559` — run_document() current implementation (bug at line 495)
  - `src/chat/subagent.rs:200-226` — run() dispatch method (caller at line 216)

  **API/Type References**:
  - `src/chat/subagent.rs:93-120` — SubagentConfig struct (has .model and .model_options fields)
  - `src/settings.rs:296-325` — get_subcommand_config() (what we're REMOVING calls to)

  **Call Sites**:
  - `src/chat/subagent.rs:216` — self.run_ocr call inside run() dispatch (remove &self.settings)
  - `src/chat/command_handlers.rs:2872` — runner.run_ocr call in /ocr command handler (remove &state.settings)

  **WHY Each Reference Matters**:
  - Lines 389, 495: These are the exact lines where get_subcommand_config() is being replaced — understand what variables flow downstream
  - Lines 435, 216, 2872: These are the signature change ripple — all callers must be updated
  - Line 402: Understand why settings is STILL needed (for ollama_client) — do NOT remove &self.settings here
  - SubagentConfig struct: Confirms that self.config.model is the resolved model ID (not config key)

  **Acceptance Criteria**:
  - [ ] run_vision() line 389 uses `self.config.model.clone()` (no get_subcommand_config)
  - [ ] run_document() line 495 uses `self.config.model.clone()` (no get_subcommand_config)
  - [ ] run_ocr() signature has no `_settings: &Settings` parameter
  - [ ] subagent.rs:216 call site matches updated signature
  - [ ] command_handlers.rs:2872 call site matches updated signature
  - [ ] Doc comments updated (no stale references to get_subcommand_config or settings param)
  - [ ] `cargo check` succeeds

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: No get_subcommand_config in SubagentRunner methods
    Tool: Bash (grep)
    Preconditions: Code changes applied
    Steps:
      1. grep -n "get_subcommand_config" src/chat/subagent.rs
      2. Verify NO matches within run_vision, run_ocr, or run_document methods
    Expected Result: Zero matches in those three methods
    Failure Indicators: Any get_subcommand_config call still present
    Evidence: .sisyphus/evidence/task-2-no-double-read.txt

  Scenario: run_ocr signature clean
    Tool: Bash (grep)
    Preconditions: Code changes applied
    Steps:
      1. grep -n "pub async fn run_ocr" src/chat/subagent.rs
      2. Verify signature does NOT include "Settings"
      3. grep -n "run_ocr" src/chat/command_handlers.rs
      4. Verify caller does NOT pass &state.settings
    Expected Result: No Settings type in run_ocr signature or callers
    Failure Indicators: Settings still in signature or callers
    Evidence: .sisyphus/evidence/task-2-ocr-signature.txt

  Scenario: Build succeeds after all fixes
    Tool: Bash (cargo)
    Preconditions: All Task 1 and Task 2 changes applied
    Steps:
      1. Run `cargo check`
      2. Verify zero errors
    Expected Result: Compiles successfully
    Failure Indicators: Any compilation error
    Evidence: .sisyphus/evidence/task-2-build.txt
  ```

  **Commit**: YES (groups with Task 1)
  - Message: `fix(subagent): use self.config for model resolution and conditional OCR spinner`
  - Files: `src/ocr/processor.rs`, `src/chat/subagent.rs`, `src/chat/command_handlers.rs`, `src/main.rs`
  - Pre-commit: `cargo test --all-features && cargo clippy --all-features -- -D warnings`

---

- [x] 3. Build, Test, and Clippy Verification

  **What to do**:
  - Run `cargo build --all-features`
  - Run `cargo test --lib --all-features`
  - Run `cargo clippy --all-features -- -D warnings`
  - Grep for any remaining `get_subcommand_config` calls in SubagentRunner's run_vision/run_ocr/run_document
  - Grep for `create_spinner|finish_spinner` usage in ocr/processor.rs (confirm it's conditional)
  - Grep for `Settings` in run_ocr signature (confirm it's gone)

  **Must NOT do**:
  - Do NOT commit separately — this is verification only before the Task 1+2 commit

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Simple verification commands
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (sequential, after Tasks 1+2)
  - **Blocks**: F1-F4
  - **Blocked By**: Tasks 1, 2

  **References**:
  - `src/ocr/processor.rs` — verify conditional spinner
  - `src/chat/subagent.rs` — verify model resolution fixes
  - `src/chat/command_handlers.rs` — verify caller updates

  **Acceptance Criteria**:
  - [ ] `cargo build --all-features` succeeds with 0 errors
  - [ ] `cargo test --lib --all-features` passes with 0 failures
  - [ ] `cargo clippy --all-features -- -D warnings` passes with 0 warnings
  - [ ] No `get_subcommand_config` in run_vision/run_ocr/run_document
  - [ ] Spinner in process_file is conditional (`if show_spinner`)
  - [ ] No `Settings` type in run_ocr signature

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Full build and test suite
    Tool: Bash (cargo)
    Preconditions: All code changes applied
    Steps:
      1. Run `cargo build --all-features 2>&1`
      2. Verify exit code 0
      3. Run `cargo test --lib --all-features 2>&1`
      4. Verify 0 failures
      5. Run `cargo clippy --all-features -- -D warnings 2>&1`
      6. Verify 0 warnings
    Expected Result: All three commands succeed cleanly
    Failure Indicators: Any error, test failure, or clippy warning
    Evidence: .sisyphus/evidence/task-3-build-test-clippy.txt

  Scenario: No stale patterns remain
    Tool: Bash (grep)
    Preconditions: All code changes applied
    Steps:
      1. grep -c "get_subcommand_config" src/chat/subagent.rs
      2. Verify count is 0 for run_vision/run_ocr/run_document scope
      3. grep "if show_spinner" src/ocr/processor.rs
      4. Verify conditional spinner exists
    Expected Result: No stale patterns, conditional spinner in place
    Failure Indicators: Remaining get_subcommand_config in target methods, unconditional spinner
    Evidence: .sisyphus/evidence/task-3-stale-patterns.txt
  ```

  **Commit**: NO (verification only — Task 1+2 commit happens after this verification)

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, grep for pattern). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo clippy --all-features -- -D warnings` + `cargo test --all-features`. Review changed files for: dead code, unused imports, stale comments, `as any` equivalents. Check that no AI slop was introduced.
  Output: `Build [PASS/FAIL] | Clippy [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [x] F3. **Real Manual QA** — `unspecified-high`
  Verify: (1) standalone OCR spinner still shows (read code path), (2) chat-mode OCR has no overlapping spinner, (3) vision uses self.config.model, (4) document uses self.config.model, (5) run_ocr has no settings param. Save evidence to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff. Verify 1:1 — everything in spec was built (no missing), nothing beyond spec was built (no creep). Check "Must NOT do" compliance. Detect cross-task contamination.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

- **Single commit**: `fix(subagent): use self.config for model resolution and conditional OCR spinner`
  - Files: `src/ocr/processor.rs`, `src/chat/subagent.rs`, `src/chat/command_handlers.rs`, `src/main.rs`
  - Pre-commit: `cargo test --all-features && cargo clippy --all-features -- -D warnings`

---

## Success Criteria

### Verification Commands
```bash
cargo build --all-features                              # Expected: Clean build
cargo test --lib --all-features                          # Expected: All tests pass (670+)
cargo clippy --all-features -- -D warnings               # Expected: Zero warnings
grep -c "get_subcommand_config" src/chat/subagent.rs     # Expected: 0 in run_vision/run_ocr/run_document
grep "if show_spinner" src/ocr/processor.rs              # Expected: Conditional spinner guard exists
grep "run_ocr" src/chat/subagent.rs | grep Settings     # Expected: No output (Settings removed)
```

### Final Checklist
- [ ] All "Must Have" present
- [ ] All "Must NOT Have" absent
- [ ] All tests pass
- [ ] Standalone OCR CLI still shows spinner
- [ ] Chat-mode OCR no longer has overlapping spinner
- [ ] All subagent methods use self.config.model consistently