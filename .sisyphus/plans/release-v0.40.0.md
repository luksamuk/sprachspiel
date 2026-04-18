# Release v0.40.0 — Documentation Update, Semver & GitHub Release

## TL;DR

> **Quick Summary**: Create a stable v0.40.0 release for ask-ai with complete documentation updates, semver versioning in the changelog, and full GitHub release pipeline (tarballs + tag + release notes).
> 
> **Deliverables**:
> - Version bumped to 0.40.0 across all files (Cargo.toml, man page, source code, architecture docs)
> - CHANGELOG.md restructured: duplicate `### Changed` merged, `[Unreleased]` → `[0.40.0] - 2026-04-17`
> - All user-facing docs updated to reflect v0.40.0 changes (removed -d/--debug, new -v/-vv, /skill subcommand, /forget --yes, enhanced todos, etc.)
> - 4 distribution tarballs generated via `make all-tarballs`
> - Annotated git tag `v0.40.0` pushed to remote
> - GitHub release created with tarballs and structured release notes
> 
> **Estimated Effort**: Medium
> **Parallel Execution**: YES - 4 waves
> **Critical Path**: Version bump → Docs update → CHANGELOG → Build tarballs → Tag → Release

---

## Context

### Original Request
User wants to: (1) update documentation, (2) add semantic versioning to the changelog, (3) create a new stable GitHub release. The version number must follow semver conventions and match the project's existing release patterns.

### Interview Summary
**Key Discussions**:
- Version: v0.40.0 chosen (MINOR bump from v0.39.5, justified by ~10 new features)
- Documentation scope: COMPLETE — all user-facing docs must reflect new features
- Tarballs: User explicitly confirmed they must be created as part of the release
- Release format: Follow existing patterns (v{semver} tags, titled releases, 4 tarballs)

**Research Findings**:
- Version 0.39.5 appears in 7+ locations across the codebase (Cargo.toml, man page, source code, architecture docs, HTML file)
- CHANGELOG.md has a structural issue: duplicate `### Changed` header at line 68 that must be merged with line 33
- Multiple doc files still reference removed `-d/--debug` flag (8+ files affected)
- IMPLEMENTATION.md references future versions (v0.39.6, v0.39.7) that were never released — needs cleanup
- README.md/project README shows Pokémon as default feature but it's actually opt-in
- Source code has hardcoded version strings in `terminal.rs` and `mod.rs`

### Metis Review
**Identified Gaps** (addressed):
- Duplicate `### Changed` in CHANGELOG: Include merge task in plan
- -d/--debug references in docs: Include cleanup task in docs update
- Version in more files than expected: ARCHITECTURE.md, ask-ai-architecture.html, source code .rs files
- Pokémon default feature mismatch in README: Include fix in docs
- IMPLEMENTATION.md references unreleased versions (v0.39.6, v0.39.7): Clean up to v0.40.0
- Cargo.lock needs commit after version bump: Include in build task
- Termux cross-compilation prerequisite for `make all-tarballs`: Document as prerequisite

---

## Work Objectives

### Core Objective
Release ask-ai v0.40.0 with all documentation accurately reflecting current features and behavior.

### Concrete Deliverables
- Version `0.40.0` in all files that currently reference `0.39.5`
- CHANGELOG.md with semver-dated section `[0.40.0] - 2026-04-17`
- All doc files updated to remove stale `-d/--debug` references and add `-v/-vv` verbosity
- 4 tarballs in `dist/` directory
- Git tag `v0.40.0` on remote
- GitHub release at `https://github.com/luksamuk/ask-ai-rs/releases/tag/v0.40.0`

### Definition of Done
- [ ] `cargo build --release` succeeds with version 0.40.0
- [ ] `grep -r "0.39.5" --include="*.toml" --include="*.1" --include="*.rs" --include="*.html" --include="*.md"` returns zero results
- [ ] `grep -r "\-d.*debug\|--debug" --include="*.md" doc/src/commands/` returns zero results (except CHANGELOG entries documenting the REMOVAL)
- [ ] `make all-tarballs` produces 4 tarballs in dist/
- [ ] `gh release view v0.40.0` shows the release

### Must Have
- Semver versioning consistent across ALL files
- CHANGELOG [Unreleased] renamed to [0.40.0] - 2026-04-17
- Duplicate `### Changed` sections in CHANGELOG merged
- All `-d/--debug` references in user docs replaced with `-v/-vv`
- Pokémon feature documented as opt-in (not default) in all docs
- 4 tarballs generated and attached to GitHub release
- Git tag v0.40.0 created and pushed

### Must NOT Have (Guardrails)
- No new features or production code changes (except version strings)
- No refactoring of production code
- No new GitHub Actions or CI changes
- No new documentation pages created
- No test code changes (version strings in test assertions are acceptable to update)
- No development docs rewrite (only version/status updates in IMPLEMENTATION.md and roadmap.md)
- No changes to the architecture of the project

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** - ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES (cargo test)
- **Automated tests**: Tests-after (run existing tests to verify version bump doesn't break anything)
- **Framework**: cargo test

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Build/Version**: Use Bash (cargo build, grep, diff)
- **Documentation**: Use Bash (grep, mdbook build)
- **Tarballs/Release**: Use Bash (make, gh, tar inspection)

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation — version bump + CHANGELOG fix):
├── Task 1: Bump version to 0.40.0 in all source/config files
├── Task 2: Fix CHANGELOG.md structure + semver date header
└── Task 3: Clean up IMPLEMENTATION.md version references

Wave 2 (Documentation — parallel docs updates):
├── Task 4: Update command docs (remove -d/--debug, add -v/-vv)
├── Task 5: Update tools/features docs (Pokémon opt-in, new tools)
├── Task 6: Update development docs (roadmap, architecture versions)
└── Task 7: Update miscellaneous docs (troubleshooting, pipelines, soul, README)

Wave 3 (Build + Release):
├── Task 8: Build release binary + generate tarballs
└── Task 9: Create git tag + GitHub release

Wave FINAL (Verification — after ALL tasks):
├── Task F1: Version consistency audit
├── Task F2: Documentation accuracy review
├── Task F3: Release verification
└── Task F4: Scope fidelity check
-> Present results -> Get explicit user okay

Critical Path: T1 → T2 → T8 → T9 → F1-F4 → user okay
Parallel Speedup: ~50% faster than sequential
Max Concurrent: 4 (Wave 2)
```

### Dependency Matrix

| Task | Depends On | Blocks | Wave |
|------|-----------|--------|------|
| 1 | - | 4, 5, 6, 7, 8 | 1 |
| 2 | - | 8 | 1 |
| 3 | - | 6, 8 | 1 |
| 4 | 1 | 8 | 2 |
| 5 | 1 | 8 | 2 |
| 6 | 1, 3 | 8 | 2 |
| 7 | 1 | 8 | 2 |
| 8 | 1, 2, 3, 4, 5, 6, 7 | 9 | 3 |
| 9 | 8 | F1, F2, F3 | 3 |

### Agent Dispatch Summary

- **Wave 1**: 3 tasks — T1 → `quick`, T2 → `quick`, T3 → `quick`
- **Wave 2**: 4 tasks — T4 → `quick`, T5 → `quick`, T6 → `quick`, T7 → `quick`
- **Wave 3**: 2 tasks — T8 → `unspecified-high`, T9 → `unspecified-high`
- **FINAL**: 4 tasks — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

---
## TODOs

- [x] 1. Bump Version to 0.40.0 in All Source/Config Files

  **What to do**:
  - Update `Cargo.toml`: `version = "0.39.5"` → `version = "0.40.0"`
  - Update `man/ask-ai.1`: `.TH ASK-AI 1 "2026-04-17" "ask-ai 0.40.0" "User Commands"`
  - Update `src/chat/view/terminal.rs` line 213: `"0.39.5"` → `"0.40.0"`
  - Update `src/chat/view/mod.rs` line 648: `"0.39.5"` → `"0.40.0"`
  - Update `ARCHITECTURE.md`: `Version: 0.39.5` → `Version: 0.40.0`
  - Update `ask-ai-architecture.html`: both `v0.39.5` → `v0.40.0`
  - Run `cargo build --release` to regenerate `Cargo.lock` with new version
  - Commit Cargo.lock alongside Cargo.toml

  **Must NOT do**:
  - Do NOT change any non-version production code
  - Do NOT update doc/src/ files (Task 4-7)
  - Do NOT update CHANGELOG.md (Task 2)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3)
  - **Blocks**: Tasks 4, 5, 6, 7, 8
  - **Blocked By**: None

  **References**:
  - `Cargo.toml:3` — Current version `version = "0.39.5"`
  - `man/ask-ai.1:1` — Man page `.TH` line
  - `src/chat/view/terminal.rs:213` — Hardcoded version in banner
  - `src/chat/view/mod.rs:648` — Hardcoded version in WelcomeInfo
  - `ARCHITECTURE.md:11` — Version line
  - `ask-ai-architecture.html:43,529` — Two HTML lines with v0.39.5

  **Acceptance Criteria**:
  - [ ] `grep -r "0.39.5" --include="*.toml" --include="*.1" --include="*.rs" --include="*.html" .` returns 0 results
  - [ ] `grep "0.40.0" Cargo.toml` returns match
  - [ ] `cargo check` succeeds

  **QA Scenarios:**
  ```
  Scenario: Version consistency across source files
    Tool: Bash
    Steps:
      1. grep -c "0.39.5" Cargo.toml man/ask-ai.1 src/chat/view/terminal.rs src/chat/view/mod.rs ARCHITECTURE.md ask-ai-architecture.html
      2. grep -c "0.40.0" Cargo.toml man/ask-ai.1 src/chat/view/terminal.rs src/chat/view/mod.rs ARCHITECTURE.md ask-ai-architecture.html
      3. cargo check
    Expected: 0.39.5 count = 0, 0.40.0 count >= 6, cargo check succeeds
    Evidence: .sisyphus/evidence/task-1-version-consistency.txt

  Scenario: Build release binary
    Tool: Bash
    Steps:
      1. cargo build --release 2>&1 | tail -5
    Expected: Build succeeds
    Evidence: .sisyphus/evidence/task-1-build.txt
  ```

  **Commit**: YES
  - Message: `chore: bump version to 0.40.0 across all files`
  - Files: Cargo.toml, Cargo.lock, man/ask-ai.1, src/chat/view/terminal.rs, src/chat/view/mod.rs, ARCHITECTURE.md, ask-ai-architecture.html
  - Pre-commit: `cargo check`

- [x] 2. Fix CHANGELOG.md Structure and Add Semver v0.40.0 Header

  **What to do**:
  - Merge duplicate `### Changed` sections (lines 33 and 68) into one
  - Replace `## [Unreleased]` with `## [0.40.0] - 2026-04-17`
  - Merge second `### Added` (line 107) into first `### Added` (line 7)
  - Add fresh `## [Unreleased]` with empty subsections at top (Keep a Changelog format)
  - Section order: `### Added` → `### Changed` → `### Fixed`

  **Must NOT do**:
  - Do NOT rewrite or rephrase any changelog entries
  - Do NOT add/delete entries
  - Do NOT touch sections below [0.39.5]

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 3)
  - **Blocks**: Task 8
  - **Blocked By**: None

  **References**:
  - `doc/src/CHANGELOG.md:33` — First `### Changed`
  - `doc/src/CHANGELOG.md:68` — Duplicate `### Changed`
  - `doc/src/CHANGELOG.md:107` — Second `### Added`
  - `doc/src/CHANGELOG.md:247` — Previous version `[0.39.5] - 2026-03-30` format ref

  **Acceptance Criteria**:
  - [ ] Exactly 1 `## [Unreleased]` at top
  - [ ] Exactly 1 `## [0.40.0] - 2026-04-17`
  - [ ] Between [0.40.0] and [0.39.5]: exactly 1 `### Added`, 1 `### Changed`, 1 `### Fixed`

  **QA Scenarios:**
  ```
  Scenario: CHANGELOG structural correctness
    Tool: Bash
    Steps:
      1. awk '/## \\[0.40.0\\]/,/## \\[0.39/' doc/src/CHANGELOG.md | grep -c '### Changed'
      2. awk '/## \\[0.40.0\\]/,/## \\[0.39/' doc/src/CHANGELOG.md | grep -c '### Added'
      3. grep -c '## \\[Unreleased\\]' doc/src/CHANGELOG.md
      4. grep '## \\[0.40.0\\] - 2026-04-17' doc/src/CHANGELOG.md
    Expected: 1 Changed, 1 Added, 1 Unreleased header, dated 0.40.0 header
    Evidence: .sisyphus/evidence/task-2-changelog-structure.txt
  ```

  **Commit**: YES
  - Message: `docs: fix CHANGELOG duplicate section and add semver v0.40.0 header`
  - Files: doc/src/CHANGELOG.md

- [x] 3. Clean Up IMPLEMENTATION.md Version References

  **What to do**:
  - Update version header to v0.40.0
  - Replace phantom v0.39.6/v0.39.7 references with v0.40.0
  - Update completed items' version markers to v0.40.0
  - Update roadmap.md v0.39.x references for completed features now in v0.40.0

  **Must NOT do**:
  - Do NOT change statuses for still-planned items
  - Do NOT add new roadmap items

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2)
  - **Blocks**: Task 6, Task 8
  - **Blocked By**: None

  **References**:
  - `IMPLEMENTATION.md:29` — Version header
  - `IMPLEMENTATION.md:437` — `v0.39.6` reference
  - `IMPLEMENTATION.md:690` — `v0.39.7` reference
  - `doc/src/development/roadmap.md:538` — v0.39.0 reference

  **Acceptance Criteria**:
  - [ ] `grep 'v0.39.[67]' IMPLEMENTATION.md` returns 0 results
  - [ ] Version header references v0.40.0

  **QA Scenarios:**
  ```
  Scenario: No phantom version references
    Tool: Bash
    Steps:
      1. grep -c 'v0.39.6\|v0.39.7' IMPLEMENTATION.md
      2. grep -c 'v0.40.0' IMPLEMENTATION.md
    Expected: 0 phantom versions, v0.40.0 present
    Evidence: .sisyphus/evidence/task-3-implementation-versions.txt
  ```

  **Commit**: YES
  - Message: `docs: clean up IMPLEMENTATION.md version references for v0.40.0`
  - Files: IMPLEMENTATION.md, doc/src/development/roadmap.md

- [x] 4. Update Command Docs (Remove -d/--debug, Add -v/-vv)

  **What to do**:
  - `doc/src/commands/README.md:52` — Replace `-d, --debug | Debug mode with logging` with `-v, -vv | Verbosity: verbose / trace level`
  - `doc/src/commands/query.md:50` — Replace `--debug | -d | Enable debug logging` row with `-v` verbose + `-vv` trace rows
  - `doc/src/commands/ocr.md:33` — Replace `--debug | -d | Enable debug mode` with `-v` / `-vv`
  - `doc/src/commands/translate.md:30` — Replace `--debug | -d | Enable debug mode` with `-v` / `-vv`
  - `doc/src/commands/vision.md:30` — Replace `--debug | -d | Enable debug mode` with `-v` / `-vv`
  - `doc/src/commands/summarize.md:36` — Replace `--debug | -d | Enable debug mode` with `-v` / `-vv`
  - In all command docs: update any example commands using `-d` or `--debug` to use `-v`
  - Add a note about `-q` (quiet) flag if not already documented
  - Update the Default Model column in README.md command overview if outdated (currently shows `lfm`, `moondream` which may be stale)

  **Must NOT do**:
  - Do NOT update CHANGELOG.md (that is Task 2)
  - Do NOT add new command doc pages

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5, 6, 7)
  - **Blocks**: Task 8
  - **Blocked By**: Task 1 (version must be bumped first for consistency)

  **References**:
  - `doc/src/commands/README.md:52` — Global options table with `-d, --debug`
  - `doc/src/commands/query.md:50` — Query options with `--debug`
  - `doc/src/commands/ocr.md:33` — OCR options with `--debug`
  - `doc/src/commands/translate.md:30` — Translate options
  - `doc/src/commands/vision.md:30` — Vision options
  - `doc/src/commands/summarize.md:36` — Summarize options

  **Acceptance Criteria**:
  - [ ] `grep -r '\-d.*debug\|--debug' doc/src/commands/` returns 0 results
  - [ ] All command docs have `-v` / `-vv` documented
  - [ ] No example commands use `-d` or `--debug`

  **QA Scenarios:**
  ```
  Scenario: No stale -d/--debug in command docs
    Tool: Bash
    Steps:
      1. grep -r '\-d.*debug\|--debug' doc/src/commands/ || echo 'CLEAN'
      2. grep -c '\-v\|\-vv' doc/src/commands/README.md doc/src/commands/query.md
    Expected: No -d/--debug found, -v/-vv present in docs
    Evidence: .sisyphus/evidence/task-4-command-flags.txt
  ```

  **Commit**: YES (grouped with Wave 2)
  - Message: `docs: update user documentation for v0.40.0 features and flags`
  - Files: all doc/src/commands/ files

- [x] 5. Update Tools/Features Docs (Pokémon Opt-in, New Tools)

  **What to do**:
  - `doc/src/tools.md` — Verify Pokémon tools documented as opt-in (check if already correct: line 128 says opt-in)
  - `doc/src/development/contributing.md:89` — Change `pokemon-tools - Pokémon data tools (default)` to `(opt-in)`, matching AGENTS.md
  - `doc/src/development/skills-system-design.md:636` — Update default features list to match current Cargo.toml (remove pokemon-tools from defaults)
  - Add/update documentation for new tools/features introduced in v0.40.0:
    - `note_edit` and `note_delete` tools (already in tools.md?
    - Enhanced todo tools (priority, tags) — verify documented
    - `/skill <name>` subcommand — verify documented in chat commands doc
    - `/forget --yes` confirmation — verify documented
  - Update feature flags table in README.md if Pokémon entry says "Yes" for Default

  **Must NOT do**:
  - Do NOT create new doc pages
  - Do NOT rewrite CHANGELOG.md

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 6, 7)
  - **Blocks**: Task 8
  - **Blocked By**: Task 1

  **References**:
  - `doc/src/tools.md:128` — Pokémon opt-in note (check correctness)
  - `doc/src/development/contributing.md:89` — Pokémon listed as default (wrong)
  - `doc/src/development/skills-system-design.md:636` — Cargo.toml defaults list (outdated)
  - `README.md` (project root) — Feature flags table
  - `Cargo.toml:7` — Current default features (ground truth)

  **Acceptance Criteria**:
  - [ ] `grep -r 'pokemon.*default' doc/src/ | grep -v 'not.*default\|opt-in\|disabled' | grep -v CHANGELOG` returns 0 results
  - [ ] New tools (note_edit, note_delete, todo priority/tags) documented
  - [ ] README.md feature table matches Cargo.toml defaults

  **QA Scenarios:**
  ```
  Scenario: Pokémon no longer listed as default
    Tool: Bash
    Steps:
      1. grep -r 'pokemon.*default\|default.*pokemon' doc/src/ | grep -v 'not.*default\|opt-in\|disabled\|CHANGELOG'
    Expected: No results (Pokémon correctly documented as opt-in)
    Evidence: .sisyphus/evidence/task-5-pokemon-optin.txt
  ```

  **Commit**: YES (grouped with Wave 2)
  - Message: see Task 4 (grouped commit)
  - Files: doc/src/tools.md, doc/src/development/contributing.md, doc/src/development/skills-system-design.md, README.md

- [x] 6. Update Development Docs (Roadmap, Architecture Versions)

  **What to do**:
  - `doc/src/development/architecture.md` — Check for stale version/model references and update if needed
  - `doc/src/development/roadmap.md` — Verify completed items reference v0.40.0 where appropriate (partially done in Task 3)
  - `doc/src/development/run-command-redesign.md:316` — Replace `--debug` reference with `-v`/`-vv`
  - `doc/src/development/prompt-refactor.md:446-456` — Replace `--debug` references with `-v`/`-vv`

  **Must NOT do**:
  - Do NOT rewrite architecture documentation
  - Do NOT add new development doc pages

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 5, 7)
  - **Blocks**: Task 8
  - **Blocked By**: Task 1, Task 3

  **References**:
  - `doc/src/development/architecture.md` — Architecture version/state
  - `doc/src/development/roadmap.md` — Feature status references
  - `doc/src/development/run-command-redesign.md:316` — `--debug` reference
  - `doc/src/development/prompt-refactor.md:446-456` — `--debug` references

  **Acceptance Criteria**:
  - [ ] No `--debug` references in development docs (except CHANGELOG removal entries)
  - [ ] Architecture doc version is current

  **QA Scenarios:**
  ```
  Scenario: Development doc flag references clean
    Tool: Bash
    Steps:
      1. grep -r '\-\-debug' doc/src/development/ | grep -v CHANGELOG
    Expected: No results
    Evidence: .sisyphus/evidence/task-6-dev-docs.txt
  ```

  **Commit**: YES (grouped with Wave 2)
  - Message: see Task 4 (grouped commit)
  - Files: doc/src/development/ files

- [x] 7. Update Miscellaneous Docs (Troubleshooting, Pipelines, Soul, README)

  **What to do**:
  - `doc/src/troubleshooting.md:303` — Replace `ask-ai -d "Test query" 2> debug.log` with `ask-ai -v "Test query" 2> verbose.log`
  - `doc/src/pipelines.md:252` — Replace `-d` flags in pipeline examples with `-v`
  - `doc/src/soul.md:433` — Replace `ask query --debug "test query"` with `ask-ai -v "test query"`
  - `README.md` (project root) — Verify feature flags table, update Default column for Pokémon (should be No), verify model references current
  - `doc/src/commands/chat.md` — Add/update documentation for:
    - `/skill <name>` subcommand (replaces `/<skill-name>` wildcard)
    - `/forget --yes` confirmation
    - `/todo` enhanced commands (get, delete, edit, priority, tags)
    - Session context resume feature

  **Must NOT do**:
  - Do NOT create new doc pages
  - Do NOT touch CHANGELOG.md

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 5, 6)
  - **Blocks**: Task 8
  - **Blocked By**: Task 1

  **References**:
  - `doc/src/troubleshooting.md:303` — `-d` flag in debug example
  - `doc/src/pipelines.md:252` — `-d` flags in pipeline examples
  - `doc/src/soul.md:433` — `--debug` flag reference
  - `README.md` (root) — Feature flags table + model references
  - `doc/src/commands/chat.md` — Chat command docs for /skill, /forget, /todo, session resume

  **Acceptance Criteria**:
  - [ ] `grep -r '\-d.*debug\|--debug' doc/src/ | grep -v CHANGELOG | grep -v 'Removed\|removed\|replace'` returns 0 results
  - [ ] Chat docs include /skill, /forget --yes, /todo enhancements, session resume
  - [ ] README.md feature table is accurate

  **QA Scenarios:**
  ```
  Scenario: No stale debug flags in non-changelog docs
    Tool: Bash
    Steps:
      1. grep -r '\-d.*debug\|--debug' doc/src/ | grep -v CHANGELOG | grep -v 'Removed\|removed\|replace'
    Expected: No results
    Evidence: .sisyphus/evidence/task-7-misc-docs.txt
  ```

  **Commit**: YES (grouped with Wave 2)
  - Message: see Task 4 (grouped commit)
  - Files: doc/src/troubleshooting.md, doc/src/pipelines.md, doc/src/soul.md, README.md, doc/src/commands/chat.md

- [x] 8. Build Release Binary + Generate Tarballs

  **What to do**:
  - Run `make all-tarballs` to build and generate all 4 distribution tarballs:
    - `dist/ask-ai-0.40.0-linux-x86_64.tar.gz`
    - `dist/ask-ai-0.40.0-linux-x86_64-all-tools.tar.gz`
    - `dist/ask-ai-0.40.0-termux-aarch64-linux-android.tar.gz`
    - `dist/ask-ai-0.40.0-termux-aarch64-linux-android-all-tools.tar.gz`
  - If Termux cross-compilation toolchain is not available, build the 2 Linux tarballs only and note the limitation
  - Verify each tarball contains: binary, man page (ask-ai.1), README.md, LICENSE.txt, install.sh, uninstall.sh
  - Verify binary version: `./target/release/ask-ai --version` (if --version flag exists) or check the binary output

  **Must NOT do**:
  - Do NOT push tarballs to git (they are release assets only)
  - Do NOT modify any source files

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Build process may need troubleshooting, complex multi-target build
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (sequential after Wave 2)
  - **Blocks**: Task 9
  - **Blocked By**: Tasks 1, 2, 3, 4, 5, 6, 7

  **References**:
  - `Makefile:286` — `all-tarballs` target
  - `Makefile:30` — VERSION extraction from Cargo.toml
  - `Makefile:211-223` — Linux tarball generation
  - `Makefile:240-254` — Termux tarball generation

  **Acceptance Criteria**:
  - [ ] `ls dist/ask-ai-0.40.0-*.tar.gz | wc -l` returns 4 (or 2 if Termux unavailable)
  - [ ] Each tarball contains ask-ai binary + install scripts
  - [ ] Binary reports version 0.40.0

  **QA Scenarios:**
  ```
  Scenario: Tarballs generated correctly
    Tool: Bash
    Steps:
      1. ls -la dist/ask-ai-0.40.0-*.tar.gz
      2. tar -tzf dist/ask-ai-0.40.0-linux-x86_64.tar.gz | head -10
      3. file target/release/ask-ai
    Expected: 4 tarballs exist, Linux tarball has binary+manpage+scripts, binary is ELF
    Failure Indicators: Missing tarballs, empty tarballs, wrong architecture
    Evidence: .sisyphus/evidence/task-8-tarballs.txt

  Scenario: Termux cross-compilation available
    Tool: Bash
    Steps:
      1. rustup target list | grep aarch64-linux-android
    Expected: If installed, all 4 tarballs. If not, 2 Linux tarballs only (document limitation).
    Evidence: .sisyphus/evidence/task-8-termux-toolchain.txt
  ```

  **Commit**: NO (tarballs are not committed)

- [x] 9. Create Git Tag + GitHub Release

  **What to do**:
  - Create annotated git tag: `git tag -a v0.40.0 -m "Release v0.40.0"`
  - Push tag to remote: `git push origin v0.40.0`
  - Create GitHub release with gh CLI:
    ```
    gh release create v0.40.0 \
      --title "v0.40.0 - Verbosity System & Security Hardening" \
      --notes "$(cat <<'RELEASE_NOTES'
    ## New Features

    - **Logging Infrastructure with `log` Crate** — Industry-standard logging replaces custom debug system (Issues #60, #61, #87, #88)
      - 4-level verbosity: `-q` (quiet), normal (default), `-v` (verbose), `-vv` (trace)
      - `RUST_LOG` env variable for fine-grained control
    - **Pre-tool Thinking Visible in Chat** — LLM thinking process shown before tool calls
    - **Chat Output Fixed at 80 Columns** — Consistent markdown rendering across terminals
    - **Memory Staleness Warnings** — Facts prompt shows age/staleness labels (Issue #70)
    - **Truncation Warnings in Tool Outputs** — Standardized `[TRUNCATED:...]` format (Issue #71)
    - **Enhanced Todo Tools** — CRUD gaps, priority levels, tags (Issue #66)
    - **Session Context Resume** — Recent conversation shown on session load
    - **Braille Art Welcome Banner** — Extended mind neuron art + random spinner animations
    - **Notes LLM Tools** — `note_edit(id, title?, content?)` and `note_delete(id)` (Issue #63)
    - **`/skill <name>` Subcommand** — Replaces wildcard skill activation (Issue #86)
    - **`/forget --yes` Confirmation** — Prevents accidental data loss (Issue #85)

    ## Security

    - **Removed LLM-controllable sandbox bypass** — `sandbox` parameter removed from all file tools. Sandbox is always enforced.
    - **Removed `enable_sandbox = false` config** — Landlock sandbox cannot be disabled
    - **Added `/tmp` and `/var/tmp`** as allowed directories for file operations

    ## Changed

    - Default model: qwen3.5:4b (multimodal, 128K context)
    - Code model default: qwen2.5-coder:7b
    - Removed `-d`/`--debug` CLI flag → replaced by `-v`/`-vv` (Issue #61)
    - Removed `debug_default` config → replaced by `verbosity` in `[output]` section
    - Todo tools now built-in (no feature flag required)
    - `Ollama` label renamed to `Server` in welcome banner

    ## Bug Fixes

    - Fixed FTS5 `conversation_id` column error in `delete_conversation()`
    - Fixed FOREIGN KEY constraint failure when saving todos
    - Fixed Unicode panic on string truncation in chat resume (Issue #69)
    - Fixed `search_files` empty `file_pattern` filtering out all results
    - Fixed `summarize`/`vision` ignoring config.toml model settings (Issue #65)
    - Fixed `/model` switch not persisted to database
    - Fixed `/f` shortcut collision between `/forget` and `/search`
    - Fixed missing `/todo` shortcuts (`/tg`, `/te`, `/td`, `/tcd`, `/tca`)
    - Fixed empty string normalization for `Option<String>` tool parameters

    ## Code Quality

    - Reduced `parse_command` complexity (Issue #35) — ~462 lines removed, 77 unit tests added
    - Reduced `registry.rs` cognitive complexity (Issue #31)
    - Reduced `context_builder.rs` cognitive complexity (Issue #30)
    - Reduced `query.rs` cognitive complexity (Issue #29)

    ## Installation

    ### Linux (x86_64)
    ```bash
    tar -xzf ask-ai-0.40.0-linux-x86_64.tar.gz
    cd ask-ai-0.40.0-linux-x86_64
    ./install.sh
    ```

    ### Linux (x86_64) - All Tools
    ```bash
    tar -xzf ask-ai-0.40.0-linux-x86_64-all-tools.tar.gz
    cd ask-ai-0.40.0-linux-x86_64-all-tools
    ./install.sh
    ```

    ### Termux (Android)
    ```bash
    tar -xzf ask-ai-0.40.0-termux-aarch64-linux-android.tar.gz
    cd ask-ai-0.40.0-termux-aarch64-linux-android
    ./install.sh
    ```

    ### Termux (Android) - All Tools
    ```bash
    tar -xzf ask-ai-0.40.0-termux-aarch64-linux-android-all-tools.tar.gz
    cd ask-ai-0.40.0-termux-aarch64-linux-android-all-tools
    ./install.sh
    ```

    **Full Changelog**: https://github.com/luksamuk/ask-ai-rs/compare/v0.39.5...v0.40.0
    RELEASE_NOTES
    )" \
      dist/ask-ai-0.40.0-linux-x86_64.tar.gz \
      dist/ask-ai-0.40.0-linux-x86_64-all-tools.tar.gz \
      dist/ask-ai-0.40.0-termux-aarch64-linux-android.tar.gz \
      dist/ask-ai-0.40.0-termux-aarch64-linux-android-all-tools.tar.gz
    ```
  - If only 2 Linux tarballs available, attach only those and note Termux limitation in release body

  **Must NOT do**:
  - Do NOT push to main/master branch (only tag push)
  - Do NOT use `--force` on any push
  - Do NOT create release as draft (user wants stable release)
  - Do NOT create prerelease

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Release process is critical and irreversible, needs careful execution
  - **Skills**: [`git-master`]
    - `git-master`: Required for proper tag creation and push workflow

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (sequential after Task 8)
  - **Blocks**: F1, F2, F3
  - **Blocked By**: Task 8

  **References**:
  - Previous release format: `gh release view v0.39.5`
  - Makefile release pattern: `Makefile:200-299`
  - AGENTS.md Release Process section

  **Acceptance Criteria**:
  - [ ] `gh release view v0.40.0` shows release
  - [ ] Release has 4 (or 2) tarball assets
  - [ ] Release is NOT draft, NOT prerelease
  - [ ] `git tag -l v0.40.0` shows tag

  **QA Scenarios:**
  ```
  Scenario: GitHub release created correctly
    Tool: Bash (gh)
    Steps:
      1. gh release view v0.40.0 --json tagName,isDraft,isPrerelease,assets
      2. Check tagName = v0.40.0, isDraft = false, isPrerelease = false
      3. Count assets >= 2
    Expected: Release found, stable, with tarballs
    Failure Indicators: Release not found, draft, or missing assets
    Evidence: .sisyphus/evidence/task-9-release.txt

  Scenario: Tag exists on remote
    Tool: Bash (git)
    Steps:
      1. git ls-remote --tags origin v0.40.0
    Expected: Tag found on remote
    Evidence: .sisyphus/evidence/task-9-tag.txt
  ```

  **Commit**: NO (this is a tag + release operation, not a file commit)

- [x] F1. **Version Consistency Audit** — `oracle`
  Run `grep -r "0.39.5" --include="*.toml" --include="*.1" --include="*.rs" --include="*.html" --include="*.md" .` — must return ZERO results. Verify `0.40.0` appears in all correct locations. Check `cargo build --release` succeeds.
  Output: `Version References [N stale / N updated] | Build [PASS/FAIL] | VERDICT`

- [x] F2. **Documentation Accuracy Review** — `unspecified-high`
  Search all doc files for stale `-d/--debug` references (should only exist in CHANGELOG documenting removal). Verify `-v/-vv` is documented. Verify Pokémon documented as opt-in. Run `cd doc && mdbook build` to verify no broken links.
  Output: `Stale References [N found] | mdBook [PASS/FAIL] | VERDICT`

- [x] F3. **Release Verification** — `unspecified-high`
  Run `gh release view v0.40.0` and verify: tag exists, 4 tarball assets attached, release notes contain all sections. Inspect each tarball: `tar -tzf dist/ask-ai-0.40.0-linux-x86_64.tar.gz | head -10` and verify binary + man page + scripts present.
  Output: `Tag [exists/missing] | Assets [4/4] | Tarball Contents [valid/invalid] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  Verify ONLY version-related changes were made: `git diff v0.39.5..v0.40.0 --stat` should show only .toml, .1, .rs (version strings), .md (docs), and .html (architecture) files. No unexpected production code changes. Check that CHANGELOG [Unreleased] is now [0.40.0].
  Output: `Files Changed [N] | Production Code [CLEAN/N issues] | Scope [COMPLIANT/VIOLATED] | VERDICT`

---

## Commit Strategy

- **Wave 1** (single commit): `chore: bump version to 0.40.0 across all files` — Cargo.toml, Cargo.lock, man/ask-ai.1, src/chat/view/terminal.rs, src/chat/view/mod.rs, ARCHITECTURE.md, ask-ai-architecture.html
  - Pre-commit: `cargo check`
- **Wave 1** (separate commit): `docs: fix CHANGELOG duplicate section and add semver v0.40.0 header` — doc/src/CHANGELOG.md
  - Pre-commit: none
- **Wave 1** (separate commit): `docs: clean up IMPLEMENTATION.md version references for v0.40.0` — IMPLEMENTATION.md
  - Pre-commit: none
- **Wave 2** (single commit): `docs: update user documentation for v0.40.0 features and flags` — all doc/src/ files updated in wave 2
  - Pre-commit: `cd doc && mdbook build`
- **Wave 3** (commit + tag): `release: v0.40.0` — version tag commit
  - Tag: `git tag -a v0.40.0 -m "Release v0.40.0"`

---

## Success Criteria

### Verification Commands
```bash
# Version consistency
grep -r "0.39.5" --include="*.toml" --include="*.1" --include="*.rs" --include="*.html" --include="*.md" . | wc -l  # Expected: 0

# Build succeeds
cargo build --release  # Expected: success

# CHANGELOG has dated semver header
grep "## \[0.40.0\]" doc/src/CHANGELOG.md  # Expected: match

# No duplicate ### Changed in [0.40.0] section
awk '/## \[0.40.0\]/,/## \[0.39/' doc/src/CHANGELOG.md | grep -c "### Changed"  # Expected: 1

# No stale -d/--debug in command docs (except CHANGELOG removal docs)
grep -r "\-d.*debug\|--debug" doc/src/commands/  # Expected: no matches

# Pokémon documented as opt-in
grep -r "pokemon.*default\|default.*pokemon" doc/src/ | grep -v "not.*default\|opt-in\|disabled"  # Expected: no matches

# Tarballs exist
ls dist/ask-ai-0.40.0-*.tar.gz | wc -l  # Expected: 4

# Release exists on GitHub
gh release view v0.40.0  # Expected: release found
```

### Final Checklist
- [ ] All "Must Have" present
- [ ] All "Must NOT Have" absent
- [ ] All tests pass (`cargo test`)
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] mdBook builds without errors (`cd doc && mdbook build`)