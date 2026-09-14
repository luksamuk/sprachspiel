# Implementation Plan for sprachspiel

**This file is now an index.** The per-issue tracker used to live here (8,200+ lines,
~520 KB) and was consolidated in September 2026.

| What you are looking for | Where it lives now |
|---|---|
| **Issue tracker** — status, priority, milestones, blocking relations, discussion | **Linear** — project ["Sprachspiel"](https://linear.app/luksamuk/project/sprachspiel-05bb6fb1a5fd). Issue bodies were migrated from GitHub and restored in full during the consolidation. |
| **Release notes** | [`doc/src/CHANGELOG.md`](./doc/src/CHANGELOG.md) |
| **Architecture** — how the system works today | [`doc/src/development/`](./doc/src/development/README.md) |
| **Decision records** — *why* something was decided | [`doc/src/adr/`](./doc/src/adr/README.md) (historical; not maintained as current behaviour) |
| **Strategic direction** | [`doc/src/development/implementation-directive.md`](./doc/src/development/implementation-directive.md) |
| **Deferred research topics** | [`doc/src/development/research-icebox.md`](./doc/src/development/research-icebox.md) |

**Why the tracker moved.** Keeping issue status in a file inside the repo meant the file
and the tracker drifted apart — sections were never marked as superseded when work landed
elsewhere, so the same file could claim an issue was `❌ NOT STARTED` in one section and
`✅ COMPLETED (supersedes it)` in another. That produced 26 contradictions, 12 references
to files that no longer exist, and led at least one audit to a wrong conclusion. Status
now has exactly one home.

## Current Version

**v0.45.0** - 2026-08-19 (Embedding Configurability)

## Current Implementation Status

- Core CLI with 5 subcommands: `query`, `chat`, `translate`, `ocr`, `summarize` (+ `vision`, `diagnostics`, `config`, `models`, `completion`)
- Interactive chat with persistent sessions (Ratatui + Crossterm; responsive at any width)
- Provider-agnostic LLM layer (`LlmProvider` trait, `OpenAICompatibleProvider`); `ollama-rs` removed
- Tool integration with error recovery — **57 tools** (the previous "50 tools in 14 categories" was stale)
- SQLite storage with sqlite-vec — **schema v15**, cosine distance
- Hybrid retrieval (BM25 + vector + RRF) with feedback-aware weighting
- Factual memory with Ebbinghaus decay and a 6-layer dedup pipeline
- Context continuity with graceful interruption; 3-layer compaction strategy
- Thinking content preservation (T3-Phase0), embedding geometry diagnostics
- Custom models via `~/.config/sprachspiel/models.toml`; SOUL.md personality; skills system
- Translation (50+ languages), OCR (multiple modes), summarization, vision analysis
- Markdown rendering, pipe support, man page, mdBook documentation, Termux/Android builds

> Feature-by-feature status is **not** duplicated here. See the Linear project.

## Architecture

Living documentation for how the system works today:

| Document | Subject |
|----------|---------|
| [Architecture](./doc/src/development/architecture.md) | System overview and design decisions |
| [Provider Architecture](./doc/src/development/provider-architecture.md) | `LlmProvider` trait, request/response types, SSE transport, retry |
| [Memory Architecture](./doc/src/development/memory-architecture.md) | Memory subsystems overview |
| [Factual Memory System](./doc/src/development/factual-memory-system.md) | Facts, decay, 6-layer dedup |
| [Feedback Architecture](./doc/src/development/feedback-architecture.md) | Feedback signals and RRF weighting |
| [Context Anatomy](./doc/src/development/context-anatomy.md) | What goes into the context window |
| [Context Continuity](./doc/src/development/context-continuity.md) | Pause/resume protocol on compaction |
| [Context Overflow](./doc/src/development/context-overflow.md) | Overflow thresholds, compaction, truncation |
| [Inter-Tool Compaction](./doc/src/development/inter-tool-compaction-design.md) | Compaction between tool calls |
| [Embedding Diagnostics](./doc/src/development/embedding-diagnostics.md) | `sprach diagnostics`, d_eff, model geometry |
| [Embedding Scheduler Redesign](./doc/src/development/embedding-scheduler-redesign.md) | Embedding pipeline scheduling |
| [Feedback & Facts](./doc/src/development/feedback-and-facts.md) | Decay, dedup pipeline, auto fact extraction |
| [Skills System Design](./doc/src/development/skills-system-design.md) | Skills architecture |
| [TODO System & Status Bar](./doc/src/development/todo-and-status-bar.md) | TODO lifecycle, status bar rendering |
| [Agent Tools](./doc/src/development/agent-tools.md) | Subagent spawning, `/skill` |
| [Content Block Streaming](./doc/src/development/content-block-streaming.md) | Streaming content blocks |
| [File Write Tools](./doc/src/development/file-write-tools.md) | Write/edit tool semantics and session state |
| [Belief System Design](./doc/src/development/belief-system-design.md) | Beliefs and contradiction handling |
| [Chat Mode Design](./doc/src/development/chat-mode-design.md) | Chat interaction design |
| [Unified Vision](./doc/src/development/unified-vision.md) | Cross-subsystem synergies |

## Decision Records

[`doc/src/adr/`](./doc/src/adr/README.md) holds decisions as they were made — rationale and
rejected alternatives. **Consult for the *why*, not the *what*.** If the code contradicts a
record, the code is right and the record needs a supersession note.

## Research

| Document | Subject |
|----------|---------|
| [Research Synthesis](./doc/src/development/research/research-appendix.md) | Complete research synthesis |
| [Papers Reference](./doc/src/development/research/papers-reference.md) | **Canonical bibliography** — cite from here, not inline |
| [Research Icebox](./doc/src/development/research-icebox.md) | Deferred refinement topics and decision records |
| [Sprach 2.0 Research](./doc/src/development/sprach-2-0-research.md) | CAS research track |

> **On citations:** the September 2026 audit found 9 wrong author/institution
> attributions, one fabricated citation and one withdrawn paper. `papers-reference.md`
> is the single source for references; verify there before citing a paper in a design
> document (tracked as LUC-143).

## Process

| Document | Purpose |
|----------|---------|
| [Contributing](./doc/src/development/contributing.md) | How to contribute |
| [PR Process](./doc/src/development/PR-PROCESS.md) | Mandatory workflow for changes |
| [AGENTS.md](./AGENTS.md) | Coding conventions and harness rules |

## Project Management

**Issue tracking lives in Linear** — project ["Sprachspiel"](https://linear.app/luksamuk/project/sprachspiel-05bb6fb1a5fd),
milestones M1–M4. GitHub (`luksamuk/sprachspiel`) keeps PRs, reviews and CI only; the old
GitHub issues are closed history with a migration comment.

- **Branch naming:** `{type}/LUC-N-{slug}` when the PR resolves that issue (the Linear
  GitHub integration auto-links by the magic word `Fixes LUC-N`). For work that merely
  *relates* to an issue, use `{type}/{slug}` without a number.
- **Board columns:** Backlog → Todo → In Progress → In Review → Done (Linear workflow states).

## Maintenance note

This file is deliberately short. It is an index and a snapshot of the current version —
**not** a tracker. When work completes, update the Linear issue and the CHANGELOG; do not
add a section here. If something belongs in this file that is not a link or a version
marker, it probably belongs in `doc/src/development/` or `doc/src/adr/` instead.

*Last consolidated: 2026-09-13 (LUC-140 follow-up, branch `refactor/backlog-consolidation`).*
