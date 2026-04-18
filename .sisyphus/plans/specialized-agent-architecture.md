# P4: Specialized Agent Architecture

## TL;DR

> **Quick Summary**: Implement a SubagentRunner that spawns one-shot LLM sessions for specialized tasks (OCR, vision, translate, summarize, document extraction), replacing direct Command::new() calls and enabling /ocr, /vision, /translate, /summarize commands in chat mode.
>
> **Deliverables**:
> - `SubagentRunner` - lightweight one-shot executor (NOT reuse of CustomCoordinator)
> - `spawn_subagent` LLM tool for autonomous subagent invocation
> - `/ocr`, `/vision`, `/translate`, `/summarize` chat commands
> - Document extraction refactor: `extract_text_with_skill()` → subagent call
> - Config: `[model.ocr]` and `[model.document]` in config.toml
> - Feature flag: `subagent-tools` (default enabled)
>
> **Estimated Effort**: Large (7-10 days)
> **Parallel Execution**: YES - 4 waves
> **Critical Path**: Task 1 (SubagentRunner) → Tasks 2-4 (types) → Task 5 (LLM tool) → Task 6 (chat commands) → Task 7 (document refactor) → Task 8 (config + polish) → Final

---

## Context

### Original Request
Implement PRIORITY 4: Specialized Agent Architecture — delegate specialized tasks (OCR, vision, translate, summarize, document extraction) to one-shot agents with optimized models.

### Interview Summary
**Key Discussions**:
- Scope: ALL 5 subagent types in one PR (ocr, vision, translate, summarize, document)
- Chat commands: YES — add /ocr, /vision, /translate, /summarize
- Config: Reuse existing [model.vision], [model.translate], [model.summarize], add [model.ocr] and [model.document]
- Test strategy: Tests-after (external dependencies make TDD impractical)
- Feature flag: Single `subagent-tools` feature flag, default enabled

**Research Findings**:
- OCR/Vision use `/api/generate` (different endpoint from chat)
- Translate/Summarize use `/api/chat` via CustomCoordinator WITHOUT tools
- Document import uses direct `Command::new()` (FIXME documented, P4 is the fix)
- CustomCoordinator is 992 lines — subagents need lightweight version
- Existing processors (OcrProcessor, VisionProcessor, etc.) should be DELEGATED to, not reimplemented

### Metis Review
**Identified Gaps** (addressed):
- Image handling: file_path parameter, tool reads + base64 encodes internally
- Subagent output display: Both — shown to user AND available to LLM
- spawn_subagent for OCR/Vision: supported via file_path parameter
- Document subagent: replaces only `extract_text_with_skill()`, NOT the whole import_document tool
- Result truncation: Max ~10K chars to prevent context overflow
- Recursion prevention: spawn_subagent NOT in subagent tool whitelists
- Two API paths: `/api/generate` for OCR/Vision, `/api/chat` for Translate/Summarize

---

## Work Objectives

### Core Objective
Create a SubagentRunner that executes specialized one-shot LLM tasks (OCR, vision, translate, summarize, document) and integrates them into chat mode as both LLM tools and user commands, while fixing the document extraction technical debt.

### Concrete Deliverables
- `src/chat/subagent.rs` — SubagentRunner, SubagentType enum
- `src/tools/subagent_tools.rs` — spawn_subagent LLM tool
- `/ocr`, `/vision`, `/translate`, `/summarize` chat commands
- Refactored document extraction (replaces Command::new with subagent)
- Config extensions ([model.ocr], [model.document])
- Feature flag `subagent-tools`

### Definition of Done
- [ ] `cargo build --features all-tools` succeeds
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `cargo test --all-features` passes
- [ ] `/ocr <path>` works in chat mode
- [ ] `/vision <path> <prompt>` works in chat mode
- [ ] `/translate <lang> <text>` works in chat mode
- [ ] `/summarize <text>` works in chat mode
- [ ] `spawn_subagent` tool works for LLM-initiated calls
- [ ] `import_document` uses subagent for PDF/EPUB extraction
- [ ] CLI subcommands (`ask-ai ocr`, `ask-ai translate`, etc.) unchanged

### Must Have
- SubagentRunner with SubagentType dispatch
- Two API paths: /api/generate (OCR/Vision) vs /api/chat (Translate/Summarize)
- spawn_subagent LLM tool registered in coordinator
- /ocr, /vision, /translate, /summarize chat commands
- Document extraction uses subagent (fixes FIXME in documents.rs:384-396)
- Result truncation (~10K chars max) for context safety
- Recursion prevention (spawn_subagent not in subagent tool list)
- Config support for [model.ocr] and [model.document]
- Feature flag `subagent-tools`

### Must NOT Have (Guardrails)
- DO NOT reuse CustomCoordinator for subagents (992 lines of multi-turn machinery not needed)
- DO NOT modify CLI subcommand code paths (OcrProcessor, VisionProcessor, etc. stay unchanged)
- DO NOT give subagents database access, history, or thinking output
- DO NOT allow subagents to call spawn_subagent (recursion prevention)
- DO NOT add streaming subagent output to the terminal
- DO NOT add multi-subagent orchestration, caching, or persistence
- DO NOT add /document chat command (document import is /doc import)
- DO NOT add subagent timeout configuration (use sensible defaults)
- DO NOT restructure existing [model.vision] or [model.translate] sections
- AI slop: no over-abstraction, no excessive comments, no generic names

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** - ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: YES (Tests-after)
- **Framework**: Rust built-in test framework (`cargo test`)
- **Phase**: Write unit tests after implementation, QA scenarios for integration

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Rust module tests**: `cargo test --all-features`
- **Build verification**: `cargo build --features all-tools`
- **Lint check**: `cargo clippy --all-features -- -D warnings`
- **CLI regression**: `cargo run -- ocr`, `cargo run -- translate`, etc.

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation - MUST complete first):
├── Task 1: SubagentRunner core module [deep]
└── Task 2: SubagentType enum + config integration [quick]

Wave 2 (Subagent types + LLM tool - MAX PARALLEL):
├── Task 3: OCR subagent implementation [unspecified-high]
├── Task 4: Vision subagent implementation [unspecified-high]
├── Task 5: Translate subagent implementation [unspecified-high]
├── Task 6: Summarize subagent implementation [unspecified-high]
├── Task 7: Document subagent implementation [deep]
└── Task 8: spawn_subagent LLM tool [unspecified-high]

Wave 3 (Chat commands + integration - after Wave 2):
├── Task 9: Chat commands (/ocr, /vision, /translate, /summarize) [unspecified-high]
├── Task 10: Document extraction refactor [deep]
├── Task 11: Config + feature flag + prompts [quick]
└── Task 12: Prompt integration (tools section) [quick]

Wave 4 (Testing + polish - after Wave 3):
├── Task 13: Unit tests [unspecified-high]
└── Task 14: Documentation + CHANGELOG [writing]

Wave FINAL (After ALL tasks — 4 parallel reviews):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Real manual QA (unspecified-high)
└── Task F4: Scope fidelity check (deep)
→ Present results → Get explicit user okay

Critical Path: Task 1 → Tasks 3-8 → Task 9 → Task 10 → Task 13 → F1-F4
Parallel Speedup: ~60% faster than sequential
Max Concurrent: 6 (Wave 2)
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 | - | 3-8 |
| 2 | - | 3-8 |
| 3 | 1, 2 | 9 |
| 4 | 1, 2 | 9 |
| 5 | 1, 2 | 9 |
| 6 | 1, 2 | 9 |
| 7 | 1, 2 | 10 |
| 8 | 1, 2 | 9, 11, 12 |
| 9 | 3-6, 8 | - |
| 10 | 7 | - |
| 11 | 2, 8 | - |
| 12 | 8 | - |
| 13 | 9-12 | F1-F4 |
| 14 | 13 | - |

### Agent Dispatch Summary

- **Wave 1**: T1 → `deep`, T2 → `quick`
- **Wave 2**: T3-T6 → `unspecified-high`, T7 → `deep`, T8 → `unspecified-high`
- **Wave 3**: T9 → `unspecified-high`, T10 → `deep`, T11-T12 → `quick`
- **Wave 4**: T13 → `unspecified-high`, T14 → `writing`
- **FINAL**: F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [x] 1. SubagentRunner Core Module

  **What to do**:
  - Create `src/chat/subagent.rs` with `SubagentRunner` struct
  - Define `SubagentType` enum: `Ocr`, `Vision`, `Translate`, `Summarize`, `Document`
  - Implement `run()` method that dispatches based on SubagentType
  - Handle two API paths: `/api/generate` for Ocr/Vision, `/api/chat` for Translate/Summarize
  - For Document type: use existing `extract_text_with_skill()` initially (refactored in Task 10)
  - Add `SubagentConfig` struct: model name, system prompt, tool whitelist, max output chars
  - Result truncation: max 10,000 chars via `truncate_to_budget()` with `[TRUNCATED]` notice
  - Error handling: return `Ok(error_message_string)` per AGENTS.md tool rules
  - No database, no history, no thinking output, no ephemeral messages

  **Must NOT do**:
  - Do NOT reuse CustomCoordinator for subagent execution
  - Do NOT add context overflow detection to subagents
  - Do NOT add continuation tag support to subagents
  - Do NOT add event callbacks to subagents

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Core architectural module with two API path dispatch
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 2)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 3-8
  - **Blocked By**: None

  **References**:
  **Pattern References**:
  - `src/chat/custom_coordinator.rs:263-293` — Struct design pattern (fields to AVOID: history, ephemeral_messages, event_callback, pre_tool_content)
  - `src/chat/custom_coordinator.rs:535-539` — add_tool() registration pattern (for Document subagent whitelist)
  - `src/ocr/processor.rs:52-69` — Direct `/api/generate` usage with images (Ocr model: glm-ocr:bf16)
  - `src/vision/processor.rs:71-77` — Direct `/api/generate` usage with images
  - `src/summarize/processor.rs:44-46` — `/api/chat` via CustomCoordinator WITHOUT tools
  - `src/main.rs:179-290` — Translate flow: `CustomCoordinator::new(ollama, model, vec![]).chat(...)`

  **API/Type References**:
  - `src/utils.rs:truncate_to_budget()` — Existing truncation function for context safety
  - `ollama_rs::generation::completion::GenerationRequest` — `/api/generate` request type
  - `ollama_rs::generation::chat::ChatMessageRequest` — `/api/chat` request type

  **WHY Each Reference Matters**:
  - custom_coordinator.rs:263-293 — Shows what NOT to include; subagents are simpler
  - ocr/processor.rs:52-69 — Direct API pattern for image-based subagents (OCR/Vision)
  - summarize/processor.rs:44-46 — Chat API pattern for text-based subagents
  - truncate_to_budget — Reuse existing truncation, don't reinvent

  **Acceptance Criteria**:
  - [ ] `src/chat/subagent.rs` created with `SubagentRunner` struct
  - [ ] `SubagentType` enum with 5 variants
  - [ ] `run()` dispatches to `/api/generate` for Ocr/Vision
  - [ ] `run()` dispatches to `/api/chat` for Translate/Summarize
  - [ ] Results truncated at ~10,000 chars
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios**:
  ```
  Scenario: SubagentRunner dispatches to correct API endpoint
    Tool: Bash (cargo test)
    Preconditions: SubagentRunner implemented
    Steps:
      1. cargo test --all-features subagent
      2. Assert tests pass
    Expected Result: All subagent unit tests pass
    Failure Indicators: Compilation errors, test failures
    Evidence: .sisyphus/evidence/task-1-subagent-tests.txt
  ```

  **Commit**: YES (groups with Task 2)
  - Message: `feat(subagent): add SubagentRunner core module and SubagentType enum`
  - Files: `src/chat/subagent.rs`, `src/chat/mod.rs`
  - Pre-commit: `cargo clippy --all-features -- -D warnings`

- [x] 2. SubagentType Config Integration

  **What to do**:
  - Add `ocr: SubcommandModelConfig` and `document: SubcommandModelConfig` to `ModelSettings` in `src/settings.rs`
  - Update `get_subcommand_config()` to support `"ocr"` and `"document"` subcommand names
  - Add `SubcommandModelConfig::default()` for OCR (model: `glm-ocr:bf16`, thinking: false, tools: false)
  - Add `SubcommandModelConfig::default()` for Document (model: same as default, thinking: false, tools: true)
  - Update Settings deserialization tests to cover new fields

  **Must NOT do**:
  - Do NOT restructure existing `[model.vision]` or `[model.translate]` sections
  - Do NOT change existing SubcommandModelConfig struct
  - Do NOT add [subagents] section (we're reusing existing model config)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Additive config changes, established patterns
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 1)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks 3-8
  - **Blocked By**: None

  **References**:
  **Pattern References**:
  - `src/settings.rs:50-79` — `ModelSettings` struct (add ocr, document fields here)
  - `src/settings.rs:290-341` — `get_subcommand_config()` (add "ocr", "document" match arms)
  - `src/settings.rs:81-100` — SubcommandModelConfig default patterns

  **Acceptance Criteria**:
  - [ ] `ModelSettings` has `ocr` and `document` fields
  - [ ] `get_subcommand_config("ocr")` returns `("glm-ocr:bf16", false, false)`
  - [ ] `get_subcommand_config("document")` returns `(default_model, false, true)`
  - [ ] Existing config tests still pass
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios**:
  ```
  Scenario: Config reads [model.ocr] and [model.document] from config.toml
    Tool: Bash (cargo test)
    Preconditions: New config fields added
    Steps:
      1. cargo test --all-features settings
      2. Assert all settings tests pass
    Expected Result: Settings tests pass, new defaults correct
    Failure Indicators: Deserialization errors for new fields
    Evidence: .sisyphus/evidence/task-2-config-tests.txt
  ```

  **Commit**: YES (groups with Task 1)
  - Message: `feat(subagent): add SubagentRunner core module and SubagentType enum`
  - Files: `src/settings.rs`
  - Pre-commit: `cargo test --all-features`

- [x] 3. OCR Subagent Implementation

  **What to do**:
  - Implement `SubagentRunner::run_ocr()` method in `src/chat/subagent.rs`
  - Delegates to existing `OcrProcessor::process_file()` (DO NOT reimplement OCR logic)
  - Model from config: `settings.get_subcommand_config("ocr")` → defaults to `glm-ocr:bf16`
  - Returns extracted text, truncated to max output chars (~10K)
  - Error: file not found, model unavailable, Ollama unreachable → return Ok(error_string)

  **Must NOT do**: Do NOT modify OcrProcessor. Do NOT add thinking/output to subagent.

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 2, parallel with Tasks 4-8. Blocks: Task 9. Blocked by: 1, 2.

  **References**:
  - `src/ocr/processor.rs:52-69` — process_file() to delegate to
  - `src/ocr/mod.rs` — Module exports

  **Acceptance Criteria**:
  - [ ] `run_ocr(path)` returns OCR text
  - [ ] File-not-found returns `Ok("Error: Image file not found: ...")`
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios**:
  ```
  Scenario: OCR subagent handles missing file
    Tool: Bash (cargo test)
    Steps: 1. Call run_ocr with nonexistent path
    Expected Result: Ok("Error: Image file not found: ...")
    Evidence: .sisyphus/evidence/task-3-ocr-error.txt
  ```

  **Commit**: YES (Wave 2 group commit)

- [x] 4. Vision Subagent Implementation

  **What to do**:
  - Implement `SubagentRunner::run_vision()` method
  - Delegates to `VisionProcessor::process()`
  - Supports multiple images: `run_vision(paths: &[PathBuf], prompt: &str)`
  - Model from config: `settings.get_subcommand_config("vision")`

  **Must NOT do**: Do NOT modify VisionProcessor.

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 2. Blocks: Task 9. Blocked by: 1, 2.

  **References**: `src/vision/processor.rs:71-77`

  **Acceptance Criteria**:
  - [ ] `run_vision(paths, prompt)` returns description
  - [ ] Multiple images supported
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES (Wave 2 group commit)

- [x] 5. Translate Subagent Implementation

  **What to do**:
  - Implement `SubagentRunner::run_translate()` method
  - Uses `/api/chat` (not /api/generate like OCR/Vision)
  - Builds translation prompt via `build_translation_prompt()`
  - Model from config: `settings.get_subcommand_config("translate")`
  - NO tools, NO thinking

  **Must NOT do**: Do NOT add tools to translate. Do NOT add thinking.

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 2. Blocks: Task 9. Blocked by: 1, 2.

  **References**:
  - `src/translate/prompt.rs` — build_translation_prompt()
  - `src/main.rs:179-290` — Translate CLI flow

  **Acceptance Criteria**:
  - [ ] `run_translate(lang_pair, text)` returns translation
  - [ ] No tools registered
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES (Wave 2 group commit)

- [x] 6. Summarize Subagent Implementation

  **What to do**:
  - Implement `SubagentRunner::run_summarize()` method
  - Uses `/api/chat`, builds summarize prompt
  - Model from config: `settings.get_subcommand_config("summarize")`
  - NO tools, NO thinking

  **Must NOT do**: Do NOT add tools. Do NOT add thinking.

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 2. Blocks: Task 9. Blocked by: 1, 2.

  **References**: `src/summarize/processor.rs:44-46`

  **Acceptance Criteria**:
  - [ ] `run_summarize(text)` returns summary
  - [ ] No tools registered
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES (Wave 2 group commit)

- [x] 7. Document Subagent Implementation

  **What to do**:
  - Implement `SubagentRunner::run_document()` method
  - MOST COMPLEX: needs tools (run_command w/whitelist + sandbox)
  - Creates minimal CustomCoordinator with ONLY `run_command` registered
  - Loads `document-processing` skill via `skills/loader.rs::get_skill_content()`
  - Passes skill instructions as system prompt
  - Model from config: `settings.get_subcommand_config("document")`
  - Returns extracted text

  **Must NOT do**:
  - Do NOT register `spawn_subagent` in tool list (recursion)
  - Do NOT use `Command::new()` directly (that's the debt we're fixing)
  - Do NOT give database access

  **Recommended Agent Profile**: Category: `deep`, Skills: []

  **Parallelization**: Wave 2. Blocks: Task 10. Blocked by: 1, 2.

  **References**:
  - `src/tools/documents.rs:384-396` — FIXME comment (the debt)
  - `src/tools/documents.rs:401-448` — Current `extract_text_with_skill()` using Command::new()
  - `src/skills/loader.rs:66-95` — get_skill_content() for overrides
  - `src/skills/builtin/document-processing.md` — Built-in skill

  **Acceptance Criteria**:
  - [ ] `run_document(path)` extracts text using run_command
  - [ ] Project-level skill overrides respected
  - [ ] spawn_subagent NOT in tool whitelist
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios**:
  ```
  Scenario: Document subagent handles missing pdftotext
    Tool: Bash (cargo test)
    Steps: 1. Call run_document when pdftotext not found
    Expected Result: Ok("Error: Could not run 'pdftotext': ...")
    Evidence: .sisyphus/evidence/task-7-document-error.txt
  ```

  **Commit**: YES (Wave 2 group commit)

- [x] 8. spawn_subagent LLM Tool

  **What to do**:
  - Create `src/tools/subagent_tools.rs`
  - Function: `spawn_subagent(subagent_type: String, prompt: String, file_path: Option<String>)`
  - ALL params String/Option<String> per AGENTS.md
  - Validates type: must be one of ocr/vision/translate/summarize/document
  - For OCR/Vision/Document: file_path REQUIRED (error if missing)
  - For Translate/Summarize: file_path ignored
  - Returns Ok(String) with result or error message
  - Full docstrings per AGENTS.md tool documentation guidelines
  - Register in `src/tools/registry.rs` under `subagent-tools` feature

  **Must NOT do**:
  - Do NOT use numeric types for params
  - Do NOT crash on invalid type (return error string)

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 2. Blocks: 9, 11, 12. Blocked by: 1, 2.

  **References**:
  - `src/tools/remember.rs` — Pattern for tool with file path handling
  - `src/tools/documents.rs:1-50` — Document tool file path validation
  - `src/tools/registry.rs` — Registration pattern

  **Acceptance Criteria**:
  - [ ] `src/tools/subagent_tools.rs` created
  - [ ] Invalid type returns Ok("Error: Unknown subagent type '...'. Valid types: ocr, vision, translate, summarize, document")`
  - [ ] Missing file_path for OCR returns error
  - [ ] Docstrings complete
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES (Wave 2 group commit)
  - Files: `src/tools/subagent_tools.rs`, `src/tools/mod.rs`, `src/tools/registry.rs`

- [x] 9. Chat Commands (/ocr, /vision, /translate, /summarize)

  **What to do**:
  - Add ChatCommand variants in `src/chat/commands.rs`:
    - `Ocr { path: String }`
    - `Vision { paths: Vec<String>, prompt: String }`
    - `Translate { lang_pair: String, text: String }`
    - `Summarize { text: String }`
  - Add parse_command match arms for `/ocr`, `/vision`, `/translate`, `/summarize`
  - Add handlers in `src/chat/command_handlers.rs`:
    - `handle_ocr_subagent(state: &mut ReplState, path: String)`
    - `handle_vision_subagent(state: &mut ReplState, paths: Vec<String>, prompt: String)`
    - `handle_translate_subagent(state: &mut ReplState, lang_pair: String, text: String)`
    - `handle_summarize_subagent(state: &mut ReplState, text: String)`
  - Each handler calls SubagentRunner internally
  - Output: displayed to user AND available to LLM for follow-up
  - Add shortcuts: `/oc`, `/vi`, `/tr`, `/su` (if not colliding)
  - Wire in `src/chat/repl.rs` dispatch

  **Must NOT do**:
  - Do NOT add /document command (that's /doc import)
  - Do NOT break existing command parsing

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 3. Blocked by: 3-6, 8.

  **References**:
  - `src/chat/commands.rs` — parse_command() pattern for adding new variants
  - `src/chat/command_handlers.rs` — handle_* functions pattern
  - `src/chat/repl.rs` — Command dispatch in REPL loop
  - `src/translate/cli.rs` — Language pair parsing (/translate en:pt)

  **Acceptance Criteria**:
  - [ ] `/ocr /tmp/image.png` works in chat
  - [ ] `/vision /tmp/img1.png /tmp/img2.png Describe both` works
  - [ ] `/translate en:pt Hello world` works
  - [ ] `/summarize Long text here` works
  - [ ] Subagent output shown to user AND available to LLM
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **QA Scenarios**:
  ```
  Scenario: /ocr chat command extracts text
    Tool: interactive_bash (tmux)
    Preconditions: ask-ai chat running, test image available
    Steps:
      1. Type /ocr /tmp/test-image.png
      2. Wait for subagent result
    Expected Result: OCR text displayed, LLM can reference it
    Evidence: .sisyphus/evidence/task-9-ocr-chat.txt

  Scenario: /translate chat command
    Tool: interactive_bash (tmux)
    Steps:
      1. Type /translate en:pt Hello world
      2. Wait for result
    Expected Result: "Olá mundo" or similar Portuguese translation
    Evidence: .sisyphus/evidence/task-9-translate-chat.txt
  ```

  **Commit**: YES
  - Message: `feat(subagent): add /ocr /vision /translate /summarize chat commands`
  - Files: `src/chat/commands.rs`, `src/chat/command_handlers.rs`, `src/chat/repl.rs`

- [x] 10. Document Extraction Refactor

  **What to do**:
  - Replace `extract_text_with_skill()` in `src/tools/documents.rs:401-448` with `SubagentRunner::run_document()` call
  - Remove FIXME comment (lines 384-396)
  - The `import_document` tool REMAINS — only the extraction step changes
  - Chunking, embedding, DB insertion stay in `import_document`
  - Update `log_tool_result` messages to reflect subagent usage

  **Must NOT do**:
  - Do NOT remove the `import_document` tool function
  - Do NOT change chunking/embedding/DB insertion logic
  - Do NOT remove the `skills-tools` feature gate (it now gates the skill-based extraction)

  **Recommended Agent Profile**: Category: `deep`, Skills: []

  **Parallelization**: Wave 3. Blocked by: 7.

  **References**:
  - `src/tools/documents.rs:384-448` — The exact code to change
  - `src/tools/documents.rs:1-50` — import_document() structure (keep unchanged)

  **Acceptance Criteria**:
  - [ ] `import_document` still works for PDF/EPUB/TXT/MD/ORG
  - [ ] No more `Command::new("pdftotext")` in documents.rs
  - [ ] FIXME comment removed
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES
  - Message: `refactor(documents): replace Command::new with subagent extraction`
  - Files: `src/tools/documents.rs`

- [x] 11. Config + Feature Flag + Cargo.toml

  **What to do**:
  - Add `subagent-tools` feature flag to `Cargo.toml` (default enabled, in `all-tools`)
  - Feature gate `src/tools/subagent_tools.rs` with `#[cfg(feature = "subagent-tools")]`
  - Feature gate subagent chat commands with `#[cfg(feature = "subagent-tools")]`
  - Add `[model.ocr]` and `[model.document]` to doc/src/ configuration docs
  - Update user documentation

  **Must NOT do**: Do NOT restructure existing features.

  **Recommended Agent Profile**: Category: `quick`, Skills: []

  **Parallelization**: Wave 3. Blocked by: 2, 8.

  **Acceptance Criteria**:
  - [ ] `cargo build --features all-tools` succeeds
  - [ ] `cargo build` (default features) includes subagent-tools
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES (Wave 3 group commit)

- [x] 12. Prompt Integration (Tools Section)

  **What to do**:
  - Add `spawn_subagent` tool description to `src/prompts/tools.rs`
  - Add tool description: type options, when to use each type, file_path requirement
  - Feature gate with `#[cfg(feature = "subagent-tools")]`
  - Add `[model.ocr]` and `[model.document]` to user docs

  **Must NOT do**: Do NOT add spawn_subagent to subagent prompt sections.

  **Recommended Agent Profile**: Category: `quick`, Skills: []

  **Parallelization**: Wave 3. Blocked by: 8.

  **References**: `src/prompts/tools.rs` — Existing tool section format

  **Acceptance Criteria**:
  - [ ] Tool appears in system prompt when subagent-tools enabled
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES (Wave 3 group commit)

- [x] 13. Unit Tests

  **What to do**:
  - Add unit tests for `SubagentRunner`:
    - `test_subagent_type_from_str()` — valid types recognized, invalid types error
    - `test_subagent_config_defaults()` — ocr defaults, document defaults
    - `test_result_truncation()` — output > 10K chars truncated
  - Add unit tests for `spawn_subagent` tool:
    - `test_invalid_subagent_type()` — unknown type returns error string
    - `test_missing_file_path_for_ocr()` — returns error string
  - Tests in `src/chat/subagent.rs` and `src/tools/subagent_tools.rs`
  - Note: Integration tests requiring Ollama are QA scenarios, not unit tests

  **Must NOT do**: Do NOT require live Ollama for unit tests.

  **Recommended Agent Profile**: Category: `unspecified-high`, Skills: []

  **Parallelization**: Wave 4. Blocked by: 9-12.

  **Acceptance Criteria**:
  - [ ] `cargo test --all-features` passes
  - [ ] Unit tests do NOT require Ollama
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES
  - Message: `test(subagent): add unit tests for SubagentRunner and spawn_subagent`

- [x] 14. Documentation + CHANGELOG

  **What to do**:
  - Update `doc/src/CHANGELOG.md` with P4 feature entries
  - Update `IMPLEMENTATION.md`: Mark P4 as ✅ COMPLETED, add implementation summary
  - Update `doc/src/tools.md` with subagent tool documentation
  - Update `doc/src/commands/chat.md` with /ocr, /vision, /translate, /summarize commands
  - Update `doc/src/development/roadmap.md`: Mark P4 as completed

  **Must NOT do**: Do NOT create new doc files (add to existing).

  **Recommended Agent Profile**: Category: `writing`, Skills: []

  **Parallelization**: Wave 4. Blocked by: 13.

  **Acceptance Criteria**:
  - [ ] CHANGELOG updated
  - [ ] IMPLEMENTATION.md updated
  - [ ] `cargo clippy --all-features -- -D warnings` passes

  **Commit**: YES
  - Message: `docs: update CHANGELOG and IMPLEMENTATION.md for P4 Specialized Agent Architecture`


---

## Final Verification Wave

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, run command). For each "Must NOT Have": search codebase for forbidden patterns. Check evidence files. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo clippy --all-features -- -D warnings` + `cargo test --all-features`. Review all changed files for: `as any`, empty catches, console.log equivalents, unused imports, dead code. Check AI slop.
  Output: `Build [PASS/FAIL] | Lint [PASS/FAIL] | Tests [N pass/N fail] | VERDICT`

- [x] F3. **Real Manual QA** — `unspecified-high`
  Start from clean state. Execute EVERY QA scenario from EVERY task. Test cross-task integration. Test edge cases: missing model, invalid image, large PDF, empty input. Save to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff. Verify 1:1 — everything in spec was built, nothing beyond spec. Check "Must NOT do" compliance. Flag unaccounted changes.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## PR Process Integration

> Maps PR-PROCESS.md phases to this plan's tasks.
> **Sisyphus executor STOPS at Phase 6.1**. Hermes takes over from there.

### Phase Mapping

| PR-PROCESS Phase | Plan Task(s) | Executor | Notes |
|-----------------|-------------|----------|-------|
| **Phase 1: Setup** (create branch, move card) | Pre-task | Sisyphus | `git checkout -b feat/subagent-architecture`
| **Phase 2: Docs FIRST** (CHANGELOG + IMPLEMENTATION.md) | Task 14 (partial) | Sisyphus | Commit docs BEFORE code per PR-PROCESS
| **Phase 2 STOP** (create Draft PR, wait for auth) | — | User | `gh pr create --draft` → STOP
| **Phase 2.5: Planning** | ✅ ALREADY DONE | Prometheus | This plan IS the planning output
| **Phase 2.6: Requirements Checkpoint** | ✅ ALREADY DONE | Prometheus | Metis gap analysis + Momus OKAY = requirements cleared
| **Phase 3: Implementation** | Tasks 1-13 | Sisyphus | After user authorizes continuation
| **Phase 4: Mark PR Ready** | Post-Task-13 | Sisyphus | `gh pr ready`, move card to In Review
| **Phase 5: Review & Iteration** | — | User | User reviews PR, adds comments, agent iterates
| **Phase 6: Review Approval** | — | User | User approves after all comments resolved
| **Phase 6.1: Create Manual Test Script** | Final Wave (partial) | Sisyphus | Create `~/MANUAL-TEST-PR_NUMBER.md` then **STOP**
| **Phase 6.2: Manual Tests** | — | Hermes | Hermes executes the manual test script
| **Phase 6.3: Smoke Test Update** | — | Hermes/Sisyphus | Review `SMOKE_TEST.md`, add sections if needed
| **Phase 6.4: Smoke Test** | — | Hermes | Optional, user-requested
| **Phase 7: Merge** | — | User | `gh pr merge N --merge --delete-branch`

### Execution Responsibility Split

```
┌─────────────────────────────────────────────────────────────┐
│              SISYPHUS EXECUTOR SCOPE                          │
│  (Stops after Phase 6.1)                                     │
│                                                              │
│  1. Create branch: feat/subagent-architecture                │
│  2. Move GitHub card to "In Progress"                         │
│  3. Update CHANGELOG + IMPLEMENTATION.md (Phase 2 docs)      │
│  4. Commit docs → push                                       │
│  5. STOP → report to user for Draft PR creation              │
│  6. [After user authorizes] Implement Tasks 1-13            │
│  7. Mark PR ready for review                                 │
│  8. Iterate on review comments                                │
│  9. After approval: create ~/MANUAL-TEST-PR_NUMBER.md        │
│  10. STOP HERE — hand off to Hermes Agent                  │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│              HERMES AGENT SCOPE                              │
│  (Phase 6.2 onwards)                                         │
│                                                              │
│  1. Execute manual test script                              │
│  2. Report results in PR comments                            │
│  3. If bugs: report → Sisyphus fixes → re-review            │
│  4. Review/update SMOKE_TEST.md if needed                   │
│  5. Execute smoke test (if requested)                        │
│  6. Report "Aprovado para merge" or failure                  │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│              USER SCOPE                                      │
│                                                              │
│  1. Authorize Draft PR creation (after Phase 2 docs)        │
│  2. Authorize Phase 3 implementation                         │
│  3. Review PR, add comments (Phase 5)                       │
│  4. Approve PR (Phase 6)                                    │
│  5. Authorize merge (Phase 7)                               │
│  6. Approve manual test script (Phase 6.1)                  │
└─────────────────────────────────────────────────────────────┘
```

### Key Constraints for Sisyphus Executor

1. **NEVER close Issue #12** — auto-closes on PR merge via `Closes #12`
2. **NEVER move card to Done** — only reviewer does this after approval
3. **NEVER merge without approval** — user explicitly authorizes
4. **ALWAYS create PR as DRAFT first** — implement, then mark ready
5. **ALWAYS commit docs before code** — CHANGELOG + IMPLEMENTATION.md first
6. **STOP at Phase 6.1** — create test script, then hand off to Hermes
7. **Project ID**: `PVT_kwHOADplIc4BRnZ9`, Issue: #12
8. **Branch name**: `feat/subagent-architecture`
---

## Commit Strategy

- **Wave 1**: `feat(subagent): add SubagentRunner core module and SubagentType enum`
- **Wave 2**: `feat(subagent): implement OCR/Vision/Translate/Summarize/Document subagent types`
- **Wave 3**: `feat(subagent): add /ocr /vision /translate /summarize chat commands`
- **Wave 3**: `refactor(documents): replace Command::new with subagent extraction`
- **Wave 3**: `feat(subagent): add config, feature flag, and prompt integration`
- **Wave 4**: `test(subagent): add unit tests for SubagentRunner and spawn_subagent`
- **Wave 4**: `docs: update CHANGELOG and IMPLEMENTATION.md for P4 Specialized Agent Architecture`

---

## Success Criteria

### Verification Commands
```bash
cargo build --features all-tools                          # Expected: success
cargo clippy --all-features -- -D warnings                 # Expected: no warnings
cargo test --all-features                                 # Expected: all pass
cargo run -- ocr test-image.png                           # Expected: CLI still works
cargo run -- translate en:pt "Hello"                      # Expected: CLI still works
```

### Final Checklist
- [ ] All "Must Have" present
- [ ] All "Must NOT Have" absent
- [ ] All tests pass
- [ ] CLI subcommands unchanged
- [ ] Chat commands (/ocr, /vision, /translate, /summarize) work
- [ ] spawn_subagent tool works for LLM-initiated calls
- [ ] Document extraction uses subagent (no more Command::new)