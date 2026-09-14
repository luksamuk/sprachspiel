# Agent Spawning Tools & /skill

> **Status:** documentação técnica viva, extraída de `IMPLEMENTATION.md` durante a
> consolidação do backlog (LUC-140 / `refactor/backlog-consolidation`). O conteúdo é
> **verbatim** da fonte; apenas os cabeçalhos foram normalizados (eram marcados como
> "PRIORITY N … COMPLETED", resíduo de tracker).

Ferramentas de spawn de subagentes e o comando `/skill`.

## SF5: Agent Spawning Tools [COMPLETED]

**Status:** ✅ COMPLETED
**Priority:** P2 (High)
**Issue:** #111

**Goal:** Replace generic `spawn_subagent` tool with dedicated spawning tools for each agent type, removing hardcoded PDF pipeline in favor of LLM-orchestrated document processing via skills.

**Implementation:**
1. **4 dedicated spawning tools** — `spawn_ocr_agent` (text extraction from images), `spawn_vision_agent` (image analysis), `spawn_translate_agent` (translation), `spawn_summarize_agent` (summarization). Each has only its relevant parameters, eliminating irrelevant optional parameters.
2. **Removed `spawn_document_agent`** — Redundant: the LLM already has `run_command` + spawning tools and follows the `document-processing` skill. The document subagent was limited (only `run_command`, no OCR/vision) and created unnecessary indirection.
3. **Removed PDF pipeline from Rust** — No hardcoded `pdftoppm`/checkpoint/etc. in the harness. The LLM orchestrates PDF processing via `run_command("pdftotext")` → `run_command("pdftoppm")` → `spawn_ocr_agent`/`spawn_vision_agent` following the `document-processing` skill.
4. **Removed `--pages` flag** — Not the harness's responsibility. When Ollama models support PDF natively, it can be added back.
5. **Removed `FileType::Pdf`/`FileType::Epub`** — `import_document` only accepts TXT, MD, ORG. For PDFs/EPUBs, the LLM extracts text via `run_command("pdftotext")` first, then imports the resulting text file.
6. **Removed `SubagentType::Document`** — No longer needed. `SubagentRunner::run_document()` method removed.
7. **Updated `document-processing` skill** — References new tool names, describes LLM-orchestrated two-phase pipeline.

---

---

## UX - `/skill <name>` Subcommand

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Move skill activation from `/<skill-name>` to `/skill <name>` to prevent namespace collisions. ✅ **COMPLETED**

**Problem:**
- Skills were previously activated as top-level commands (e.g., `/document-processing`)
- Any skill name could collide with existing commands (e.g., a skill named "forget", "new", "help")
- No clear separation between built-in commands and user-defined skills
- The wildcard `_` match arm processed skill names last, making collision behavior unpredictable

**Implementation:**
- ✅ `/skill <name>` is now the explicit command to activate a skill
- ✅ `/skill` (no args) lists available skills (`ChatCommand::SkillList`)
- ✅ `/sk` is a shortcut for `/skill`
- ✅ `/<skill-name>` wildcard removed — unknown commands are now invalid (not skill activations)
- ✅ `/skill list` attempts to activate a skill named "list" — no reserved words
- ✅ Help text updated

**Related:** Issue #86 (CLOSED via PR #87)

---

---
