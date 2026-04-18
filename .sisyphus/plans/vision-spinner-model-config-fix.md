# Fix: Vision Spinner + Model Options + get_subcommand_config Missing Arms

## TL;DR

> **Quick Summary**: Fix 3 vision bugs: (1) spinner overlap in chat mode, (2) VisionProcessor hardcodes model_options ignoring models.toml config, (3) get_subcommand_config() missing "vision" and "code" match arms so user config is never consulted.
>
> **Deliverables**:
> - VisionProcessor::process() accepts ollama, model_options, show_spinner params (replaces settings)
> - Conditional spinner in VisionProcessor (shows in CLI, hides in chat)
> - model_options from SubagentConfig used instead of hardcoded temperature(0.1)
> - get_subcommand_config() handles "vision" and "code" subcommands
> - Both callers (main.rs CLI, subagent.rs chat) updated
>
> **Estimated Effort**: Short
> **Parallel Execution**: NO — single wave, all interdependent
> **Critical Path**: Task 1 → Task 2 → Task 3 → Verify

---

## Context

### Original Request
User reported: (1) spinner blinking in vision "all modes", (2) vision tried to use qwen3.5:9b instead of configured kimi-k2.5:cloud.

Root cause analysis found 3 bugs:
1. VisionProcessor::process() has unconditional spinner (overlaps with "Thinking..." in chat)
2. VisionProcessor::process() hardcodes `ModelOptions::default().temperature(0.1).num_predict(...)` — discards SubagentConfig's model_options (num_ctx, temperature from models.toml)
3. `get_subcommand_config("vision")` in settings.rs falls through to `_ => SubcommandModelConfig::default()` because "vision" and "code" are missing from the match — so `[model.vision] model = "kimi-k2.5:cloud"` in config.toml is never consulted

### Interview Summary
**Key Discussions**:
- Follow same OcrProcessor pattern for VisionProcessor (show_spinner + model_options + ollama params)
- num_predict should be layered ON TOP of passed model_options (it's a per-request concern, not model-level)
- Temperature should come from model config via build_model_options() — users can set it in models.toml
- get_subcommand_config fix is purely additive (no behavior change for users without [model.vision] or [model.code] config)
- Remove settings: &Settings from VisionProcessor::process() — replaced by ollama: &Ollama

**Research Findings**:
- VisionProcessor::process() uses settings ONLY for settings.ollama_client() (line 51) — can be replaced by ollama param
- handle_vision() in main.rs:637 resolves model_config but only extracts model_id, discarding build_model_options()
- self.model.vision and self.model.code fields EXIST in ModelSettings (lines 72,74) — they're just not used in the match
- The "code" fallback at settings.rs:315 uses DEFAULT_CODE_MODEL hardcoded, ignoring user's [model.code] config

### Metis Review
**Identified Gaps** (addressed):
- settings: &Settings becomes dead code after adding ollama param → remove it entirely
- handle_vision() CLI also needs model_options fix → same PR
- num_predict merge strategy → layer on top of passed model_options
- "code" match arm also missing → fix both vision and code in same change
- Thinking/tools settings from [model.vision] also ignored → adding match arm fixes this too

---

## Work Objectives

### Core Objective
Make VisionProcessor follow the OcrProcessor pattern: accept explicit params, use model_options from config, conditional spinner. Fix get_subcommand_config() to handle vision and code subcommands.

### Concrete Deliverables
- `src/vision/processor.rs` — new signature with ollama, model_options, show_spinner; conditional spinner; no hardcoded ModelOptions
- `src/settings.rs` — add "vision" and "code" match arms
- `src/main.rs` — handle_vision() passes ollama, model_options, show_spinner=true
- `src/chat/subagent.rs` — run_vision() passes ollama, model_options, show_spinner=false

### Definition of Done
- [ ] `cargo test --all-features` passes
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] No `ModelOptions::default()` or hardcoded `temperature(0.1)` in vision/processor.rs
- [ ] No unconditional `create_spinner` in vision/processor.rs
- [ ] get_subcommand_config("vision") returns user's configured model
- [ ] get_subcommand_config("code") returns user's configured model (or DEFAULT_CODE_MODEL fallback)

### Must Have
- VisionProcessor::process() accepts ollama: &Ollama, model_options: ModelOptions, show_spinner: bool
- Conditional spinner (if show_spinner { Some(...) } else { None })
- model_options from caller used + num_predict layered on top
- settings: &Settings removed from process() signature
- "vision" => &self.model.vision added to get_subcommand_config match
- "code" => &self.model.code added to get_subcommand_config match
- Both callers updated (main.rs, subagent.rs)

### Must NOT Have (Guardrails)
- G1: Do NOT split VisionProcessor into process_file/process_batch — current structure works
- G2: Do NOT add DEFAULT_VISION_MODEL constant — fallback to global default is correct
- G3: Do NOT change VisionArgs struct or max_tokens field
- G4: Do NOT modify other subcommand handlers (translate, document, summarize)
- G5: Do NOT fix command_handlers double-resolution (Bug 6 — still deferred)
- G6: Do NOT add new dependencies or features
- G7: Do NOT remove Settings import from processor.rs until settings param is gone
- G8: Do NOT change run_vision() beyond updating the process() call

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed.

### Test Decision
- **Infrastructure exists**: YES (cargo test)
- **Automated tests**: Tests-after
- **Framework**: cargo test + cargo clippy

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Sequential):
├── Task 1: Fix get_subcommand_config missing arms [quick]
├── Task 2: Refactor VisionProcessor signature + conditional spinner [quick]
└── Task 3: Build, test, clippy verification [quick]

Wave FINAL (4 parallel reviews):
├── F1-F4: Standard verification
```

---

## TODOs

- [x] 1. Fix get_subcommand_config Missing Arms

  **What to do**:
  - In `src/settings.rs:297-304`, add `"vision" => &self.model.vision,` match arm
  - Add `"code" => &self.model.code,` match arm
  - Verify `cargo check` passes

  **Must NOT do**:
  - Do NOT change the fallback logic (lines 309-325)
  - Do NOT add DEFAULT_VISION_MODEL
  - Do NOT modify other match arms

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (sequential, Task 1 first)
  - **Blocks**: Task 2 (subagent needs correct model resolution)
  - **Blocked By**: None

  **References**:
  - `src/settings.rs:297-304` — current match statement
  - `src/settings.rs:72,74` — self.model.code and self.model.vision fields
  - `src/settings.rs:309-325` — fallback logic that must remain unchanged

  **Acceptance Criteria**:
  - [ ] Match statement includes "vision" => &self.model.vision
  - [ ] Match statement includes "code" => &self.model.code
  - [ ] `cargo check` passes

  **QA Scenarios**:
  ```
  Scenario: Missing match arms added
    Tool: Bash (grep)
    Steps:
      1. grep -n '"vision"\|"code"' src/settings.rs
      2. Verify both appear in get_subcommand_config match
    Expected Result: "vision" and "code" in match arms
    Evidence: .sisyphus/evidence/task-1-missing-arms.txt
  ```

  **Commit**: NO (groups with Task 2)

---

- [x] 2. Refactor VisionProcessor Signature + Conditional Spinner

  **What to do**:

  **src/vision/processor.rs** — change `process()` signature:
  ```
  BEFORE:
    pub async fn process(
        &self,
        args: &VisionArgs,
        model: &str,
        settings: &Settings,
    ) -> VisionResult<VisionOutput> {

  AFTER:
    pub async fn process(
        &self,
        args: &VisionArgs,
        model: &str,
        ollama: &Ollama,
        model_options: ModelOptions,
        show_spinner: bool,
    ) -> VisionResult<VisionOutput> {
  ```

  **src/vision/processor.rs** — remove hardcoded ModelOptions:
  ```
  BEFORE (line 51-55):
    let ollama = settings.ollama_client();
    let model_options = ModelOptions::default()
        .temperature(0.1)
        .num_predict(args.max_tokens as i32);

  AFTER:
    // Layer num_predict on top of the passed model_options (per-request concern)
    let model_options = model_options.num_predict(args.max_tokens as i32);
  ```

  **src/vision/processor.rs** — conditional spinner:
  ```
  BEFORE (line 63-69, 78):
    let spinner_msg = if file_count == 1 { ... } else { ... };
    let spinner = create_spinner(&spinner_msg);
    ...
    finish_spinner(spinner);

  AFTER:
    let spinner_msg = if file_count == 1 { ... } else { ... };
    let spinner = if show_spinner {
        Some(create_spinner(&spinner_msg))
    } else {
        None
    };
    ...
    if let Some(sp) = spinner {
        finish_spinner(sp);
    }
  ```

  **src/vision/processor.rs** — update imports:
  ```
  REMOVE: use crate::settings::Settings;
  ADD: use ollama_rs::Ollama;
  (create_spinner/finish_spinner imports stay — still used conditionally)
  ```

  **src/main.rs:655** — update CLI vision caller:
  ```
  BEFORE:
    let processor = VisionProcessor::new();
    match processor.process(&args, &model_id, settings).await {

  AFTER:
    let model_options = model_config.build_model_options().num_predict(args.max_tokens as i32);
    let ollama = settings.ollama_client();
    let processor = VisionProcessor::new();
    match processor.process(&args, &model_id, &ollama, model_options, true).await {
  ```
  Note: model_config is already resolved at line 637 — just need to use build_model_options()

  **src/chat/subagent.rs:400-403** — update subagent vision caller:
  ```
  BEFORE:
    let processor = VisionProcessor::new();
    let output = processor
        .process(&args, &model, &self.settings)
        .await

  AFTER:
    let processor = VisionProcessor::new();
    let output = processor
        .process(&args, &model, &self.ollama, self.config.model_options.clone(), false)
        .await
  ```
  Note: show_spinner=false because chat mode has "Thinking..." spinner. model_options from SubagentConfig (resolved via models.toml).

  **Must NOT do**:
  - Do NOT remove `use crate::spinner::{create_spinner, finish_spinner}` — still used conditionally
  - Do NOT split process() into process_file/process_batch
  - Do NOT change VisionArgs struct
  - Do NOT change run_vision() beyond updating the process() call
  - Do NOT touch command_handlers.rs handle_subagent_vision() — it creates SubagentRunner which calls run_vision() internally

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:
  **Pattern References**:
  - `src/ocr/processor.rs:30-88` — OcrProcessor pattern to follow exactly
  - `src/main.rs:463-486` — handle_ocr() CLI pattern for model_options resolution
  - `src/chat/subagent.rs:430-445` — run_ocr() subagent pattern for passing params

  **API/Type References**:
  - `src/vision/processor.rs:24-91` — current process() implementation
  - `src/main.rs:637-638` — model_config resolution (already exists, just need build_model_options())
  - `src/chat/subagent.rs:380-410` — run_vision() current implementation

  **Acceptance Criteria**:
  - [ ] VisionProcessor::process() has ollama: &Ollama param (no settings: &Settings)
  - [ ] VisionProcessor::process() has model_options: ModelOptions param
  - [ ] VisionProcessor::process() has show_spinner: bool param
  - [ ] No `ModelOptions::default()` or hardcoded `temperature(0.1)` in processor.rs
  - [ ] num_predict layered on top of passed model_options
  - [ ] Conditional spinner with `if show_spinner { Some(...) } else { None }`
  - [ ] main.rs passes show_spinner=true in CLI path
  - [ ] subagent.rs passes show_spinner=false in chat path
  - [ ] `cargo check` passes

  **QA Scenarios**:
  ```
  Scenario: No hardcoded ModelOptions in processor
    Tool: Bash (grep)
    Steps:
      1. grep -n "ModelOptions::default\|temperature(0.1)" src/vision/processor.rs
    Expected Result: Zero matches
    Evidence: .sisyphus/evidence/task-2-no-hardcoded-modelopts.txt

  Scenario: Conditional spinner in processor
    Tool: Bash (grep)
    Steps:
      1. grep -n "show_spinner\|create_spinner" src/vision/processor.rs
      2. Verify "if show_spinner" pattern and no unconditional create_spinner
    Expected Result: Conditional spinner guard exists
    Evidence: .sisyphus/evidence/task-2-conditional-spinner.txt

  Scenario: Build after all changes
    Tool: Bash (cargo)
    Steps:
      1. Run cargo check --all-features
      2. Verify zero errors
    Expected Result: Compiles successfully
    Evidence: .sisyphus/evidence/task-2-build.txt
  ```

  **Commit**: YES (groups with Task 1)
  - Message: `fix(vision): add conditional spinner, use model_options from config, fix get_subcommand_config missing arms`
  - Files: `src/vision/processor.rs`, `src/settings.rs`, `src/main.rs`, `src/chat/subagent.rs`
  - Pre-commit: `cargo test --all-features && cargo clippy --all-features -- -D warnings`

---

- [x] 3. Build, Test, and Clippy Verification

  **What to do**:
  - Run `cargo build --all-features`
  - Run `cargo test --lib --all-features`
  - Run `cargo clippy --all-features -- -D warnings`
  - Verify no stale patterns remain in vision/processor.rs

  **Commit**: NO (verification only)

---

## Final Verification Wave (MANDATORY)

- [ ] F1. **Plan Compliance Audit** — `oracle`
- [ ] F2. **Code Quality Review** — `unspecified-high`
- [ ] F3. **Real Manual QA** — `unspecified-high`
- [ ] F4. **Scope Fidelity Check** — `deep`

---

## Commit Strategy

- **Single commit**: `fix(vision): add conditional spinner, use model_options from config, fix get_subcommand_config missing arms`
  - Files: `src/vision/processor.rs`, `src/settings.rs`, `src/main.rs`, `src/chat/subagent.rs`
  - Pre-commit: `cargo test --all-features && cargo clippy --all-features -- -D warnings`

---

## Success Criteria

### Verification Commands
```bash
cargo build --all-features                              # Expected: Clean build
cargo test --lib --all-features                          # Expected: All tests pass
cargo clippy --all-features -- -D warnings               # Expected: Zero warnings
grep -c "ModelOptions::default\|temperature(0.1)" src/vision/processor.rs  # Expected: 0
grep "if show_spinner" src/vision/processor.rs           # Expected: Conditional guard exists
grep -n '"vision"\|"code"' src/settings.rs               # Expected: Both in match
```

### Final Checklist
- [ ] Vision spinner conditional (no blinking in chat mode)
- [ ] Vision uses model_options from config.toml
- [ ] get_subcommand_config handles "vision" and "code"
- [ ] All tests pass