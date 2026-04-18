# Fix: Subagent Model Resolution + Prompt Quality

## TL;DR

> **Quick Summary**: Fix 3 bugs where subagents use wrong model/temperature references and non-existent tools in prompts, plus improve delegation documentation.
> 
> **Deliverables**:
> - Fix `get_subcommand_config("translate")` to return "translategemma:4b" instead of default
> - Integrate `user_models::resolve_model_config()` into SubagentConfig so subagents use per-model temperature/num_ctx
> - Rewrite subagent system prompts to describe direct task execution (not non-existent tools)
> - Improve `spawn_subagent` tool documentation for delegation strategy
> - Add missing user-facing docs for the subagent system
> 
> **Estimated Effort**: Medium
> **Parallel Execution**: YES - 3 waves
> **Critical Path**: Task 1 → Task 2+3 (parallel) → Task 4 → Task 5+6 (parallel)

---

## Context

### Original Request
User reported: "Os modelos usados nos subagentes, em especial translate, não são os modelos cadastrados via models.toml. Deveria ser." Also: "Verificar se os subagentes estão apropriadamente documentados com relação a como devem ser chamados, e se seus devidos prompts estão corretos."

### Root Cause Analysis
1. **Wrong model for translate**: `get_subcommand_config("translate")` falls back to `self.model.default` ("qwen3.5:4b") instead of "translategemma:4b". The translate fallback only existed in `main.rs` CLI handler, not in `get_subcommand_config()`.
2. **Wrong temperature for all subagents**: `SubagentConfig::default_model_options()` returns `temperature(0.0)`, but translategemma uses 0.2, glm-ocr uses 0.1, etc. from ModelConfig in `config.rs`.
3. **Subagent prompts reference non-existent tools**: Prompts say "use the `ocr_image` tool", "call `translate_text`" etc. but subagents run BARE — no tools are registered (except Document which has `run_command`).

### Investigation Summary
**Key Discussions**:
- User confirmed: use Full ModelConfig (temperature, num_ctx from models.toml)
- User confirmed: tests-after strategy (not TDD)
- User clarified: subagents operate as CLI-module replacements within chat, composing context as tools

**Research Findings**:
- `settings.rs:317` has special case for `"ocr"` → "glm-ocr:bf16" but NO such case for `"translate"`
- CLI translate handler in `main.rs:241-247` has `"translategemma"` fallback but this is NOT in `get_subcommand_config()`
- All 3 subagent paths (tool, slash command, direct runner) bypass `user_models::resolve_model_config()`
- `subagent.rs:509` (run_document) also hardcodes `temperature(0.0)`

### Metis Review
**Identified Gaps** (addressed):
- `run_document()` at line 509 also hardcodes temperature — MUST fix
- Command handler path also uses bare `SubagentConfig::new(model, "brief string")` — MUST fix both paths
- Test for `default_model_options()` returning temperature(0.0) will break — update test expectations

---

## Work Objectives

### Core Objective
Make subagents use the correct model name and ModelConfig (temperature, num_ctx) from config.toml/models.toml, matching CLI behavior. Fix prompt quality to accurately describe subagent behavior.

### Concrete Deliverables
- `get_subcommand_config("translate")` returns "translategemma:4b" as fallback
- All subagent paths use `user_models::resolve_model_config()` for ModelConfig
- Subagent system prompts describe direct task execution (no tool references)
- `spawn_subagent` tool documentation includes delegation strategy
- User-facing docs for subagent system in `doc/src/tools.md`

### Definition of Done
- [x] `cargo test --all-features` passes
- [x] `cargo clippy --all-features -- -D warnings` passes
- [x] `/translate` in chat uses translategemma model with temp 0.2
- [x] `spawn_subagent` with type "translate" uses translategemma with temp 0.2
- [x] `/ocr` in chat uses glm-ocr with temp 0.1
- [x] All subagent prompts describe direct task execution, no phantom tools

### Must Have
- `get_subcommand_config("translate")` returns "translategemma:4b" fallback (matching "ocr" pattern)
- `SubagentConfig` carries `Option<ModelOptions>` field resolved from `user_models::resolve_model_config()`
- `SubagentRunner.run_chat()` and `run_generate()` use ModelConfig's options (not hardcoded temp=0.0)
- `SubagentRunner.run_document()` uses ModelConfig's options (not hardcoded temp=0.0)
- All system prompts in `subagent_tools.rs` rewritten to describe task execution, not tool usage
- Both call paths fixed: `subagent_tools.rs` (tool) AND `command_handlers.rs` (slash commands)

### Must NOT Have (Guardrails)
- Do NOT modify CLI subcommand code in `main.rs` (it already works correctly)
- Do NOT change config.toml structure or backward compatibility
- Do NOT modify SubagentRunner architecture (keep it lightweight, no Coordinator for text subagents)
- Do NOT give database access to subagents (G3)
- Do NOT add `spawn_subagent` to subagent tool whitelists (G5)
- Do NOT modify `config.rs` built-in model definitions
- Do NOT add "translategemma" as a hardcoded string in SubagentConfig — use get_subcommand_config()

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** - ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: Tests-after (add regression tests for model resolution)
- **Framework**: cargo test

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Backend**: Use Bash (cargo test, cargo clippy) — Build, lint, test

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately - model resolution fix):
├── Task 1: Fix translate model fallback in get_subcommand_config [quick]

Wave 2 (After Wave 1 - ModelConfig integration):
├── Task 2: Integrate ModelConfig into SubagentConfig + SubagentRunner [deep]

Wave 3 (After Wave 2 - prompts + docs, parallel):
├── Task 4: Rewrite subagent system prompts [quick]
├── Task 5: Improve spawn_subagent tool docs + delegation guidance [quick]
├── Task 6: Add user-facing subagent system docs [writing]

Wave FINAL (After ALL tasks):
├── F1: Plan compliance audit (oracle)
├── F2: Code quality review (unspecified-high)
├── F3: Real manual QA (unspecified-high)
├── F4: Scope fidelity check (deep)
→ Present results → Get explicit user okay

Critical Path: Task 1 → Task 2 → Task 4+5+6 → F1-F4 → user okay
Parallel Speedup: ~50% faster than sequential
Max Concurrent: 3 (Wave 3)
```

### Dependency Matrix

| Task | Depends On | Blocks | Wave |
|------|-----------|--------|------|
| 1 | — | 2 | 1 |
| 2 | 1 | 4, 5, 6 | 2 |
| 4 | 2 | F1-F4 | 3 |
| 5 | 2 | F1-F4 | 3 |
| 6 | 2 | F1-F4 | 3 |

### Agent Dispatch Summary

- **Wave 1**: 1 task - T1 → `quick`
- **Wave 2**: 1 task - T2 → `deep`
- **Wave 3**: 3 tasks - T4 → `quick`, T5 → `quick`, T6 → `writing`
- **FINAL**: 4 tasks - F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [x] 1. Fix translate model fallback in `get_subcommand_config()`

  **What to do**:
  - In `src/settings.rs`, modify `get_subcommand_config()` method (around line 313-322)
  - Add `else if subcommand == "translate"` branch that returns `"translategemma:4b".to_string()`
  - Follow the exact pattern of the existing `"ocr"` branch at line 317-318
  - The result: when no `[model.translate]` config is set, translate subagents get translategemma:4b instead of qwen3.5:4b

  **Must NOT do**:
  - Do NOT modify the CLI translate handler in `main.rs` — it already has the correct fallback
  - Do NOT change the config.toml structure
  - Do NOT modify other subcommand branches in get_subcommand_config()

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (foundation for Task 2)
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 2
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/settings.rs:315-318` — Existing `"ocr"` special case to follow
  - `src/main.rs:241-247` — CLI translate handler that has the "translategemma" fallback (for understanding, NOT for modification)

  **API/Type References**:
  - `src/settings.rs:296-350` — Full `get_subcommand_config()` method
  - `src/config.rs` — Built-in model definitions showing translategemma has temperature 0.2 and num_ctx 4096

  **WHY Each Reference Matters**:
  - `settings.rs:315-318`: This is the exact pattern to add the translate branch — just add an `else if subcommand == "translate"` before the final `else`
  - `main.rs:241-247`: Shows how the CLI does it correctly — confirms "translategemma" (without `:4b`) is the model name that gets resolved via `user_models::get_model_config()`

  **Acceptance Criteria**:

  - [ ] `get_subcommand_config("translate")` returns `("translategemma:4b", false, false)` when `[model.translate]` is not set
  - [ ] `get_subcommand_config("translate")` returns `([user-configured-model], ..., ...)` when `[model.translate] model = "custom-model"` IS set
  - [ ] `get_subcommand_config("ocr")` still returns `("glm-ocr:bf16", false, false)` (no regression)
  - [ ] `get_subcommand_config("summarize")` still returns default model (no regression)
  - [ ] `cargo test --all-features` passes
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Translate model fallback returns translategemma
    Tool: Bash (cargo test)
    Preconditions: Changes to settings.rs compiled
    Steps:
      1. cargo test --lib settings -- --nocapture
      2. Verify translate fallback test passes
    Expected Result: All settings tests pass, translate returns "translategemma:4b"
    Failure Indicators: Any test failure related to model resolution
    Evidence: .sisyphus/evidence/task-1-translate-fallback.txt

  Scenario: No regression in OCR/summarize/vision/model defaults
    Tool: Bash (cargo test)
    Preconditions: Changes to settings.rs compiled
    Steps:
      1. cargo test --all-features
      2. Verify all existing tests pass (626+ tests)
    Expected Result: 0 failures, 0 warnings
    Evidence: .sisyphus/evidence/task-1-no-regression.txt
  ```

  **Commit**: YES
  - Message: `fix(settings): add translate model fallback in get_subcommand_config()`
  - Files: `src/settings.rs`, `src/settings.rs` (tests)
  - Pre-commit: `cargo test --lib settings`

---

- [x] 2. Integrate ModelConfig into SubagentConfig + SubagentRunner

  **What to do**:
  - **SubagentConfig** (`src/chat/subagent.rs`): Add `model_options: ModelOptions` field (replace the `default_model_options()` method)
  - Create a builder function or modify `SubagentConfig::new()` to accept `ModelOptions`
  - Build `ModelOptions` from `user_models::resolve_model_config(&model_name)` where model_name comes from `get_subcommand_config()`
  - **SubagentRunner**: Use `self.config.model_options.clone()` instead of `self.config.default_model_options()` in:
    - `run_generate()` (line ~224) — for OCR and Vision
    - `run_chat()` (line ~251) — for Translate and Summarize
    - `run_summarize()` (line ~321) — for Summarize
    - `run_document()` (line ~509) — for Document
  - **Callers**: Update both paths:
    - `src/tools/subagent_tools.rs`: `build_*_config()` functions need to resolve ModelConfig and build ModelOptions
    - `src/chat/command_handlers.rs`: `handle_subagent_*` functions need the same resolution
  - **Fallback**: If `user_models::resolve_model_config()` returns None (model not found), log a warning and fall back to `ModelOptions::default().temperature(0.0)`

  **Must NOT do**:
  - Do NOT change `SubagentRunner` to use `CustomCoordinator` for text-based subagents (keep it lightweight)
  - Do NOT give database access to subagents
  - Do NOT add tools to subagents (except Document which already has `run_command`)
  - Do NOT modify the CLI handlers in `main.rs`

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Task 1 for translate fallback)
  - **Parallel Group**: Wave 2
  - **Blocks**: Tasks 4, 5, 6
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/main.rs:241-263` — CLI translate handler: `user_models::get_model_config(&translate_model)` then `model_config.build_model_options()`
  - `src/summarize/processor.rs:36-41` — SummarizeProcessor: `let model_config = crate::user_models::resolve_model_config(model_id);`
  - `src/settings.rs:296-350` — `get_subcommand_config()` method that returns model name

  **API/Type References**:
  - `src/chat/subagent.rs:109-147` — `SubagentConfig` struct and its builder methods
  - `src/chat/subagent.rs:144-146` — `default_model_options()` that returns `temperature(0.0)` — this needs to change
  - `src/config.rs` — `ModelConfig` struct with `build_model_options()` method
  - `src/user_models.rs` — `resolve_model_config()` and `get_model_config()` functions

  **WHY Each Reference Matters**:
  - `main.rs:241-263`: The CORRECT pattern that subagents should follow — resolve ModelConfig, build options from it
  - `subagent.rs:144-146`: The INCORRECT pattern that returns `ModelOptions::default().temperature(0.0)` — this is what we're replacing
  - `config.rs`: Contains ModelConfig definitions with per-model temperature/num_ctx values

  **Acceptance Criteria**:

  - [ ] `SubagentConfig` struct has a `model_options: ModelOptions` field
  - [ ] `SubagentRunner.run_generate()` uses `self.config.model_options` instead of `self.config.default_model_options()`
  - [ ] `SubagentRunner.run_chat()` uses `self.config.model_options` instead of `self.config.default_model_options()`
  - [ ] `SubagentRunner.run_summarize()` uses `self.config.model_options` instead of hardcoded `ModelOptions::default().temperature(0.0)`
  - [ ] `SubagentRunner.run_document()` uses `self.config.model_options` instead of hardcoded `ModelOptions::default().temperature(0.0)`
  - [ ] `build_translate_config()` in `subagent_tools.rs` resolves ModelConfig with translategemma:4b and uses temperature 0.2
  - [ ] `build_ocr_config()` resolves ModelConfig with glm-ocr:bf16 and uses temperature 0.1
  - [ ] `cargo test --all-features` passes
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Translate subagent uses correct ModelConfig options
    Tool: Bash (cargo test)
    Preconditions: Changes compiled
    Steps:
      1. cargo test --lib subagent -- --nocapture
      2. Verify SubagentConfig tests pass
    Expected Result: translate config has temperature=0.2, not 0.0
    Evidence: .sisyphus/evidence/task-2-translate-modelconfig.txt

  Scenario: OCR subagent uses correct ModelConfig options
    Tool: Bash (cargo test)
    Preconditions: Changes compiled
    Steps:
      1. cargo test --lib subagent -- --nocapture
      2. Verify OCR config has temperature=0.1, not 0.0
    Expected Result: OCR uses glm-ocr:bf16 with temperature 0.1
    Evidence: .sisyphus/evidence/task-2-ocr-modelconfig.txt

  Scenario: No regressions in existing tests
    Tool: Bash (cargo test)
    Preconditions: Changes compiled
    Steps:
      1. cargo test --all-features
    Expected Result: 626+ tests pass, 0 failures
    Evidence: .sisyphus/evidence/task-2-no-regression.txt
  ```

  **Commit**: YES
  - Message: `feat(subagent): integrate ModelConfig into SubagentConfig for per-model temperature and num_ctx`
  - Files: `src/chat/subagent.rs`, `src/tools/subagent_tools.rs`, `src/chat/command_handlers.rs`
  - Pre-commit: `cargo test --all-features`

---

- [x] 3. ~~(merged into Task 2)~~

---

- [x] 4. Rewrite subagent system prompts to describe direct task execution

  **What to do**:
  - Rewrite all 5 system prompts in `src/tools/subagent_tools.rs` (lines 19-36):
    - `OCR_SYSTEM_PROMPT`: Remove "call `ocr_image`" references, describe direct OCR task execution
    - `VISION_SYSTEM_PROMPT`: Remove "call `describe_image`" references, describe direct image analysis
    - `TRANSLATE_SYSTEM_PROMPT`: Remove "call `translate_text`" references, describe direct translation
    - `SUMMARIZE_SYSTEM_PROMPT` is built dynamically via `build_system_prompt()` — verify it's correct
    - `DOCUMENT_SYSTEM_PROMPT`: Remove any tool references except `run_command` (which IS available)
  - Each prompt should:
    1. State the role clearly
    2. Describe what input to expect
    3. Describe what output format to produce
    4. NOT reference any tools (except Document which has `run_command`)
    5. Be concise (under 200 chars) to minimize token usage

  **Must NOT do**:
  - Do NOT make prompts verbose or over-engineered — keep them minimal
  - Do NOT add tool definitions to subagent prompts (they run bare)
  - Do NOT change Document subagent prompt to remove `run_command` reference (it IS available)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 5 and 6)
  - **Parallel Group**: Wave 3
  - **Blocks**: F1-F4
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `src/tools/subagent_tools.rs:19-36` — Current system prompts (5 constants)
  - `src/chat/subagent.rs:280-293` — `run_translate()` which uses `crate::translate::build_translation_prompt()` — Translate has its own prompt building
  - `src/chat/subagent.rs:307-338` — `run_summarize()` which uses `build_system_prompt()` — Summarize already has good dynamic prompts

  **API/Type References**:
  - `src/prompts/builder.rs` — `build_system_prompt()` and `PromptType` enum
  - `src/chat/subagent.rs:312-314` — `PromptConfig::new(PromptType::Summarize).with_model_id(...)` — Summarize already uses dynamic prompts

  **Acceptance Criteria**:

  - [ ] `OCR_SYSTEM_PROMPT` describes direct OCR extraction (no tool references)
  - [ ] `VISION_SYSTEM_PROMPT` describes direct vision analysis (no tool references)
  - [ ] `TRANSLATE_SYSTEM_PROMPT` describes direct translation (no tool references)
  - [ ] `DOCUMENT_SYSTEM_PROMPT` describes document processing with `run_command` (only real tool)
  - [ ] No prompt references `ocr_image`, `describe_image`, `translate_text`, or `summarize_text`
  - [ ] `cargo test --all-features` passes
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: No phantom tool references in prompts
    Tool: Bash (grep)
    Preconditions: Changes compiled
    Steps:
      1. grep -r "ocr_image\|describe_image\|translate_text\|summarize_text" src/
    Expected Result: Zero matches (these function names don't exist in subagent context)
    Evidence: .sisyphus/evidence/task-4-no-phantom-tools.txt

  Scenario: Prompts describe direct execution
    Tool: Bash (grep)
    Preconditions: Changes compiled
    Steps:
      1. grep "SYSTEM_PROMPT" src/tools/subagent_tools.rs
      2. Verify each prompt describes its task without tool references
    Expected Result: 4 constant prompts (OCR, VISION, TRANSLATE, DOCUMENT) + 1 dynamic (SUMMARIZE)
    Evidence: .sisyphus/evidence/task-4-prompt-review.txt
  ```

  **Commit**: YES
  - Message: `fix(subagent): rewrite system prompts to describe direct task execution`
  - Files: `src/tools/subagent_tools.rs`
  - Pre-commit: `cargo test --lib subagent_tools`

---

- [x] 5. Improve spawn_subagent tool documentation and delegation guidance

  **What to do**:
  - In `src/tools/subagent_tools.rs`, enhance the docstring of `spawn_subagent` function:
    - Add delegation strategy guidance: "Use this tool when the main model cannot perform the task as well as a specialized model. Examples: OCR on images, translation between languages, vision analysis requiring a multimodal model, summarization of long text, document extraction from PDFs."
    - Add when-to-use vs when-not-to guidance
    - Add examples showing effective delegation prompts
  - In `src/prompts/tools.rs`, add a section for the subagent system explaining:
    - When to delegate to subagents vs handling directly
    - What each subagent type excels at
    - How to write effective delegation prompts

  **Must NOT do**:
  - Do NOT modify tool parameter signatures (that would break the LLM interface)
  - Do NOT add new parameters to `spawn_subagent`
  - Do NOT exceed reasonable prompt token budgets

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 4 and 6)
  - **Parallel Group**: Wave 3
  - **Blocks**: F1-F4
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `src/tools/subagent_tools.rs:38-76` — Current spawn_subagent docstring
  - `src/prompts/tools.rs` — Where tool descriptions are assembled for the system prompt

  **Acceptance Criteria**:

  - [ ] `spawn_subagent` docstring includes delegation strategy guidance
  - [ ] `spawn_subagent` docstring includes when-to-use descriptions per type
  - [ ] `src/prompts/tools.rs` has a subagent delegation guidance section
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Delegation guidance present in prompts
    Tool: Bash (grep)
    Preconditions: Changes compiled
    Steps:
      1. grep -c "delegat\|specialized\|subagent" src/prompts/tools.rs
    Expected Result: Count > 5 (meaningful delegation guidance present)
    Evidence: .sisyphus/evidence/task-5-delegation-guidance.txt
  ```

  **Commit**: YES
  - Message: `docs(subagent): improve spawn_subagent tool docs and delegation guidance`
  - Files: `src/tools/subagent_tools.rs`, `src/prompts/tools.rs`
  - Pre-commit: `cargo clippy --all-features -- -D warnings`

---

- [x] 6. Add user-facing subagent system documentation

  **What to do**:
  - Add a section to `doc/src/tools.md` describing the subagent system:
    - What subagents are and how they work
    - Available subagent types (OCR, Vision, Translate, Summarize, Document)
    - How to use chat commands: `/ocr`, `/vision`, `/translate`, `/summarize`
    - How the LLM delegates via `spawn_subagent`
    - Model configuration: `[model.ocr]`, `[model.vision]`, `[model.translate]`, `[model.summarize]`, `[model.document]`
    - Feature flag: `subagent-tools` (default enabled)
    - Chat commands are ALWAYS available (not feature-gated)
  - Update `doc/src/CHANGELOG.md` if there's an existing P4 entry that doesn't mention this fix

  **Must NOT do**:
  - Do NOT create a new standalone file — add to existing `doc/src/tools.md`
  - Do NOT modify `man/ask-ai.1` for this fix (separate concern)

  **Recommended Agent Profile**:
  - **Category**: `writing`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 4 and 5)
  - **Parallel Group**: Wave 3
  - **Blocks**: F1-F4
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `doc/src/tools.md` — Existing tools documentation (add section here)
  - `doc/src/commands/chat.md` — Chat commands documentation (may reference subagent commands)
  - `src/settings.rs:375-480` — Sample config showing `[model.*]` sections

  **Acceptance Criteria**:

  - [ ] `doc/src/tools.md` has a "Subagent System" section
  - [ ] Section explains all 5 subagent types
  - [ ] Section explains chat commands vs spawn_subagent distinction
  - [ ] Section explains model configuration via `[model.*]` sections
  - [ ] Section explains feature flag vs chat command availability
  - [ ] `mdbook build` succeeds (if doc build is available)

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Documentation exists and is complete
    Tool: Bash (grep)
    Preconditions: Changes written
    Steps:
      1. grep -c "Subagent\|spawn_subagent\|/ocr\|/vision\|/translate\|/summarize" doc/src/tools.md
    Expected Result: Count > 10 (meaningful documentation present)
    Evidence: .sisyphus/evidence/task-6-docs-exist.txt
  ```

  **Commit**: YES
  - Message: `docs(subagent): add subagent system documentation to tools.md`
  - Files: `doc/src/tools.md`
  - Pre-commit: `cd doc && mdbook build 2>/dev/null || true`

---

- [x] 7. Add regression tests for model resolution

  **What to do**:
  - Add tests in `src/settings.rs` (or a new test module) that verify:
    - `get_subcommand_config("translate")` returns `"translategemma:4b"` when no config override
    - `get_subcommand_config("translate")` returns custom model when `[model.translate] model = "custom"` IS set
    - `get_subcommand_config("ocr")` returns `"glm-ocr:bf16"` when no config override
    - `get_subcommand_config("ocr")` returns custom model when `[model.ocr] model = "custom"` IS set
    - `get_subcommand_config("vision")` returns default model when no config override
    - `get_subcommand_config("summarize")` returns default model when no config override
    - `get_subcommand_config("document")` returns default model when no config override
  - Add tests in `src/chat/subagent.rs` (or `src/tools/subagent_tools.rs`) that verify:
    - `build_translate_config()` creates `SubagentConfig` with model containing "translategemma"
    - `build_ocr_config()` creates `SubagentConfig` with model containing "glm-ocr"
    - `build_summarize_config()` creates `SubagentConfig` with model from default or custom config
    - `SubagentConfig` with `model_options` from ModelConfig preserves temperature (0.2 for translategemma, 0.1 for glm-ocr)

  **Must NOT do**:
  - Do NOT test with real Ollama server — use mock/dummy values
  - Do NOT create integration tests that require running services

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 4, 5, 6)
  - **Parallel Group**: Wave 3 (or after Task 2)
  - **Blocks**: F1-F4
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `src/settings.rs:530-640` — Existing Settings tests
  - `src/tools/subagent_tools.rs:223-297` — Existing subagent_tools tests
  - `src/chat/subagent.rs:534-623` — Existing SubagentRunner tests

  **Acceptance Criteria**:

  - [ ] Test `get_subcommand_config_translate_default` passes
  - [ ] Test `get_subcommand_config_translate_custom` passes
  - [ ] Test `get_subcommand_config_ocr_default` passes
  - [ ] Test `get_subcommand_config_ocr_custom` passes
  - [ ] Test `build_translate_config_uses_translategemma` passes
  - [ ] Test `build_ocr_config_uses_glm_ocr` passes
  - [ ] Test `subagent_config_model_options_from_model_config` passes
  - [ ] `cargo test --all-features` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: All regression tests pass
    Tool: Bash (cargo test)
    Preconditions: Changes compiled
    Steps:
      1. cargo test --lib settings subagent subagent_tools -- --nocapture
    Expected Result: All new tests pass, all existing tests pass
    Evidence: .sisyphus/evidence/task-7-regression-tests.txt
  ```

  **Commit**: YES
  - Message: `test(subagent): add regression tests for model resolution and ModelConfig integration`
  - Files: `src/settings.rs`, `src/tools/subagent_tools.rs`, `src/chat/subagent.rs`
  - Pre-commit: `cargo test --lib settings subagent subagent_tools`

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists. For each "Must NOT Have": search codebase for forbidden patterns. Check evidence files. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo clippy --all-features -- -D warnings` + `cargo test --all-features`. Review changed files for: unused imports, commented-out code, hardcoded model strings outside settings.rs, logic errors in model resolution.
  Output: `Build [PASS/FAIL] | Lint [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [x] F3. **Real Manual QA** — `unspecified-high`
  Start from clean state. Verify: translate model is translategemma (not qwen), OCR model is glm-ocr, temperature values match ModelConfig, subagent prompts don't reference phantom tools. Test both call paths: tool and slash commands.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: verify 1:1 match between spec and implementation. Check "Must NOT do" compliance. Detect scope creep (e.g., changes to CLI handlers, config.rs modifications).
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

- **Task 1**: `fix(settings): add translate model fallback in get_subcommand_config()` — src/settings.rs
- **Task 2**: `feat(subagent): integrate ModelConfig into SubagentConfig for per-model temperature and num_ctx` — src/chat/subagent.rs, src/tools/subagent_tools.rs, src/chat/command_handlers.rs
- **Task 4**: `fix(subagent): rewrite system prompts to describe direct task execution` — src/tools/subagent_tools.rs
- **Task 5**: `docs(subagent): improve spawn_subagent tool docs and delegation guidance` — src/tools/subagent_tools.rs, src/prompts/tools.rs
- **Task 6**: `docs(subagent): add subagent system documentation to tools.md` — doc/src/tools.md
- **Task 7**: `test(subagent): add regression tests for model resolution and ModelConfig integration` — src/settings.rs, src/tools/subagent_tools.rs, src/chat/subagent.rs

---

## Success Criteria

### Verification Commands
```bash
cargo test --all-features                # Expected: all pass
cargo clippy --all-features -- -D warnings  # Expected: 0 warnings
cargo build --features all-tools         # Expected: success
```

### Final Checklist
- [x] `get_subcommand_config("translate")` returns "translategemma:4b" as fallback
- [x] All subagent paths use ModelConfig (temperature, num_ctx from models.toml)
- [x] No phantom tool references in subagent prompts
- [x] spawn_subagent tool has delegation guidance
- [x] Documentation describes subagent system with config examples
- [x] No regressions in existing tests