# Implementation Plan for sprachspiel

**Note**: This document tracks implementation status. For strategic direction, see:

## Quick Links

### Canonical Documents

| Document | Purpose |
|----------|---------|
| **[Implementation Directive](./doc/src/development/implementation-directive.md)** | Definitive direction for continuous learning feature |
| [Architecture](./doc/src/development/architecture.md) | Design decisions and system architecture |
| [Roadmap](./doc/src/development/roadmap.md) | Current development status and future plans |

### Reference Documents

| Document | Description |
|----------|-------------|
| [Skills System Design](./doc/src/development/skills-system-design.md) | Skills architecture |
| [Research Synthesis](./doc/src/development/research/research-appendix.md) | Complete research synthesis |
| [Papers Reference](./doc/src/development/research/papers-reference.md) | arXiv links for MemOS, OpenClaw-RL, MemGPT |

### External

- [GitHub Project Board](https://github.com/luksamuk?tab=projects) - Kanban board for task tracking

## Current Version

**v0.43.0** - 2026-05-08 (Sprachspiel Rename + Multi-Backend Positioning)

## Current Implementation Status

✅ **Completed:**

- Core CLI with 5 subcommands (query, chat, translate, ocr, summarize)
- Interactive chat mode with persistent sessions
- Custom models support via `~/.config/sprachspiel/models.toml`
- Built-in models: llama3.1, translategemma, glm-ocr (user models in config)
- Thinking support for cloud models (configurable via `thinking = true`)
- Dynamic model selection with capability detection
- Tool integration with error recovery (50 tools in 14 categories)
- Translation (50+ languages)
- OCR with multiple modes
- Summarization with styles
- Vision analysis
- Markdown rendering
- Pipe support
- Debug mode, Think mode, Code mode
- Token metrics display (`/context`)
- Context management foundation
- Semantic search (`/search`) with hybrid retrieval (BM25 + vector + RRF)
- SQLite storage with sqlite-vec extension
- Embedding generation with Matryoshka truncation (768d → 256d)
- AGENTS.md context injection with security sanitization
- **SOUL.md personality system** - User-configurable agent personality
- **Context Continuity with Graceful Interruption** - LLM pauses/resumes during overflow
- **Factual Memory System** - Persistent fact storage with decay and conflict resolution
- Complete documentation with mdBook
- Man page
- Termux/Android builds
- Error recovery for tool/network errors

### v0.21.x - ChatSession Integration

- ChatSession integration (auto-save messages + embeddings)
- `/migrate` command (JSON → SQLite)
- `/reindex` command (rebuild embeddings)
- Context overflow handling (auto-compaction at 80%)
- Auto-retrieval (M relevant + N recent messages)
- Context composition based on "Lost in the Middle" research

### v0.22.x - Chunking & Compaction

- Message chunking for long messages (>1024 chars)
- UTF-8 safe chunking with char boundary detection
- Synchronous chunking (guaranteed persistence)
- Embedding recovery on startup
- Middle compaction (preserve first N + last N)
- Auto-compaction at 72% warning and 80% overflow
- Visual context utilization bar in /context
- Remember tool for conversation recall
- Conversation-aware retrieval (enrichment)
- Project-aware query mode

### v0.26.x - Memory & Storage

- Source attribution in memory system (`SourceType` enum)
- SQLite as primary storage (schema v4, `/restore`, auto-migration)
- `ConversationStorage` deprecated, removed from REPL

### v0.27.x - Quality Improvements

- Markdown in compaction summaries
- Web scraping content quality improvements
- Compaction visual indicator

### v0.28.x - CLI Tools & Timeout

- **CLI Tools Infrastructure (Phase 1)**
  - External module with types, config, platform detection
  - Per-tool TOML parsing for tools.toml
  - `check_tool_availability()` and `run_command()` tools
  - Simplified run_command API: single command_line string
  - Debug logging for tool failures
  - Fixed duplicate error messages in REPL

- **run_command Security Redesign**
  - No shell features (pipes, redirects, command chains blocked)
  - Mandatory whitelist (only configured tools can execute)
  - head/tail parameters for LLM-controlled output truncation
  - Landlock sandbox (enabled by default on Linux, kernel 5.13+)
  - Platform-specific sandbox handling (Termux, macOS documented)
  - Pattern validation with proper ordering (multi-char before single-char)

- **run_command Timeout & Parameter Types**
  - Fixed critical bug: processes not killed on timeout
  - Changed to tokio::process::Command with kill_on_drop(true)
  - Fixed parameter types from Option<usize> to Option<String> (LLM compatibility)
  - Removed dead code (executor.rs, registry.rs)
  - Added unit tests for timeout and string parameter handling

- **SQLite Cleanup**
  - Created `src/project.rs` with `get_project_id()` and `normalize_git_url()`
  - Updated `history.rs` to be purely migration module (deprecated)
  - Clear separation: project identification vs. legacy storage
  - `history.rs` kept for `/restore` command (disaster recovery)
  - Updated user documentation: `doc/src/commands/chat.md` now explains SQLite storage

---

## Priority Roadmap

### Milestones

| Milestone | Codename | Description | Cards |
|-----------|----------|-------------|-------|
| **[M1]** | Core Evolution | All work before TUI and Sprach 2.0 (7 waves) | #11, #13, #14, #36, #49, #50, #52, #72, #74–#76, #90–#97, #105–#107, #116, #118–#123, #132–#138, #145–#148, #151, #152, #153, #157, #182, #193, #204, #205 |
| **[M2]** | UX & Pre-Launch | TUI design + implementation, benchmarks, learned patterns, diff rendering, DiffWidget | #16, #117, #124, #125 |
| **[M3]** | Sprach 2.0 | CAS research, cognitive extensions, plugin system | #15, #77–#80, #99–#101, #139, #140 + Privacy Filter, ADR: Empathy, meta_cognize, Behavioral Conflict, T3-Phase3 |
| **[M4]** | Future | Deferred features and research | B2–B5, B8–B10 + Attention Priming, Semantic Chunking, Metadata Enrichment, Semantic Dedup, HyDE, Behavioral Embeddings, Behavioral RRF, GAC (#141) |

**M1 Waves:** W1 (Quick Wins: #105, #36) → W2 (Provider Chain: #116→#123, #72) ✅ COMPLETED → W3 (Feedback Completion: #90–#97) → W4 (Embedding Geometry & Flexibility + T3-Phase0: #133→#138, #106, #107, #151, #136) → W5 (M1 Backlog: #13, #14, #49, #50, #52, #74–#76, #132, #204, #205) → W6 (Responsive Chat Rebuild: #145→#148) ✅ COMPLETED → W7 (Thinking Trace Pipeline & Retrieval: #152, #153, #137)

**Priority within milestones** is determined by card order (top = highest priority) on the GitHub Project Board. Cards are referenced by their issue number (e.g., #72, #116).

**M2 note:** M2 is the complete TUI milestone — design, prototyping, and implementation. Builds on top of the Responsive Chat Rebuild (M1, W6) which provides the Ratatui rendering engine, event loop, and CrosstermInput. Benchmarks (#124) are the last thing completed before public release. Learned Patterns (#125) enriches the TUI experience. **Design inputs:** R-32 (ratatui-cheese widget adoption — Help, Fieldset, Select/MultiSelect, List+Paginator; Palette evaluation; direct dependency `ratatui-cheese = "0.7"`), R-33 (first-run onboarding wizard — OnboardingWizard state machine, sub-item of #16). Both evaluated in `doc/m2-ratatui-cheese-evaluation.md` (absorbed into research icebox). **Additional M2 design inputs from competitive benchmark (R-34):** Pinned Messages, Notes/Scratchpad, Context Panel, and Jump Navigation (R-35, R-36) are TUI features that depend on #16 sidebar panels. Diff rendering and DiffWidget depend on the TUI widget infrastructure. **LaTeX rendering (R-37)** is evaluated for M2/M3 via `term-maths` crate with native ratatui MathWidget.

**M1 note:** #11 (Parallel Tool Execution) depends on #121 (Consumer Migration). The multi-provider chain is #116 → #118 → #119 → #120 → #121 → #122 → #123. T3-Phase0 (#151) has NO dependency on #107 or #136 — re-embedding uses the existing background embedding recovery pipeline (normalized items get `has_embedding=0`, then re-embedded on startup). The previous `Depends on: W4.4 (#107)` was artificial. The previous "Joint PR with #136" was decoupled (#136 now depends on #106 and #135, after which it becomes W4.7). T3-Phase0 also includes continuation thinking fix (5th data loss path) and embedding consistency fix (see R5-R7). #157 (Norm Correction) is a W4.x addendum — ~20 lines of Rust, 1 SQL migration, depends on #133 (diagnostics). #182 (System Prompt Clarifications) is an independent prompt-only fix (Instruction Hierarchy + Language Note + TOOL USAGE reformulation + token optimization) — can be done in any wave.

**M3 change:** S2.2 (Content Relations Graph) elevated from LOW to MEDIUM priority. Competitive analysis shows that graph-based retrieval is a key differentiator in the memory-augmented agent space, and delay risks falling behind. T3-Phase3 (Semantic/Reflect + Facts Integration) added to M3 — depends on W7.1 (Thinking-Aware Retrieval) completion.

**M3 additions:** #139 (PCA Projection Search) enables efficient approximate retrieval for S2.1/S2.2 graph features. #140 (Geometry Documentation) provides the complete model selection guide and reference documentation.

**M4 additions:** #141 (Geometry-Aware Consolidation) depends on #142 (Memory Consolidation Design), which in turn depends on S2.1 (#77) and S2.2 (#78) graph features. Both deferred to M4 because the graph substrate must exist first.

> **See also:** [Research Icebox](./doc/src/development/research-icebox.md) for deferred refinement topics, competitive research, and decision records.

### M1 Implementation Waves

M1 contains ~38 open cards organized into 7 implementation waves. Each wave has a theme and completion criterion. Waves are sequential by default, but W1 (Quick Wins) can be done in parallel with early W2 work since W1 items have no dependencies.

| Wave | Codename | Theme | Cards | Completion Criterion |
|------|----------|-------|-------|---------------------|
| **W1** | Quick Wins | Small independent items, no dependencies | #126, #105, #36 | #126 ✅ COMPLETED; #105 ✅ COMPLETED; #36 ✅ COMPLETED |
| **W2** | Provider Chain | Multi-provider migration (10-12 week dependency chain) | #116, #118, #119, #120, #121, #11, #122, #123, #72, **#201** | ✅ COMPLETED — `ollama-rs` removed from Cargo.toml; #201 message-ordering shipped; #72 closes with #123 merge |
| **W3** | Feedback Completion | Close decay activation, research & implement feedback expansion | #90, #91, #92, #93, #94, #95, #96, #97 | All feedback items researched and implemented or deferred |
| **W4** | Embedding Geometry & Flexibility + T3-Phase0 | Embedding diagnostics, geometry-aware config, model validation, provider abstraction, thinking preservation, prompt clarifications | #133, #134, #106, #135, #107, #151, #136, #138, #157, #182 | Diagnostics subcommand works ✅; fact threshold validated ✅; norm correction ✅; system prompt clarified ✅; at least one alternative model benchmarked; thinking content preserved in DB ✅; embedding model registry + geometry-aware dimensions; instruction hierarchy in prompt |
| **W5** | M1 Backlog | Batch doc processing, context, secrets, personalities, file tracking, file write tools | #132, #74, #75, #76, #13, #14, #49, #50, #52, #204, #205 | All items completed or deferred to M2 |
| **W6** | Responsive Chat Rebuild | Replace println+ANSI with Ratatui for responsive chat rendering | #145, #146, #147, #148 | ✅ COMPLETED — All chat rendering via ChatView/RatatuiView; rustyline removed; responsive at any terminal width |
| **W7** | Thinking Trace Pipeline & Retrieval | Preserve thinking content, T3 Struct pipeline, thinking-aware retrieval | #152, #153, #137 | Thinking traces preserved and transformable; retrieval includes thinking context; RRF adapts to d_eff with trace awareness |

**Wave dependencies:**

- **W1** has no blockers — can start immediately. #126 (rename) touches many files but is independent; do it first since it's `priority:critical`
- **W2** has internal dependency chain: `#116 → #118 → #119 → #120 → #121 → #122 → #123`; `#11` depends on `#121`; `#72` closes when chain completes
- **W3**: `#90` is closable now (decay fix merged); `#91`-`#97` need research before implementation can be sized
- **W4**: independent of W2 (embedding config is orthogonal to provider migration). Expanded from original scope (config + provider) to include geometry-aware changes from embedding audit and T3-Phase0. Sub-phases:
  - **W4.0** (#133): Diagnostics subcommand (`sprach diagnostics`) — measure d_eff, average magnitude, threshold pass rate ✅ COMPLETED
  - **W4.1** (#134): Validate fact semantic threshold (0.70 vs 0.80) before changing — data-driven decision ✅ COMPLETED
  - **W4.2** (#106): Configurable embedding model + server-side Matryoshka — the original W4 scope
  - **W4.3** (#135): Benchmark alternative models (Nomic v2, Snowflake, mxbai, qwen3) with d_eff metric
  - **W4.4** (#107): Embedding provider abstraction — multi-provider embedding support. **Includes token-aware chunking** (Prioridade 3 SOTA): replace chars/token estimate with real tokenizer counts via `/tokenize` endpoint or tokenizer crate. Eliminates the root cause of chunk sizing bugs (chars/token imprecision). See "Embedding Fallback + Chunk Sizing Fix" section for context.
  - **W4.5** (#151): T3-Phase0 (Preserve Thinking Content) — standalone PR, no dependency on #107 or #136. Re-embedding uses existing `/reindex --yes` recovery pipeline. Includes continuation thinking fix.
  - **W4.6** (#138): Documentation rewrite — model selection guide, hybrid search explanation, provider docs
  - **W4.7** (#136): Geometry-Aware Embedding Configuration and Model Registry — depends on #106 (configurable model) and #135 (d_eff measurements). Rewritten scope: `embedding_models` table, diagnostics integration, `recommended_dimensions()`, dynamic vec0 dimensions. See Decision Record D-10 and D-11.
- **W5**: independent — can be picked up between waves or as mental breaks from larger work
- **W6**: starts after critical bugs are resolved. 4 sequential PRs (CommandOutput → Rendering → Input+Event Loop → Final Transition). Depends on W5 completion being far enough along that the REPL is stable. Prerequisite for M2 TUI (#16).
- **W7**: starts after W6-PR3 (#147) is merged and W4.5 (T3-Phase0) is complete. Sub-phases:
  - **W7.0** (#152): T3-Phase1 — ThinkingTrace Pipeline + Struct Transform. Background job, same-model/CPU-fallback cascade, `[thinking_trace]` config section (already created in Phase 0). **Do not start before #147 merged.**
  - **W7.1** (#153 + #137): T3-Phase2 (Thinking-Aware Retrieval + RRF Fusion) + Geometry-Aware RRF. RRF weights adapt to d_eff and are aware of thinking traces. #137 moved from W4.6 so RRF is designed with trace awareness from the start.

### ✅ PRIORITY 0: Rename ask-ai → Sprachspiel (COMPLETED) [M1]

**Status:** ✅ COMPLETED
**Issue:** #126
**Branch:** `feat/rename-to-sprachspiel`

**Goal:** Complete project rename from ask-ai to Sprachspiel, shorten binary to `sprach`, and remove Ollama vendor-exclusivity from documentation.

**Design Decisions:**
- **No migration code:** No fallback paths, no `APP_DIR_LEGACY`, no backward compatibility dirs. Users rename manually.
- **No transition period:** No symlink/wrapper. Clean cut.
- **DB migration only:** `migrate_legacy_db()` extended: `embeddings.db` → `sprachspiel.db` and `ask-ai.db` → `sprachspiel.db` (simple rename-if-exists).
- **Full rename:** Binary, crate, all paths, all docs, all scripts, man page, assets, module names.
- **Short binary name:** CLI command is `sprach` (6 chars), project identity stays `sprachspiel`. Config/data dirs are `~/.config/sprachspiel/`, `~/.local/share/sprachspiel/`, `.sprachspiel/`, `sprachspiel.db`.
- **Multi-backend positioning:** Documentation now says "LLM models via Ollama and compatible backends" instead of "Ollama LLM models". Config keys `[ollama]` unchanged (reflects real code). Sample config updated: "LLM server" instead of "Ollama server".

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Art generation (BANNER_LOGO for SPRACHSPIEL) | ✅ Done |
| 1 | Constants & path centralization (APP_NAME, DB_FILENAME, .join() calls) | ✅ Done |
| 2 | Binary & module rename (Cargo.toml, main.rs, doc comments) | ✅ Done |
| 3 | User-facing strings & error messages (~12 refs in 6 files) | ✅ Done |
| 4 | Makefile, scripts & man page | ✅ Done |
| 5 | Assets & art (BANNER_LOGO, PDF rename) | ✅ Done |
| 6 | Documentation (doc/, CHANGELOG, AGENTS.md, .opencode/, IMPLEMENTATION.md) | ✅ Done |
| 7 | Testing (cargo build, clippy, test, manual) | ✅ Done |
| 8 | Binary rename: sprachspiel → sprach (short CLI command) | ✅ Done |
| 9 | Documentation: remove Ollama vendor-exclusivity (11 files) | ✅ Done |
| 10 | Sample config: remove ask-ai refs in settings.rs | ✅ Done |
| 11 | GitHub repo rename (`ask-ollama-rs` → `sprachspiel`), update local remote, configure GitHub Pages | 📋 Pending (post-merge) |
| 12 | Post-merge documentation cleanup: remaining ask-ai/sprach refs, GitHub URLs → sprachspiel, book.toml, launch reel, proc-macro naming | ✅ Done |

**Note:** GitHub automatically redirects old repo URLs (`ask-ai-rs`) to the new name (`sprachspiel`) indefinitely after rename. URLs updated pre-emptively in docs.

**Files to Create:**
- `src/consts/app.rs` — Centralized app name constants (`APP_NAME`, `DB_FILENAME`)

**Files to Modify (major):**
- `Cargo.toml` — `name`, `bin_name`
- `src/main.rs` — `name`, `bin_name`, `about`
- `src/chat/view/mod.rs` — `BANNER_LOGO` constant
- `src/db/connection.rs` — DB path, migration logic
- `src/settings.rs` — config paths
- `src/skills/loader.rs` — skills directory paths
- `src/chat/input/mod.rs` — history path
- `src/external/config.rs` — tools config path
- `src/tools/run_cmd.rs` — sandbox paths
- `src/logging.rs` — log path, module path
- `src/soul.rs` — SOUL.md path
- `src/user_models.rs` — models.toml path
- `Makefile` — `BINARY`, `TARGET`, all tarball names
- `man/ask-ai.1` → `man/sprach.1`
- `scripts/install-ask-ai.sh` → `scripts/install-sprach.sh`
- `scripts/install.sh`, `scripts/uninstall.sh`

**Extra scope:**
- All `ask-ollama-rs` remnants in docs, assets, .opencode skills, SMOKE_TEST, STRESS_TEST
- All `ask-ai` references in documentation (~1295 refs)
- `assets/ask-ai-banner.py` → `assets/sprachspiel-banner.py`
- `assets/ask-ai-architecture.pdf` → `assets/sprachspiel-architecture.pdf`
- `.ask-ai/` → `.sprachspiel/` (project skills directory)
- `ask-ai.db` → `sprachspiel.db` (with migration from old names)
- GitHub repo rename (`ask-ollama-rs` → `sprachspiel`) — last step, post-merge

**Related:** Issue #126

**Clippy Debt Cards (identified during rename, to be created as issues):**

These were identified during the `cargo clippy` audit after the rename. They are NOT part of the rename PR but are tracked here for card creation:

| Card | Description | Count | Priority | Issue |
|------|-------------|-------|----------|-------|
| Unwrap/expect/panic triage | All 44 clippy violations annotated with `#[expect]`; mutex `.unwrap()` → `.expect()` | 44 sites fixed | 🔴 Critical | #128 (✅ COMPLETED) |
| Function extraction | Refactor 26 functions exceeding 100 lines (top 5 first) | 26 functions | 🔴 Critical | #129 (🔄 IN PROGRESS) |
| Complexity reduction | Reduce cognitive complexity in 13 functions (max: 62/15) | 13 functions | 🔴 Critical | #130 |
| Remove `#![expect(print)]` | Remove crate-level print expects before TUI; add module-level expects only to CLI modules | 2 attrs | 📋 TUI-prereq | #131 |

---

### ✅ PRIORITY: Config Upgrade Command — #105 [M1]

**Status:** ✅ COMPLETED
**Issue:** #105
**PR:** #192
**Branch:** `feat/105-config-upgrade`
**Depends on:** None (W1 quick win — no dependencies)

**Goal:** Add a `sprach config upgrade` subcommand that merges missing default fields into the user's existing `config.toml`, preserving all existing values, user comments, and formatting.

**Problem Statement:**

Every release adds new config fields (e.g., `[feedback]` in v0.40, `[facts]` in v0.42, `[retrieval]` in v0.43, `[thinking_trace]` in v0.45). Users with existing configs miss new fields because:
- `serde(default)` silently fills missing fields with no user-visible indication
- The sample config (`--init-config`) creates a full file but does not merge with existing
- Users must read CHANGELOG to discover new fields and add them manually

**Proposed Solution:**

```bash
sprach config upgrade [--dry-run] [--no-backup]
```

**Behavior:**
1. Read user config with `toml` (preserves structure)
2. Parse with `serde` to detect which fields are present
3. Compare against `Settings::default()` to find missing fields
4. Insert missing fields with doc-comments using `toml_edit` (extracted from `SAMPLE_CONFIG`)
5. Write back, preserving all existing content (insert-only — never modifies existing fields)

**Example Output:**

```
$ sprach config upgrade
Config: /home/user/.config/sprachspiel/config.toml

Found 3 new fields:
  - facts.auto_extract (default: true, bool)
  - facts.max_facts (default: 3, u32)
  - facts.auto_extract_notify (default: true, bool)

Backup created: /home/user/.config/sprachspiel/config.toml.bak
Upgraded 3 fields successfully.
```

**Architecture:**

- Two-pass approach: `toml` for detection, `toml_edit` for file operations
- New module: `src/commands/config_upgrade.rs`
- New dependency: `toml_edit = "0.22"` (compatible with `toml 0.8`)
- Insert-only: never modify existing fields or comments
- Backup file created before upgrade: `.bak` or `.bak.YYYYMMDD-HHMMSS` if `.bak` exists
- Doc-comments extracted from `SAMPLE_CONFIG` static (parsing of comment blocks preceding each field)
- Invalid TOML → report parser error, abort (no destructive overwrite)

**Design Decisions:**

1. **`--no-backup` (opt-out, default = backup active)** — Aligns with the principle of not destroying user data. Inverts the issue's original `--backup` proposal but is consistent with `--dry-run` (also opt-in, also a "safety off" flag).
2. **Insert-only semantics** — The command is purely additive. Never modifies or removes existing values. This guarantees zero risk of data loss.
3. **`toml_edit` for file writes, `toml` for parsing** — `toml_edit` preserves comments and formatting, which is essential for the "non-destructive" promise. `toml` is faster and more lenient for detection.
4. **Doc-comments from `SAMPLE_CONFIG` parsing** — Reuses the existing sample config string. The `extract_field_comment(path)` helper parses the comment block immediately preceding each field assignment in the sample. This guarantees sync with `Settings::default()` whenever someone edits the sample.
5. **Invalid TOML aborts** — The command does NOT attempt to recover from invalid syntax. It reports the parser error, suggests `--init-config` for a full reset, and exits with code 1. Avoids silent corruption.
6. **No interactive prompts** — The command is fully non-interactive. Suitable for scripts and CI.

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1 | Add `toml_edit` to `Cargo.toml` | `Cargo.toml` | ✅ |
| 2 | Refactor `create_sample_config` to expose `SAMPLE_CONFIG: &str` constant | `src/settings.rs` | ✅ |
| 3 | Create `src/commands/mod.rs` + skeleton of `config_upgrade.rs` | `src/commands/*` | ✅ |
| 4 | Implement `MissingField` struct + comment extraction from `SAMPLE_CONFIG` | `src/commands/config_upgrade.rs` | ✅ |
| 5 | Implement `ConfigUpgrader::new` (read config, parse with `toml`) | `src/commands/config_upgrade.rs` | ✅ |
| 6 | Implement `detect_missing` (field-by-field comparison) | `src/commands/config_upgrade.rs` | ✅ |
| 7 | Implement `apply` (write with `toml_edit`, insert with comments) | `src/commands/config_upgrade.rs` | ✅ |
| 8 | Implement `backup` (`.bak` or timestamped variant) | `src/commands/config_upgrade.rs` | ✅ |
| 9 | Add CLI args + `handle_config_upgrade` dispatch in `main.rs` | `src/translate/cli.rs`, `src/main.rs`, `src/translate/mod.rs` | ✅ |
| 10 | 17 unit tests inline (cfg(test)) | `src/commands/config_upgrade.rs` | ✅ |
| 11 | Documentation: `doc/src/commands/config-upgrade.md`, `doc/src/SUMMARY.md`, `man/sprach.1` | various | ✅ |
| 12 | Quality gates: `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`, `cargo test --all-features`, `cargo audit` | — | ✅ |

**Files to Create:**
- `src/commands/mod.rs` — Module exports
- `src/commands/config_upgrade.rs` — `MissingField`, `ConfigUpgrader`, `UpgradeReport`, CLI logic
- `doc/src/commands/config-upgrade.md` — User documentation page

**Files to Modify:**
- `Cargo.toml` — Add `toml_edit = "0.22"`
- `src/settings.rs` — Expose `SAMPLE_CONFIG: &str` constant; refactor `create_sample_config` to use it
- `src/translate/cli.rs` — Add `Commands::Config(ConfigArgs)`, `ConfigArgs`, `ConfigAction::Upgrade(UpgradeArgs)`, `UpgradeArgs`
- `src/translate/mod.rs` — Re-export `ConfigArgs`, `UpgradeArgs`, `ConfigAction`
- `src/main.rs` — Add `mod commands;`, dispatch `Commands::Config` → `handle_config`
- `doc/src/SUMMARY.md` — Add entry to commands section
- `man/sprach.1` — Document `config upgrade` subcommand

**Test Plan (17 unit tests in `#[cfg(test)]`):**

1. `test_detect_no_missing_when_complete` — full config yields empty list
2. `test_detect_missing_entire_section` — `[facts]` absent
3. `test_detect_missing_field_in_existing_section` — `[facts].max_facts` absent
4. `test_detect_multiple_missing` — combination
5. `test_apply_preserves_existing_values` — existing values unchanged
6. `test_apply_inserts_missing_with_default` — inserted values match `Settings::default()`
7. `test_apply_preserves_user_comments` — user comments above existing fields survive
8. `test_apply_writes_correctly_with_toml_edit` — file is valid TOML after write
9. `test_backup_creates_bak_file` — `.bak` is created
10. `test_backup_uses_timestamp_if_bak_exists` — second run uses `.bak.YYYYMMDD-HHMMSS`
11. `test_dry_run_does_not_modify_file` — file mtime/content unchanged
12. `test_dry_run_does_not_create_backup` — no `.bak` created in dry-run
13. `test_no_backup_flag_skips_backup` — explicit `--no-backup` works
14. `test_invalid_toml_returns_error` — malformed TOML aborts with error
15. `test_missing_config_file_returns_error` — non-existent config aborts
16. `test_extract_comment_from_sample` — comment extraction works for known fields
17. `test_dotted_path_notation` — `facts.auto_extract` path resolution

**Estimated effort:** ~5 days (consistency with the issue's original estimate)

**Reference:** Issue #105 (canonical)

---

**Implementation Summary** (added on PR review pass)

**Files changed:** 11 total (3 new, 8 modified), 1949 insertions, 382 deletions at the original implementation commit. The cleanup pass (commits after PR review) added 4 more commits: `dcb468f` (R1+R3 cleanup), `91c9345` (R4 SAFETY doc + nested regression test), `f6d9987` (R7 split I/O + R5 strengthen test), `a440414` (R6 manpage aliases).

**Test count:** 21 unit tests in `config_upgrade.rs` (3 new since original: `test_apply_creates_deeply_nested_table` for the unsafe-regression test, plus 3 strengthened assertions in `test_run_upgrade_already_up_to_date`). All passing.

**Quality gates:** `cargo fmt --check`, `cargo clippy --all-features -- -D warnings -A clippy::allow_attributes -A clippy::too_many_lines -A clippy::cognitive_complexity`, `cargo test --all-features` (2764+ total tests), `cargo audit` (no new advisories) — all passing.

**Aliases:** `cfg` (for `config`) and `up` (for `upgrade`) are intentional and documented in `man/sprach.1`.

**Unsafe in `ensure_table_chain`:** The raw-pointer descent is the only `unsafe` in the codebase. The review suggested refactoring to recursion; this was attempted but the borrow checker (NLL) cannot express the required 'reborrow a nested field and return the deepest reference' pattern. Polonius would solve this but is not yet stable. The `unsafe` is retained with a tightened SAFETY justification (each pointer explained, aliasing argument made explicit) and a new regression test (`test_apply_creates_deeply_nested_table`) that exercises two distinct 3-level nested paths to provide ongoing evidence that the pointer arithmetic is sound.

---

### 🔵 PRIORITY: Session Forget — Destructive Session Deletion with Confirmations — #36 [M1]

**Status:** ✅ COMPLETED
**Issue:** #36
**PR:** #195
**Branch:** `feat/36-session-forget`
**Depends on:** None (W1 quick win — no dependencies)

**Goal:** Replace the `/forget` command with `/session forget` — the canonical path for deleting sessions by name or ID, with preview confirmation, cascade deletion, and context-sensitive autocomplete. Also add unique name constraint to `/save`.

**Problem Statement:**

1. `/forget --yes` only deletes the current session — no way to delete a specific session by name or ID.
2. `/forget` is a destructive alias with no preview of what will be deleted.
3. `/save <name>` allows duplicate names within the same project, causing ambiguity.
4. Tab completion for `/session` subcommands doesn't exist — users must memorize arguments.

**Redesign Decisions (per user discussion):**

1. **Remove `/forget` entirely.** `/session forget` is the only path. No aliases for destructive commands — the user must be explicit.
2. **`--yes` is not autocompletable.** Safety feature: the user must type `--yes` manually after seeing the preview. Tab will not suggest `--yes` after a session name.
3. **Session names are unique per project.** `/save <name>` rejects duplicate names within the same project. The same name in different projects is allowed.
4. **Notes and facts are NOT deleted** with a session. They belong to the project, not the session (no `conversation_id` column).

**Canonical Commands:**

| Command | Action |
|---------|--------|
| `/session forget` | Preview: warns that `--yes` is required (current session) |
| `/session forget --yes` | Deletes current session, starts fresh |
| `/session forget <name>` | Preview: shows counts, warns that `--yes` is required |
| `/session forget <name> --yes` | Deletes session identified by name |
| `/session forget --id <id> --yes` | Deletes session by ID (for disambiguation) |

**Autocomplete Behavior:**

| Input | Tab shows |
|--------|-----------|
| `/session` | `forget` (Delete current or specific session), `list`, `new`, `load`, `save` |
| `/session forget ` | `--id` (Select session by ID), `--yes` (Confirm deletion — current session), session names (newest first) |
| `/session forget --id ` | Session IDs with session name as description (newest first) |
| `/session forget <name>` | **Nothing** — `--yes` must be typed manually (safety) |

**Project vs. Session (ELI15):**

- **Project** = where you are (derived from git remote or folder name). Groups sessions, facts, notes, documents.
- **Session** = a conversation within a project. Contains messages, embeddings, chunks, and todos.
- `/list` only shows sessions from the current project. Facts, notes, and documents are shared across all sessions in the project.
- Deleting a session deletes its messages, embeddings, chunks, and todos. Facts/notes/documents remain intact.

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1 | Add `SessionForgetTarget` enum + `SessionForget` variant to `ChatCommand` | `src/chat/commands.rs` | ✅ COMPLETED |
| 2 | Remove `Forget { confirmed: bool }` variant from `ChatCommand` | `src/chat/commands.rs` | ✅ COMPLETED |
| 3 | Update `parse_session_subcommand()` for new format | `src/chat/commands.rs` | ✅ COMPLETED |
| 4 | Remove `/forget` shortcut from `parse_command()` dispatch | `src/chat/commands.rs` | ✅ COMPLETED |
| 5 | Add `SessionItemCounts` struct + `count_session_items()` | `src/content/db.rs` | ✅ COMPLETED |
| 6 | Add `handle_session_forget()` handler with preview + confirmation | `src/chat/command_handlers.rs` | ✅ COMPLETED |
| 7 | Remove `handle_forget_cmd()` + `handle_forget()` | `src/chat/command_handlers.rs` | ✅ COMPLETED |
| 8 | Add unique name constraint in `handle_save()` | `src/chat/command_handlers.rs` | ✅ COMPLETED |
| 9 | Update `ChatCompleter` — session subcommands + session names + `--id` + `--yes` | `src/chat/completer.rs` | ✅ COMPLETED |
| 10 | Remove `/forget` from `SLASH_COMMANDS` | `src/chat/completer.rs` | ✅ COMPLETED |
| 11 | Update `app.rs` to refresh `session_names` in completer | `src/chat/app.rs` | ✅ COMPLETED |
| 12 | Update help text, man page, user docs | `src/chat/commands.rs`, `man/sprach.1`, `doc/src/commands/chat.md` | ✅ COMPLETED |
| 13 | Unit tests (parsing, DB, completer, handler) | `src/chat/commands.rs`, `src/chat/completer.rs`, `src/content/db.rs` | ✅ COMPLETED |
| 14 | Quality gates: fmt, clippy, test | — | ✅ COMPLETED |

**Design Decisions:**

1. **`/forget` removed entirely.** No alias for destructive commands. The canonical path is `/session forget`. This removes cognitive load (one way to do things) and prevents accidental execution.
2. **`SessionForgetTarget` enum:**
   - `Current` — deletes current session (equivalent to old `/forget --yes`)
   - `ByName(String)` — deletes session by name
   - `ById(String)` — deletes session by ID (disambiguation for duplicate names from pre-constraint data)
3. **Preview before deletion.** Without `--yes`, shows: session name, ID, message count, embedding count, todo count. Notes and facts are NOT shown (they belong to the project, not the session).
4. **`--yes` not autocompletable.** After `/session forget <name>`, Tab offers nothing. The user must manually type `--yes` to confirm. This is a safety feature, not a limitation.
5. **Unique name constraint in `/save`.** `handle_save()` checks `find_conversation(name)` before renaming. If another session in the same project has the name, it returns an error. Same-name-as-own-session is allowed (idempotent rename).
6. **Cascade deletion is existing code.** `delete_conversation()` already handles: embeddings → chunks → content items → conversation. `session_todos` has `ON DELETE CASCADE` FK. No new deletion logic needed.
7. **Notes and facts survive session deletion.** They have `project_id`/`scope` scoping, no `conversation_id`. Deleting a session only removes messages, embeddings, chunks, and todos.
8. **Duplicate name disambiguation for legacy data.** If `find_conversation()` finds multiple sessions with the same name (from before the constraint), the error message suggests using `--id`.

**Estimated effort:** ~3-4 days (increased from 2 days due to: autocomplete, `/forget` removal, unique name constraint)

**Reference:** Issue #36

---

### 🔴 PRIORITY 0: T3-Phase0 — Preserve Thinking Content + Schema Foundation [M1]

**Status:** ✅ COMPLETED
**Issue:** #151
**PR:** #189
**Branch:** `feat/151-thinking-preserve`
**Depends on:** None (dependency on #107 was artificial — re-embedding uses existing `/reindex --yes` recovery pipeline; #136 decoupled — see D-11)

**Goal:** Fix the architectural bug where `strip_thinking_tags()` permanently deletes thinking content before storage. Preserve thinking traces as the most valuable RAG corpus for reasoning tasks (Arabzadeh et al. 2026, arXiv:2605.03344).

**Problem Statement:**

The paper "RAG over Thinking Traces Can Improve Reasoning Tasks" demonstrates that thinking traces are a fundamentally superior corpus to conventional documents for reasoning tasks via RAG (+56.3% accuracy on AIME). Sprachspiel currently commits the exact error the paper identifies as suboptimal: systematically discarding thinking traces.

**Architecture Bug — Asymmetric Storage:**

```
CASO 1: Normal assistant messages (message_type = 'normal')
→ strip_thinking_tags() REMOVES all <thinking> content BEFORE storage
→ File: src/chat/thinking.rs:123 (called via strip_thinking_tags wrapper)
→ Result: thinking trace is LOST permanently

CASO 2: Pre-tool messages (message_type = 'pre_tool_content')
→ Thinking is CONCATENATED inline as <thinking> XML tags in content field
→ Result: thinking trace is stored INCIDENTALLY, mixed with content
```

**Key Insight:** `process_thinking()` already correctly splits thinking from content, but callers use `strip_thinking_tags()` for storage. The preservation path exists — we need to use it.

**Implementation Phases (12 Steps):**

| Step | Description | Status | Key Files |
|------|-------------|--------|-----------|
| 1 | Schema — Migration v13→v14: `ALTER TABLE content_items ADD COLUMN thinking_content TEXT` | ✅ | `schema.rs`, `connection.rs` |
| 2a | `ContentItem` + DB operations: `thinking_content: Option<String>`, `insert_content_item()` param, 6 SQL queries, 2 inline constructions, `row_to_content_item()` | ✅ | `types.rs`, `db.rs` |
| 2b | Data migration — normalize existing pre-tool messages with inline `<thinking>` tags: `Database::normalize_inline_thinking()` method; resets `has_embedding=0`, deletes stale embeddings/chunks; runs in background spawn before embedding recovery; explicit `BEGIN`/`COMMIT` transaction for atomicity | ✅ | `connection.rs`, `repl.rs`, `repl_tui.rs` |
| 3 | `SendMessageResult.thinking` + `SavedMessage.thinking` (with `#[serde(default)]`) | ✅ | `core.rs`, `session.rs` |
| 4 | Replace storage callers: `extract_thinking()` (respects API-native `thinking` field) instead of `strip_thinking_tags()` in `process_chat_response()` and `send_message_stream()` | ✅ | `core.rs` |
| 5 | Normalize `add_pre_tool_message()` — remove inline `<thinking>` formatting, pass `thinking_content` as separate DB column | ✅ | `session.rs` |
| 6 | `ContinuationResult` thinking field + `handle_continuation(user_message_id: Option<i64>)` signature + accumulation logic | ✅ | `continuation.rs` |
| 7 | `add_assistant_message(content, thinking, prompt_tokens)` — 6 callers (5 pass `None`, 1 passes thinking) | ✅ | `session.rs`, `command_handlers.rs` |
| 10 | `process_send_result()` — pass `thinking` to `add_assistant_message()`, derive from continuation or direct result | ✅ | `continuation.rs` |
| 11 | `load_sqlite()` — map `item.thinking_content` → `SavedMessage.thinking` | ✅ | `session.rs` |
| 8 | `ThinkingTraceSettings` + `[thinking_trace]` config + `get_messages_for_llm(include_thinking)` + `RetrievalConfig.include_thinking` + context builder injection | ✅ | `settings.rs`, `session.rs`, `context_builder.rs`, `command_handlers.rs` |
| 9 | Tests: migration, roundtrip, continuation, retrocompat, search, compaction, pre-tool, inline data migration, feature flag | ✅ | Test files |
| 12 | Documentation — naming cleanup: `[t3]` → `[thinking_trace]`, `T3Status` → `ThinkingTraceStatus` | ✅ | Various docs |

**Step Dependencies:**
```
1 → 2a → 2b → 3 → (4 ‖ 5) → (6 ‖ 7) → 10 → 11 → 8 → 9 → 12
```

**`thinking_trace_status` deferred to T3-Phase1 (#152):** In Phase 0, `thinking_content IS NOT NULL` is equivalent to "has thinking." Phase 1 introduces the Thinking Trace Transform pipeline and needs `ThinkingTraceStatus` enum (`None=0, Raw=1, Pending=2, Done=3`) stored as `thinking_trace_status INTEGER DEFAULT 0`. See Decision Record D-09.

**Naming Convention:**

| Context | Name |
|---------|------|
| Config section | `[thinking_trace]` |
| Rust struct | `ThinkingTraceSettings` |
| DB column (Phase 1) | `thinking_trace_status` |
| Enum (Phase 1) | `ThinkingTraceStatus` |
| Doc first mention | Thinking Trace Transform (T3) |
| Doc subsequent | T3 (acceptable shorthand) |

**Refinements from Deep Analysis:**

| # | Refinement | Impact | Detail |
|---|-----------|--------|--------|
| R1 | Inline data migration for existing pre-tool messages | Medium | Step 2b — `Database::normalize_inline_thinking()` uses `process_thinking()` to split inline `<thinking>` tags from stored `content` into `thinking_content` column. Without this, old rows pollute LLM context with raw tags. Called from `repl.rs` (where both `db` and `chat::thinking` are importable, avoiding circular `db`→`chat` dep). |
| R2 | `handle_continuation()` needs `user_message_id` param | Low | Step 6 — add `user_message_id: Option<i64>` to signature. `process_send_result()` already has this value and passes it at call site (L80). |
| R3 | `ChatMessage` has no `with_thinking()` builder | Low | Step 8 — construct `ChatMessage` manually: `msg.thinking = Some(thinking.clone())` when `[thinking_trace] enabled`. Follows pattern in `custom_coordinator.rs:780-794`. |
| R4 | `get_messages_for_llm()` + `build_context()` need thinking injection | Medium | Step 8 — `get_messages_for_llm(system_prompt, include_thinking: bool)` passes thinking to `ChatMessage`. `RetrievalConfig.include_thinking` field added. Both gated by `settings.thinking_trace.enabled`. |
| R5 | `normalize_inline_thinking()` must reset `has_embedding=0` for stale embeddings | High | Step 2b extension — when `content` is rewritten (thinking removed), the existing embedding (computed from old text with `<thinking>` tags) becomes semantically stale. The UPDATE must also set `has_embedding = 0`, delete the stale row from `content_embeddings` (vec0), and delete stale `content_chunks`. The existing background embedding recovery pipeline (repl_tui.rs) then picks up these items and regenerates embeddings from the cleaned content. Without this fix, vector search returns results with embedding/content mismatch. |
| R6 | `normalize_inline_thinking()` must run in background, not block startup | Medium | Step 2b extension — the method was initially called synchronously in `init_chat_database()`, blocking the TUI before it renders. Move to the background `tokio::spawn` in `run_startup_tasks()` / `repl_tui.rs`, running before the embedding recovery pipeline. For DBs with many pre-tool messages, this avoids a visible startup delay. The normalization itself is fast (sub-second typically), but the subsequent embedding regeneration it triggers is the slow part — which already runs in background with ⚙ status bar indicator. |
| R7 | Normalization result should be communicated via log + chat message | Low | Step 2b extension — when items are normalized, log the count ("Normalized N content items") and show a system message in the chat ("💾 Migrated N pre-tool messages — embeddings being regenerated"). No dedicated progress bar needed: normalization is sub-second, and the ⚙ indicator from the subsequent embedding recovery already provides visual feedback. |

**Files to Create:**
- None (inline data migration is a method on `Database`, not a new file)

**Files to Modify:**
- `src/db/schema.rs` — `SCHEMA_VERSION = 14`, `thinking_content TEXT` in CREATE TABLE
- `src/db/connection.rs` — `migrate_v13_to_v14()` (ALTER TABLE only) + dispatcher + `normalize_inline_thinking()` method (with `has_embedding=0` reset, stale embedding/chunk deletion)
- `src/content/types.rs` — `ContentItem.thinking_content: Option<String>`
- `src/content/db.rs` — `insert_content_item()` param, 6 SQL queries, 2 inline constructions, `row_to_content_item()`
- `src/chat/core.rs` — `SendMessageResult.thinking`, 2× `process_thinking()` replacing `strip_thinking_tags()`
- `src/chat/session.rs` — `SavedMessage.thinking`, `add_assistant_message()` param, `add_pre_tool_message()` storage, `load_sqlite()`, `get_messages_for_llm(include_thinking)` param
- `src/chat/continuation.rs` — `ContinuationResult` 3 fields, `handle_continuation(user_message_id)` signature, `process_send_result()` thinking
- `src/chat/mod.rs` — Add `process_thinking` to re-exports
- `src/chat/thinking.rs` — (no change — `process_thinking` already public)
- `src/settings.rs` — `ThinkingTraceSettings`, `Settings.thinking_trace`, sample config
- `src/retrieval/context_builder.rs` — `RetrievalConfig.include_thinking`, `push_messages_as_chat_messages(thinking)` param
- `src/chat/repl.rs` — Move `normalize_inline_thinking()` call from `init_chat_database()` to background spawn
- `src/chat/repl_tui.rs` — Add normalization step before embedding recovery in background spawn
- `src/chat/command_handlers.rs` — Pass `include_thinking` to `get_messages_for_llm()`, pass `None` for thinking in 5 `add_assistant_message()` callers
- `IMPLEMENTATION.md` — Naming cleanup
- `doc/src/development/architecture.md` — Naming update
- `doc/src/development/research-icebox.md` — D-09 naming update

**Design Decisions:**

1. **Preserve always, transform later:** `thinking_content` is always saved regardless of `[thinking_trace] enabled`. This ensures no data loss. The flag only controls whether the Thinking Trace Transform pipeline processes traces.
2. **No `ContentType::ThinkingTrace` variant:** Thinking is an attribute of a message, not a separate content type. The `thinking_content` column in `content_items` is the correct approach. Transform outputs live in a separate `thinking_traces` table (Phase 1). See Decision Record D-07.
3. **`strip_thinking_tags()` remains for display:** The function is still used by views and query mode to strip thinking from displayed content. Only the storage path changes.
4. **No `thinking_trace_status` column in Phase 0:** In Phase 0, `thinking_content IS NOT NULL` is equivalent to "has thinking content." The `ThinkingTraceStatus` enum (`None=0, Raw=1, Pending=2, Done=3`) and `thinking_trace_status INTEGER DEFAULT 0` column are deferred to T3-Phase1 (#152) when the transform pipeline needs state tracking. See Decision Record D-09.
5. **Continuation thinking uses original `previous_message_id`:** All pre-tool messages from continuation turns reference the same user message as the initial turn. This is semantically correct — all are "what the assistant thought before calling a tool, in the same response to the same user message." Multiple pre-tool messages with the same parent are expected and handled by the `previous_item_id` FK.
6. **Compaction summary does NOT preserve thinking:** Compaction summaries are content generated by the LLM (not original thinking traces). The retrieval path retrieves original traces from `thinking_content`, not from summaries. See Decision Record D-08.
7. **Inline data migration uses `process_thinking()` from caller level:** The `db` module does not import from `chat` (dependency direction is `chat`→`db`). The `normalize_inline_thinking()` method on `Database` accepts a closure or the caller (`repl.rs`) iterates rows and calls `process_thinking()` directly. This avoids circular dependency.
8. **`ChatMessage.thinking` set manually (no builder):** `ollama-rs 0.3.4` has `ChatMessage { thinking: Option<String> }` but no `with_thinking()` method. We set the field directly, following the pattern in `custom_coordinator.rs:780-794`.
9. **Embedding consistency on content rewrite (R5):** When `normalize_inline_thinking()` rewrites `content` (removing inline `<thinking>` tags), existing embeddings become semantically stale. The UPDATE must also set `has_embedding = 0` and delete stale vec0 + chunk rows. The existing background embedding recovery pipeline (repl_tui.rs) then regenerates embeddings from the cleaned content. Without this, vector search returns stale results.
10. **No `/reindex` needed — use background recovery (R6):** Normalized items join the `has_embedding = 0` queue, which the background embedding recovery pipeline on startup already handles. No manual `/reindex --yes` is required. The pipeline runs as a `tokio::spawn` with ⚙ status bar indicator, so embedding regeneration does not block the TUI.
11. **Normalization runs in background spawn (R6):** The `normalize_inline_thinking()` call was initially synchronous in `init_chat_database()`, blocking TUI startup. Moved to the background `tokio::spawn` in `repl_tui.rs`, running before the embedding recovery step. Sub-second for typical DBs; the slow part (embedding regen) already runs in background.
12. **No progress bar for normalization (R7):** Normalization is sub-second for typical DBs. The ⚙ indicator from the subsequent embedding recovery provides visual feedback. If count > 0, a log message and chat system message inform the user. A dedicated progress bar would be over-engineering for this operation.
13. **Retroactive recovery is impossible:** Normal assistant messages never stored thinking in the DB (`strip_thinking_tags()` ran before insertion). No raw/original response is retained. Only pre-tool messages had inline `<thinking>` tags, recovered by `normalize_inline_thinking()`. This is documented as a known limitation.
14. **API-native `thinking` field must be respected (review fix D-14):** Both streaming and non-streaming paths use `extract_thinking()` — which checks `response.message.thinking` (API-native field from R1, Kimi) before falling back to regex-based `process_thinking()`. The initial implementation only used `process_thinking()` in the streaming path, silently dropping native thinking from compatible models.
15. **Atomic batch normalization (review fix D-15):** `normalize_inline_thinking()` wraps the per-row loop in `conn.execute_batch("BEGIN")` / `conn.execute_batch("COMMIT")`. If the process is interrupted mid-batch, SQLite auto-rollbacks the incomplete transaction. On next startup, the same rows still have `<thinking>` tags and normalization reruns (idempotent).

**Reference:** Arabzadeh et al. 2026, arXiv:2605.03344 — "RAG over Thinking Traces Can Improve Reasoning Tasks"

---

### 🔴 PRIORITY: Norm Correction in Embedding Tables — #157 [M1]

**Status:** ✅ COMPLETED
**Issue:** #157
**PR:** #184
**Branch:** `feat/norm-correction-and-threshold-validation`
**Depends on:** #133 (Embedding Diagnostics) ✅ COMPLETED
**Prerequisite of:** #153 (TAP-2 — thinking-aware retrieval)

**Goal:** Add norm correction to embedding tables to correct systematic cosine similarity underestimation when d_eff is low (Matryoshka 768→256). Applied as multiplicative correction at query time: `corrected_similarity = (1 - distance) * sqrt(nc_query * nc_result)`.

**Background:** TurboQuant (Zandieh et al., ICLR 2026) and RaBitQ (Gao & Long, SIGMOD 2024) show that scalar quantization introduces systematic underestimation of cosine similarity, amplified when effective dimensionality (d_eff) is low. This directly impacts TAP-2 (#153, thinking-aware retrieval), fact dedup, and all semantic retrieval.

**Implementation Summary:**
- Schema v12→v13: added `+norm_correction FLOAT` auxiliary column to all three vec0 tables (sqlite-vec supports INTEGER, FLOAT, TEXT, and BLOB auxiliary column types; using FLOAT avoids string↔float conversion overhead)
- `TruncateResult` struct in `embeddings/truncate.rs` carries both normalized vector and `norm_correction = 1/(|truncated_vec|²)`
- `embed()` and `embed_batch()` return `TruncateResult`; all DB insertion functions accept `norm_correction: f32`
- All semantic search functions (`search_content_semantic`, `search_facts_semantic`) read `norm_correction` from vec0 auxiliary columns and apply `sqrt(nc_query * nc_result)` correction
- `ContentSearchParams` and `search_messages_hybrid` accept `query_norm_correction: f32` parameter
- Migration v12→v13: DROP+re-CREATE vec0 tables, reset `has_embedding` flags (recovery pipeline regenerates embeddings with norm_correction)

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Schema migration v12→v13: add `+norm_correction FLOAT` to vec0 tables | ✅ |
| 2 | Calculate norm_correction on embedding insert (`1/(|truncated_vec|²)`) | ✅ |
| 3 | Apply norm correction in scoring (search, dedup) | ✅ |
| 4 | Enhance diagnostics report with norm correction awareness | ✅ |
| 5 | Add threshold validation recommendation to diagnostics (joint with #134) | ✅ |
| 6 | Tests: migration, norm calculation, threshold recommendation | ✅ |

**Effort:** ~1.5 days (20+ lines Rust, 1 SQL migration, diagnostics enhancement)

**Cross-refs:** R-25 (research-icebox.md), #133 (diagnostics), #153 (TAP-2)

---

### 🔴 PRIORITY: System Prompt Clarifications — #182 [M1]

**Status:** ✅ COMPLETED
**Issue:** #182
**PR:** #183
**No dependencies** — prompt-only changes, can be done in any wave

**Goal:** Fix three issues identified in the system prompt auto-diagnosis:

1. **Instruction Hierarchy** (P0-HIGH) — Without explicit priority, the model may prioritize SOUL.md behavioral defaults over USER FACTS constraints (e.g., "confirm before rm" vs "rm is not authorized")
2. **Language Note** (Medium) — Language convention only in SOUL.md, disappears with `--soulless`
3. **TOOL USAGE Reformulation** (Medium) — Generic 3-step process partially conflicts with SOUL.md's "Search first"
4. **Token Optimization** (Low) — Reduce TODO (10→3 lines) and Notes (30→8 lines) tool descriptions

**Design Decisions:**
- Instruction Hierarchy order: USER FACTS > SOUL > TOOL DESCRIPTIONS > BASE INSTRUCTIONS
- Language note in base prompt (not SOUL.md) so it persists with `--soulless`
- Architectural layers (SOUL/OPERATION/CAPABILITY) are intentional and will NOT be consolidated
- "Redundancy" in auto-diagnosis (Behavior 3x, Memory 3x, File Safety 2x) is intentional layering

**Implementation Phases:**

| Phase | Description | Effort | Priority | Status |
|-------|-------------|--------|----------|--------|
| A | Add `### INSTRUCTION HIERARCHY` to `SYSTEM_PROMPT_BASE` | ~10 lines | P0-HIGH | ✅ Done |
| B | Add `### LANGUAGE` note in prompt builder | ~5 lines | Medium | ✅ Done |
| C | Reformulate `### TOOL USAGE` — concise behavioral instruction | ~5 lines | Medium | ✅ Done |
| D | Reduce TODO and Notes tool descriptions | ~25 lines | Low | ✅ Done |

**Files to Modify:** `src/prompts/base.rs` (A, C), `src/prompts/builder.rs` (B), `src/prompts/tools.rs` (D)

**Cross-refs:** R-31 (research-icebox.md), #16 (TUI), #180 (MCP Client Phase 1)

---

### 🔴 PRIORITY 0: Fix 100% CPU During LLM Streaming — #193 [M1]

**Status:** ✅ COMPLETED
**Issue:** #193
**PR:** #194
**Branch:** `fix/193-cpu-spinlock-streaming`
**Depends on:** None (critical bug, bypasses wave order)

**Goal:** Fix the busy-wait spinlock that causes Sprachspiel to consume 100% CPU (one full core) during LLM message streaming.

**Problem Statement:**

The TUI event loop in `src/chat/repl_tui.rs` uses `event::poll(Duration::from_millis(0))` when the LLM is streaming (`has_llm_task = true`). This zero-duration poll returns `Ready(None)` instantly on every iteration, making the crossterm event branch of `tokio::select!` always ready. The Tokio runtime never parks the thread — the loop spins thousands of times per second, consuming an entire CPU core.

**Root Cause Analysis:**

```
loop {
  tokio::select! {
    crossterm_event = poll(0ms)  →  Ready(None) INSTANTLY
    llm_event = rx.recv().await  →  Pending (waiting for tokens)
    spinner = interval.tick()    →  Pending (waiting 120ms)
  }
  → match None {} → nothing happens
  → view.render() → terminal redraw (redundant during streaming)
  → loop restarts → poll(0ms) again
}
```

**Secondary Issue:** `view.render()` at line 473 is called on every loop iteration, including when no actual event was processed. During streaming, `stream_token()` and `stream_thinking()` already call `render()` per token, making the unconditional render redundant (thousands of redraws/sec with no new content).

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1 | Replace `Duration::from_millis(0)` with `Duration::from_millis(5)` | `src/chat/repl_tui.rs` | ✅ |
| 2 | Skip redundant `view.render()` when no event was processed during streaming | `src/chat/repl_tui.rs` | ✅ |
| 3 | Update comment explaining the trade-off | `src/chat/repl_tui.rs` | ✅ |
| 4 | Tests: verify CPU usage, verify Ctrl+C responsiveness | manual | ✅ (user confirmed) |

**Design Decisions:**

1. **5ms poll timeout (not 1ms):** 5ms reduces loop iterations to ~200/sec (CPU <5%) while keeping Ctrl+C latency imperceptible. 1ms also works but 5ms gives more headroom on slow machines.
2. **Skip render when no event during streaming:** `stream_token()` / `stream_thinking()` already render per token. The render at the end of the loop is for non-streaming state changes. Only render when: (a) a real crossterm event was processed, (b) an LLM event was processed, or (c) a spinner tick occurred.
3. **No new dependencies.** Pure timeout value + render guard change.

**Impact:**

| Metric | Before | After |
|--------|--------|-------|
| CPU during streaming | ~100% (1 core) | <5% |
| Ctrl+C latency | 0ms | ≤5ms (imperceptible) |
| Token streaming speed | Unaffected | Unaffected |
| Terminal redraws/sec during streaming | ~thousands | ~8 (tokens/sec) |

**Reference:** Issue #193

---

### 🔴 PRIORITY: Remove search_files tool — #214 [M1]

**Status:** 🔄 IN PROGRESS
**Issue:** #214
**Depends on:** None (independent bug fix)

**Goal:** Remove the unreliable `search_files` tool (6 root-cause bugs). The LLM uses `run_command("rg -n pattern path")` instead. `rg` (ripgrep) is added to the default external tools whitelist.

**Bugs Fixed by Removal:**
1. 100-file search cap (`MAX_RESULTS = 100`) — 80% of files in large projects never searched
2. `max_depth(5)` — nested directories silently skipped
3. 1MB file size cap — large files silently skipped, no warning
4. Binary files silently skipped via `read_to_string` failure
5. No output priority ordering — first 100 in walkdir order, not most relevant
6. Naive `glob_to_regex` — no `**` recursive globs, no char classes, no escape handling

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1 | Remove `search_files()`, `collect_files()`, `glob_to_regex()` from `files.rs` | `src/tools/files.rs` | ❌ NOT STARTED |
| 2 | Remove `walkdir` from Cargo.toml | `Cargo.toml` | ❌ NOT STARTED |
| 3 | Remove tool registration from `registry.rs` | `src/tools/registry.rs` | ❌ NOT STARTED |
| 4 | Update prompts (`tools.rs`, `builder.rs`) — add `run_command("rg ...")` guidance | `src/prompts/tools.rs`, `src/prompts/builder.rs` | ❌ NOT STARTED |
| 5 | Remove obsolete tests (glob_to_regex + regex pattern tests) | `src/tools/files.rs` | ❌ NOT STARTED |
| 6 | Add `rg` to `ExternalToolsConfig::with_defaults()` + install hints | `src/external/types.rs`, `src/external/config.rs` | ❌ NOT STARTED |
| 7 | Add `rg` to `generate_default_toml()` template | `src/external/config.rs` | ❌ NOT STARTED |
| 8 | Update SMOKE_TEST.md section 12.2 | `SMOKE_TEST.md` | ❌ NOT STARTED |
| 9 | Quality gates (fmt, clippy, test, dead code) | — | ❌ NOT STARTED |

**Design Decisions:**
1. **Remove, don't replace** — Instead of wrapping `rg` in a custom tool (Option A in the issue), remove `search_files` entirely. The LLM uses `run_command("rg -n pattern path")` which already has whitelist enforcement, Landlock sandbox, timeout, and head/tail. No wrapper code to maintain.
2. **#208 is a separate PR** — Issue #208 (permissive shell operators) would enable `rg pattern | head -20`, but is a security philosophy change with its own scope. Even without #208, `run_command("rg -n pattern path", "50", null, null)` works.
3. **`rg` in default whitelist** — Added to `ExternalToolsConfig::with_defaults()` so it works out-of-the-box without user configuration. Install hints for Arch/Debian/Fedora/Termux.
4. **`walkdir` removed** — Only used by `collect_files()` which is deleted. `regex` crate stays (used by `soul.rs`, `skills/sanitize.rs`, `chat/thinking.rs`, `files_blocklist.rs`).

---

### 🔴 PRIORITY: File Write Tools — Prompt Guidance, Uniqueness Check, and Result Format — #204 [M1]

**Status:** ❌ NOT STARTED
**Issue:** #204
**Depends on:** None (independent quick wins)

**Goal:** Fix the three most critical gaps in Sprachspiel's file write tools identified in the competitive benchmark against Hermes, Claude Code, and OpenCode:

1. **Write tools are invisible to the LLM** — `write_file`, `edit_file`, and `append_file` are not mentioned in the system prompt. The LLM must "discover" them from the tool list.
2. **`edit_file` has no uniqueness check** — `edit_replace()` silently replaces ALL occurrences of the search string. All competitors reject multi-match and ask for more context.
3. **Result format is opaque** — `"Successfully edited 'foo': 42 lines -> 45 lines (+3). Operation: replace"` gives no diff, no `+N/-M` breakdown.

**Motivation (Competitive Benchmark):**

- **Hermes**: 8-strategy fuzzy matching, uniqueness check, auto-lint delta, unified diff in output, "Did you mean?" hints on failure
- **Claude Code**: Exact matching with multi-match rejection, must-read-before-edit enforcement, mtime+content staleness check, structured patch output for UI
- **OpenCode**: Uniqueness enforcement, must-read-before-edit, mtime staleness check, `{diff, additions, removals}` metadata

Sprachspiel currently has: zero diff, zero fuzziness, zero uniqueness check, zero prompt guidance for write tools.

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1 | Add `### FILE WRITE TOOLS` section to system prompt with "read before edit" guidance, edit vs write preference, uniqueness explanation | `src/prompts/tools.rs` | ❌ NOT STARTED |
| 2 | Add uniqueness check in `edit_replace()`: if `search` appears >1x, reject with error showing line numbers of first 3 occurrences | `src/tools/files_write.rs` | ❌ NOT STARTED |
| 3 | Improve result format: `"+N/-M lines (X→Y). Operation: Z"` instead of `"X lines -> Y lines (+Z). Operation: W"` | `src/tools/files_write.rs` | ❌ NOT STARTED |
| 4 | Tests for uniqueness check (multi-match rejection, single-match pass, zero-match error) | `src/tools/files_write.rs` | ❌ NOT STARTED |

**Cross-refs:** R-34 (file I/O benchmark), #205 (file session state + staleness), #13, #50

---

### 🔴 PRIORITY: File Session State + Staleness Detection — #205 [M1]

**Status:** ❌ NOT STARTED
**Issue:** #205
**Supersedes:** #13 (File Session State) and #50 (Staleness Detection)
**Depends on:** #204 (Phase 1 — prompt guidance prepares the LLM for staleness errors)

**Goal:** Track which files have been read in the current session and detect when a file has been modified externally before editing. This prevents the LLM from operating on stale content.

**Architectural Context:**
- #118 (Tool Trait + `#[sprachspiel::tool]`) is CLOSED. The proc-macro generates unit structs, so tools are currently stateless.
- **Approach A (implement now):** `Lazy<Arc<Mutex<FileSessionState>>>` global state. Works because session state is inherently global, lock contention is minimal, and migration to Approach B is straightforward.
- **Approach B (post-#121):** State injection via tool struct fields. When Consumer Migration enables stateful tool structs, migrate `FILE_SESSION_STATE` to in-constructor injection.

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1 | Create `src/tools/file_state.rs` with `FileSessionState`, `ReadFileEntry`, `StaleReason`, `FILE_SESSION_STATE` global | New file | ❌ NOT STARTED |
| 2 | Add `record_read()` calls in `read_file`, `read_file_segment`, `search_files` | `src/tools/files.rs` | ❌ NOT STARTED |
| 3 | Add `record_edit()` calls and re-record-after-write in `write_file`, `edit_file`, `append_file` | `src/tools/files_write.rs` | ❌ NOT STARTED |
| 4 | Must-read-before-edit check: `has_been_read()` before `edit_file` and `write_file` (overwrite) | `src/tools/files_write.rs` | ❌ NOT STARTED |
| 5 | Staleness check: `check_stale()` before `edit_file` and `write_file` | `src/tools/files_write.rs` | ❌ NOT STARTED |
| 6 | Module export in `src/tools/mod.rs` | `src/tools/mod.rs` | ❌ NOT STARTED |
| 7 | Unit tests for `record_read`, `record_edit`, `check_stale`, `has_been_read` | `src/tools/file_state.rs` | ❌ NOT STARTED |

**Design Decisions:**

1. **mtime+size without hash.** Claude Code and OpenCode both use mtime-only or mtime+size. If mtime and size match, content is almost certainly the same. Hash (seahash) adds a dependency for minimal benefit. Can be added later as defense-in-depth.
2. **Must-read-before-edit for `edit_file` AND `write_file` (overwrite of existing file).** Claude Code enforces read-before-write too. Creating new files does not require a prior read.
3. **Re-record-after-write.** After a write, record the new mtime+size as the known state. Without this, the next edit of the same file fails with stale false-positive.
4. **Clear LLM error messages.** "File 'foo.rs' has not been read in this session. Use read_file first." and "File 'foo.rs' has been modified since it was last read. Re-read the file before editing."

**Cross-refs:** R-34 (file I/O benchmark), #204 (quick wins — prompt guidance prerequisite), #13, #50, #118 (Tool Trait — CLOSED)

---

### 🔴 PRIORITY: Unwrap/Expect/Panic Triage — #128 [M1]

**Status:** ✅ COMPLETED
**Issue:** #128
**Branch:** `refactor/unwrap-expect-panic-triage`

**Goal:** Audit all `unwrap()`, `expect()`, and `panic!` sites in production code and replace with explicit error handling (`?`, `map_err`, appropriate error types).

**Principle:** CLI entry points (main, command handlers) can keep `unwrap`/`expect` because the program should crash with a clear message. Internal library functions should propagate errors with `?`.

**Scope (Updated v0.43.0):**

| Category | Count | Approach |
|----------|-------|----------|
| `unwrap()` on Result | ~30 production + 385 test | Replace production with `?` or `map_err` |
| `unwrap()` on Option | ~12 production | Replace with `ok_or`/`ok_or_else` or pattern match |
| `expect()` on Result | ~10 production | Replace with `?` + context, or keep with justification |
| `expect()` on Option | ~2 production | Replace with `ok_or` + context |
| `panic!` in library code | 2 sites | Replace with `return Err(...)` |

**Note:** Most `unwrap()`/`expect()` calls are in `#[cfg(test)]` blocks — those are acceptable and will not be changed. Only production code paths are in scope.

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Mutex `lock().unwrap()` → `lock().expect("lock poisoned: ...")` (19 sites in todo.rs + command_handlers.rs) | ✅ Completed |
| 2 | `Regex::new().unwrap()` → `static_regex()` helper with `#[expect(clippy::expect_used)]` (14 sites) | ✅ Completed |
| 3 | `chunker.rs` 3 `.unwrap()` → `.expect()` with contract justification messages | ✅ Completed |
| 4 | Option `.unwrap()` → `.expect()` for dedup.rs, command_handlers.rs, todo.rs (3 sites) | ✅ Completed |
| 5 | `soul.rs` Result `.unwrap()` already covered by Phase 2 | ✅ Completed |
| 6 | `main.rs` `args.language.as_ref().unwrap()` → `.expect()` | ✅ Completed |
| 7 | `truncate.rs` 2 `panic!` → `#[expect(clippy::panic)]` with justification | ✅ Completed |
| 8 | Add `#[expect(clippy::expect_used)]` / `#[expect(clippy::unwrap_used)]` annotations to all 44 sites across 16 files | ✅ Completed |
| 9 | `command_handlers.rs` 5 remaining `.unwrap()` on mutex → `.expect()` + `#[expect]` | ✅ Completed |
| 10 | Fix unfulfilled `#[expect(clippy::unwrap_used)]` → `#[expect(clippy::expect_used)]` on `static_regex()` | ✅ Completed |
| 11 | Run `cargo clippy --lib -- -D warnings` — PASSING | ✅ Completed |
| 12 | Run `cargo test` — 951+ tests PASSING | ✅ Completed |

**Summary of changes across 16 files:**

- `src/tools/todo.rs`: 12 `#[expect(clippy::expect_used)]` on mutex locks and guard.get
- `src/chat/command_handlers.rs`: 9 `#[expect(clippy::expect_used)]` on mutex locks + guard.get, 5 `.unwrap()` → `.expect()`
- `src/chat/commands.rs`: 4 `#[expect(clippy::unwrap_used)]` on Option `.find()` after `.contains()` guard
- `src/embeddings/chunker.rs`: 3 `#[expect(clippy::expect_used)]` on Option `.next()` after boundary checks
- `src/skills/sanitize.rs`: 1 `#[expect(clippy::expect_used)]` on function, 1 on `.next()` after empty check
- `src/soul.rs`: 1 `#[expect(clippy::expect_used)]` on `static_regex()` function
- `src/chat/thinking.rs`: 1 `#[expect(clippy::expect_used)]` (pre-existing)
- `src/embeddings/truncate.rs`: 2 `#[expect(clippy::panic)]` (pre-existing)
- `src/spinner.rs`: 2 `#[expect(clippy::expect_used)]` on progress bar template strings
- `src/markdown.rs`: 2 `#[expect(clippy::expect_used)]` on OnceLock skin getters
- `src/tools/files_blocklist.rs`: 1 `#[expect(clippy::expect_used)]` on RegexSet default patterns
- `src/logging.rs`: 1 `#[expect(clippy::unwrap_used)]` on `/dev/null` fallback
- `src/embeddings/regenerate.rs`: 1 `#[expect(clippy::expect_used)]` on progress bar template
- `src/embeddings/recovery.rs`: 1 `#[expect(clippy::expect_used)]` on progress bar template
- `src/config.rs`: 1 `#[expect(clippy::unwrap_used)]` on HashMap::get for DEFAULT_MODEL
- `src/chat/input/rustyline.rs`: 1 `#[expect(clippy::expect_used)]` on Editor::with_config
- `src/facts/dedup.rs`: 1 `#[expect(clippy::expect_used)]` on Iterator::next() after is_empty guard
- `src/main.rs`: 1 `#[expect(clippy::expect_used)]` on args.language after validate()

**Related:** Issue #128

---

### 🔴 PRIORITY: Function Extraction — Reduce Long Functions — #129 [M1]

**Status:** ✅ COMPLETED — All 5 original targets addressed
**Issue:** #129
**Branch:** `refactor/function-extraction`
**PR:** #144

**Goal:** Reduce the 5 worst `too_many_lines` violations. Three functions were genuinely extracted (893→~95 lines, -89%). Two dispatch tables received `#[allow(clippy::too_many_lines)]` with justification — each arm is trivially linear routing/parsing, and wrappers would add ~100 lines of ceremony with no complexity reduction.

**Top 5 Targets Completed:**

| Lines Before | Lines After | File | Function | Strategy |
|-------------|-------------|------|----------|----------|
| 484 | 35 | `src/db/connection.rs` | `apply_migrations` → 10 extracted functions | Extraction |
| 409 | 40 | `src/prompts/tools.rs` | `build_tool_context` → 14 section functions | Extraction |
| 339 | ~20 | `src/facts/dedup.rs` | `deduplicate_and_insert` → dispatcher + layer functions | Extraction |
| 304 | 304 | `src/chat/command_handlers.rs` | `handle_command` — dispatch table | `#[allow]` + 7 handlers |
| 278 | 278 | `src/chat/commands.rs` | `parse_command` — command parsing table | `#[allow]` |

**Phase 1.1: `apply_migrations` (484→35 lines)**

Extracted 4 helper functions and 10 migration functions:
- `column_exists(conn, table, column) -> Result<bool>` — eliminates 15x repeated PRAGMA pattern
- `table_exists(conn, table) -> Result<bool>` — idempotent table check
- `add_column_if_missing(conn, table, column, col_type) -> Result<bool>` — conditional column add
- `add_columns_if_missing(conn, table, columns) -> Result<()>` — batch column add
- `migrate_v2_to_v3` through `migrate_v11_to_v12` — 10 independent, idempotent migration functions
- `apply_migrations` — thin dispatcher: `if from_version < N { Self::migrate_vN_to_VN+1(conn)?; }`

**Phase 1.2: `build_tool_context` (409→40 lines)**

Extracted 1 helper and 14 section functions:
- `filter_available(tools, blacklist) -> Vec<&str>` — deduplicates blacklist filtering
- `weather_section`, `pokemon_section`, `serper_search_section`, `ddg_search_section`, `calc_section`, `file_section`, `system_section`, `led_section`, `todo_section`, `notes_section`, `feedback_section`, `document_section`, `agent_section`, `external_section` — each returns `Option<String>`
- `build_tool_context` — thin dispatcher calling `if let Some(s) = section_fn(blacklist) { sections.push(s); }`

**Phase 1.3: `deduplicate_and_insert` (339→~20 lines)**

Extracted the dedup pipeline into types and layer functions:
- `DedupResult` enum (Inserted, ExactDuplicate, NormalizedDuplicate, SemanticDuplicate, Updated, Fts5Conflict, Error)
- `UpdateReason` enum (PreferenceOverride, PolarityContradiction, Fts5Contradiction)
- `DedupConfig` struct (source, generate_embedding flag)
- `DedupContext` struct (reduces 8-parameter sprawl across layer functions)
- `check_exact_match()` — Layer 1
- `check_normalized_match()` + `resolve_global_normalized()` — Layer 2
- `check_semantic_match()` + `resolve_semantic_results()` — Layer 3.5
- `check_fts5_conflicts()` + `resolve_global_fts5_conflicts()` + `resolve_project_fts5_conflicts()` — Layer 3
- `insert_and_return()` + `do_insert()` — insert helpers
- `deduplicate_and_insert()` — thin dispatcher (4 layer calls + insert fallback)

**Phase 1.4: `handle_command` (304 lines — dispatch table)**

Extracted 7 inline handler functions:
- `handle_quit()` — async, saves session + flushes embeddings before exit
- `handle_forget_cmd()` — confirmation check wrapper
- `handle_save_cmd()` — error display wrapper returning HandleResult
- `handle_load_cmd()` — error display wrapper returning HandleResult
- `handle_debug_toggle()` — debug mode toggle with status message
- `handle_skill_cmd()` — skill lookup + activation
- `handle_skill_list_cmd()` — list available skills

`handle_command` itself is a dispatch table where each arm calls a handler and returns HandleResult. Reducing below 100 lines would require ~30 wrapper functions adding ceremony without reducing complexity. Annotated with `#[allow(clippy::too_many_lines)]` with justification.

**Phase 1.5: `parse_command` (278 lines — command parsing table)**

`parse_command` is a command parsing match where each arm parses input strings into `ChatCommand` variants — inherently linear. Same `#[allow]` approach with justification. Also annotated `parse_note_add` (state-machine parser) and `parse_note_subcommand` (sub-command dispatch).

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1.1 | Extract `apply_migrations` sub-functions (484→35) | ✅ Completed |
| 1.2 | Extract `build_tool_context` section functions (409→40) | ✅ Completed |
| 1.3 | Extract `deduplicate_and_insert` layers into separate functions (339→20) | ✅ Completed |
| 1.4 | Extract `handle_command` inline handlers + `#[allow]` dispatch table (304→304) | ✅ Completed |
| 1.5 | `#[allow]` for `parse_command` command parsing table (278→278) | ✅ Completed |

**Commits:**
- `d401875` refactor: extract migration functions from run_migrations (484→35 lines)
- `ba3873b` refactor: extract tool sections from build_tool_context (409→40 lines)
- `fa76a72` fix(dedup): fix compilation errors from Phase 3.3 extraction
- `f78768f` refactor(command_handlers, commands): extract inline handlers and add #[allow] for dispatch tables

**Related:** Issue #129, PR #144

---

### ✅ PRIORITY 0: Factual Memory System (COMPLETED) [M1]

**Status:** ✅ COMPLETED

**Goal:** Enable sprachspiel to remember user preferences and project facts across sessions.

**Problem Statement:**
- Users must repeat contextual information every session (e.g., "my docs are in ~/docs")
- No persistent storage for facts about user/project
- AGENTS.md is static and project-level only
- LLM doesn't learn from interactions

**Solution:** Persistent fact storage with automatic decay, LLM-autonomous management, and intelligent conflict resolution.

**Documentation:** See [Factual Memory System Design](./doc/src/development/factual-memory-system.md) for complete design.

**Key Insight:** Factual Memory and Feedback System (PRIORITY 5) are **orthogonal** and **complementary**:
- Factual Memory → "What do I know about the user/project?"
- Feedback System → "How should I weight retrieved messages?"
- They operate at different layers and don't conflict.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│                    FACTUAL MEMORY SYSTEM                    │
│                    (SIMPLIFIED)                             │
├─────────────────────────────────────────────────────────────┤
│  Storage: SQLite (facts table + FTS5, same DB)             │
│  Scope: project (default) + global (override)               │
│  Categories: preference (180d), fact (30d)                  │
│  Classification: Heuristic only (no LLM)                   │
│  Search: FTS5 + Semantic (Layer 3.5, cosine ≥ 0.70)       │
│  Conflict Resolution: 6-layer dedup pipeline                │
│  Decay: Ebbinghaus curve with access reinforcement          │
│  Limits: 500 chars/fact, 2200 chars total in prompt         │
└─────────────────────────────────────────────────────────────┘
```

**Design Decisions:**
- **Only 2 categories:** `preference` (180d) and `fact` (30d). No `context` category (handled by RAG).
- **No embeddings:** FTS5 keyword search only, simpler and faster.
- **Heuristic classification:** No LLM for classification (pattern matching), LLM only for conflict resolution.
- **Hard limit:** 500 chars per fact (rejected at DB), 2200 chars total (truncated in prompt with Unicode-safe truncation).
- **Same DB:** Uses existing `embeddings.db`, no separate storage.

**Implementation Phases:**

| Phase | Description | Status | Effort |
|-------|-------------|--------|--------|
| 0.1 | Schema (facts table + FTS5, migration v5→v6) | ✅ DONE | 0.5 day |
| 0.2 | Core module (types, CRUD, decay) | ✅ DONE | 1 day |
| 0.3 | LLM tools (fact_add/search/remove) | ✅ DONE | 1 day |
| 0.4 | Prompt injection (## User Facts section) | ✅ DONE | 0.5 day |
| 0.5 | Decay startup + /fact prune command | ✅ DONE | 0.5 day |
| 0.6 | User commands (/fact add/list/remove/search) | ✅ DONE | 0.5 day |
| 0.7 | Conflict resolution (detect + resolve) | ✅ DONE | 0.5 day |
| 0.8 | Testing & documentation | ✅ DONE | 0.5 day |
| **Total** | | ✅ **COMPLETED** | **5 days** |

**Files to Create:**
- `src/facts/mod.rs` - Module exports
- `src/facts/types.rs` - Category, Scope, Source, Fact structs
- `src/facts/db.rs` - CRUD operations
- `src/facts/classify.rs` - Heuristic classification
- `src/facts/decay.rs` - Ebbinghaus decay calculations
- `src/facts/conflict.rs` - Conflict detection and resolution
- `src/facts/prompt.rs` - Build "## User Facts" section
- `src/tools/facts.rs` - LLM tools

**Files to Modify:**
- `src/db/schema.rs` - Add facts table (v6)
- `src/db/connection.rs` - Migration v5→v6
- `src/prompts/builder.rs` - Add `with_facts()`
- `src/chat/core.rs` - Load facts on session start
- `src/chat/repl.rs` - Add /fact command parsing
- `src/chat/command_handlers.rs` - Add /fact handlers
- `Cargo.toml` - Add `fact-tools` feature

**LLM Tools (autonomous):**

```rust
fact_add(content, scope?)   // LLM calls autonomously, auto-classified
fact_search(query, scope?)  // LLM searches facts (FTS5)
fact_remove(id)             // LLM removes incorrect facts
```

**User Commands:**

```
/fact add <text>            // Add project fact (auto-classified)
/fact add --global <text>   // Add global fact
/fact list                  // List all facts
/fact list --global         // List global facts only
/fact remove <id>           // Remove a fact
/fact search <query>        // Search facts
/fact prune                 // Manual decay run
```

**Related:** Issue #20

---

### ✅ PRIORITY 0: TODO System Activation (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.34.0)

**Goal:** Activate the existing TODO system to enable task tracking for both LLM and users.

**Problem Statement:**
- TODO system (`src/chat/todo_state.rs` and `src/tools/todo.rs`) was implemented but not integrated
- LLM tools registered but no synchronization with session state
- No user commands to manage TODOs interactively
- Tasks not persisted across sessions

**Solution:** Activate the TODO system with full integration.

**Implementation:**

| Component | Description | Status |
|-----------|-------------|--------|
| Tools sync | `load_from_session()` / `save_to_session()` functions | ✅ |
| User commands | `/todo add/list/update/clear-done/clear-all` | ✅ |
| Command handlers | `handle_todo_*` functions | ✅ |
| Prompt integration | `format_todos_for_prompt()` in system prompt | ✅ |
| Session persistence | Load/save todos with session in `repl.rs` | ✅ |

**Files Modified:**
- `src/tools/todo.rs` - Added `load_from_session()`, `save_to_session()`, `format_todos_for_prompt()`
- `src/chat/commands.rs` - Added `ChatCommand::TodoAdd/TodoList/TodoUpdate/TodoClearDone/TodoClearAll`
- `src/chat/command_handlers.rs` - Added `handle_todo_*` functions
- `src/chat/repl.rs` - Added command handling and session sync
- `src/prompts/builder.rs` - Added `todos` field to `PromptConfig`
- `src/chat/core.rs` - Added `todos_section` parameter to `build_session_system_prompt()`

**LLM Tools (already registered):**

```
todo_add(description)       // Add a new task
todo_list()                 // List all tasks
todo_update(id, status)     // Update task status
todo_clear_done()            // Clear completed tasks
todo_clear_all()             // Clear all tasks
```

**User Commands:**

```
/todo add <description>            // Add a new task
/todo list                          // List all tasks
/todo update <id> <status>          // Update task status (pending|in_progress|done)
/todo clear-done                    // Clear completed tasks
/todo clear-all                      // Clear all tasks
```

**Architecture:**

```
┌─────────────────────────────────────────┐
│           TODO SYSTEM FLOW              │
├─────────────────────────────────────────┤
│  Session Start                          │
│  └── load_from_session(session.todos)   │
│      └── Copies to global TODO_STATE    │
│                                         │
│  During Session                         │
│  ├── LLM calls todo_* tools            │
│  │   └── Operates on TODO_STATE        │
│  ├── User runs /todo commands          │
│  │   └── Operates on TODO_STATE        │
│  │   └── Syncs to session.todos       │
│  └── System prompt includes todos      │
│      └── format_todos_for_prompt()    │
│                                         │
│  Session End                            │
│  └── save_sqlite()                     │
│      └── session.todos.to_rows()      │
│          └── Database persistence      │
└─────────────────────────────────────────┘
```

**Estimated effort:** 0.5 day → **Actual:** 0.5 day

**Related:** Issue #25

---

### ✅ PRIORITY 1: Enhance Todo Tools — CRUD Gaps, Priority, and Tags (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Fix technical debt in todo tools by adding missing CRUD operations, priority levels, and tags/categories.

**Implementation Summary:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1.1 | Add `todo_get(id)` tool | ✅ Done |
| 1.2 | Add `todo_delete(id)` tool | ✅ Done |
| 1.3 | Add `todo_edit(id, description?, priority?, tags?)` tool | ✅ Done |
| 1.4 | Register new tools in registry | ✅ Done |
| 1.5 | Add tool descriptions to prompts | ✅ Done |
| 1.6 | Update slash commands and handlers | ✅ Done |
| 2.1 | Add `Priority` enum | ✅ Done |
| 2.2 | Add `tags: Vec<String>` to `Task` | ✅ Done |
| 2.3 | Extend `todo_add(description, priority?, tags?)` | ✅ Done |
| 2.4 | Extend `todo_edit(id, description?, priority?, tags?)` | ✅ Done |
| 2.5 | Extend `todo_list(filter?)` with filtering | ✅ Done |
| 2.6 | Extend `format_list_filtered()` for priority/tags | ✅ Done |
| 2.7 | DB migration v8→v9 for `priority` and `tags` columns | ✅ Done |
| 2.8 | Update `to_rows()`/`from_rows()` | ✅ Done |
| 2.9 | Update prompts and docs | ✅ Done |
| 2.10 | Manual tests | ✅ Done |
| 2.11 | Smoke test | ✅ Done (63/64 pass, 1 skipped) |
| 2.12 | Bug fix: error messages for /todo edit/get/delete without args | ✅ Done |
| 2.13 | Refactor: extract `parse_todo_subcommand`, remove YAGNI code | ✅ Done |

**Key files:** `src/chat/todo_state.rs`, `src/tools/todo.rs`, `src/db/connection.rs`, `src/db/operations.rs`, `src/db/schema.rs`, `src/tools/registry.rs`, `src/chat/commands.rs`, `src/chat/command_handlers.rs`, `src/prompts/tools.rs`

**Closes:** Issue #66 via PR #82

---

### ✅ PRIORITY 1: Code Quality - Prompts Centralization (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.33.0)

**Goal:** Centralize all prompts in `prompts/` module for maintainability.

**Problem:**
- Prompts for compaction and continuation were embedded in `core.rs`
- Difficult to find and modify prompts scattered across files
- Inconsistent prompt management

**Solution:** Move prompts to centralized location in `prompts/` module.

**Tasks Completed:**

| Task | File | Status |
|------|------|--------|
| Add `COMPACTION_PROMPT` constant | `prompts/base.rs` | ✅ |
| Add `CONTINUATION_PROMPT_TEMPLATE` constant | `prompts/base.rs` | ✅ |
| Create `build_compaction_prompt()` function | `prompts/builder.rs` | ✅ |
| Move `build_continuation_prompt()` | `prompts/builder.rs` | ✅ |
| Update exports in `prompts/mod.rs` | `prompts/mod.rs` | ✅ |
| Refactor `core.rs` to use centralized prompts | `chat/core.rs` | ✅ |

**Files Modified:**
- `src/prompts/base.rs` - Added `COMPACTION_PROMPT` and `CONTINUATION_PROMPT_TEMPLATE`
- `src/prompts/builder.rs` - Added `build_compaction_prompt()`, moved `build_continuation_prompt()`
- `src/prompts/mod.rs` - Updated exports
- `src/chat/core.rs` - Removed ~50 lines of prompt templates, now uses centralized functions

**Estimated effort:** 0.5 day → **Actual:** 0.5 day

**Related:** Issue #21

---

### ✅ PRIORITY 1: Code Quality - run_chat_repl Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.35.0)

**Goal:** Reduce cyclomatic complexity of `run_chat_repl` from 78/25 to <25/25.

**Context:** Phase 1 (Issue #7) completed the initial refactoring, extracting 600+ lines into separate modules. Issue #22 tracks follow-up improvements.

**Result:** Cognitive complexity reduced from 78/25 to **eliminated** (no Clippy warning for `run_chat_repl`).

**Implementation:**

| Phase | File | Task | Lines | Status |
|-------|------|------|-------|--------|
| 1 | `src/chat/continuation.rs` (NEW) | Create file with `ContinuationResult` struct | ~320 | ✅ |
| 2 | `src/chat/command_handlers.rs` | Add `handle_command_result()`, `handle_model_switch()`, `print_context_info()` | ~400 | ✅ |
| 3 | `src/chat/repl.rs` | Extract `create_session()`, `resolve_session_model()`, `resolve_thinking_mode()`, `init_database()`, `run_startup_tasks()`, `handle_user_message()` | ~300 | ✅ |
| 4 | Tests | `cargo test --all-features` | - | ✅ |
| 5 | Clippy | `cargo clippy --all-features -- -W clippy::cognitive_complexity` | - | ✅ |

**Files Modified:**
- `src/chat/repl.rs`: 1090 → 540 lines (~550 lines removed)
- `src/chat/command_handlers.rs`: Added dispatch functions
- `src/chat/continuation.rs`: NEW, continuation handling
- `src/chat/mod.rs`: Updated exports

**Commits:** Part of PR #28

**Related:** Issue #22, PR #28

---

### ✅ PRIORITY 4: Code Quality - query.rs Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.40.0)

**Goal:** Reduce cognitive complexity of `run_query` from 32/25 to <25/25.

**Context:** Non-interactive mode function (CLI query mode).

**Implementation:**

| Phase | Task | Status |
|-------|------|--------|
| 1 | Create `src/db/init.rs` | ✅ init_database_core() |
| 2 | Refactor `src/chat/repl.rs` | ✅ init_chat_database() |
| 3 | Create `src/query/mod.rs` | ✅ Module structure |
| 4 | Create `src/query/context.rs` | ✅ QueryContext + builder |
| 5 | Create `src/query/executor.rs` | ✅ execute_query_with_retry() |
| 6 | Create `src/query/coordinator.rs` | ✅ build_query_coordinator() |
| 7 | Refactor `src/query.rs` | ✅ run_query ~100 lines |
| 8 | Tests & Clippy | ✅ Clean, complexity <25/25 |

**Files Created:**
- `src/db/init.rs` - Core DB initialization (44 lines)
- `src/query/mod.rs` - Module exports, run_query (335 lines)
- `src/query/context.rs` - QueryContext struct (219 lines)
- `src/query/coordinator.rs` - Coordinator builder (55 lines)
- `src/query/executor.rs` - Execution with retry (119 lines)

**Files Modified:**
- `src/db/mod.rs` - Export init module
- `src/chat/repl.rs` - Use init_chat_database()

**Complexity Reduction:**
- Original: 516 lines in query.rs, cognitive complexity 32/25
- Final: ~100 lines in run_query, complexity below threshold (no longer flagged)
- Duplicate retry loop removed (lines 410-489 → single execute_retry_loop function)

**Commits:**
- `768bfb6` refactor: reduce query.rs cognitive complexity (Issue #29)

**Related:** Issue #29, PR #58

---

### ✅ PRIORITY 4: Code Quality - context_builder.rs Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED

**Goal:** Reduce cognitive complexity of `build_context` from 27/25 to <25/25.

**Context:** Retrieval context building function in `src/retrieval/context_builder.rs`.

**Analysis:**
- Function `build_context` (lines 180-378) had complexity 27/25
- Complexity sources:
  1. Nested `if let` in retrieval logic (4 levels deep)
  2. Repeated `match msg.role` blocks (same pattern twice)
  3. Multiple `if use_debug` scattered throughout

**Implementation:**

| Phase | Task | Status |
|-------|------|--------|
| 1 | Extract `push_messages_as_chat_messages()` helper + tests | ✅ Done |
| 2 | Extract `RetrievalResult` struct + `perform_retrieval()` | ✅ Done |
| 3 | Add `log_if_debug!` macro + refactor both functions | ✅ Done |
| 4 | Run tests and clippy, verify complexity < 25/25 | ✅ Done |

**Files Modified:**
- `src/retrieval/context_builder.rs` - Added helper functions, macro, tests

**Complexity Reduction:**
- Before: 27/25 (flagged by clippy)
- After: No clippy warning (complexity below threshold)

**Commits:**
- `c46d12c` refactor(context_builder): extract push_messages_as_chat_messages helper (Phase 1)
- `ed83e21` refactor(context_builder): extract perform_retrieval helper (Phase 2)
- `0abb06b` refactor(context_builder): add log_if_debug macro (Phase 3)

**Related:** Issue #30

---

### ✅ PRIORITY 4: Code Quality - registry.rs Complexity (COMPLETED) [M1]

**Status:** ✅ COMPLETED (Issue #31)

**Goal:** Reduce cognitive complexity of `register_tools` from 56/25 to <25/25.

**Context:** Tool registration function - largest complexity in codebase.

**Bugs Discovered During Analysis:**

| # | Bug | Description | Fix |
|---|-----|-------------|-----|
| B1 | `finance-tools` missing | `get_available_tool_names()` didn't include `get_stock_quote` | Added `finance-tools` block |
| B2 | `web_scrape` condition mismatch | Different `#[cfg]` conditions | Unified to `#[cfg(feature = "search-tools")]` |
| B3 | `test_tool` ignores blacklist | Always registered | Added blacklist check |

**Design Decision:** During review, we discovered that `todo-tools` was incorrectly feature-gated. Since `TodoState` is always part of `ChatSession`, todo tools should be built-in (like facts and notes).

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Create branch, update docs (with bugs) | ✅ Done |
| 2 | Fix bug B1: finance-tools in get_available_tool_names | ✅ Done |
| 3 | Fix bug B2: web_scrape condition | ✅ Done |
| 4 | Fix bug B3: test_tool blacklist check | ✅ Done |
| 5 | Extract 13 `register_*_tools()` helpers | ✅ Done |
| 6 | Extract 13 `get_*_tool_names()` helpers | ✅ Done |
| 7 | Refactor `register_tools()` | ✅ Done |
| 8 | Refactor `get_available_tool_names()` | ✅ Done |
| 9 | Run tests and clippy | ✅ Done |
| 10 | Make todo-tools built-in (remove feature gates) | ✅ Done |

**Complexity Reduction:**

| Function | Before | After |
|----------|--------|-------|
| `register_tools` | 56/25 | <25/25 (no warning) |
| `get_available_tool_names` | ~30/25 | <25/25 (no warning) |

**Files Modified:**
- `src/tools/registry.rs` - Extracted 26 helper functions, 2 macros, refactored main functions
- `src/tools/mod.rs` - Removed `todo-tools` feature gates
- `src/macros.rs` - Added `log_if_debug!` macro
- `src/retrieval/context_builder.rs` - Use shared macro
- `src/prompts/tools.rs` - Removed `todo-tools` feature gate
- `Cargo.toml` - Removed `todo-tools` from default and all-tools features

**Commits:**
- `f2884d7` docs: update CHANGELOG and IMPLEMENTATION with bug fixes for Issue #31
- `05c3639` refactor: reduce registry.rs cognitive complexity (Issue #31)
- `fcdcd9e` docs: mark Issue #31 as completed
- `7995956` docs: add Issue #63 to roadmap (notes tools missing)
- `4404bf9` fix: apply PR review feedback
- `3a86403` fix: make todo-tools built-in (remove feature gates)

**Related:** Issue #31, PR #62

---

### 🔵 PRIORITY 4: Code Quality - commands.rs Complexity (parse_command) [M1]

**Status:** ✅ COMPLETED (PR #84, ready for review)

**Goal:** Reduce cyclomatic complexity of `parse_command` from ~450 lines to manageable size, eliminate `CommandResult` enum duplication, and remove session subcommand duplication.

**Context:** `src/chat/commands.rs` (1919 lines). Five problems identified:

1. **Monolithic `parse_command`** — 44 match arms, ~645 lines of match code
2. **16 shortcut duplicates** — `/fa`, `/na`, `/di`, etc. copy 100% of parent subcommand logic (~135 lines)
3. **Two mirror enums** — `ChatCommand` and `CommandResult` with 23+ identical variants
4. **30 pass-through variants** in `execute_command` — no logic, just wrapping ChatCommand → CommandResult
5. **Session duplication** — `ChatCommand::Session` duplicates `New/Load/List/Save/Forget` (~151 lines)

**Implementation Phases:**

| Phase | Description | Lines Removed | Status |
|-------|-------------|---------------|--------|
| 1.1 | Extract `parse_fact_subcommand()` | ~70 (shortcut dedup) | ✅ Done |
| 1.2 | Extract `parse_note_subcommand()` | ~60 (shortcut dedup) | ✅ Done |
| 1.3 | Extract `parse_doc_subcommand()` | ~42 (shortcut dedup) | ✅ Done |
| 1.4 | Extract `parse_session_subcommand()` | ~13 (shortcut dedup) | ✅ Done |
| 1.5 | Consolidate 2-letter shortcuts as delegates | ~135 | ✅ Done |
| 1.6 | Add unit tests for extracted parsers | +490 (76 tests) | ✅ Done |
| 2 | Eliminate `CommandResult` enum, move execute logic to `command_handlers.rs` | ~321 | ✅ Done |
| 3 | Eliminate `SessionSubcommand` duplication | ~49 | ✅ Done |

**Estimated total reduction:** ~462 lines (1919 → ~1457)

**Files Modified:**
- `src/chat/commands.rs` — Extract parsers, delete `CommandResult`, delete `execute_command`, delete `SessionSubcommand`
- `src/chat/command_handlers.rs` — Absorb `execute_command` logic, create `handle_command()` using `ChatCommand`
- `src/chat/repl.rs` — Replace `execute_command + handle_command_result` with `handle_command`

**Branch:** `refactor/parse-command-complexity`
**PR:** #84 (ready for review)

**Commits:**
- `b5df9f0` docs: update CHANGELOG and IMPLEMENTATION.md for parse_command refactoring
- `e2b9e35` refactor: extract group parsers and consolidate 2-letter shortcuts
- `a5c2d80` refactor: eliminate CommandResult enum, add handle_command to command_handlers
- `bd8b927` refactor: eliminate SessionSubcommand enum and ChatCommand::Session variant
- `e226374` test: add unit tests for extracted subcommand parsers
- fix: remove /f shortcut from /forget, move to /search (collision causing data loss)
- fix: add missing /todo shortcuts (/tg, /te, /td, /tcd, /tca)

> **Note:** These shortcuts were later removed in PR #154 alias cleanup. Only canonical commands (`/todo add`, `/todo list`, etc.) remain.

**Bugs found during manual testing (fixed):**
- `/f` was mapped to `/forget` instead of `/search` — collision causing accidental data loss
- Missing `/todo` shortcuts for get, edit, delete, clear-done, clear-all

**Pre-existing bugs (NOT from PR, separate issues):**
- Session save/load persistence (1.3, 1.5) — `/session save` reports success but data not found by `/session list`
- FTS schema mismatch (1.7) — `content_fts` table missing `conversation_id` column (FIXED in PR #87)
- FOREIGN KEY constraint on todos — session save FK warning on todo mutations (FIXED in PR #87)

**Estimated effort:** 2-3 days

**Related:** Issue #35

---

### ✅ PRIORITY 5: UX - `/forget --yes` Confirmation [M1] (COMPLETED)

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Require explicit confirmation for `/forget` command to prevent accidental data loss. ✅ **COMPLETED**

**Problem:**
- `/forget` is the most destructive command — it deletes the entire conversation from the database
- Previously executed immediately with no confirmation
- A typo (e.g., `/forget` instead of `/forgets`) could destroy hours of conversation
- The `/f` shortcut was previously mapped to `/forget`, causing accidental data loss (fixed in PR #84)

**Implementation:**
- ✅ `/forget` without `--yes` → warn: "This will permanently delete this conversation. Use /forget --yes to confirm."
- ✅ `/forget --yes` → execute the forget operation
- ✅ No shortcuts for `/forget` (already enforced in PR #84)
- ✅ `ChatCommand::Forget` became `ChatCommand::Forget { confirmed: bool }`
- ✅ Parser validates `--yes` flag, rejects invalid arguments
- ✅ FK constraint bug in `save_sqlite()` fixed — `ensure_conversation_exists()` added

**Related:** Issue #85 (CLOSED via PR #87)

---

### ✅ PRIORITY 5: UX - `/skill <name>` Subcommand [M1] (COMPLETED)

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

### ✅ PRIORITY 5: Code Quality - Replace Debug Logs with `log` Crate + Verbosity System [M1]

**Status:** ✅ COMPLETED (v0.40.0)

**Goal:** Simplify verbosity system to 4 levels, remove debug mode, and integrate with REPL.

**Motivation:**
- **Simplified UX** - Most users only need 2 levels (normal and verbose)
- **Clearer semantics** - 4 levels are easier to understand than 5
- **Cleaner code** - Removed debug-specific logic and `debug_default` config

**Resolved Design Decisions:**

| Aspect | Old Design | New Design |
|--------|-----------|------------|
| Verbosity Levels | 5 (Quiet, Normal, Verbose, Debug, Trace) | 4 (Quiet, Normal, Verbose, Trace) |
| Normal Level | `warn` | `info` (shows tool calls) |
| Verbose Level | `info` | `debug` (shows tool calls + results) |
| Debug Level | `-vv` → `debug` | Removed (now verbose) |
| Trace Level | `-vvv` → `trace` | `-vv` → `trace` (replaced debug) |
| Debug Flag | `-d/--debug` (dry-run) | Removed |
| Verbose Flags | `-v`/`-vv`/`-q` | `-v` (verbose), `-vv` (trace) |
| Debug Toggle | `/debug` command | `/debug` command (Normal ↔ Trace) |
| `debug_default` | Config option | Removed |
| Rustyline Debug | Shown in normal mode | Always suppressed |
| Quiet Mode | Suppresses only warnings | Also suppresses spinners |
| `use_debug` Param | Passed to many functions | Removed from all functions |
| `Verbosity::Debug` | Exists | Removed |
| Future TUI | stderr logging | Logging to file instead |

| Verbosity | Flag | Log Level | Behavior |
|-----------|------|-----------|----------|
| Quiet | (none) | `error` | Only errors, no spinners |
| Normal | (default) | `info` | Tool calls visible + errors |
| Verbose | `-v` | `debug` | Tool calls + results + internal state |
| Trace | `-vv` | `trace` | Everything (including embedding distances, token budgets) |

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Simplify Verbosity enum (4 levels, remove Debug) | ✅ Completed |
| 2 | Update logging.rs - Verbosity struct with 4 variants | ✅ Completed |
| 3 | Update Rustyline input - Always suppress debug output | ✅ Completed |
| 4 | Update quiet mode - Spinners suppressed | ✅ Completed |
| 5 | Remove `debug_default` from config | ✅ Completed |
| 6 | Update `/debug` command - Toggle Normal ↔ Trace | ✅ Completed |
| 7 | Remove `use_debug` parameter from ALL functions | ✅ Completed |
| 8 | Update `/debug` DB error message (remove debug reference) | ✅ Completed |
| 9 | Remove `dbg!()` macro | ✅ Completed |
| 10 | Update tool call format - `🔧 name(args)` (no "Calling:") | ✅ Completed |
| 11 | Chat interactive mode ignores quiet flag | ✅ Completed |
| 12 | Tests & clippy & documentation | ✅ Completed |

**Files Created:**
- `src/logging.rs` — Logging initialization, Verbosity enum (4 levels), init(), set_verbosity(), 6 unit tests

**Files Modified:**
- `Cargo.toml` — Updated dependencies
- `src/main.rs` — Removed `-d/--debug` flag, simplified `-v`/`-vv` flags
- `src/lib.rs` — Added `pub mod logging`
- `src/chat/cli.rs` — Updated verbosity flags
- `src/chat/repl.rs` — Quiet mode handling, removed debug banners
- `src/chat/input/rustyline.rs` — Always suppress debug output
- `src/chat/command_handlers.rs` — `/debug` command syncs log level, not use_debug
- `src/db/connection.rs` — DB error message update (no debug reference)
- `src/settings.rs` — Removed `debug_default`, `debug_tools`, `verbosity` types updated

**Related Issues:**
- Issue #60 — Replace log_debug with log crate
- Issue #61 — Bug: `--debug` flag is dry-run mode, not debug logging
- Issue #87 — Simplify verbosity to 4 levels
- Issue #88 — Remove debug mode, update `/debug` command

---

### ✅ PRIORITY 4: Code Quality - Dead Code Cleanup (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.37.0)

**Goal:** Remove explicitly marked dead code and document justifications for retained `#[allow(dead_code)]` annotations.

**Context:** Codebase had 80 `#[allow(dead_code)]` annotations. Some are legitimate (future use, enum completeness, serde fields), but others are clearly dead code marked "no longer used".

**Removed (4 items):**

| File | Line | Code | Reason |
|------|------|------|--------|
| `src/context_overflow.rs` | 35 | `estimate_messages_tokens()` | Replaced by `estimate_chat_messages_tokens()` |
| `src/context_overflow.rs` | 60 | `MAX_TOOL_RESULT_TOKENS` | No longer used |
| `src/context_overflow.rs` | 64 | `CHARS_PER_TOKEN` | No longer used |
| `src/context_overflow.rs` | 69 | `truncate_tool_result()` | No longer used |

**Retained with Justification (~76 items):**
- Active use: `cosine_similarity()` (used by `facts::verify` for deduplication)
- Future use: `estimate_tokens_code()`
- Enum completeness: `ContextStatus` variants, `ResolutionAction::Add`
- Serde/API fields: Weather, Serper, Vision, OCR response structs
- Test-only: `Database::in_memory()`, test helper methods
- Feature-gated: LED methods (used with `led-tools` feature)

**Related:** Issue #37

---

### ✅ PRIORITY 4: Status Bar Above Prompt (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.37.2)

**Goal:** Add a dynamic status bar above the prompt input showing real-time context information.

**Implementation:**

| File | Changes |
|------|---------|
| `src/chat/view/mod.rs` | Added `StatusBarInfo` struct, `STATUS_BAR_LINES` constant, `format_status_bar()` method, visual truncation |
| `src/chat/repl_state.rs` | Added `get_status_bar_info()` method to ReplState |
| `src/chat/repl.rs` | Integrated status bar rendering before prompt, ANSI clear codes with terminal width detection, prompt `>>> ` |

**Features:**
- Model name, context usage (XX.XK/YYYK), progress bar with percentage
- Think/Tools indicators (🧠🔧) in status bar
- Colored progress bar: Green (< 50%), Yellow (50-75%), Red (> 75%)
- Fixed width (77 visual characters) to prevent overflow
- Clean prompt: `>>> ` (model and indicators moved to status bar)
- ANSI codes clear status bar and input lines based on terminal width
- Dynamic calculation using `calculate_context_metrics()`
- Unicode-aware width calculation using `unicode-width` crate

**Files Modified:**
- `src/chat/view/mod.rs` - `StatusBarInfo` struct with `format_status_bar()`, `truncate_visual()` helper
- `src/chat/repl_state.rs` - `get_status_bar_info()` method
- `src/chat/repl.rs` - `build_status_bar()`, `calculate_visual_lines()`, `build_clear_code()` helpers

**Technical Details:**
- Uses `termimad::terminal_size()` to detect terminal width
- Uses `unicode_width::UnicodeWidthStr` for proper character width (CJK, etc.)
- Calculates visual lines: `total_width.div_ceil(terminal_width).max(1)`
- Clears correct number of lines: 3 (status bar) + N (visual lines of input)
- Fallback to 1 line if terminal width unavailable

**Commits:**
- `8433736` docs: update CHANGELOG and IMPLEMENTATION for status bar feature
- `c20e2d1` feat: add status bar above prompt
- `a707f02` fix: correct spacing around separators in status bar
- `4bf6a78` fix: remove extra whitespace from status bar content line
- `fd7a28a` fix: use visual truncation for status bar content line
- `d288e50` fix: reduce status bar content width to 77 columns
- `3b51308` revert: remove status bar from spinner
- `5e03f46` feat: change prompt from '>' to '>>>'
- `921bd6f` docs: update CHANGELOG and IMPLEMENTATION with final status bar details
- `716fb50` feat: detect terminal width for ANSI clear codes

**Design Decision:**
Status bar during spinner ("Thinking...") was attempted but caused display issues with ANSI codes across different terminals. Reverted to simpler approach where status bar appears only above prompt.

**Known Limitations:**
- Emoji width may be imprecise (but user input typically doesn't contain emojis)
- Terminal width detection may fail in some environments (fallback to 1 line)
- Long input wrapping to many lines may still leave minor visual artifacts

**Related:** Issue #47

---

### 🔵 PRIORITY 4: Code Quality - Notes System (COMPLETED) [M1]

**Status:** ✅ COMPLETED

**Goal:** Persistent notes with semantic search.

**Features:**
- `/note add/list/show/edit/delete` commands
- Notes stored with embeddings for semantic search
- FTS5 full-text search for keyword matching
- Hybrid search (BM25 + vector) includes notes in results
- Project/global scope like facts

**Architecture Decision:** Unified `content_items` table (see below)

**Dependencies:** None

**Estimated effort:** 5 days

**Reference:** `doc/src/development/planning-session-cli-tools.md` lines 157-160, 303-311

**Related:** Issue #6, Issue #34

**Implementation Plan:**

| Phase | Description | Status |
|-------|-------------|--------|
| 0 | Fix TODO persistence bug (Issue #34) | ✅ Done |
| 1 | Schema v7 + migration (preserve data) | ✅ Done |
| 2 | Types + Operations (content module) | ✅ Done |
| 3 | Unified search operations | ✅ Done |
| 4 | Note commands | ✅ Done |
| 5 | Embeddings for notes | ✅ Done |
| 6 | Tests and documentation | ✅ Done |
| 7 | Embedding regeneration after migration | ✅ Done |

**Commits:**
- `be0b279` - docs: update roadmap priorities
- `9f2a50b` - docs: update CHANGELOG and IMPLEMENTATION.md
- `99c92e3` - fix(todo): sync TodoState to session after LLM interaction
- `34f3c12` - feat(db): add schema v7 with content_items unified table
- `0d66a05` - feat(content): add content module with Note CRUD operations
- `c88e324` - feat(content): add unified search operations for content_items
- `a416f42` - feat(notes): add /note commands for persistent notes
- `e5a8a57` - feat(content): add embedding support for notes
- `d2544bc` - test(content): add tests for note operations
- `9245699` - docs: update IMPLEMENTATION.md with completed phases
- `7cf2fbf` - docs: update IMPLEMENTATION.md - Notes System complete
- `b4b013b` - docs(chat): add /note commands documentation
- `5694cd9` - feat(remember): integrate notes into retrieval system
- `cf3abe1` - fix: fail fast on database initialization failure

**Migration Note (v6→v7):**

The schema migration from v6 to v7 includes a breaking change for embeddings:

1. **Removed broken embedding migration** - The attempt to migrate embeddings from `message_embeddings` to `content_embeddings` caused UNIQUE constraint errors when multiple messages had the same content.
2. **Embeddings are regenerated** - After migration completes, all embeddings are regenerated from source content with a progress bar.
3. **User data preserved** - Messages, notes, and facts are preserved. Only embeddings (derived data) are rebuilt.

**Critical Bugs Fixed During Unification:**

| Bug | Description | Fix |
|-----|-------------|-----|
| #12 | Migration dropped wrong table (`chunk_embeddings_v2` is V7, not V2) | Changed to `DROP TABLE IF EXISTS chunk_embeddings` |
| #13 | Items with chunks never marked `has_embedding=1` | Added marking logic after successful chunk embedding |
| #14 | `regenerate_all_embeddings()` deleted all chunks on startup | Removed chunk cleanup, only clean orphan chunks |
| #7 | Embedding context length exceeded (512 tokens vs 1024 chars) | Dynamic chunk sizing based on model context |
| #8 | Orphan chunks caused infinite recovery loops | Clean orphan chunks at startup |
| #42 | `note_add` panics with Unicode content | Use `truncate_chars()` for character-aware slicing |

**Dynamic Chunking Architecture:**

The embedding system now dynamically calculates chunk sizes based on the model's context length:

```rust
// src/embeddings/chunk_config.rs
pub struct DynamicChunkConfig {
    context_length: usize,      // From Ollama API (e.g., 512)
    chunk_percent: f32,         // 0.90 (90% of available context)
    overlap_percent: f32,       // 0.20 (20% overlap between chunks)
    prefix_margin: usize,       // 30 tokens for "search_document: "
    chars_per_token: f32,       // 3.0 (conservative for Portuguese/code)
}
```

**Key Parameters:**
- `chunk_percent`: 90% - Reserve 10% for tokenizer variance
- `overlap_percent`: 20% - RAG best practice for context preservation
- `prefix_margin`: 30 tokens - "search_document: " prefix (~20 tokens) + safety margin
- `chars_per_token`: 3.0 - Conservative for mixed Portuguese/code content

**Migration to v0.34.0:**

When upgrading from v6 to v7:
1. Backup your v6 database: `cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.v6`
2. Run the new version - migration happens automatically
3. All 283 messages will be migrated to `content_items`
4. Embeddings are regenerated (first startup takes ~2 minutes)
5. V2 tables (`messages`, `message_chunks`, etc.) are dropped

This ensures:
- No UNIQUE constraint failures during migration
- Clean embedding state after schema upgrade
- All search functionality works correctly after first startup
- Second startup is instant (0 items to regenerate)

**Bug #15: `/clear` Reloaded Old Messages from Database**

The `/clear` command was intended to "clear messages (preserves context for retrieval)" but:
- It only cleared `session.messages` in memory
- On session reload (`load_sqlite`), ALL messages from database were restored
- Sessions appeared to "undo" the clear after app restart

**Solution:**
- Renamed `/clear` to `/new` to better reflect behavior
- `/new` now generates a NEW `session.id` (e.g., `session-1712345678`)
- Old messages stay in database (searchable via `/search` and `remember()`)
- New session starts empty
- Added `count_all_content_items()` to check if database has searchable content

**Difference from `/forget`:**
| Command | Session ID | Database | Searchable |
|---------|-------------|----------|------------|
| `/new` | New | Preserved | Yes |
| `/forget` | New | Deleted | No |

---

### Architecture: Content Items (Schema v7)

**Unified table approach** for messages, notes, and future documents.

**Tables:**

```sql
-- Unified content storage
CREATE TABLE content_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type TEXT NOT NULL CHECK(content_type IN ('message', 'note', 'document')),
    
    -- Message fields (nullable)
    conversation_id TEXT,
    role TEXT CHECK(role IN ('user', 'assistant', 'system', 'tool')),
    message_type TEXT DEFAULT 'normal',
    previous_item_id INTEGER REFERENCES content_items(id),
    prompt_tokens INTEGER,
    
    -- Note/Document fields (nullable)
    scope TEXT CHECK(scope IN ('project', 'global')),
    source TEXT CHECK(source IN ('user', 'llm')),
    title TEXT,
    
    -- Common fields
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    project_id TEXT,
    has_embedding INTEGER DEFAULT 0
);

-- Unified chunks for long content
CREATE TABLE content_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES content_items(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    start_offset INTEGER NOT NULL,
    end_offset INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    has_embedding INTEGER DEFAULT 0
);

-- Unified embeddings (vec0)
CREATE VIRTUAL TABLE content_embeddings USING vec0(
    item_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT
);

CREATE VIRTUAL TABLE chunk_embeddings USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding FLOAT[256],
    +content_type TEXT,
    +conversation_id TEXT,
    +project_id TEXT
);

-- Unified FTS5
CREATE VIRTUAL TABLE content_fts USING fts5(
    content,
    content='content_items',
    content_rowid='id',
    tokenize='porter unicode61'
);
```

**Migration Strategy:**
1. Create new tables
2. Copy data from `messages` → `content_items`
3. Copy data from `message_chunks` → `content_chunks`
4. Copy embeddings from `message_embeddings` → `content_embeddings`
5. Copy embeddings from `chunk_embeddings` → `chunk_embeddings` (new table)
6. Populate FTS5
7. Keep old tables renamed as backup

---

### ✅ PRIORITY 3: Bug - Notes LLM Tools Missing [M1]

**Status:** ✅ COMPLETED

**Issue:** #63

**PR:** #64

**Summary:** Only `note_add` exists as LLM tool. LLM cannot edit or delete notes it creates.

**Design Decision:** Only `note_edit` and `note_delete` are needed. Other operations are covered by existing `remember` tool:
- `note_list` → `remember(query)` discovers notes
- `note_show` → `remember(id="note:N")` returns full note content
- `note_search` → `remember(query)` searches across notes, docs, messages

**Implementation:**
- Added `note_edit(id, title?, content?)` and `note_delete(id)` to `src/tools/notes.rs`
- Added `parse_note_id()` helper (accepts "42" and "note:42" formats)
- Registered tools in `src/tools/registry.rs`
- Updated prompts in `src/prompts/tools.rs`
- Commits: `c809a76`, `e847288`, `f795e4e`, `b98adf9`, `80a6acf`

**Also included in PR #64:**
- Braille art welcome banner (replaced jp2a ASCII art)
  - 14-line colored braille art from extended-mind.png (width 39)
  - Reordered session info by importance: Model, Server, Tools, Think, Vision, Sandbox, Project, Session, Version
  - Added Skills count line (shown when tools enabled and skills > 0)
  - Expanded `WelcomeInfo` from 7 to 13 fields (added skill_count)
  - Separate Facts/Notes/Docs count lines
  - "Ollama" label renamed to "Server"
  - Removed embed_model from banner
  - Added `count_facts()`, `count_notes()`, `count_documents()` to Database
- Fix: config.toml model settings in summarize/vision subcommands (Issue #65)

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `parse_note_id()` helper (accepts "42" or "note:42") | ✅ Done |
| 2 | Add `note_edit(id, title?, content?)` tool | ✅ Done |
| 3 | Add `note_delete(id)` tool | ✅ Done |
| 4 | Register tools in registry.rs | ✅ Done |
| 5 | Update prompts/tools.rs | ✅ Done |
| 6 | Build, test, clippy | ✅ Done |
| 7 | Braille art banner | ✅ Done |
| 8 | Fix config.toml model in summarize/vision | ✅ Done |

**Also included in PR #64:**
- Braille art welcome banner (replaced jp2a ASCII art)
- Fix: config.toml model settings in summarize/vision subcommands (Issue #65, commit `aa0744b`)

**Estimated effort:** 0.5-1 day

---

### ✅ Bug: summarize/vision ignoring config.toml model settings (COMPLETED)

**Status:** ✅ COMPLETED

**Issue:** #65

**PR:** #64

**Summary:** `summarize` and `vision` subcommands were falling back to hardcoded `qwen3.5:4b` instead of respecting `config.toml` model settings.

**Root Cause:** Both subcommands called `ModelConfig::default()` instead of `resolve_model_config()`.

**Fix:** Changed to use `resolve_model_config()` which reads from CLI flag → config.toml → hardcoded fallback.

**Commit:** `aa0744b`

---

### ✅ PRIORITY 3: Skills System (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.38.0)

**Goal:** Markdown-defined AI behaviors with progressive disclosure.

**Design Research (2026-03-24):**
- Analyzed Hermes Agent skills system at `~/.hermes/hermes-agent`
- Confirmed INDEX + on-demand loading pattern (not inject all skills)
- Confirmed `SKILL.md` format with YAML frontmatter
- Confirmed deduplication priority: project > user > builtin

**Architecture:**
```
System Prompt
├── SKILLS INDEX (names + descriptions)
│   └── <available_skills> section
└── Tools section

On-demand Loading:
├── LLM sees relevant skill in INDEX
├── LLM calls skill_view(name="document-processing")
└── System returns full SKILL.md content
```

**Features:**
- `skill_list()` tool - returns INDEX (names + descriptions)
- `skill_view(name)` tool - loads full skill content
- Builtin skills embedded in binary (`include_str!`)
- User skills at `~/.config/sprachspiel/skills/<name>/SKILL.md`
- Project skills at `.sprachspiel/skills/<name>/SKILL.md`

**Dependencies:** None (CLI Tools completed in v0.28.x)

**Implementation Phases:**

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | ✅ COMPLETED | Skills Module (types, loader, sanitize, mod) |
| 2 | ✅ COMPLETED | Builtin Skills (4 .md files) |
| 3 | ✅ COMPLETED | Skills Tools (skill_list, skill_view) |
| 4 | ✅ COMPLETED | Prompt Integration (INDEX section) |
| 5 | ✅ COMPLETED | Testing (clippy, tests pass) |
| 6 | ✅ COMPLETED | Skills Slash Commands (activate skills via /skill-name) |

### Phase 6: Skills Slash Commands

**Goal:** Allow users to activate skills via slash commands (`/document-processing`).

**Behavior:**
```
/document-processing                    → Loads skill, shows activation message
/document-processing extrair texto.pdf  → Loads skill + sends user message
/skill-list                             → Lists available skills
```

**Architecture:**
- Dynamic slash command detection based on available skills
- Skill content injected into session system prompt
- Skills activated for current session only

**Implementation:**

| File | Change |
|------|--------|
| `src/chat/commands.rs` | Add `ChatCommand::Skill { name }` and `CommandResult::Skill` |
| `src/chat/commands.rs` | Modify `parse_command()` to detect `/skill-name` dynamically |
| `src/chat/session.rs` | Add `active_skill: Option<Skill>` field |
| `src/prompts/builder.rs` | Inject active skill into system prompt |

**Estimated effort:** 2 hours

**Reference:** Hermes Agent `agent/skill_commands.py`

**Files Created:**
- `src/skills/mod.rs` - Public API
- `src/skills/types.rs` - Skill, SkillIndex, SkillSource, Frontmatter
- `src/skills/loader.rs` - YAML parsing, directory scanning, deduplication
- `src/skills/sanitize.rs` - Injection pattern detection, validation
- `src/skills/builtin/document-processing.md` - PDF and ePub extraction skill
- `src/skills/builtin/ocr-images.md` - OCR for images skill
- `src/skills/builtin/code-analysis.md` - Code analysis skill
- `src/skills/builtin/web-scraping.md` - Web scraping skill
- `src/tools/skill_tools.rs` - skill_list, skill_view tools

**Files Modified:**
- `src/prompts/builder.rs` - Added SKILLS INDEX section, active_skill field
- `src/main.rs` - Added skills module
- `src/tools/mod.rs` - Added skill_tools module
- `src/tools/registry.rs` - Registered skills tools
- `src/Cargo.toml` - Added serde_yaml, skills-tools feature
- `src/chat/commands.rs` - Added ChatCommand::Skill, CommandResult::Skill, parse detection
- `src/chat/session.rs` - Added ActiveSkill struct, active_skill field
- `src/chat/command_handlers.rs` - Added handle_skill_activated
- `src/chat/core.rs` - Wired active_skill into build_session_system_prompt

**Commits:**
- `74a25be` feat(skills): add skills module with types, loader, sanitize, and builtin skills
- `73ced3a` feat(skills): implement skill_list and skill_view tools with registry integration

**Reference:** `doc/src/development/skills-system-design.md`

**Related:** Issue #8

---

### ✅ PRIORITY 3: Document Import Tool (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.39.0)

**Goal:** Import documents for semantic search and retrieval.

**Dependencies:** Skills System ✅ COMPLETED (v0.38.0)

**Features:**
- **File Formats:** TXT, MD, ORG (builtin), PDF, EPUB (requires `skills-tools` feature)
- **File Size Limit:** 5MB for uploaded files; larger files rejected with helpful error
- **Chunking:** Same system as notes/messages (~512 tokens)
- **Scope:** Project-scoped by default, optional global scope
- **Commands:** `/doc import`, `/doc list`, `/doc show`, `/doc delete` (shortcuts: `/di`, `/dl`, `/ds`, `/dd`)
- **LLM Tool:** `import_document(path, scope?)` for autonomous import
- **Storage:** content_items table with ContentType::Document
- **Retrieval:** Integrated with `remember()` tool via hybrid search

**Feature Flag Dependencies:**
- `document-tools` feature enabled by default
- PDF/EPUB import requires `skills-tools` feature (also default)
- TXT/MD/ORG import works standalone (no skills dependency)
- Included in `all-tools` feature

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Database & Types (document.rs, db/schema.rs, migration v8) | ✅ Done |
| 2 | LLM Tool (tools/documents.rs) | ✅ Done |
| 3 | Commands (commands.rs, command_handlers.rs) | ✅ Done |
| 4 | Embeddings integration | ✅ Done |
| 5 | Tests | ✅ Done |
| 6 | Documentation | ✅ Done |

**Files Created:**
- `src/content/document.rs` - Document struct, FileType enum, detect_file_type(), extract_title(), MAX_DOCUMENT_SIZE constant
- `src/tools/documents.rs` - import_document() LLM tool

**Files Modified:**
- `src/content/mod.rs` - Export document module
- `src/content/db.rs` - Document CRUD operations (insert_document, get_document, list_documents, delete_document)
- `src/db/schema.rs` - Migration v8: added filename, file_type, word_count columns
- `src/db/connection.rs` - Migration v7→v8 for document columns
- `src/tools/mod.rs` - Add documents module (feature-gated)
- `src/tools/registry.rs` - Register import_document tool (feature-gated)
- `src/chat/commands.rs` - Added CommandResult variants and parsing for /doc commands
- `src/chat/command_handlers.rs` - Added handlers for document commands (feature-gated)
- `Cargo.toml` - Added `document-tools` feature flag (default, included in all-tools)

**Commits:**
- PR #53 - Full implementation

**Reference:** `doc/src/development/planning-session-cli-tools.md` lines 151-156, 287-302

**Related:** Issue #9

---

### ✅ PRIORITY 3: Embedding Fallback for Oversized Content (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.37.2)

**Goal:** Handle content that exceeds embedding model's context window.

**Original Problem (v0.37.1):** When embedding fails due to context overflow, the old `embed_with_fallback()` returned `Vec<Vec<f32>>` (multiple embeddings), but callers tried to insert all of them with the same `chunk_id`, causing PRIMARY KEY constraint violations.

**Bugs Discovered:**

1. **PRIMARY KEY Violation:** `chunk_embeddings.chunk_id` is PRIMARY KEY, so only ONE embedding per chunk. Old code tried to insert multiple.

2. **`has_embedding` Marked Incorrectly:** Even when embeddings failed, `has_embedding` was set to 1, preventing recovery on next startup.

3. **Dangling Chunks:** Chunks created in memory but never persisted to database.

**New Design (v0.37.2):**

```
embed_chunk_with_fallback(ctx, db, client, context_length, division_count)
    │
    ├─► Try client.embed(content)
    │       │
    │       ├─► Success → db.update_content_chunk_embedding() → return Ok
    │       │
    │       └─► Error: ContextExceeded → FALLBACK
    │
    └─► FALLBACK:
            │
            ├─► Check MAX_FALLBACK_DIVISIONS (4) - panic if exceeded
            ├─► Check MAX_CHUNKS_PER_ITEM (64) - panic if exceeded
            ├─► Check MIN_CHUNK_TOKENS (32) - panic if below
            │
            ├─► Divide content with halved config
            │
            ├─► db.transaction() - ATOMIC
            │       ├─► UPDATE chunk 0 content (first chunk)
            │       └─► INSERT chunks 1..N (new chunks)
            │
            └─► For each chunk: embed_chunk_with_fallback() recursively
```

**Key Changes:**

| Old (v0.37.1) | New (v0.37.2) |
|---------------|---------------|
| `embed_with_fallback() -> Vec<Vec<f32>>` | `embed_chunk_with_fallback(ctx) -> Result<EmbedResult, FallbackError>` |
| Multiple embeddings, same chunk_id | Creates new chunks atomically |
| Caller manages embeddings | Function manages chunks + embeddings |
| Silent failures with `let _ = ...` | Panics on limit exceeded (configuration error) |
| No transaction protection | Atomic transactions for chunk creation |

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Create `src/embeddings/fallback.rs` module | ✅ Done |
| 2 | Add `EmbedContext`, `EmbedItemContext` structs | ✅ Done |
| 3 | Add `embed_chunk_with_fallback()` | ✅ Done |
| 4 | Add `embed_item_with_fallback()` | ✅ Done |
| 5 | Add protection constants | ✅ Done |
| 6 | Simplify `client.rs` - remove old `embed_with_fallback()` | ✅ Done |
| 7 | Update `session.rs` callers | ✅ Done |
| 8 | Update `regenerate.rs` callers | ✅ Done |
| 9 | Update `recovery.rs` callers | ✅ Done |
| 10 | Update `command_handlers.rs` callers | ✅ Done |
| 11 | Add tests for fallback module | ✅ Done |
| 12 | Update documentation | ✅ Done |

**New Files:**
- `src/embeddings/fallback.rs` - Complete fallback logic with atomic transactions

**Modified Files:**
- `src/embeddings/client.rs` - Simplified, made `DEFAULT_CONTEXT_LENGTH` public
- `src/embeddings/mod.rs` - Export new module
- `src/chat/session.rs` - Use new fallback functions
- `src/embeddings/regenerate.rs` - Use new fallback functions
- `src/embeddings/recovery.rs` - Use new fallback functions
- `src/chat/command_handlers.rs` - Use new fallback functions

**Protection Constants:**
```rust
const MAX_FALLBACK_DIVISIONS: usize = 4;   // 512→256→128→64→32
const MAX_CHUNKS_PER_ITEM: usize = 64;      // Prevent DB explosion
const MIN_CHUNK_TOKENS: usize = 32;         // Minimum before aborting
```

**Related:** Issue #40, PR #46

---

### 🟠 PRIORITY 2: Context Overflow During Multi-Tool Execution (COMPLETED)

**Status:** ✅ COMPLETED (v0.37.0)

**Goal:** Prevent context overflow when LLM calls multiple tools in sequence AND fix infinite compaction loop caused by oversized summaries.

**Problems:**

1. **Multi-Tool Overflow:** Auto-compaction only happens BEFORE the first message. When tools execute sequentially, results accumulate in history without token checks. Large tool outputs (file reads, command outputs) can overflow context during multi-tool chains.

2. **Compaction Loop (Critical Bug):** Compaction summaries had no size limit. With 368 messages being summarized, the LLM generated ~18,000 token summaries, causing immediate re-compaction in an infinite loop.

**Root Cause Analysis:**
- Trigger was too late (95%+ context usage)
- No buffer reserved before overflow
- Summary had no token limit, generating massive summaries
- Template was generic, not structured for context preservation

**Solution:** Three-layer protection with percentage-based thresholds:

**Layer 1: Percentage-Based Compaction Triggers**
```rust
// Scales with context window size (32K, 128K, 200K)
MODERATE_USAGE_PERCENT = 0.75  // Warning at 75% (8K remaining for 32K)
CRITICAL_USAGE_PERCENT = 0.88  // Auto-compact at 88% (4K remaining for 32K)
INTER_TOOL_USAGE_PERCENT = 0.94 // Inter-tool warning at 94% (2K remaining)
EMERGENCY_USAGE_PERCENT = 0.97  // Emergency truncation at 97% (1K remaining)

// Absolute minimums for small contexts:
PRE_TOOL_MIN = 2_000 tokens
COMPACTION_MIN = 1_000 tokens
INTER_TOOL_MIN = 512 tokens
EMERGENCY_MIN = 256 tokens
```

**Layer 2: Structured Summary with Hard Limit**
```rust
MAX_SUMMARY_TOKENS = 3_000 tokens
Template: Goal, Instructions, Progress, Discoveries, Relevant Files
Auto-truncate if LLM ignores limit
```

**Layer 3: Inter-Tool Protection** (from Phase 1 implementation)
```rust
MODERATE_USAGE = 75%  → Warning before first tool
CRITICAL_USAGE = 88%   → Auto-compact threshold
INTER_TOOL_USAGE = 94% → Warning during tool execution
EMERGENCY_USAGE = 97%  → Truncate result as last resort
```

**Critical Token Calculation Bugs Fixed (v0.37.0):**

Three separate double-counting bugs were discovered and fixed:

1. **`calculate_context_metrics()` double-counted system + tools**
   - Comments said `real_history_tokens` was "history only"
   - But it's actually the TOTAL from Ollama's `prompt_eval_count`
   - Function was adding system + tools again, causing double-count
   - Fix: Use total directly, derive history by subtraction

2. **`needs_inter_tool_compaction()` and related functions**
   - Received `history_tokens + system_tokens` and summed them again
   - Fix: Accept single `total_tokens` parameter

3. **Pre-tool warning showed wrong remaining tokens**
   - Used `context_window - history_real_tokens()` 
   - Missing system + tools in remaining calculation
   - Fix: Use `total_tokens` from `ContextStatus`

4. **Pre-tool warning said "Auto-compacting..." but didn't compact**
   - Logic showed warning at 75%, called `auto_compact_if_needed()`
   - But `auto_compact_if_needed()` only compacts at 88%
   - Fix: Split logic - warning at 75%, compact at 88%

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add inter-tool context check (80% threshold) | ✅ Done |
| 2 | Add emergency truncation (90% threshold) | ✅ Done |
| 3 | Add `needs_inter_tool_compaction()` function | ✅ Done |
| 4 | Add `truncate_to_budget()` for emergency truncation | ✅ Done |
| 5 | Add `ContextNearLimit` and `ContextTruncated` events | ✅ Done |
| 6 | Add tests for new functions | ✅ Done |
| 7 | Add percentage-based thresholds | ✅ Done |
| 8 | Restructure `COMPACTION_PROMPT` with structured template | ✅ Done |
| 9 | Add summary truncation in `compact_conversation()` | ✅ Done |
| 10 | Update `auto_compact_if_needed()` to use percentage thresholds | ✅ Done |
| 11 | Add `needs_buffered_compaction()` function | ✅ Done |
| 12 | Fix `calculate_context_metrics()` double-counting | ✅ Done |
| 13 | Fix `needs_inter_tool_compaction()` signature | ✅ Done |
| 14 | Fix pre-tool warning remaining calculation | ✅ Done |
| 15 | Split warning vs compact logic in continuation.rs | ✅ Done |
| 16 | Remove duplicate warning in core.rs | ✅ Done |

**Files Modified:**
- `src/context_overflow.rs` - Percentage thresholds, `calculate_thresholds()`, fixed function signatures
- `src/tokens.rs` - Fixed `calculate_context_metrics()` to not double-count
- `src/chat/continuation.rs` - Split warning/compact logic, fixed remaining calculation
- `src/chat/core.rs` - Removed duplicate warning when tools enabled
- `src/chat/custom_coordinator.rs` - Updated function calls for new signatures
- `src/prompts/base.rs` - Restructured `COMPACTION_PROMPT` with structured template
- `src/utils.rs` - Added `truncate_to_budget()` for emergency truncation
- `tests/context_tool_overflow.rs` - Updated for percentage-based thresholds
- `tests/context_recovery_flow.rs` - Updated for percentage-based thresholds

**Constants:**
- `MODERATE_USAGE_PERCENT = 0.75` - Warning threshold (75%)
- `CRITICAL_USAGE_PERCENT = 0.88` - Auto-compact threshold (88%)
- `INTER_TOOL_USAGE_PERCENT = 0.94` - Inter-tool warning (94%)
- `EMERGENCY_USAGE_PERCENT = 0.97` - Emergency truncation (97%)
- `PRE_TOOL_MIN = 2_000` - Minimum buffer for warning
- `COMPACTION_MIN = 1_000` - Minimum buffer for compaction
- `INTER_TOOL_MIN = 512` - Minimum buffer for inter-tool
- `EMERGENCY_MIN = 256` - Minimum buffer for emergency
- `RESPONSE_MARGIN = 2_000` - Tokens reserved for model response
- `MAX_SUMMARY_TOKENS = 3_000` - Hard limit on summary size
- `DEFAULT_OVERFLOW_THRESHOLD = 0.75` - For display purposes

**New Compaction Template:**
```markdown
## Goal
[1-2 sentences: What is the user trying to accomplish?]

## Instructions
- [Important user constraints and preferences, max 3 items]

## Progress
**Completed:** [Work done, max 5 items]
**Pending:** [Work remaining, max 3 items]

## Discoveries
[Key insights learned, max 3 items]

## Relevant Files
- [Files read/edited/concerned, max 5 items]
- Root path: [Project root if relevant]
```

**Flow Implemented:**
1. Before first message: Check if context needs compaction (trigger at `context - COMPACTION_BUFFER`)
2. Between tools: Check if context > 80% → emit `ContextNearLimit` event
3. Emergency: If context > 90% → truncate result → emit `ContextTruncated` event
4. After compaction: Generate summary with template, truncate if > MAX_SUMMARY_TOKENS

**Research Sources:**
- OpenCode compaction.ts: `COMPACTION_BUFFER = 20,000`, structured template
- LangChain: Token-based triggers, summary best practices
- sprachspiel context: Zettelkasten and learning focus (smaller buffer than code agents)

**Note for Future:** When implementing parallel tool execution, the nudge mechanism via `continuation_prompt` should be reviewed to handle multiple concurrent tool completions.

**v0.37.0 Addition - Inter-Tool Compaction:**

Automatic context compaction during multi-tool execution (implemented in PR #45):

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `ChatEvent::ContextNeedsCompaction` | ✅ Done |
| 2 | Add `needs_compaction` flag to `ContextCheckResult` | ✅ Done |
| 3 | Modify `process_response()` to stop tool execution on compaction needed | ✅ Done |
| 4 | Add `OverflowHandleResult` enum for error classification | ✅ Done |
| 5 | Add automatic continuation loop in `handle_user_message()` | ✅ Done |
| 6 | Add MAX_COMPACTION_CYCLES limit (3) | ✅ Done |

**New Files/Functions:**
- `src/chat/custom_coordinator.rs`: Added `ChatEvent::ContextNeedsCompaction`, error string format with `CONTEXT_NEEDS_COMPACT:` prefix
- `src/chat/continuation.rs`: Added `OverflowHandleResult`, `is_inter_tool_compaction_error()`, `parse_inter_tool_compaction_error()`, `handle_inter_tool_compaction_error()`, `build_inter_tool_compaction_prompt()`
- `src/prompts/base.rs`: Added `CONTINUATION_PROMPT_INTER_TOOL` for continuation after compaction
- `src/chat/repl.rs`: Added `MAX_COMPACTION_CYCLES` constant (module level)

**Flow:**
1. During multi-tool execution, check if `remaining < COMPACTION_BUFFER` after each tool
2. If true, emit `ContextNeedsCompaction` event and return error string with `CONTEXT_NEEDS_COMPACT:` prefix
3. `handle_overflow_error()` detects the error, returns `OverflowHandleResult::InterToolCompaction`
4. `handle_user_message()` detects `InterToolCompaction`, compacts, sends continuation prompt
5. LLM continues automatically (max 3 compaction cycles per message)

**Refactoring (v0.37.0):**
- Removed unused `CoordinatorError` enum (never used)
- Removed unused `CompactionStats` struct and `compaction_stats()` method
- Removed unused `_threshold` parameter from `check_context_overflow()`
- Removed unused `_system_prompt` and `_use_debug` parameters from `auto_compact_if_needed()`
- Simplified `check_and_handle_context_overflow()` signature (removed `_tool_name`)
- Moved `MAX_COMPACTION_CYCLES` to module level in `repl.rs`

**Related:** Issue #43

---

### 🟣 PRIORITY 5: Feedback Infrastructure [M1]

**Status:** ✅ COMPLETED (merged PR #98)
**Related Issue:** #23
**Detailed Plan:** [`doc/src/development/feedback-architecture.md`](./doc/src/development/feedback-architecture.md) — feedback-driven memory with active forgetting (architecture, formulas, and data model)

**Goal:** Implement a complete feedback-driven memory system: capture explicit feedback signals (Good/Bad/Correction) with decay-weighted RRF fusion for retrieval ranking, activate content item decay (ghost fields become functional), and connect feedback to forgetting speed. Feedback is harness-only (no fine-tuning) — signals affect RRF fusion scoring AND content importance/decay, not model weights.

**Key Insight:** Feedback improves *how we retrieve* past messages. Factual Memory provides *what we know* about the user. Both layers work together:

```
Context Assembly:
├── System Prompt
│   └── [FACTUAL MEMORY] ← "User prefers Portuguese"
│       "Docs are in ~/docs"
├── Retrieved Context (messages)
│   └── [FEEDBACK WEIGHT] ← Message #42: +1.0 (good, decayed)
│       Message #15: -1.0 (bad, decayed)
│       RRF multiplier: clamp(0.1, 3.0)
│   └── [CONTENT DECAY] ← Message #42: importance=0.55 (good feedback +0.05)
│       Message #15: importance=0.30 (bad feedback -0.1 → pruned sooner)
│       access_count: 12 (retrieved 12 times → reinforced)
└── Response
```

#### Architecture Decision Records (ADRs)

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-001 | Feedback is harness-only (no fine-tuning) | No GPU, no training pipeline. RAG/ICL/BoN are valid inference-time methods (Wu et al. 2025, Long et al. 2026). |
| ADR-002 | Decay formula: `2^(-t/half_life)` | Aligns with existing facts system (`src/facts/decay.rs`). `exp(-t/h)` is equivalent but confusing; `2^(-t/h)` matches Ebbinghaus curve already in code. |
| ADR-003 | Messages-only scope is Phase 1 (not permanent) | When Unified Knowledge Store ships, `feedback_signals.item_id` can reference `knowledge_items.id`. Migration: v10→messages, v11+→all sources. |
| ADR-004 | LLM self-feedback = 30% weight | Self-approval bias defense. Wu et al. (2025): self-verification consistently beaten by majority voting. Long et al. (2026): verification steps rarely change outcomes — predominantly confirmatory rechecks (arXiv:2602.03485). Configurable via `config.toml [feedback].llm_feedback_weight`. |
| ADR-005 | Good=+1.0, Bad=-1.0, Correction=+1.0 | Binary-like symmetric signals (no partial credit). Drori et al. (2025): strict 0/1 verification via Lean proofs and code execution (arXiv:2502.09955). Granularity comes from temporal decay, not base_value. Correction value is in metadata text, not numerical weight. |
| ADR-006 | Score clamping: `.clamp(0.1, 3.0)` | Original `.max(-0.9).min(2.0)` allowed negative scores (bug: `1.0 + (-2.0) = -1.0 → max(-1.0, -0.9) = -0.9`). New clamp: min 0.1 (90% max suppression), max 3.0 (3× amplification cap). |
| ADR-008 | Content Decay Activation | `content_items` ghost fields activated: `decay_score`/`access_count`/`last_accessed` now functional with Ebbinghaus decay. Content-type half-lives: messages=90d, notes=60d, documents=120d. Feedback adjusts importance (good +0.05, bad -0.1), creating a forgetting loop. `decay_score` is persisted by `run_content_decay_cycle()`, enabling accurate "items at risk" queries in `/context`. |
| ADR-009 | Retrieval Reinforces Retention | `on_content_access()` called on retrieval — increments `access_count`, updates `last_accessed`. Same pattern as facts system. RRF (immediate ranking) and access_count (future retention) are separate signals — not double-counting. |

#### Key Corrections from Original Plan

| Item | Original (implementation-directive.md) | Corrected (v2 plan) | ADR |
|------|---------------------------------------|---------------------|-----|
| Bad base_value | -0.5 | **-1.0** | ADR-005 |
| Correction base_value | 1.2 | **1.0** | ADR-005 |
| Decay formula | `exp(-t/h)` | **`2^(-t/h)`** | ADR-002 |
| LLM feedback weight | 1.0 (same as user) | **0.3 (30% discount)** | ADR-004 |
| RRF score clamping | `.max(-0.9).min(2.0)` | **`.clamp(0.1, 3.0)`** | ADR-006 |
| `/fc` shortcut | Present | **Removed** (correction always needs text) | — |

#### Key Corrections from V3

| Item | V3 | V4 | ADR |
|------|----|----|-----|
| "NO modification of content_items" | Explicit guardrail | REMOVED — feedback adjusts importance | ADR-008 |
| Content decay | Not addressed | Activated — all content_items decay | ADR-008 |
| access_count = 0 forever | Implicit limitation | Fixed — on_content_access() on retrieval | ADR-009 |
| Feedback → importance | Explicitly forbidden | Changed — good/bad adjusts importance | ADR-008 |

#### Implementation Phases

| Phase | Description | Effort | Key Correction | Status |
|-------|-------------|--------|----------------|--------|
| 1.1 | `/feedback` command + schema | 2 days | ADR-005 values; `/fc` removed | ✅ Done |
| 1.2 | Weight propagation | 1 day | — | ✅ Done |
| 1.3 | `/context` enhancement | 0.5 day | — | ✅ Done |
| 1.4 | Implicit signal capture | 1 day | — | ✅ Done |
| 1.5 | Weighted retrieval | 3 days | — | ✅ Done |
| 1.6 | Decay implementation | 1 day | `2^(-t/h)` + LLM 30% discount | ✅ Done |
| 1.7 | Content decay module | 2 days | ADR-008: Ebbinghaus for content_items | ✅ Done |
| 1.8 | Access tracking + importance adj. | 2 days | ADR-009: retrieval reinforces retention | ✅ Done |
| 1.9 | Decay cycle integration | 1 day | Startup trigger + /content prune | ✅ Done |
| **Total** | | **13.5 days** | | |

**Reserved Code (Phase 2):** The following functions in `src/feedback/prompt.rs` are implemented and tested but not yet wired into production. They are reserved for Phase 2 (Feedback-Aware Retrieval) and are documented with `#[allow(dead_code)] // Reserved for Phase 2`:

| Function | Purpose | Expected Use |
|----------|---------|-------------|
| `compute_feedback_boost_map()` | Struct-based version of boost computation using `Database` type directly | Phase 2 RRF fusion in `search_content_hybrid()` — will replace the direct `db::feedback_ops::compute_feedback_boost()` call |
| `build_feedback_section()` | Format feedback stats for `/context` display | Phase 2 `/context` enhancement — will replace inline formatting in `command_handlers.rs:1892-1916` |
| `build_decay_section()` | Format decay stats for `/context` display | Phase 2 `/context` enhancement — same as above |

**Boost Computation API Difference:** Two versions exist by design:
- `db::feedback_ops::compute_feedback_boost()` — DB-query-based, iterates rows directly. **Production (Phase 1).**
- `feedback::prompt::compute_feedback_boost_map()` → `feedback::decay::compute_total_boost()` → `decayed_weight()` — Struct-based, loads `FeedbackSignal` structs first. **Phase 2.** More composable when retrieval modules already have structs loaded.

Both use the same canonical decay formula via `feedback::decay::decayed_weight_raw()` (ADR-002).

Additionally, `src/feedback/decay.rs` provides the canonical decay computation:
- `decayed_weight_raw()` — Single point of calculation using unix timestamps with fractional-day precision
- `decayed_weight()` — Wrapper with `DateTime<Utc>` API (reserved for Phase 2)
- `compute_total_boost()` — Accumulates weights with first-stage clamping (reserved for Phase 2)

**Future Refactoring Note:** `facts/decay.rs` and `content/decay.rs` share an identical structural pattern (constants for half-lives, `compute_retention()`, `should_prune()`). A future refactoring could extract a shared `Decayable` trait or common `decay` module to eliminate this duplication.

**Sprach 2.0 Note:** The article's "Learned Personality" proposal (S2.5 — SOUL.md patching) overlaps with but extends P5. P5 captures *what happened* (feedback signals for retrieval weighting); S2.5 adjusts *who I am* (personality modification with human approval). Both are complementary.

---

### ✅ PRIORITY 2: Context Continuation (COMPLETED) [M1]

**Goal:** Enable LLM to gracefully pause reasoning when context fills up, then automatically continue after compaction.

**Problem Statement:**
- LLM can run out of context mid-task (complex multi-step operations)
- Current auto-compaction happens AFTER response completes
- No mechanism for LLM to signal "I need to pause and continue later"
- Lost work when context overflow occurs during tool execution

**Solution (Implemented):** Tag-based continuation protocol:
1. ✅ LLM receives context % in prompt (`with_context_status()`)
2. ✅ LLM instructed to emit `<continuation_needed>` tag when pausing
3. ✅ System detects tag, compacts, and injects continuation prompt via ephemeral
4. ✅ LLM continues without user intervention
5. ✅ Supports nested continuations (up to 3)

**Implementation Completed:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Context status in prompt | ✅ Done |
| 2 | Continuation tag parsing | ✅ Done |
| 3 | Ephemeral messages support | ✅ Done |
| 4 | Continuation loop in REPL | ✅ Done |
| 5 | Tests and documentation | ✅ Done |

**Key Components Implemented:**

1. **Context Status Section** (prompts/builder.rs)
   - `PromptConfig.context_status` field
   - Dynamic section showing usage % injected when >72%
   - Warning when critical: `⚠️ CRITICAL: Context window is nearly full`

2. **CONTEXT MANAGEMENT Instructions** (prompts/base.rs)
   - `CONTEXT_MANAGEMENT_INSTRUCTION` constant
   - Instructs LLM on pause protocol
   - Injected when context is overflow (>80%)

3. **ContinuationTag Parsing** (chat/custom_coordinator.rs)
   - `ContinuationTag` struct with `paused_at` and `next_step`
   - `parse_continuation_tag()` extracts and strips tag
   - Ignores tags inside code blocks

4. **Ephemeral Messages** (chat/custom_coordinator.rs)
   - `push_ephemeral()` for continuation prompts
   - Prepended to requests but never persisted

5. **Continuation Loop** (chat/repl.rs)
   - `build_continuation_prompt()` creates resume instructions
   - `send_message()` accepts optional continuation_tag
   - Automatic continuation after compaction
   - Token metrics accumulated across continuations

4. **Ephemeral Messages** (custom_coordinator.rs)
   - `ephemeral_messages: Vec<ChatMessage>` - not saved to history
   - `push_ephemeral()` - add temporary message
   - `take_ephemeral()` - retrieve and clear
   - Prepended to request before history

5. **Continuation Loop** (repl.rs)
   - Detect continuation tag in response
   - Compact context
   - Inject continuation prompt as ephemeral message
   - Continue generation loop
   - Auto-retry until no continuation needed

**Constants (Reused):**
- `DEFAULT_OVERFLOW_THRESHOLD: f32 = 0.8` - Critical (80%)
- `PRE_TOOL_THRESHOLD: f32 = 0.75` - Warning (75%)
- Warning at 72% (90% of 80%)

**Data Structures:**

```rust
pub struct ContinuationTag {
    pub paused_at: String,   // Where reasoning stopped
    pub next_step: String,   // What was about to be done
}

// Note: Continuation is detected via SendMessageResult.continuation_needed field,
// not via ChatEvent. The ChatEvent enum only has PreToolContent, ToolCall, ToolResult.
```

**Edge Cases:**
- Tag embedded in code block → Should NOT be parsed
- Multiple tags → Parse first one only
- Empty tag content → Treat as no continuation
- Tag in pre-tool content → Parse and handle

**Testing:**
1. Unit tests for `parse_continuation_tag()`
2. Integration test for continuation loop
3. Edge case tests (empty, multiple, in code block)
4. Manual testing with simulated context pressure

**Dependencies:** None (all infrastructure exists)

**Estimated effort:** 1-2 days

---

### ✅ PRIORITY 1: PreToolContent Persistence & Context Enrichment (COMPLETED) [M1]

**Status:** ✅ COMPLETED (All phases done)

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Schema v5 (message_type, previous_message_id) | ✅ Done |
| 1 | insert_message_with_type() | ✅ Done |
| 1 | get_subsequent_assistant_messages() | ✅ Done |
| 1 | get_previous_message_id() | ✅ Done |
| 1 | enrich_with_context() for multiple messages | ✅ Done |
| 1 | SearchResult struct updated | ✅ Done |
| 1 | All queries updated with message_type | ✅ Done |
| 2 | PreToolContent struct | ✅ Done |
| 2 | CustomCoordinator accumulators | ✅ Done |
| 2 | take_pre_tool_content() | ✅ Done |
| 2 | process_response() accumulation | ✅ Done |
| 2 | SendMessageResult updated | ✅ Done |
| 3 | SavedMessage.message_type | ✅ Done |
| 3 | add_pre_tool_message() | ✅ Done |
| 3 | add_user_message() returns message_id | ✅ Done |
| 3 | update_message_previous_id() | ✅ Done |
| 3 | get_conversation_messages includes message_type | ✅ Done |
| 4 | format_retrieved_context() | ✅ Done |
| 4 | Prompts MEMORY TOOLS navigation section | ✅ Done |
| 4 | remember.rs shows message_type | ✅ Done |

**Key Files Modified:**
- `src/db/schema.rs` - Schema v5 definition
- `src/db/connection.rs` - Migration v4→5
- `src/db/operations.rs` - New methods, updated SearchResult
- `src/chat/session.rs` - SavedMessage.message_type, add_pre_tool_message()
- `src/chat/custom_coordinator.rs` - PreToolContent accumulation
- `src/chat/repl.rs` - PreToolContent extraction and saving
- `src/prompts/builder.rs` - MEMORY TOOLS navigation instructions
- `src/tools/remember.rs` - Shows subsequent_messages with type

**Commits:**
- `0f9a6d2 feat(db): add message_type and previous_message_id columns (schema v5)`
- `7b91c47 feat(chat): accumulate PreToolContent in CustomCoordinator`

### ✅ PRIORITY 1: SOUL.md - AI Personality System (COMPLETED) [M1]

**Status:** ✅ COMPLETED (v0.29.0)

**Implementation:**
- `src/soul.rs` - Module for loading and processing SOUL.md
- `src/prompts/base.rs` - Added `PERSONALITY_DEFAULT` fallback
- `src/prompts/builder.rs` - Integrated SOUL layer into prompt assembly
- `src/prompts/personality.rs` - REMOVED (Pepe personality)
- CLI flags: `--soulless` for `chat` and `query` commands
- Documentation: `doc/src/soul.md`

**Breaking Change:** Pepe personality removed. Users should create their own `~/.config/sprachspiel/SOUL.md` for custom personalities.

---

### 🔴 PRIORITY 2: File Write Tools (COMPLETED)

**Status:** ✅ COMPLETED (v0.32.0)

**Goal:** Enable LLM to create, edit, and append to files safely.

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | `write_file` tool | ✅ Done |
| 2 | `edit_file` tool | ✅ Done |
| 3 | `append_file` tool | ✅ Done |
| 4 | Blocklist module | ✅ Done |
| 5 | Integration into read tools | ✅ Done |
| 6 | Documentation | ✅ Done |

**Key Files Modified:**
- `src/tools/files_blocklist.rs` - Shared security module with blocked patterns
- `src/tools/files_write.rs` - Write operations module (write_file, edit_file, append_file)
- `src/tools/files.rs` - Added blocklist checks to read operations
- `src/tools/mod.rs` - Export new modules
- `src/tools/registry.rs` - Register new tools
- `src/external/types.rs` - Added `FileToolsConfig` struct
- `src/external/config.rs` - Added `FileToolsSection` for TOML parsing
- `src/external/mod.rs` - Export `FileToolsConfig`
- `Cargo.toml` - Added `tempfile = "3"` as dev dependency
- `doc/src/tools.md` - Documented all 8 file tools

**Commits:**
- `f0e9481 feat: add files_blocklist module with shared security logic`
- `e8dfabe feat: add write_file, edit_file, and append_file tools`
- `82fa9e5 feat: add file-tools config section for blocked patterns`
- `fed4a9e feat: integrate blocklist into read operations`
- `4a08cb0 fix: use strip_prefix instead of manual slicing in clippy`

**Security Model:**
- **Sandbox always enforced** for all file operations (cannot be disabled)
- **Blocked patterns** for sensitive files (`.env`, `secrets`, `.pem`, etc.)
- **5MB size limit** per operation
- **Atomic writes** (temp file + rename) to prevent corruption
- **UTF-8 validation** - reject binary content
- **`/tmp` and `/var/tmp`** allowed for tool interoperability

**Configuration:**
```toml
[file-tools]
max_file_size = 5242880  # 5MB
blocked_patterns = [".env.*", "*secret*", "*.pem"]
block_read = true   # Block reading sensitive files
block_list = false  # Allow listing (filenames visible)
# block_write is always true, not configurable
```

**Reference:** `doc/src/development/file-write-tools.md` - Full implementation plan

---

### ✅ PRIORITY 3: Code Quality - run_chat_repl Refactoring (COMPLETED) [M1]

**Status:** ✅ COMPLETED (PR #19 merged)

**Goal:** Refactor the oversized `run_chat_repl` function (~1100 lines) into smaller, testable units with abstractions for future TUI migration.

**Problem:**
- `run_chat_repl` is 1100+ lines and hard to maintain
- Complex command handling with 20+ branches
- Difficult to test individual command behaviors
- High cognitive load for code reviewers
- Tight coupling to rustyline (blocks future TUI migration)

**Solution:** Extract into layered architecture with traits for input/output abstraction.

### Architecture

```
Layer 0 (Base): input.rs (trait), view.rs (trait) - NO dependencies
Layer 1 (Session): session.rs, cli.rs
Layer 2 (Implementations): input/rustyline.rs, view/terminal.rs
Layer 3 (State): repl_state.rs
Layer 4 (Core): core.rs, command_handlers.rs
Layer 5 (Entry): repl.rs (coordinator)
```

### New Modules

| File | Purpose | Status |
|------|---------|--------|
| `src/chat/input/mod.rs` | `InputBackend` trait, `InputResult` | ✅ Done |
| `src/chat/input/rustyline.rs` | `RustylineInput` implementation | ✅ Done |
| `src/chat/view/mod.rs` | `ChatView` trait, `TokenMetrics`, `WelcomeInfo` | ✅ Done |
| `src/chat/view/terminal.rs` | `TerminalView` implementation | ✅ Done |
| `src/chat/repl_state.rs` | `ReplState` struct, `ReplStateBuilder` | ✅ Done |
| `src/chat/core.rs` | `send_message`, `compact_conversation`, etc. | ✅ Done |
| `src/chat/command_handlers.rs` | Command handlers using ReplState | ✅ Done |

### Implementation Phases

| Phase | Module | Description | Status |
|-------|--------|-------------|--------|
| 1 | `input/mod.rs` | `InputBackend` trait (empty, for TUI) | ✅ Done |
| 2 | `view/mod.rs` | `ChatView` trait (empty, for TUI) | ✅ Done |
| 3 | `repl_state.rs` | Consolidate state variables | ✅ Done |
| 4 | `input/rustyline.rs` | Implement RustylineInput | ✅ Done |
| 5 | `view/terminal.rs` | Implement TerminalView | ✅ Done |
| 6 | `core.rs` | Extract send_message, compact_conversation, etc. | ✅ Done |
| 7 | `command_handlers.rs` | Extract command handlers | ✅ Done |
| 8 | `repl.rs` | Refactor to use ReplState + abstractions | ✅ Done |
| 9 | Tests | Unit tests for refactored modules | ✅ Done |

### Phase Order Rationale

**Why Phase 8 comes before Phase 7:**

The async command handlers in `repl.rs` need ~8 parameters each (session, ollama, model_config, db, embedding_client, etc.). Extracting them now would require:

```rust
// Before Phase 8 - messy with many parameters
pub async fn handle_compact(
    ollama: &Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
) -> Result<(), String>
```

**After Phase 8**, we'll have `ReplState` populated in the REPL loop:

```rust
// After Phase 8 - clean single parameter
pub async fn handle_compact(state: &mut ReplState) -> Result<(), String>
```

`ReplState` (from Phase 3) already contains all the necessary fields. Phase 8 populates it in the REPL loop, then Phase 7 extracts handlers with clean signatures.

### Checkpoint 1 (2026-03-13)

**Completed:** Phases 1-5
- Input abstraction layer (`InputBackend` trait)
- Output abstraction layer (`ChatView` trait)
- RustylineInput implementation with history/completion
- TerminalView implementation with all output methods
- ReplState struct with builder pattern

### Checkpoint 2 (2026-03-13)

**Completed:** Phase 6
- Created `src/chat/core.rs` with:
  - `TokenMetrics` struct (moved from repl.rs)
  - `SendMessageResult` struct (moved from repl.rs)
  - `send_message()` async function
  - `setup_coordinator()` function
  - `prepare_messages()` async function
  - `process_chat_response()` function
  - `build_session_system_prompt()` function
  - `build_continuation_prompt()` function
  - `auto_compact_if_needed()` async function
  - `compact_conversation()` async function
- Removed ~600 lines of duplicated code from `repl.rs`
- Updated `view/mod.rs` to re-export `TokenMetrics` from core

**Next:** Phase 7 - Extract command handlers using `ReplState`

### Checkpoint 3 (2026-03-13)

**Completed:** ReplState extended + Phase order finalized
- Added `Settings` to `ReplState` struct and `ReplStateBuilder`
- Documented why Phase 8 comes before Phase 7 (ReplState enables cleaner handler extraction)
- `repl.rs` reduced from ~1916 to ~1359 lines (557 lines moved to core.rs)

### Checkpoint 4 (2026-03-14) - Phase 8 COMPLETE

**Completed:** Variable migration to ReplState
- Created `ReplState` at start of `run_chat_repl` (line 318)
- Migrated all variables to `state.*` references:
  - [x] `use_debug` → `state.use_debug`
  - [x] `cli_code` → `state.cli_code`
  - [x] `cli_soulless` → `state.cli_soulless`
  - [x] `agents_md` → `state.agents_md`
  - [x] `tools_active` → `state.tools_active`
  - [x] `capabilities` → `state.capabilities`
  - [x] `model_config` → `state.model_config`
  - [x] `current_model_name` → `state.current_model_name`
  - [x] `session` → `state.session.*` (fields and methods)
  - [x] `ollama` → `state.ollama`
  - [x] `db` → `state.db`
  - [x] `embedding_client` → `state.embedding_client`
  - [x] `settings` → `state.settings`

**Commits:**
- `7a9e3a3` - Add command_handlers.rs placeholder
- `06b1f8a` - Migrate use_debug, cli_code, cli_soulless, agents_md
- `0d80f57` - Migrate settings
- `c3f9c2f` - Migrate ollama, db, embedding_client
- `038039a` - Migrate tools_active
- `12b4dcf` - Migrate current_model_name
- `19ea48c` - Migrate model_config and capabilities
- `08d6101` - Migrate session

**Phase Order Rationale:**
- Phase 8 populates `ReplState` in the REPL loop
- Phase 7 then extracts handlers with clean 1-parameter signatures: `fn handle_xxx(state: &mut ReplState)`
- Without ReplState, handlers would need 8+ parameters each

**Current State:**
- Phases 1-8 complete
- `repl.rs` reduced from ~1916 to ~1080 lines
- Phase 7 COMPLETE (all handlers extracted)

### Checkpoint 5 (2026-03-14) - Phase 7 COMPLETE ✅

**Completed:** Handler extraction from repl.rs to command_handlers.rs
- [x] Phase 0: Fixed variable references (`session`, `ollama`, `db`, `agents_md` → `state.*`)
- [x] Phase 1: Simple handlers (think, tools, retrieval, debug, tool-output)
- [x] Phase 2: Sync handlers (undo)
- [x] Phase 3: Async handlers (search, restore, reindex)
- [x] Phase 4: Complex async handler (compact)
- [x] Phase 5: Most complex handler (retry)

**Handlers Extracted (11/11 - ALL COMPLETE):**
| Handler | Type | Status |
|---------|------|--------|
| `handle_think_toggled` | sync | ✅ |
| `handle_tools_toggled` | sync | ✅ |
| `handle_retrieval_toggled` | sync | ✅ |
| `handle_tool_output_changed` | sync | ✅ |
| `handle_debug_toggled` | sync | ✅ |
| `handle_undo` | sync | ✅ |
| `handle_search` | async | ✅ |
| `handle_restore` | sync | ✅ |
| `handle_reindex` | async | ✅ |
| `handle_compact` | async | ✅ |
| `handle_retry` | async | ✅ |

**File Size Reduction:**
- `repl.rs`: 1380 → 1080 lines (300 lines reduced, 22% reduction)
- `command_handlers.rs`: 48 → 424 lines (new functionality)

**Commits in this session:**
- `e37a6c2` - Complete ReplState migration in repl.rs loop
- `bdc5d5b` - Extract simple command handlers to command_handlers.rs
- `3758238` - Extract handle_undo to command_handlers.rs
- `4eb2f48` - Extract async handlers (search, restore, reindex)
- `96eb775` - Progress update - 9 handlers extracted
- `66510ee` - Extract handle_compact to command_handlers.rs
- `aa076eb` - Extract handle_retry to command_handlers.rs

**Phase 7 Complete!** All command handlers have been extracted with clean signatures.

### Checkpoint 6 (2026-03-14) - Phase 9 COMPLETE ✅

**Completed:** Unit tests for command handlers
- Added 10 unit tests for `command_handlers.rs`
- Tests cover: think_toggle, tools_toggle, retrieval_toggle, debug_toggle, tool_output_changed, undo
- All tests pass with `--all-features`
- Clippy passes with `-D warnings`

**Tests Added:**
| Test | Coverage |
|------|----------|
| `test_handle_think_toggled_unsupported` | Model doesn't support thinking |
| `test_handle_think_toggled_enabled` | Model supports thinking |
| `test_handle_tools_toggled_unsupported` | Model doesn't support tools |
| `test_handle_tools_toggled_supported` | Model supports tools, enable |
| `test_handle_tools_toggled_disables_when_false` | Disable tools |
| `test_handle_retrieval_toggled_enabled` | Enable retrieval |
| `test_handle_retrieval_toggled_disabled` | Disable retrieval |
| `test_handle_debug_toggled` | Toggle debug mode |
| `test_handle_tool_output_changed` | Change output level |
| `test_handle_undo_empty_session` | Undo with empty session |

**Quality Checks:**
- [x] `cargo build --all-features` - compiles without errors
- [x] `cargo clippy --all-features -- -D warnings` - no warnings
- [x] `cargo test --all-features` - 362 tests pass
- [x] Functional behavior unchanged (handlers extracted, not modified)

**Final File Sizes:**
| File | Before | After | Change |
|------|--------|-------|--------|
| `repl.rs` | 1380 lines | 1080 lines | -300 (22%) |
| `command_handlers.rs` | 48 lines | 490 lines | +442 (new) |

### TUI Preparation

This refactoring prepares for future `ratatui.rs` TUI:

- `InputBackend` trait enables swapping rustyline for TUI input widget
- `ChatView` trait enables swapping println for TUI rendering
- `ReplState` separates state from I/O layer
- `ChatCore` makes business logic reusable across UIs

See `doc/src/development/roadmap.md` - TUI section for future work.

**Benefits:**
- Each function under 200 lines
- Individual behaviors testable in isolation
- Clearer separation of concerns
- Input/output abstraction for TUI migration
- Easier code review for changes

**Estimate:** 16-24 hours → **Actual: 24h**

**Branch:** `refactor/run-chat-repl-decoupling`

**PR:** [#19](https://github.com/luksamuk/sprachspiel/pull/19) (MERGED)

**Issues:** [#7](https://github.com/luksamuk/sprachspiel/issues/7) (CLOSED), [#22](https://github.com/luksamuk/sprachspiel/issues/22) (OPEN - follow-up)

---

### ✅ PRIORITY 4: Specialized Agent Architecture [M1]

**Status:** ✅ COMPLETED (v0.41.0)

**Implementation:**
- Created `src/chat/subagent.rs` - `SubagentRunner` for one-shot execution with dual API path support
- Added dedicated spawn tools in `src/tools/subagent_tools.rs` - `spawn_ocr_agent`, `spawn_vision_agent`, `spawn_translate_agent`, `spawn_summarize_agent`
- Implemented chat commands: `/ocr`, `/vision`, `/translate`, `/summarize` in `src/chat/commands/`
- Refactored document extraction to use subagent architecture (Issue #9)
- Added config support for `[model.ocr]` and `[model.document]` in `src/config/models.rs`
- Created feature flag `subagent-tools` in `Cargo.toml`
- Updated `doc/src/CHANGELOG.md` with P4 release notes

**Key Files Modified:**
- `src/chat/subagent.rs` (new) - Core subagent execution engine
- `src/tools/subagent_tools.rs` (new) - Tools for LLM-initiated subagent spawning
- `src/chat/commands/mod.rs` - Added specialized command handlers
- `src/config/models.rs` - Added OCR and document model configuration
- `Cargo.toml` - Added `subagent-tools` feature flag

**Related Issues:** #9 (Document Import), #12 (OCR/Vision Integration)

**OCR Prompt Strategy (v0.42.0-dev):**

- Added `OcrMode::into_descriptive_prompt()` — restricted, mode-specific prompts for vision models
- Added `is_glm_ocr_model()` — model detection for prompt selection
- Added `parse_ocr_mode()` — convenience parser for LLM string parameters
- Added `prompt_override: Option<&str>` on `OcrProcessor::process_file()` and `process_batch()`
- Added `ocr_mode: OcrMode` field on `SubagentConfig` with builder method
- Added `ocr_mode: Option<String>` parameter on `spawn_ocr_agent()` tool
- Updated `/ocr` chat command to accept optional mode parameter
- Updated all 3 OCR entry points (CLI, chat, subagent) with model-aware prompt selection
- Removed dead `OCR_SYSTEM_PROMPT` constant
- YAGNI dead code removal: uses_chat_api(), tool_whitelist, with_tool_whitelist(), with_max_output_chars(), with_model_options(), SubagentRunner::settings, run_generate()
- Module-level #![allow(dead_code)] removed from security.rs and subagent.rs

**Commits:**
- `c25be97` feat(ocr): add model-aware prompt selection with descriptive prompts for vision models
- `4c8a81a` feat(subagent): propagate ocr_mode through subagent pipeline
- `6864b04` feat(chat): add mode parameter to /ocr command with model-aware prompts
- `0574c18` chore(ocr): remove dead OCR_SYSTEM_PROMPT constant

---

### 🟡 Parallel Tool Execution — #11 [M1]

**Status:** ❌ NOT STARTED  
**Depends on:** #121 (Consumer Migration — coordinator refactoring to use agnostic types)

**W2 Wave Context — Cancel token in parallel tool batches:**

Adopt the cancel-aware sleep pattern from #116 (`sleep_or_cancel` in `src/retry.rs`). When parallel tool execution is interrupted mid-batch, the cancel token MUST abort the in-flight tool calls and propagate cleanly to the retry loop in `core.rs`. The `classify_for_retry` infrastructure from #116 must be reused — parallel execution does not change the retry semantics, only the tool execution path.

**Goal:** Execute independent tool calls in parallel for faster response times.

**Problem:**
- Current implementation executes tool calls sequentially
- LLM often requests multiple independent tools (e.g., `get_weather` + `get_current_datetime`)
- Sequential execution unnecessarily increases latency
- User waits longer for responses with multiple tools

**Solution:** Detect independent tool calls and execute concurrently using `tokio::join!` or `futures::join_all`.

**Architecture:**

```rust
// Current: Sequential execution
for tool_call in tool_calls {
    let result = execute_tool(&tool_call).await;
    results.push(result);
}

// Proposed: Parallel execution
let futures: Vec<_> = tool_calls.iter()
    .map(|tc| execute_tool(tc))
    .collect();
let results = futures::future::join_all(futures).await;
```

**Dependencies Analysis:**
- Tools are independent if they don't modify shared state
- Read-only tools (weather, calc, search) can run in parallel
- Tools that modify state (file writes, database) need sequential execution

**Implementation Phases:**

| Phase | Task | Duration |
|-------|------|----------|
| 0.1 | Add `busy_timeout` to DB connection | 0.5h |
| 0.2 | Evaluate WAL mode and implement if viable | 0.5 day |
| 0.3 | Build DB concurrency integration test binary | 1 day |
| 1 | Identify which tools are safe for parallel execution | 0.5 day |
| 2 | Implement dependency analysis in CustomCoordinator | 1 day |
| 3 | Parallel execution with `join_all` | 1 day |
| 4 | Preserve sequential order for stateful tools | 0.5 day |
| 5 | Tests and benchmarks | 1 day |

**Phase 0: Database Concurrency Prerequisites**

Before parallel tool execution can work reliably, the SQLite connection must
handle concurrent access properly. Today the DB uses `Arc<Mutex<Connection>>`
which serializes all access, but this creates contention when background
operations (embedding generation, recovery) hold the lock for extended periods.

**Phase 0.1: Add `busy_timeout`** (1 line change in `init_connection()`)

```rust
conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
```

This makes SQLite wait up to 5 seconds when the database is locked, instead
of failing immediately with SQLITE_BUSY. Prevents the "unable to open database
file" error observed in smoke tests when embedding generation competes with
tool calls for the DB lock.

**Phase 0.2: Evaluate WAL mode** (design decision)

```rust
conn.execute_batch("PRAGMA journal_mode=WAL;")?;
```

WAL (Write-Ahead Logging) enables concurrent reads during writes. Without WAL,
`Arc<Mutex<Connection>>` serializes all access — even read-only tools wait.
With WAL, multiple read connections can operate while a write is in progress.

**Risks to evaluate:**
- WAL changes file behavior (requires testing: backup, recovery, cross-platform)
- WAL creates `-wal` and `-shm` files alongside the database
- WAL may have different performance characteristics on network filesystems
- Must verify compatibility with existing backup/restore procedures

**Phase 0.3: DB Concurrency Integration Test** (`tests/db_concurrency.rs`)

A separate integration test binary that simulates concurrent DB access
patterns without requiring a running LLM or Ollama server. Uses its own
temporary database file to avoid affecting user data.

**Test scenarios:**

| Scenario | Description | Contention Level |
|----------|-------------|------------------|
| A | Two lightweight reads (`note_show` + `note_list`) | None |
| B | Two reads with embedding (`remember(query=x)` + `remember(query=y)`) | Low |
| C | Heavy write + read (`import_document` + `remember(query)`) | High |
| D | Background embedding + read (send message + `remember()`) | High — the smoke test case |
| E | Auto-compact + read (after long conversation + `remember()`) | Medium |

**Binary design:**

```bash
# Run with default temporary database
cargo test --test db_concurrency

# Run with specific database (for manual testing)
cargo test --test db_concurrency -- --db-path /tmp/test_concurrency.db

# Run specific scenario
cargo test --test db_concurrency -- --scenario heavy_write_read
```

**Architecture of the test binary:**

```rust
// tests/db_concurrency.rs
// Simulates concurrent DB access patterns that occur during LLM tool calls

use std::sync::Arc;
use std::time::{Duration, Instant};

/// Spawns N concurrent DB operations and measures:
/// - Total time (should be near-max of individual ops, not sum)
/// - Whether any operation fails with SQLITE_BUSY or lock errors
/// - Whether data integrity is preserved under concurrent access
async fn run_concurrent_scenario(
    db: Arc<Database>,
    scenario: Scenario,
) -> ConcurrencyResult {
    let futures: Vec<_> = scenario.operations()
        .map(|op| tokio::spawn(async move { op.execute(&db).await }))
        .collect();

    let start = Instant::now();
    let results = futures::future::join_all(futures).await;
    let elapsed = start.elapsed();

    ConcurrencyResult {
        scenario: scenario.name(),
        total_time: elapsed,
        individual_times: results.iter().map(|r| r.time).collect(),
        errors: results.iter().filter(|r| r.is_err()).collect(),
        integrity_ok: verify_data_integrity(&db),
    }
}
```

**Key metric:** With proper concurrency support (WAL + busy_timeout), total
time for parallel reads should approach the max of individual operation times,
not the sum. Without WAL, the Mutex serializes everything and total time
approaches the sum.

**Current DB concurrency sources (existing background operations):**

1. **Embedding generation** — `tokio::spawn` in `session.rs` (lines 374, 502, 613)
   holds DB lock while writing embeddings after each user message
2. **Embedding recovery** — `recovery.rs` runs on startup, may hold lock during
   orphan cleanup and embedding generation
3. **Auto-compact** — `auto_compact_if_needed` runs after each LLM response,
   performs multiple DB reads and writes

**Tools that access the DB (potential parallel readers):**

| Tool | Access Type | Estimated Duration |
|------|-------------|-------------------|
| `remember(query=...)` | Read (semantic search) | ~100ms (with embedding) |
| `remember(id=...)` | Read (SELECT by ID) | <5ms |
| `note_show` / `note_list` | Read (SELECT) | <10ms |
| `note_edit` / `note_add` | Write (UPDATE/INSERT) | <5ms |
| `fact_remember` / `fact_recall` | Write/Read | <5ms |
| `import_document` | Write (INSERT + chunking) | ~100ms-2s |
| `/fact list` / `/doc list` | Read | <5ms |

**Safe for Parallel (read-only, no DB access):**
- `get_weather`, `get_current_datetime`
- `read_file`, `read_file_segment`, `count_lines`, `list_directory`, `search_files`
- `web_search`, `search_duckduckgo`
- `calculate`
- `get_pokemon_*` (all Pokemon tools)
- `get_system_info`

**Requires Sequential (stateful/write):**
- `run_command` (may have side effects)
- `write_file`, `edit_file`, `append_file` (when implemented)
- Database operations
- File writes

**Implementation note:** The read-only vs write classification above should be formalized in code (e.g., `ToolCategory::ReadOnly` / `ToolCategory::Stateful` enum) to enable the runtime parallel execution decision.

**Dependencies:** Phase 0 must be completed before Phase 1-5

**Estimated effort:** 5-6 days (including Phase 0)

**Related:** Issue #11

---

### Session Context Resume

**Status:** ✅ COMPLETED (v0.39.5)

**Goal:** Show a brief summary of recent messages when resuming a chat session, so the user can quickly remember what they were discussing.

**Problem Statement:**
When a user opens the chat and a previous session is loaded, they currently only see:
```
Resumed session: default (47 messages)
```
This provides no context about what was discussed. The user has to scroll up or issue `/search` to recall the conversation topic.

**Solution:** Display the last few user/assistant message exchanges automatically when a session is resumed. No LLM call needed — simply show the last N messages from the session's in-memory history.

**Design Decisions:**
- Show only User and Assistant messages (skip System and Tool messages)
- Show the last 3 exchanges (a "exchange" = one User message + its Assistant response)
- Truncate each message to ~80 characters for readability
- Use `format_role_label()` from `src/consts/roles.rs` for consistent role labels with emojis
- Only display when a session is resumed (not for new or anonymous sessions)
- Display after the welcome banner and resume message

**Example Output:**
```
Resumed session: default (47 messages)
Recent context (47 messages):
  👤 User: Can you check the auth middleware?
  🤖 Assistant: I found the issue - the token validation is checking expired tokens...
  👤 User: What about the refresh token logic?
  🤖 Assistant: The refresh logic looks fine, but the middleware needs to pass...
```

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `get_recent_exchanges()` to `ChatSession` | ✅ Done |
| 2 | Add `RecentContextInfo` struct and formatting to `view/mod.rs` | ✅ Done |
| 3 | Add `show_recent_context()` to `TerminalView` | ✅ Done |
| 4 | Integrate in `repl.rs` after resume message | ✅ Done |
| 5 | Make `truncate_str` pub(crate) in `view/mod.rs` | ✅ Done |
| 6 | Tests and verification | ✅ Done |

**Implementation:**
- Added `ChatSession::get_recent_exchanges()` method that walks messages forward, pairing User+Assistant into exchanges
- Added `RecentContextInfo` and `RecentMessage` structs in `view/mod.rs` with `format_context_summary()` method
- Made `truncate_str()` pub(crate) and added `MAX_CONTEXT_LINE_LENGTH` constant
- Added `TerminalView::show_recent_context()` method that extracts exchanges and formats them
- Wired up in `repl.rs` to call `show_recent_context()` after resume message
- Added 6 unit tests for `get_recent_exchanges()` and 3 tests for `RecentContextInfo`

**Related:** Issue #67

---

## 🟡 Configurable Embedding Model + Server-Side Matryoshka — #106 [M1]

**Status:** 📋 READY  
**Depends on:** None  
**Estimated effort:** 1 week (4 phases)  
**Issue:** #106

**Goal:** Make the embedding model configurable in `models.toml` and use Ollama's `dimensions` parameter for server-side Matryoshka truncation instead of client-side truncation.

**Prerequisite for:** #107 (Embedding Provider Abstraction) → #72 (Multi-Provider)

**Background:** Currently, the embedding model (`nomic-embed-text-v2-moe:latest`), dimensions (768→256), context length (512), and prefix (`"search_document: "`) are all hardcoded in `src/embeddings/client.rs` and `src/embeddings/truncate.rs`. Additionally, `truncate_and_normalize()` does client-side Matryoshka truncation, which is redundant since Ollama v0.11.11 (Sept 2025) supports the `dimensions` parameter on `/api/embed` for server-side truncation with L2 normalization.

### Current Hardcoded Constants

| Constant | Value | File |
|---|---|---|
| `DEFAULT_EMBEDDING_MODEL` | `nomic-embed-text-v2-moe:latest` | `client.rs:16` |
| `FULL_DIMENSIONS` | 768 | `truncate.rs:7` |
| `TRUNCATED_DIMENSIONS` | 256 | `truncate.rs:9` |
| `DEFAULT_CONTEXT_LENGTH` | 512 | `client.rs:21` |
| `"search_document: "` prefix | Hardcoded | `client.rs:214,266` |
| `EMBEDDING_PREFIX_TOKENS` | 30 | `client.rs:43` |
| DB vec0 tables | `FLOAT[256]` | `schema.rs:177,187`; `connection.rs:343,352` |

### Key Discovery: Ollama `dimensions` Parameter

Since Ollama v0.11.11 (Sept 2025), the `/api/embed` endpoint supports a `dimensions` parameter for server-side Matryoshka truncation. The parameter truncates the output embedding vector before L2 normalization. llama.cpp also supports this on its `/v1/embeddings` endpoint.

### Proposed Config (`models.toml`)

```toml
[embedding]
model = "nomic-embed-text-v2-moe:latest"
dimensions = 256        # Matryoshka truncated dims (via Ollama API "dimensions")
context_length = 8192   # Auto-detected from Ollama model info
prefix = "search_document: "  # Model-specific prefix, empty string if none
```

### Implementation Phases

| Phase | Description | Effort |
|-------|-------------|--------|
| 1. Config | Add `[embedding]` section to `Settings` / `config.toml`; replace hardcoded constants with config reads (defaults matching current behavior); auto-detect `context_length` from Ollama model info | 2-3 days |
| 2. Server-side truncation | Add `dimensions` field to Ollama embed API request; remove or bypass `truncate_and_normalize()` when `dimensions` is set; keep client-side truncation as fallback for older Ollama | 1-2 days |
| 3. DB migration | Migration that recreates `vec0` tables with dynamic `FLOAT[N]` from config; warn user and require reindex when dimensions change; `regenerate_all_embeddings()` already exists via `/reindex` | 2-3 days |
| 4. Validation | Test alternative models (nomic-embed-text v1.5, mxbai-embed-large, qwen3-embedding:0.6b); verify no regression with current model | 1-2 days |

### Matryoshka-Capable Embedding Models (Ollama)

| Model | Full Dims | Matryoshka → 256? | Context | Size | MTEB | Recommendation |
|---|---|---|---|---|---|---|
| nomic-embed-text-v2-moe | 768 | ✅ (64-768) | 8192 | 957MB | ~62 | Current default, multilingual |
| nomic-embed-text (v1.5) | 768 | ✅ (64-768) | 8192 | 274MB | 62.39 | English-only, lighter |
| mxbai-embed-large | 1024 | ✅ (64-1024) | 512 | 700MB | 64.68 | Best retrieval, short context |
| qwen3-embedding (0.6B) | 4096 | ✅ (32-4096) | 8192 | ~400MB | ~60 | Instruction-aware |
| qwen3-embedding (8B) | 4096 | ✅ (32-4096) | 8192 | ~5GB Q4 | 70.58 | SOTA quality |
| snowflake-arctic-embed2 | 1024 | ✅ (256) | 8192 | 1.2GB | 55.98 | Multilingual |
| embeddinggemma | 768 | ✅ (128-768) | 8192 | ~300MB | good/size | Google, no special prefix |

### Matryoshka-Capable Embedding Models (llama.cpp / OpenAI-compatible)

These models work with llama.cpp server's `/v1/embeddings` endpoint which also supports the `dimensions` parameter:

| Provider | Model | Full Dims | Matryoshka? | Context | Notes |
|---|---|---|---|---|---|
| OpenAI | text-embedding-3-small | 1536 | ✅ (512) | 8191 | $0.02/M tokens |
| OpenAI | text-embedding-3-large | 3072 | ✅ (256-3072) | 8191 | $0.13/M tokens |
| Any HF GGUF | nomic-embed-text-v1.5-GGUF | 768 | ✅ | 8192 | Can load custom fine-tunes |
| Any HF GGUF | bge-m3-GGUF | 1024 | ✅ | 8192 | Multilingual, dense+sparse+ColBERT |
| Any HF GGUF | snowflake-arctic-embed-m-GGUF | 768/1024 | ✅ | 8192 | Size variants 22M-335M |

### Validation Criteria

- [ ] `[embedding]` section in config.toml works
- [ ] Changing `model` triggers reindex prompt
- [ ] Changing `dimensions` triggers DB migration + reindex
- [ ] Server-side `dimensions` parameter used when available
- [ ] Client-side truncation still works as fallback
- [ ] No regression in search quality with nomic-embed-text-v2-moe (current model)
- [ ] At least one alternative model tested and validated

### Geometry-Aware Validation Criteria (from Embedding Geometry Audit)

These criteria extend the original validation with geometry metrics discovered in the embedding audit (d_eff=7, d̄=0.353, SPREAD system):

- [x] `sprach diagnostics` reports d_eff, average magnitude, threshold pass rate (#133) — PR #181 merged
- [ ] Fact semantic threshold decision is data-driven: measure recall@k at 0.70 and 0.80 before changing (#134)
- [ ] Alternative models are benchmarked by d_eff, retrieval quality, and multilingual support (#135)
- [ ] Default dimensions formula: `max(d_eff × 4, 64)` replaces hardcoded `FLOAT[256]` — via `embedding_models` registry table (#136, after #106/#135)
- [ ] RRF weights adapt to d_eff: d_eff≤10 → BM25=0.7/cosine=0.3; d_eff 11-25 → 0.5/0.5; d_eff>25 → 0.4/0.6 (#137)
- [ ] Documentation explains model selection criteria (d_eff, SPREAD system, multilingual support) alongside provider config (#138)

**Related:** Issue #106, Issue #107 (Embedding Provider Abstraction), Issue #72 (Multi-Provider), #133–#138 (Geometry sub-phases)

---

## 🟢 Embedding Geometry Audit Actions — #133–#138 [M1/W4]

**Status:** 📋 READY (sub-phases of W4)
**Issues:** #133–#138

**Background:** An embedding geometry audit revealed that the current nomic-embed-text-v2-moe model produces embeddings with **d_eff=7** (effective dimensionality out of 256 truncated dimensions) and **d̄=0.353** (mean cosine similarity across all directions). The audit identified that the SPREAD system (θ∈{0°, 30°, 60°, 90°}) operates at near-random similarity levels for θ≥60°, and BM25 silently compensates for poor vector discrimination via the hardcoded 0.4/0.6 RRF weights.

**Audit Reference:** Internal embedding geometry audit (see `doc/src/development/embedding-research.md` for findings)

**Key Findings:**

| Metric | Value | Impact |
|--------|-------|--------|
| `SEMANTIC_SEARCH_THRESHOLD` | 0.70 (hardcoded in `conflict.rs:230`) | May be too permissive for d_eff=7; facts accepted at near-random similarity |
| d_eff (effective dimensionality) | 7 out of 256 | Only 7 dimensions carry discriminative signal; 249 are noise |
| d̄ (mean cosine similarity) | 0.353 | Random pair similarity is 35%, high baseline noise |
| BM25 RRF weight | 0.4 (hardcoded in `search.rs`, `content/db.rs`) | BM25 silently compensates for poor vector discrimination |
| Cosine RRF weight | 0.6 (hardcoded) | Overweights vector similarity despite low d_eff |
| FLOAT[256] dimensions | Hardcoded in 9 schema locations | Should be `max(d_eff × 4, 64)` = 64, not 256 |

**Sub-Phases:**

| Phase | Issue | Description | Priority | Milestone |
|-------|-------|-------------|----------|-----------|
| W4.0 | #133 | `sprach diagnostics` — diagnose d_eff, d̄, regime, variance explained | High | M1 |
| W4.1 | #134 | ✅ COMPLETED Validate fact semantic threshold 0.70 vs 0.80 — PR #184 | High | M1 |
| W4.0b | #157 | ✅ COMPLETED Norm correction in embedding tables — PR #184 | High | M1 |
| W4.2 | #106 | Configurable embedding model + server-side Matryoshka | High | M1 |
| W4.3 | #135 | Benchmark alternative models (Nomic v2, Snowflake, mxbai, qwen3) with d_eff | High | M1 |
| W4.4 | #107 | Embedding provider abstraction — multi-provider support | High | M1 |
| W4.5 | #151 | T3-Phase0 — Preserve Thinking Content + continuation thinking fix | Critical | M1 |
| W4.6 | #138 | Documentation rewrite — model selection, hybrid search, provider docs | Medium | M1 |
| W4.7 | #136 | Geometry-Aware Embedding Configuration and Model Registry (depends on #106, #135) | Medium | M1 |

### Embedding Diagnostics Subcommand — #133 [M1/W4.0]

**Status:** ✅ COMPLETED (PR #181 merged)
**Issue:** #133
**Branch:** `feat/embedding-diagnostics`
**Depends on:** None
**Estimated effort:** 2-3 days

**Goal:** Add `sprach diagnostics` command that performs spectral analysis on stored embeddings, reporting d_eff, mean cosine distance (d̄), regime classification, and variance distribution. This is the W4.0 gateway card — foundational infrastructure for all subsequent W4 phases.

**Design Decisions:**

| Decision | Rationale |
|----------|-----------|
| Subcommand name `diagnostics` (alias `diag`) | Long form for clarity, short alias for convenience |
| Default: combine all 3 embedding sources | Matches issue spec; `--source` flag for granular analysis |
| Pure-Rust power iteration SVD (no new crate) | Avoids ~500KB binary increase + 15 deps; d_eff needs only top ~20 eigenvalues |
| `zerocopy::FromBytes` for BLOB deserialization | Already in Cargo.toml; consistent with write path using `IntoBytes::as_bytes()` |
| Output includes source breakdown in header | Even default (combined) mode shows "content: N, chunks: M, facts: K" |

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | New module `src/diagnostics/` + DB read functions + CLI subcommand | ✅ |
| 2 | Spectral analysis: d_eff, d̄, eigenvalues, regime classification | ✅ |
| 3 | Terminal display formatting + warnings + tests | ✅ |

**Files to Create:**

| File | Content |
|------|---------|
| `src/diagnostics/mod.rs` | Module root, re-exports |
| `src/diagnostics/embeddings.rs` | Spectral analysis: d_eff, d̄, eigenvalues, regime |
| `src/diagnostics/display.rs` | Terminal output formatting |

**Files to Modify:**

| File | Change |
|------|--------|
| `src/translate/cli.rs` | Add `Diagnostics(DiagArgs)` to `Commands` enum + `DiagArgs` struct |
| `src/main.rs` | Add `mod diagnostics;`, `Commands::Diagnostics` handler, `handle_diag()` |
| `src/content/db.rs` | Add `get_all_content_embedding_vectors()`, `get_all_chunk_embedding_vectors()` |
| `src/facts/db.rs` | Add `get_all_fact_embedding_vectors()` |

**Output Format (default — all sources combined):**

```
Embedding Diagnostics — nomic-embed-text-v2-moe
══════════════════════════════════════════
Vectors: 23 (content: 18, chunks: 2, facts: 3)
Nominal dimensions: 256
d_eff (participation ratio): 7.0 (2.74%)
Mean cosine distance (d̄): 0.353
Min/max cosine distance: 0.073 / 0.592

Regime Analysis:
  θ=0.70 → SPREAD (d̄ >= θ' = 0.30)
  θ=0.75 → SPREAD (d̄ >= θ' = 0.25)
  θ=0.80 → SPREAD (d̄ >= θ' = 0.20)
  θ=0.85 → SPREAD (d̄ >= θ' = 0.15)

Variance Explained:
  50% → PC #3
  90% → PC #10
  95% → PC #12
  99% → PC #15

⚠️  d_eff/25 ≈ 1 — vector search has minimal discriminative power.
    BM25 is silently compensating. Consider RRF weight adjustment.
```

**Output Format (`--source content`):**

```
Embedding Diagnostics — nomic-embed-text-v2-moe [content]
═════════════════════════════════════════════════
Vectors: 18
...
```

**CLI Syntax:**

```bash
sprach diagnostics                            # All sources combined
sprach diagnostics --source content           # content_embeddings only
sprach diagnostics --source chunks            # chunk_embeddings_v2 only
sprach diagnostics --source facts             # fact_embeddings only
sprach diag                                   # Shortcut (alias)
```

**Algorithms (no external dependencies):**

1. **d_eff (Participation Ratio):** `d_eff = (Σλᵢ)² / Σλᵢ²` — covariance matrix trace + power iteration for eigenvalues
2. **d̄ (Mean Cosine Distance):** Gram matrix `G = X·X^T`, `d̄ = 1 - mean(Gᵢⱼ)` for i≠j
3. **Regime Classification:** SPREAD if `d̄ ≥ (1 - θ)`, TIGHT otherwise
4. **Variance Explained:** Cumulative eigenvalue sum / total

**Warnings:**

| Condition | Message |
|-----------|---------|
| N < 100 | `⚠ Corpus is small (N=X). d_eff estimates are unreliable (max d_eff from PCA is N-1).` |
| d_eff/25 < 2 | `⚠ d_eff/25 ≈ N — vector search has minimal discriminative power.` |
| N = 0 | `No embeddings found in database. Run a chat session first.` |

**New DB methods (BLOB → Vec\<f32\> deserialization):**

```rust
// src/content/db.rs
pub fn get_all_content_embedding_vectors(&self) -> Result<Vec<Vec<f32>>>
pub fn get_all_chunk_embedding_vectors(&self) -> Result<Vec<Vec<f32>>>

// src/facts/db.rs
pub fn get_all_fact_embedding_vectors(&self) -> Result<Vec<Vec<f32>>>
```

**Key insight:** The codebase currently NEVER reads embedding vectors back from vec0 tables — only KNN distances are queried. These new methods are the first to perform bulk SELECT + BLOB deserialization.

**Bug Fix: Schema Migration Forward-Reference (discovered during DB upgrade testing)**

When opening a database at schema version ≤ 8, the `init_connection()` function executes `SCHEMA_SQL` before running incremental migrations. `SCHEMA_SQL` contained `CREATE INDEX IF NOT EXISTS idx_facts_embedding ON facts(has_embedding)` — but the `has_embedding` column is only added by `migrate_v10_to_v11()`. For databases at v8, this index creation failed with `no such column: has_embedding`, preventing the database from opening at all. The error was logged at `log::debug!` (silenced by default) and surfaced as a generic "DATABASE INITIALIZATION FAILED" with no actionable detail.

**Fix (4 files):**
- `src/db/schema.rs` — Removed `idx_facts_embedding` from `SCHEMA_SQL` (already created in `migrate_v10_to_v11()`)
- `src/db/init.rs` — Changed `log::debug!` → `log::error!`; replaced tuple return with `DatabaseInitResult` struct that includes the original error message
- `src/chat/repl.rs` — Use `DatabaseInitResult.error_detail` instead of generic message
- `src/query/context.rs` — Adapted to `DatabaseInitResult`

**Verified:** Database at `user_version=8` now migrates successfully to `user_version=12` with all columns, indexes, and vec0 tables intact.

**Bug Fix: TUI Hang During Startup Embedding Recovery (discovered after migration fix)**

After the migration v11→v12 fix, databases upgrading from v8 now have all `has_embedding` flags reset to 0, causing the startup embedding recovery pipeline to process 1700+ items. The pipeline (`regenerate_all_embeddings`, `recover_missing_embeddings`, `recover_missing_fact_embeddings`, `verify_and_dedup_facts`) ran synchronously via `.await` before the event loop in `run_chat_repl_tui()`, freezing the TUI for minutes. The `⚙ 0/1` indicator appeared but the prompt was unreachable.

**Fix:** Moved the entire embedding recovery pipeline to `tokio::spawn` in `src/chat/repl_tui.rs`. The TUI event loop starts immediately and is fully interactive. Progress is reported via the existing `EmbeddingProgressTx` channel (e.g., `⚙ 12/1741`). The indicator clears automatically when `poll_embedding_progress()` receives `current >= total`. Removed dead `clear_embedding_progress()` method from `App`.

**Verified:** TUI is interactive from the first frame; `⚙ N/M` indicator updates in real time during background embedding generation.

**Bug Fix: Application Blocks on Exit During Embedding Flush**

`/quit` and Ctrl+D called `flush_pending_embeddings()` and `flush_pending_fact_embeddings()` synchronously before exiting. After schema migration v11→v12 resets all `has_embedding` flags, this could block for minutes. The startup recovery pipeline already handles missing embeddings on next boot, making the exit flush redundant.

**Fix:** Removed the synchronous embedding flush from both exit paths (`handle_eof` and `handle_quit`). Exit is now instantaneous. Removed dead code: `flush_pending_embeddings()`, `flush_pending_fact_embeddings()`, `recover_missing_embeddings_with_progress()`, `clear_embedding_progress()`.

**Verified:** `/quit` exits immediately; next boot recovers pending embeddings via the background pipeline.

**Bug Fix: Embedding Progress Indicator Shows `current > total`**

The `⚙ N/M` indicator in the TUI status bar could show `processed` exceeding `total` (e.g., `⚙ 1800/1743`). Root causes:

1. **`regenerate_all_embeddings`**: `total = items.len() + chunks.len()` was calculated once at startup. When an item was split into N chunks, the original `total` only counted the item as 1 unit of work, but each chunk was processed independently. Similarly, `embed_item_with_fallback` could trigger recursive fallback chunking, creating more work.
2. **`recover_missing_embeddings`**: `total_missing = items.len()` only counted items, ignoring pre-existing chunks. When items were split into chunks, the total didn't grow. Skipped items (empty content, already-chunked) didn't increment `processed`, causing `processed < total` at completion.
3. **`recover_missing_fact_embeddings` and `verify_and_dedup_facts`**: Did not report progress via the `EmbeddingProgressTx` channel, leaving the indicator stale or invisible during these phases.

**Fix:**
1. `total` is now a mutable value that grows dynamically when items are split into chunks (`total += num_chunks - 1`).
2. Each chunk within a multi-chunk item increments `processed` individually (no longer relying on `progress.position()` which only counts `.inc()` calls).
3. Skipped items now increment `processed` so it always converges to `total`.
4. All four recovery functions (`regenerate_all_embeddings`, `recover_missing_embeddings`, `recover_missing_fact_embeddings`, `verify_and_dedup_facts`) now report progress via the `EmbeddingProgressTx` channel.

**Verified:** TUI shows `⚙ 50/1807` → `⚙ 161/1875` → ... → `⚙ 9205/11442` → indicator cleared. `current ≤ total` invariant holds throughout.

**Bug Fix: ANSI Escape Codes Appearing as Literal Text in TUI Error Messages**

Error messages like `✗ ␛[31mError:␛[0m Internal Server Error (ref: ...)` displayed raw ANSI escape codes instead of being rendered as colors. The TUI already applies red styling via `Span::styled(line, error_style())`, but `format_tool_error()` generated ANSI codes (e.g., `\x1B[31m` for red) when `is_plain_mode()` was false. In TUI mode, these codes appeared as garbled text.

**Root cause:** Double coloring — ratatui applies `error_style()` (bold red), and `format_error_with_ansi()` also wraps the text with `\x1B[31m...\x1B[0m`. The ANSI codes are not interpreted by ratatui widgets and appear as literal characters.

**Fix (2 layers):**
1. **Layer 1:** `format_error_with_status()` now uses `format_error_plain()` when `is_tui_mode()` is true, since the TUI renderer handles styling via `Span::styled()`.
2. **Layer 2 (defense-in-depth):** `show_error()`, `CommandOutput::Error`, `LlmEvent::Error`, and all other `ChatMessage::error()` call sites in `ratatui_view.rs` strip ANSI codes via `strip_ansi_codes()`. This catches ANSI from any source, not just `format_tool_error()`.

**Tests:** Added `test_format_error_tui_mode_no_ansi` and `test_format_tool_error_no_ansi_in_tui` to verify TUI mode produces no ANSI codes.

**Related:** Issue #133

**Bug Fix: Empty Assistant Messages from Ctrl+C Cancellation (Issue #185)** — ✅ COMPLETED

Three interrelated bugs discovered via production database investigation (12 items with `has_embedding = 0`):

1. **Empty assistant messages from Ctrl+C:** When the user pressed Ctrl+C during LLM streaming, `chat_stream()` in `custom_coordinator.rs` broke the streaming loop but returned `Ok(ChatMessageResponse)` with `full_content = ""`. This propagated as success through `process_send_result()` → `add_assistant_message("")`, persisting empty assistant messages. These messages: have no semantic value, confuse the LLM with empty turns, and can never receive embeddings (permanently stuck at `has_embedding = 0`). Evidence: 5 empty assistant messages (id 123, 232, 239, 278, 326) in production DB.

2. **Short content infinite recovery loop:** Items with `content.len() < 10` or `content.trim().is_empty()` were skipped by recovery/regenerate code but left with `has_embedding = 0`. On every startup, recovery queries `WHERE has_embedding = 0`, found these items, skipped them, and left them as `has_embedding = 0` — forever. Evidence: 7 short user messages ("Vai." at 4 chars, "Prossiga." at 9 chars × 6). Fix: filter by content length in recovery/reindex SQL queries (`AND length(content) >= 10 AND content != ''`), extract `MIN_EMBED_CONTENT_LEN` constant.

3. **No cleanup command:** Added `/gc` command for on-demand database garbage collection (empty messages, orphan chunks, orphan embeddings). Not automatic — user decides when to clean.

4. **Fact embedding regeneration on every startup:** `verify_and_dedup_facts()` called `generate_fact_embedding()` for ALL active facts on every startup, making N Ollama API calls even when all facts already had embeddings in the vec0 table. This caused the "indexing N facts" progress message on every boot. Fix: Verification now reads existing embeddings from DB via `get_all_fact_embedding_vectors()` and only generates new embeddings for facts with missing vec0 rows.

5. **vec0 re-embedding UNIQUE constraint failure:** `update_fact_embedding()`, `update_content_item_embedding()`, and `update_content_chunk_embedding()` used bare `INSERT INTO` for vec0 tables. If called for an entity that already had an embedding, the INSERT would fail with `UNIQUE constraint failed` because vec0 virtual tables use the entity ID as PRIMARY KEY and do not support `INSERT OR REPLACE`. Fix: All three methods now use `DELETE + INSERT` pattern.

**Implementation phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Prevent empty assistant messages: `add_assistant_message()` validation + `process_send_result()` skip | ✅ COMPLETED |
| 2 | Filter by content length in recovery/reindex queries + `MIN_EMBED_CONTENT_LEN` constant | ✅ COMPLETED |
| 3 | `/gc` command: ChatCommand::Gc, parser, handler, DB method, help text | ✅ COMPLETED |
| 4 | Read existing fact embeddings from DB instead of regenerating on every startup | ✅ COMPLETED |
| 5 | `DELETE + INSERT` pattern for all vec0 embedding update methods | ✅ COMPLETED |
| 6 | Orphan embedding cleanup in `/gc` (content, chunk, and fact embeddings) | ✅ COMPLETED |

### Threshold Validation — #134 [M1/W4.1]

**Status:** ✅ COMPLETED
**Issue:** #134
**PR:** #184
**Branch:** `feat/norm-correction-and-threshold-validation`
**Depends on:** #133 (Embedding Diagnostics) ✅ COMPLETED
**Joint PR with:** #157 (Norm Correction)

**Goal:** Data-driven validation of `SEMANTIC_SEARCH_THRESHOLD` (currently 0.70 in `src/facts/conflict.rs:230`) before potentially changing to 0.80. Use `sprach diagnostics` to measure whether the current threshold is appropriate given the measured d_eff and d̄.

**Implementation Summary:**
- `SEMANTIC_SEARCH_THRESHOLD` renamed to `DEFAULT_SEMANTIC_SEARCH_THRESHOLD` (kept as canonical default)
- Configurable `[facts].semantic_threshold` in `FactSettings` (default: 0.70, serde default)
- Threaded through `DedupContext.semantic_threshold` → `deduplicate_and_insert()` (8 args, `#[allow(clippy::too_many_arguments)]`)
- All 3 callers updated: `command_handlers.rs`, `fact_tools.rs`, `extract.rs`
- New `[retrieval]` config section with `keyword_weight` (default: 0.4) and `semantic_weight` (default: 0.6)
- Hardcoded `KEYWORD_WEIGHT`/`SEMANTIC_WEIGHT` constants removed from `context_builder.rs`
- `ThresholdRecommendation` struct in `diagnostics/embeddings.rs` with `recommend_threshold()` function
- Diagnostics report now includes **Recommended configuration** section with data-driven threshold and weight suggestions
- 6 new tests for threshold recommendation logic

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `[facts].semantic_threshold` to config | ✅ |
| 2 | Thread through dedup pipeline | ✅ |
| 3 | Add `[retrieval]` section with keyword_weight/semantic_weight | ✅ |
| 4 | Add `ThresholdRecommendation` struct and `recommend_threshold()` | ✅ |
| 5 | Add recommendation section to diagnostics display | ✅ |
| 6 | Tests for threshold recommendation logic | ✅ |

**Files to Modify:**
- `src/diagnostics/embeddings.rs` — Add `ThresholdRecommendation` struct, `recommend_threshold()` function, extend `EmbeddingDiagnostics`
- `src/diagnostics/display.rs` — Add "Threshold Recommendation" section to markdown report

**Effort:** ~0.5 day

**Deferred to Later Milestones:**

| Phase | Issue | Description | Priority | Milestone |
|-------|-------|-------------|----------|-----------|
| S2.6 | #139 | PCA Projection Search for vector retrieval | Medium | M3 |
| S2.7 | #140 | Embedding geometry documentation & model selection guide | Medium | M3 |
| B9 | #141 | Geometry-Aware Consolidation (GAC) | Low | M4 |
| B10 | #142 | Memory Consolidation Design (prerequisite for GAC) | Low | M4 |

**Affected Code:**

| File | Constant/Logic | Change |
|------|----------------|--------|
| `src/facts/conflict.rs:230` | `SEMANTIC_SEARCH_THRESHOLD = 0.70` | Validate with data before potentially changing to 0.80 (#134) |
| `src/retrieval/search.rs` | RRF weight `0.4/0.6` hardcoded | Adapt to d_eff: ≤10→0.7/0.3, 11-25→0.5/0.5, >25→0.4/0.6 (#137) |
| `src/content/db.rs` | RRF weight `0.4/0.6` hardcoded | Same as above (#137) |
| `src/embeddings/truncate.rs` | `FULL_DIMENSIONS=768, TRUNCATED_DIMENSIONS=256` | Make configurable, formula: `max(d_eff × 4, 64)` (#136) |
| `src/embeddings/client.rs` | `DEFAULT_EMBEDDING_MODEL` | Make configurable from config.toml (#106) |
| `src/db/schema.rs` | `FLOAT[256]` in 9 locations | Dynamic dimensions from config (#136) |
| `src/db/connection.rs` | `FLOAT[256]` in migration code | Migration to resize vec0 tables (#136) |

**RRF Weight Decision Table (from audit):**

| d_eff range | BM25 weight | Cosine weight | Rationale |
|-------------|-------------|---------------|-----------|
| d_eff ≤ 10 | 0.7 | 0.3 | Vector signal too weak; BM25 must dominate |
| d_eff 11-25 | 0.5 | 0.5 | Balanced signal from both sources |
| d_eff > 25 | 0.6 | 0.4 | Vector signal reliable; cosine can carry weight |

**Paper References (not stored in repo):**

- Matryoshka Representation Learning: arXiv:2205.13147 (Kusupati et al., 2022)
- In-Context Learning with Vector Retrieval: arXiv:2310.05342 (Borgeaud & Rekha)
- The Platonic Representation Hypothesis: arXiv:2405.07987 (Huh et al., 2024)

---

## 🔵 Core Enhancements — #72 [M1]

Features that enhance core functionality before Sprach 2.0 work begins.

### Retry Threshold with Backoff — #116 [M1]

**Status:** ✅ COMPLETED  
**Depends on:** None  
**Estimated effort:** 1.5–2 days  
**Issue:** #116  
**Branch:** `feat/116-retry-threshold-backoff`  
**PR:** #197 (merged 2026-06-02)

**Goal:** Make server errors (500, OOM, cold start) and tool execution errors recoverable with exponential backoff, instead of immediately failing.

#### W2 Wave Context

**Position in chain:** W2.0 — first card in the W2 Provider Chain, no dependencies.

**Upstream dependencies:** None (W1 quick wins already merged).

**Downstream pending work (documented in each issue's section):**
- **#118 (Tool Trait):** `Tool::call()` signature must remain `Result<String, Box<dyn Error>>` so the recovery pattern in #116 stays compatible. Proc macro cannot change error semantics.
- **#119 (Agnostic Provider Types):** `src/retry.rs` will be relocated to `src/provider/retry.rs`. `classify_for_retry(&OllamaError)` gets a sibling `classify_for_retry(&ProviderError)`. `ProviderError` should carry retry semantics (either via `retry_category()` method or via variant names).
- **#120 (OllamaProvider reqwest):** Reqwest-native errors (timeout, connect, 5xx) need explicit classification into the same `RetryCategory`. SSE parse errors → `NetworkRetry`. 429 → `RateLimitRetry`.
- **#121 (Consumer Migration):** `core.rs` retry loops (P6.0e.5) gain +1 day effort — must migrate `classify_for_retry(&OllamaError)` to `classify_for_retry(&ProviderError)`. `custom_coordinator.rs` (P6.0e.4) gain +0.5 day — the recovery pattern (push `ChatMessage::tool(error_msg)`) must use `LlmMessage::tool` from #119 via the `recovery::push_tool_result` wrapper.
- **#122 (OpenAI-Compatible Provider):** Uses `RateLimitRetry` variant pre-emptively added in #116. Must wire `Retry-After` header parsing. Cloud-specific non-retryable: `content_filter`, `invalid_api_key`, `insufficient_quota`. **Streaming tool call handling** differs fundamentally from Ollama native — see `doc/src/development/research/openai-streaming-tool-calls.md` for the full investigation (Ollama delivers complete tool call per chunk; OpenAI delivers arguments incrementally). **Recommended approach:** implement 100% OpenAI-compat first, test against Ollama's `/v1/chat/completions` endpoint, broaden testing (llama-swap, vLLM, llama.cpp) in next demand.
- **#123 (Remove ollama-rs):** `coordinator.rs` (100% coupled to `OllamaError`) gets rewritten or removed. `src/retry.rs` module is relocated in #119, so #123 just removes the last `use ollama_rs::error::OllamaError` lines. #123 effort increases from 2-3d to 3-4d due to the larger rewriter scope caused by #116.
- **#11 (Parallel Tool Execution):** Must adopt the per-tool-error-recovery pattern from #116. `join_all` collects `Vec<Result<String, String>>`, each error becomes a `LlmMessage::tool(error_msg)`. No batch-abort on single tool failure.

**W2 closure criterion (#123):** All `#[allow(dead_code)]` from W2 are resolved; no residual `OllamaError` coupling outside `src/provider/`.

#### W2 dead_code policy relaxation

Within the W2 mini-sprint, the project-wide `#[allow(dead_code)]` policy is **flexibilized**: code prepared for W2 future use (e.g., the `retry_after: Option<Duration>` field of `RateLimitRetry` wired in #122) is acceptable **as long as** it is resolved by the W2 closure (#123). Every `#[allow(dead_code)]` in W2 code MUST carry:
1. A justification comment with the future W2 issue number (e.g., `// Used in #122 Retry-After header parsing`).
2. The strict `cfg(test)` check at the W2 closure audit (per AGENTS.md "Quality Gates").

**Problem:**
- `OllamaError::InternalError` (HTTP 500) is classified as **non-recoverable** — conversations die immediately when Ollama has transient issues
- `ToolCallError::InternalToolError` (tool execution failure) is also **non-recoverable** — the model never sees the error and cannot self-correct
- No backoff strategy — all 3 retries happen at 0ms interval, potentially worsening server load
- No per-category retry limits — network errors and tool errors share the same `MAX_RETRIES = 3`

**Solution:** Classify errors into `RetryCategory` with per-category retry limits and backoff strategies.

**Proposed types:**

```rust
enum RetryCategory {
    /// Tool errors (UnknownTool, InvalidArgs) — immediate retry, 3 attempts
    ImmediateRetry { max_attempts: usize },
    /// Network errors (timeout, connection) — exponential backoff, 5 attempts
    NetworkRetry { max_attempts: usize },
    /// Server errors (500, OOM, cold start) — long backoff, 3 attempts
    ServerRetry { max_attempts: usize },
    /// Rate limiting (HTTP 429) — respects Retry-After header
    /// Pre-emptive: wired in #122 (OpenAI-compatible provider)
    RateLimitRetry {
        max_attempts: usize,
        retry_after: Option<Duration>,  // parsed from Retry-After header
    },
    /// Parsing errors (malformed JSON from model) — no automatic retry
    NoRetry,
}

fn retry_delay(category: &RetryCategory, attempt: usize) -> Duration {
    match category {
        ImmediateRetry { .. } => Duration::ZERO,
        NetworkRetry { .. } => Duration::from_millis(100 * 2_u64.pow(attempt as u32 - 1)),
        ServerRetry { .. } => Duration::from_secs(5 * attempt as u64),
        RateLimitRetry { retry_after, .. } => retry_after.unwrap_or(Duration::from_secs(2)),
        NoRetry => Duration::ZERO,
    }
}
```

**Key behavior changes:**

| Scenario | Before | After |
|----------|--------|-------|
| Ollama 500 (cold start) | Fail immediately | Retry 5s/10s/15s, model continues |
| Network timeout | 3 retries, 0ms delay | 5 retries with backoff (100ms→1.6s) |
| Tool execution failure | Non-recoverable (conversation dies) | Recoverable — model sees error, can self-correct |
| JSON parse error | Retry with 0ms | No automatic retry (model self-corrects naturally) |
| User Ctrl+C during 15s backoff | Waits 15s | Aborts immediately (cancel-aware sleep) |

**Implementation:**

| File | Change |
|------|--------|
| `src/retry.rs` (NEW) | `RetryCategory`, `classify_for_retry()`, `retry_delay()`, `max_attempts()`, `is_retryable()` |
| `src/chat/recovery.rs` (NEW) | `push_tool_result()` wrapper around `ChatMessage::tool()` — single migration point in #121 |
| `src/chat/coordinator.rs` | Deprecate `is_ollama_error_recoverable()` (will be removed in #119). InternalError → ServerRetry. InternalToolError → ImmediateRetry. |
| `src/chat/core.rs` (loops 565-616 and 789-852) | Use `classify_for_retry()` + `retry_delay()` + `tokio::select!` with cancel_token. UI: `"Retrying in Xs..."` |
| `src/chat/custom_coordinator.rs` (lines 931-942) | Tool `Err` → `push_tool_result(history, error_msg)` instead of `return Err(OllamaError::ToolCallError(InternalToolError(e)))`. LLM sees the error and can self-correct. |
| `src/query/executor.rs` (line 103) | Same classification + backoff in `execute_retry_loop()` |
| `src/chat/mod.rs` | Re-export `recovery` module |

**Design Decisions:**

1. **Wrapper `recovery::push_tool_result()`** — localizes the `ChatMessage::tool()` call site. #121 swaps the implementation to `LlmMessage::tool()` in one place.
2. **`RateLimitRetry` variant pre-emptively added** — the enum has a field `retry_after: Option<Duration>` that is unused until #122 wires up `Retry-After` header parsing. Justified by the W2 dead_code policy relaxation.
3. **Cancel-aware sleep** — `tokio::time::sleep(delay)` runs inside `tokio::select!` with the cancel token, so Ctrl+C aborts backoff immediately. This is non-negotiable: a 15s server-retry backoff that ignores Ctrl+C is a UX regression.
4. **Tool execution errors become recoverable** — `InternalToolError` previously was non-recoverable. Now the tool error is formatted into a human-readable message and pushed via `push_tool_result()`, allowing the LLM to see "tool X failed because Y" and try a different approach within the same turn.
5. **No `format_recovery_message` rewrite** — that function (in `coordinator.rs`) still produces the recovery prompt for non-tool errors (UnknownTool, InvalidArgs, etc.). It is unrelated to the new `RetryCategory` infrastructure.
6. **`MAX_RETRIES` constant in `coordinator.rs` kept** — still used for the final fallback debug log line. The new per-category limits live in `src/retry.rs` as private constants.
7. **Forward migration to `ProviderError` is pre-planned** — the signature of `classify_for_retry(&OllamaError)` will change to `classify_for_retry(&ProviderError)` in #119, but the body (the match arms) is the same shape with different variant names.

**Test Plan (15 unit tests in `#[cfg(test)]`):**

1. `test_classify_server_error` — `InternalError` → `ServerRetry`
2. `test_classify_network_error` — `ReqwestError` → `NetworkRetry`
3. `test_classify_tool_error_internal` — `ToolCallError::InternalToolError` → `ImmediateRetry`
4. `test_classify_tool_error_unknown` — `ToolCallError::UnknownToolName` → `ImmediateRetry`
5. `test_classify_tool_error_invalid_args` — `ToolCallError::InvalidToolArguments` → `ImmediateRetry`
6. `test_classify_json_error` — `JsonError` → `NoRetry`
7. `test_classify_other_error` — `Other` → `NoRetry`
8. `test_retry_delay_server_attempt_1` — Server attempt 1 → 5s
9. `test_retry_delay_server_attempt_2` — Server attempt 2 → 10s
10. `test_retry_delay_server_attempt_3` — Server attempt 3 → 15s
11. `test_retry_delay_network_exponential` — Network: 100ms, 200ms, 400ms, 800ms, 1.6s
12. `test_retry_delay_immediate_zero` — ImmediateRetry → Duration::ZERO
13. `test_retry_delay_ratelimit_with_retry_after` — RateLimit with retry_after=3s → 3s
14. `test_retry_delay_ratelimit_without_retry_after` — RateLimit with retry_after=None → 2s default
15. `test_max_attempts_per_category` — Server=3, Network=5, Tool=3, RateLimit=3, NoRetry=0
16. `test_is_retryable` — `NoRetry` → false, others → true
17. `test_retry_after_field_is_unused_until_122` — the field is set but never read (will be wired in #122)

Plus regression tests: existing tests in `coordinator.rs:190-218` for `classify_ollama_error` continue to pass.

#### Implementation Summary (post-merge)

PR #197 was merged on 2026-06-02 after addressing all 5 review threads. Final state:

**What landed:**
- `src/retry.rs` (NEW) — 389 lines, 20 unit tests for `RetryCategory` classification, `retry_delay`, `max_attempts`, `is_retryable`, `sleep_or_cancel`
- `src/chat/recovery.rs` (NEW) — `push_tool_result` wrapper, 2 unit tests
- `src/capabilities.rs` — added `check_server_health()` with 3s timeout, 2 unit tests (fixes startup hang)
- `src/chat/coordinator.rs` — `MAX_RETRIES` and `is_ollama_error_recoverable` removed (review feedback, YAGNI)
- `src/chat/core.rs` — both retry loops (send_message, send_message_stream) migrated; `// TODO(#120)` comments
- `src/chat/custom_coordinator.rs` — `InternalToolError` → push tool message; exception documented
- `src/chat/repl.rs` — `check_server_health()` call before DB init with clear error on failure
- `src/query/executor.rs` — query mode retry loop migrated; `// TODO(#120)` comment

**Final test count:** 2831 tests pass (4 tests removed: `test_is_ollama_error_recoverable` × 4 cases; 3 `test_format_*` retained).

**Review feedback addressed:**
- Thread 1: `MAX_RETRIES` removed (zero call sites in production, YAGNI)
- Thread 2: `is_ollama_error_recoverable` removed (no production callers, only deprecated tests)
- Thread 3: `MAX_RATELIMIT_RETRIES` removed (no reader anywhere)
- Thread 4: `RateLimitRetry` variant kept (defensible by W2 policy, verified in tests)
- Thread 5: Wrapper exception documented in 2 places (custom_coordinator.rs:942 + recovery.rs docstring)

**W2 closure status:** 1 of 9 cards in the W2 Provider Chain completed. Remaining: #118, #119, #120, #121, #122, #123, #11, #72 (parent).

#### Known Limitations (resolved by #120)

Manual testing of PR #197 revealed that **Scenarios 2, 3, and 4** from `MANUAL_TEST_116.md` are NOT fully mitigated by this PR. The retry infrastructure is correctly designed and the loops are in place, but the `ollama-rs` HTTP client does not surface these errors when Ollama hangs (kill -STOP, packet drop, server stopped) — it just hangs indefinitely. The `classify_for_retry(&OllamaError)` classification works, but the upstream `OllamaError` is never produced.

| Scenario | What works | What's blocked | Resolved by |
|----------|------------|----------------|-------------|
| **1 — Tool error recovery** | ✅ LLM self-corrects within same turn | — | This PR (#116) |
| **2 — Server 500 + linear backoff** | Retry infrastructure ready (5s/10s/15s) | `ollama-rs` hangs on Ollama stop/kill -STOP instead of returning 500 | **#120** (reqwest direct) |
| **3 — Network timeout + exp. backoff** | Retry infrastructure ready (100ms→1.6s) | `ollama-rs` lacks configurable timeout — packet drop hangs indefinitely | **#120** (reqwest + `.timeout()`) |
| **4 — Cancel-aware sleep** | `sleep_or_cancel()` ready with `tokio::select!` | Cancel token plumbing through `ollama-rs` is incomplete | **#120** (reqwest + cancel propagation) |
| **5 — UX quality** | Retrying in Xs... message format | — | This PR (#116) |
| **Bonus — startup hang** | ✅ **FIXED in this PR** | — | `check_server_health()` with 3s timeout |

The `// TODO(#120)` comments in the retry loops (`core.rs` × 2, `executor.rs` × 1) document the dependency.

**Acceptance criteria for #120** (must pass when #120 is implemented):
- Scenario 2: `kill -STOP` Ollama mid-request → "Retrying in 5s..." appears → resume with `kill -CONT` → conversation completes
- Scenario 3: `iptables -A OUTPUT -p tcp --dport 11434 -j DROP` BEFORE query → exponential backoff visible (100ms→200ms→400ms→800ms→1.6s) → graceful error after 5 attempts
- Scenario 4: Trigger any retry, Ctrl+C during backoff → returns in <1s

These criteria are also recorded in the #120 section of this document and in PR #197's body.

**Related:** Issue #72 (Multi-Provider parent), #118, #119, #120, #121, #122, #123, #11

---

### Multi-Provider Support — #72 [M1]

**Status:** 📋 PLANNED  
**Depends on:** #116 (retry — do first), #106 (Configurable Embedding Model)  
**Estimated effort:** 10–12 weeks (7 sequential sub-phases, each independently mergable)  
**Issue:** #72

**Goal:** Remove `ollama-rs` dependency entirely and implement a generic `LlmProvider` trait with direct reqwest-based Ollama and OpenAI-compatible implementations.

**Motivation:**
- **Full control:** No limitations from ollama-rs (e.g., missing `prompt_eval_count` in v0.3.4)
- **Multi-provider:** Support Ollama (local) and OpenAI-compatible APIs (llama.cpp, LM Studio, cloud)
- **Extensibility:** Future providers (Anthropic, Google) fit naturally into the trait
- **Tool macro ownership:** Our own `#[sprachspiel::tool]` proc macro, not dependent on third-party
- **Error handling control:** Full control over error formatting, including recovery messages when LLM calls wrong tool

**Architecture:**

```
┌──────────────────────────────────────────────────────┐
│           sprachspiel (business logic)                    │
│   Uses agnostic types:                                │
│   LlmMessage, LlmResponse, ToolInfo, ProviderError    │
├──────────────────────────────────────────────────────┤
│              LlmProvider trait                        │
│   chat() → LlmResponse                                │
│   generate() → String                                  │
│   embed() → Vec<f32>                                   │
│   detect_capabilities() → ProviderCapabilities        │
│   list_models() → Vec<ProviderModel>                  │
├──────────┬──────────────┬────────────────────────────┤
│ Ollama   │  OpenAI-     │  (future)                  │
│ Native   │  Compatible  │  Anthropic, Google          │
│ (reqwest │  (reqwest +  │                            │
│  direto) │   serde)     │                            │
└──────────┴──────────────┴────────────────────────────┘
```

**Coupling analysis (current state):**

| Metric | Count |
|--------|-------|
| Files with `ollama_rs` imports | 36 |
| Total `ollama_rs` type references | 131 |
| Tools using `#[ollama_rs::function]` macro | 36 |
| Files with direct `Ollama` client usage | 14 |
| Files with `ChatMessage` usage | 14 |

**Implementation phases (each independently mergable):**

```
#116 ──► #118 ──► #119 ──► #120 ──► (SSE) ──► #121 ──► #122 ──► #123
Retry    Tool     Tipos     Ollama    Streaming    Migração  OpenAI    Remove
Backoff  Trait/   Agnóst.   Provider  SSE          Consum.  Compat.  ollama-rs
         Macro
```

---

#### Tool Trait + Proc Macro `#[sprachspiel::tool]` — #118 [M1]

**Status:** ✅ COMPLETED  
**Depends on:** None  
**Estimated effort:** 1.5 weeks (actual: ~2 weeks including review iteration)  
**Issue:** #118  
**Branch:** `feat/118-tool-trait-proc-macro`  
**PR:** #198 (open)

**Goal:** Replace `ollama_rs::generation::tools::Tool` trait and `#[ollama_rs::function]` macro with our own, removing the tightest coupling surface (58 tools across 21 files).

**Merge criterion:** All 58 tools use `#[sprachspiel::tool]`, no tool uses `#[ollama_rs::function]`.

**Sub-deliverable status:**

| Sub-item | Status | Description |
|----------|--------|-------------|
| Trait + macro (Commit 1-3) | ✅ COMPLETED in PR #198 | `Tool` trait + `#[sprachspiel::tool]` proc-macro in `sprachspiel-tool-derive/`; dual-impl pattern preserves ollama-rs compat. |
| Bridge (Commit 2, 4, 5) | ✅ COMPLETED in PR #198 | Blanket impl, ToolRegistrar, CustomCoordinator all adopt `crate::tools::Tool` as primary bound. |
| Tool migration (Commit 6) | ✅ COMPLETED in PR #198 | All 58 tools migrated to `#[sprachspiel::tool]`. |
| DDG reimpl (Commit 7) | ✅ COMPLETED in PR #198 | `DdgSearcher` reimplemented ad-hoc, replacing ollama-rs's `DDGSearcher`. |
| Serper removal (Commit 8) | ✅ COMPLETED in PR #198 | `serper-tools` feature flag and all Serper code removed. MCP-based search planned post-W2. |

**W2 Wave Context — ollama-rs coexistence:**

This PR achieves the migration while keeping ollama-rs as a dependency for the W2 wave (planned removal in #123). The migration uses a **dual-impl macro** (emits both `impl crate::tools::Tool` and `impl ollama_rs::Tool`) so all migrated tools work with both the ollama-rs `Coordinator` and our `CustomCoordinator` without per-tool changes. This is a **pragmatic compromise**: the project's own trait is now the primary surface, and ollama-rs becomes a downstream consumer rather than the source of truth.

**Known limitation (carried into W2):**

Tool calls appear batched at the end of the stream in multi-round cycles. When a chat cycle involves multiple rounds (model makes tool calls → observes results → makes more tool calls → final response), all tool calls and their results appear in a single block at the end of the chat history, with the model's thinking/text emitted first. This is caused by the Ollama API design — tool calls are aggregated in a single `done=true` chunk per round, and round ordering is lost between rounds of a multi-round cycle. The TUI cannot reconstruct the correct visual order from the Ollama stream alone. This is a **structural refactor** that requires `process_response` to return `Result<Vec<ChatRound>>` and the TUI to render `Vec<MessageGroup>`. Tracked in **#201 (P0)**, which **blocks the next release** and is the next thing to be worked on after this PR merges.

**Files created:**

| File | Content |
|------|---------|
| `sprachspiel-tool-derive/Cargo.toml` | Proc-macro crate, depends on `syn`, `quote`, `proc-macro2` |
| `sprachspiel-tool-derive/src/lib.rs` | `#[proc_macro_attribute] fn tool` — emits dual `Tool` impl + `Params` struct with derives |
| `sprachspiel-tool-derive/LICENSE` | MIT license |
| `sprachspiel-tool-derive/NOTICE` | Attribution chain |
| `sprachspiel-tool-derive/tests/macro_test.rs` | 7 unit tests for the macro |
| `sprachspiel-tool-derive/tests/compile_fail.rs` | trybuild harness |
| `sprachspiel-tool-derive/tests/ui/*.rs` + `.stderr` | 4 compile-fail tests |
| `src/tools/tool_trait.rs` | `Tool` trait, `Parameters`, `ToolResult`, `ToolInfo`, `ToolType`, `ToolFunctionInfo` |

**Files modified (24 tool files, registry, coordinator, prompts, settings, utils, consts, configs):**

- 21 tool files migrated from `#[ollama_rs::function]` to `#[sprachspiel::tool]`
- `src/tools/registry.rs` — `ToolRegistrar` adopts `crate::tools::Tool` as primary bound
- `src/chat/custom_coordinator.rs` — `add_tool`, `ToolHolder`, `CustomToolInfo::new` adopt `crate::tools::Tool`
- `src/tools/search_builtin.rs` — replaced ollama-rs `DDGSearcher` with our own implementation
- `src/tools/serper.rs` — DELETED (Serper dropped in favor of MCP-based search)
- `src/prompts/tools.rs` — removed Serper prompt section
- `src/utils.rs` — removed `post_json_with_headers` (was Serper-only)
- `src/consts/api.rs` — removed `SERPER_API_URL` constant
- `src/settings.rs` — updated tool comment examples
- `Cargo.toml` — removed `serper-tools` feature, added `[workspace]` to include proc-macro crate
- `IMPLEMENTATION.md` — this section (status: COMPLETED, sub-deliverable table)

---

#### Agnostic Provider Types — #119 [M1]

**Status:** ✅ COMPLETED (PR #203)  
**Depends on:** #118 (error types should be compatible with new Tool trait)  
**Estimated effort:** 1 week  
**Merge criterion:** Types compile, `From` conversions tested, no existing files changed + **ProviderError carries retry classification semantics (consumed by #120)** — **MET**

**W2 Wave Context — ProviderError retry semantics:**

The `ProviderError` enum introduced in #119 MUST carry retry classification semantics so that `classify_for_retry(&ProviderError)` from the `src/retry.rs` infrastructure (created in #116) maps cleanly. Each variant must map to exactly one `RetryCategory`:

| `ProviderError` variant | `RetryCategory` | Rationale |
|------------------------|-----------------|-----------|
| `Api` (HTTP 5xx) | `ServerRetry` | Cold start, OOM, transient — 5s linear backoff |
| `Timeout` | `NetworkRetry` | Network timeout — 100ms→1.6s exponential backoff |
| `Connection` | `NetworkRetry` | Connection refused — same as Timeout |
| `RateLimit` | `RateLimitRetry` | HTTP 429 — respect `Retry-After` header |
| `Config` | `NoRetry` | Bad config — retry won't help |
| `Api` (parse error / 4xx) | `NoRetry` | Malformed response or client error |

The `classify_for_retry(&OllamaError)` function in `src/retry.rs` (created in #116) will be deprecated in favor of `classify_for_retry(&ProviderError)` with the mapping above. The `From<OllamaError> for ProviderError` conversion in `src/provider/conversions.rs` MUST preserve the retry category semantics during the transition.

**Goal:** Define `LlmMessage`, `LlmResponse`, `ProviderError` and bidirectional conversions from ollama-rs types.

**Files created:**

| File | Content |
|------|---------|
| `src/provider/mod.rs` | Module exports, `LlmProvider` trait definition |
| `src/provider/types.rs` | `LlmMessage`, `LlmRole`, `LlmResponse`, `ToolCallInfo`, `ProviderError`, `ProviderCapabilities`, `ProviderOptions`, `RetryCategory`, `retry_delay()` |
| `src/provider/conversions.rs` | `From<ollama_rs::ChatMessage>` / `Into<ollama_rs::ChatMessage>`, `From<OllamaError> for ProviderError`, unit tests |

**LlmProvider trait:**

```rust
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: Vec<LlmMessage>, tools: Vec<ToolInfo>, options: ProviderOptions) -> Result<LlmResponse, ProviderError>;
    async fn chat_stream(&self, messages: Vec<LlmMessage>, tools: Vec<ToolInfo>, options: ProviderOptions) -> Result<Pin<Box<dyn Stream<Item = Result<LlmStreamChunk, ProviderError>> + Send>>, ProviderError>;
    async fn generate(&self, prompt: &str, images: Vec<String>, audio: Vec<String>, options: ProviderOptions) -> Result<String, ProviderError>;
    async fn embed(&self, text: &str, model: &str, dimensions: Option<usize>) -> Result<Vec<f32>, ProviderError>;
    async fn detect_capabilities(&self, model: &str) -> Result<ProviderCapabilities, ProviderError>;
    fn provider_name(&self) -> &str;
    async fn is_available(&self) -> bool;
}
```

**No existing files were modified in this phase.** Types are defined + conversions are implemented, but business code still uses ollama-rs directly. All dead code in `src/provider/` annotated with `#[allow(dead_code)] // Consumed by #120/#121` per W2 policy.

**Implementation Summary:**
- `LlmMessage` with `LlmRole` (User, Assistant, System, Tool) — supports text, images, audio (base64), tool calls/results
- `LlmResponse` with `content`, `tool_calls`, `model`, `finish_reason`, `usage`
- `LlmToolCall` / `LlmToolResult` — structured tool interactions
- `ProviderError` with retry classification: `RetryCategory` enum + `retry_delay()` helper
- `ProviderOptions` — temperature, top_p, top_k, max_tokens, stop_sequences, seed, think, audio
- `ProviderCapabilities` — tools, vision, audio_in, audio_out, insert, stream, thinking, embeddings, model_list
- `LlmStreamChunk` — streaming response chunks
- `ToolInfo`, `ToolFunctionInfo`, `ToolType` — tool schema definitions
- Bidirectional conversions with `ollama_rs` types (lossless roundtrip tested)
- Unit tests: `test_roundtrip_user_text`, `test_roundtrip_assistant_with_tool_calls`, `test_roundtrip_tool_result`, `test_provider_error_retry_categories`

---

#### OllamaProvider (reqwest direct) — #120 [M1]

**Status:** ✅ COMPLETED (PR #206 — merged `d78831b` to `master`)  
**Depends on:** #119 (uses agnostic types)  
**Estimated effort:** 2–3 weeks  
**Merge criterion:** OllamaProvider passes same smoke tests as ollama-rs client + **retry acceptance criteria from #116 manual test (Scenarios 2, 3, 4) MUST pass**

**Goal:** Implement `OllamaProvider` that talks to Ollama API via reqwest, without depending on `ollama-rs::Ollama`. Introduces **named providers** in `models.toml` (breaking change: `ollama_host`/`ollama_port` removed from `config.toml`).

**Files created/modified:**

| File | Content |
|------|---------|
| `src/user_models.rs` | **MODIFY** — Parse `[provider."name"]` and `[models."name"]` with `provider = "name"`; remove `ollama_host`/`ollama_port` fallback; new `require_providers()` helper (E1); pre-parse heuristic for commented-out `[provider.*]` (E1) |
| `src/settings.rs` | **MODIFY** — Remove `ollama_host`/`ollama_port` from `ModelSettings` and `SAMPLE_CONFIG` |
| `src/provider/factory.rs` | **NEW** — `build_provider(name, all_providers) -> Result<Box<dyn LlmProvider>>`; `ProviderKind::Ollama` | `OpenAICompatible` (unimplemented!) |
| `src/provider/ollama.rs` | **NEW** — `OllamaProvider` struct, `LlmProvider` impl |
| `src/provider/ollama_api.rs` | **NEW** — Ollama API request/response structs (serde), endpoint URLs |
| `src/provider/streaming.rs` | **NEW** — NDJSON parser with idle timeout for chat_stream |
| `src/provider/mod.rs` | **MODIFY** — Export factory, types |
| `src/main.rs` / `src/repl.rs` | **MODIFY** — Use `factory::build_provider()` instead of `settings.ollama_client()` |
| `src/chat/model_switch.rs` | **MODIFY** — `switch_model()` receives `&dyn LlmProvider` |

**Sub-deliverables (each testable independently, sequential commits):**

| Sub | Description | Effort |
|-----|-------------|--------|
| 1 | Provider config parsing + factory + breaking change config format | 1 week |
| 2 | `POST /api/chat` + `chat_stream` (NDJSON, idle timeout, retry, shared client) | 1 week |
| 3 | Tool calling (parse/format `tool_calls`) | 3-5 days |
| 4 | `POST /api/generate` (vision/OCR images + format) | 2-3 days |
| 5 | `POST /api/embed` (Matryoshka `dimensions` parameter) | 2 days |
| 6 | `GET /api/show` / `/api/tags` → `detect_capabilities()` + full integration | 1-2 days |

**Breaking Changes (Config Format):**

```toml
# models.toml (NEW FORMAT - REQUIRED)
[provider."my-ollama"]
kind = "ollama"
base_url = "http://localhost:11434"
connect_timeout_secs = 5
read_timeout_secs = 300
stream_idle_timeout_secs = 60
max_retries = 3
retry_base_delay_ms = 2000
retry_max_delay_ms = 16000
retry_jitter_percent = 20

[models.glm-5.1]
model_id = "glm-5.1:cloud"
num_ctx = 202757
thinking = true
tools = true
provider = "my-ollama"

# config.toml — olama_host/port REMOVED from [model]
[model]
default = "glm-5.1"
thinking = false
```

**Defaults (Docstrings on Struct):**
- `connect_timeout_secs = 5`
- `read_timeout_secs = 300` 
- `stream_idle_timeout_secs = 60`
- `max_retries = 3`
- `retry_base_delay_ms = 2000`
- `retry_max_delay_ms = 16000`
- `retry_jitter_percent = 20`

**Key Features:**
- Shared `reqwest::Client` singleton — connection pooling, keep-alive
- NDJSON streaming parser with **idle timeout (60s default)** — prevents hangs
- Exponential backoff retry with **±20% jitter** + **`Retry-After` header parsing** (P0 for #122)
- `base_url` auto-normalization: `localhost:11434` → `http://localhost:11434`
- Health check: lazy in chat (first use), immediate in query/embed

**W2 Wave Context — Acceptance criteria from #116 manual test:**

This issue MUST pass the following manual test scenarios from `MANUAL_TEST_116.md` (located in `~/`) before being marked as complete. These scenarios are the **reason** #120 is critical — `ollama-rs` cannot surface the errors needed for retry to work.

- **Scenario 2 — Server 500 + linear backoff:** `kill -STOP` Ollama mid-request. Expect "Retrying in 5s..." → "Retrying in 10s..." → "Retrying in 15s..." (3 attempts, 5s linear backoff). Resume with `kill -CONT`. Conversation must complete successfully. Requires `ProviderError::Api` mapped to `RetryCategory::ServerRetry`.
- **Scenario 3 — Network timeout + exponential backoff:** `sudo iptables -A OUTPUT -p tcp --dport 11434 -j DROP` BEFORE query. Expect "Retrying in 100ms..." → "200ms..." → "400ms..." → "800ms..." → "1.6s..." (5 attempts, exponential backoff). Graceful error after exhaustion. Requires reqwest client with `.timeout(Duration)` and `ProviderError::Timeout`/`Connection` mapped to `RetryCategory::NetworkRetry`.
- **Scenario 4 — Cancel-aware sleep:** Trigger ServerRetry via any method. Press Ctrl+C during 5s/10s/15s backoff. Expect returns in <1 second (cancel respected). Requires cancel token propagation through reqwest's `tokio::select!`.

The retry infrastructure (`RetryCategory`, `retry_delay`, `sleep_or_cancel`) from #116 is in place — #120 makes `OllamaProvider` produce the right `ProviderError` variants for the retry loop.

**OpenAI-Compatible Placeholder:**
- `ProviderKind::OpenAICompatible` exists in enum
- `build_provider()` returns `Err(anyhow!("OpenAICompatibleProvider not yet implemented (see #122)"))`
- No feature flag — compiles, errors at runtime if referenced

---

#### #120 PR #206 — Code Review Findings (2026-06-11)

PR #206 received a comprehensive code review (REQUEST_CHANGES). The findings are categorized below as **(A) Resolved in PR #206**, **(B) Deferred to #121**, or **(C) Deferred to #122**.

**A. Resolved in PR #206 (this PR):**

| # | Issue | File | Action |
|---|-------|------|--------|
| A1 | **BUG CRÍTICO:** `OllamaToolCallFunction.arguments: String` breaks tool calls in Ollama native (deserializes as `String` but Ollama sends JSON object) | `src/provider/ollama_api.rs:27`, `ollama.rs:307,412` | Change to `serde_json::Value` |
| A2 | **BUG ESTRUTURAL:** `LazyLock<USER_MODELS_FILE>` with `process::exit(1)` crashes `cargo test --lib` when `models.toml` is missing | `src/user_models.rs:216-222` | Return `UserModelsFile::default()` (empty HashMaps); caller decides |
| A3 | **Clippy 7 errors:** `derivable_impls` on `ProviderKind`, `for_kv_map` on provider iter, `if_same_then_else` on classify_error | `user_models.rs:40, 197`, `ollama.rs:27` | Derive `Default`, use `.values_mut()`, consolidate if |
| A4 | **Docstring stale:** `long_about` mentions duplicate model detection that doesn't exist (Pitfall 11) | `src/translate/cli.rs:305-306` | Remove duplicate claim from docstring |
| A5 | **factory.rs module docstring inconsistent with impl** (says "JSON object" for Ollama but struct has `String`) | `src/provider/factory.rs:15` | Update after A1 |
| A6 | **`ModelsUpgradeReport` 3 dead code fields** (Pitfall 2/3) | `src/commands/models_upgrade.rs:47-53` | Remove struct; return `Vec<String>` only |
| A7 | **`let _ = action_verb;` dead code literal** | `models_upgrade.rs:116` | Remove line |
| A8 | **`provider_name()` inherent method duplicates trait method** | `src/provider/ollama.rs:55-58` | Replace with `const PROVIDER_NAME: &str = "ollama";` |
| A9 | **Dead Cargo.toml deps:** `anyhow`, `bytes`, `tracing` (optional w/o feature) | `Cargo.toml` | Remove if confirmed unused |
| A10 | **Division by zero in `backoff_delay`** (retry_base_delay_ms == 0) | `src/provider/ollama.rs:177-179` | Guard with `if retry_base_delay_ms == 0` |
| A11 | **`rand::random::<u64>() % 0` panic** (jitter_range == 0) | `src/provider/ollama.rs:184` | Guard with `if jitter_range.is_zero() { return delay; }` |
| A12 | **`unwrap_or_default()` silent in production** (embeddings) | `src/provider/ollama.rs:556` | Add `log::warn!` companion |
| A13 | **`#![allow(clippy::print_stdout)]` module-level** | `src/commands/models_upgrade.rs:18-19` | Use `#[expect(...)]` |
| A14 | **`eprintln!` without `log::error!` companion** | `src/user_models.rs:219` | Add `log::error!` before |
| A15 | **`process::exit(1)` in `handle_models_upgrade`** inconsistent with rest of code | `src/main.rs:733-735` | Return `Err(...)` |
| A16 | **Streaming silently drops malformed NDJSON lines** | `src/provider/ollama.rs:394` | Add `log::warn!` with counter |
| A17 | **Integration test for `build_provider`** (or `#[cfg(test)]` mock) | `tests/provider_factory.rs` (new) | Add smoke test calling `build_provider("my-ollama", &cfg).provider_name()` |
| A18 | **TUI rendering glitch:** `/session forget` shows as `sessiontforget` | (view/welcome line) | Investigate and fix width/truncation |
| A19 | **`SMOKE_TEST.md` outdated:** references `/forget` instead of `/session forget` | `SMOKE_TEST.md` | Update docs |

**B. Deferred to #121 (Consumer Migration):**

| # | Issue | Reason |
|---|-------|--------|
| B1 | **`Settings::ollama_client()` deprecated regression:** returns `Ollama::default()` ignoring custom host/port (regression for users with custom config) | #121 migrates all 5 call sites; method will be **removed** in #121. Documented limitation: users with custom `ollama_host`/`ollama_port` in `config.toml` must migrate to `models.toml` `[provider]` block before upgrading. |
| B2 | **`#[allow(dead_code)]` chain on `factory::build_provider` → `OllamaProvider::new` → `provider_name`** | All 3 annotations justified by "Used in #121". Integration test (A17) provides call site. |
| B3 | **`LlmProvider` trait `#[allow(dead_code)] // Consumed by #120`** | Comment is now factually wrong (it's consumed BY this PR). Corrected in this PR to `// Consumed by OllamaProvider in this PR; build_provider call site in #121`. |
| B4 | **`RateLimit retry_after: Option<Duration>` claim 'Will be populated from header'** in #116 | Header parsing is #122 work. Remove claim from #120 code or mark as `#122 TODO`. |

**C. Deferred to #122 (OpenAI-Compatible Provider):**

| # | Issue | Reason |
|---|-------|--------|
| C1 | **`tracing` optional dependency without feature flag** | If `tracing` is genuinely unused in the PR, remove. If it will be used in #122 for instrumented spans, add `tracing` feature flag. |

**D. Pre-existing clippy limitations (W2 follow-up):**

| # | Issue | Reason |
|---|-------|--------|
| D1 | **`src/chat/command_handlers.rs:28` imports `DocumentEntry`/`DocumentListData` without `#[cfg(feature = "document-tools")]`** | Pre-existing: imports are unconditionally pulled in, but their USES are gated by `#[cfg(feature = "document-tools")]` (line 3023, etc.). When compiling `--no-default-features --features X` (X ≠ document-tools), the import is "unused" — 1 error per individual feature. |
| D2 | **`src/chat/tui/markdown.rs` has unused items gated by `#[cfg(feature = "...")]`** | Pre-existing: variables `lang` (lines 270, 1523) and `render_mermaid` (line 1426) and function `mermaid_style` (line 888) trigger "unused" errors when features are turned off individually. |
| D3 | **`src/markdown/standalone.rs:59` and `src/markdown/table.rs:115` have unused items** | Pre-existing: `render_special` (standalone) and `lang` (table) trigger "unused" errors in feature-gated builds. |
| D4 | **`src/tools/weather.rs` uses `crate::utils::fetch_json` (gated by `search-tools`) but `pub mod weather;` in `src/tools/mod.rs:14` is unconditional** | Pre-existing: this combination causes compile errors in any feature combination that lacks `search-tools`. `pub mod weather;` should be `#[cfg(feature = "weather-tools")]` and the body should also gate its functions with `#[cfg(feature = "weather-tools")]`. |
| D5 | **`src/tools/weather.rs` has unfulfilled `#[expect(dead_code)]` annotations** (lines 361, 366, 369, 385, etc.) | Pre-existing: when the module is compiled WITHOUT `weather-tools` (which still happens because D4 is unfixed), these expectations are unfulfilled. |

**E. Follow-up (PR #120 review, Hefesto round 3):**

| # | Issue | Resolution |
|---|-------|-----------|
| E1 | **Provider bail-out was unreachable in `repl_tui.rs:82`** | The earlier `resolve_model_config` call (in `repl.rs:638`, `query/context.rs:119`, `summarize/processor.rs:36`, `main.rs::handle_vision`, `model_switch.rs:56`) used `process::exit(1)` with a generic "Unknown model" message when `models.toml` was missing the `[provider]` block, masking the actual config error. **Resolution:** new `user_models::require_providers()` helper called at every entry point; pre-parse heuristic in `load_user_models_internal` detects the common user error (commented-out `[provider.*]`) before TOML parse fails with the generic `missing field 'provider'` message. Both fixes preserve `cargo clippy --all-features` clean. |

**Merge criterion update:** PR #206 is mergeable when all A1-A19 are resolved and clippy passes. B1-B4 are documented as limitations to be resolved in #121; C1 in #122. D1-D5 are pre-existing feature-gating bugs to be fixed in a W2 close-out PR (not blocking). E1 is the bail-out fix from this round.

**Implementation summary (PR #206, commit `d78831b`):**

- New `OllamaProvider` in `src/provider/ollama.rs` (native reqwest, ~707 lines)
- New `LlmProvider` trait + agnostic types in `src/provider/{mod.rs,types.rs,conversions.rs}` (foundation for #122)
- New `factory::build_provider()` for instantiating providers from config
- New NDJSON streaming with idle timeout (60s default) and structured log warnings
- Retry with exponential backoff (200ms→16s) + 20% jitter, division-by-zero guards
- Health check at startup, file write sandbox, embed() with warn-on-empty fallback
- `models upgrade` subcommand (additive, never destructive)
- Breaking config change: `ollama_host`/`ollama_port` removed from `config.toml`; users migrate to `models.toml` `[provider]` section via `sprach models upgrade`
- `cargo clippy --all-features`: 0 warnings
- 2952 tests pass (1461 lib + 1488 bin + 3 integration `provider_factory.rs`)
- W2 chain progress: #116 ✅ → #118 ✅ → #119 ✅ → **#120 ✅** → #121 → #122 → #123

**Next in W2 chain:** #121 (Consumer Migration — migrate all business modules from `ollama_rs` to `LlmProvider`), #122 (OpenAI-Compatible Provider), #123 (Remove ollama-rs).

---

#### Streaming SSE

**Status:** 📋 PLANNED  
**Depends on:** #120  
**Estimated effort:** 1 week  
**Merge criterion:** SSE parsing works, testable via `sprach query "text" --stream`

**Goal:** Add SSE (Server-Sent Events) streaming to `OllamaProvider`. Not currently used by the CLI, but required for the TUI (M3). Can be tested independently via query mode with `--stream` flag (plain text, no markdown rendering).

**Files to create:**

| File | Content |
|------|---------|
| `src/provider/streaming.rs` | SSE parser, `StreamEvent` type, `StreamChunk` type |
| `src/provider/ollama.rs` extension | `chat_streamed()` / `generate_streamed()` methods |

---

#### Consumer Migration — #121 [M1]

**Status:** ✅ COMPLETED (branch `feat/121-consumer-migration-openai`, PR #207 — implementation complete, 47 commits, awaiting final review/merge)  
**Depends on:** #118 (Tool trait) ✅ + #119 (agnostic types) ✅ + #120 (OllamaProvider) ✅  
**Estimated effort:** 2–3 weeks (revised: ~5 weeks for OpenAI-first strategy)  
**Issue:** #121  
**Branch:** `feat/121-consumer-migration-openai`  
**Merge criterion:** No `use ollama_rs` in business modules AND `OpenAICompatibleProvider` is the default for all backends (Ollama, llama.cpp, vLLM) AND `models upgrade` migrates old `kind="ollama"` configs automatically.

**Strategic Shift (vs. original plan):** This PR implements the **OpenAI-First strategy** (R2 from planning session). The default provider is now `OpenAICompatibleProvider` (OpenAI-spec HTTP), not Ollama's native `/api/chat`. Ollama is reached through `http://localhost:11434/v1`, llama.cpp through llama-swap, etc. The `OllamaProvider` introduced in #120 is **removed**; `ProviderKind::Ollama` is kept as a deprecated alias in `factory.rs` for backward compat (returns a runtime error if used, prompting user to run `sprach models upgrade`).

**Why OpenAI-First:** Maintaining two HTTP transports (Ollama native + OpenAI compat) creates permanent maintenance debt. The OpenAI-spec API is the de facto standard for local LLM serving (Ollama, llama.cpp, vLLM, llama-swap, LM Studio all expose it). Issue [ollama/ollama#11325](https://github.com/ollama/ollama/issues/11325) was closed as "not planned" — Ollama's OpenAI-compat does NOT support `top_k`, `min_p`, etc. — so a unified path requires the strict OpenAI-subset of parameters.

**Breaking changes in `models.toml`:**

1. **`kind` default changes from `"ollama"` to `"openai"`** — existing configs with `kind = "ollama"` are auto-migrated by `sprach models upgrade`.
2. **`base_url` requires `/v1` suffix** — e.g., `http://localhost:11434/v1` for Ollama; the migration adds it automatically.
3. **Fields removed from `UserModelConfig`:** `top_k`, `repeat_penalty`, `think` — not supported by OpenAI API nor by Ollama's OpenAI-compat endpoint. They were Ollama-native only and cannot be tunneled through the OpenAI-spec body.
4. **New field added:** `seed` (cross-provider, optional) — supported by both OpenAI spec and Ollama's `/v1/chat/completions`.

**Goal:** Migrate all consumers from `ollama_rs` types to `LlmProvider` and agnostic types, with OpenAI-compatible HTTP as the single transport.

**Sub-deliverables (each mergable independently):**

| Sub | Component | Files | Effort | Prerequisite |
|-----|-----------|-------|--------|-------------|
| P6.0e.1 | `capabilities.rs` → `LlmProvider::detect_capabilities()` + auto-detect `num_ctx` via `/v1/models` and `/api/show` fallback | 1 | 0.5-1 day | P6.0d |
| P6.0e.2 | `embeddings/client.rs` → `LlmProvider::embed()` (uses `/v1/embeddings`) | 1 | 1 day | P6.0d |
| P6.0e.3 | `subagent.rs` → `LlmProvider::chat()/generate()` | 1 | 1-2 days | P6.0d |
| P6.0e.4 | `custom_coordinator.rs` → uses `LlmProvider` + agnostic types | 1 | 1 week | P6.0b+c+d |
| P6.0e.5 | `core.rs` → receives `Box<dyn LlmProvider>` | 1 | 0.5 day | P6.0e.4 |
| P6.0e.6 | `repl.rs`, `repl_state.rs`, `model_switch.rs` | 3 | 0.5 day | P6.0e.5 |
| P6.0e.7 | `query/*.rs` | 4 | 1 day | P6.0e.4 |
| P6.0e.8 | `vision/processor.rs`, `ocr/processor.rs`, `summarize/processor.rs` | 3 | 1 day | P6.0e.3 |
| P6.0e.9 | `main.rs` — provider construction | 1 | 0.5 day | P6.0e.6 |
| P6.0e.10 | Remove `#[allow(dead_code)]` from `factory::build_provider`, `OllamaProvider::new`, `provider_name` (chain reachable via P6.0e.9 + P6.0e.5) | 1 | 0.1 day | P6.0e.9 |
| **P6.0e.new1** | `OpenAICompatibleProvider` with SSE streaming, tool calling, embeddings, `/v1/models`, **Retry-After header parsing** | 4 | 1.5-2 weeks | P6.0d |
| **P6.0e.11** | `models upgrade` migration: `kind="ollama"` → `openai`, add `/v1` suffix to `base_url` | 1 | 0.3 day | P6.0e.9 |
| **P6.0e.12** | Remove `OllamaProvider` source; keep `ProviderKind::Ollama` as deprecated alias | 1 | 0.1 day | P6.0e.11 |

**B items from #120 review (resolved in #121):**
- **B1**: `Settings::ollama_client()` deprecated regression — **REMOVED in #121**; all 5 call sites migrated to `factory::build_provider()`.
- **B2**: `#[allow(dead_code)]` chain on `factory::build_provider` → `OllamaProvider::new` → `provider_name` — **RESOLVED** when `OpenAICompatibleProvider::new` becomes the new factory target.
- **B3**: `LlmProvider` trait `#[allow(dead_code)]` — **REMOVED**; trait is now consumed by `OpenAICompatibleProvider` directly.
- **B4**: `Retry-After` header parsing — **IMPLEMENTED in #121** via `OpenAICompatibleProvider::classify_error()` reading `Retry-After` from HTTP headers on 429 responses. Wires up the previously-unused `retry_after: Option<Duration>` field on `ProviderError::RateLimit`.

**Acceptance criteria:**
- 35 files in `src/` no longer contain `use ollama_rs` (verified by `rg 'use ollama_rs' src/`)
- `cargo clippy --all-features -- -D warnings` clean
- `cargo test --all-features` passing
- Manual test against llama-swap (Ollama + llama.cpp + vLLM) passes
- `models upgrade` migrates existing configs correctly

**Indexing Configuration Redesign (W2 #121 extension — user feedback):**

The first version of the W2 #121 extension added a provider-level `embedding = true` flag on `[provider.*]` and a `[embedding]` section in `config.toml`. After user review, this design was replaced with a model-level approach where the embedding capability is declared on the **model** (not the provider), and the section is renamed from `[embedding]` to `[indexing]` (merged with the old `[retrieval]`).

**New schema (W2 #121 extension, final):**

```toml
# config.toml
[indexing]
model = "nomic"               # ALIAS from models.toml [models.*]
probe = true                  # opt-out; default true
keyword_weight = 0.4          # moved from [retrieval]
semantic_weight = 0.6         # moved from [retrieval]
```

```toml
# models.toml
[provider."llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"
# NO `embedding = true` here — provider is just a transport

[models."gemma4-e2b"]
model_id = "gemma4-e2b:think"
provider = "llama-swap"
# Chat model (no embeddings flag → safe for -m and /model)

[models."nomic"]
model_id = "nomic-embed-text-v2-moe"
provider = "llama-swap"
embeddings = true   # opt-in: reserved for /v1/embeddings
dimensions = 768    # REQUIRED when embeddings = true
```

**Resolution rules (in `Settings::resolve_indexing_model`):**

1. `[indexing].model` is empty → fatal error: `[indexing].model is empty`.
2. The alias doesn't exist in `models.toml` → fatal error: `Indexing alias '<name>' not found in models.toml`.
3. The alias exists but doesn't have `embeddings = true` → fatal error: `Model '<name>' is not declared as an embedding model`.
4. The alias has `embeddings = true` but no `dimensions` → fatal error (caught at models.toml load time).
5. The alias's `provider` doesn't exist → fatal error.

**Probe (adaptive + strict verify, opt-out, default on):** the probe does NOT pass `dimensions` in the request body (some providers reject it). The response's vector dim count is compared against the alias's declared `dimensions`. Mismatch is a fatal error:
```
Error: Probe indexing dim mismatch: alias declares dimensions=768,
but provider returned 256 dimensions for model 'nomic'.
...
```

**Embedding-only model rejection:** `-m <alias>` and `/model <alias>` reject aliases with `embeddings = true`. The TUI completer filters them out automatically. `sprach --list` shows them with a `[embeddings-only]` tag.

**Files added/modified:**

| File | Change |
|------|--------|
| `src/provider/embedding_models.rs` (new) | Hardcoded list of 11 well-known embedding model fragments; `is_potential_embedding_model(name)`. Kept as `#[cfg(test)]` (no production call sites in this PR; reserved for future tooling). |
| `src/user_models.rs` | `ProviderConfig.embedding: bool` REMOVED. `UserModelConfig.embeddings: bool` (default `false`) + `dimensions: Option<u32>` ADDED. Validation: if `embeddings = true` and `dimensions` is None, fail at load time. |
| `src/settings.rs` | `Settings.embedding: EmbeddingSettings` → `Settings.indexing: IndexingSettings`. Removed `IndexingSettings.provider` field. Merged `keyword_weight` and `semantic_weight` from the (now-removed) `RetrievalSettings`. Added 4 helper methods: `indexing_model_alias()`, `indexing_probe_enabled()`, `indexing_keyword_weight()`, `indexing_semantic_weight()`. Added `resolve_indexing_model()` returning `(UserModelConfig, ProviderConfig, model_id, dimensions)`. Removed `resolve_embedding_provider()`. |
| `src/embeddings/client.rs` | `with_model(ollama, model_id, dimensions)` now takes 3 args. `DEFAULT_EMBEDDING_MODEL` constant REMOVED. |
| `src/provider/openai_compat.rs` | `probe_embedding(model)` now returns `Result<usize, ProviderError>` (the response dim count). ADAPTIVE: no `dimensions` in request body. |
| `src/provider/ollama_shim.rs` | `probe_embedding(model)` shim delegation returns `Result<usize, ...>`. |
| `src/db/init.rs` | `EmbeddingInit` → `IndexingInit { provider, model_id, dimensions, probe }`. `run_embedding_probe` → `run_indexing_probe(provider, model_id, dimensions, probe_enabled)` with strict dim verify. |
| `src/chat/repl.rs` | `init_chat_database` uses `Settings::resolve_indexing_model()`. Builds separate `Ollama` (shim) for embedding provider. Probe with strict dim verify before DB init. |
| `src/chat/model_switch.rs` | Rejects embedding-only models in `/model` command. |
| `src/main.rs` | `--model` flag rejects embedding-only models (both `handle_query_subcommand` and `handle_legacy_query`). `sprach --list` adds `[embeddings-only]` tag. |
| `src/retrieval/search.rs` | `run_search(db, ollama, embedding_model_id, embedding_dimensions, query, ...)` — was just `embedding_model_name`. |
| `src/chat/command_handlers.rs` | `handle_search` and reindex command use `Settings::resolve_indexing_model()` to get the model_id and dimensions. |
| `src/user_models.rs` | New helpers: `list_chat_model_names()` (filters out embedding-only), `is_model_embedding_only(name)`. |
| `src/chat/repl_tui.rs` | TUI completer uses `list_chat_model_names()` (filters out embedding-only). |
| `src/query/mod.rs` | `settings.indexing.keyword_weight` and `.semantic_weight` (moved from `settings.retrieval.*`). |
| `src/query/context.rs` | Resolves indexing alias for query subcommand (skips the probe). |
| `src/commands/config_upgrade.rs` | Auto-detects missing `[indexing]` section (renamed from `[embedding]`). |
| `src/commands/models_upgrade.rs` | `MissingEmbeddingFlag { name }` REMOVED. `MissingDimensions { alias }` ADDED (warning when model has `embeddings = true` but no `dimensions`). |
| `src/lib.rs` | `pub mod commands;` to expose the upgrade tests. |
| `src/settings.rs::SAMPLE_CONFIG` | `[embedding]` section replaced with `[indexing]`; `[retrieval]` section removed. |
| `~/.config/sprachspiel/models.toml` | `embedding = true` removed from `[provider."llama-swap"]`. `[models."nomic"]` block added with `embeddings = true`, `dimensions = 768`. |
| `~/.config/sprachspiel/config.toml` | `[embedding]` → `[indexing]`, `model = "nomic"` (alias), `provider = "llama-swap"` removed (inferred from alias), `keyword_weight` and `semantic_weight` added. `[retrieval]` section removed. |

**Tests:** 1530 lib tests pass. New tests:
- `user_models` `UserModelConfig.embeddings` + `dimensions` (4 tests: default false, opt-in, dimensions required when embeddings, dimensions optional for chat models)
- `user_models::is_model_embedding_only` (1 test)
- `user_models::list_chat_model_names` (helper, no dedicated test)
- `settings` `IndexingSettings` parsing (4 tests: default, minimal, full, omitted) + `resolve_indexing_model` (4 tests: empty alias, alias not found, alias not embedding-capable, alias success)
- `db::init` `IndexingInit` (4 tests: struct construction, skip_persistence, empty model_id rejected, whitespace model_id rejected)
- `embeddings::client::with_model` (2 tests: constructor, stores model name and dimensions)
- `query::tests::test_query_uses_indexing_weights` (regression test)
- `commands::config_upgrade` (2 tests: missing indexing section, present indexing section; renamed from `embedding`)
- `commands::models_upgrade` (3 tests: missing dimensions detected, present dimensions OK, chat models don't need dimensions; renamed from `embedding flag`)
- `main` smoke test: `sprach --list` shows `nomic` with `[embeddings-only]` tag

**12 commits** in this PR (in order):
1. `c8891cc` refactor(user_models): move embedding flag to UserModelConfig
2. `a806a7d` refactor(settings): rename [embedding] to [indexing] and merge [retrieval]
3. `f1fcf1b` feat(settings): add resolve_indexing_model alias resolver
4. `e56ddab` refactor(db): rename EmbeddingInit to IndexingInit with dimensions
5. `74c5663` refactor(embeddings): pass dimensions to EmbeddingClient
6. `23046d0` refactor(chat,db,provider): wire indexing pipeline with strict dim probe
7. `978139f` refactor(callsites): resolve indexing alias for search and reindex
8. `7064a84` test(query): add regression test for [indexing] weights
9. `4de9a04` feat(rules): reject embedding-only models in -m and /model
10. `227363e` refactor(migrations): model-level MissingDimensions warning

(Commits 11 (chore: user config) and 12 (docs) are in this same PR but not listed above — see CHANGELOG.md and configuration.md for the user-facing documentation.)

**ReAct Regression Bugs Investigation (W2 #121 follow-up):**

During smoke testing of #121, three apparent ReAct regression bugs were investigated:

1. **TUI message ordering** (real bug, fixed): Tool messages appeared in reverse order (`tool → tool → ... → assistant`) because `drain_and_add_tool_messages()` (in `event_loop.rs`) called `add_message()` (= `messages.push` at the end) AFTER the Assistant of that round had already been finalized via `finalize_streaming_zone_as_is()`. **Fix:** use `insert_before_streaming_zone()` (in `app.rs`) which has three-way logic: insert before streaming zone if present, insert before trailing tool messages, fall back to push. Resolves the bug at all 7 call sites, not just `ToolCallStarted`.

2. **Cold-model 400 (transient 4xx) without diagnostic trace** (real bug, fixed): When `is_transient_4xx_error()` classified an error as transient (e.g., llama-swap model swap, empty body) and the retry path executed, the 4xx body was NOT logged — only the surfacing path logged it. This made it impossible to diagnose cold-model failures from logs alone. **Fix:** added a `log::debug!` of the truncated 4xx body in the transient retry path of `chat_with_retry()` (in `openai_compat.rs`), mirroring the surfacing-path diagnostic.

3. **Silent context reset (32K→994 tokens)** (FALSE ALARM): Initially appeared that `session.messages` was being silently truncated between requests. Investigation revealed:
   - `Session: messages=N` (logged by `context_builder.rs:321`) reports `session.messages.len()` — the canonical session state.
   - `Complete: messages_count=M` (logged by `event_loop.rs`) reports `app.message_count()` — the TUI chat area, which includes `Thinking` and `AssistantStreaming` placeholders that exist transiently during multi-round tool cycles.
   - The two counters measure different things: `Session: messages` grows by 2 per user turn (user message + assistant response), while `Complete: messages_count` is the total UI rendering, which can be 5-10× the session count during multi-round tool cycles.
   - The `base_tokens=880` after a `base_tokens=32179` is just `prompt_eval_count` from the Ollama response — the size of THAT SPECIFIC REQUEST's prompt, which legitimately varies with how much context was sent.
   - **No silent truncation occurs.** The session grows monotonically. This was a measurement-comparison confusion, not a real bug.

**Files:** `src/chat/event_loop.rs` (Bug 1), `src/provider/openai_compat.rs` (Bug 2).

---

#### OpenAI-Compatible Provider Resilience — #122 [M1]

**Status:** ✅ COMPLETED (merged into branch `feat/121-consumer-migration-openai`, awaiting PR #207 review)  
**Depends on:** #121 (OpenAI-compatible provider already implemented and merged into this branch)  
**Estimated effort:** 2–3 weeks  
**Issue:** #122  
**Merge criterion:** Provider consumer layer treats the LLM call as a first-class event stream: tool-call preview, post-tool streaming, visible retry, and tool-execution lifecycle events.

**Strategic context:** The OpenAI-compatible HTTP transport and agnostic provider types were built in #121. Smoke testing revealed that the **consumer layer** (`custom_coordinator.rs` and the TUI event loop) still sees streaming as "accumulate chunks, then run the non-streaming tool loop". This made the provider feel fragile: tool arguments were collected silently, post-tool turns were not streamed, retry was invisible, and long tool executions gave no progress feedback. A comparison with the Pi Coding Agent (`~/git/thirdparty/pi`) showed that Pi's robustness comes from treating the LLM call as a **rich, push-based event stream** consumed uniformly by the loop and the UI. This issue refactored Sprachspiel's consumer layer to match that model.

**Reference analysis:** `~/papers/sprachspiel-openai-provider-lessons-from-pi.md` maps Pi's event-stream design onto Sprachspiel's types and derives the implementation plan below.

**Design decisions (approved and implemented):**

| Topic | Decision | Rationale |
|---|---|---|
| Tool-call accumulator | `ToolCallAccumulator` lives in the provider | Single source of truth for `LlmToolCall`; no duplicated JSON parsing in coordinator |
| Stream API | `chat_stream` returns `Stream<Item = Result<LlmStreamEvent, ProviderError>>` | Replaces pull-based `LlmStreamChunk`; enables lifecycle events |
| `LlmStreamChunk` | Remove completely | Superseded by `LlmStreamEvent` |
| `ollama_shim.rs` | Can be broken/simplified/removed | Ollama is reachable via OpenAI-compatible endpoints; shim no longer needs to preserve `ollama_rs` API |
| Retry events | Variants inside `LlmStreamEvent` | Keeps the stream unified; rendered in the **status bar** (red, right-aligned), not the message buffer |
| Tool-call preview | Rendered inside a volatile `LiveTurn` keyed by `tool_call_id`; previews carry `is_streaming = true` until frozen | Replaces fragile single-buffer preview matching (`is_tool_preview`/`find_tool_preview_index`) with exact key-based identity |
| Post-tool streaming | All ReAct turns stream, including after tool results | Removed `InterToolText` event and the non-streaming `process_next()` path |
| Streaming buffer | Two-Buffer model: `App::messages` (committed history) + `App::live_turn` (volatile turn) | Eliminates duplicated thinking, fragile insertion heuristics, and multi-round ordering bugs |
| Tool execution output | Skeleton only (`Started` / `Finished`) | Full partial-output callbacks deferred to a follow-up |

**New event vocabulary (`src/provider/types.rs`):**

```rust
pub enum LlmStreamEvent {
    // Lifecycle
    Start,
    Done { reason: Option<String>, usage: Option<LlmUsage> },
    Error { error: ProviderError },

    // Content blocks
    TextStart,
    TextDelta { delta: String },
    TextEnd { content: String },

    ThinkingStart { signature: Option<String> },
    ThinkingDelta { delta: String },
    ThinkingEnd { content: String },

    // Tool-call lifecycle
    ToolCallStart { index: u32, id: Option<String>, name: Option<String> },
    ToolCallDelta { index: u32, id: Option<String>, name_delta: Option<String>, argument_delta: String },
    ToolCallEnd { index: u32, call: LlmToolCall },

    // Retry lifecycle (rendered in status bar)
    ProviderRetryStarted { attempt: u32, max_attempts: u32, delay_ms: u64, reason: String },
    ProviderRetryFinished { success: bool, attempt: u32 },
}
```

**New `LlmEvent` variants (`src/chat/llm_event.rs`):**

```rust
ToolCallPreview { tool_call_id: String, name: String, args: serde_json::Value }
ToolExecutionStarted { tool_call_id: String, name: String, args: serde_json::Value }
ToolExecutionFinished { tool_call_id: String, result: String, is_error: bool }
ProviderRetryStarted { attempt: u32, max_attempts: u32, delay_ms: u64, reason: String }
ProviderRetryFinished { success: bool, attempt: u32 }
```

**Implementation phases (each a granular commit):**

| Phase | Commit focus | Files | Status |
|---|---|---|---|
| 0 | Add `LlmStreamEvent`, `LlmUsage`, lifecycle `LlmEvent` variants, `ChatMessage::is_tool_preview` flag | `src/provider/types.rs`, `src/chat/llm_event.rs`, `src/chat/app.rs` | ✅ `6c2138e` |
| 1 | Extract `ToolCallAccumulator` from SSE parser | `src/provider/openai_compat.rs` (+ new `src/provider/tool_accumulator.rs`) | ✅ `6a1863c` |
| 2 | Emit retry lifecycle events from `chat_with_retry` and `chat_stream` | `src/provider/openai_compat.rs` | ✅ `0d98b1e` |
| 3 | Replace `LlmStreamChunk` with `LlmStreamEvent`; simplify/remove `ollama_shim.rs` | `src/provider/openai_compat.rs`, `src/provider/ollama_shim.rs`, `src/provider/types.rs` | ✅ `cc17d58` |
| 4a | `chat_stream` consumes `LlmStreamEvent`; emits `ToolCallPreview` | `src/chat/custom_coordinator.rs` | ✅ `70c58df` |
| 4b | Make post-tool turns streaming (`process_next_stream`) | `src/chat/custom_coordinator.rs` | ✅ `4ff21bf` |
| 5 | Tool execution lifecycle events | `src/chat/custom_coordinator.rs`, `src/chat/llm_event.rs` | ✅ `d3b3181` |
| 6a | Render tool-call preview in message buffer | `src/chat/event_loop.rs`, `src/chat/app.rs`, `src/chat/view/*.rs` | ✅ `4a16cf6` |
| 6b | Render tool execution finished state | `src/chat/event_loop.rs`, `src/chat/app.rs`, `src/chat/view/*.rs` | ✅ `4a16cf6` |
| 6c | Render retry status in status bar | `src/chat/event_loop.rs`, `src/chat/app.rs`, `src/chat/view/*.rs` | ✅ `4a16cf6` |
| 7 | Remove `InterToolText`, dead code, docs, clippy, tests | All above | ✅ `1fdc309` |
| 8 | Two-Buffer live turn: introduce `LiveTurn`, `App::live_turn`, drive events through live turn | `src/chat/tui/live_turn.rs`, `src/chat/app.rs`, `src/chat/event_loop.rs`, `src/chat/view/ratatui_view.rs` | ✅ `33566f4` |
| 9 | Drive live turn from event loop; remove `StreamBlockDone`, `block_finalized` | `src/chat/event_loop.rs`, `src/chat/core.rs`, `src/chat/llm_event.rs`, `src/chat/app.rs` | ✅ `ae5d217` |
| 10 | Remove legacy insertion methods (`insert_before_streaming_zone`, `insert_after_round_0`, `insert_at_round_boundary`, `streaming_zone_start`, `find_tool_preview_index`) and legacy preview flag (`is_tool_preview`/`freeze_preview`) | `src/chat/app.rs`, `src/chat/tui/components/chat_area.rs`, `src/chat/tui/live_turn.rs` | ✅ uncommitted |
| 11 | Remove legacy tool-message channel (`tool_call_rx`/`drain_tool_messages`/`drain_and_add_tool_messages`/TUI_CALLBACK wiring); tool results enter only via `ToolExecutionFinished`; suppress `PreToolContent` in streaming TUI path | `src/chat/view/ratatui_view.rs`, `src/chat/event_loop.rs`, `src/chat/repl_tui.rs`, `src/chat/core.rs`, `src/chat/llm_event.rs` | ✅ uncommitted |

**Testing plan (executed):**

- 1543 lib tests passing (`cargo test --features all-tools --lib`) after Two-Buffer cleanup + TUI bug fixes.
- SSE parser events with incremental tool-call arguments (unit tests in `tool_accumulator.rs`).
- Post-tool streaming validated by code path; mock-provider test deferred to follow-up.
- Retry events emitted from `OpenAICompatibleProvider::chat_with_retry` (integration scenario deferred to manual test).
- Tool lifecycle event sequence (`Started` → `Finished`) logged and forwarded to TUI.
- TUI preview insertion order enforced by `freeze_all_tool_previews()` on `ToolCallStarted`.
- TUI retry overlay rendered in status bar (red, right-aligned) via `StatusBarState::overlay`.

**Known limitations / follow-up:**

- `ToolExecutionStarted`/`Finished` currently only mark start/end. Full partial-output streaming for long-running tools is deferred to a follow-up issue.
- The Two-Buffer redesign is structurally complete; visual styling of streaming/preview states is reserved for future work.
- `cargo clippy` (default features) passes; `--all-features` and individual feature flags still expose pre-existing dead-code/feature-gating warnings that are out of scope for #122 (documented in AGENTS.md and to be addressed in W2 close-out).

**Related:** Issue #122, #121 (predecessor), #123 (final ollama-rs removal). Reference: `~/papers/sprachspiel-openai-provider-lessons-from-pi.md`.

---

#### TUI Streaming Bug Fixes — #121/#122 follow-up (commit `baab7be`)

**Status:** ✅ COMPLETED  
**Commits:** `02e2a9f` (tool result truncation) + `baab7be` (7 interconnected bug fixes)

Smoke testing of PR #207 with `glm-5.2:cloud` and `gemma4-e2b:think` revealed seven interconnected bugs in the TUI streaming path. The symptoms were:

1. **Context count drops** from ~16K (during reasoning) to ~766/1.0K (after completion).
2. **Tool calls disappear** — N tool calls accumulate during the ReAct loop, then "all disappear" and only 3 remain.
3. **Text is replaced** — pre-tool text streamed before tool calls vanishes when the final response is committed.
4. **Tool call arguments not shown** — `🔧 list_directory() (id)` with no args displayed.
5. **Empty tool name** — `🔧 () (list_directory)` with the name field empty and the id in its place.
6. **Status bar corruption** — `eprintln!` leaking into the ratatui alternate screen, producing artifacts like `cursive=` from tool result content appearing in the modeline.

**Root causes and fixes:**

| Bug | Root cause | Fix | Files |
|-----|-----------|-----|-------|
| A: Context drops to ~1K | `stream_turn` ignores `LlmStreamEvent::Done` usage → `final_data` always `None` → `TokenMetrics::default()` (zeros) | Capture `usage` from `Done` event and populate `final_data` with `prompt_eval_count`/`eval_count` | `custom_coordinator.rs` |
| B: Fallback excludes system+tools | `history_real_tokens()` fallback estimates only message content, not system prompt (~3.5K) + tools (~2.9K) | Add `history_real_tokens_with_overhead(overhead)`; `spawn_llm_task` passes `system_tokens + tools_tokens` | `session.rs`, `event_loop.rs` |
| C: Text replaced | `finalize_stream()` uses `retain()` to remove ALL `Text` blocks, replacing with single `post_tool_content` block | Remove only the LAST `Text` block (via `rposition`+`remove`), preserving earlier rounds' pre-tool text | `app.rs` |
| D: Tool calls collide | `tool_call_id = call.function.name.replace(' ', '_')` — same tool in multiple rounds shares the same id → `set_tool_result` overwrites earlier results, `ToolExecutionStarted` skips freezing later previews | Generate unique id via monotonic counter: `{tool_name}_{counter}` | `custom_coordinator.rs` |
| E: Previews orphaned | `ToolExecutionStarted` handler skips `freeze_tool_preview_by_name` when a block with the same id already exists | Always call `freeze_tool_preview_by_name`; add guard against duplicate blocks | `event_loop.rs`, `live_turn.rs` |
| F: Results overwritten | `set_tool_result` finds the last block with `id == tool_call_id` without checking if it already has a result | Prefer blocks with `result.is_none()` before overwriting | `live_turn.rs` |
| Args: Arguments not displayed | `freeze_tool_preview_by_name` uses preview args (empty `Object({})`) when the provider didn't stream `argument_delta` — common with Ollama/cloud providers that only send args in `ToolCallEnd` | Fall back to `ToolExecutionStarted` args when preview args are empty | `live_turn.rs` |

**Additional fix (earlier in session, commit `baab7be`):**

| Bug | Root cause | Fix |
|-----|-----------|-----|
| eprintln leak | After commit `5c4df48` removed `set_tui_callback()` from `RatatuiView::new()`, `tui_aware_print`/`display_tool_call`/`log_tool_result` fell through to `eprintln!`, corrupting the alternate screen | Add `if crate::logging::is_tui_mode() { return; }` early return to all three functions |

**Token count fix (earlier in session, commit `baab7be`):**

| Bug | Root cause | Fix |
|-----|-----------|-----|
| estimate_status_bar ignores system+tools | `estimate_status_bar()` passed `String::new()` as system prompt, so system (~3.5K) + tools (~2.9K) were not counted during streaming | Use `build_pre_tool_prompt(self)` for the real system prompt and add `tool_count * TOKENS_PER_TOOL` |

**Verification (terminal-use / tu with `glm-5.2:cloud`):**

- Tool calls show name and args: `🔧  list_directory(path=.) (list_directory_1)`, `🔧  read_file(path=Cargo.toml) (read_file_2)` ✅
- 80+ tool calls accumulate across ReAct rounds without disappearing ✅
- Pre-tool text preserved between rounds (not replaced) ✅
- Status bar shows 23K–94K (realistic), not 1K–766 ✅
- No status bar corruption from `eprintln!` leaks ✅
- IDs unique: `list_directory_1`, `list_directory_2`, ..., `read_file_67`, etc. ✅

**Test count:** 1543 lib tests passing (`cargo test --features all-tools --lib`), up from 1538.

---

#### BUG-1 Root Cause Re-test (Round 3) — Revised Diagnosis & Fix

**Status:** ✅ COMPLETED (commit pending — awaiting Hermes Agent re-test)
**Previous rounds:** `PR207-TEST-RESULTS.md` (R1), `PR207-RETEST2-RESULTS.md` (R2), `PR207-RETEST3-RESULTS.md` (R3)
**Previous fix attempts:** `9162fd7` (broaden empty-args check), `e24a146` (BUG-2 ordering + BUG-1 debug logging)

Re-testing of PR #207 (round 3, commit `e24a146`) confirmed BUG-2 (thinking block ordering) is **FIXED** but BUG-1 (empty tool call args with local models) **still fails** with BeeLama/DFlash. The debug logs added in `e24a146` revealed the previous diagnosis was incomplete: the `freeze_tool_preview_by_name` correction (lines 303-338) presumed the streaming preview and the `ToolExecutionStarted` used the **same** `tool_call_id`, which is only true for cloud models. A deeper code review found **two interlinked bugs**:

| Bug | Root cause (revised) | Fix |
|-----|---------------------|-----|
| (i) `tool_call_callback` dead | `custom_coordinator.rs:377` registers the callback that emits `LlmEvent::ToolCallStarted` (which drives `freeze_all_tool_previews` + `LlmState::ToolCall` transition), but the callback was **never invoked** in `stream_turn` after the migration to `LlmStreamEvent` granular events. Consequence: the event_loop never froze previews in the streaming path, leaving them orphaned and causing `freeze_tool_preview_by_name` to miss the match. | Invoke `tool_call_callback` on the **first** `LlmStreamEvent::ToolCallStart` of each turn (flag `tool_call_started_emitted` reset per `stream_turn`). Preserves single-fire semantics for multi-tool turns. |
| (ii) `tool_call_id` divergence | `LlmStreamEvent::ToolCallStart` creates the preview with the provider's id (empty for BeeLama, `"call_123"` for cloud). `ToolExecutionStarted` (coordinator:1203) synthesizes `format!("{name}_{counter}")` — a different id. `freeze_tool_preview_by_name` can't match by id, and name-match fails when the name is also empty (BeeLama). | Preserve the stream id in a new `stream_tool_call_ids: Vec<String>` field, populated in `ToolCallEnd` (preserving `LlmToolCall.id` which `ollama_rs::ToolCall` lacks). The ReAct execution loop reuses this id when non-empty, falling back to the `{name}_{counter}` synthetic id only when the stream id is empty. |
| (iii) `freeze_all_tool_previews` timing | Calling `freeze_all_tool_previews` in the `ToolCallStarted` handler (event_loop:426) froze previews **before** `argument_delta` populated args (for cloud models), replicating BUG-1 for cloud too once (i) was fixed. | Move `freeze_all_tool_previews` from the `ToolCallStarted` handler to the start of the `ToolExecutionStarted` handler (after the stream ended and all previews are fully populated). `ToolCallStarted` still does `finalize_streaming_zone_as_is` + `increment_round` + `set_llm_state(ToolCall)` for immediate visual feedback. |
| (iv) name-match defense | Even with (i)+(ii)+(iii), an edge case remains: a frozen block with a divergent stream id AND an empty name (provider streamed neither) leaves no match path in `freeze_tool_preview_by_name`. | Add a defense-in-depth fallback in `freeze_tool_preview_by_name`: after id-match in `blocks` and `tool_previews` fail, scan `blocks` in reverse for the last `TurnBlock::ToolCall` with `result.is_none()`, empty/matching name, and empty args; update its name, args, and `tool_call_id` so `set_tool_result` can find it. |

**Files changed:**

| File | Change |
|------|--------|
| `src/chat/custom_coordinator.rs` | Add `stream_tool_call_ids: Vec<String>` field; reset at start of `stream_turn`; populate in `ToolCallEnd`; reuse in ReAct loop (prefer stream id, fallback to `{name}_{counter}`); invoke `tool_call_callback` on first `ToolCallStart` per turn via `tool_call_started_emitted` flag |
| `src/chat/event_loop.rs` | Move `freeze_all_tool_previews()` from `ToolCallStarted` handler to `ToolExecutionStarted` handler (before `freeze_tool_preview_by_name`); keep `finalize_streaming_zone_as_is` + `increment_round` + `set_llm_state` in `ToolCallStarted` |
| `src/chat/tui/live_turn.rs` | Add BUG-1 fallback (iii) in `freeze_tool_preview_by_name`: name-match over frozen `blocks` (last block with no result + empty/matching name + empty args) updates name/args/tool_call_id |

**Unit tests added (4):**

| Test | Scenario |
|------|----------|
| `freeze_tool_preview_by_name_local_model_empty_stream_id` | BeeLama flow: preview id="" name="" args={} → freeze_all → `freeze_tool_preview_by_name("read_file_1", "read_file", {path})` → block has args (via fallback) + `set_tool_result` finds it |
| `freeze_tool_preview_by_name_cloud_model_streamed_id_and_args` | Cloud flow: preview id="call_123" name="read_file" args={path} → freeze_all → `freeze_tool_preview_by_name("call_123", ...)` → match by id, args preserved |
| `freeze_tool_preview_by_name_multi_round_no_collision` | Same tool in R1 and R2 — unique ids via counter, `set_tool_result` finds correct block each round |
| `freeze_tool_preview_by_name_fallback_updates_frozen_block_by_name` | Divergent non-empty stream id + matching name + empty args → fallback updates block without duplicate |

**Verification:**

- `cargo build --features all-tools` ✅
- `cargo test --lib --features all-tools` → **1552 passed, 0 failed** (up from 1548) ✅
- `cargo clippy -- -D warnings -A clippy::allow_attributes -A clippy::too_many_lines -A clippy::cognitive_complexity` → **0 errors** ✅
- `cargo fmt --check` → clean ✅
- Manual re-test with BeeLama/DFlash (local) and glm-5.2:cloud: **pending Hermes Agent re-test** (skill `pr-testing`)

**Why the previous fix `e24a146` didn't work:** The correction in `freeze_tool_preview_by_name` (lines 303-338) assumed the block frozen by `freeze_all_tool_previews` had the **same** `tool_call_id` as the `ToolExecutionStarted`. This was only true for cloud models (which stream `call_123` and the coordinator reused that id). For BeeLama, (i) prevented `freeze_all_tool_previews` from running at all, so no block existed to match; and (ii) made the synthesized id diverge from the (empty) stream id, so even after (i) was hypothetically fixed, the id-match would still fail. The combination of (i) + (ii) + (iii) + (iv) is required to close BUG-1 for both local and cloud models.

**R4 completion:** Commit `294133c` (applied by Hermes Agent during R4 testing) added the final BUG-1 layer: the `ToolCallEnd` handler now updates the preview with the final parsed `call.arguments` (during streaming, each `argument_delta` is a partial JSON fragment that cannot parse alone, so the preview stayed as a partial `Value::String`), and the `ToolCallDelta` handler now treats `Value::Object({})` (from `ToolCallStart`) as an empty string base for concatenation. See R4 results: `~/PR207-BUG1-R4-TEST-RESULTS.md`.

---

#### Provider Switching Bug — /model does not rebuild LLM client (R4 finding)

**Status:** ✅ COMPLETED
**Found in:** R4 re-test (`~/PR207-BUG1-R4-TEST-RESULTS.md` section "Bug Adicional Encontrado")

**Bug:** Switching models via `/model <novo>` mid-session updated `state.model_config` (carrying the new model's `model_id`) but did NOT rebuild `state.ollama` — the HTTP client/shim that actually sends requests. The `ollama` client stayed bound to the initial model's provider (e.g., `llama-swap`), even when the new model declared `provider = "ollama"` in `models.toml`. Result: the next prompt went to the wrong provider → `⛔ no router for requested model` (from llama-swap, which doesn't know `glm-5.2:cloud`).

**Root cause:** `model_switch::switch_model()` received `ollama: &Ollama` (immutable) only for `ModelCapabilities::detect()` — it could not rebuild the client and return it. `ModelSwitchResult` did not carry the new client. The caller `handle_model_switch` updated 5 state fields but omitted `state.ollama`.

**Fix (Opção B1 — refactor `switch_model` to build and return the new client):** `switch_model` now receives `settings: &Settings` instead of `ollama: &Ollama`. It builds the new client internally via `settings.ollama_client_for_model(model_name)` (which resolves the provider from `models.toml` — each model declares `provider = "<name>"`), uses it for capability detection (so detection hits the right provider from the start — previously it used the old provider and could fall back to the old model's capabilities), and returns the new client in `ModelSwitchResult.ollama`. The caller assigns `state.ollama = result.ollama;`.

| Layer | File | Change |
|-------|------|--------|
| Signature | `model_switch.rs` | `ollama: &Ollama` → `settings: &Settings`; build client internally via `settings.ollama_client_for_model()` |
| Struct | `model_switch.rs` | Add `pub ollama: crate::provider::Ollama` field to `ModelSwitchResult` |
| Caller | `command_handlers.rs` | Pass `&state.settings` instead of `&state.ollama`; add `state.ollama = result.ollama;` |
| Test | `model_switch.rs` | `switch_model_rebuilds_ollama_for_new_provider` — verifies `result.ollama` points to the new model's provider `base_url` (via `CompatOllama` Debug impl), using the environment's `models.toml` (skips if models missing) |

**Note on #123:** The `ollama` field carries the `CompatOllama` shim type because `CustomCoordinator` still uses the shim's API (`send_chat_messages_stream_events`), not the `LlmProvider` trait. When #123 (Remove ollama-rs) migrates the coordinator to `Box<dyn LlmProvider>`, this field's type changes alongside — a localized change. This fix does NOT invade #123's scope.

**Verification:** 
- Unit test: `switch_model_rebuilds_ollama_for_new_provider` ✅ (1553 lib tests pass, up from 1552)
- Manual R5 re-test (Hermes Agent, pending): reproduce R4 scenario — start with local model (llama-swap), `/model glm-5.2:cloud` (ollama provider), send prompt → must work without `no router for requested model`; switch back to local, send prompt → must work.

---

#### Embedding Fallback + Chunk Sizing Fix (R5 follow-up)

**Status:** ✅ COMPLETED
**Commits:** `fe7c267` (is_context_exceeded + chunk sizing), `7dfd0d4` (find_sentence_boundary min limit)

When switching the embedding model to `lfm2.5-embed-350m` (512-token batch size), 38 chunks failed with `input (544 tokens) is too large to process. increase the physical batch size`. The root cause was in the sprachspiel embedding pipeline, not the backend:

| Bug | Root cause | Fix |
|-----|-----------|-----|
| Fallback never triggered | `is_context_exceeded()` only recognized `context_length`, `context length`, `maximum context`, `token limit`, `sequence length` — NOT the BeeLama/llama.cpp error format (`batch size`, `too large to process`) | Added `batch size`, `too large to process`, `too long` to the pattern list (`client.rs:70-77`) |
| Chunks oversized | `DEFAULT_CHARS_PER_TOKEN=3.0` and `DEFAULT_CHUNK_PERCENT=0.80` generated chunks of 1132 chars (~377 estimated tokens) that the real tokenizer counted as 544+ | Reduced to `2.0` and `0.65` — new max_chars = 613 (~307 estimated tokens, headroom for 50% tokenizer imprecision) |
| Chunk explosion | `find_sentence_boundary()` could shrink chunks to ~185 chars (one sentence) when the nearest boundary was found early, generating 79 chunks for a 10KB text (exceeding `MAX_CHUNKS_PER_ITEM=64`) | Added `min_chunk_size` parameter (60% of `max_chars`) — the function refuses to return a boundary that would shrink the chunk below this minimum |

**SOTA evolution mapping** (deferred to future PRs):
- **Token-aware chunking** (replace chars/token estimate with real tokenizer): confirmed in W4.4 (#107)
- **Recursive character splitting** (`\n\n` → `\n` → `. ` → ` `): já mapeado no M4 SemanticChunker
- **Document-aware chunking**: já mapeado no M4 SemanticChunker, after milestone 2

**Verification:** 1555 lib tests pass (+2 new: `test_sentence_boundary_respects_min_chunk_size`, `test_repetitive_text_no_chunk_explosion`).

---

#### Remove ollama-rs — #123 [M1]

**Status:** ✅ COMPLETED  
**PR:** #213  
**Branch:** `feat/123-remove-ollama-rs`  
**Depends on:** #121 (all consumers migrated) ✅  
**Estimated effort:** 1.5–2 weeks (actual: ~4 days across 10 phases)

**Implementation Summary:**

The `ollama-rs` crate has been completely removed from the dependency tree. All LLM communication goes through `OpenAICompatibleProvider` implementing the `LlmProvider` trait. The 730-line `ollama_shim.rs` compatibility bridge is deleted. All 17 production files that referenced `ollama_rs` types have been migrated to agnostic types (`LlmMessage`, `LlmResponse`, `ProviderError`, `ProviderOptions`, `LlmToolCall`, `LlmRole`).

**10 Phases (tracked via Linear LUC-40 → LUC-49):**

| Phase | Scope | Commit |
|-------|-------|--------|
| 1 | Retry/error path migration — relocated `retry.rs` to `provider/retry.rs`, `classify_for_retry` accepts `ProviderError` | `010e11b` |
| 2 | Coordinator migration — `CustomCoordinator` → `Coordinator`, all types migrated, string-sniffing eliminated | `ca424d6` |
| 3 | Business consumer migration — 15+ files migrated from `ChatMessage` to `LlmMessage` | `f81af47` |
| 4 | Remove `ollama_shim.rs` — `CompatOllama` replaced by `OpenAICompatibleProvider` directly | `968bd65` |
| 5 | Remove `ollama-rs` from `Cargo.toml` — dependency tree clean | `da865a0` |
| 6 | TTFB watchdog — `parse_sse_stream` gains 120s time-to-first-byte timeout | `515d10c` |
| 7 | W2 dead_code audit + clippy feature-matrix fixes | `d0ef724`, `3c00431` |
| 8 | Comment audit — Portuguese translated, stale W2 narrative removed, `get_ollama` → `get_provider` | `84e7a2b`–`ac389ab` |
| 9 | Documentation review — Pass 1 (OpenCode) | (this commit) |
| 10 | Manual tests — pending Hermes Agent execution | TBD |

**Quality gates:**
- `cargo build --all-features` without `ollama-rs` ✅
- `cargo clippy --all-features -- -D warnings` clean ✅
- `cargo test --all-features --lib` — 1547 passed, 0 failed ✅
- `rg 'use ollama_rs' src/` returns nothing ✅
- `rg 'ollama_shim' src/` returns nothing ✅
- `rg 'CompatOllama' src/` returns nothing ✅
- Feature-matrix clippy (`--no-default-features --features weather-tools`) — 0 unused/never-used warnings ✅

**Related:** Issue #72 (Multi-Provider Support parent — closes when #123 merges), Issue #123

---

#### Cycle-Aware Message Ordering in TUI — #201 [M1] 🚨 **P0 — BLOCKS NEXT RELEASE** ✅ **MERGED**

**Status:** ✅ COMPLETED  
**Depends on:** None (orthogonal to W2 Provider Chain; can be worked in parallel with #119+)  
**Estimated effort:** 1–2 weeks (TUI event loop + tests + manual test)  
**Issue:** #201  
**Branch:** `fix/201-cycle-aware-message-ordering`  
**PR:** #202

**Goal:** Fix UX regression where tool calls and their results from multiple rounds appear in a single block at the end of the chat history, with the model's thinking/text content emitted first. In a multi-round cycle (model searches → observes results → searches again → final response), tool indicators are now correctly positioned after the content of the round they belong to, making the model's reasoning flow readable.

**Root cause (Ollama API design, NOT a TUI bug):**

The Ollama API streams thinking/content tokens in real-time but aggregates `tool_calls` into a single `done=true` chunk per round. The TUI event loop had no mechanism to group messages by round — `insert_before_streaming_zone()` positioned inter-round content before ALL tool messages regardless of which round they belonged to.

**Implementation (completed, single PR):**

1. **`ChatMessage.round_index: usize`** — Ephemeral field (default 0, not persisted to SQLite, reset per user prompt). Tracks which round of a multi-round cycle each message belongs to. Builder method `with_round_index(round)`.

2. **`App.current_round: usize`** — Incremented on `ToolCallStarted` and `InterToolText`. Reset to 0 on `Complete`/`Cancelled`/`Error` and on each new user prompt (`handle_key_line`). Accessor methods: `current_round()`, `increment_round()`, `reset_round()`.

3. **`App.insert_at_round_boundary(message)`** — Positions inter-round content after all messages with `round_index <= message.round_index`, respecting the `AssistantStreaming` boundary. Replaces `insert_before_streaming_zone()` for round-aware inserts in the `InterToolText` handler. Uses a two-step algorithm: (a) find the `AssistantStreaming` boundary at the tail, (b) scan backward in the stable zone for the last message with `round_index <= target_round`, insert after it.

4. **Event loop updates** — `InterToolText` handler: drains tool messages BEFORE incrementing `current_round` (using `prev_round` for correct round_index), then increments round, uses `insert_at_round_boundary()` with `round_index` for both thinking and content. `ToolCallStarted`: increments `current_round`. `Complete`/`Cancelled`/`Error`: drains tool messages BEFORE resetting `current_round` so they get the correct round_index. `StreamDone`: drains tool messages BEFORE creating the final response message, ensuring the last round's tool messages appear before the answer instead of after it. `drain_and_add_tool_messages()`: changed from using `current_round()` implicitly to accepting an explicit `round: usize` parameter, preventing round_index assignment bugs where tool messages from round N were assigned round N+1 or 0. `handle_key_line()`: resets `current_round` at start of each prompt.

5. **`MessageGroup` + `build_lines()` refactor — SKIPPED (YAGNI)** — Messages are already in correct temporal order after insertion; `build_lines()` iterates over flat `Vec<ChatMessage>` without needing group-level rendering. No visual separator between rounds (user's decision). This avoids adding a `round.rs` module for a grouping mechanism that provides no rendering benefit.

**Files changed:**
- `src/chat/tui/components/chat_area.rs` — `ChatMessage` gains `round_index: usize` (default 0), `with_round_index()` builder, `round_index: 0` in all constructors
- `src/chat/app.rs` — `current_round: usize`, `insert_at_round_boundary()`, `current_round()`, `increment_round()`, `reset_round()`, 10 new unit tests
- `src/chat/event_loop.rs` — `InterToolText` round-aware, `ToolCallStarted` round increment, `Complete`/`Cancelled`/`Error` round reset, `drain_and_add_tool_messages` round assignment, `handle_key_line` round reset

**Files NOT changed:** `custom_coordinator.rs`, `core.rs`, `session.rs`, `continuation.rs`, `repl.rs`, `llm_event.rs` — coordinator, persistence, and event types are untouched.

**Key decisions:**
- `round_index` is ephemeral — not persisted to SQLite, zeroed per user prompt, does not affect embeddings or retrieval.
- No visual separator between rounds — correct ordering is sufficient.
- No `process_response` return type change — TUI reconstructs rounds from event sequence.
- No `MessageGroup`/`round.rs` — messages are correctly ordered after insertion, `build_lines()` flat iteration is sufficient (YAGNI).
- `insert_at_round_boundary()` only excludes `AssistantStreaming` from the round-index search (not `Thinking`), because finalized `Thinking` blocks from `InterToolText` are stable content that should participate in the round boundary.
- `insert_before_streaming_zone()` is preserved for `ViewAction` handlers (round-0 content, compact separator) that don't need round awareness.
- **`drain_and_add_tool_messages()` uses explicit `round` parameter** — not `current_round()` — because tool messages must be assigned the round they were generated in, which is the round BEFORE any increment for `InterToolText` and BEFORE reset for `Complete`/`Error`/`Cancelled`. This prevents the bug where round N tool messages got round_index N+1 (after increment) or 0 (after reset).
- **`InterToolText` handler drains tool messages BEFORE incrementing round** — this ensures tool messages from the previous round carry that round's index, and subsequent `insert_at_round_boundary()` for the new round's content positions correctly after them.

**Unit tests (15 new — 10 original + 4 for round_index fix + 1 for StreamDone drain):**
- `test_insert_at_round_boundary_round0_no_rounds_yet`
- `test_insert_at_round_boundary_round1_after_round0`
- `test_insert_at_round_boundary_round1_between_round0_and_streaming`
- `test_insert_at_round_boundary_round2_after_round1`
- `test_insert_at_round_boundary_all_same_round`
- `test_insert_at_round_boundary_multiround_realistic`
- `test_round_lifecycle_increment_and_reset`
- `test_round_index_default_zero`
- `test_with_round_index_builder`
- `test_tool_messages_get_correct_round_index_before_increment` — verifies InterToolText handler drains tool messages with prev_round (before round increment), not the new round
- `test_tool_messages_before_round_reset_on_complete` — verifies Complete handler drains tool messages before resetting round, giving them the correct last-round index instead of 0
- `test_three_round_tool_call_ordering` — full 3-round cycle: Streaming(0) → Tools(1) → InterToolText(2) → Tools(2) → InterToolText(3), verifying all round_index values and message ordering
- `test_tool_messages_positioned_before_next_round_content` — verifies tool messages from round N appear before round N+1 content inserted via `insert_at_round_boundary`
- `test_stream_done_drains_tool_messages_before_final_response` — verifies that tool messages from the last round appear before the final response message, simulating the StreamDone drain behavior

**Manual test:** completed via `tu` terminal debugger with glm-5.1 model — verified multi-round web search, pre-tool content preservation, and StreamDone drain ordering.

**Merged:** PR #202 merged 2026-06-07. Two root causes fixed: (1) drain tool messages with correct round_index before round increment/reset, (2) `finalize_streaming_zone_as_is()` converts ALL `AssistantStreaming` blocks to preserve pre-tool content. No release blocker remains.

**Related:** Issue #118 (tool trait), #119 (agnostic types — `ChatRound`/`ChatCycle` may live in `src/llm_provider/types.rs` after #119), issue #199 (multi-model validation, may be subsumed by #201 manual test).

---

### Auto Fact Extraction (autoDream-lite) — #73 (CLOSED)

**Status:** ✅ COMPLETED  
**Depends on:** P0 (Factual Memory System — completed)  
**Estimated effort:** 3-5 days (original) + 2 days (bug fixes)

**Implementation summary:**

Key files:
- `src/facts/dedup.rs` — Centralized dedup pipeline (`DedupResult`, `DedupConfig`, `deduplicate_and_insert()`), single source of truth for all 3 callers
- `src/facts/extract.rs` — Heuristic extraction, thin dedup wrapper (delegates to `dedup::deduplicate_and_insert()`), validation
- `src/facts/lang.rs` — Centralized EN/PT patterns, PT→EN translation, `normalize_to_storage_format()` (ADR-E4), `normalize_for_comparison()` (Lemma strip), `normalize_adverb_verb()` (adverb expansion), `lemmatize_verb()` (3rd person → base form)
- `src/facts/conflict.rs` — Conflict detection, preference override, lowered threshold
- `src/facts/db.rs` — FTS5 search, exact match, normalized match, BM25 scoring
- `src/facts/prompt.rs` — System prompt scope separation (Global/Project), defense-in-depth normalization
- `src/facts/types.rs` — Global scope forces project_id=None
- `src/tools/fact_tools.rs` — LLM tool with validation + thin dedup wrapper (delegates to `dedup::deduplicate_and_insert()`)
- `src/chat/repl.rs` — Async `try_auto_extract_facts()` passes embedding_client for Layer 3.5
- `src/chat/command_handlers.rs` — `/fact add` CLI with validation + thin dedup wrapper (delegates to `dedup::deduplicate_and_insert()`)
- `src/embeddings/client.rs` — Semaphore(1) for serialized embedding requests, 30s timeout

**Architecture: Six-layer dedup pipeline:**
1. **Layer 1: Exact content match** — case-insensitive, trimmed comparison via `find_exact_fact()`
2. **Layer 2: Normalized content match** — `normalize_for_comparison()` strips pronouns/subjects and lemmatizes verbs (3rd person → base form), catches "I prefer X" ≈ "User prefers X" ≈ "prefers X" → all normalize to "prefer X"
3. **Layer 3.5: Semantic embedding (insert-time)** — cosine similarity ≥ 0.70 (`SEMANTIC_SEARCH_THRESHOLD` in conflict.rs). Runs BEFORE Layer 3 (FTS5). Triple-based disambiguation: `extract_fact_triple()` distinguishes contradictions (same predicate, different object → Update) from duplicates (same triple → Skip) from related facts (different predicate → fall through). `is_contradiction()` fallback catches polarity opposition (like/hate, negation). Covers `Category::Preference` (includes identity facts).
4. **Layer 3: FTS5 BM25 search** — keyword matching with threshold 0.75 (lowered from 0.85)
5. **Layer 4 (startup): Semantic verification** — `verify_and_dedup_facts()` O(n²) cosine comparison at threshold 0.90
6. **Global-wins-project** — When a Global-scope fact conflicts with an existing Project-scope fact, the Global fact wins and the Project fact is removed

**Bug fixes (from smoke test #1):**
- Bug #1: Dedup broken — Fixed with three-layer pipeline, exact match, normalized match, threshold 0.75
- Bug #2: PT→EN inconsistent — Fixed with expanded `translate_pt_to_en()` (3rd-person PT, hybrid LLM forms), `fact_add` English-only instruction
- Bug #3: `/fact list` scope — Fixed with `FactListScope::All/Global/Project`, separate sections
- Bug #4: Non-fact validation — Fixed with `is_extractable_sentence()` in `fact_add`
- Bug #5: PT commands — Fixed with `command_starters()` check in `fact_add`
- Bug #1/6: Global project_id — Fixed with `Fact::new()` forcing `project_id=None` for Global scope
- Scope separation — System prompt groups facts by scope (Global Preferences/Facts, then Project)
- Global-wins-project — New Global fact removes conflicting Project facts
- Preference override — "prefer dark mode" vs "prefer light mode" detected as contradiction

**Bug fixes (from smoke test #2):**
- Bug #1: Adverb modifier normalization — `normalize_adverb_verb()` in `lang.rs` handles EN patterns like "I really like X" → "User really likes X" and PT patterns like "Eu sempre prefiro X" → "User always prefers X" via regex expansion after static prefix lists fail. Covers 15 EN adverbs × 8 verbs + 13 PT adverbs × 6 verbs + negation ("I usually don't like" → "User usually doesn't like"). Falls through to no-change if pattern doesn't match.
- Bug #2: Layer 2 verb lemmatization — `normalize_for_comparison()` now lemmatizes third-person verbs after stripping the subject: "prefers dark mode" → "prefer dark mode" matches "prefer dark mode". Added `VERB_LEMMAS` constant and `lemmatize_verb()` function with explicit lemma map + generic trailing-'s' rule with 'ss' guard.
- Bug #3: `/fact add` CLI dedup parity — `handle_fact_add()` in `command_handlers.rs` now calls `normalize_to_storage_format()` (ADR-E4), performs Layer 1 (exact match) and Layer 2 (normalized match) dedup before FTS5, performs Layer 3.5 semantic contradiction detection when embedding client is available, and eagerly generates embeddings after insertion. Changed from synchronous `fn` to `async fn`. Previously, `/fact add` stored raw user input without normalization, used only FTS5 dedup, and never generated embeddings (`has_embedding=0` until startup recovery).
- Bug #4: Layer 3.5 testability documentation — Added SMOKE_TEST.md sections 21.14 (`/fact add` dedup parity test) and 21.15 (`/tools` toggle for auto-extraction-based Layer 3.5 testing). The `/tools` command disables LLM tool calls, forcing contradiction detection through the auto-extraction path, making Layer 3.5 independently testable.

**Bug fixes (from smoke test #3):**
- Bug S42.4/S43.1: "prefer dark mode" + "prefer light mode" coexist — Layer 3.5 triple-based contradiction detection added. `FactTriple` struct and `extract_fact_triple()` in `conflict.rs` extract (subject, predicate, object) triples from storage-format facts. When the semantic search (cos ≥ 0.70) finds similar candidates, triple disambiguation distinguishes contradictions (same predicate, different object → Update) from duplicates (same triple → Skip). Pattern constants `TRIPLE_PREFERENCE_PREFIXES` and `TRIPLE_IDENTITY_PREFIXES` in `lang.rs` serve as source of truth. Covers preference overrides, identity changes, and adverb+verb combos. Zero ML, sub-millisecond.
- Bug S42.4 ROOT CAUSE: sqlite-vec L2 vs cosine metric mismatch — `search_facts_semantic()` computed `similarity = 1.0 - distance`, which is only correct for cosine distance. sqlite-vec defaults to L2 distance; the correct conversion is `1.0 - (L2² / 2.0)`. The broken formula caused ALL similarity scores to be ~0.25–0.35 too low, making the entire Layer 3.5 pipeline non-functional. Fixed in `facts/db.rs`, `content/db.rs`. *Discovered by Hermes Agent.*
- Bug S42.4 race condition: async embedding missing on Layer 3.5 search — Fire-and-forget `tokio::spawn` for embedding generation meant fact #2's search couldn't find fact #1's embedding. Fixed by making embedding generation synchronous (await). Also changed gate from `Category::Preference` to `extract_fact_triple().is_some()`.
- Bug #4: Missing replacement fact insertion — In `command_handlers.rs`, after deleting old fact in contradiction path, `return;` skipped inserting the replacement. Both triple and polarity paths affected. Fixed with explicit `Fact::new()` + `db.insert_fact()` + sync embedding. *Discovered by Hermes Agent.*
- Bug #5: Accumulative predicates false positives — `FactTriple::contradicts()` treated ALL same-predicate pairs as contradictions, so "likes Python" vs "likes Rust" was incorrectly flagged. Fixed with two-tier logic: exclusive predicates (prefers, name is, lives in) → any different object = contradiction; accumulative predicates (likes, loves, hates, uses) → only if `object_word_overlap()` > 0.3 ("likes dark mode" vs "likes light mode" shares "mode" → contradiction; "likes Python" vs "likes Rust" shares nothing → coexist). Added `EXCLUSIVE_PREDICATES`, `POSITIVE_PREDICATES`, `NEGATIVE_PREDICATES`, `STOP_WORDS` constants in `lang.rs`; `is_exclusive_predicate()`, `is_polarity_flip()`, `object_word_overlap()` in `conflict.rs`. Enforcement test `test_all_predicates_classified` guarantees all labels are classified. *Discovered by Hermes Agent.*
- Bug ADR-E4 (PT identity): PT identity facts stored in first person — `translate_pt_to_en()` generated "My name is Ana" and "I live in São Paulo" instead of "User's name is Ana" and "User lives in São Paulo". Fixed by changing PT identity outputs in `translate_pt_to_en()` to third-person English. Now consistent with EN identity normalization.
- `normalize_for_comparison()` identity prefix "i am a " added — "I am a developer" now correctly strips full prefix including article, consistent with "User is a developer".

**ADR References:**
- ADR-L1: All fact content stored in English (PT→EN via `lang::translate_pt_to_en()`)
- ADR-L2: Normalization output always English ("User prefers" not "User prefere")
- ADR-L3: EN+PT classification keywords in `lang::preference_keywords()`
- ADR-L4/L5: All string patterns centralized in `lang.rs`, no duplication
- ADR-E4 (revised): Third-person normalization applied at storage time (not just render time). All facts stored as "User prefers X". `normalize_to_third_person()` in prompt rendering remains as defense-in-depth.

**Phase 2 (P6.7, planned):** Embedding-based semantic dedup — ✅ COMPLETED (see P6.7 below)

---

### Batch Document Processing — Improve PDF Ingestion UX — #132 [M1]

**Status:** ❌ NOT STARTED
**Priority:** high
**Depends on:** None
**Estimated effort:** 3-4 days

**Goal:** Reduce PDF/document processing overhead from ~170 tool calls to ~8 by enabling batch OCR in subagent tools and updating the document-processing skill to orchestrate efficiently.

**Problem Statement:**

When processing a PDF with 82 pages via the embedded document-processing skill, the LLM makes ~170 tool calls (82 pdftoppm calls + 82 spawn_ocr_agent calls + metadata checks). This wastes ~160K tokens of orchestration context and provides terrible UX (20-40 minutes of sequential processing). The LLM also attempts to create shell scripts to automate the process, which fails because `run_command` blocks pipes and shell features for security reasons.

**Architecture Decision (ADR-BATCH-1): LLM Stays in Control**

No native PDF pipeline in Rust. The LLM remains in control of what content to import, using `run_command` (whitelist/sandbox/Landlock) for external tool execution. Documents remain "mini articles curated by the LLM" — curated synthesis, not raw text dumps. `FileType::Pdf/Epub` stays removed from `import_document` — PDFs are extracted via `run_command`, then imported as curated text.

**Why not a native Rust pipeline?** Calling `pdftotext`/`pdftoppm` directly from Rust bypasses the security layer (whitelist, sandbox, Landlock) that `run_command` enforces. A malicious binary in the PATH named `pdftoppm` would execute without any validation. Keeping execution through `run_command` preserves the defense-in-depth security model. The trade-off is ~8 tool calls instead of 0, which is acceptable given the security benefit.

**Solution (single delivery, two parts):**

**Part 1 — Batch OCR in subagent tools:**

`spawn_ocr_agent` accepts comma-separated paths (like `spawn_vision_agent` already does). Internally, `SubagentRunner::run_ocr_batch()` iterates over paths sequentially (Ollama `/api/generate` doesn't support multi-image OCR) and concatenates results with `--- Page N ---` separators. This reduces tool calls from 82 to 1-2 and orchestration tokens from ~160K to ~6K.

**Part 2 — Improved document-processing skill:**

Updated instructions that guide the LLM to use batch patterns: convert all visual pages with a single `pdftoppm` call, then pass all paths to a single `spawn_ocr_agent` invocation. Clear heuristics for when to use batch vs. single-page processing. Clarify that `import_document` stores curated knowledge, not raw text.

**Important limitation:** Each page still requires one `/api/generate` call to the OCR model (Ollama API limitation). Batch reduces tool call overhead and orchestration tokens, but does not reduce the number of LLM inference calls. Time improvement comes from UX (single progress indicator) rather than parallelism.

**Cost Analysis:**

| Approach | Tool Calls | Orchestration Tokens | Content Tokens |
|----------|-------------|----------------------|----------------|
| Status quo (1 per page) | ~170 | ~160K | Unchanged |
| Batch subagents (this task) | ~8 | ~6K | Unchanged |

**Implementation Phases:**

| Phase | Description | Files | Status |
|-------|-------------|-------|--------|
| 1.1 | `spawn_ocr_agent` accepts comma-separated paths | `src/tools/subagent_tools.rs` | ❌ |
| 1.2 | `SubagentRunner::run_ocr_batch()` iterates, concatenates results | `src/chat/subagent.rs` | ❌ |
| 1.3 | Progress indicator during batch OCR | `src/chat/subagent.rs` | ❌ |
| 1.4 | Update `spawn_ocr_agent` docstring with batch examples | `src/prompts/tools.rs` | ❌ |
| 1.5 | Update document-processing skill for batch patterns | `src/skills/builtin/document-processing.md` | ❌ |
| 2.1 | Rewrite "Page Selection Strategy" — batch pdftoppm usage | `src/skills/builtin/document-processing.md` | ❌ |
| 2.2 | Heuristics for batch vs single-page processing | `src/skills/builtin/document-processing.md` | ❌ |
| 2.3 | Clarify `import_document` docstring — curated knowledge | `src/tools/documents.rs` | ❌ |
| 3 | Tests (SMOKE_TEST section 14) | `SMOKE_TEST.md` | ❌ |
| 4 | Documentation (CHANGELOG, IMPLEMENTATION.md) | Both | ❌ |

**Existing code leverage:**
- `OcrProcessor::process_batch()` already exists (`src/ocr/processor.rs:93`) — reused by `run_ocr_batch()`
- `spawn_vision_agent` already supports comma-separated paths (`src/tools/subagent_tools.rs:192-197`) — same pattern
- `validate_subagent_paths()` handles multi-path validation (`src/security.rs`) — reused
- Sandbox allows `/tmp` — pdftoppm output in /tmp is within subagent sandbox ✅

**What is NOT in this task:**
- ❌ Native PDF pipeline calling pdftotext/pdftoppm directly from Rust
- ❌ Restoring `FileType::Pdf/Epub` in `import_document`
- ❌ Bypassing `run_command` security layer
- ❌ `TempDir` auto-cleanup (LLM instructed to use /tmp, already sandboxed)
- ❌ Change to document concept (remains "curated mini article")

**Reference:** Hermes Agent research (internal analysis), Issue #9 (Document Import Tool, COMPLETED), SF5 (Agent Spawning Tools, COMPLETED)

**Related:** Issue #132

---

### Context Pinning — #74 [M1]

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** None  
**Estimated effort:** 2-4 days (after research)

**Goal:** Allow users to mark specific messages or decisions as high-priority, preserving them during compaction.

**Current state:** All messages are eligible for compaction. There is no mechanism to pin important context.

**Proposal:** Add `/pin <id>` command and message metadata to exempt specific messages from compaction.

**Open questions:**
- How many pins are reasonable? Unlimited pins could fill context.
- Should pins expire? Or require manual unpin?
- How does pinning interact with the compaction algorithm in `src/chat/core.rs`?
- UI: how does user see what's pinned?

**Related:** Issue #74

---

### Dynamic Context Limits — #75 [M1]

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** None  
**Estimated effort:** 1-2 days (after research)

**Goal:** Calculate tool operation limits (max_lines, max_tokens for results) dynamically based on remaining context, instead of using fixed constants.

**Current state:** Some limits already adapt (pre-tool warning in `custom_coordinator.rs`), but `max_lines` in `read_file` and tool result truncation use fixed values.

**Open questions:**
- What's the right formula? Context_remaining - buffer = available_for_tool?
- Need more research to understand the full complexity.

**Related:** Issue #75

---

### Secret Scanning (Content) — #76 [M1]

**Status:** 📋 PLANNED  
**Depends on:** Existing `files_blocklist.rs` (path-based scanning)  
**Estimated effort:** 1-2 days

**Goal:** Scan file CONTENT for credential patterns (AWS keys, GitHub tokens, OpenAI keys, SSH private keys) before write operations, extending the existing path-based blocklist.

**Current state:** `src/tools/files_blocklist.rs` blocks writing to sensitive FILE PATHS (`.env`, `id_rsa`, etc.). However, file CONTENT is not scanned — writing `AKIAIOSFODNN7EXAMPLE` to `notes.txt` would succeed. This is an evolution of the blocklist concept, not a new security layer.

**Proposal:** Add `scan_content_for_secrets(content: &str)` function that checks content against 25+ credential patterns before allowing write operations.

**Security rule:** Never log or display secret values — only show rule ID and label (e.g., "AWS Access Key detected").

**Related:** Issue #76

### Config Upgrade Command — #105 [M1]

**Status:** 📋 PLANNED
**Depends on:** None
**Estimated effort:** 5 days
**Issue:** #105 (canonical — P6.5 and P1 #105 are the same task)

**Note:** P6.5 and P1 #105 describe the same feature. Use #105 as the canonical issue. See P1 section for full implementation details.

#### Sub-item: `--skin` CLI Override for Chat

Add `--skin` CLI flag to `sprach chat` that overrides `[display] skin` from config.toml for the current session only.

**Priority:** CLI `--skin` > config.toml `[display] skin` > default `"dark"`

**Scope:** ChatArgs-only (not global `Cli`). No effect on query, translate, summarize, vision, or ocr.

**Estimated effort:** 0.5 day

**Behavior:**
- `sprach chat --skin light` → Uses light theme (Catppuccin Latte) for this session
- `sprach chat --skin mono` → Uses monochrome theme (no colors)
- `sprach chat --skin dark` → Explicitly uses dark theme (same as default)
- `sprach chat` → Uses whatever `[display] skin` is in config.toml, or `"dark"` default
- Invalid skin value → Warning to stderr, falls back to `"dark"`

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `VALID_SKINS` constant + `resolve_skin()` in `src/settings.rs` | 📋 |
| 2 | Add `pub skin: Option<String>` to `ChatArgs` in `src/chat/cli.rs` | 📋 |
| 3 | Add `resolved_skin: String` to `ReplState` + builder in `src/chat/repl_state.rs` | 📋 |
| 4 | Resolve skin in `run_chat_repl()` (CLI > config > default) in `src/chat/repl.rs` | 📋 |
| 5 | Use `state.resolved_skin` instead of `state.settings.display.skin` in `src/chat/repl_tui.rs` | 📋 |
| 6 | Update documentation (`configuration.md`, `commands/chat.md`) | 📋 |

**Files to Modify:** `src/settings.rs`, `src/chat/cli.rs`, `src/chat/repl_state.rs`, `src/chat/repl.rs`, `src/chat/repl_tui.rs`, `doc/src/configuration.md`, `doc/src/commands/chat.md`

**Files NOT Modified:** `src/main.rs` (not a global flag), `src/chat/tui/markdown.rs` (already accepts `&str`), non-chat subcommand CLIs

---

### Fact Embeddings & Semantic Dedup — #108 (CLOSED)

**Status:** ✅ COMPLETED [M1]
**Depends on:** P6.1 (Auto Fact Extraction — completed)
**Estimated effort:** 5-7 days (completed)

**Goal:** Add embedding-based semantic dedup as Layer 3.5/4 on top of the existing dedup pipeline, enabling reliable detection of semantically equivalent facts regardless of phrasing, language, or subject form.

**Architecture: Six-layer dedup pipeline:**
1. **Layer 1: Exact content match** — case-insensitive, trimmed comparison via `find_exact_fact()`
2. **Layer 2: Normalized content match** — `normalize_for_comparison()` strips pronouns/subjects
3. **Layer 3.5: Semantic embedding (insert-time)** — cosine ≥ 0.70, runs BEFORE FTS5; triple disambiguation + `is_contradiction()` fallback
4. **Layer 3: FTS5 BM25 search** — keyword matching with threshold 0.75
5. **Layer 4 (startup): Semantic verification** — `verify_and_dedup_facts()` O(n²) pairwise cosine at threshold 0.90
6. **Global-wins-project** — Global-scope facts override conflicting Project-scope facts

**Schema changes (v10 → v11):**
- Added `has_embedding INTEGER DEFAULT 0` column to `facts` table
- Added `fact_embeddings` vec0 virtual table (256d Matryoshka, same model as content embeddings)
- Added `idx_facts_embedding` partial index on `has_embedding WHERE has_embedding = 0 AND invalidated_at IS NULL`

**Schema changes (v11 → v12):**
- All 3 vec0 tables now use `distance_metric=cosine` (was default L2)
- Migration drops and recreates vec0 tables, resets `has_embedding` flags for startup recovery
- Application-level L2→cosine conversion removed: `1.0 - (distance²/2)` → `1.0 - distance`

**New modules:**
- `src/facts/embedding.rs` — `generate_fact_embedding()` wrapper around `EmbeddingClient::embed()`
- `src/facts/recovery.rs` — `recover_missing_fact_embeddings()` + `flush_pending_fact_embeddings()` for startup/shutdown
- `src/facts/verify.rs` — `verify_and_dedup_facts()` with O(n²) pair-wise cosine similarity comparison at threshold 0.90

**New DB methods:**
- `update_fact_embedding()` — Insert into `fact_embeddings` vec0, set `has_embedding = 1`
- `search_facts_semantic()` — KNN search via vec0, filter by scope
- `get_facts_for_reindex()` — Find facts with `has_embedding = 0`
- `delete_fact()` now also removes from `fact_embeddings`

**Embedding lifecycle:**
- **Eager (insert-time):** After `insert_fact()` in both auto-extraction and `fact_add`, `EmbeddingClient::embed()` generates embedding synchronously via `Semaphore(1)` (serialized, 30s timeout). If Ollama offline, `has_embedding = 0` and startup recovery catches up.
- **Startup recovery:** `recover_missing_fact_embeddings()` — generates embeddings for all facts with `has_embedding = 0`, then verifies no facts remain without embeddings (logs warning if any still missing).
- **Startup verification:** `verify_and_dedup_facts()` — pair-wise cosine comparison, resolves duplicates/contradictions/global-wins-project.
- **Shutdown:** `flush_pending_fact_embeddings()` — completes pending embedding generation before exit.

**Startup sequence:**
```
recover_missing_embeddings()           ← Content embeddings (existing)
recover_missing_fact_embeddings()      ← Fact embeddings (NEW)
verify_and_dedup_facts()               ← Semantic dedup (NEW)
```

**Conflict resolution (semantic):**
- Duplicate (cos ≥ 0.90, no contradiction) → Keep newer, remove older
- Contradiction (cos ≥ 0.90, with `is_contradiction()`) → Keep newer, remove older
- Global-wins-project → Global fact removes Project duplicate

**Silent by design:** All startup/shutdown operations use `log::info/debug` only; no visual output unless errors occur.

**Re-exports:** `EmbeddingError` and `cosine_similarity` now re-exported from `embeddings` module for use by fact modules.

**Bug discovered (2026-04-26):** sqlite-vec L2 vs cosine metric mismatch — `search_facts_semantic()` used `1.0 - distance` (only correct for cosine distance), but sqlite-vec defaults to L2 distance. Fixed to `1.0 - (distance² / 2.0)` for L2-normalized vectors. The same bug existed in `content/db.rs` for content and chunk search. Also fixed comparison direction in `content/db.rs:790` (`<` → `>`, highest cosine wins). This was the root cause of S42.4/S43.1 — the entire Layer 3.5 pipeline was non-functional because all similarity scores were ~0.25–0.35 too low. **Phase 2 fix:** Schema v12 added `distance_metric=cosine` to all vec0 tables, eliminating the application-level conversion entirely. *Discovered by Hermes Agent.*

**Bug discovered (2026-04-26):** Ascending sort in `search_content_semantic()` — results were sorted ascending by score (least similar first), then truncated. This inverted RRF ranking: the least similar semantic result received the highest RRF weight. Changed to descending sort (most similar first) so rank 1 = best match.

**Bug discovered (2026-04-26):** Accumulative predicates false positives — `FactTriple::contradicts()` treated all same-predicate pairs as contradictions, so "likes Python" vs "likes Rust" was incorrectly flagged. Fixed with two-tier logic: exclusive vs accumulative predicates + `object_word_overlap()` for same-category detection. Known limitation: "likes vim" vs "likes emacs" (no word overlap) is not a contradiction — deferred to Phase 2 (LLM adjudication). *Discovered by Hermes Agent.*

**Refactoring (2026-04-27):** Centralized fact dedup pipeline into `src/facts/dedup.rs`. The three insertion callers (`command_handlers.rs`, `fact_tools.rs`, `extract.rs`) previously duplicated ~65-75% of the dedup pipeline logic, diverging in behavior. Created `DedupResult` enum (7 variants: Inserted, ExactDuplicate, NormalizedDuplicate, SemanticDuplicate, Updated, Fts5Conflict, Error), `DedupConfig` struct, and `deduplicate_and_insert()` as the single source of truth. Each caller is now a thin wrapper that formats `DedupResult` for its UI. This fixes 4 behavioral bugs in the LLM tool path: (1) threshold 0.90→0.70, (2) missing triple disambiguation, (3) Layer 3.5 after Layer 3, (4) fire-and-forget embedding. Net line reduction: -1229. Removed `Fact::for_insert()` (dead code).

**Related:** Issue #73

**Goal:** Add a `sprach config upgrade` subcommand that merges missing default fields into the user's existing `config.toml`, adding doc comments only for new fields. Users don't have to manually track which config fields are new after each update.

**Problem:**
- Every release adds new config fields (`[feedback]` in v0.40, `[facts]` in v0.42)
- `serde(default)` silently fills missing fields — no user-visible indication
- Users must read CHANGELOG to discover new fields and add them manually
- `--init-config` creates a full config, but doesn't merge with existing

**Solution:** Two-pass approach using `toml_edit` (comment preservation) + `toml` (value parsing):

```
sprach config upgrade [--dry-run] [--backup]
```

1. Read user's `config.toml` with `toml_edit::DocumentMut` (preserves comments and formatting)
2. Parse with `toml::from_str::<Settings>()` to detect which fields are present
3. Compare against `Settings::default()` to find missing fields
4. Insert missing fields with doc comments using `toml_edit`
5. Write back, preserving all existing content

**Design Decisions:**
- Insert-only: never modify existing fields or comments
- Cannot distinguish "explicitly set to default" from "missing" — acceptable limitation
- Comments come from a static const map keyed by field path
- Backup file created before upgrade (`config.toml.bak`)
- `--dry-run` flag shows what would be added without modifying

**New Files:**
- `src/commands/config_upgrade.rs` — `ConfigUpgrader` struct with upgrade algorithm

**New Dependency:**
- `toml_edit = "0.25"` — parse/write TOML with comment preservation

**Related:** Issue #105

---

### Content Staleness Indicators — #96 [M1]

**Status:** ✅ COMPLETED (v0.39.5)  
**Estimated effort:** 0.5 day

**Goal:** Inject staleness warnings into the facts prompt when facts are old.

**Current state:** `src/facts/prompt.rs` formats facts without age indicators. Facts with `last_accessed` > 30 days may be outdated but are presented with the same confidence as fresh facts.

**Implementation:**

Added `get_staleness_label()` function in `src/facts/prompt.rs` with priority-based labels:
- `(stale)` — when `decay_score < 0.3` (badly decayed)
- `(N days ago)` — when `last_accessed` > 30 days (not recently used)
- `(unused)` — when `access_count == 0` and age > 7 days (never retrieved)
- No label for fresh facts (avoids noise)

Modified `build_facts_section()` to append staleness label after fact content:
```rust
for fact in preferences {
    let staleness = get_staleness_label(fact);
    section.push_str(&format!("- {}{}\n", fact.content, staleness));
}
```

**Complexity:** Very low — single file change in `src/facts/prompt.rs`.

**Related:** Issue #70

---

### Truncation Warnings in Tool Outputs — #96 related [M1]

**Status:** ✅ COMPLETED (v0.39.5)  
**Estimated effort:** 0.5 day

**Goal:** Add explicit truncation metadata in tool outputs when file reads or search results are limited.

**Current state:** `read_file` with `max_lines` silently truncates. No `[TRUNCATED]` indicator in output.

**Implementation:**

Modified truncation handling across three files:

1. **`src/tools/files.rs`** — `read_file`:
   - Added `[TRUNCATED: Showing lines 1-N of M. Use read_file_segment to read more.]` when `max_lines` truncates output
   - Calculates `total_lines` before truncation to include total count
   - Only appends notice when actually truncated (skips if `max_lines >= total_lines`)

2. **`src/tools/files.rs`** — `search_files`:
   - Changed from `... (stopped after N matches)` to `[TRUNCATED: Showing N matches. Refine your search pattern for fewer results.]`

3. **`src/tools/remember.rs`**:
   - Added `REMEMBER_NOTE_PREVIEW_CHARS` (150), `REMEMBER_MESSAGE_PREVIEW_CHARS` (200), `REMEMBER_SUBMESSAGE_PREVIEW_CHARS` (100) constants
   - Notes/docs: `[TRUNCATED: 150 of N chars. Use remember(id="note:X") for full content.]`
   - Messages: `[TRUNCATED: 200 of N chars. Use remember(id="msg:X") for full content.]`
   - Sub-messages: `[+N chars]` (no retrievable ID, so simplified format)
   - All truncation uses Unicode-safe `.chars().take()` pattern

**Complexity:** Low — modify output formatting in `read_file`, `search_files`, and `remember`.

**Related:** Issue #71

---

### ✅ Bug: Embeddings Fail on Startup When Input Exceeds Context Window [M1]

**Status:** ✅ COMPLETED (Issue #40, PR #102, merged 2026-04-24)

**Complements:** PR #46 (Issue #40) — PR #46 fixed the fallback architecture; PR #102 fixed residual robustness issues.

**Goal:** Fix embedding generation failures when content exceeds the embedding model's context window during startup regeneration/recovery.

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Proactive context length check in `embed()` | ✅ Done |
| 2 | Cache `context_length` in `EmbeddingClient` via `OnceCell` | ✅ Done |
| 3 | Handle `ContextExceeded` variant in fallback match arms | ✅ Done |
| 4 | Replace `panic!` with graceful degradation in `regenerate.rs` | ✅ Done |
| 5 | Consistent empty content validation in `recovery.rs` | ✅ Done |
| 6 | Fix `has_embedding` marking logic | ✅ Done |
| 7 | Increased safety margins (CONTEXT_SAFETY_MARGIN 10%→20%, EMBEDDING_PREFIX_TOKENS 20→30, DEFAULT_CHUNK_PERCENT 90%→80%, DEFAULT_PREFIX_MARGIN 30→40) | ✅ Done |
| 8 | Documented token estimation limitations (ollama-rs v0.3.4 ignores `prompt_eval_count`) | ✅ Done |

**Files Modified:**
- `src/embeddings/client.rs` — Proactive context check, cached context length, API error → ContextExceeded conversion, increased safety margins, documented estimation limitations
- `src/embeddings/fallback.rs` — Handle `ContextExceeded` variant in both fallback paths
- `src/embeddings/regenerate.rs` — Replace panic with graceful degradation
- `src/embeddings/recovery.rs` — Add empty content validation, fix has_embedding marking
- `src/content/db.rs` — New `mark_item_embedding_if_complete()` method
- `src/embeddings/chunk_config.rs` — Reduced chunk percent (90%→80%), increased prefix margin (30→40)
- `SMOKE_TEST.md` — Section 4.3 for embedding startup resilience
- `.opencode/skills/` — Updated next-demand, pr-workflow, pr-testing, release-process with duplicate checks and card management

**Related:** Issue #40 (canonical), Issue #39 (duplicate, closed), PR #102, Issue #103 (future: exact token counts via reqwest)

---

## 🔵 LOW PRIORITY: Extended Features

Features planned for future releases:

| Priority | Feature | Description | Dependencies | Issue |
|----------|---------|-------------|--------------|-------|
| P8 | File Session State | Explicit file tracking | None | #13 |
| P9 | Skills System Extended | Multilingual sanitization, security enhancements | Skills System, Specialized Agents | #14 |
| P10 | File Staleness | Detect outdated file content | None | #50 |
| P11 | Extended Personalities | Per-personality model config | None | #49 |
| P12 | Plugin System | User-defined tools | None | #15 |
| P13 | TUI | Ratatui-based terminal interface | None | #16 |
| P14 | Memory Enhancement 2-5 | Query routing, filtering | Doc Import | #17 |

**Note:** OCR/Vision Tools Integration was merged into Priority 4 (Specialized Agent Architecture).

---

### File Session State — #13 [M1]

**Status:** ❌ NOT STARTED

**Goal:** Explicit file tracking for session context.

**Related:** Issue #13

---

### Extended Personalities System — #49 [M1]

**Status:** ❌ NOT STARTED

**Goal:** Per-personality model configuration and separate memory context.

**Reason for Priority:** Didactic use case requires separate personalities soon.

**Current State (SOUL.md):**
- Multiple personality files supported via symlinks
- Symlink approach: `ln -sf ~/.config/sprachspiel/SPRACH.md ~/.config/sprachspiel/SOUL.md`

**What's MISSING:**
- Per-personality model configuration
- Separate memory context per personality
- Personality directory support (`personalities/`)

**Dependencies:** None

**Estimated effort:** 2-3 days

**Related:** Issue #49

---

### Multilingual Skill Sanitization — #14 [M1]

**Status:** ❌ NOT STARTED

**Goal:** Enhanced security for multilingual skill content.

**Background:**
- Skills System (P3) uses English-only sanitization
- Multilingual prompt injection can bypass English-based detection (documentated in research)
- Specialized Agent Architecture (P4) enables translate functionality within chat sessions

**Features:**

| Feature | Description | Dependency |
|---------|-------------|------------|
| **Language Detection** | Detect non-Latin characters, log warnings | None (can implement now) |
| **Translate-then-Detect** | Translate non-English content, then scan | P4 (Specialized Agents) |
| **ML Detection** | XLM-RoBERTa fine-tuned (optional) | ML infrastructure |
| **LLM-as-Critic** | Second LLM reviews before loading | Token costs |

**Implementation Phases:**

| Phase | Description | Dependency |
|-------|-------------|------------|
| 1 | Language detection + warning | None ✅ |
| 2 | Translate-then-detect | P4 |
| 3 | ML model (optional) | Future |

**Research:**
- HackerNoon: Multilingual prompt injection bypasses Azure Content Filter
- arXiv:2512.23684: Hidden prompt injection in 500 ICML papers
- arXiv:2410.21337v1: XLM-RoBERTa achieves 99% accuracy

**Dependencies:**
- Skills System (P3) ✅
- Phase 2 requires P4 (Specialized Agent Architecture)

**Estimated effort:** Phase 1: 2-3 hours | Phase 2: TBD

**Reference:** `doc/src/development/skills-system-design.md` → Future Considerations

**Related:** Issue #14

---

### Skills Management Tool — #52 [M1]

**Status:** ❌ NOT STARTED

**Goal:** Allow LLM to create, modify, and delete skills automatically.

**Background:**
- Skills System (P3) provides read-only access via `skill_list()` and `skill_view()`
- LLMs often discover repeatable workflows that should be captured as skills
- Hermes Agent shows successful pattern with `skill_manage()` tool

**Scope (MVP):**

| Action | Parameters | Description |
|--------|------------|-------------|
| `create` | `name`, `content` | Create new skill with SKILL.md |
| `patch` | `name`, `old_string`, `new_string` | Find-and-replace in skill |
| `delete` | `name` | Remove skill directory |

**NOT in MVP:**
- `edit` (full rewrite) - use patch
- `write_file/remove_file` (supporting files) - references can wait
- Skills Hub integration (community skills) - users install manually
- Categories - can wait

**Architecture:**

```
src/skills/
├── manager.rs       # NOVO: create_skill, patch_skill, delete_skill
├── loader.rs        # ✅ load_skill_indexes, get_skill_content
├── sanitize.rs      # ✅ validate_skill_file, is_valid_skill_name
└── types.rs         # ✅ Skill, SkillIndex, Frontmatter

src/tools/skill_tools.rs
├── skill_list()     # ✅
├── skill_view()     # ✅
└── skill_manage()   # NOVO
```

**Security:**

1. Builtin skills are protected (cannot edit/delete)
2. `old_string` must be unique (or `replace_all=true`)
3. Name validation: `[a-z0-9_-]` only (no path traversal)
4. Atomic writes (tempfile + rename, never partial writes)
5. Frontmatter validation (name + description required)
6. Max size: 256KB (same as read)

**Directories:**

```
~/.config/sprachspiel/skills/          ← User skills (writable)
PROJECT/.sprachspiel/skills/           ← Project skills (writable)

Priority for writes: project > user (same as reads)
Priority for deletes: user only (cannot delete project from CLI)
```

**Estimated effort:** 3-4 hours

**Dependencies:** Requires P3 (Skills System) ✅ COMPLETED

**Related:** Issue #52

---

### File Staleness Detection — #50 [M1]

**Status:** ❌ NOT STARTED

**Goal:** Prevent file edits based on outdated content.

**Problem:**
When the LLM edits a file using `edit_file` or `write_file`, it may operate on outdated content if:
1. The file was modified externally (by another process, user, or git operations)
2. The LLM's context contains stale information about the file's structure

**Proposed Solution:**
- Track modification time (mtime) when a file is read
- Before edit operations, compare current mtime with stored mtime
- If different, return warning: "File has been modified since it was read."

**Dependencies:** None

**Estimated effort:** 1-2 days

**Related:** Issue #50

---

### 🔴 PRIORITY: Responsive Chat Rebuild with Ratatui [M1]

**Status:** ✅ COMPLETED (W6-PR3: Streaming Refinement + Tab Completion + Intelligent Table Reflow + Textarea Integration + Chat Selection)

**Goal:** Rebuild the chat REPL using Ratatui as the rendering framework to achieve responsive layout that adapts to terminal width. Replace the current `println!` + hardcoded ANSI approach with a declarative rendering model.

**Motivation:** The current chat interface only renders correctly at 80 columns. Any terminal resize produces broken output — truncated banners, overflow status bars, misaligned markdown. The root cause is raw `println!` + ANSI escapes scattered across 600+ call sites with hardcoded widths. Ratatui solves this by treating the terminal as a draw surface with responsive layout constraints.

**IMPORTANT:** This is NOT the full TUI (#16). This is a responsive chat mode rebuild — same UX as current chat, but rendered via Ratatui. The full TUI (sidebars, /queue, /steer, multi-pane) remains M2 issue #16 and is NOT in scope.

**Why Now (M1):** The chat is the primary user interface. Responsive rendering is a prerequisite for any future TUI work (#16, #117), and the architecture already has `ChatView` and `InputBackend` traits designed for this migration. Delaying means building more println-based features that will need rewriting later.

**Milestone:** M1 (Core Evolution) — high priority, after critical bugs.

**Delivery Model:** 4 sequential PRs, each leaving the codebase functional and testable.

---

#### PR 1: CommandResult — Decouple Logic from Presentation (~5-6 days) — #145

**Goal:** Migrate all command handlers from direct `println!`/`eprintln!` to typed `CommandResult` enum, with rendering via `ChatView`.

**Current Problem:** `command_handlers.rs` has 336 direct print calls with embedded ANSI codes. This makes TUI migration impossible because the output format is baked into the logic.

**Solution:** Create `CommandResult` enum and route all output through `ChatView`.

**Branch:** `refactor/command-result-decouple`

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1.1 | Create `src/chat/command_output.rs` with `CommandOutput` enum, data structs, and `ChatView::show_command_output()` | ✅ COMPLETED |
| 1.2 | Migrate all command handlers from `println!`/`eprintln!` to `Vec<CommandOutput>` | ✅ COMPLETED |
| 1.3 | Migrate REPL loop to consume `CommandOutput` via `ChatView::show_command_outputs()` | ✅ COMPLETED |
| 1.4 | Migrate `repl.rs` startup/error/status messages to `ChatView` methods | ✅ COMPLETED |
| 1.5 | Migrate `core.rs` response rendering to `ChatView` (thinking, markdown, tokens, compaction) | ✅ COMPLETED |
| 1.6 | Migrate `thinking.rs` to `ChatView::show_thinking()` + `extract_thinking()`; add `show_warning()`, `show_progress()` convenience methods | ✅ COMPLETED |
| 1.7 | Add module-level `#![expect(print)]` to CLI modules (terminal.rs, repl.rs, thinking.rs, core.rs) | ✅ COMPLETED |
| 1.8 | Remove crate-level `#![expect(print)]` from `lib.rs` | ✅ COMPLETED |
| 1.9 | Migrate `search.rs` → `SearchOutcome` (return data, not print) | ✅ COMPLETED |
| 1.10 | Migrate `session.rs` warnings → `log::warn!` | ✅ COMPLETED |
| 1.11 | Migrate `model_switch.rs` warning → `warnings.push()` | ✅ COMPLETED |
| 1.12 | Migrate `setup_coordinator` callback → `mpsc` channel (`ViewEvent`) | ✅ COMPLETED |
| 1.13 | Migrate `repl.rs` help line → `ChatView::show_help_line()` | ✅ COMPLETED |
| 1.14 | Migrate `core.rs` continuation clear → `ChatView::clear_continuation_line()` | ✅ COMPLETED |
| 1.15 | Keep `display_thinking()` for query mode (documented, not removed) | ✅ COMPLETED |
| 1.16 | Add module-level `#![expect(print)]` to all non-chat modules (~31 files) | ✅ COMPLETED |

**Completed Items — W6-PR1 Full Scope:**

All previously deferred items have been completed as part of W6-PR1.
The `chat/` module now has zero direct print calls except:
- `view/terminal.rs` (75): Rendering layer — intentional, module-level expect
- `repl.rs` (10): Terminal control codes (ANSI positioning, ^C, ^D) — inherent, module-level expect
- `thinking.rs` (4): Legacy `display_thinking()` retained for query mode — module-level expect

All other modules declare their own `#![expect(print)]` with justification comments.
Crate-level `#![expect]` removed from `lib.rs`.

| Phase | Item | Description | Status |
|-------|------|-------------|--------|
| 1.9 | `search.rs` → `SearchOutcome` | Refactor `run_search()` to return `SearchOutcome` enum instead of printing. Renamed `display_results()` → `format_results()` returning String. Wired `handle_search()` to produce `CommandOutput::SearchResults`. | ✅ COMPLETED |
| 1.10 | `session.rs` → `log::warn!` | Replaced 6× `eprintln!("Warning: ...")` with `log::warn!("...")`. Removed `"Warning: "` prefix (log level indicates it). | ✅ COMPLETED |
| 1.11 | `model_switch.rs` → `warnings.push()` | Moved capability detection `eprintln!` into `ModelSwitchResult.warnings` Vec. Caller already renders warnings via `CommandOutput::Warning`. | ✅ COMPLETED |
| 1.12 | `setup_coordinator` → mpsc channel | Created `ViewEvent` enum (`PreToolContent`, `ContextNeedsCompaction`), `ViewEventSender`/`ViewEventReceiver`, `create_view_event_channel()` in `view/mod.rs`. Callback sends events via channel. `drain_into(view)` renders after coordinator call. Eliminated coordinator `eprintln!`. `display_thinking()` retained for query mode only. | ✅ COMPLETED |
| 1.13 | Help line → `ChatView::show_help_line()` | Migrated `print!("{}", WelcomeInfo::help_line())` to `view.show_help_line()`. Added trait method + `TerminalView` impl. | ✅ COMPLETED |
| 1.14 | Continuation clear → `ChatView::clear_continuation_line()` | Migrated `eprint!("\x1B[2K\r")` to `view.clear_continuation_line()`. Added trait method + `TerminalView` impl. | ✅ COMPLETED |
| 1.15 | Keep `display_thinking()` for query mode | Retained for `query/mod.rs` (non-REPL query mode). Updated module doc to clarify it's only for query mode. Coordinator callback now uses `ViewEvent` channel instead. | ✅ COMPLETED |
| 1.16 | Module-level expects everywhere | Added `#![expect(print_stdout)]` and/or `#![expect(print_stderr)]` with justification to ~31 non-chat modules. Removed `#![expect]` from files with only test/print-in-doc print calls (5 files). | ✅ COMPLETED |
| 1.8 | Remove crate-level expects | Removed `#![expect(clippy::print_stdout)]` and `#![expect(clippy::print_stderr)]` from `src/lib.rs`. Updated crate-level doc comment to explain new per-module approach. | ✅ COMPLETED |

**TUI Reuse Design Notes:**

- `SearchOutcome` → `RatatuiView` renders `CommandOutput::SearchResults` via tui-markdown widgets
- `ViewEvent` → Same channel pattern reused in `TuiView`. Future `drain_into_tui()` updates Ratatui state instead of calling `ChatView` methods
- `ChatView::show_help_line()` / `clear_continuation_line()` → `RatatuiView` renders help text in chat area / signals full redraw
- Each non-chat module declaring `#![expect(print)]` makes future TUI migration audit trivial: modules without expects have been fully migrated

**Remaining Print Calls After All Phases (justified, intentional):**

- `view/terminal.rs` (~75): Rendering layer — `TerminalView` prints by design
- `repl.rs` (~10): Terminal control codes (ANSI escape, prompt echo, ^C/^D) — inherent to terminal REPL

**Bugs found during PR:**
- ✅ **Commands being sent as messages** — `repl.rs` handled command output but forgot `continue;` after command processing, causing commands to fall through to `handle_user_message()`. Fixed by adding `continue;` after rendering command outputs.

**CommandResult Enum:**

```rust
pub enum CommandResult {
    System(String),              // Info message → chat area
    Error(String),                // Error message → styled red
    FactList(FactListData),      // Structured fact listing
    NoteList(NoteListData),      // Structured note listing
    TodoList(TodoListData),       // Structured todo listing
    SessionInfo(SessionData),    // Session info display
    ContextInfo(ContextData),     // Context metrics display
    DocumentList(DocListData),    // Document listing
    SkillList(Vec<SkillInfo>),   // Skill listing
    FeedbackConfirm(FeedbackData),// Feedback confirmation
    CompactResult(CompactData),  // Compaction result
    ExportResult(String),         // Exported content
    Quit,                        // Exit REPL
    Continue,                    // No output (continue loop)
}
```

**Files to Create:**
- `src/chat/command_result.rs` — `CommandResult` enum + data structs

**Files to Modify:**
- `src/chat/command_handlers.rs` — All 336 print calls → `CommandResult`
- `src/chat/repl.rs` — Consume `CommandResult` via `ChatView`
- `src/chat/core.rs` — Tool display via `ChatView`
- `src/chat/thinking.rs` — Thinking display via `ChatView`
- `src/chat/view/mod.rs` — Add new methods to `ChatView` trait
- `src/chat/view/terminal.rs` — Implement new methods for `TerminalView`
- `src/lib.rs` — Remove crate-level print expects, add module-level to CLI modules

**Checkpoint:** Codebase functions identically to current version, but all output goes through `ChatView` / `CommandResult`.

---

#### PR 2: Responsive Chat Render + CrosstermInput (~8-10 days) — #146

**Goal:** Replace println+ANSI rendering with Ratatui for responsive chat. Replace rustyline with CrosstermInput for basic input. Chat is functional at any terminal width. No feature flag — ratatui is the only chat mode. Non-chat subcommands (query, translate, OCR, summarize) continue using termimad+indicatif.

**Why combined (originally PR2+PR3):** Rustyline and Ratatui are technically incompatible — both require raw mode and terminal control. A "visual-only" PR with rustyline input would not work. Combining rendering+input in PR2 ensures the chat is functionally usable from the start.

**Branch:** `feat/ratatui-infrastructure`

**Architecture:**

```
┌─ App (event loop via tokio + crossterm) ─────────────────┐
│                                                           │
│  crossterm events ─→ AppEvent::Input(key)                │
│  LLM streaming   ─→ AppEvent::LlmToken(text)            │
│  LLM complete    ─→ AppEvent::LlmComplete(response)      │
│  LLM error       ─→ AppEvent::LlmError(error)            │
│  Commands         ─→ AppEvent::CommandOutput(result)       │
│  Terminal resize  ─→ AppEvent::Resize(w, h)               │
│                                                           │
│  App::handle_event() ─→ update state ─→ terminal.draw()  │
│                                                           │
│  Input disabled during LLM processing                     │
│  Spinner in status bar (rattles frames, no indicatif)    │
└───────────────────────────────────────────────────────────┘
```

**Layout (responsive to terminal width):**

```
┌──────────────────────────────────────────────────┐
│  Chat Area (scrollable Paragraph/List)            │
│  - WelcomeInfo, messages, tool calls, thinking     │
│  - Streaming: plain text during, markdown after    │
│  - Width = terminal.width (no hardcoded 80)        │
├──────────────────────────────────────────────────┤
│  ⠋ Thinking... │ llama3.1 │ 47K/128K 37% │ 🧠🔧 │  ← Status bar with spinner
├──────────────────────────────────────────────────┤
│  >>> _                                            │  ← CrosstermInput
└──────────────────────────────────────────────────┘
```

**Spinner in Status Bar:**

When the LLM is processing, the status bar replaces the model name with an animated spinner using `rattles` frames. `indicatif::ProgressBar` is NOT used in chat mode — the spinner is a native ratatui widget.

| State | Status Bar Content |
|-------|-------------------|
| **Idle** | `llama3.1 │ 47K/128K 37% │ 🧠🔧` |
| **Thinking** | `⠋ Thinking... │ 47K/128K │ 🧠🔧` |
| **Streaming** | `llama3.1 │ 49K/128K 38% │ 🧠🔧` |
| **Tool call** | `⠋ Running tool... │ 47K/128K │ 🔧` |

Input is disabled during Thinking/Streaming/Tool call states. Only Ctrl+C cancels.

**Markdown Rendering Strategy:**

| State | Behavior |
|-------|----------|
| **Streaming tokens** | Plain text append to chat area (fast, no parsing overhead) |
| **Complete response** | Re-render full message with `tui-markdown` (syntax highlighting, headers, bold, code blocks) |

`tui-markdown` uses `pulldown-cmark` internally. Custom `StyleSheet` implementations for dark/light/mono themes map to the existing `DisplaySettings.skin` from `config.toml`.

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 2.1 | Add dependencies to `Cargo.toml`: `ratatui 0.30`, `crossterm 0.29`, `tui-markdown 0.3` (highlight-code), `unicode-segmentation 1.11`; remove `rustyline` | ✅ COMPLETED |
| 2.2 | Create `src/chat/tui/mod.rs` — Terminal setup (`enter_tui()`, `exit_tui()`, `restore_terminal_on_panic()`) using crossterm raw mode + alternate screen | ✅ COMPLETED |
| 2.3 | Create `src/chat/app.rs` — `App` state, `LlmState` enum, `handle_key()`, `render()`, `tick_spinner()` | ✅ COMPLETED |
| 2.4 | Create `src/chat/input/crossterm_input.rs` — `CrosstermInput` implementing `InputBackend` (Enter, Backspace, Ctrl+C/D, arrows, history). Tab completion deferred to PR3. | ✅ COMPLETED |
| 2.5 | Create `src/chat/view/ratatui_view.rs` — `RatatuiView` implementing `ChatView` (18 trait methods + all CommandOutput variants) | ✅ COMPLETED |
| 2.6 | Create TUI components: `chat_area.rs` (ChatMessage enum), `status_bar.rs` (responsive + spinner), `input_line.rs` (InputState) | ✅ COMPLETED |
| 2.7 | Create `src/chat/tui/markdown.rs` — `tui-markdown` with `MarkdownTheme` enum (Dark/Light/Mono) and `StyleSheet` implementations | ✅ COMPLETED |
| 2.8 | Responsive `WelcomeInfo` and `StatusBarInfo` rendered as chat area messages via `RatatuiView::show_welcome()` and `RatatuiView::show_recent_context()` | ✅ COMPLETED |
| 2.9 | Spinner: braille animation frames in ratatui status bar widget (replaces indicatif for chat mode) | ✅ COMPLETED |
| 2.10 | Wire `run_chat_repl()` → `run_chat_repl_tui()` TUI event loop; CrosstermInput replaces rustyline; handle_user_message delegates to existing handler | ✅ COMPLETED |
| 2.11 | Streaming: plain text during LLM response (ChatMessage::assistant_streaming), full markdown render on completion (ChatMessage::assistant_markdown). Spinner animation deferred to PR3 (await blocks render loop). | ✅ COMPLETED (deferred animation to PR3) |
| 2.12 | Color mapping: `colors::*` ANSI constants → ratatui `Style` in `src/chat/tui/styles.rs` | ✅ COMPLETED |
| 2.13 | Tests: `cargo test` (990+), `cargo clippy` clean, `cargo fmt`, dead_code annotations for PR3 scaffolding | ✅ COMPLETED |
| 2.14 | Bug 1 fix: strip ANSI codes from RatatuiView content paths | ✅ COMPLETED |
| 2.15 | Bug 2 fix: TUI callback for tool calls (routes through ChatView instead of `eprintln!`) | ✅ COMPLETED |
| 2.16 | Bug 3 fix: `ScrollState` with auto-scroll-to-bottom + PageUp/PageDown/Home/End | ✅ COMPLETED |
| 2.17 | Native ratatui banner: braille art with ANSI→Line parsing, +30 brightness, responsive 3-tier layout | ✅ COMPLETED |
| 2.18 | `ChatView::suppress_progress_spinner()` + `debug_tools::set_tui_callback()` | ✅ COMPLETED |
| 2.19 | `MessageType` enum replaces `role`/`is_thinking`/`is_markdown`/`is_banner` bool fields in `ChatMessage` | ✅ COMPLETED |
| 2.20 | Message rendering reform: continuous flow, `>>> ` for user (bold cyan), no prefix for assistant, `[Thinking]` dim cyan, dim for tool/system, `✗` for error, no blank lines | ✅ COMPLETED |
| 2.21 | Tool call drain uses `ChatMessage::tool()` (dim, no `[System]` prefix) instead of `ChatMessage::system()` | ✅ COMPLETED |
| 2.22 | Cleanup: removed unused `user_label_style`, `assistant_label_style`, `thinking_label_style` from `styles.rs` | ✅ COMPLETED |
| 2.23 | Bug fix: resize event resets scroll to bottom (`app.scroll_to_bottom()`) | ✅ COMPLETED |
| 2.24 | Bug fix: multi-line System/Tool/Tool messages split into separate `Line`s (emojis and newlines render correctly) | ✅ COMPLETED |
| 2.25 | Blank line before Assistant/AssistantStreaming/Thinking for visual separation | ✅ COMPLETED |
| 2.26 | Thinking: 4-space indent + Unicode-aware responsive word-wrap (`wrap_line()` + `hard_break_word()`) | ✅ COMPLETED |
| 2.27 | Bug fix: auto-scroll uses `u16::MAX` (Paragraph clamps to bottom) instead of `lines.len()` approximation that breaks when wrap creates extra lines | ✅ COMPLETED |

**New Dependencies:**
```toml
ratatui = "0.30"
crossterm = { version = "0.29", features = ["event-stream"] }
tui-markdown = { version = "0.3", features = ["highlight-code"] }
unicode-segmentation = "1.11"
```

**Removed Dependencies:**
```toml
rustyline = "14"         # Removed — replaced by CrosstermInput
```

**Kept Dependencies (unchanged):**
```toml
termimad = "0.34"       # query/translate/summarize/ocr (non-chat)
indicatif = "0.17"       # subcommand spinners (non-chat only)
rattles = "0.2"          # spinner frames (unused in chat, kept for future)
```

**tui-markdown Notes:**
- Feature `highlight-code` enabled for syntax highlighting in code blocks
- Custom `StyleSheet` implementations: `DarkStyleSheet`, `LightStyleSheet`, `MonoStyleSheet`
- Maps from existing `DisplaySettings.skin` config ("dark", "light", "mono")
- Limitation: same style for inline code and code blocks (acceptable for chat)
- No code block borders/background (plain styled text with syntax highlighting)

#### PR 2 Bugs and Banner Details

**Bug 1: ANSI Escape Codes Render as Literal Text in RatatuiView**

All content paths in `RatatuiView` that pass through `ChatMessage::system()` are rendered via `Line::raw()` in `chat_area.rs:139`. ANSI escape codes (`\x1B[36m`, `\x1B[1;36m`, `\x1B[2m`, etc.) from `colors::*` in `view/mod.rs` appear as literal text instead of being interpreted.

**Affected paths:**
- `RatatuiView::show_welcome()` — `WelcomeInfo::to_boxed_string()` (BANNER_LOGO 256-color, EXTENDED_MIND_ART 24-bit true-color, session lines BOLD_CYAN/DIM/BOLD_YELLOW)
- `RatatuiView::show_recent_context()` — `RecentContextInfo::format_context_summary()` (BOLD_CYAN, BOLD_YELLOW, DIM, RESET)
- All `render_*()` methods in RatatuiView (lines 405-640) using `colors::*`

**Fix:** Add `strip_ansi_codes()` to `src/utils.rs` (hand-parse ESC sequences, no regex). Add `add_system_message()` helper to RatatuiView that strips ANSI before adding to chat. Replace all `self.app.add_message(ChatMessage::system(...))` calls in RatatuiView with `self.add_system_message(...)`.

**Bug 2: Tool Call Output Corrupts TUI Alternate Screen**

`display_tool_call()` and `log_tool_result()` in `debug_tools.rs` write to `eprintln!()`. In TUI mode, the terminal uses ratatui's alternate screen buffer, so raw stderr output corrupts the display — tool call text appears as garbage over the TUI. Additionally, tool visual indicators (📝, 💾, ⚡, 📄, 👍, 📖) in `src/tools/` used `suspend_for_print(|| { eprintln!(...) })` which also bypasses the TUI callback.

**Fix (Phase 1 — `TUI_CALLBACK` for debug_tools):** Global `TUI_CALLBACK` pattern in `debug_tools.rs`. When the TUI starts, `RatatuiView::new()` creates an `mpsc::channel` and registers a callback (`Arc<dyn Fn(&str) + Sync + Send>`). `display_tool_call()` and `log_tool_result()` check `TUI_CALLBACK` and route through it when set, sending formatted lines as `ChatMessage::system()` via the channel. `RatatuiView::render()` drains the channel each frame. On exit, `RatatuiView::restore()` clears the callback.

**Fix (Phase 2 — `tui_aware_print` for all tool indicators):** Added `tui_aware_print()` to `debug_tools.rs` — a single function that checks `TUI_CALLBACK` and routes through it (TUI mode) or falls back to `suspend_for_print` with `TOOL_DIM` styling (terminal mode). Replaced all 17 `suspend_for_print(|| { eprintln!("{TOOL_DIM}...{RESET}", ...) })` calls in tools (notes, facts, documents, run_cmd, feedback, skills) and 6 raw `eprintln!` calls (sandbox warnings + importance adjustment) with `tui_aware_print()`. Removed `#![expect(clippy::print_stderr)]` from all tool files since they no longer use `eprintln!` directly.

**Native Ratatui Banner: Braille Art with ANSI Parsing**

The TUI welcome screen uses the existing `EXTENDED_MIND_ART` braille art (14 lines × 39 cols) with `parse_ansi_to_line()` converting ANSI 24-bit true-color sequences to ratatui `Line<Span>` objects. Colors are boosted +30 RGB ( originals preserved as comments for revert). Three responsive tiers:

| Terminal Width | Layout |
|---------------|--------|
| ≥ 60 cols | Side-by-side: braille art left (39 cols) + session info right ("penduradas" if info is shorter) |
| 35-59 cols | Stacked: braille art + session info below |
| < 35 cols | Info-only: just session info lines, no banner |

**Braille art details:**
- Source: `EXTENDED_MIND_ART` constant in `src/chat/view/mod.rs` (14 lines × 39 cols, `\x1B[38;2;R;G;Bm` true-color ANSI)
- `parse_ansi_to_line()` in `src/chat/tui/banner.rs` converts ANSI sequences to `Line<Span>` with boosted colors
- `build_styled_banner()` stacks art lines; `build_session_info()` creates info lines
- Layout logic: side-by-side when width ≥ 60, stacked when ≥ 35, info-only below
- Colors boosted +30 RGB to compensate for terminal dimming; originals kept as inline comments

**Key architecture decisions:**
- `ScrollState` struct with `auto_scroll: bool` + `manual_offset: u16` in `App`
- `ScrollState::effective_scroll_from_top()` computes `Paragraph::scroll((y, 0))` offset
- `auto_scroll=true` → show bottom (newest messages); PageUp disables auto_scroll; typing/Home/End re-enable
- Tool calls routed through TUI callback (`debug_tools::TUI_CALLBACK`) → `mpsc::channel` → `ChatMessage::system()`
- `RatatuiView::render()` drains `tool_call_rx` channel each frame
- `tick_spinner()` called inside `render()` — every `show_*` method triggers render which ticks spinner

**Removed Dependencies (PR 2 bug fixes):**
```toml
ratatui-image = "11.0.2"   # Removed — braille art replaces embedded image
image = "0.25"              # Removed — only needed for ratatui-image
```

#### PR 2 Message Visual Reform (Phases 2.19–2.22)

**Goal:** Replace the bracketed `[Label]` format with a continuous chat flow that matches the TerminalView style. Messages are differentiated by style (color, weight, prefix) rather than `[You]`, `[Assistant]`, `[System]`, `[Error]` labels.

**Message rendering rules:**

| Type | Rendering | Style |
|------|-----------|-------|
| User | `>>> ` prefix + content | Bold cyan |
| Assistant (complete) | No prefix | Markdown via tui-markdown |
| Assistant (streaming) | No prefix | Plain text |
| Thinking | `[Thinking]` label, then indented content | Dim cyan label, dim indented content |
| Tool call/result | No prefix, content contains 🔧 | Dim |
| System info | No prefix | Dim |
| Error | `✗` prefix + content | Bold red |
| Banner | Responsive braille art layout | As before |

**Architecture change:** `MessageType` enum replaces the previous `role: String` + `is_thinking/is_markdown/is_banner` bool fields. Each variant maps to a distinct rendering style. Blank lines before Assistant and Thinking messages provide visual separation.

**Removed dead code:** `user_label_style()`, `assistant_label_style()`, `thinking_label_style()` — no longer needed since messages no longer use `[Label]` prefixes.

**Tool call routing:** `RatatuiView::render()` drain now creates `ChatMessage::tool()` instead of `ChatMessage::system()`, ensuring tool calls render as dim text without a `[System]` prefix.

**Multi-line rendering:** `MessageType::System`, `MessageType::Tool`, and `MessageType::Error` now iterate over `content.lines()` to create separate `Line` entries per line, fixing the bug where `\n` characters inside a single `Span` were not rendered as line breaks by ratatui. This fixes "recent context" display where emojis like 👤 and 🤖 appeared on the same line.

**Unicode-aware word-wrap:** Thinking block content uses `wrap_line()` and `hard_break_word()` functions that are Unicode-aware — they count visual width (CJK = 2 columns, combining chars = 0) instead of byte length. This prevents panic or corruption when wrapping Portuguese text with accents (olá, não, etc.) or CJK characters.

**Resize handling:** Terminal resize events (`CrosstermEvent::Resize`) now call `app.scroll_to_bottom()` to reset auto-scroll, ensuring the newest content stays visible after window resizing.

**Spinner limitation:** The status bar spinner only animates during `show_*` method calls (when `render()` ticks the spinner). During LLM "thinking" periods with no output, the main event loop is blocked on `handle_user_message_tui().await`, so the spinner freezes. This is a known limitation that will be resolved by the async event loop in PR3 (#147).

**New Files:**
- `src/chat/tui/banner.rs` — Banner rendering: `load_banner_protocol()`, `build_styled_banner()`, `build_session_info()`, responsive tier logic

**Files Modified:**
- `Cargo.toml` — Removed ratatui-image and image deps
- `src/utils.rs` — Added `strip_ansi_codes()` + tests
- `src/debug_tools.rs` — Added `TUI_CALLBACK` global, `set_tui_callback()`, routing in `display_tool_call()` and `log_tool_result()`
- `src/chat/view/mod.rs` — Made `EXTENDED_MIND_ART` `pub(crate)`, added `suppress_progress_spinner()` default method
- `src/chat/view/ratatui_view.rs` — Added `tool_call_rx` channel, TUI callback setup in `new()`, drain in `render()`, clear in `restore()`, `add_system_message()` helper
- `src/chat/tui/banner.rs` — Rewritten: `parse_ansi_to_line()`, brightness boost, responsive 3-tier layout with braille art
- `src/chat/tui/components/chat_area.rs` — `MessageType` enum, `ChatMessage` simplified, continuous flow rendering, blank lines before Assistant/Thinking, 4-space indent for thinking, `wrap_line()` + `hard_break_word()` for Unicode-aware word-wrap, multi-line System/Tool/Error rendering, resize auto-scroll
- `src/chat/tui/styles.rs` — Removed `user_label_style`, `assistant_label_style`, `thinking_label_style` (dead code after reform); kept `bold_yellow` with allow(dead_code)
- `src/chat/view/ratatui_view.rs` — Tool call drain uses `ChatMessage::tool()` instead of `ChatMessage::system()`
- `src/chat/app.rs` — `ScrollState` struct, `scroll_to_bottom()` proxy method for resize handling, `handle_key()` with PageUp/PageDown/Home/End, `tick_spinner()` in render chain
- `src/chat/repl_tui.rs` — `CrosstermEvent::Resize` handler calls `view.app_mut().scroll_to_bottom()`

**Files to Create:**
- `src/chat/tui/mod.rs` — Terminal setup
- `src/chat/tui/components/mod.rs` — Component module
- `src/chat/tui/components/chat_area.rs` — Scrollable message area widget
- `src/chat/tui/components/status_bar.rs` — Responsive status bar with spinner
- `src/chat/tui/components/input_line.rs` — Input line display widget
- `src/chat/view/ratatui_view.rs` — RatatuiView implementing ChatView
- `src/chat/tui/markdown.rs` — Markdown rendering with theme support
- `src/chat/tui/styles.rs` — ANSI-to-ratatui color mapping
- `src/chat/app.rs` — App state, AppEvent, render loop
- `src/chat/input/crossterm_input.rs` — CrosstermInput implementing InputBackend

**Files to Modify:**
- `Cargo.toml` — Add ratatui/crossterm/tui-markdown/unicode-segmentation deps, remove rustyline
- `src/chat/mod.rs` — Add tui, app modules
- `src/chat/input/mod.rs` — Add CrosstermInput, remove RustylineInput
- `src/chat/view/mod.rs` — Add RatatuiView re-export, responsive helpers
- `src/chat/repl.rs` — Refactor to call App::run() for chat mode

**Files to Remove:**
- `src/chat/input/rustyline.rs` — Replaced by CrosstermInput

**Checkpoint:** Chat mode fully functional via ratatui + crossterm. Responsive at any terminal width. Non-chat subcommands (query, translate, OCR, summarize) unchanged, still use termimad+indicatif. Basic input works (Enter, Backspace, Ctrl+C/D, arrows, history). Tab completion deferred to PR3.

---

#### PR 3: Streaming Refinement + Tab Completion + Intelligent Table Reflow (~12.5 days) — #147

**Goal:** Make the TUI chat fully functional with async streaming, tab completion, Ctrl+C cancellation, multi-line input, intelligent table rendering with rigid/elastic columns, cell word-wrapping, and row separators. Resolve all PR2 deferred limitations.

**Why longer than originally estimated (4-5 → 12.5 days):** The original estimate assumed the async event loop was partially in place. In reality, `handle_user_message_tui().await` blocks the entire event loop, requiring a full architectural migration from synchronous polling to async mpsc channels. Additionally, Ctrl+C cancellation requires `tokio::select!` integration with the LLM call, multi-line input requires input state refactoring, and intelligent table reflow with rigid/elastic column sizing and cell word-wrapping added 1.75 days.

**Deferred from PR2 (known limitations to be resolved in PR3):**

| Limitation | Impact | PR3 Phase |
|-----------|--------|-----------|
| Spinner freezes during LLM thinking | Status bar spinner only animates during `show_*` calls; main loop blocked on `handle_user_message_tui().await` | 3.2 (mpsc async channel) |
| Status bar not updated during streaming | Progress bar only updates after response completes (`update_status_tokens` in `handle_user_message_tui`); no mid-response updates | 3.2 (mpsc async channel) |
| InputState/CrosstermInput dual state | Both `InputState` (TUI rendering) and `CrosstermInput` (history management) maintain buffer/cursor with manual synchronisation in `App::history_prev/next` | 3.1 (tab completion + input unification) |
| `LlmState::ToolCall` unused | Tool call UI shows spinner label but `App::set_llm_state(ToolCall)` not wired to actual tool calls | 3.4 (tool display) |
| `assistant_streaming` rendering | `ChatMessage::assistant_streaming` exists but plain text rendering only; no incremental markdown | 3.3 (streaming refinement) |
| `/compact` indicatif spinner artifact | `handle_compact()` at `command_handlers.rs:789` passes `false` hardcoded to `suppress_spinner`, ignoring `RatatuiView::suppress_progress_spinner()` (which returns `true`). Indicatif `ProgressBar` writes ANSI to stderr, corrupting ratatui alternate screen. | 3.0 (quick win) |
| No Ctrl+C cancellation during LLM processing | `InputResult::Interrupted` is returned by `handle_key()` but ignored during `handle_user_message_tui().await` — user cannot cancel long LLM responses | 3.3 (Ctrl+C cancellation) |
| No multi-line input | Enter always submits; no Shift+Enter for newlines in input | 3.5 (multi-line) |

**Indicatif Spinner Audit (all call sites in chat path):**

| File | Line | Call | Suppress? | Risk |
|------|------|------|-----------|------|
| `core.rs:494` | `create_spinner_suppressed("Thinking...", suppress)` | Suppress from `ChatView` | ✅ Safe — `suppress=true` in TUI → `ProgressBar::hidden()` |
| `core.rs:544` | `finish_spinner(spinner.clone())` | Retry success | ✅ Safe — hidden spinner finish is no-op |
| `core.rs:557` | `finish_spinner(spinner)` | Normal finish | ✅ Safe — hidden spinner finish is no-op |
| `core.rs:719` | `create_spinner_suppressed("Compacting...", suppress)` | `/compact` | ❌ **BUG** — `suppress` comes from `compact_conversation()` which uses `view.suppress_progress_spinner()` from `handle_compact()` at `command_handlers.rs:789` which passes `false` hardcoded instead of `view.suppress_progress_spinner()` |
| `continuation.rs:249` | `view.show_progress("Paused...")` | Post-compaction | ✅ Safe — uses ChatView, not indicatif |
| `continuation.rs:302` | `view.show_progress(format!(...))` | Pre-continuation | ✅ Safe — uses ChatView |
| `continuation.rs:420` | `view.show_progress("Auto-compacting...")` | Overflow compaction | ✅ Safe — uses ChatView |
| `continuation.rs:464` | `view.show_progress(format!(...))` | Inter-tool compaction | ✅ Safe — uses ChatView |

**Only 1 bug found:** `handle_compact()` passes `false` instead of `view.suppress_progress_spinner()`. Fix: thread the `suppress` flag through `handle_compact()` → `compact_conversation()` → `create_spinner_suppressed()`.

**Implementation Phases:**

| Phase | Description | Effort | Status |
|-------|-------------|-------|--------|
| 3.0 | Quick wins: fix `/compact` suppress_spinner bug, audit all indicatif call sites | 0.75d | ✅ COMPLETED |
| 3.1 | Tab completion: `ChatCompleter` struct with `/commands` + `model_names`; unify `InputState`/`CrosstermInput` dual state | 2.5d | ✅ COMPLETED |
| 3.2 | MPSC streaming channel: async event loop with `tokio::sync::mpsc`, `AppEvent::LlmToken`/`LlmComplete`/`LlmError`, background LLM task | 2d | ✅ COMPLETED |
| 3.3 | Streaming token display + Ctrl+C cancellation: incremental `ChatMessage::assistant_streaming` updates, markdown re-render on completion, `tokio::select!` for cancellation | 2.5d | ✅ COMPLETED |
| 3.4 | Tool call/result display + error recovery: activate `LlmState::ToolCall`, wire `TUI_CALLBACK` during streaming, error display in TUI | 1d | ✅ COMPLETED (3.4a: table detection, 3.4b: embedding output suppression) |
| 3.5 | Multi-line input: Shift+Enter for newline, dynamic input line height, cursor navigation across lines | 1d | ✅ COMPLETED |
| 3.6 | Integration, testing, polish | 1d | ✅ COMPLETED |
| 3.7 | Intelligent table reflow: rigid/elastic column classification, cell word-wrapping, markdown alignment (`:---`/`---:`/`:---:`), row separators (`├─┼─┤`), shared `wrap_line` extraction to `src/chat/tui/wrap.rs` | 1.5d | ✅ COMPLETED |
| 3.8 | Table collapsing in recent context: collapse table blocks to `(...)` before flattening, preventing pipe chars in single-line summary | 0.25d | ✅ COMPLETED |
| 3.9 | ratatui-textarea integration: replace InputState with TextArea<'static>, custom key mappings (Enter/Shift+Enter, Ctrl+C clear/cancel, Ctrl+W cut-word, Ctrl+Y yank, Ctrl+A/E navigation), history nav (↑/↓ single-line vs multi-line) | 2d | ✅ COMPLETED |
| 3.10 | Rewritten input_line.rs (669→165 lines) + simplified CrosstermInput (583→115 lines, history-only), removed InputState | 1d | ✅ COMPLETED |
| 3.11 | Floating completion menu: CompletionMenuState + render_overlay(), common prefix highlighting, navigation (arrows/Tab/Enter/Esc), 80% width overlay above status bar | 1.5d | ✅ COMPLETED |
| 3.12 | ArgCompletion enum for extensible sub-completions: `/model` and `/m` both trigger model name completion via try_model_arg_fragment(), complete_model() takes cmd_trigger for correct prefix | 0.5d | ✅ COMPLETED |
| 3.13 | Chat text selection: ChatSelection component (click/drag in chat area, visual highlight white-on-blue), mouse_to_visual_pos() for coordinate mapping, visual_lines_cache for text extraction, 10 unit tests | 1.5d | ✅ COMPLETED |
| 3.14 | Copy from chat selection: Ctrl+Shift+C copies selected text to system clipboard via cli-clipboard (best-effort on Termux) | 0.25d | ✅ COMPLETED |
| 3.15 | Input vs chat selection mutual exclusion: typing in textarea clears chat selection, Enter also clears, click outside chat clears selection, scroll/Tab don't conflict | 0.25d | ✅ COMPLETED |
| 3.16 | Explicit key bindings: switch textarea.input() to input_without_shortcuts(), rebind all needed keys (movement, selection, editing, clipboard). Ctrl+Y → system clipboard paste, Ctrl+C → copy selection before clearing. Visual text selection rendering in input_line via textarea.selection_range(). | 1d | ✅ COMPLETED |
| 3.17 | Completion menu fixes: Enter confirms and submits line (not stuck), Ctrl+C/Ctrl+Shift+C/V dismiss menu, auto-complete never replaces text (only shows/hides). | 0.25d | ✅ COMPLETED |
| 3.18 | Embedding progress indicator: StatusBarState.embedding_progress field shows ⚙ current/total in status bar. mpsc::UnboundedChannel wired through App::with_embedding_channel() and RatatuiView::embedding_tx(). Startup indexing shows indicator during regeneration/recovery. | 0.5d | ✅ COMPLETED |
| 3.19 | StaticSubcommands ArgCompletion: `/think on|off` and `/tools-output compact|full|hidden` show subcommand completions. `ArgCompletion::StaticSubcommands` variant, `try_static_subcommand_fragment()`, `get_static_subcommands()`, `complete_static_subcommand()` in ChatCompleter. Embedding progress channel wired from RatatuiView through session. | 0.25d | ✅ COMPLETED |
| 3.20 | Busy-wait fix in TUI event loop: `poll(0ms)` → `poll(SPINNER_TICK_MS)` reduces idle CPU from ~4300 iters/sec (5% CPU) to ~8 iters/sec (near-zero). Revised for conditional exception: `poll(0ms)` during streaming (avoids token delay), `poll(120ms)` during idle (saves CPU). | 0.1d | ✅ COMPLETED |
| 3.21 | Flaky spinner tests fix: `serial_test` + `#[serial]` in the 4 tests of `spinner.rs`; retry assertion with `yield_now` (1000x) tolerates scheduling delays. Resolves race condition in `ACTIVE_SPINNER` global (`RwLock`) when tests run in parallel. Zero failures in 20/20 stress runs. | 0.1d | ✅ COMPLETED |
| 3.22 | Per-message embedding progress: `tokio::spawn` in `session.rs` sends `(0,1)` before spawn and `(1,1)` on completion via `EmbeddingProgressTx`. Startup regeneration sends `(0,total)` initially then `(current,total)` per item. `poll_embedding_progress()` drains channel, keeps latest. 4 unit tests in `app.rs`. | 0.5d | ✅ COMPLETED |
| 3.23 | `/reindex --yes` confirmation gate + embedding reset bug: `ChatCommand::Reindex { confirmed: bool }` requires `--yes` to prevent accidental regeneration. `Database::reset_all_embedding_flags()` deletes vec0 embeddings (`content_embeddings`, `chunk_embeddings_v2`, `fact_embeddings`) AND all `content_chunks` rows (which are derived data that would otherwise be duplicated), then resets `has_embedding=0` so `regenerate_all_embeddings()` re-processes everything. Previously `/reindex` returned "0 of 0" because `get_content_items_for_reindex()` only queries `WHERE has_embedding=0`. Also fixed duplicate chunk bug where count grew on repeated `/reindex --yes` (131→156→181) because `insert_content_chunk()` doesn't check for duplicates. Also fixed `ReindexData.total` to show `items_processed + chunks_processed`. `ResetStats` struct reports items/chunks_deleted/facts. Concurrent reindex guard: `ChatSession.is_reindexing: Arc<AtomicBool>`. Background execution: TUI mode uses `tokio::spawn` so the event loop stays responsive. `App::async_message_rx: mpsc::UnboundedReceiver<String>` and `poll_async_messages()` deliver the completion message to the chat area. Terminal mode runs synchronously with `quiet=false` progress bar. | 0.75d | ✅ COMPLETED |
| **Total** | | **~24.45d** | |

**Deferred items (usability feature creep):**

These items emerged during usability fine-tuning — direct response to human feedback.
TUI usability requires iterative adjustment based on real usage patterns, so this scope
extension is expected and purposeful. Items are deferred for dedicated design discussion,
not abandoned.

| Item | Description | Rationale |
|------|-------------|-----------|
| Shift+Enter multiline rendering | Shift+Enter inserts newline (works), but terminal compatibility varies. Some terminals don't distinguish Shift+Enter from Enter. May need Ctrl+Enter for newline or Enter=submit/Ctrl+Enter=newline. Needs careful UX planning. | Terminal compatibility concern; may need alternative keybinding |
| Background embedding progress per-message | ~~Channel infrastructure exists~~ → **Implemented in 3.22**: `tokio::spawn` sends `(0,1)` before and `(1,1)` after. Per-message progress visible in modeline ⚙ during async embedding. Startup and `/reindex` progress also wired (3.23). | ~~Low priority~~ → ✅ COMPLETED |


**Key architectural change: async event loop**

The event loop migrates from synchronous polling to async dual-source:

```
BEFORE (PR2):
  loop { poll(100ms) → key? → handle_key() → render() }
  On Enter: handle_user_message_tui().await (BLOCKS event loop)

AFTER (PR3):
  loop { tokio::select! {
    crossterm_event? → handle_key() → render()
    llm_event?       → update_streaming() → render()
    llm_done?        → re_render_markdown() → set_idle() → render()
    llm_error?       → show_error() → set_idle() → render()
    ctrl_c?          → cancel_llm() → set_idle() → render()
  }}
  On Enter: tokio::spawn(handle_user_message()) → sends tokens via mpsc
  Ctrl+C: send cancellation signal → drop LLM result → set idle
```

**Tab completion architecture:**

```rust
pub struct ChatCompleter {
    slash_commands: Vec<&'static str>,  // ["/help", "/model", "/new", ...]
    model_names: Vec<String>,           // from user_models::list_all_model_names()
}

impl ChatCompleter {
    pub fn complete(&self, input: &str) -> Option<String> {
        // If input starts with '/', complete against slash_commands
        // If input starts with '/model ', complete against model_names
        // Otherwise, no completion
    }
}
```

**Ctrl+C cancellation architecture:**

```rust
// In repl_tui.rs:
let cancel_token = tokio_util::sync::CancellationToken::new();
let cancel_clone = cancel_token.clone();

tokio::spawn(async move {
    let result = handle_user_message(..., cancel_clone).await;
    llm_sender.send(LlmEvent::Complete(result)).ok();
});

loop {
    tokio::select! {
        key = crossterm_event() => { ... }
        event = llm_receiver.recv() => { ... }
        _ = cancel_token.cancelled() => { ... }
    }
}
```

**Files to Create:**
- `src/chat/completer.rs` — `ChatCompleter` struct with slash commands + model names + `ArgCompletion` enum
- `src/chat/tui/wrap.rs` — Shared `wrap_line` + `hard_break_word` (extracted from chat_area)
- `src/chat/tui/components/completion_menu.rs` — `CompletionMenuState` + `render_overlay()` floating menu
- `src/chat/tui/components/chat_selection.rs` — `ChatSelection` + `mouse_to_visual_pos()` + `selection_style()`

**Files to Modify:**
- `src/chat/app.rs` — Add `mpsc::Receiver<LlmEvent>`, streaming message update, Tab key handling, Ctrl+C cancellation, `Shift+Enter` handling, spinner presets, `TextArea<'static>` replacing InputState, `CompletionMenuState`, `ChatSelection`, visual_lines/scroll/rect caches, `&mut self` render, Ctrl+Shift+C copy, explicit key bindings with `input_without_shortcuts()`, `CursorMove` for movement/selection, Ctrl+Y system clipboard paste, Ctrl+C copy selection, Ctrl+W/K/X edit operations, embedding progress channel (`mpsc::UnboundedReceiver<(usize, usize)>`), `with_embedding_channel()`, `poll_embedding_progress()`, `set/clear_embedding_progress()`
- `src/chat/completer.rs` (NEW) — ChatCompleter with completion candidates + ArgCompletion enum, `StaticSubcommands` variant, `try_static_subcommand_fragment()`, `get_static_subcommands()`, `complete_static_subcommand()`
- `src/chat/input/crossterm_input.rs` — Simplified to history-only (removed buffer/cursor/editing)
- `src/chat/input/mod.rs` — Export ChatCompleter
- `src/chat/view/ratatui_view.rs` — Streaming message update, tool call during streaming, error display, `collapse_tables` in recent context
- `src/chat/view/terminal.rs` — `collapse_tables` in recent context
- `src/chat/repl_tui.rs` — Async event loop with `tokio::select!`, LLM task spawning, cancellation, mouse click/drag/scroll handling, embedding progress indicator during startup recovery, wires `embedding_tx` to session
- `src/chat/core.rs` — Accept `CancellationToken`, return streaming sender, fix `suppress_spinner` in `compact_conversation`, `on_tool_call` callback
- `src/chat/llm_event.rs` — `LlmEvent::ToolCallStarted` variant
- `src/chat/command_handlers.rs` — Thread `suppress_progress_spinner()` through `handle_compact()`
- `src/chat/tui/mod.rs` — Add `pub mod wrap;`
- `src/chat/tui/markdown.rs` — `ColumnAlign` enum, `parse_separator_line`, rigid/elastic `calculate_col_widths`, cell wrapping (`wrap_cell_content`, `align_cell_text`, `build_row_expanded`), row separators, `collapse_tables`
- `src/chat/tui/components/input_line.rs` — Rewritten for TextArea rendering with selection highlight (669→316 lines), `build_display_lines()` + `apply_selection_to_line()`
- `src/chat/tui/components/chat_area.rs` — Streaming token incremental append, selection highlight (`apply_selection_highlight`), `build_lines()` extraction, `RenderMetadata` return, `Line<'_>` lifetime
- `src/chat/tui/components/mod.rs` — Add `pub mod completion_menu;`, `pub mod chat_selection;`
- `src/chat/tui/components/status_bar.rs` — Green spinner, leading space, remove dead `with_spinner()`, `embedding_progress` field with `⚙ current/total` indicator
- `src/chat/view/ratatui_view.rs` — `embedding_tx` field, `embedding_tx()` getter for background tasks
- `src/chat/session.rs` — `embedding_tx: Option<mpsc::UnboundedSender<(usize, usize)>>` field for progress reporting
- `Cargo.toml` — Add `ratatui-textarea = "0.9.1"`, `cli-clipboard = "0.4.0"`

**Checkpoint:** Chat mode fully functional with tab completion, floating completion menu, streaming markdown, tool display, error recovery, Ctrl+C cancellation, multi-line input (textarea), chat text selection (mouse), copy to clipboard (Ctrl+Shift+C), intelligent table reflow, table collapsing in recent context, explicit key bindings (input_without_shortcuts), visual text selection, embedding progress indicator, completion menu fixes, static subcommand completion. Non-chat subcommands unchanged.

---

#### PR 3 Post-Merge: Streaming Display Bug Fixes — `feat/tui-streaming-refinement`

**Goal:** Fix two streaming display bugs (thinking block fragmentation, tool call ordering) AND resolve the fundamental architectural limitation where streaming content is **lost during tool calls**.

**Historical Bugs Fixed (Phases 1-7):**

- Phase 1 (`41b0708`): Thinking block fragmentation fix — `append_stream_thinking/token()` now find existing blocks via reverse search within the streaming zone, and `finalize_stream()` only consolidates Thinking blocks within the zone (not globally)
- Phase 2 (`eaeb5d2`): Streaming zone awareness — same methods restrict reverse search to the streaming zone; tool-call Thinking blocks from earlier rounds are preserved
- Phase 3: `insert_before_streaming_zone()` — tool messages and ViewActions now inserted before the streaming zone when LLM is active, not appended after
- Phase 4: ViewAction ordering — `apply_view_action()` uses `insert_before_streaming_zone()` for content ViewActions during tool calls
- Phase 5: Deduplication — `has_streaming_zone()` prevents `ShowAssistantResponse`/`ShowMarkdown` from duplicating content already shown via streaming tokens
- Phase 6: Synchronous ViewEvent drain — `ViewEventReceiver::drain_into_llm_channel()` sends ViewEvents directly to `llm_tx` before `StreamDone`
- Phase 7: Tests — 10 new tests (23 app tests, 4 view tests), 1122 total pass, clippy clean
- Phase 8: End-of-conversation inter-tool text duplication fix — `pre_tool_content` accumulator now gates on `already_streamed`; only first-round (streamed) content accumulates, preventing `StreamBlockDone` from re-displaying inter-tool text from `process_next()` rounds that was already shown via `LlmEvent::InterToolText`
- Phase 9: First-round pre-tool duplication fix — `StreamBlockDone` handler no longer calls `stream_done()`/`finalize_stream()`; the streaming zone is already finalized by `ToolCallStarted` (which calls `finalize_streaming_zone_as_is()`). Calling `finalize_stream()` on an already-converted zone caused a DUPLICATE `Assistant_markdown` to be appended (the "no AssistantStreaming found → push new message" fallback). `StreamBlockDone` is now a unit variant (no content/thinking/metrics fields) since the handler only sets `block_finalized = true` and transitions to `ToolCall` state. Tests updated to use `finalize_streaming_zone_as_is()` matching real-world flow.
- Phase 10: Thinking block visual refinement — `[Thinking]` label replaced with `🧠 Thinking` header (dim cyan) + `│` left border (dim cyan, same as table `BD_VLINE`). Content is rendered as full Markdown via `render_markdown()` (supports headers, bold, code blocks, tables). Each content line (including wrapped sub-lines) is prefixed with `│ `. New `wrap_styled_line()` in `wrap.rs` provides width-aware word-wrap of `Line<Span>` with style preservation, ensuring resize responsiveness. Terminal (non-TUI) `show_thinking()` and `display_thinking()` synchronized to `🧠 Thinking` + `│ ` border. Removed `thinking_content_style()` (unused — styles come from `render_markdown()`). 10 new `wrap_styled_line` tests.

**Root Cause of Content Loss During Tool Calls (NEW — Architectural):**

The content loss during tool calls is **not fixable by local adjustments** — it is a fundamental architectural limitation. The streaming system was designed for a SINGLE monolithic streaming response per turn. When tool calls interrupt the stream, the pre-tool streamed content cannot be preserved because:

1. `chat_stream()` accumulates streaming content via `StreamToken` events → displayed as `AssistantStreaming` in the TUI
2. When tool calls are detected, `chat_stream()` stops streaming and enters `process_response()` (synchronous mode)
3. `process_response()` saves pre-tool content to `pre_tool_content` (for session history), then executes tools
4. `process_next()` (called by `process_response()`) makes a **new** non-streaming LLM call and returns the **post-tool** response
5. `send_message_stream()` sends the **post-tool** content in `StreamDone`
6. `finalize_stream()` replaces the `AssistantStreaming` message (which held pre-tool text) with the post-tool `Assistant` message

**Result: the pre-tool streamed text disappears from the display.**

The thinking blocks also disappear because `finalize_stream()` receives the post-tool `thinking` from `process_next()`, not the pre-tool thinking that was streamed. It therefore removes the pre-tool `Thinking` block and replaces it with nothing (or the wrong thinking).

For the complete architectural analysis, state-of-the-art research, and the proposed solution, see **`doc/CONTENT_BLOCK_ARCHITECTURE.md`**.

**Proposed Fix: Content Block Stateful Streaming**

The state-of-the-art solution (used by assistant-ui, TUUI, Claude SDK) is **content block index tracking**: each assistant response consists of multiple numbered content blocks (Block 0 = pre-tool text, Block 1 = tool call, Block 2 = tool result, Block 3 = post-tool text, etc.). Each block is finalized independently and **can never be overwritten**. When `StreamBlockDone` arrives for Block 0, `finalize_stream()` converts its `AssistantStreaming` to a stable `Assistant` message. Block 0 is now permanent. When post-tool streaming begins, new `StreamToken` events create a fresh `AssistantStreaming` (the new active block). When `StreamDone` arrives, only the LAST block (post-tool) is finalized — Block 0 is untouched because it's outside the streaming zone.

**Architecture Insight:** No `StreamBlockStart` event is needed because `finalize_stream()` already creates the boundary implicitly. After `finalize_stream` removes `AssistantStreaming` from the zone, any new streaming naturally starts a new block. The "fronteira" is the absence of streaming messages, not an explicit state flag.

**Implemented Changes:**

| Change | File | Status |
|--------|------|--------|
| Add `StreamBlockDone` variant to `LlmEvent` | `llm_event.rs` | ✅ DONE |
| Emit `StreamBlockDone` from `send_message_stream()` | `core.rs` | ✅ DONE |
| Update event loop: handle `StreamBlockDone` | `repl_tui.rs` | ✅ DONE |
| Add `block_finalized` flag to `App` | `app.rs` | ✅ DONE |
| `finalize_stream()` preserves stable blocks automatically | `app.rs` | ✅ VERIFIED |
| Tests: single block, pre/post tool, multiple tools, state clearing | `app.rs` | ✅ 4 tests |

**What Was NOT Needed:**

| Change | Reason |
|--------|--------|
| `StreamBlockStart` event | `finalize_stream()` removes `AssistantStreaming`, so new streaming naturally creates a new block |
| `block_id` in `ChatMessage` | Blocks are distinguished by position (stable vs streaming zone). Not needed. |
| `start_new_stream_block()` | Not needed — `append_stream_token()` creates new `AssistantStreaming` when none exists in zone |
| Replace `finalize_stream()` with multi-block variant | Existing logic already works: only touches LAST `AssistantStreaming` in zone |

**Architecture Principle:** The streaming zone (contiguous tail of `Thinking`/`AssistantStreaming`) IS the active block. Finalizing converts it to stable messages (outside the zone). New streaming creates a new zone = new block. No explicit block tracking needed.

**Estimativa**: ~10 horas (~2 dias de trabalho efetivo).

For the full implementation plan, test cases, ADRs, and checklist, see **`doc/CONTENT_BLOCK_ARCHITECTURE.md`**.

---

Problems:
- `tool_call_rx` is drained in `render()`, which runs AFTER `finalize_stream()` processes `StreamDone` — tool messages appear after the final response
- `ViewAction`s (PreToolContent) race with `StreamDone` through the async forwarding task — no ordering guarantee
- Pre-tool content is duplicated: shown via `StreamToken` during streaming, then again via `ViewAction::ShowAssistantResponse` after tool execution
- `show_assistant_response()` always adds a new `Assistant` message, even during streaming when one is already being displayed

**Phase 1 — Completed: Streaming thinking block fragmentation fix** (`41b0708`)

- `append_stream_thinking()` and `append_stream_token()` now find existing blocks via reverse search within the streaming zone instead of creating fragmented duplicates
- New `streaming_zone_start()` helper method on `App` returns the start index of the contiguous tail of `Thinking`/`AssistantStreaming` messages
- Streaming zone definition: contiguous tail of `Thinking`/`AssistantStreaming` messages at the end of the message list. Everything before this zone (`User`, `Assistant`, `Tool`, `System`, `Error`, `Banner`) is stable and must not be modified by streaming operations
- 11 unit tests added

**Phase 2 — Completed: Streaming zone awareness for Thinking blocks** (`eaeb5d2`)

- `append_stream_thinking()` and `append_stream_token()` restrict reverse search to the streaming zone (contiguous tail of Thinking/AssistantStreaming messages)
- `finalize_stream()` only consolidates/removes Thinking blocks within the streaming zone, preserving tool-call Thinking blocks from earlier rounds
- 13 unit tests pass (2 new: streaming zone preservation tests)

**Phase 3 — Implemented: Move tool message drain from `render()` to event loop with `insert_before_streaming_zone()`**

Instead of adding a new `LlmEvent::ToolMessage` variant (which would require routing `TUI_CALLBACK` through the per-task `llm_tx` channel), this phase keeps the existing `tool_call_rx` channel but moves the drain from `RatatuiView::render()` to the event loop. Tool messages are now inserted before the streaming zone when the LLM is active, ensuring correct visual ordering.

Changes:
- `App::insert_before_streaming_zone(message)` — new method that finds the streaming zone start and inserts before it, pushing streaming content down
- `App::has_streaming_zone()` — new method that returns `true` when there are Thinking/AssistantStreaming messages in the streaming zone (used for deduplication)
- `RatatuiView::render()` — removed the `tool_call_rx` drain loop (tool messages no longer drained during render)
- `RatatuiView::drain_tool_messages()` — new method that returns `Vec<String>` of pending tool messages (drained from `tool_call_rx`)
- Event loop in `repl_tui.rs` — after each `tokio::select!` iteration, drains tool messages via `view.drain_tool_messages()` and inserts them before the streaming zone (or appends when LLM is idle)

**Phase 4 — Implemented: ViewAction ordering with `insert_before_streaming_zone()`**

When the LLM is in `ToolCall` or `Streaming` state, ViewActions (PreToolContent, ShowMarkdown, ShowAssistantResponse) are now inserted before the streaming zone. `apply_view_action()` checks `llm_state` and `has_streaming_zone()` to determine the correct placement strategy.

Changes:
- `apply_view_action()` in `repl_tui.rs` — checks `llm_state` and `has_streaming_zone()` before routing ViewActions
- `ShowAssistantResponse` — when streaming zone exists, skips the assistant message (deduplication: content is already streaming via StreamToken); thinking is still inserted before the zone when LLM is active
- `ShowMarkdown` — when streaming zone exists, skips content (deduplication); otherwise inserts before zone when LLM is active
- `ShowThinking` — always inserted before streaming zone when LLM is active (not a duplicate)
- `ShowSystem`, `ShowError`, etc. — continue to append normally

**Phase 5 — Implemented: Deduplication — avoid double-display of content during streaming**

Pre-tool content is no longer shown twice (once via StreamToken and again via ViewAction). When `has_streaming_zone()` returns true, `ShowAssistantResponse` and `ShowMarkdown` skip adding the content message since it's already being displayed via streaming tokens. `ShowThinking` content from pre-tool rounds is still inserted before the zone since it's not a duplicate.

Changes:
- `apply_view_action()` — `ShowAssistantResponse` checks `has_streaming_zone()` and skips the assistant message when content is already streaming; only thinking (from pre-tool rounds) is inserted
- `ShowMarkdown` — checks `has_streaming_zone()` and skips when streaming zone exists

**Phase 6 — Implemented: Synchronous ViewEvent drain in `send_message_stream()`**

The async forwarding task (`tokio::spawn` in `spawn_llm_task`) that forwarded `ViewAction`s from `view_rx` to `llm_tx` introduced ordering uncertainty. ViewActions could arrive before or after `StreamDone` because the forwarding task is async. Fixed by draining `ViewEventReceiver` directly into `llm_tx` as `LlmEvent::ViewAction` in `send_message_stream()`, AFTER the coordinator call completes but BEFORE sending `StreamDone`. This guarantees: tool calls execute → ViewEvents are queued → drain sends them to `llm_tx` → THEN `StreamDone`.

Changes:
- `ViewEventReceiver::drain_into_llm_channel()` — new method that drains `ViewEvent`s directly into `llm_tx` as `LlmEvent::ViewAction` (separate ShowThinking + ShowMarkdown for PreToolContent, ShowContextWarning for compaction)
- `send_message_stream()` in `core.rs` — uses `drain_into_llm_channel(&llm_tx)` instead of `drain_into(view)` for the streaming path
- The non-streaming `send_message()` continues to use `drain_into(view)` (correct for TerminalView)
- The async forwarding task in `spawn_llm_task()` is still needed for ViewActions from `ChannelView` direct calls (like `show_system("Retrying...")`)

**Phase 7 — Implemented: Tests**

- Unit tests for `App::insert_before_streaming_zone()`:
  - Insert tool message before streaming zone (user → tool → thinking → streaming)
  - Insert when no streaming zone (falls back to append)
  - Insert in mid-conversation with existing tool rounds
  - Insert when only streaming messages exist (no stable messages before)
- Unit tests for `App::has_streaming_zone()`:
  - Returns true for Thinking-only zone
  - Returns true for AssistantStreaming-only zone
  - Returns true for interleaved zone
  - Returns false for empty messages
  - Returns false for stable-only messages (User, Assistant, Tool)
- Unit tests for `ViewEventReceiver::drain_into_llm_channel()`:
  - PreToolContent (with thinking) → ShowThinking + ShowMarkdown
  - ContextNeedsCompaction → ShowContextWarning
  - Multiple events drained in order
  - Empty content skipped (only thinking emitted)
- All 1122 existing tests pass + 10 new tests

**Completed Work:**

| Commit | Description | Status |
|--------|-------------|--------|
| `5b27134` | Merge duplicate `### Added` section in CHANGELOG | ✅ COMPLETED |
| `41b0708` | Fix streaming thinking block fragmentation (11 tests) | ✅ COMPLETED |
| `eaeb5d2` | Fix streaming zone awareness for Thinking blocks (13 tests) | ✅ COMPLETED |

**Remaining Work:**

| Phase | Description | Status |
|-------|-------------|--------|
| 3 | Move tool messages from `render()` to event loop with `insert_before_streaming_zone()` | ✅ COMPLETED |
| 4 | ViewAction ordering with `insert_before_streaming_zone()` | ✅ COMPLETED |
| 5 | Deduplication — streaming-zone-aware `show_assistant_response()`/`show_thinking()` | ✅ COMPLETED |
| 6 | Synchronous ViewEvent drain in `send_message_stream()` | ✅ COMPLETED |
| 7 | Tests (23 app tests + 4 view tests, clippy, fmt) | ✅ COMPLETED |
| 8 | End-of-conversation inter-tool text duplication — `pre_tool_content` accumulation gate on `already_streamed` | ✅ COMPLETED |
| 9 | First-round pre-tool duplication — `StreamBlockDone` handler no-op (zone finalized by `ToolCallStarted`) | ✅ COMPLETED |
| 10 | Thinking block visual refinement — `🧠 Thinking` header + `│` left border + Markdown rendering + `wrap_styled_line()` | ✅ COMPLETED |

**Post-PR3 Completions (on `feat/tui-streaming-refinement` branch):**

| Commit | Description | Status |
|--------|-------------|--------|
| `b0311f0` | `/reindex --yes` confirmation gate, duplicate chunk deletion, async message channel | ✅ COMPLETED |
| `da7ca2e` | Word-wrap input, dynamic height, Alt+Enter newline fallback | ✅ COMPLETED |
| `02ea1ed` | Fix duplicate sections in CHANGELOG (6 versions); tool detail lines to log::debug only | ✅ COMPLETED |
| `48411ab` | InterToolText thinking field; cancel_token for Ctrl+C tool-loop interruption | ✅ COMPLETED |
| `6b2cf90` | Drain tool messages after state transitions — fixes tool-before-thinking ordering | ✅ COMPLETED |
| `bcf1cdd` | `/toggle-style` rename; `↳` indent on compact tool results | ✅ COMPLETED |
| *(uncommitted)* | Command alias/shortcut removal, autocomplete descriptions, subcommand letter alias removal | ✅ COMPLETED |
| *(uncommitted)* | Mermaid width truncation (`…` ellipsis for lines exceeding terminal width) | ✅ COMPLETED |
| *(uncommitted)* | `/toggle-style` command — toggle Mermaid/source view, syntax highlighting, table format | ✅ COMPLETED |
| *(uncommitted)* | Status bar style indicator (🎨 on / 📄 off) | ✅ COMPLETED |
| *(uncommitted)* | `tui_aware_print()` — route tool indicators through TUI callback | ✅ COMPLETED |
| *(uncommitted)* | Remove sub-agent output truncation, increase vision max_tokens to 8192 | ✅ COMPLETED |
| *(uncommitted)* | Remove `prompt` param from `spawn_ocr_agent`, update system prompt | ✅ COMPLETED |
| *(uncommitted)* | Fix tool message ordering regression (append during ToolCall state) | ✅ COMPLETED |
| *(uncommitted)* | Tool call indicators rendered bright (not dim), tool results stay dim | ✅ COMPLETED |
| *(uncommitted)* | P0: Fix mouse selection offset with wrapped lines (`wrap_visual_lines` + `source_line_map`) | ✅ COMPLETED |
| *(uncommitted)* | P1: Filter empty tool parameter values from display (`display_tool_call`, `log_tool_call`) | ✅ COMPLETED |
| 0.44.0 | LaTeX formula rendering via `term-maths` crate — ```latex/```math fenced blocks + `$$` display math as Unicode art (Issue #190, PR #191) | ✅ COMPLETED |

**Known Bugs (on `feat/tui-streaming-refinement` branch):**

| Bug | Severity | Status |
|-----|----------|--------|
| ~~Scroll viewport discrepancy — lines disappear at TUI bottom~~ | ~~Medium~~ | ✅ Fixed (`18030f0` grapheme-level width) |
| ~~Mouse selection offset with wrapped lines~~ | ~~High~~ | ✅ Fixed (`wrap_visual_lines` + `source_line_map`) |
| ~~Tool output verbosity (empty params like `head=`)~~ | ~~Low~~ | ✅ Fixed (`display_tool_call` empty-value filter) |
| mermaid-text `sequenceDiagram` byte-slicing panic on emoji | Low | 🛡️ Mitigated (`call_mermaid_safely` + width truncation) |
| mermaid-text column misalignment with wide Unicode | Low | 🛡️ Mitigated (`MERMAID_INSTRUCTION` emoji avoidance) |

**Bug 1: Scroll Viewport Discrepancy — Lines Disappear at TUI Bottom**

`count_wrapped_lines()` (our `wrap_line()` using `chars().map(|c| c.width())`) and `ratatui::Paragraph::wrap()` (using `StyledGrapheme.symbol.width()` = `UnicodeWidthStr::width()`) diverge on line count for wide/ambiguous-width characters. When our wrap-undercount produces fewer visual lines than ratatui's actual wrap, the scroll offset (`effective_scroll_from_top`) is too small, and the bottom lines of the chat are pushed below the viewport.

Root causes:
1. **Emoji with ZWJ sequences** (🇧🇷, 👨‍💻): `UnicodeWidthChar::width()` (char-level) treats regional indicators as width 0 each, giving total 0. `UnicodeWidthStr::width()` (grapheme-level) correctly gives width 2. Result: our wrap undercounts, scroll offset too small, bottom lines vanish.
2. **Flag emojis** (🇧🇷 = 2 regional indicators): Same as above — each `🇧` and `🇷` is width 0 individually, but the grapheme pair is width 2.
3. **Trim difference**: Our `wrap_line()` collapses whitespace via `split_whitespace()`. Ratatui's `WordWrapper` with `trim: false` preserves leading/trailing spaces. Result: our wrap produces fewer lines when content has multiple consecutive spaces.
4. **Oversized grapheme handling**: Ratatui's `WordWrapper` **drops** graphemes wider than `max_line_width`. Our `hard_break_word()` **preserves** them. For emoji (width=2) in narrow terminals (<3 cols), ratatui drops them, we keep them.

Proposed fix: Replace `count_wrapped_lines()` with `count_ratatui_wrapped_lines()` that uses ratatui's own `WordWrapper` to count wrapped lines exactly as `Paragraph::wrap()` would render them. Also update `wrap_line()` and `measure_spans_width()` to use `UnicodeWidthStr::width()` for string-level width instead of `chars().map(|c| c.width()).sum()`.

**Bug 2: mermaid-text `sequenceDiagram` Byte-Slicing Panic on Emoji**

The `mermaid-text` crate v0.56 panics when rendering `sequenceDiagram` labels containing multi-byte emoji like ✅. The error: `end byte index 10 is not a char boundary; it is inside '✅' (bytes 8..11) of 'G-->>R: ✅ Accepted'`. The gantt renderer was fixed in v0.56, but `sequenceDiagram` still uses byte-slicing instead of char-slicing for arrow label parsing.

Mitigation: `call_mermaid_safely()` in `src/markdown/mermaid.rs` catches the panic via `catch_unwind` and suppresses the Rust panic hook (which would call `restore_terminal_on_panic()` and destroy the TUI alternate screen). The `MERMAID_INSTRUCTION` prompt tells the LLM to avoid emojis and wide Unicode in Mermaid labels. Affects only rendering — fallback to raw code block is graceful. Upstream bug report needed.

**Bug 3: Mouse Selection Offset with Wrapped Lines (P0)**

When lines in the chat area wrap (long lines spanning multiple display rows), mouse click/drag selection was misaligned from the actual content position. Root cause: `visual_lines_cache` had one entry per source `Line`, but `scroll_from_top` was in display-row space. When a line wraps, one source `Line` produces N display rows, causing indices to diverge. Selection coordinates from `mouse_to_visual_pos()` are in display-row space (matching `scroll_from_top`), but the old `apply_selection_highlight()` indexed into `lines[]` using display-row indices, which were off by the accumulated wrap offset.

Fix: `wrap_visual_lines()` expands each source `Line` into one or more display-row strings (matching ratatui's `Wrap { trim: false }`), producing a `source_line_map: Vec<usize>` that maps each display row back to its source line. `apply_selection_highlight()` now takes `source_line_map` and converts display-row selection coordinates to source-line indices before highlighting. `App.source_line_map_cache` stored alongside `visual_lines_cache` for potential future use in text extraction. `count_ratatui_wrapped_lines()` and `count_word_wrapped_graphemes()` gated with `#[cfg(test)]` (superseded by `wrap_visual_lines()` in production). 13 new tests.

**Bug 4: Tool Output Verbosity — Empty Parameter Values (P1)**

Tool call indicators like `⚡ run_cmd(head=, tail=, command="ls")` showed empty parameter values as `key=` which was visually noisy and confusing. Fix: `display_tool_call()` compact format now omits parameters where the value is empty string (`head=` → entire `head=, ` suppressed). `log_tool_call()` verbose/trace mode skips detail lines for empty values. Added `test_display_tool_call_filters_empty_values` test.

Additional mitigation: `sequenceDiagram` ignores `max_width`, producing lines that overflow the terminal. These are now truncated with `…` ellipsis via `truncate_visual_width()` in both `render_mermaid_tui()` (TUI) and `render_mermaid_rich()` (standalone).

**Bug 3: mermaid-text Column Misalignment with Wide Unicode**

`mermaid-text` uses `chars().count()` instead of `UnicodeWidthChar::width()` in `draw_tag()` (line 1276) and `box_table::put_str()` advances by 1 per char. This causes column misalignment in rendered diagrams when labels contain emojis or CJK characters. Not fixable on our side — requires upstream fix. Mitigated by `MERMAID_INSTRUCTION` telling the LLM to avoid emojis and wide Unicode in Mermaid labels.

---

#### Mermaid Width Truncation + `/toggle-style` Command

**Status:** ✅ COMPLETED (on `feat/tui-streaming-refinement` branch)

**Goal:** Two UX features for the TUI chat:
1. **Mermaid width truncation**: Lines exceeding `max_width` (especially `sequenceDiagram` which ignores `max_width`) are truncated with `…` ellipsis at the end. Uses `truncate_visual_width()` (already in `src/utils.rs`) for grapheme-level accuracy.
2. **`/toggle-style` command** (previously `/togglestyle`): Single boolean toggle — "want to see the code underneath". When style rendering is off:
   - Mermaid blocks show as source code blocks (no diagram rendering)
   - Code block syntax highlighting (syntect fg colors) is stripped — plain text with Catppuccin background preserved
   - Tables use pipe-delimited plain format (`| col1 | col2 |`) instead of box-drawing borders (┌─┐)
   - Status bar shows 📄 indicator (🎨 when style is on)
   - Old `/togglestyle` still accepted for backward compatibility

**Files changed:**
- `src/chat/tui/markdown.rs` — `render_mermaid_tui()` truncation, `apply_code_block_background()` style_enabled gate, `render_table_plain_lines()` pipe-delimited tables, `render_markdown_impl()` mermaid/table/code branches
- `src/markdown/mermaid.rs` — `render_mermaid_rich()` standalone truncation
- `src/chat/app.rs` — `style_enabled` field, `toggle_style()` method
- `src/chat/tui/components/chat_area.rs` — `render()` and `build_lines()` accept `style_enabled`
- `src/chat/tui/components/status_bar.rs` — `style_enabled` field + 🎨/📄 indicator
- `src/chat/commands.rs` — `ChatCommand::ToggleStyle` variant
- `src/chat/command_handlers.rs` — ToggleStyle match arm (placeholder for exhaustiveness)
- `src/chat/repl_tui.rs` — ToggleStyle handled directly (needs App access for `toggle_style()`)
- `src/chat/completer.rs` — `/toggle-style` tab completion entry

**Tests added:**
- `test_render_table_plain_lines_basic` — simple 2-column table → 3 lines
- `test_render_table_plain_lines_alignment_indicators` — `:---`, `:---:`, `---:` colons
- `test_render_table_plain_lines_empty_table` — invalid table → empty
- `test_style_disabled_strips_code_highlight` — fg colors stripped when off
- `test_style_disabled_uses_plain_tables` — no box-drawing when off
- `test_mermaid_lines_truncated_to_width` — no line exceeds max_width
- `test_style_disabled_mermaid_shows_source` — raw source block when off
- `test_toggle_style_flips_state` — App.toggle_style() + status bar sync

---

#### Command Alias/Shortcut Removal + Autocomplete Descriptions + Word-Wrap Input

**Status:** ✅ COMPLETED (on `feat/tui-streaming-refinement` branch)

**Goal:** Three UX improvements for the TUI chat:
1. Remove ~40 command shortcuts/aliases (only `/quit` and `/exit` remain as synonyms)
2. Show descriptions in autocomplete for single prefix matches (e.g., `/he` → `/help — Show available commands`)
3. Word-wrap input with dynamic height (33% max, 3-line minimum)

**Implementation:**

| Feature | Description | Status |
|---------|-------------|--------|
| Shortcut removal | Removed ~40 single/two-letter shortcuts from `SLASH_COMMANDS` in `completer.rs` | ✅ Done |
| `/quit` + `/exit` synonym pair | Only synonym pair kept; parser maps `/exit` → `/quit` | ✅ Done |
| Subcommand letter aliases | Removed letter aliases from `parse_note_add()`, `parse_note_subcommand()` | ✅ Done |
| `format_help()` update | Removed shortcuts section from `/help` output | ✅ Done |
| Prefix description | `complete_slash_command()` returns `CompletionResult::Multiple` with 1 item+description for prefix matches | ✅ Done |
| Word-wrap input | `wrap_line()` from `wrap.rs` shared with chat_area; dynamic height max 33%, min 3 lines | ✅ Done |
| Alt+Enter fallback | `Alt+Enter` inserts newline when Shift+Enter isn't supported by terminal | ✅ Done |
| Tests updated | Rewrote `test_complete_slash_command_exact_shortcut` → `test_complete_slash_command_prefix_shows_description` | ✅ Done |
| Documentation | `doc/src/commands/chat.md` alias tables updated, CHANGELOG entries added | ✅ Done |

**Key Files Modified:**
- `src/chat/completer.rs` — `SLASH_COMMANDS` shortcuts removed; `complete_slash_command()` descriptions on prefix match; `ArgCompletion::StaticSubcommands`, `get_static_subcommands()`, `complete_static_subcommand()`
- `src/chat/commands.rs` — Simplified `parse_command()`; removed `map_*_shortcut()` functions; removed subcommand letter aliases; `format_help()` shortcuts section removed; `get_static_subcommands()` shortcuts `/t`, `/to` removed
- `src/chat/app.rs` — `cached_input_screen_lines`, `WrapMode::WordOrGlyph`, Alt+Enter handler
- `src/chat/tui/components/input_line.rs` — Rewritten with `wrap_line()`, `SelectionRange`, `cursor_visual_position()`
- `src/chat/tui/wrap.rs` — Shared `wrap_line()` function
- `doc/src/commands/chat.md` — All aliases removed from tables
- `doc/src/CHANGELOG.md` — Entries for all three features

**Design Decisions:**
- `/quit` and `/exit` are the ONLY synonymous pair kept (user preference)
- All other aliases removed: single-letter shortcuts, two-letter shortcuts, subcommand letter aliases
- Autocomplete shows descriptions even for single prefix matches via `CompletionResult::Multiple`
- Input word-wrap uses `wrap_line()` from `wrap.rs` (shared with chat_area)
- Input max height: 33% of terminal, minimum 3 lines
- `Alt+Enter` added as newline fallback for terminals that don't support `Shift+Enter`

---

#### PR 4: Final Transition — Remove Rustyline, Make Ratatui Default (~4-5 days) — #148

**Status:** 🔄 IN PROGRESS
**Branch:** `refactor/w6-pr4-final-transition`
**Depends on:** W6-PR3 (#147) — merged ✅

**Goal:** Remove TerminalView, hardcoded ANSI escapes, CHAT_TERMINAL_WIDTH = 80, and rustyline-related code. Ratatui is the only chat rendering mode.

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 4.1 | Remove `TerminalView` (println-based implementation) | ✅ COMPLETED (PR2) |
| 4.2 | Remove `RustylineInput` and `rustyline` dependency | ✅ COMPLETED (PR2) |
| 4.3 | Remove hardcoded `\x1B[` ANSI escape codes from chat modules | ✅ COMPLETED (PR4) — pre-TUI `eprintln!` with ANSI are correct (running before alternate screen). `view/mod.rs` ANSI codes serve non-chat pipe-safe output intentionally. |
| 4.4 | Remove `CHAT_TERMINAL_WIDTH = 80` constant — width is now dynamic | ✅ COMPLETED (PR3) |
| 4.5 | Remove `build_status_bar()` and `build_clear_code()` from `repl.rs` | ✅ COMPLETED (PR3) |
| 4.6 | Simplify `run_chat_repl()` → direct `App::run()` call | ✅ COMPLETED (PR4) — `repl.rs` handles pre-TUI setup then delegates to `repl_tui` (necessary — DB/session setup must happen before TUI). |
| 4.7 | Clean up `src/chat/view/mod.rs` — remove stale TUI migration comments, update `ChatView` trait docs | ✅ COMPLETED (PR3) — ANSI helpers in `view/mod.rs` serve pipe-safe non-chat output (banner, status bar, context) |
| 4.8 | Remove `termimad` dependency — replaced by standalone renderer | ✅ COMPLETED (PR3) |
| 4.9 | Remove YAGNI dead code — full sweep (Hefesto PR3 review) | ✅ COMPLETED (PR3) — removed 21+ items: dead getters from `App`, `CompletionMenuState::len()/is_empty()`, `content_contains_table()`, `CompletionResult::Multiple { cycle_index }`, `set_model_names()`, `handle_user_message()` non-streaming path, `get_status_bar_info()`, `ChatEvent::ToolCall/ToolResult` variants, `CustomCoordinator` builder methods (`format/keep_alive/tool_count`), `SubagentType` methods gated behind `#[cfg(test)]`; added `log::error!/warn!` companions for all `eprintln!` in production code |
| 4.10 | Update `src/spinner.rs` — chat mode uses ratatui widget exclusively | ✅ COMPLETED (PR4) — Updated doc comment (backend-agnostic), gated `SpinnerGuard` and `create_custom_spinner` behind `#[cfg(test)]` (test-only code) |
| 4.11 | Refactor `auto_compact_if_needed` into `CompactionContext<'_>` — reduce 8-arg function to struct with methods | ✅ COMPLETED (PR4) — New `src/chat/compaction.rs` with `CompactionContext` struct + `compact_if_needed()` method. Removed old `auto_compact_if_needed()` from `core.rs`. Updated 7 call sites in `continuation.rs` and `command_handlers.rs`. |
| 4.12 | Decompose `run_app_loop()` — extract handler methods from event loop | ✅ COMPLETED (PR4) — `repl_tui.rs` reduced from ~1060 to 378 lines. Handler functions extracted to `event_loop.rs` (~821 lines). `EventLoopState` struct NOT used — handlers are free functions with explicit params due to `tokio::select!` borrow constraints. `LoopAction` enum (`Continue`/`Quit`) replaces `Option<()>`. |
| 4.13 | Provider-agnostic strings audit — remove remaining "Ollama" references, replace with "LLM server" or backend-agnostic phrasing | ✅ COMPLETED (PR4) — 12 user-facing strings replaced. Added `ERR_LLM_CONNECTION`, `ERR_LLM_NOT_RUNNING`, `ERR_LLM_ERROR`, `ERR_LLM_CLIENT_UNAVAILABLE` constants in `src/consts/app.rs`. Config keys `ollama_host`/`ollama_port` NOT renamed (W2 scope). |
| 4.14 | Documentation: CHANGELOG, architecture, roadmap | ✅ COMPLETED (PR4) — Updated IMPLEMENTATION.md (serves as changelog), SMOKE_TEST.md Section 23, PR-PROCESS.md Phase 6, architecture.md. Created manual-test-verification skill. |
| 4.15 | Stale doc cleanup — remove TerminalView/TUI-migration/rustyline references from docstrings and logging | ✅ COMPLETED (PR4) — 9 edits in 6 files: `ratatui_view.rs` TerminalView→standalone renderer, `input/mod.rs` removed "future migration" framing and stale "IMPORTANT" note, `view/mod.rs` replaced "TUI Migration" with event-flow description, `mod.rs` TUI Migration→TUI Architecture, `search.rs` "future TUI migration"→"independent of rendering", `logging.rs` removed dead `rustyline` filter. Zero logic changes. |
| 4.16 | Test on Linux, macOS, Termux at various terminal widths | 📋 NOT STARTED |

**Dependencies Removed:**
- `rustyline = "14"` — removed in PR2
- `termimad = "0.34"` — removed in PR3, replaced by standalone renderer (`src/markdown/standalone.rs`)

**Dependencies Kept:**
- `indicatif = "0.17"` — non-chat subcommand spinners (query, translate, summarize, OCR)
- `rattles = "0.2"` — animation frames (chat status bar widget)

**Files Removed (in PR2/PR3):**
- `src/chat/view/terminal.rs` — replaced by RatatuiView
- `src/chat/input/rustyline.rs` — replaced by CrosstermInput

**Remaining Work (PR4):**
- Clean up `repl.rs` pre-TUI error ANSI codes (minor — errors are pre-alternate-screen)
- Simplify `run_chat_repl()` setup flow
- Update `src/spinner.rs` for clean chat/non-chat split
- Documentation update

**Significant Issues from PR3 Testing (deferred to PR4):**

| # | Issue | Severity | Source | Fix |
|---|-------|----------|--------|-----|
| 6 | 🧠 indicator stays in status bar when `/think` toggles off | ✅ **FIXED** | Test #2 | `update_status_model()` after `/think` command already in event loop (commit 7bc4511) |
| 12 | `/togglestyle` alias should not exist | ✅ **FIXED** | Test #23/#36 | Removed `"togglestyle"` alias from `commands.rs` in PR3 YAGNI sweep |
| 5 | `/think on` toggles instead of explicitly enabling | ✅ **FIXED** | Test #2 | `ChatCommand::Think { enabled: Option<bool> }` parser now supports `/think on`/`/think off` explicitly |
| 7 | `/compact` freezes TUI — no async progress | ✅ **FIXED** | Test #31 | `spawn_compact_task()` runs compaction in background tokio task; `LlmState::Compacting` state; Ctrl+C ignored during compaction |
| 8 | `/compact` output not streamed | ✅ **FIXED** | Test #31 | `LlmEvent::CompactStreamToken/Done` events stream summary tokens in real time |
| 9 | `/compact` output appears truncated/sparse | ✅ **FIXED** | Test #31 | `MAX_SUMMARY_TOKENS` removed; `COMPACTION_PROMPT` rewritten to preserve ALL context |
| 13 | Embedding hang on exit (several seconds, no visual cue) | ✅ **FIXED** | Test #1 | Added `view.show_system("Saving embeddings...")` before `/quit` and Ctrl+D flush paths |
| 14 | Home/End keys don't work in Kitty terminal | 🟡 Low | Test #3 | Add Kitty key mappings (`^[OH`/`^[OF`) or document limitation |
| 17 | Multi-line input loses newlines on submit | ✅ **FIXED** | Test #8 | Fixed `MessageType::User` rendering to split on `\n` with `>>> ` prefix + `    ` continuation |
| 18 | Bracketed paste loses newlines (Ctrl+V from external clipboard) | 🟡 Low | Test #12 | Investigate crossterm/Kitty clipboard protocol |
| 20 | Diagram rendering CPU creep (5-7% with multiple mermaid in scrollback) | 🟡 Low | Test #29 | Cache rendered diagrams, skip re-rendering off-screen content |
| 23 | Mono theme preserves colors in prompt/thinking | 🟡 Low | Test #4 | Strip all colors except bold/underline in mono theme |

**Requirements Checkpoint (Phase 2.6) — All ✅ CLEAR:**

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| R1 | 🧠 indicator disappear on `/think` toggle off | ✅ | Add `update_status_model()` after `/think` in event loop |
| R2 | Multi-line newlines preserved on submit | ✅ | Data path preserves `\n` (`textarea.lines().join("\n")` + `trim()` keeps internal newlines). Bug likely in rendering or paste handling — investigate at runtime. |
| R3 | "Saving embeddings..." visual hint on exit | ✅ | Add `view.show_system()` before flush |
| R4 | Pre-TUI ANSI codes justified | ✅ | Serve pre-TUI init errors before alternate screen. No change needed. |
| R5 | `run_chat_repl()` simplification justified | ✅ | Already delegates to `run_chat_repl_tui()`. Pre-TUI setup cannot be eliminated. |
| R6–R10 | Phases 4.4–4.9 completed in PR2/PR3 | ✅ | No action needed |
| R11 | `spinner.rs` chat/non-chat split | ✅ | Add doc comment. Review `SpinnerGuard` dead_code. |
| R12 | `CompactionContext` refactor | ✅ | Struct with 8 fields. 7 call sites in `continuation.rs` + `command_handlers.rs`. |
| R13 | `run_app_loop()` decomposition | ✅ | New `src/chat/event_loop.rs` with `EventLoopState` + 3 handler methods. Main loop stays in `repl_tui.rs`. |
| R14 | Provider-agnostic strings | ✅ | 29 replacements in ~10 files. Config keys NOT renamed (W2 scope). |
| R15 | Documentation | ✅ | CHANGELOG done. Architecture update at end. |
| R16 | Manual testing | ✅ | 80/120/200 cols, /think, Shift+Enter, exit |
| R17 | `log::error!/warn!` companions | ✅ | Verify any new eprintln in PR4 |
| R18 | No unjustified `#[allow(dead_code)]` | ✅ | Review SpinnerGuard |
| R19 | Functions ≤ 200 lines | ✅ | Each handler method ≤ 200 lines |
| R20 | Quality gates pass | ✅ | cargo fmt, clippy, test |

**Architecture Decisions:**

1. **Phase 4.3 (ANSI codes):** Pre-TUI `eprintln!` with ANSI in `repl.rs` are correct — they run BEFORE the alternate screen is activated. No TUI corruption. Status: COMPLETE with justification.

2. **Phase 4.6 (`run_chat_repl()` simplification):** Already delegates to `run_chat_repl_tui()`. Pre-TUI setup (database, session) must happen before TUI and cannot be merged. Status: COMPLETE with justification.

3. **Phase 4.12 (Event loop decomposition):** New file `src/chat/event_loop.rs`:
   - Free functions (not `EventLoopState` methods) due to `tokio::select!` borrow constraints
   - `handle_key_line()` — user text submission + command routing
   - `handle_interrupt()` — Ctrl+C handling
   - `handle_eof()` — Ctrl+D / EOF handling
   - `handle_llm_event()` — streaming tokens, tool calls, errors
   - `apply_view_action()` — renders ViewAction variants to RatatuiView
   - `drain_and_add_tool_messages()` — processes tool results during streaming
   - `spawn_llm_task()` / `spawn_compact_task()` — async task spawners
   - `LoopAction` enum (`Continue`/`Quit`) replaces `Option<()>`
   - Main `loop { tokio::select! { ... } }` stays in `repl_tui.rs` as thin dispatcher (378 lines)

4. **Phase 4.13 (strings):** Config keys `ollama_host`/`ollama_port` NOT renamed — breaking change for W2.

5. **Phase 4.15 (stale doc cleanup):** 6 files, 9 edits, zero logic changes. Removed stale references to `TerminalView` (deleted in PR2 but still mentioned in docstrings), "TUI Migration" framing (TUI is now the current architecture, not "future"), "future RatatuiView" comments (RatatuiView already consumes ViewEvents via ChannelView), and dead `rustyline` filter in `logging.rs` (rustyline removed as dependency in PR2, the `.starts_with("rustyline")` check matched nothing).

6. **Bug #17 (multi-line):** `textarea.lines().join("\n")` preserves newlines. `trim()` only strips leading/trailing whitespace. Bug is in rendering or `CrosstermEvent::Paste` handling.

**CompactionContext Refactor (Phase 4.11):**

Current signature (8 parameters):
```rust
pub async fn auto_compact_if_needed(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
    context_window: usize,
    view: &mut dyn ChatView,
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
)
```

Proposed struct:
```rust
pub struct CompactionContext<'a> {
    ollama: &'a ollama_rs::Ollama,
    model_config: &'a ModelConfig,
    session: &'a mut ChatSession,
    settings: &'a Settings,
    agents_md: Option<&'a str>,
    context_window: usize,
    view: &'a mut dyn ChatView,
    llm_tx: tokio::sync::mpsc::Sender<LlmEvent>,
}
```

Call sites (7 total): `continuation.rs` (lines 113, 169, 259, 315, 430, 492), `command_handlers.rs` (line 1038).

**Provider-Agnostic Strings Audit (Phase 4.13):**

29 user-facing strings containing "Ollama" across ~10 files. Key replacements: error messages → "LLM server", help text → "LLM model", status → "the LLM server". Config keys (`ollama_host`, `ollama_port`) NOT renamed.

**Implementation Order:**

| # | Phase | Effort | Risk |
|---|-------|--------|------|
| 1 | B1: 🧠 indicator fix | 0.5 day | Low |
| 2 | B2: Multi-line newlines | 0.5 day | Medium |
| 3 | B3: Embedding exit hint | 0.5 day | Low |
| 4 | 4.10: `spinner.rs` | 0.5 day | Low |
| 5 | 4.11: `CompactionContext` | 1 day | Medium |
| 6 | 4.12: `EventLoopState` | 2 days | High |
| 7 | 4.13: Strings audit | 0.5 day | Low |
| 8 | 4.14–4.16: Docs + tests | 1 day | Low |
| 9 | 4.15: Stale doc cleanup | 0.5 day | Low |

**Blockers found in PR3 testing (must fix before merge — tracked in PR3 branch):**

| # | Issue | Severity | Source | Fix |
|---|-------|----------|--------|-----|
| 1 | Plain mode (`--plain`) outputs ANSI codes (not pipe-safe) | 🔴 High | Test #18/#30 | Add `use_plain` parameter to `display_thinking()`, strip all ANSI when plain |
| 2 | Vision proceeds despite no-capability warning | 🔴 High | Test #24 | Abort vision/OCR when model lacks capability, add `--force` for override |
| 3 | Multi-tool inter-tool text appears after all tools instead of interleaved | 🔴 High | Test #26 | Investigate `InterToolText` event timing in `custom_coordinator.rs` vs tool message drain |

---

#### Revised Overall Migration Map

```
PR 1: CommandOutput (✅ COMPLETED)
  ├── 336 println calls → CommandOutput enum
  ├── ChatView gains new methods
  ├── All output goes through typed channels
  └── Codebase functional, identical behavior

PR 2: Ratatui Render + CrosstermInput (THIS PR)
  ├── Ratatui + crossterm + tui-markdown deps added
  ├── Rustyline REMOVED (incompatible with ratatui)
  ├── RatatuiView implements ChatView
  ├── CrosstermInput implements InputBackend
  ├── App event loop with render cycle
  ├── Responsive: chat area, status bar, welcome
  ├── Spinner in status bar (rattles directly, no indicatif)
  ├── Streaming: plain text → markdown on completion
  ├── Input disabled during LLM processing
  └── Non-chat subcommands keep termimad + indicatif

PR 3: Refinement + Tab Completion
  ├── Tab completion via ChatCompleter reuse
  ├── Streaming token display refinement
  ├── Tool call/result display in chat area
  ├── Error recovery in TUI
  ├── Full handle_user_message() integration
  └── Multi-line input support

PR 4: Cleanup
  ├── Remove TerminalView (println-based)
  ├── Remove hardcoded ANSI from chat
  ├── Remove CHAT_TERMINAL_WIDTH = 80
  ├── Simplify run_chat_repl() → App::run()
  └── Subcommands keep termimad/indicatif
```

**Estimated Total Effort:** 16-23 days (~3-4 weeks), reduced from original 18-26 days because PR2+PR3 merge eliminates duplicate setup and the --tui flag infrastructure.

**Dependencies:**
- Blocks: None (can start after critical bugs)
- Blocked by: Critical bugs currently on the board
- Enables: TUI (#16) — this rebuild is a prerequisite for the full TUI

**Relationship to TUI #16:** This rebuild replaces the println-based chat rendering with ratatui, making the chat responsive at any terminal width. The full TUI (#16) will build ON TOP of this infrastructure, adding sidebars, /queue, /steer, multi-pane layout, and the full UX design. Think of this as "laying the foundation" — the rendering engine, event loop, and input handling — while #16 is "building the house" on top of it.

**Related:** Issue #16 (TUI — full design, builds on top of this), Issue #131 (Remove print expects — prerequisite handled in PR 1), Issues #145, #146, #147, #148 (W6 PRs)

---

### TUI (Terminal User Interface) — #16 [M2]

**Status:** ❌ NOT STARTED

**Goal:** Build the full TUI experience: sidebars, /queue, /steer, multi-pane layout, and complete UX design on top of the Responsive Chat Rebuild infrastructure.

**Depends on:** Responsive Chat Rebuild (M1, W6) — the Ratatui rendering engine, event loop, and CrosstermInput from the Rebuild are prerequisites for this milestone.

See `doc/src/development/roadmap.md` - TUI section for detailed implementation plan.

**Milestone:** M2 — full design + implementation. TUI is the pre-launch product experience.

**What W6 (Responsive Chat Rebuild) Already Delivers:**

These items from the original #16 scope are completed by the Responsive Chat Rebuild (M1, W6) and should NOT be re-implemented:

| Item | W6 Deliverable | PR |
|------|---------------|-----|
| Chat pane with markdown rendering | `RatatuiView` + `tui-markdown` | PR 2 |
| Input pane with history | `CrosstermInput` + history + tab completion | PR 3 |
| Status bar (model, context, tokens) | Ratatui `StatusBar` widget, responsive | PR 2 |
| Ratatui research | Architecture defined in W6 plan | PR 1-4 |
| Terminal resize handling | `AppEvent::Resize` in event loop | PR 3 |
| `TuiInput` implementing `InputBackend` | `CrosstermInput` | PR 3 |
| `TuiView` implementing `ChatView` | `RatatuiView` | PR 2 |
| Remove print expects | CommandResult enum replaces all println | PR 1 |
| Concurrent input channel (mpsc) | Event loop with tokio channels | PR 3 |
| Responsive layout at any terminal width | Declarative ratatui layout | PR 2 |

**What #16 Still Needs to Build (on top of W6):**

| Item | Description | Effort |
|------|-------------|--------|
| Sidebar for tools/messages | Multi-pane layout with tool call details | 1-2 weeks |
| `/queue` and `/steer` busy-input modes | Concurrent input during LLM execution | 2-3 weeks (design + impl) |
| `ApplicationBackend` trait | Formal decoupling for CLI/TUI/ACP backends | 1-2 weeks |
| UX design mockups | Full TUI wireframes with sidebar, scrollback, panes | 1 week |
| Mascote ASCII indicator | Visual state indicator ("Nó de Ideias") | 1-2 days |
| ACP/B8 adapter | Third backend consuming same application via JSON-RPC | 2-3 weeks |
| PageUp/PageDown scrollback | History navigation in chat area | 2-3 days |
| Configurable layout | User preferences for pane sizes, visibility | 1-2 days |

**Pre-migration cleanup (DONE by W6):**

These items were in #16's scope but are now completed by the Responsive Chat Rebuild:

- ~~Remove `#![expect(clippy::print_stdout)]` and `#![expect(clippy::print_stderr)]` from `lib.rs`~~ → Done in PR 1 of W6
- ~~Audit each module that currently uses `println!`/`eprintln!` directly~~ → Done in PR 1 of W6 (CommandResult enum)
- ~~Replace logic module prints with `ChatView` method calls~~ → Done in PR 1 of W6

**Architectural Requirement (ACP/B8 Prerequisite):**

The TUI implementation MUST create a clean application layer decoupling core logic from I/O. This decoupling is required for ACP (B8) — the ACP adapter will be a third I/O backend consuming the same application layer via JSON-RPC over stdio instead of rustyline or ratatui.

W6 delivers the rendering layer (`RatatuiView` + `CrosstermInput` + `App` event loop). #16 adds the formal `ApplicationBackend` trait on top:

```
ApplicationBackend (trait) — #16 creates this
   ├── CLI (historical, removed in W6 PR 4)
   ├── TUI (RatatuiView + CrosstermInput) — W6 delivers this
   └── ACP (stdio JSON-RPC) — B8
```

The `ApplicationBackend` trait should expose:
- `send_message(&mut self, msg: &str) -> EventStream` — sends message, returns stream of events
- `create_session(&mut self) -> SessionId` — creates new session
- `load_session(&mut self, id: &SessionId) -> Result<Session>` — loads existing session
- `list_sessions(&mut self) -> Vec<SessionInfo>` — lists available sessions
- `cancel(&mut self) -> Result<()>` — cancels ongoing operation

**Estimated effort:** 5-7 weeks (reduced from 8-10 weeks because W6 delivers the foundation)

**Components still needed for #16:**

| Component | Status | Inherited from W6? |
|-----------|--------|---------------------|
| Chat pane with markdown rendering | ✅ Delivered | ✅ PR 2 |
| Input pane with history | ✅ Delivered | ✅ PR 3 |
| Status bar | ✅ Delivered | ✅ PR 2 |
| Sidebar for tools/messages | 📋 Needed | ❌ New for #16 |
| `/queue` busy-input mode | 📋 Needed | ❌ #117 |
| `/steer` busy-input mode | 📋 Needed | ❌ #117 |
| `ApplicationBackend` trait | 📋 Needed | ❌ Formal abstraction layer |
| `BusyInputMode` enum | 📋 Needed | ❌ #117 |
| UX design + mockups | 📋 Needed | ❌ New for #16 |
| PageUp/PageDown scrollback | 📋 Needed | ❌ New for #16 |

**Mascote idea:** An ASCII mascote (Sprach described itself as "Nó de Ideias" — Idea Knot) could serve as a visual indicator of system state. When reflection triggers fire (see S2.3), the mascote's expression could change to signal the user. This follows patterns from other agent frameworks where visual feedback helps users understand internal state. Note for #16 implementation.

#### TUI Interaction Modes (`/queue` and `/steer`) — #117 [M2]

**Status:** 📋 PLANNED (part of P14 TUI milestone, M2)  
**Depends on:** TUI must exist first (concurrent input requires async input backend)  
**Estimated effort:** 2-3 days (design) + 1-2 weeks (implementation)  
**Inspiration:** Hermes Agent `/queue` and `/steer` commands

**Goal:** Enable three busy-input modes in the TUI so users can interact with a running agent without destructive interruption.

**Three modes:**

| Mode | Input during execution | UX |
|------|----------------------|-----|
| `interrupt` (default) | Ctrl+C kills current run, starts new | Current CLI behavior |
| `queue` | `/queue <prompt>` enqueues for next turn | `"Queued: check logs" → waits for current run` |
| `steer` | `/steer <prompt>` injects guidance mid-run | `"⏩ Steer: focus on errors" → arrives after next tool call` |

**Config:**
```toml
[tui]
busy_input_mode = "steer"  # "interrupt" | "queue" | "steer"
```

**Architecture (M3 — TUI Implementation):**

```
┌─ TUI Thread ─────────────────┐     ┌─ Agent Thread ───────────────┐
│                               │     │                               │
│  Input: "check logs"         │────►│  mpsc::Receiver               │
│  ↓ match busy_input_mode     │     │  ↓                            │
│  ├─ interrupt → send Cancel  │     │  Agent Loop:                  │
│  ├─ queue → push PendingQueue│     │    coordinator.chat()         │
│  └─ steer → push PendingSteer│     │    ↓                          │
│                               │     │    process_response()         │
│  Status: "⏩ Steer queued"    │     │      ↓ after tool result     │
│                               │     │      inject_steer_if_pending │
└───────────────────────────────┘     └───────────────────────────────┘
```

**Key design decisions (from Hermes Agent analysis):**

| Aspect | `/queue` | `/steer` |
|--------|----------|----------|
| Turn boundary | New turn after current finishes | Same turn, after next tool |
| Role alternation | New user message | Injected into tool result (no violation) |
| Merging | Never merges (each = separate turn) | Multiple steers concatenate |
| Use case | Sequential tasks: "do A, then B" | Mid-course correction: "actually focus on X" |
| Cache impact | Full new turn | Minimal (single message modification) |

**New components:**

| Component | File | Description |
|-----------|------|-------------|
| `PendingQueue` | `src/chat/busy_input.rs` | FIFO of messages, each becomes a separate turn |
| `PendingSteer` | `src/chat/busy_input.rs` | Steer buffer, injected after tool result |
| `BusyInputMode` | `src/chat/busy_input.rs` | Enum: Interrupt, Queue, Steer |
| `inject_steer()` | `src/chat/custom_coordinator.rs` | Appends steer text to last tool result |
| Concurrent input | `src/chat/input/tui.rs` | `TuiInput` with `mpsc` channel for busy-input |

**Why TUI-only:** The current rustyline input is blocking — it cannot receive input while the LLM is running. `/queue` and `/steer` require a concurrent input channel, which the TUI naturally provides via its event loop.

**Reference:** Hermes Agent `agent/run_agent.py` (steer: line 4151, queue: gateway/run.py line 685), `cli.py` (lines 6295-6330)

**Related:** Issue #16

---

### Plugin System — #15 [M3]

**Status:** ❌ NOT STARTED

**Goal:** Pluggable architecture for extending sprachspiel functionality with external tools.

**Dependencies:** TBD

**Estimated effort:** TBD

**Related:** Issue #15

**Sprach 2.0 Note:** The article adds architectural details to P15: (1) 4-layer architecture (Runtime WASM → Host Interface → Plugin Manifest → Plugin Code), (2) sandbox by capabilities (allowed/denied lists, not total isolation), (3) semantic versioning (DEC-005: major equal, minor ≥ required), (4) TOML manifest format. See S2.4 in PRIORITY 7 for details.

**Sub-items to address during P15 research:**

1. **MCP Client Integration:** Dynamic tool discovery via MCP protocol. Primary path for extending functionality without native code changes.
2. **Extensible Hooks:** Lifecycle hooks (PreToolCall, PostFileWrite, PreCompact) as a lightweight plugin alternative. May or may not be implemented depending on scope.
3. **Post-edit verification as EXTERNAL service:** NOT built into sprachspiel. Code verification (syntax, typecheck, lint) should be a plugin or external service that the harness invokes. This keeps sprachspiel focused on research, interaction, and cognitive evolution.
4. **Scope clarification:** sprachspiel is NOT a code-specific harness. Features specific to software development workflows should be delegated to external tools/plugins. The core should remain focused on general-purpose cognitive interaction.

#### Background: Opt-in Tools

The current architecture supports **feature flags** for optional tools:

- `pokemon-tools` - Opt-in (not in default build)
- `led-tools` - Opt-in (not in default build)
- `finance-tools` - Opt-in (not in default build)
- `search-tools` - Opt-in (not in default build)

This is a precedent for the plugin system: tools that require:
- External APIs (PokéAPI, Google Finance)
- Specific hardware (LED control)
- Opt-in due to size/complexity

#### Architecture Direction

The Plugin System should support two paradigms:

**1. Native Plugins (Rust WASM/WebAssembly)**

```rust
// Future: ./plugins/my_plugin.wasm
pub fn register_tools(registry: &mut ToolRegistry) {
    registry.register(MyTool::new());
}
```

**2. MCP (Model Context Protocol) Support**

MCP is an open standard for connecting AI applications to external systems:

- **Standardized interface**: JSON Schema for tool definitions
- **Server-based**: External processes provide tools via MCP protocol
- **Dynamic discovery**: Tools are listed at runtime, not compile-time
- **Security**: Human-in-the-loop for sensitive operations

⚠️ **CRITICAL SECURITY ADVISORY (2026-04-19):** The Anthropic MCP SDK has a **by-design vulnerability** in `StdioServerParameters` that allows arbitrary command execution. The STDIO transport configuration passes commands directly to the OS without validation — even failed connections execute the command. This affects 7,000+ public MCP servers and 150M+ downloads (CVE-2025-65720, CVE-2026-30623, CVE-2026-30624, CVE-2026-30618, CVE-2026-33224, CVE-2026-30625, CVE-2026-30615, CVE-2026-26015, CVE-2026-40933, CVE-2025-49596, CVE-2026-22252, CVE-2026-22688, CVE-2025-54994, CVE-2025-54136). Anthropic has declined to fix this, calling it "expected behavior."

**sprachspiel's MCP security requirements (ADR-007):**
1. `sprachspiel` MUST NOT use the Anthropic MCP SDK's `StdioServerParameters` directly for untrusted input
2. MCP server configurations containing `command` fields MUST be treated as arbitrary code execution — equivalent to running a shell command
3. User confirmation MUST be required before installing or connecting to any MCP server (no zero-click auto-discovery)
4. An allowlist of approved MCP server commands MUST be maintained in `config.toml` (`[mcp].allowed_servers`)
5. MCP servers SHOULD prefer Streamable HTTP transport over STDIO when available (HTTP transport does not spawn arbitrary processes)
6. When STDIO transport is required, the server process MUST run with minimal privileges (seccomp/cgroups/namespace restrictions)
7. MCP marketplace/server registry URLs MUST be treated as untrusted input — URLs in server configurations can trigger hidden STDIO configurations (CVE category 4 from the OX Security research)

**Reference:** https://modelcontextprotocol.io

**Example MCP Tool Definition:**
```json
{
  "name": "get_weather",
  "description": "Get current weather for a location",
  "inputSchema": {
    "type": "object",
    "properties": {
      "location": { "type": "string" }
    },
    "required": ["location"]
  }
}
```

#### Research Summary

| System | Approach | Type Safety | Security |
|--------|----------|-------------|----------|
| MCP | JSON Schema + server | Runtime validation | Human approval ⚠️ RCE risk via STDIO (CVE-2025-65720 et al.) |
| AI SDK (Vercel) | Zod Schema + execute | Compile-time | Needs approval |
| Hermes Agent | Skills (Markdown) + Tools (Rust) | Compile-time for tools | Sanitization |
| **sprachspiel (current)** | Rust code + feature flags | Compile-time | Blacklist |

#### Implementation Phases

**Phase 1: MCP Client Integration**
- Implement MCP client to connect to external tool servers
- Support `tools/list` and `tools/call` operations
- Human confirmation UI for tool invocations
- ⚠️ **ADR-007 constraints:** STDIO transport REQUIRES explicit user approval + command allowlist in `config.toml`. Prefer HTTP/SSE transport. Never use Anthropic SDK `StdioServerParameters` directly.

**Phase 2: Native Plugin System**
- WASM module loading with sandbox
- Plugin registry API
- Hot-reload support

**Phase 3: Plugin Distribution**
- Plugin discovery mechanism
- Version management
- Security scanning

#### Why Not Generic HTTP Tool

A generic `http_request` tool has been considered and **rejected** for these reasons:

1. **Security**: No input sanitization, can call ANY URL
2. **Type Safety**: LLM must infer JSON schemas from responses
3. **Error Handling**: Runtime errors only, no compile-time validation
4. **Complexity**: LLMs struggle with complex nested APIs without typed schemas

The industry standard (MCP, Claude Code, etc.) uses **typed tool schemas**, not raw HTTP.

#### References

- [Model Context Protocol](https://modelcontextprotocol.io)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [Vercel AI SDK Tools](https://sdk.vercel.ai/docs/ai-sdk-core/tools-and-tool-calling)
- [OWASP LLM Top 10](https://genai.owasp.org/llm-top-10/)

---

## Sprach 2.0 — CAS Research [M3]

**Status:** 🟡 RESEARCH NEEDED  
**Comprehensive Design:** See [Sprach 2.0 Research](./doc/src/development/sprach-2-0-research.md) for open questions, code analysis, and implementation details.

Based on the Sprach 2.0 self-analysis article, which identifies sprachspiel as a Complex Adaptive System (CAS) with emergent properties but limited open-endedness. The proposals below aim to increase emergent connectivity and adaptive behavior.

**Prerequisite:** All P1-P5 current items must be completed before starting P7 work.

### S2.1: Visualize Connections Tool — #77

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** None  
**Estimated effort:** 2-3 days (after research)

LLM tool that, given an item ID or query, finds top-N most similar items via embedding similarity and returns a Mermaid graph visualization.

**Existing infrastructure:**
- `search_content_semantic()` in `content/db.rs` — vector search works
- `content_embeddings` (vec0) — 256d embeddings already stored
- `ContentSearchResult.score` — similarity distance already computed
- `EmbeddingClient` — configurable embedding model

**Open questions:**
- How to handle items without embeddings?
- Mermaid rendering: terminal output vs. file vs. markdown block?
- Should connections be calculated on-the-fly or cached? (DEC-001: cache incrementally)
- What N is optimal for meaningful graphs without noise?

---

### S2.2: Content Relations Graph — #78

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** #77
**Estimated effort:** 5-8 days (after research)

Persistent `content_relations` table with a two-layer architecture:
1. **Layer 1 (Discovery):** Embedding-based, automatic, finds proximity (`find_similar(query_embedding, threshold=0.75)`)
2. **Layer 2 (Classification):** LLM-based, on-demand, classifies relation type (`classify_relation(source, target)`)

**Relation types** (inspired by Zettelkasten):

| Type | Definition | Example |
|------|-----------|---------|
| `extends` | B develops A | Carvalho extends Maturana |
| `contradicts` | B contests A | Lucas contests Estrada |
| `instantiates` | B is case of A | "Eu-difuso" instantiates "Strange Loop" |
| `cites` | B references A | Note cites Villalobos |
| `presupposes` | B assumes A as base | Enactivism presupposes autopoiesis |
| `resolves` | B dissolves tension in A | Synthesis resolves Ellis+Gödel |
| `questions` | B problematizes A | Critique questions Clark |

**Schema:**

```sql
CREATE TABLE content_relations (
    source_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    relation_type TEXT NOT NULL,      -- enum of 7 types
    strength REAL NOT NULL,           -- cosine similarity (0-1)
    confidence REAL NOT NULL,         -- LLM confidence (0-1)
    justification TEXT,               -- 1-sentence LLM explanation
    created_at INTEGER NOT NULL,
    PRIMARY KEY (source_id, target_id)
);
```

**Cache incremental approach (DEC-001):** Classification runs on-demand, results are cached. Graph grows organically by usage, not pre-computed.

**Existing infrastructure:**
- `content_items` unified table (schema v8) with migration system
- `EmbeddingClient` for similarity computation
- `ContentSearchResult` with distance scoring

**Open questions:**
- When to create relations? On-query (lazy) vs on-insert (eager) vs batch?
- Should unused relations decay (like facts)?
- Is persistent storage better than lazy computation (S2.1 only)?
- Scalability: 10K items × 10 relations = 100K rows — acceptable for SQLite?

---

### S2.3: Reflection on Triggers + Curation — #79

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** #77, #78 (needs relation detection)  
**Estimated effort:** 4-7 days (after research)

Self-reflection triggered by specific events (not periodic). Reflection results are saved as drafts requiring human approval.

**Trigger types (DEC-002):**

| Trigger | Criterion | Example |
|---------|----------|---------|
| Error | Tool failure, insufficient context | `visualize_connections()` returns empty |
| Surprise | Embedding distant from expected | Query "enactivism" returns note about "Turing" |
| Conflict | Two notes contradict each other | Carvalho vs. Villalobos on closure |
| Pattern | Same query repeated N times | User asks about "open-endedness" 3× in 5 sessions |
| On-demand | User requests | "Sprach, reflect on X" → `/reflect` command |

**Curation pipeline (DEC-003):**

Reflections are saved as **drafts**, not published automatically:

1. **Novelty:** Cosine similarity < 0.85 with existing notes
2. **Actionability:** Must imply ≥1 concrete change (tool, note, behavior)
3. **Density:** Minimum 200 words, ≥1 Zettelkasten connection
4. **Human approval:** Draft → `/approve-patch` → published

**Existing infrastructure:**
- `note_add` tool (LLM can create notes)
- `ChatSession` with message counting
- `ContentSource::Llm` source attribution
- Fact decay system (model for reflection aging)

**Open questions:**
- How to detect "surprise" triggers? (embedding distance threshold tuning)
- How to detect "conflict" triggers? (contradictory notes identification)
- How to detect "pattern" triggers? (repeated query tracking)
- What prompt template produces useful reflections vs. noise?
- Where to store drafts? Database with `status=draft` flag?

---

### S2.4: Plugin System (WASM)

**NOTE:** This is already tracked as PRIORITY 15 in this document. The Sprach 2.0 article adds architectural details:

- **4-layer architecture:** Runtime WASM → Host Interface → Plugin Manifest → Plugin Code
- **Sandbox by capabilities** (DEC-004): allowed/denied lists, not total isolation
- **Semantic versioning** (DEC-005): Major equal, minor ≥ required
- **Example manifest:** TOML with `name`, `version`, `[capabilities]`
- **State of art:** WASM confirmed as emerging standard; alternatives (E2B, Daytona) need evaluation

These details should be incorporated into P15 when research begins.

---

### S2.5: SOUL.md Patching with Approval — #80

**Status:** 🟡 RESEARCH NEEDED  
**Depends on:** #79 (curation pipeline feeds personality adjustment)  
**Estimated effort:** 3-5 days (after research)

Dynamic personality adaptation through LLM-generated patches to SOUL.md, with mandatory human approval.

**Flow (DEC-006):**

1. User gives feedback ("too verbose", "too technical")
2. Sprach generates a **suggestion patch** (not automatic)
3. Lucas reviews via `/apply-patch` command
4. If approved: patch applied + git commit automatic

**Key difference from P5 (Feedback Infrastructure):**
- P5 captures **what happened** (signal + weight for retrieval)
- S2.5 adjusts **who I am** (personality modification)

Both are complementary: P5 improves *retrieval quality*, S2.5 improves *behavior style*.

**Existing infrastructure:**
- `src/soul.rs` — loads SOUL.md statically (no dynamic updates yet)
- `src/facts/` — model for decay and scope
- `src/tools/notes.rs` — model for LLM-generated content with source attribution

**Open questions:**
- Should SOUL.md be in git? What about users without git?
- Patch format: search-replace? Section-level? Line-level?
- How to validate patches don't corrupt SOUL.md structure?
- Backup mechanism: timestamped copies before patching?

---

### S2.6: Skills Auto-Registration and Meta-Architecture

**Status:** 🕐 AWAITING MATURATION  
**Depends on:** #77–#80 operational  
**Estimated effort:** TBD

Meta-level architecture where skills can create and register other skills. Requires S2.1-S2.5 to be operational and well-tested before this becomes meaningful.

**Why wait:** Needs more experimentation with 6.1-6.5 before meta-level design makes sense.

---

### Sprach 2.0: Validated Decisions (DEC-001 to DEC-007)

The following architectural decisions from the Sprach 2.0 article have been validated by state-of-the-art research:

| Decision | Ruling | Validation |
|----------|--------|------------|
| **DEC-001** Cache incremental for `content_relations` | On-demand, not pre-computed | GraphSeek 2026, Graph RAG 2026 |
| **DEC-002** Reflection triggers over periodic | Specific triggers, not time-based | ICML 2025, MeCo arXiv 2025 |
| **DEC-003** Curation with human approval | Drafts, not auto-publish | Rewire.it, "Human-in-the-loop" |
| **DEC-004** WASM sandbox by capabilities | Allowed/denied, not total isolation. **CRITICAL (2026-04-19):** DEC-007 extends this — `process_spawn` deny is meaningless when MCP STDIO transport itself *is* process spawning. STDIO MCP servers require explicit allowlist + sandbox. | The New Stack 2026, MCP-SandboxScan, OX Security 2026 |
| **DEC-005** Semantic versioning for plugins | Major equal, minor ≥ | OpenFang, "Semver + manifest signing" |
| **DEC-006** SOUL.md patches with human approval | Suggestions, not automatic | MetaMind NeurIPS 2025, "Human oversight" |
| **DEC-007** MCP STDIO security: no untrusted command execution | Explicit approval + allowlist + sandbox for STDIO | OX Security 2026, CVE-2025-65720 et al., Anthropic MCP SDK vulnerability |

**Competitors identified:**
- Joplin GSoC 2026: Note graphs with AI (similar to S2.1 + S2.2)
- OpenClaw: WASM sandbox for community skills (similar to S2.4)

---

### ADR-007: MCP STDIO Transport Security

**Date:** 2026-04-19  
**Status:** Accepted  
**Severity:** CRITICAL

#### Context

The Anthropic MCP SDK has a by-design Remote Code Execution (RCE) vulnerability in its STDIO transport. `StdioServerParameters` executes arbitrary OS commands with the parent application's privileges **before any validation or connection attempt occurs**. This means that simply configuring an MCP server connection can execute malicious commands on the host system, even if the connection fails.

**Affected CVEs:** CVE-2025-65720, CVE-2026-30623, CVE-2026-30624, CVE-2026-30618, CVE-2026-33224, CVE-2026-30625, CVE-2026-30615, CVE-2026-26015, CVE-2026-40933, CVE-2025-49596, CVE-2026-22252, CVE-2026-22688, CVE-2025-54994, CVE-2025-54136

**Scope:** 7000+ MCP servers, 150M+ downloads affected. Anthropic declined to fix ("expected behavior").

**Impact on sprachspiel:** Currently zero — sprachspiel has no MCP code. However, P6 (Phase 1) includes MCP Client Integration (P15/Plugin System), making this a future-critical concern.

#### Decision

1. **Never use `StdioServerParameters` directly.** If STDIO transport is supported, it will be through a sandboxed wrapper that validates commands against an explicit allowlist before execution.
2. **Mandatory human confirmation for MCP server installation.** Users must explicitly approve each MCP server, with clear warning about the security implications.
3. **`config.toml` command allowlist.** STDIO MCP server configurations must declare an explicit `allowed_commands` list. Any command not on the list is rejected.
4. **HTTP transport preference.** Prefer HTTP/SSE transport over STDIO wherever possible. STDIO should require explicit opt-in with security acknowledgment.
5. **Extend DEC-004 WASM sandbox to MCP processes.** STDIO MCP servers run inside the same WASM sandbox that plugins use, with `process_spawn` capability denied by default.

#### Consequences

- **Positive:** sprachspiel users are protected from the RCE vulnerability by design. The allowlist + sandbox approach means even a malicious MCP server config cannot execute arbitrary commands.
- **Negative:** STDIO MCP servers with complex startup commands may not work out-of-the-box. Users will need to review and approve each server's command list. This is intentional — security over convenience.
- **Relation to DEC-004:** `denied = ["process_spawn"]` is **meaningless** when MCP STDIO transport itself *is* process spawning. DEC-007 fixes this gap by requiring an explicit allowlist and sandbox for STDIO transport, making the DEC-004 capability model effective even with MCP.

#### References

- OX Security: "MCP Vulnerabilities Could Expose AI Apps to RCE, Data Theft and Other Attacks" (2026)
- CVE-2025-65720 et al.
- Anthropic MCP SDK `StdioServerParameters` source code
- DEC-004: WASM Sandbox by Capabilities

---

## Streaming Architecture (Future)

The `ollama-rs` library (already included with `stream` feature) provides streaming capabilities:

```rust
// Streaming API
pub async fn send_chat_messages_stream(
    &self,
    request: ChatMessageRequest,
) -> Result<ChatMessageResponseStream>

// ChatMessage includes thinking content
pub struct ChatMessage {
    pub content: String,
    pub thinking: Option<String>,  // For DeepSeek R1, etc.
    // ...
}
```

**Current Status:** Non-streaming only (`send_chat_messages()`)
**Streaming Path:** `send_chat_messages_stream()` or `send_chat_messages_with_history_stream()`

**Implementation Considerations:**

1. **CLI Mode (current):** `termimad` is synchronous, requires block buffering
2. **TUI Mode (future):** Ratatui supports incremental rendering via `tui-markdown`
3. **Thinking Display:** Separate pane in TUI, inline dimmed text in CLI

See: `doc/src/development/roadmap.md` - TUI section for detailed streaming approach

---

## Small Features [branch: small-features]

### ✅ SF1: Colored User Prompt After Enter [COMPLETED]

**Status:** ✅ COMPLETED
**Priority:** P2 (High)
**Commit:** `635ae55`
**PR:** #112

User input now displays with `BOLD_CYAN` on `>>>` and `CYAN` on the text after pressing Enter, matching the User role label style in context display.

**Changes:**
- `src/chat/view/mod.rs`: Made `colors` module `pub` so repl.rs can access it
- `src/chat/repl.rs`: Import `colors`, styled `println!(">>> {}", line)` with ANSI colors

---

### ✅ SF2: Clippy Configuration [COMPLETED]

**Status:** ✅ COMPLETED
**Priority:** P2 (High)
**Commit:** `0e8b459`
**PR:** #112

Added `clippy.toml` with thresholds and `[lints.clippy]` in `Cargo.toml` to enforce code quality standards in CI and local dev.

**Changes:**
- `clippy.toml`: Thresholds for `too-many-arguments` (7), `cognitive-complexity` (25), `type-complexity` (250), `doc-valid-idents` (project-specific terms like AskAI, Vec0, GGUF, etc.)
- `Cargo.toml`: `[lints.clippy]` section — `too_many_arguments` and `type_complexity` as "warn", `missing_transmute_annotations` as "allow"

---

### ✅ SF3: Rename Database to `ask-ai.db` + `--db` CLI Flag [COMPLETED]

**Status:** ✅ COMPLETED (merged #113)
**Priority:** P2 (High)
**Issue:** #109

**Goal:** Rename `embeddings.db` → `ask-ai.db` and add `--db <path>` CLI flag for custom database location (useful for testing).

**Scope:**
1. Change 3 occurrences of `"embeddings.db"` → `"ask-ai.db"` in `src/db/connection.rs`
2. Add migration: if `embeddings.db` exists but `ask-ai.db` doesn't, auto-rename the file (preserve user data)
3. Add `--db <path>` global CLI flag in `src/main.rs` / `src/config.rs`
4. Propagate custom path through `Settings` / `get_storage_path()` with override
5. Update SMOKE_TEST.md paths
6. Update doc references (IMPLEMENTATION.md, architecture.md, etc.)

**Open questions:**
- Should `--db` override XDG path entirely, or just the filename?
- Should `--db :memory:` be supported for ephemeral testing?

---

### ✅ SF4: Logging Overhaul [COMPLETED]

**Status:** ✅ COMPLETED
**Priority:** P2 (High)
**Issue:** #110

**Goal:** Clean up log levels, add file-based logging, enforce data sensitivity policy.

**Implementation:**
1. **Replaced `env_logger` with custom `MultiLogger`** — Implements `log::Log` trait with dual output: colored stderr + file (`~/.local/share/sprachspiel/sprachspiel.log`). Removed `env_logger` dependency.
2. **Audited all `log::info!` calls** — Demoted operational info (recovery stats, execution notices) → `debug!`. Promoted service events (DB migration) → `warn!`. Target: 0 `info!` in normal operation. ✅
3. **Raised terminal default from `info` → `warn`** — Only warnings/errors visible by default. `-v` for debug, `-vv` for trace.
4. **Added file logging** — Default to `~/.local/share/sprachspiel/sprachspiel.log`. File always receives `warn+`. Trace mode raises file level to `info`. Rotation: 5 MB, keeps 1 backup (`.1`).
5. **Data sensitivity audit** — Truncated PII leakage in 3 locations:
   - `custom_coordinator.rs:609` — message content truncated to 80 chars
   - `dedup.rs:502,676` — fact content truncated to 80 chars
   - Added `truncate_for_log()` public helper + policy documented in `src/logging.rs` doc comments
6. **Verbosity alias update** — `"info"` alias removed (Normal now = warn), added `"warn"` alias

---

### ✅ SF5: Agent Spawning Tools [COMPLETED]

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

## Draft Priorities (M2, M4)

> **Note:** These are draft priorities — not yet issues or cards. They document planned work that has been researched and designed but not yet scheduled for implementation. For deferred topics and competitive research, see [Research Icebox](./doc/src/development/research-icebox.md).

---

### Benchmark Infrastructure — #124 [M2]

**Status:** 📋 DRAFT
**Depends on:** P6.0 (multi-provider, for cloud model benchmarks)
**Estimated effort:** 4-6 weeks (all tiers)
**Priority within M2:** End of milestone — last thing before public release

**Goal:** Establish benchmarks that validate sprachspiel's unique memory features with published numbers. No other CLI tool has published benchmarks for feedback-driven memory decay or retrieval-reinforced retention.

**Rationale:** Without benchmarks, claims about memory architecture are assertions. YourMemory published 59% Recall@5 on LoCoMo and established credibility. sprachspiel needs equivalent validation.

**Sub-items:**

| Sub | Description | Effort | Tier |
|-----|-------------|--------|------|
| B1.1 | LoCoMo benchmark adaptation (Recall@K, category breakdown, Stale Memory Precision) | 1-2 weeks | 1 (must-have before launch) |
| B1.2 | Custom "Feedback-Driven Decay" benchmark — first ever published (precision@K over time, decay curve alignment, retrieval-reinforced retention) | 2-3 weeks | 1 (must-have before launch) |
| B1.3 | RAG quality benchmark (BM25-only vs vector-only vs hybrid+RRF, with and without feedback boost) | 1 week | 2 (nice-to-have) |
| B1.4 | Stale Memory Precision benchmark (does decay correctly suppress outdated info?) | 3-5 days | 2 (nice-to-have) |

**Key metric targets:**

| Metric | Baseline (no memory) | YourMemory | sprachspiel Target |
|--------|---------------------|------------|----------------|
| LoCoMo Recall@5 | ~20-30% | 59% | ≥55% |
| Feedback impact on recall | N/A | N/A | +15-25% for "good" items |
| Stale memory precision | 0% | 100% (claimed) | ≥80% |

**Dependencies:** B1.1 requires P6.0 (multi-provider) to test with cloud models. B1.2 can run with Ollama only.

---

### Learned Patterns / Behavioral Intelligence — #125 [M2]

**Status:** 📋 DRAFT
**Depends on:** P5 (feedback — ✅ COMPLETED)
**Estimated effort:** 2-3 weeks
**Priority within M2:** Medium

**Goal:** Enable the system to detect and adapt to user behavior patterns, and provide system reminders triggered by operational events.

**Sub-items:**

| Sub | Description | Effort |
|-----|-------------|--------|
| B6.1 | System Reminders — `ReminderTrigger` enum (TurnCount, ToolFailure, ContextLow, etc.), templates for each trigger, conditional injection into context | 3-5 days |
| B6.2 | Auto-extraction of usage patterns — "User often asks about X" → boost relevant memories; "User prefers style Y" → adapt prompts | 5-7 days |
| B6.3 | Decay Management UI — `/memory stats` (show decay status), `/memory forget <id>` (manual decay boost), `/memory prune` (force decay of low-importance) | 3-5 days |

**Reference:** Unified Vision (see `doc/src/development/unified-vision.md`), Phase 5

---

### Verification Layer (Study Sessions) [M4]

**Status:** 📋 DRAFT
**Depends on:** P5 (feedback — ✅ COMPLETED)
**Estimated effort:** 3-4 weeks
**Priority within M4:** First (before B2)

**Goal:** Content that passes verification (quizzes, cross-model checks) receives importance boosts and slower decay, creating a "verified knowledge" tier in the memory system.

**Sub-items:**

| Sub | Description | Effort |
|-----|-------------|--------|
| B3.1 | `study` source type in `content_items` + differentiated importance (verified content starts at importance 0.9, decays with 180d half-life) | 2-3 days |
| B3.2 | `/study import <files>` and `/study quiz` commands — import study material, generate quiz questions, verify answers | 5-7 days |
| B3.3 | Verifier trait — `Verifier` trait with `verify()` method; CodeVerifier (sandbox + test execution), CrossModelVerifier (second model checks), StudyVerifier (quiz check) | 5-7 days |
| B3.4 | Best-of-N implementation — generate N candidates, verify each, return first that passes | 3-5 days |

**Reference:** Unified Vision (see `doc/src/development/unified-vision.md`), Phase 4

---

### Belief Engine Abstraction [M4]

**Status:** 📋 DRAFT
**Depends on:** B3 (Verification Layer — first in M4)
**Estimated effort:** 2-3 weeks
**Priority within M4:** After B3

**Goal:** Extract the contradiction detection engine from `conflict.rs` into a domain-independent `BeliefEngine` that can be used by both the fact store and the content store.

**Sub-items:**

| Sub | Description | Effort |
|-----|-------------|--------|
| B2.1 | Extract `BeliefEngine` from `conflict.rs` — `analyze()` returns `ConflictVerdict` without taking action; fact store calls `BeliefEngine::analyze()` then decides policy | 3-5 days |
| B2.2 | Content store calls `BeliefEngine` for belief revision — conflicting beliefs between conversations marked as "superseded" (not deleted) | 3-5 days |
| B2.3 | Expand verbs and PT patterns in `lang.rs` — more triple prefixes, PT irregular patterns, negation patterns | 2-3 days |
| B2.4 | Belief versioning — `invalidated_at` timestamp instead of delete (inspired by Kumiho AGM postulates; immutable revisions + mutable tag pointers) | 3-5 days |

**Refinement topics (see research-icebox.md):** R-05 (LLM adjudication for edge cases), R-06 (Crepe/Datalog rule engine), R-10 (multi-source belief reconciliation)

**Reference:** `doc/src/development/belief-system-design.md`

---

### Context Engineering Evolution [M4]

**Status:** 📋 DRAFT
**Depends on:** P6.0 (provider migration for `prompt_eval_count`)
**Estimated effort:** 4-6 weeks (filtered subset)
**Priority within M4:** B4.1 high, B4.2 medium, B4.3-B4.4 research

**Goal:** Improve context management with better token estimation, resilience patterns, and non-destructive archival — filtered from 10 recommendations in evolution research to only the viable ones.

**Sub-items:**

| Sub | Description | Effort | Priority |
|-----|-------------|--------|----------|
| B4.1 | **Anchor-based token estimation** — Calibrate word-based estimates using known token patterns (Rust keywords, Markdown structures, common code patterns). Avoids tiktoken dependency while improving accuracy from ~80% to ~90-95%. | 1 week | **High** |
| B4.2 | **Circuit breaker for context operations** — Rate limiting + fallback strategies for long-running context operations (embedding, compaction, recovery). Prevents cascading failures during concurrent access. | 1 week | Medium |
| B4.3 | **Importance-based eviction** — Evict content based on importance_score + feedback_weight rather than LRU during compaction. Research needed to understand how importance_score and feedback_score interact. | 2 weeks | 🟡 Research |
| B4.4 | **Non-destructive context collapse** — Archive original messages instead of deleting during compaction; enable on-demand recovery of archived content. Low priority, needs schema changes. | 2 weeks | Low |

**Excluded (see research-icebox.md R-01, R-02, R-03, R-04):** 5-level compression pipeline, tiktoken integration, speculative execution, attention-based prompt optimization.

**Token estimation philosophy:** The project prefers avoiding tiktoken wherever possible. Approach: (1) anchor-based estimation (B4.1), (2) `prompt_eval_count` from Ollama API (P6.0d) for exact post-hoc counts, (3) tiktoken only as last resort if both above prove insufficient.

---

### MCP Server (Memory as Service) [M4]

**Status:** 📋 DRAFT — needs further reflection
**Depends on:** P15 (MCP Client — Phase 1)
**Estimated effort:** 2-3 weeks
**Priority within M4:** Final — after other M4 items

**Goal:** Expose sprachspiel's memory system (feedback-driven decay, hybrid retrieval, fact dedup) as an MCP server that other tools (Claude Code, Cursor, Cline) can consume.

**Note:** This is distinct from P15 (MCP Client). P15 is about consuming external MCP servers. B5 is about providing sprachspiel's memory as an MCP server. They are complementary but independent.

**Open questions:**
- What to expose? Facts? Content? Search? All three?
- Authentication model for API access
- How decay and feedback signals translate to MCP tool interfaces
- Whether this positions sprachspiel as a CLI tool or a memory library/service

**Reference:** Research icebox R-09

**Relationship to B8 (ACP Agent Integration):** B8 (ACP) subsumes B5's use case. ACP's MCP-over-ACP capability allows ACP clients (editors) to inject MCP servers into sprachspiel sessions, providing the same tool-level access that B5 would offer. Additionally, ACP gives users session management, streaming, and the full agent experience — not just individual tool calls. If B8 is implemented, B5 becomes redundant unless there's demand for a standalone memory API that doesn't require ACP session setup. **Recommendation:** Implement B8 first, evaluate if B5 is still needed.

---

### Content Relations Graph — Priority Elevation — #78 [M3]

**Status:** Priority elevation: S2.2 from LOW → **MEDIUM**
**No new card or issue.** This records the decision to elevate S2.2's priority when M3 work begins.

**Rationale:** YourMemory's graph layer with BFS expansion is their killer feature after Ebbinghaus decay. It accounts for their LoCoMo performance lead. A memory system without content relations is incomplete in 2026.

---

### ACP Agent Integration [M2/M3]

**Status:** 📋 DRAFT
**Depends on:** P14 TUI (ApplicationBackend decoupling — B8.1, B8.2)
**Estimated effort:** 4-8 weeks
**Priority within M2/M3:** After P14 (TUI) decoupling. ACP requires ApplicationBackend trait.

**Goal:** Implement the Agent Client Protocol (ACP) to expose sprachspiel as an agent that editors (Zed, JetBrains, Neovim, VS Code) and other ACP-compatible clients can use directly, replacing the need for a standalone MCP Server (B5).

**Rationale:** ACP is the emerging standard (like LSP for language servers) for editor↔agent communication. Instead of exposing individual tools via MCP (B5), ACP exposes the entire agent — sessions, conversation history, tools, memory, facts, and all. This gives users the ability to use sprachspiel directly inside their editor with full session persistence, facts, and tool integration, rather than having another agent call sprachspiel's tools piecemeal.

**Key insight — ACP vs MCP:**
- **MCP Server (B5):** Exposes sprachspiel's memory as individual tools (search_facts, add_fact, etc). Other agents call these tools.
- **ACP Agent (B8):** Exposes sprachspiel as a complete agent. The user talks to sprachspiel inside their editor, with full sessions, tools, and memory.
- **ACP subsumes B5's use case** via MCP-over-ACP: ACP clients can inject MCP servers that provide the same individual tool access.
- **Recommendation:** Implement B8 first. B5 becomes a subset of B8's MCP-over-ACP capability, not a separate deliverable.

**Prerequisite: ApplicationBackend Decoupling (P14 architectural requirement)**

The TUI implementation must create a clean `ApplicationBackend` trait that separates core logic from I/O. Currently, `ChatCore` and `repl.rs` are tightly coupled — the REPL directly calls `send_message()` and renders output inline.

The TUI refactoring creates this separation (see P14 for the target architecture). ACP will be the third implementation:

```
ApplicationBackend (trait)
   ├── CLI (RustylineInput + TerminalView) — current
   ├── TUI (TuiInput + TuiView) — P14
   └── ACP (stdio JSON-RPC → AcpBackend) — B8
```

**Sub-items:**

| Sub | Description | Effort | Priority |
|-----|-------------|--------|----------|
| B8.1 | Create `ApplicationBackend` trait in `src/chat/backend.rs` with event stream architecture | 1 week | **High** (prerequisite for TUI and ACP) |
| B8.2 | Refactor `repl.rs` to use `ApplicationBackend` instead of direct ChatCore calls | 3-5 days | **High** (prerequisite) |
| B8.3 | Add `sprach acp` subcommand that starts ACP server over stdio | 1-2 weeks | High |
| B8.4 | Implement ACP Agent trait: initialize, session/new, session/load, session/prompt | 2-3 weeks | High |
| B8.5 | Bridge ChatEvent → ACP session/update notifications (text, tool_call, plan) | 1-2 weeks | High |
| B8.6 | Implement session/resume and session/close for SQLite persistence | 1 week | Medium |
| B8.7 | Tool call reporting: map CustomToolInfo to ACP tool_call notifications with kind/status | 3-5 days | Medium |
| B8.8 | Permission system: session/request_permission for destructive tool calls | 1 week | Medium |
| B8.9 | MCP-over-ACP: allow ACP clients to inject MCP servers into sprachspiel sessions | 2-3 weeks | Low (M4-later) |

**Architecture:**

```
sprachspiel (binário único)
├── CLI mode (atual)
│   └── RustylineInput + TerminalView → ApplicationBackend → ChatCore → Ollama
├── TUI mode (P14)
│   └── TuiInput + TuiView → ApplicationBackend → ChatCore → Ollama
└── ACP mode (B8)
    └── stdio JSON-RPC → AcpBackend → ApplicationBackend → ChatCore → Ollama
```

**What already maps to ACP:**

| ACP Concept | sprachspiel Equivalent | Status |
|-------------|-------------------|--------|
| session/new | ChatSession::new() | ✅ Exists |
| session/load | ChatSession::load_from_sqlite() | ✅ Exists |
| session/prompt | ChatCore::send_message() | ✅ Exists |
| session/update (agent_message) | ChatEvent::PreToolContent | ✅ Exists as events |
| session/update (tool_call) | ChatEvent::ToolCall | ✅ Exists as events |
| session/update (plan) | — | 🟡 Could use /plan |
| session/request_permission | — | ❌ New (B8.8) |
| session/cancel | — | ❌ New |
| Tool schemas (JSON Schema) | CustomToolInfo | ✅ Exists |
| fs/read_text_file (Client) | File tools exist | ✅ Adapt |
| terminal/create (Client) | run_command exists | ✅ Adapt |

**SDK choices:**

| SDK | Status | Recommendation |
|-----|--------|----------------|
| agent-client-protocol v0.x (trait-based) | Stable, published on crates.io | Start here, migrate later |
| agent-client-protocol v1.0 (SACP builder-based) | In development, more ergonomic | Migrate when stable |

**Open questions:**
- Transport: stdio only (matches ACP spec, simpler) or stdio + HTTP/SSE (remote access)?
- Authentication model for remote access (if HTTP transport added)
- How to handle tool permissions (some tools are destructive, ACP has request_permission)
- Relationship between ACP's MCP-over-ACP and B5 (MCP Server): should B5 be deprecated in favor of B8?
- SDK version: start with v0.x and migrate to v1.0 SACP when stable?

**Reference:**
- ACP specification: https://agentclientprotocol.com/
- ACP Rust SDK: https://agentclientprotocol.com/libraries/rust/
- ACP clients: Zed, JetBrains, Neovim (CodeCompanion/Avante), VS Code, Obsidian, Unity
- ACP agents: Claude Agent, Codex CLI, OpenCode, Cline, Cursor CLI, Gemini CLI
- OpenCode ACP support: https://opencode.ai/docs/acp/

---

### Privacy Filter Integration (PII Redaction Sidecar) [M3]

**Status:** 📋 DRAFT
**Depends on:** None (sidecar is standalone)
**Estimated effort:** 2-3 days
**Priority within M3:** After core S2.x items; defensive improvement

**Goal:** Integrate the OpenAI Privacy Filter model (1.4B params, Apache 2.0) as an optional Python sidecar for PII detection and redaction in facts, logs, and tool outputs.

**Architecture:** Python sidecar on localhost:8199 (ONNX rejected per D-06). sprachspiel calls via HTTP. Must be optional — falls back to `truncate_for_log()`.

**Integration points:**

| Point | File | Description |
|-------|------|-------------|
| Fact redaction | `src/facts/prompt.rs` | Redact PII in facts before injecting into system prompt |
| Log sanitization | `src/logging.rs` | Replace `truncate_for_log()` with semantic PII detection |
| Tool output scrubbing | `src/chat/custom_coordinator.rs` | Redact PII in tool results before context injection |
| Context overflow | `src/context_overflow.rs` | Redact PII in compacted summaries (future) |

**Config:**

```toml
[privacy_filter]
enabled = true
sidecar_url = "http://localhost:8199"
mode = "replace"  # replace | tag | remove
fallback = "passthrough"  # passthrough | truncate | block
categories = []  # empty = all 8 categories
timeout_ms = 2000
```

**Open questions:** Sidecar lifecycle, caching strategy, PT-BR boundary issues, false positive tolerance on code.

**Refinement topics (see research-icebox.md):** R-18 (Rust-native classifier as long-term goal)

**Source:** Privacy filter integration proposal (internal analysis)

---

### ADR: Empathy Is Not Failure, Opacity Is [M3]

**Status:** 📋 DRAFT
**Depends on:** None
**Estimated effort:** ~1 hour
**Priority within M3:** First — must be written before #99/#100/#101 implementation

**Goal:** Formalize the architectural decision that the system should not suppress empathetic responses, but make behavioral shifts visible and offer the user a choice. This reframes meta-cognition from "detect and correct failures" to "detect changes and make them visible."

**Key insight:** When the system shifts tone (e.g., analytical → supportive), the empathy is not a bug. The opacity about the shift is. The correct behavior is: name the change and ask the user which mode they prefer.

**Implications for existing cards:**
- #99 (Layer 1 Skill): Include the reframed guardrail — not "never use phenomenological language" but "never claim phenomenology misleadingly; name what's happening and offer choice"
- #100 (Layer 2 Telemetry): Detector should focus on unannounced system drift, not user-initiated topic changes
- #101 (Layer 3 Reflection): Validation step is mandatory — system must ask user before classifying a behavioral shift as a failure

**Source:** Meta-cognition brainstorm (internal analysis, Section 0.5)

---

### meta_cognize() Active Behavioral Tool [M3]

**Status:** 📋 DRAFT
**Depends on:** #100 (Behavioral Telemetry Layer 2 — produces the data this tool returns)
**Estimated effort:** 2-3 days
**Priority within M3:** After Layer 2 (#100)

**Goal:** LLM-callable tool that returns the current behavioral state: detected mode, whether shift was confirmed, suggestions. Complements passive Layer 2 telemetry by making behavioral reflection explicit and traceable. Each `meta_cognize()` call produces structured data that feeds Layer 3 (#101) reflection pipeline.

**Example output:**

```json
{
  "current_mode": "supportive",
  "mode_confirmed": false,
  "shift_detected": true,
  "shift_turn": 11,
  "suggestion": "Ask user which mode they prefer"
}
```

**Source:** Meta-cognition brainstorm (internal analysis, Section 4.2)

**Refinement topics (see research-icebox.md):** R-14 (full research record)

---

### Behavioral Conflict Detection (SOUL.md vs Emergent) [M3]

**Status:** 📋 DRAFT
**Depends on:** #77 and #78 (Visualize Connections + Relations Graph — structural foundation)
**Estimated effort:** 3-5 days
**Priority within M3:** After S2.1/S2.2

**Goal:** Detect tensions between configured personality (SOUL.md) and emergent behavioral patterns. Analogous to factual contradiction detection but for personality: "SOUL.md says 'challenge premises', but operational pattern is 'shift to supportive on vulnerability'."

**Source:** Meta-cognition brainstorm (internal analysis, Section 4.3)

**Refinement topics (see research-icebox.md):** R-15 (full research record)

---

### Attention Priming (Chunk Reordering) [M4]

**Status:** 📋 DRAFT — Quick Win
**Depends on:** None
**Estimated effort:** ~1 day
**Priority within M4:** First (quick win opener)

**Goal:** Reorder retrieved chunks to position top-2 at the beginning and next 2 at the end of context. Mitigates "Lost in the Middle" effect (Liu et al. 2023, Cuconasu et al. 2025) with zero architecture change — only reordering in `format_retrieved_context()`.

**Implementation:** `[best, 2nd_best, ...middle..., 3rd_best, 4th_best]`

**Source:** RAG improvement research (internal analysis, Section 5)

---

### Context-Aware Chunking (SemanticChunker) [M4]

**Status:** 📋 DRAFT
**Depends on:** None (replaces current TokenChunker). Token-aware chunking (Prioridade 3) confirmed in W4.4 (#107).
**Estimated effort:** 3-5 days
**Priority within M4:** After Attention Priming

**Goal:** Replace fixed-size chunking with semantic chunking that respects paragraph/sentence boundaries. Paragraph Group Chunking reaches nDCG@5 of 0.459 vs <0.244 for fixed (Shaukat et al. 2026). Config: `[embedding] chunking = "semantic" | "fixed"`.

**Algorithm:** Split by `\n\n` → sentences (regex) → fallback to token boundary with overlap. Preserve section metadata (nearest heading). This covers SOTA Prioridade 4 (recursive character splitting with separator hierarchy) and document-aware chunking (headers/code blocks).

**SOTA evolution mapping (2025-2026 research):**
- **Prioridade 3 (token-aware chunking):** Replace chars/token estimate with real tokenizer counts via `/tokenize` endpoint or tokenizer crate. Eliminates the root cause of chunk sizing bugs. **Confirmed in W4.4 (#107 — Embedding provider abstraction)** as part of the provider abstraction work.
- **Prioridade 4 (recursive character splitting):** Already covered by this M4 draft — the `\n\n` → `\n` → sentence → token hierarchy IS recursive character splitting.
- **Document-aware chunking:** Already covered by this M4 draft ("Preserve section metadata (nearest heading)"). After milestone 2 (TUI).
- **Semantic chunking (embedding similarity):** NOT recommended for sprachspiel — 4-5x indexing cost, not justified for a local chat app.

**Source:** RAG improvement research (internal analysis, Section 1)

**Competitive research (see research-icebox.md):** C-12 (Shaukat et al.)

---

### Metadata Enrichment (Chunk Authority & Recency) [M4]

**Status:** 📋 DRAFT
**Depends on:** Schema v13 (new `chunk_metadata` table)
**Estimated effort:** 1-2 weeks (Phase 1: static metadata), 1 week (Phase 2: version + recency)
**Priority within M4:** After Context-Aware Chunking

**Goal:** Annotate chunks at ingestion time with entity_type, source, authority (0.0-1.0), recency (exponential decay), version. RRF scoring: BM25(0.3) + cosine(0.4) + metadata_boost(0.3). Addresses ClashEval finding that LLMs overwrite correct knowledge with incorrect retrieved evidence >60% of the time when no authority signal exists.

**Source:** RAG improvement research (internal analysis, Section 2)

**Competitive research (see research-icebox.md):** C-13 (ClashEval)

---

### Semantic Deduplication Pre-Indexing [M4]

**Status:** 📋 DRAFT
**Depends on:** None
**Estimated effort:** 3-5 days (Option A: cosine similarity)
**Priority within M4:** Batch job — can run alongside other M4 work

**Goal:** Clustering by cosine similarity (threshold ~0.92) to eliminate near-duplicate chunks before indexing. Runs as offline batch job (`sprach reindex --dedup`), NOT in hot ingestion path. Option A: O(n²) vector comparison (simple, works for ≤100k chunks). Option B: MinHash+LSH (scales better, adds dependency).

**Source:** RAG improvement research (internal analysis, Section 4)

---

### Q&A Pairing / HyDE-like Embedding [M4]

**Status:** 📋 DRAFT
**Depends on:** #106 (Configurable Embedding Model — needs smaller model for enrichment)
**Estimated effort:** 1-2 weeks
**Priority within M4:** After Metadata Enrichment

**Goal:** Generate question-answer pairs per chunk at ingestion time using a small LLM (e.g., qwen3:0.6b). Embed the question instead of raw text, moving embeddings closer to query distribution. HyDE (Gao et al. 2022) demonstrates this improves retrieval significantly. Stored as `enriched_question` column. Config: `[embedding] enrich = true`.

**Trade-off:** Inference cost proportional to corpus size. Worth it for static documents that are queried repeatedly; not worth it for dynamic chat messages.

**Source:** RAG improvement research (internal analysis, Section 3)

**Competitive research (see research-icebox.md):** C-14 (HyDE + Dense X Retrieval)

---

### Behavioral Embeddings (Conversation Mode Vectors) [M4]

**Status:** 📋 DRAFT — Research
**Depends on:** #100 (Layer 2 telemetry producing calibration data)
**Estimated effort:** 1-2 weeks
**Priority within M4:** Low — premature without Layer 2 data

**Goal:** Train vector representations of conversation mode for more precise shift detection than heuristic keyword matching. Enables mode clustering, cross-session behavioral similarity, and pattern recognition. Evolution of Layer 2 telemetry.

**Source:** Meta-cognition brainstorm (internal analysis, Section 4.1)

**Refinement topics (see research-icebox.md):** R-13 (full research record)

---

### Feedback × Meta-cog Integration (Behavioral RRF) [M4]

**Status:** 📋 DRAFT
**Depends on:** #100 (Layer 2) and #101 (Layer 3) being stable
**Estimated effort:** 1 week
**Priority within M4:** Final — after meta-cognition layers are proven

**Goal:** Add behavioral_alignment as second signal in RRF score alongside content feedback. Responses generated in unconfirmed behavioral mode have reduced retrieval weight. Behavioral decay: patterns that user consistently redirects decay faster (analogous to Ebbinghaus but for habits, not facts).

**Source:** Meta-cognition brainstorm (internal analysis, Section 3)

---

### S2.6 — PCA Projection Search for Vector Retrieval [M3] — #139

**Status:** 📋 DRAFT
**Depends on:** W4 geometry work (#133, #136)
**Estimated effort:** 2-3 weeks
**Priority:** Medium (enables S2.1 #77 and S2.2 #78 graph features)

**Goal:** Implement PCA projection for embedding dimensional reduction at query time, improving retrieval speed and reducing noise from low-information dimensions. The audit found d_eff=7 for the current model, meaning PCA can dramatically reduce vector size while preserving discriminative signal.

**Algorithm:** Compute PCA on a sample of stored embeddings → project queries and stored vectors to d_eff dimensions → cosine similarity on projected vectors. Projection matrix computed once per reindex, stored alongside embeddings.

**Why M3, not M1:** PCA requires the graph substrate from S2.1/S2.2 to be useful. The d_eff diagnostic (W4.0) and geometry-aware dimensions (W4.5) are prerequisites that define the PCA target dimensionality.

**Source:** Embedding Geometry Audit — PCA as alternative to Matryoshka truncation for d_eff ≤ 10 models.

**Issue:** #139

---

### S2.7 — Embedding Geometry Documentation & Model Selection Guide [M3] — #140

**Status:** 📋 DRAFT
**Depends on:** W4.7 (#138) — extends the M1 documentation rewrite with full academic references
**Estimated effort:** 1 week
**Priority:** Medium

**Goal:** Complete reference documentation covering:
1. **Model Selection Guide** — Criteria for choosing embedding models (d_eff, multilingual, context length, cost, Matryoshka support)
2. **Hybrid Search Explanation** — How BM25 + cosine + RRF work, why weights adapt by d_eff
3. **SPREAD System Reference** — θ∈{0°, 30°, 60°, 90°} and what similarity values mean at each angle
4. **Provider Configuration** — How to configure OpenAI-compatible providers (llama.cpp, LM Studio, vLLM, cloud APIs) in `config.toml`
5. **Benchmark Results** — d_eff, retrieval quality, and performance comparisons across models

**Why M3, not M1:** M1 documentation (#138) covers operational guidance (how to configure, how to switch models). M3 documentation (#140) adds academic depth (why SPREAD matters, how d_eff relates to information theory, model comparison table with citations).

**Source:** Embedding Geometry Audit — documentation deliverables.

**Issue:** #140

---

### B9 — Geometry-Aware Consolidation (GAC) [M4] — #141

**Status:** 📋 DRAFT
**Depends on:** B10 (#142 — Memory Consolidation Design), S2.1 (#77 — Content Relations Graph), S2.2 (#78 — Graph Query Engine)
**Estimated effort:** 3-4 weeks
**Priority:** Low

**Goal:** Use embedding geometry (d_eff, SPREAD angles) to optimize memory consolidation. Instead of uniform decay, facts and content items with high geometric utility (wide angular spread, high d_eff position) decay more slowly. Items that cluster near existing knowledge (low angular distance) decay faster — they're redundant.

**Why M4:** GAC requires the graph substrate from S2.1/S2.2 to compute angular distances between knowledge items. It also requires B10 (consolidation design) to define what "consolidation" means operationally. Both prerequisites are M3/M4 scope.

**Source:** Embedding Geometry Audit — consolidation recommendation.

**Issue:** #141

---

### B10 — Memory Consolidation Design [M4] — #142

**Status:** 📋 DRAFT
**Depends on:** S2.1 (#77 — Content Relations Graph), S2.2 (#78 — Graph Query Engine)
**Estimated effort:** 1-2 weeks (design only, no implementation)
**Priority:** Low

**Goal:** Design document for how memory consolidation works in Sprachspiel:
1. What triggers consolidation? (time-based, access-based, geometry-based)
2. What happens to consolidated items? (merge, archive, compress)
3. How does consolidation interact with existing decay (Ebbinghaus) and importance (feedback)?
4. How does consolidation differ from deletion? (information preservation vs. removal)
5. What is the user experience? (visibility, undo, confirmation)

**Why M4:** Design document only. S2.1/S2.2 graph features are prerequisite because consolidation requires understanding item relationships.

**Source:** Embedding Geometry Audit — consolidation design prerequisite for GAC (B9).

**Issue:** #142

---

## Documentation

Full documentation is available in the `doc/` directory:

```bash
# View user documentation
cd doc
mdbook serve

# Or build static site
mdbook build

# View man page
man sprach
```

## For Developers

See the development documentation:

1. [Architecture](./doc/src/development/architecture.md) - Technical architecture
2. [Roadmap](./doc/src/development/roadmap.md) - Future plans
3. [Contributing](./doc/src/development/contributing.md) - How to contribute
4. [Context Composition Design](./doc/src/development/context_composition_design.md) - v0.21.0 design decisions

## Legacy Content

The original detailed implementation notes have been moved to:

- `doc/src/development/architecture.md` - Architecture decisions
- `doc/src/development/roadmap.md` - Future plans
- `doc/src/CHANGELOG.md` - Version history

## Last Updated

2026-04-11 - P6 Core Enhancements added, milestone tags [M1]/[M2]/[M3], P4 extras, P5 verbosity merge, P15 sub-items with scope clarification
2026-04-25 - Milestones restructured: M2→UX & TUI Design (design phase), M3→Sprach 2.0+CAS+TUI impl+Plugin System, M4→Future (was M3). P14 TUI split into M2(design) and M3(impl). P7,P14,P15 moved from M2 to M3.
2026-04-27 - SF1 (colored prompt) and SF2 (clippy config) completed. SF3 (db rename), SF4 (logging), SF5 (PDF pipeline) documented as NOT STARTED.
2026-04-27b - SF3 (db rename + --db flag) completed and merged (#113). SF4 (logging overhaul) completed.
2026-04-27c - SF4 merged (#114). SF5 (PDF vision pipeline) completed.
2026-04-27d - SF5 revised per PR review: replaced spawn_subagent with 4 dedicated spawning tools, removed spawn_document_agent, removed PDF pipeline from Rust, removed FileType::Pdf/Epub, updated document-processing skill.
2026-04-28 - P6.0 decomposed into 7 sub-phases (P6.0a–P6.0g) for full ollama-rs removal. Added P6.0a (retry threshold with backoff). Added P14.IM (TUI interaction modes: /queue, /steer). Updated milestones M1–M3 with provider migration and interaction modes.
2026-04-28 - Draft priorities B1-B7 added. Milestones restructured: M2 now includes B1 (benchmarks) and B6 (learned patterns). M4 now has structured draft priorities (B2-B5). S2.2 (Content Relations) elevated to MEDIUM. Research icebox created at doc/src/development/research-icebox.md.
2026-04-29 - Added B8 (ACP Agent Integration) as draft priority. Updated P14 to include ApplicationBackend decoupling as architectural requirement for TUI/ACP. Updated B5 to note subsumption by B8 (ACP's MCP-over-ACP). Added R-11 (ACP) and R-12 (ApplicationBackend) to research icebox. Updated R-09 (MCP Server) to reference B5/B8.
2026-04-30 - M1 reorganized into 3 phases (Feedback+QuickWins → P6.0 Core → Low Priority). P6.5 consolidated with P1 #105 (duplicate). P5.1 verified as ~95% implemented (ADR-008/009). #103 and #17 marked for closure (obsolete). #90 (P5.1) flagged for verification and potential closure.
2026-05-07 - M1 implementation waves formalized (W1-W5) with themes, cards, and completion criteria. Board TODO column reordered by implementation priority. #90 Scrum Status moved to Ready (decay_score fix merged).
2026-05-07 - New board drafts from idea triage: M3 (Privacy Filter, ADR: Empathy, meta_cognize tool, Behavioral Conflict) and M4 (Attention Priming, Semantic Chunking, Metadata Enrichment, Semantic Dedup, HyDE, Behavioral Embeddings, Behavioral RRF). Added R-13 through R-18, C-11 through C-14, D-06 to research icebox. ONNX for Privacy Filter explicitly rejected (D-06).
2026-05-29 - Bug Fix: Compaction Overflow (Issue #187). 3-layer progressive compaction strategy to handle context exceeding model window during summarization.

 ## Bug Fix: Compaction Overflow (Issue #187) — ✅ COMPLETED

When a conversation's context exceeds the model's context window, `/compact` fails with `"The prompt is too long"` because the compaction prompt itself exceeds the window. This is a chicken-and-egg problem: you need compaction to reduce context, but compaction requires sending the full context to the model.

**Root Cause:** `compact_conversation()` in `src/chat/core.rs` constructs a single prompt with ALL middle messages and sends it to the model. If the middle section exceeds the model's context window, the API call fails. There is NO handling for this case.

**Research:** Claude Code implements a 5-tier cascade (microcompact → snip → context collapse → auto compact → reactive compact with PTL retry). OpenCode has aggressive pre-pruning of tool outputs before summarization. Academic literature (arXiv:2308.15022) validates recursive summarization for long dialogue memory.

**3-Layer Strategy:**

| Layer | Name | Mechanism | Status |
|-------|------|-----------|--------|
| 1 | Pre-pruning | Strip long tool outputs (>500 chars) before sending to LLM | ✅ COMPLETED |
| 2 | Chunked recursive summarization | Split into chunks, summarize each, combine summaries, recurse if needed | ✅ COMPLETED |
| 3 | Fallback truncation | Drop oldest middle messages until prompt fits in 50% of window | ✅ COMPLETED |

**Implementation phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Pre-pruning: `pre_prune_messages()` truncates tool outputs > `PRUNE_TOOL_RESULT_THRESHOLD` | ✅ COMPLETED |
| 2 | Fallback truncation: `fallback_truncate()` drops oldest middle messages to fit window | ✅ COMPLETED |
| 3 | Integration: `compact_conversation()` uses Layer 1 then Layer 2 before LLM call | ✅ COMPLETED |
| 4 | Chunked recursive summarization: `split_into_chunks()` + `compact_recursive()` | ✅ COMPLETED |
| 5 | Tests for all 3 layers | ✅ COMPLETED (14 new tests) |
| 6 | Documentation (CHANGELOG, IMPLEMENTATION, context-anatomy.md, architecture.md, ADR) | ✅ COMPLETED |

**New constants (`src/context_overflow.rs`):**

| Constant | Value | Purpose |
|----------|-------|---------|
| `PRUNE_TOOL_RESULT_THRESHOLD` | 500 | Min chars before truncating tool outputs |
| `PRUNE_TOOL_RESULT_KEEP_CHARS` | 100 | Chars to keep from truncated tool output |
| `COMPACTION_MAX_CONTEXT_RATIO` | 0.60 | Max ratio of context window per chunk |
| `MAX_RECURSION_DEPTH` | 3 | Max recursion levels for chunked summarization |
| `TRUNCATION_TARGET_RATIO` | 0.50 | Target ratio after fallback truncation |
| `COMPACTION_PROMPT_OVERHEAD` | 3000 | Reserved tokens for prompt + response (was 2500, increased for accuracy) |
| `COMPACT_MSG_OVERHEAD` | 10 | Per-message overhead in compaction (vs. `MESSAGE_OVERHEAD=4` elsewhere) |
| `ESTIMATION_SAFETY_MARGIN` | 1.20 | 20% buffer on token estimates to compensate for underestimation |

**New functions (`src/context_overflow.rs`):**

| Function | Purpose |
|----------|---------|
| `pre_prune_messages()` | Strip long tool outputs, keep first 100 chars + notice |
| `estimate_messages_tokens()` | Token estimation for SavedMessage list (standard, used for thresholds) |
| `estimate_compaction_tokens()` | Token estimation for compaction with 20% safety margin and `COMPACT_MSG_OVERHEAD` |
| `fits_in_context()` | Check if messages fit within context window (uses `estimate_compaction_tokens`) |
| `max_chunk_tokens()` | Calculate chunk size for recursive summarization |
| `split_into_chunks()` | Split messages into token-bounded chunks with overlap |
| `fallback_truncate()` | Drop oldest middle messages to fit context window |
| `is_prompt_too_long_error()` | Detect Ollama overflow errors for error-retry |

**New functions (`src/chat/core.rs`):**

| Function | Purpose |
|----------|---------|
| `compact_recursive()` | Recursively summarize chunks using `Box::pin` for async recursion |
| `build_conversation_text()` | Format messages into conversation text (extracted from `compact_conversation`) |
| `compact_with_llm()` | Send compaction prompt to LLM and return summary (extracted from `compact_conversation`) |

**Defense in depth (error-retry):**

| Layer | Error Recovery | Behavior |
|-------|---------------|----------|
| 1 (single-pass) | `is_prompt_too_long_error()` | Catches "prompt too long" from Ollama, falls through to Layer 2 |
| 2 (chunked) | Already catches all errors | Falls through to Layer 3 |
| 3 (truncation) | `is_prompt_too_long_error()` | Catches "prompt too long", returns detailed diagnostics |

**Bug Fix: Estimation Undercount & Error Recovery (post-PR#188)**

**Problem:** `fits_in_context()` used `estimate_tokens()` (words/0.75 heuristic) with `MESSAGE_OVERHEAD=4` per message and `COMPACTION_PROMPT_OVERHEAD=2500`. This underestimated real token counts by 15-40% for mixed-content conversations (code, Portuguese text, tool JSON). When the estimate said "fits" but the LLM rejected the prompt as "too long", compaction failed with no recovery — Layer 2 and 3 were never reached because the decision was made *before* the LLM call.

**Fix:**
1. `is_prompt_too_long_error()` detects overflow errors from Ollama
2. Layer 1 (single-pass) catches overflow errors and falls through to Layer 2
3. Layer 3 (truncation) catches overflow errors and returns actionable diagnostics
4. `ESTIMATION_SAFETY_MARGIN = 1.20` — 20% buffer on token estimates
5. `COMPACT_MSG_OVERHEAD = 10` — realistic per-message overhead for compaction
6. `COMPACTION_PROMPT_OVERHEAD = 3000` — accounts for all prompt components

| What changed | Old | New |
|--------------|-----|-----|
| `fits_in_context()` | Used `estimate_messages_tokens()` (MESSAGE_OVERHEAD=4) | Uses `estimate_compaction_tokens()` (COMPACT_MSG_OVERHEAD=10, ×1.20 safety) |
| `COMPACTION_PROMPT_OVERHEAD` | 2500 (local const in core.rs) | 3000 (public const in context_overflow.rs) |
| `compact_conversation()` | Layer 1 returns `Err` on LLM overflow | Layer 1 catches overflow, falls through to Layer 2 |
| Layer 3 | Returns raw error on LLM overflow | Catches overflow, returns detailed diagnostics |

**Affected Code:**

| File | Change |
|------|--------|
| `src/context_overflow.rs` | Added `COMPACT_MSG_OVERHEAD`, `COMPACTION_PROMPT_OVERHEAD` (public, was local), `ESTIMATION_SAFETY_MARGIN`, `estimate_compaction_tokens()`, `is_prompt_too_long_error()`. Modified `fits_in_context()` to use `estimate_compaction_tokens()`. Added 11 new tests. |
| `src/chat/core.rs` | Layer 1 error-retry: catches "prompt too long" and falls through to Layer 2. Layer 3 error-retry: catches "prompt too long" and returns detailed diagnostics. Removed 2 local `COMPACTION_PROMPT_OVERHEAD` constants (replaced by public const). Uses `estimate_compaction_tokens()` instead of `estimate_messages_tokens()`. |
| `doc/src/CHANGELOG.md` | Added defense-in-depth and estimation fix details to #187 entry |

2026-05-30 - Bug Fixes: Context Prompt Corrections (PR #188). User message duplication, /retry wrong message, continuation empty message, system prompt clarity improvements.

 ## Bug Fixes: Context Prompt Corrections (PR #188) — ✅ COMPLETED

Six fixes targeting LLM prompt construction bugs and system prompt clarity issues identified during compaction analysis.

### Bug 1: User Message Duplication in LLM Prompt

**Problem:** Every user message appeared twice in the prompt sent to the LLM. `add_user_message()` (called in `handle_user_message_stream()`) added the user message to `session.messages`. Then `build_context()` included it via `session.messages[start_idx..]`, AND `prepare_messages()` also added `ChatMessage::user(user_input)` at the end.

**Root cause:** `build_context()` and `prepare_messages()` both added the current query — `build_context()` from session history, `prepare_messages()` as the explicit query position.

**Fix:** `build_context()` now calculates `end_exclusive` that excludes the last User message from `session.messages[start_idx..end_exclusive]`. Since `prepare_messages()` always adds the current query at position 6 (after recent messages), the user message appears exactly once. Uses `saturating_sub(1)` for safety.

**Edge cases tested:**
- Last message is User → excluded (normal chat path)
- Last message is Assistant → `end_exclusive = len` (retry path)
- Last message is Tool → `end_exclusive = len` (tool response path)
- Empty session → `end_exclusive = 0`
- Single User message → `end_exclusive = 0` (fully excluded, added by prepare_messages)

**Files:** `src/retrieval/context_builder.rs` (end_exclusive calculation + 5 new tests)

### Bug 2: `/retry` Used Wrong User Message

**Problem:** `handle_retry()` called `remove_last_assistant_messages()` which removes assistant messages AND the preceding user message. Then `get_last_user_message()` searched `session.messages` — but the correct message was already removed, returning the previous user message (or none).

**Additional problem:** The user message was never restored to `session.messages`, leaving the session history broken after `/retry` (missing user message from the conversation).

**Fix:** Capture user content BEFORE removal with `get_last_user_message()`. After removal, restore it with `add_user_message()` and `save_sqlite()`. Then send the correct content to `send_message()`. Early return when no user message exists or no assistant messages to remove.

**Files:** `src/chat/command_handlers.rs` (handle_retry rewrite)

### Bug 3: Continuation Injected Empty User Message

**Problem:** The continuation path called `send_message()` with `user_input=""`. `prepare_messages()` unconditionally pushed `ChatMessage::user("")` into the prompt. The actual continuation prompt was already added as an ephemeral message by `coordinator.push_ephemeral()`, making the empty user message redundant and confusing for the LLM.

**Fix:** `prepare_messages()` now skips adding `ChatMessage::user()` when `user_input.is_empty()`. Only affects the continuation path — normal chat never submits empty input.

**Files:** `src/chat/core.rs` (prepare_messages conditional)

### Fix 4: Compaction Prompt Staleness Labels

**Problem:** `COMPACTION_PROMPT`'s "DO NOT include" list didn't mention staleness labels like `(stale)`, `(62 days ago)`, `(unused)`. When the LLM summarized facts with these labels, the relative dates became inaccurate over time (e.g., "62 days ago" in a summary is wrong days or weeks later).

**Fix:** Added staleness labels to the "DO NOT include" list in `COMPACTION_PROMPT` with explanation that they become inaccurate.

**Files:** `src/prompts/base.rs` (COMPACTION_PROMPT)

### Fix 5: Instruction Hierarchy Examples Less Confusing

**Problem:** The INSTRUCTION HIERARCHY examples used `"rm is not authorized"` (prohibition) for USER FACTS and `"confirm before destructive"` (overlapping behavior) for SOUL. This gave the false impression that USER FACTS and SOUL conflict, when they're actually complementary (USER FACTS > SOUL resolves any conflict).

**Fix:** Changed examples to `"rm requires confirmation"` (preference) for USER FACTS and `"be concise"` (clearly different concern) for SOUL. Now the hierarchy examples show distinct, non-overlapping concerns.

**Files:** `src/prompts/base.rs` (SYSTEM_PROMPT_BASE)

### Fix 6: Language Note for Mixed-Language Facts

**Problem:** PT→EN normalization only translates prefixes, leaving objects in Portuguese (e.g., "User prefers respostas curtas"). This mixed-language output confused the LLM, which saw it as a formatting error.

**Fix:** Added a note in the `### LANGUAGE` section: "USER FACTS may contain mixed language (English subject, Portuguese object) due to automatic normalization. Interpret them semantically, not literally."

**Files:** `src/prompts/builder.rs` (LANGUAGE section)

**Affected Code:**

| File | Change |
|------|--------|
| `src/retrieval/context_builder.rs` | `end_exclusive` calculation excludes last User message. 5 new tests covering edge cases. |
| `src/chat/command_handlers.rs` | `handle_retry()` captures user content before removal, restores with `add_user_message()`, saves session. |
| `src/chat/core.rs` | `prepare_messages()` skips `ChatMessage::user()` when `user_input.is_empty()`. |
| `src/prompts/base.rs` | INSTRUCTION HIERARCHY examples updated. COMPACTION_PROMPT staleness label exclusion. |
| `src/prompts/builder.rs` | LANGUAGE section: mixed-language note added. |
| `doc/src/CHANGELOG.md` | All 6 entries documented. |
| `doc/src/development/context-anatomy.md` | User message deduplication note, staleness label note, User Facts format updated. |

2026-05-07 - #126 created: Rename ask-ai → Sprachspiel (priority:critical). Full codebase audit: ~60 source files + 82 doc files + config/data directory paths + man page + DB filename. 2-4 days estimated.