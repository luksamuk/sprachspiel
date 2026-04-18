# Fix: Subagent Security + Model Resolution + OCR Prompt Restriction

## TL;DR

> **Quick Summary**: Fix 4 interconnected problems in the subagent system: (1) file security bypass, (2) vision multi-image support, (3) model resolution consistency, (4) OCR prompt restriction for glm-ocr.
>
> **Deliverables**:
> - New `src/security.rs` module with `validate_subagent_path()` — tilde expansion + CWD sandbox + blocklist
> - Security validation added to ALL subagent file-reading paths (6 total)
> - Vision multi-image parsing (comma-separated) in `spawn_subagent`
> - OcrProcessor/VisionProcessor accept model + model_options from SubagentConfig
> - `run_vision()`/`run_ocr()` use `self.config` instead of re-reading settings
> - glm-ocr prompt detection: override custom prompts with OcrMode prefixes when model starts with "glm-ocr"
> - Updated manual test scenarios
> - Regression tests for security and model resolution
>
> **Estimated Effort**: Medium
> **Parallel Execution**: YES - 3 waves + final verification
> **Critical Path**: Task 1 (security.rs) → Task 2-4 (parallel) → Task 5-7 (parallel) → Task 8 → F1-F4

---

## Context

### Original Request
User identified 4 issues:
1. "A leitura de arquivos não está passando pela nossa checagem extra de sandbox" — All subagent file reads bypass blocklist + CWD sandbox checks
2. "vision pode aceitar mais de uma imagem no file_path, mas só ele" — Vision should support multiple images, others should not
3. "Por que vision não leu a config? Ele está usando qwen3.5:9b em vez de kimi-k2.5:cloud" — Vision (and all subagents) should use config key → ModelConfig flow consistently
4. "No caso do OCR, especificamente quando glm-ocr for o modelo sendo usado, definir e permitir apenas os prompts padrão" — glm-ocr requires specific prompt prefixes; other models should allow custom prompts

### Interview Summary
**Key Discussions**:
- User chose `security.rs` module for path validation (not importing from `files.rs`)
- User chose global default model as fallback for vision/summarize/document when no user config
- Confirmed: CWD sandbox + blocklist is sufficient, no Landlock needed for subagents
- Confirmed: comma-separated parsing for vision multi-image in `spawn_subagent`

**Research Findings**:
- 8 file-reading paths bypass ALL security checks (6 in subagent paths, 2 in CLI paths)
- OcrProcessor hardcodes `"glm-ocr:bf16"` and `temperature(0.0)` — ignores SubagentConfig
- VisionProcessor hardcodes `temperature(0.1)` and `num_predict(2048)` — ignores SubagentConfig
- `run_vision()` calls `self.settings.get_subcommand_config("vision")` AGAIN instead of using `self.config`
- `run_ocr()` delegates to OcrProcessor which completely ignores SubagentConfig
- `BlocklistConfig::load()` needs caching (`Lazy`) — no caching currently
- `validate_path()` in `files.rs` does CWD sandbox + /tmp check — we need same logic in `security.rs`

### Metis Review
**Identified Gaps** (addressed):
- Document subagent passes file path in prompt text (LLM uses run_command) — validate at entry point anyway
- `import_document` tool also bypasses security — included in scope per user's "duas camadas" requirement
- `BlocklistConfig::load()` is expensive — must cache with `once_cell::sync::Lazy`
- OCR processor signature changes affect CLI paths — add parameters with backward-compatible defaults
- Prompt override should only activate when model_id starts with `"glm-ocr"`, not all OCR models

---

## Work Objectives

### Core Objective
Make all subagent file reads go through security validation, use ModelConfig consistently across all subagent types, support multi-image for vision, and restrict OCR prompts for glm-ocr.

### Concrete Deliverables
- `src/security.rs` — new module with `validate_subagent_path()` and cached `BlocklistConfig`
- Security validation in `src/chat/subagent.rs`, `src/tools/subagent_tools.rs`, `src/chat/command_handlers.rs`, `src/vision/processor.rs`, `src/ocr/processor.rs`
- Model resolution fixes in `src/chat/subagent.rs`, `src/vision/processor.rs`, `src/ocr/processor.rs`
- Multi-image parsing in `src/tools/subagent_tools.rs`
- glm-ocr prompt detection in `src/chat/subagent.rs`
- Updated manual test scenarios
- Regression tests

### Definition of Done
- [ ] `cargo test --all-features` passes (all 666+ tests)
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] All subagent file-reading paths validate against blocklist + CWD sandbox
- [ ] Vision supports comma-separated multi-image via `spawn_subagent`
- [ ] All subagents use `SubagentConfig::new(config_key)` consistently (no hardcoded models)
- [ ] glm-ocr model gets standard OCR prompt prefixes; other models keep custom prompts
- [ ] Manual test scenarios cover security, multi-image, model resolution, and OCR prompts

### Must Have
- Security validation (blocklist + CWD sandbox) on every subagent file-reading path
- `validate_subagent_path()` in new `src/security.rs` module (not in `files.rs`)
- `BlocklistConfig` caching with `once_cell::sync::Lazy`
- Vision multi-image: comma-separated parsing for `spawn_subagent("vision", ..., Some("path1,path2"))`
- OcrProcessor/VisionProcessor accept `model` and `model_options` parameters
- `run_vision()`/`run_ocr()` use `self.config` instead of re-reading settings
- glm-ocr prompt detection: when model starts with `"glm-ocr"`, use `OcrMode` prefixes

### Must NOT Have (Guardrails)
- G1: Do NOT add Landlock/seccomp for subagents — CWD sandbox + blocklist sufficient
- G2: Do NOT change `validate_path()` in `files.rs` — create separate `security.rs`
- G3: Do NOT change `SubagentConfig` struct — it already has `model` and `model_options`
- G4: Do NOT break CLI paths (`/ocr`, `/vision`) — they must continue working
- G5: Do NOT change `import_document` tool security in this fix (separate concern — but we WILL add subagent path validation)
- G6: Do NOT cache `BlocklistConfig` for `files.rs` tools — only for subagent security
- G7: Do NOT hardcode model names in new code — always resolve via config key → ModelConfig
- G8: Do NOT apply OCR prompt restriction to non-glm-ocr models — only when model_id starts with "glm-ocr"

---

## Verification Strategy (MANDATORY)

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: Tests-after
- **Framework**: Rust built-in (`cargo test`)

### QA Policy
Every task includes agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Library/Module**: Use `cargo test` — unit tests for security validation, model resolution, multi-image parsing
- **API/Backend**: Use Bash — integration-style verification

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately - foundation):
├── Task 1: Create security.rs module [quick]
└── Task 2: Add vision fallback in get_subcommand_config [quick]

Wave 2 (After Wave 1 - core fixes, MAX PARALLEL):
├── Task 3: Add security validation to all subagent paths [deep]
├── Task 4: Fix OcrProcessor to accept model + model_options [deep]
└── Task 5: Fix VisionProcessor to accept model_options [quick]

Wave 3 (After Wave 2 - integration features):
├── Task 6: Fix run_vision/run_ocr to use self.config [unspecified-high]
├── Task 7: Add multi-image support for vision in spawn_subagent [unspecified-high]
└── Task 8: Add glm-ocr prompt detection [quick]

Wave 4 (After Wave 3 - documentation + tests):
├── Task 9: Update manual test scenarios [writing]
└── Task 10: Add regression tests [quick]

Wave FINAL (After ALL tasks — 4 parallel reviews, then user okay):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Real manual QA (unspecified-high)
└── Task F4: Scope fidelity check (deep)
-> Present results -> Get explicit user okay
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 | - | 3 |
| 2 | - | 4, 5, 6 |
| 3 | 1 | - |
| 4 | 2 | 6 |
| 5 | 2 | 6 |
| 6 | 4, 5 | - |
| 7 | - | - |
| 8 | 4 | - |
| 9 | 6, 7, 8 | - |
| 10 | 3, 6, 7, 8 | F1-F4 |

### Agent Dispatch Summary

- **Wave 1**: 2 tasks - T1 → `quick`, T2 → `quick`
- **Wave 2**: 3 tasks - T3 → `deep`, T4 → `deep`, T5 → `quick`
- **Wave 3**: 3 tasks - T6 → `unspecified-high`, T7 → `unspecified-high`, T8 → `quick`
- **Wave 4**: 2 tasks - T9 → `writing`, T10 → `quick`
- **FINAL**: 4 tasks - F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

---

- [x] 1. Create `src/security.rs` module with `validate_subagent_path()`

  **What to do**:
  - Create `src/security.rs` with a `validate_subagent_path()` function that:
    1. Expands `~` in path using `expand_tilde_path()`
    2. Canonicalizes the path
    3. Checks that the canonical path is within CWD or `/tmp`/`/var/tmp`
    4. Loads `BlocklistConfig` (cached via `once_cell::sync::Lazy`) and checks `is_blocked_for_read()`
    5. Returns `Result<PathBuf, String>` where error messages are user-friendly
  - Cache `BlocklistConfig` using `once_cell::sync::Lazy<BlocklistConfig>` — load once, reuse forever
  - Add `pub mod security;` to `src/main.rs`
  - The function signature: `pub fn validate_subagent_path(path: &Path) -> Result<PathBuf, String>`
  - Helper: `pub fn validate_subagent_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String>` for vision multi-image
  - Follow the same logic as `validate_path()` in `files.rs:861-907` but return `Result<PathBuf, String>` (not `Result<PathBuf, Box<dyn Error>>`)
  - Follow the same blocklist check pattern as `read_file` in `files.rs:66-74`

  **Must NOT do**:
  - G2: Do NOT modify `validate_path()` in `files.rs`
  - G6: Do NOT add caching for `BlocklistConfig` in `files.rs`

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 2)
  - **Parallel Group**: Wave 1 (with Task 2)
  - **Blocks**: Task 3
  - **Blocked By**: None

  **References**:

  **Pattern References** (existing code to follow):
  - `src/tools/files.rs:861-907` — `validate_path()` — CWD sandbox pattern to replicate
  - `src/tools/files.rs:66-74` — Blocklist check pattern in `read_file()` function
  - `src/tools/files_blocklist.rs:38-69` — `DEFAULT_BLOCKED_PATTERNS` definitions
  - `src/tools/files_blocklist.rs:136-198` — `BlocklistConfig::load()` and `is_blocked_for_read()`
  - `src/tools/files_blocklist.rs:204-206` — `is_blocked_for_read()` function signature

  **WHY Each Reference Matters**:
  - `validate_path()`: Exact same security logic to replicate (CWD + /tmp check)
  - `read_file()`: Shows the call chain: expand_tilde → validate_path → is_blocked_for_read
  - `DEFAULT_BLOCKED_PATTERNS`: Need to understand what's blocked to document in tests
  - `BlocklistConfig::load()`: Need to understand for caching pattern
  - `is_blocked_for_read()`: Need to understand the signature for calling it

  **Acceptance Criteria**:
  - [ ] `src/security.rs` exists with `validate_subagent_path()` and `validate_subagent_paths()`
  - [ ] `BlocklistConfig` is cached via `once_cell::sync::Lazy`
  - [ ] `src/main.rs` includes `pub mod security;`
  - [ ] `cargo test --all-features` compiles and passes
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Blocklist blocks sensitive files
    Tool: Bash (cargo test)
    Preconditions: security.rs module compiles
    Steps:
      1. Run cargo test --all-features security
      2. Verify validate_subagent_path rejects .env files
    Expected Result: All security module tests pass, blocklist check works
    Evidence: .sisyphus/evidence/task-1-blocklist-check.txt

  Scenario: CWD sandbox restricts outside paths
    Tool: Bash (cargo test)
    Preconditions: security.rs module compiles
    Steps:
      1. Run cargo test --all-features security
      2. Verify validate_subagent_path("/etc/passwd") returns Err
    Expected Result: Security sandbox tests pass
    Evidence: .sisyphus/evidence/task-1-cwd-sandbox.txt
  ```

  **Commit**: YES (group with Task 2)
  - Message: `feat(security): add validate_subagent_path module for subagent file security`
  - Files: `src/security.rs`, `src/main.rs`

---

- [ ] 2. Add vision/summarize/document fallback in get_subcommand_config

  **What to do**:
  - In `src/settings.rs`, add explicit fallback cases for "vision", "summarize", and "document" in `get_subcommand_config()`:
    - "vision" → no special fallback needed (already falls through to `self.model.default` which is correct)
    - "summarize" → no special fallback needed (same as vision)
    - "document" → no special fallback needed (same as vision)
  - The real fix is that these subcommands already get `self.model.default` as fallback, which IS the global default. This is correct per user's choice.
  - However, verify the existing behavior is working: `get_subcommand_config("vision")` should return the user's config if set, otherwise the global default.
  - Add tests verifying: `get_subcommand_config("vision")` returns correct model; `get_subcommand_config("summarize")` returns correct model.

  **Must NOT do**:
  - G7: Do NOT hardcode model names — the fallback is already `self.model.default`

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 1)
  - **Parallel Group**: Wave 1 (with Task 1)
  - **Blocks**: Tasks 4, 5, 6
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/settings.rs:296-353` — `get_subcommand_config()` method with existing fallback chain
  - `src/settings.rs:317-324` — The `unwrap_or_else` block with "translate" and "ocr" fallbacks

  **WHY Each Reference Matters**:
  - `get_subcommand_config()`: This is the method we need to verify — it's the central model resolution for all subcommands
  - The fallback block: Shows the pattern for "translate" → "translategemma", "ocr" → "glm-ocr"

  **Acceptance Criteria**:
  - [ ] `get_subcommand_config("vision")` returns user config model if set, otherwise global default
  - [ ] `get_subcommand_config("summarize")` returns user config model if set, otherwise global default
  - [ ] Tests added for vision and summarize model resolution
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Vision model resolution from config
    Tool: Bash (cargo test)
    Preconditions: settings.rs compiles
    Steps:
      1. Run cargo test test_get_subcommand_config
      2. Verify vision returns configured model or default
    Expected Result: Vision model test passes
    Evidence: .sisyphus/evidence/task-2-vision-model.txt
  ```

  **Commit**: YES (group with Task 1)
  - Message: `feat(security): add validate_subagent_path module for subagent file security`
  - Files: `src/settings.rs`

---

- [x] 3. Add security validation to all subagent file-reading paths

  **What to do**:
  - Import `validate_subagent_path` and `validate_subagent_paths` from `crate::security`
  - Add validation at these 6 points:
  
  1. **`src/chat/subagent.rs:run_generate()` (line ~232-240)**: Before `tokio::fs::read(&path)`, call `validate_subagent_path(Path::new(&path))?` and use the returned canonical path
  
  2. **`src/chat/subagent.rs:run_vision()` (line ~374-398)**: Before creating `VisionArgs`, call `validate_subagent_paths(&paths)?` and use the returned validated paths
  
  3. **`src/chat/subagent.rs:run_ocr()` (line ~425-441)**: Before calling `OcrProcessor::process_file()`, call `validate_subagent_path(path)?`
  
  4. **`src/chat/subagent.rs:run_document()` (line ~467-553)**: Before checking `path.exists()`, call `validate_subagent_path(path)?`
  
  5. **`src/chat/command_handlers.rs:handle_subagent_ocr()` (line ~2846-2873)**: After `expand_tilde_path`, call `validate_subagent_path(&file_path)?`
  
  6. **`src/chat/command_handlers.rs:handle_subagent_vision()` (line ~2876-2912)**: After expanding tildes, call `validate_subagent_paths(&path_bufs)?`

  - Note: `spawn_subagent` in `subagent_tools.rs` passes `file_path` to `SubagentRunner::run()` which calls `run_generate()`. So validating inside the runner methods catches the tool path too.

  **Must NOT do**:
  - G1: Do NOT add Landlock for subagents
  - G5: Do NOT change `import_document` in this task

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 4, 5)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 10
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/security.rs` (Task 1) — `validate_subagent_path()` and `validate_subagent_paths()`
  - `src/chat/subagent.rs:232-260` — `run_generate()` — add validation before `tokio::fs::read`
  - `src/chat/subagent.rs:374-404` — `run_vision()` — add validation before creating VisionArgs
  - `src/chat/subagent.rs:425-441` — `run_ocr()` — add validation before OcrProcessor
  - `src/chat/subagent.rs:467-553` — `run_document()` — add validation before path.exists check
  - `src/chat/command_handlers.rs:2846-2873` — `handle_subagent_ocr()` — add validation after tilde expansion
  - `src/chat/command_handlers.rs:2876-2912` — `handle_subagent_vision()` — add validation after tilde expansion

  **WHY Each Reference Matters**:
  - `security.rs`: The new validation function to call
  - Each insertion point: Need to know exact line numbers and context for adding validation
  - Error return type: Must return `Ok(err_msg_string)` per AGENTS.md philosophy for tool paths, and `Err` for command handler paths

  **Acceptance Criteria**:
  - [ ] All 6 file-reading paths call `validate_subagent_path()` or `validate_subagent_paths()`
  - [ ] `run_generate()` validates and uses canonical path before `tokio::fs::read`
  - [ ] `run_vision()` validates all paths before creating VisionArgs
  - [ ] `run_ocr()` validates path before OcrProcessor
  - [ ] `run_document()` validates path before existence check
  - [ ] `/ocr` command handler validates path after tilde expansion
  - [ ] `/vision` command handler validates all paths after tilde expansion
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Subagent rejects path outside CWD
    Tool: Bash (cargo test)
    Preconditions: Tasks 1 and 3 complete
    Steps:
      1. Run cargo test --all-features
      2. Verify validation rejects /etc/passwd, ~/.ssh/id_rsa
    Expected Result: Paths outside CWD rejected with error message
    Failure Indicators: Test that tries to read /etc/passwd succeeds
    Evidence: .sisyphus/evidence/task-3-cwd-sandbox.txt

  Scenario: Subagent rejects blocked files even within CWD
    Tool: Bash (cargo test)
    Preconditions: Tasks 1 and 3 complete
    Steps:
      1. Create test .env file in CWD
      2. Verify validate_subagent_path rejects it
      3. Cleanup test file
    Expected Result: .env file rejected even within CWD
    Evidence: .sisyphus/evidence/task-3-blocklist.txt
  ```

  **Commit**: YES
  - Message: `feat(security): add path validation to all subagent file-reading paths`
  - Files: `src/chat/subagent.rs`, `src/chat/command_handlers.rs`

---

- [x] 4. Fix OcrProcessor to accept model + model_options from SubagentConfig

  **What to do**:
  - Modify `OcrProcessor::process_file()` in `src/ocr/processor.rs`:
    - Change signature from `(&self, path: &Path, mode: OcrMode, settings: &Settings)` to `(&self, path: &Path, mode: OcrMode, model: &str, model_options: ModelOptions)`
    - Remove hardcoded `"glm-ocr:bf16".to_string()` and `ModelOptions::default().temperature(0.0)` 
    - Use `model` parameter for `GenerationRequest::new()` and `model_options` for `.options()`
    - Remove `settings` parameter (no longer needed — was only used for `ollama_client()`)
    - Add `ollama: &Ollama` parameter instead of getting it from settings
  - Update all callers of `OcrProcessor::process_file()`:
    - `src/chat/subagent.rs:run_ocr()` — pass `self.config.model`, `self.config.model_options.clone()`, and `self.ollama.clone()`
    - `src/ocr/cli.rs` — pass the model from `settings.get_subcommand_config("ocr")` and settings.ollama_client()
    - `src/main.rs` OCR command — same pattern

  **Must NOT do**:
  - G4: Do NOT break CLI `/ocr` path
  - G7: Do NOT hardcode model names

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 3, 5)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 6
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `src/ocr/processor.rs:30-80` — `OcrProcessor::process_file()` — the function to modify
  - `src/ocr/processor.rs:49-52` — Hardcoded `"glm-ocr:bf16"` and `temperature(0.0)` to replace
  - `src/chat/subagent.rs:425-441` — `run_ocr()` caller to update
  - `src/ocr/cli.rs` — CLI caller that uses OcrProcessor

  **API/Type References**:
  - `src/chat/subagent.rs:112-123` — `SubagentConfig` struct with `model` and `model_options` fields
  - `ollama_rs::models::ModelOptions` — type for model options

  **WHY Each Reference Matters**:
  - `process_file()`: The core function to change — currently hardcodes model and temperature
  - `run_ocr()`: Must update to pass self.config values
  - `SubagentConfig`: Has the model and model_options that should be forwarded
  - `ModelOptions`: The type signature for the new parameter

  **Acceptance Criteria**:
  - [ ] `OcrProcessor::process_file()` accepts `model: &str`, `model_options: ModelOptions`, `ollama: &Ollama`
  - [ ] No hardcoded model names or temperatures remain in `OcrProcessor`
  - [ ] All callers updated (subagent.rs, cli.rs, main.rs)
  - [ ] `cargo test --all-features` passes
  - [ ] Existing OCR tests still pass

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: OcrProcessor uses SubagentConfig model
    Tool: Bash (cargo test)
    Preconditions: Task 4 complete
    Steps:
      1. Run cargo test --all-features ocr
      2. Verify OcrProcessor accepts model parameter
    Expected Result: All OCR tests pass, no hardcoded "glm-ocr:bf16"
    Evidence: .sisyphus/evidence/task-4-ocr-model.txt
  ```

  **Commit**: YES
  - Message: `refactor(ocr): accept model and model_options parameters in OcrProcessor`
  - Files: `src/ocr/processor.rs`, `src/chat/subagent.rs`, `src/ocr/cli.rs`, `src/main.rs`

---

- [x] 5. Fix VisionProcessor to accept model_options from SubagentConfig

  **What to do**:
  - Modify `VisionProcessor::process()` in `src/vision/processor.rs`:
    - Change signature to accept `model_options: ModelOptions` parameter
    - Remove hardcoded `ModelOptions::default().temperature(0.1).num_predict(args.max_tokens as i32)`
    - Use the passed `model_options` parameter instead, merging in `num_predict` from `args.max_tokens`
    - Specifically: `model_options.num_predict(args.max_tokens as i32)` — override only num_predict from args
  - Update callers:
    - `src/chat/subagent.rs:run_vision()` — pass `self.config.model_options.clone()`
    - `src/vision/cli.rs` — pass model options from resolved config

  **Must NOT do**:
  - G4: Do NOT break `/vision` CLI path
  - G7: Do NOT hardcode model names or temperatures

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 3, 4)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 6
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `src/vision/processor.rs:24-80` — `VisionProcessor::process()` — function to modify
  - `src/vision/processor.rs:53-55` — Hardcoded `temperature(0.1)` to replace
  - `src/chat/subagent.rs:374-404` — `run_vision()` caller to update

  **WHY Each Reference Matters**:
  - `process()`: Must accept model_options parameter
  - Hardcoded temperature: Must be removed, replaced by SubagentConfig values
  - `run_vision()`: Must pass self.config.model_options instead of hardcoding

  **Acceptance Criteria**:
  - [ ] `VisionProcessor::process()` accepts `model_options: ModelOptions`
  - [ ] No hardcoded temperature values in `VisionProcessor`
  - [ ] `num_predict` from `args.max_tokens` still applied on top of model_options
  - [ ] All callers updated
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: VisionProcessor uses SubagentConfig model_options
    Tool: Bash (cargo test)
    Preconditions: Task 5 complete
    Steps:
      1. Run cargo test --all-features vision
      2. Verify no hardcoded temperature(0.1)
    Expected Result: All vision tests pass
    Evidence: .sisyphus/evidence/task-5-vision-model-options.txt
  ```

  **Commit**: YES
  - Message: `refactor(vision): accept model_options parameter in VisionProcessor`
  - Files: `src/vision/processor.rs`, `src/chat/subagent.rs`, `src/vision/cli.rs`

---

- [x] 6. Fix run_vision/run_ocr to use self.config instead of re-reading settings

  **What to do**:
  - **`run_vision()` (subagent.rs:374-404)**: 
    - Remove the line `let (model, _thinking, _tools) = self.settings.get_subcommand_config("vision");`
    - Use `self.config.model.clone()` instead of `&model`
    - Pass `self.config.model_options.clone()` to `VisionProcessor::process()` (after Task 5)
    - The run_vision method already receives `self` which has all config needed
  - **`run_ocr()` (subagent.rs:425-441)**:
    - Remove `settings: &Settings` parameter (no longer needed after Task 4)
    - Pass `self.config.model.clone()` and `self.config.model_options.clone()` to `OcrProcessor::process_file()`
    - Pass `self.ollama.clone()` instead of getting it from settings
  - Update all callers of `run_ocr()` to remove the `settings` parameter if it changes

  **Must NOT do**:
  - G3: Do NOT change SubagentConfig struct (model and model_options already exist)
  - G4: Do NOT break CLI paths

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 7, 8)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 9, Task 10
  - **Blocked By**: Tasks 4, 5

  **References**:

  **Pattern References**:
  - `src/chat/subagent.rs:374-404` — `run_vision()` — remove settings re-read, use self.config
  - `src/chat/subagent.rs:425-441` — `run_ocr()` — remove settings param, use self.config
  - `src/chat/subagent.rs:383` — `self.settings.get_subcommand_config("vision")` to remove

  **WHY Each Reference Matters**:
  - `run_vision()`: Currently IGNORES self.config.model for vision and re-reads from settings
  - `run_ocr()`: Currently delegates to OcrProcessor which hardcodes model/options — but after Task 4, OcrProcessor accepts parameters
  - Line 383: The exact line that needs to be removed/replaced

  **Acceptance Criteria**:
  - [ ] `run_vision()` uses `self.config.model` and `self.config.model_options`
  - [ ] `run_ocr()` uses `self.config.model` and `self.config.model_options`
  - [ ] No `self.settings.get_subcommand_config("vision")` call in `run_vision()`
  - [ ] `run_ocr()` doesn't take `settings` parameter (uses self.ollama instead)
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Vision uses SubagentConfig model
    Tool: Bash (cargo test)
    Preconditions: Tasks 4, 5, 6 complete
    Steps:
      1. Run cargo test --all-features subagent
      2. Verify no settings re-read in run_vision
    Expected Result: Vision config tests pass
    Evidence: .sisyphus/evidence/task-6-vision-config.txt
  ```

  **Commit**: YES
  - Message: `fix(subagent): use self.config for model and model_options in run_vision/run_ocr`
  - Files: `src/chat/subagent.rs`

---

- [x] 7. Add multi-image support for vision in spawn_subagent

  **What to do**:
  - In `src/tools/subagent_tools.rs`, modify `spawn_subagent()` to parse comma-separated `file_path` when `subagent_type == "vision"`:
    ```rust
    // If vision, parse comma-separated paths
    let resolved_paths = if agent_type == SubagentType::Vision {
        match &resolved_path {
            Some(p) => parse_comma_separated_paths(p)?, // returns Vec<PathBuf>
            None => return Ok(error_message),
        }
    } else {
        // Single path for OCR
        match &resolved_path {
            Some(p) => vec![PathBuf::from(p)],
            None => return Ok(error_message),
        }
    };
    ```
  - Add a `parse_comma_separated_paths()` helper that:
    - Splits on `,`
    - Expands tilde for each path
    - Validates each path with `validate_subagent_paths()`
    - Returns `Vec<PathBuf>`
  - Modify `SubagentRunner::run()` to accept `Vec<PathBuf>` or modify the vision branch:
    - For vision: pass `resolved_paths.as_slice()` to `run_vision()`
    - For OCR: pass single path `&resolved_paths[0]` to `run_generate()` or `run_ocr()`
  - Update the `run()` method dispatch to handle `Vec<PathBuf>` for vision
  - Update error message for missing file_path when type is OCR to be clear it's a single path
  - Update `spawn_subagent` docstring to document multi-image for vision

  **Must NOT do**:
  - G4: Do NOT break single-image vision calls (backward compatible)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 6, 8)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 9, Task 10
  - **Blocked By**: Task 1 (security.rs needed for path validation)

  **References**:

  **Pattern References**:
  - `src/tools/subagent_tools.rs:69-177` — `spawn_subagent()` function to modify
  - `src/tools/subagent_tools.rs:110-121` — file_path validation for OCR/Vision
  - `src/tools/subagent_tools.rs:124` — tilde expansion for file_path
  - `src/chat/subagent.rs:205-221` — `SubagentRunner::run()` method dispatch

  **WHY Each Reference Matters**:
  - `spawn_subagent()`: Entry point for LLM-initiated subagent calls — must parse multi-image
  - file_path validation: Currently validates single path, must support multiple for vision
  - `run()` method: Must be updated to pass Vec<PathBuf> for vision

  **Acceptance Criteria**:
  - [ ] `spawn_subagent("vision", ..., Some("path1.png,path2.jpg"))` processes both images
  - [ ] `spawn_subagent("ocr", ..., Some("path.png"))` still works with single image
  - [ ] `spawn_subagent("vision", ..., Some("single.png"))` still works (single image backward compatible)
  - [ ] Error for missing file_path on OCR is clear: "file_path is required"
  - [ ] Each path in multi-image goes through security validation
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Multi-image vision parses comma-separated paths
    Tool: Bash (cargo test)
    Preconditions: Task 7 complete
    Steps:
      1. Test parse_comma_separated_paths("a.png,b.jpg")
      2. Verify returns Vec of 2 PathBufs
    Expected Result: 2 paths parsed correctly
    Failure Indicators: Single path returned, or comma not split
    Evidence: .sisyphus/evidence/task-7-multi-image.txt

  Scenario: Single path vision still works
    Tool: Bash (cargo test)
    Preconditions: Task 7 complete
    Steps:
      1. Test parse_comma_separated_paths("a.png")
      2. Verify returns Vec of 1 PathBuf
    Expected Result: 1 path parsed correctly
    Evidence: .sisyphus/evidence/task-7-single-image.txt
  ```

  **Commit**: YES
  - Message: `feat(subagent): add multi-image support for vision in spawn_subagent`
  - Files: `src/tools/subagent_tools.rs`, `src/chat/subagent.rs`

---

- [x] 8. Add glm-ocr prompt detection and restriction

  **What to do**:
  - In `src/chat/subagent.rs`, add a method or logic in `run()` (or `run_ocr()`) that:
    - When `self.config.model` starts with `"glm-ocr"`, overrides the prompt with the appropriate `OcrMode` prefix
    - When the model is NOT `"glm-ocr"`, keeps the user's custom prompt as-is
  - The logic should be:
    ```rust
    // In run_ocr() or the OCR branch of run():
    let effective_prompt = if self.config.model.starts_with("glm-ocr") {
        // glm-ocr requires specific prompt prefixes — use default OcrMode::Text
        OcrMode::Text.into_prompt().to_string()
    } else {
        // Other models accept custom prompts
        prompt
    };
    ```
  - For the `/ocr` command, this is already handled (users specify mode explicitly via `OcrMode`)
  - For `spawn_subagent`, the LLM provides a free-form prompt — we need to override it when glm-ocr is the model
  - For `spawn_subagent`, add a new optional parameter `ocr_mode` or detect from prompt content? **Decision**: Always use OcrMode::Text as default when overriding for glm-ocr. The `OcrMode` enum is already public.
  - Update `spawn_subagent` docstring to note that OCR prompts are overridden for glm-ocr compatibility

  **Must NOT do**:
  - G8: Do NOT apply prompt restriction to non-glm-ocr models
  - G7: Do NOT hardcode "glm-ocr" — use `starts_with("glm-ocr")` check

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 6, 7)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 10
  - **Blocked By**: Task 4 (OcrProcessor API change)

  **References**:

  **Pattern References**:
  - `src/ocr/mode.rs:27-34` — `OcrMode::into_prompt()` — the standard OCR prompt prefixes
  - `src/chat/subagent.rs:425-441` — `run_ocr()` — where to add prompt override logic
  - `src/tools/subagent_tools.rs:19-21` — `OCR_SYSTEM_PROMPT` constant

  **WHY Each Reference Matters**:
  - `OcrMode::into_prompt()`: The exact prompt prefixes to use when glm-ocr is detected
  - `run_ocr()`: Where to add the model detection and prompt override
  - `OCR_SYSTEM_PROMPT`: The current system prompt for OCR subagent — should remain as-is (it's the system prompt, not the user prompt)

  **Acceptance Criteria**:
  - [ ] When `self.config.model.starts_with("glm-ocr")`, user prompt is replaced with `OcrMode::Text.into_prompt()`
  - [ ] When model does NOT start with "glm-ocr", user prompt is kept as-is
  - [ ] `/ocr` command with explicit mode still works (not affected)
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: glm-ocr model gets OcrMode::Text prefix
    Tool: Bash (cargo test)
    Preconditions: Task 8 complete
    Steps:
      1. Create SubagentConfig with model "glm-ocr:bf16"
      2. Call run with custom prompt "extract all text"
      3. Verify prompt is replaced with "Text Recognition:"
    Expected Result: Custom prompt replaced for glm-ocr
    Evidence: .sisyphus/evidence/task-8-glm-ocr-prompt.txt

  Scenario: Non-glm-ocr model keeps custom prompt
    Tool: Bash (cargo test)
    Preconditions: Task 8 complete
    Steps:
      1. Create SubagentConfig with model "moondream:1.8b"
      2. Call run with custom prompt "describe this image"
      3. Verify prompt is kept as-is
    Expected Result: Custom prompt preserved for non-glm-ocr
    Evidence: .sisyphus/evidence/task-8-custom-prompt.txt
  ```

  **Commit**: YES
  - Message: `fix(subagent): override custom prompts for glm-ocr model compatibility`
  - Files: `src/chat/subagent.rs`

---

- [x] 9. Update manual test scenarios

  **What to do**:
  - Create or update manual test scenarios document covering:
    1. Security: Try to read /etc/passwd, ~/.ssh/id_rsa, ~/.env via OCR/vision/document subagent
    2. Security: Try to read files outside CWD (e.g., ~/testfiles/vision/test.jpg should be rejected unless in CWD)
    3. Security: Try to read blocked files within CWD (e.g., .env in project dir)
    4. Multi-image vision: `spawn_subagent("vision", "describe", Some("img1.png,img2.png"))`
    5. Single-image vision: `spawn_subagent("vision", "describe", Some("img1.png"))` still works
    6. Model resolution: Verify OCR uses glm-ocr config, vision uses user config, translate uses translategemma
    7. OCR prompt: Verify glm-ocr gets "Text Recognition:" prefix, custom model keeps custom prompt
    8. All subcommand CLI commands still work: /ocr, /vision, /translate, /summarize

  **Must NOT do**:
  - G4: Do NOT break existing CLI commands

  **Recommended Agent Profile**:
  - **Category**: `writing`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 10)
  - **Parallel Group**: Wave 4
  - **Blocks**: F1-F4
  - **Blocked By**: Tasks 6, 7, 8

  **References**:

  **Pattern References**:
  - `MANUAL-TEST-P4.md` (if exists) — Previous manual test scenarios to update

  **Acceptance Criteria**:
  - [ ] Manual test document exists with scenarios for all 4 fixes
  - [ ] Each scenario has exact steps, expected results, and failure indicators
  - [ ] Document covers both tool path (`spawn_subagent`) and CLI path (`/ocr`, `/vision`)

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: Manual test document completeness
    Tool: Bash (ls)
    Preconditions: Task 9 complete
    Steps:
      1. Verify manual test document exists
      2. Check it covers security, multi-image, model resolution, OCR prompt
    Expected Result: Document exists with all required sections
    Evidence: .sisyphus/evidence/task-9-manual-test.txt
  ```

  **Commit**: YES
  - Message: `docs(tests): add manual test scenarios for subagent security and model resolution`
  - Files: `MANUAL-TEST-SUBAGENT-SECURITY.md` (or update existing)

---

- [x] 10. Add regression tests for security, model resolution, and multi-image

  **What to do**:
  - Add tests to `src/security.rs` (or `src/security/tests.rs`):
    - `test_validate_subagent_path_rejects_outside_cwd` — path outside CWD returns Err
    - `test_validate_subagent_path_accepts_cwd_file` — path within CWD returns Ok
    - `test_validate_subagent_path_accepts_tmp` — path in /tmp returns Ok
    - `test_validate_subagent_path_rejects_blocked_env` — .env file returns Err
    - `test_validate_subagent_path_rejects_blocked_ssh` — SSH key returns Err
    - `test_validate_subagent_paths_multi` — multiple paths validated
  - Add tests to `src/chat/subagent.rs`:
    - `test_run_generate_validates_path` — verifies path validation before file read
    - `test_ocr_prompt_override_for_glm_ocr` — verifies prompt override when model starts with "glm-ocr"
    - `test_ocr_prompt_preserved_for_other_models` — verifies custom prompt kept for non-glm-ocr
    - `test_vision_config_used_not_settings` — verifies self.config is used, not settings re-read
  - Add tests to `src/tools/subagent_tools.rs`:
    - `test_parse_comma_separated_paths` — verifies multi-image parsing
    - `test_single_path_vision` — verifies single path still works

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO — needs Tasks 3, 6, 7, 8 complete
  - **Parallel Group**: Wave 4 (with Task 9)
  - **Blocks**: F1-F4
  - **Blocked By**: Tasks 3, 6, 7, 8

  **References**:

  **Pattern References**:
  - `src/chat/subagent.rs:#tests` — Existing subagent tests (pattern reference)
  - `src/tools/subagent_tools.rs:#tests` — Existing tool tests (pattern reference)
  - `src/tools/files_blocklist.rs:#tests` — Blocklist test patterns
  - `src/tools/files.rs:#tests` — Files security test patterns

  **WHY Each Reference Matters**:
  - Existing test patterns: Need to follow the same testing style
  - Blocklist tests: Pattern for testing is_blocked_for_read
  - Files security tests: Pattern for testing validate_path

  **Acceptance Criteria**:
  - [ ] All new tests pass with `cargo test --all-features`
  - [ ] `cargo clippy --all-features -- -D warnings` passes
  - [ ] Security tests cover: CWD sandbox, blocklist, /tmp allowance
  - [ ] Model resolution tests cover: glm-ocr, translategemma, vision config
  - [ ] Multi-image tests cover: comma parsing, single path backward compat
  - [ ] OCR prompt tests cover: glm-ocr override, non-glm-ocr preservation

  **QA Scenarios (MANDATORY)**:

  ```
  Scenario: All regression tests pass
    Tool: Bash (cargo test)
    Preconditions: All tasks complete
    Steps:
      1. Run cargo test --all-features
      2. Verify 0 failures
    Expected Result: All tests pass (existing + new)
    Failure Indicators: Any test failure
    Evidence: .sisyphus/evidence/task-10-regression-tests.txt

  Scenario: Clippy clean
    Tool: Bash (cargo clippy)
    Preconditions: All tasks complete
    Steps:
      1. Run cargo clippy --all-features -- -D warnings
    Expected Result: Zero warnings
    Failure Indicators: Any clippy warning
    Evidence: .sisyphus/evidence/task-10-clippy.txt
  ```

  **Commit**: YES
  - Message: `test(subagent): add regression tests for security, model resolution, and multi-image`
  - Files: `src/security.rs`, `src/chat/subagent.rs`, `src/tools/subagent_tools.rs`

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, check function signature, grep for pattern). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Check evidence files exist in .sisyphus/evidence/. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-high`
  Run `tsc --noEmit` + linter + `cargo test`. Review all changed files for: `as any`/`@ts-ignore`, empty catches, console.log in prod, commented-out code, unused imports. Check AI slop: excessive comments, over-abstraction, generic names. Check security: are ALL file-reading paths validated? Are blocked patterns checked? Is CWD sandbox enforced?
  Output: `Build [PASS/FAIL] | Lint [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [x] F3. **Real Manual QA** — `unspecified-high`
  Start from clean state. Execute EVERY QA scenario from EVERY task. Test cross-task integration. Test edge cases: empty paths, non-image files for vision, very long paths, paths with spaces. Save evidence to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff. Verify 1:1 — everything in spec was built (no missing), nothing beyond spec was built (no creep). Check "Must NOT Have" compliance. Detect cross-task contamination.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

- **Wave 1**: `feat(security): add validate_subagent_path module for subagent file security` (Tasks 1+2)
- **Wave 2**: `refactor(ocr): accept model and model_options parameters in OcrProcessor` (Task 4)
- **Wave 2**: `refactor(vision): accept model_options parameter in VisionProcessor` (Task 5)
- **Wave 2**: `feat(security): add path validation to all subagent file-reading paths` (Task 3)
- **Wave 3**: `fix(subagent): use self.config for model and model_options in run_vision/run_ocr` (Task 6)
- **Wave 3**: `feat(subagent): add multi-image support for vision in spawn_subagent` (Task 7)
- **Wave 3**: `fix(subagent): override custom prompts for glm-ocr model compatibility` (Task 8)
- **Wave 4**: `docs(tests): add manual test scenarios` + `test(subagent): add regression tests` (Tasks 9+10)

---

## Success Criteria

### Verification Commands
```bash
cargo test --all-features     # Expected: All tests pass (670+)
cargo clippy --all-features -- -D warnings  # Expected: Zero warnings
cargo build --features all-tools  # Expected: Clean build
```

### Final Checklist
- [ ] All "Must Have" present
- [ ] All "Must NOT Have" absent
- [ ] All tests pass
- [ ] All subagent file paths validated (blocklist + CWD sandbox)
- [ ] Vision multi-image works via comma-separated paths
- [ ] All subagents use config key → ModelConfig flow consistently
- [ ] glm-ocr gets standard OCR prompt prefixes; other models keep custom prompts