# Implementation Status

> **Source of truth:** The detailed implementation tracker is [`IMPLEMENTATION.md`](../../../IMPLEMENTATION.md) in the repository root. This page provides a navigable snapshot for the mdBook. For full details, changelog entries, and per-issue tracking, consult the root file.

## Current Version

**v0.44.0** — Responsive Chat Rebuild (W6, PR #155)

## Milestones

| Milestone | Codename | Description | Status |
|-----------|----------|-------------|--------|
| **[M1]** | Core Evolution | 7 waves of core work before TUI and Sprach 2.0 | 🟡 In Progress (W1–W2, W6 complete; W3–W5, W7 remaining) |
| **[M2]** | UX & Pre-Launch | TUI design + implementation, benchmarks, learned patterns | 📋 Planned |
| **[M3]** | Sprach 2.0 | CAS research, cognitive extensions, plugin system | 📋 Planned |
| **[M4]** | Future | Deferred features and research | 📋 Planned |

## M1 Wave Status

| Wave | Codename | Theme | Status |
|------|----------|-------|--------|
| **W1** | Quick Wins | Small independent items (#126, #105, #36) | ✅ Complete |
| **W2** | Provider Chain | Multi-provider migration (#116→#123, #72, #201) | ✅ Complete |
| **W3** | Feedback Completion | Close decay activation, feedback expansion (#90–#97) | 📋 Not Started |
| **W4** | Embedding Geometry & Flexibility | Diagnostics, geometry-aware config, T3-Phase0 (#133–#138, #106, #107, #151, #136, #157, #182) | 🟡 In Progress |
| **W5** | M1 Backlog | Batch processing, context, secrets, personalities, file write tools (#13, #14, #49, #50, #52, #74–#76, #132, #204, #205) | 📋 Not Started |
| **W6** | Responsive Chat Rebuild | Ratatui-based responsive rendering (#145–#148) | ✅ Complete |
| **W7** | Thinking Trace Pipeline | Preserve thinking, T3 Struct pipeline, thinking-aware retrieval (#152, #153, #137) | 📋 Not Started |

## Completed Features (Summary)

- ✅ Core CLI with 5 subcommands (query, chat, translate, ocr, summarize)
- ✅ Interactive chat mode with persistent sessions (Ratatui + Crossterm)
- ✅ Provider-agnostic LLM layer (`LlmProvider` trait, `OpenAICompatibleProvider`)
- ✅ `ollama-rs` dependency fully removed (W2 complete)
- ✅ Tool integration with error recovery (50 tools, `#[sprachspiel::tool]` proc macro)
- ✅ Factual Memory System with Ebbinghaus decay and 6-layer dedup
- ✅ Feedback-Driven Memory (good/bad/correction signals with RRF fusion)
- ✅ Context Continuity with Graceful Interruption
- ✅ 3-Layer Compaction Strategy (pre-prune → chunked recursive → fallback truncation)
- ✅ SQLite storage with sqlite-vec (schema v12, cosine distance)
- ✅ Embedding generation with Matryoshka truncation (768d → 256d)
- ✅ Responsive chat rendering at any terminal width (Ratatui)
- ✅ Thinking content preservation (T3-Phase0, #151)
- ✅ Config Upgrade command (`sprach config upgrade`)
- ✅ Session management (`/save`, `/load`, `/list`, `/forget`)
- ✅ SOUL.md personality system
- ✅ Skills system with `/skill <name>`
- ✅ Specialized agent architecture (OCR, vision, translate, summarize)
- ✅ Man page, mdBook documentation, Termux/Android builds

## Key Architectural Decisions

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-001 | Harness-only, no fine-tuning | Local-first constraint; harness adjustments are O(1) |
| ADR-002 | Ebbinghaus decay consistent with Facts system | `2^(-t/h)` formula shared across all memory systems |
| ADR-003 | Feedback targets `content_items.id` | Separate table preserves auditability |
| ADR-004 | LLM self-feedback weighted at 30% | Self-verification is unreliable (Aha Moment, Self-Verification Dilemma) |
| ADR-005 | Binary Good/Bad feedback (±1.0) | Strict verification paradigm (Drori et al. 2025) |
| ADR-006 | Clamp prevents negative scores | `(1.0 + feedback_boost).clamp(0.1, 3.0)` in RRF |
| ADR-007 | MCP STDIO requires explicit allowlist + sandbox | By-design RCE vulnerability in STDIO transport |

## For Developers

- **Full implementation tracker:** [`IMPLEMENTATION.md`](../../../IMPLEMENTATION.md) (root)
- **Strategic direction:** [Implementation Directive](./implementation-directive.md)
- **Architecture:** [Architecture](./architecture.md)
- **Provider design:** [Provider Architecture](./provider-architecture.md)
- **Feature status:** [Feature Status](./feature-status.md)
- **Completed features:** [Completed Features](./completed-features.md)
- **Roadmap:** [Roadmap](./roadmap.md)