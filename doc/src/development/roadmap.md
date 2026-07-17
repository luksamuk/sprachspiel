# Roadmap

This document outlines milestones and the current development direction of Sprachspiel. For detailed per-issue tracking, see [Feature Status](./feature-status.md). For completed features, see [Completed Features](./completed-features.md).

## Milestones

| Milestone | Codename | Description | Status |
|-----------|----------|-------------|--------|
| **[M1]** | Core Evolution | 7 waves of core work before TUI and Sprach 2.0 | 🟡 In Progress (W1–W2, W6 complete; W3–W5, W7 remaining) |
| **[M2]** | UX & Pre-Launch | TUI design + implementation, benchmarks, learned patterns | 📋 Planned |
| **[M3]** | Sprach 2.0 | CAS research, cognitive extensions, plugin system | 📋 Planned |
| **[M4]** | Future | Deferred features and research | 📋 Planned |

## M1 Implementation Waves

| Wave | Codename | Theme | Status |
|------|----------|-------|--------|
| **W1** | Quick Wins | Small independent items (#126, #105, #36) | ✅ Complete |
| **W2** | Provider Chain | Multi-provider migration (#116→#123, #72, #201) | ✅ Complete |
| **W3** | Feedback Completion | Close decay activation, feedback expansion (#90–#97) | 📋 Not Started |
| **W4** | Embedding Geometry & Flexibility | Diagnostics, geometry-aware config, T3-Phase0 (#133–#138, #106, #107, #151, #136, #157, #182) | 🟡 In Progress |
| **W5** | M1 Backlog | Batch processing, context, secrets, personalities, file write tools (#13, #14, #49, #50, #52, #74–#76, #132, #204, #205) | 📋 Not Started |
| **W6** | Responsive Chat Rebuild | Ratatui-based responsive rendering (#145–#148) | ✅ Complete |
| **W7** | Thinking Trace Pipeline | Preserve thinking, T3 Struct pipeline, thinking-aware retrieval (#152, #153, #137) | 📋 Not Started |

**Wave dependencies:**
- W1 has no blockers — ✅ Complete
- W2 has internal dependency chain: `#116 → #118 → #119 → #120 → #121 → #122 → #123` — ✅ Complete
- W3: `#90` closable now; `#91`-`#97` need research
- W4: independent of W2. T3-Phase0 (#151) is standalone
- W5: independent — can be picked up between waves
- W6: ✅ Complete (v0.44.0, PR #155)
- W7: starts after W6-PR3 (#147) and W4.5 (#151) complete

## Current State Summary

Sprachspiel is a feature-rich CLI with:
- 5 subcommands (query, chat, translate, ocr, summarize)
- 50 tools across 14 categories (feature-flagged)
- Provider-agnostic LLM layer (`LlmProvider` trait, `OpenAICompatibleProvider`)
- Ratatui-based responsive chat at any terminal width
- Persistent conversation history with SQLite + sqlite-vec
- Hybrid retrieval (BM25 + Semantic + RRF) with feedback-aware weighting
- Factual memory system with Ebbinghaus decay and 6-layer dedup
- Context continuity with graceful interruption
- 3-layer compaction strategy (pre-prune → chunked recursive → fallback truncation)
- Thinking content preservation (T3-Phase0)

See [Completed Features](./completed-features.md) for the full list.

## Upcoming Release

### v0.45.0 (Planned)

**Infrastructure:**
- Config Upgrade Command (#105) — `sprach config upgrade` merges missing default fields into existing config.toml — ✅ Complete
- Retry Threshold with Backoff (#116) — Recoverable server errors with exponential backoff — ✅ Complete
- Tool Trait + Proc Macro (#118) — `#[sprachspiel::tool]` replacing `#[ollama_rs::function]` — ✅ Complete
- Provider-agnostic types (#119–#123) — Full `ollama-rs` removal — ✅ Complete

## Known Issues

No critical bugs currently open. See [GitHub Issues](https://github.com/luksamuk/sprachspiel/issues) for the latest status.

## Sprach 2.0: CAS Research [M3]

**Status:** 🟡 RESEARCH NEEDED
**Full Design:** See [Sprach 2.0 Research](./sprach-2-0-research.md) for open questions, code analysis, and implementation details.

Self-analysis identifying sprachspiel as a Complex Adaptive System (CAS) with emergent properties but limited open-endedness. Proposals aim to increase emergent connectivity and adaptive behavior.

| ID | Proposal | Depends On | Status | Effort |
|----|----------|------------|--------|--------|
| S2.1 | Visualize Connections Tool | None | 🟡 Research | 2-3 days |
| S2.2 | Content Relations Graph (2-layer) | S2.1 | 🟡 Research | 5-8 days |
| S2.3 | Reflection on Triggers + Curation | S2.1, S2.2 | 🟡 Research | 4-7 days |
| S2.4 | Plugin System (WASM) | — | 📋 #15 (existing) | 3-4 weeks |
| S2.5 | SOUL.md Patching + `/apply-patch` | S2.3 | 🟡 Research | 3-5 days |
| S2.6 | Skills Auto-Registration (Meta) | S2.1-S2.5 | 🕐 Wait | TBD |
| S2.meta1 | Meta-cognition Skill (Layer 1) | None | 🟡 Prototype (Issue #99) | 1h |
| S2.meta2 | Behavioral Telemetry (Layer 2) | Feedback system, S2.meta1 | 📋 Planned (Issue #100) | 2-3 days |
| S2.meta3 | Behavioral Reflection + Personality (Layer 3) | S2.3, S2.5, S2.meta2 | 📋 Planned (Issue #101) | 1-2 weeks |

## [M4] Future — Deferred

Features explicitly deferred with no current priority:

| Feature | Reason |
|---------|--------|
| AutoDream full daemon (4-phase) | After Sprach 2.0 |
| Cost Tracking | After Sprach 2.0 |
| Multi-scope Memory (team/private) | Not applicable — harness is not code-focused |
| Context Collapse | Observe, don't implement |
| VCR Testing | When CI is robust |
| Speculation | Indefinite deferral |
| Remote Agent | #15+ |
| ACP Agent Integration | B8 — after TUI decoupling (#16) |
| Team Memory Sync | Team use only |
| Remote Managed Settings | Enterprise |
| Worktree-aware sessions | Niche |
| Thinkback Marketplace | Too premature |
| Session Summary / Away Summary | Discarded — session continuation already works |

## See Also

- [Feature Status](./feature-status.md) — Per-issue tracking
- [Completed Features](./completed-features.md) — Implemented features
- [Implementation Status](./implementation-status.md) — Current snapshot
- [Implementation Directive](./implementation-directive.md) — Strategic direction
- [Research Icebox](./research-icebox.md) — Deferred research topics