# Roadmap

Strategic direction for Sprachspiel: the milestones, what each one means, and the
reasoning behind the sequencing. **For per-issue status, see [Linear](https://linear.app/luksamuk/project/sprachspiel-05bb6fb1a5fd)** —
this document deliberately carries no per-issue tracking, because two sources of truth
for status is exactly what produced the contradictions this was cleaned up from.

## Milestones

| Milestone | Codename | What it covers | Why it is sequenced here |
|-----------|----------|----------------|--------------------------|
| **M1** | Core Evolution | Everything before the TUI and Sprach 2.0 — provider chain, embedding geometry, feedback, thinking traces | The substrate. TUI and Sprach 2.0 both assume these are settled |
| **M2** | UX & Pre-Launch | TUI design + implementation, benchmarks, learned patterns, diff rendering | Needs M1's stable internals; also the last step before public release |
| **M3** | Sprach 2.0 | CAS research, cognitive extensions, plugin system | Builds on the memory/retrieval substrate from M1; exploratory by nature |
| **M4** | Future | Deferred features and research | No current priority — see the deferred table below |

## M1 Implementation Waves

M1 is large enough to need internal order. Waves are sequential by default; W5 is
independent and can be picked up between other work.

| Wave | Codename | Theme | Dependency |
|------|----------|-------|------------|
| **W1** | Quick Wins | Small independent items | none |
| **W2** | Provider Chain | Multi-provider migration | internal chain: retry → tool trait → agnostic types → provider → consumer migration → removal |
| **W3** | Feedback Completion | Decay activation, feedback expansion | W3 items need research before sizing |
| **W4** | Embedding Geometry & Flexibility | Diagnostics, geometry-aware config, thinking preservation | independent of W2 — embedding config is orthogonal to provider migration |
| **W5** | M1 Backlog | Batch processing, context, secrets, file tools | independent — good filler between larger waves |
| **W6** | Responsive Chat Rebuild | Ratatui rendering engine, event loop, crossterm input | after critical bugs; prerequisite for M2's TUI |
| **W7** | Thinking Trace Pipeline | ThinkingTrace pipeline, thinking-aware retrieval | after W6 and W4's thinking-preservation work |

**Why this order:** the provider chain had to land first because every other subsystem
talks to the LLM through it. Embedding geometry is orthogonal and was safe to run in
parallel. W6 (Ratatui) preceded the TUI milestone deliberately — it delivered the
rendering engine, event loop and input backend as a *working chat*, so M2 builds on
something already in production rather than a rewrite.

> The `#NNN` issue numbers that used to appear here were removed: they were the
> mechanism by which this file and the tracker drifted apart. Waves are described by
> theme now; the issues live in Linear.

## Architecture Direction

Where the system is going, independent of which milestone delivers it.

- **Provider abstraction** — one `LlmProvider` trait; OpenAI-compatible as the universal
  wire format. Ollama, llama.cpp, LM Studio, vLLM and cloud providers are all reached
  through it. No vendor-specific client code in business modules.
- **Memory as the differentiator** — hybrid retrieval (BM25 + vector + RRF) with
  feedback-aware weighting, a factual memory system with Ebbinghaus decay, and thinking
  traces as a retrieval corpus. The competitive bet is memory quality, not model parity.
- **Harness-only adaptation** — no fine-tuning. Everything the system "learns" is an
  adjustment in the harness (prompts, weights, retrieval), which keeps it O(1) to change
  and viable on local hardware.
- **Responsive by default** — the chat UI adapts to terminal width; nothing assumes 80
  columns.

## Sprach 2.0: CAS Research [M3]

**Full design and open questions:** [Sprach 2.0 Research](./sprach-2-0-research.md)

Self-analysis identifying Sprachspiel as a Complex Adaptive System with emergent
properties but limited open-endedness. Proposals aim to increase emergent connectivity
and adaptive behaviour.

| ID | Proposal | Depends on | Effort |
|----|----------|------------|--------|
| S2.1 | Visualize Connections Tool | none | 2-3 days |
| S2.2 | Content Relations Graph (2-layer) | S2.1 | 5-8 days |
| S2.3 | Reflection on Triggers + Curation | S2.1, S2.2 | 4-7 days |
| S2.4 | Plugin System (WASM) | — | 3-4 weeks |
| S2.5 | SOUL.md Patching + `/apply-patch` | S2.3 | 3-5 days |
| S2.6 | Skills Auto-Registration (Meta) | S2.1–S2.5 | TBD |
| S2.meta1 | Meta-cognition Skill (Layer 1) | none | 1h |
| S2.meta2 | Behavioural Telemetry (Layer 2) | feedback system, S2.meta1 | 2-3 days |
| S2.meta3 | Behavioural Reflection + Personality (Layer 3) | S2.3, S2.5, S2.meta2 | 1-2 weeks |

The three meta-cognition layers are deliberately ordered: Layer 1 is a prototype and
data-collection instrument (it works with high-reasoning models but cannot guarantee
execution), Layer 2 is the real implementation (deterministic heuristics in the harness,
not LLM self-monitoring), and Layer 3 needs S2.3 and S2.5 first. The reframing that
matters: **empathy is not a bug — opacity is.** The goal is not to suppress behavioural
shifts but to make them visible and give the user control.

## [M4] Future — Deferred

Features explicitly deferred, with the reason. "Deferred" here means *no current
priority*, not *rejected*.

| Feature | Why deferred |
|---------|--------------|
| AutoDream full daemon (4-phase) | After Sprach 2.0 |
| Cost Tracking | After Sprach 2.0 |
| Multi-scope Memory (team/private) | Not applicable — the harness is not code-focused |
| Context Collapse | Observe, don't implement |
| VCR Testing | When CI is robust |
| Speculation | Indefinite deferral |
| Remote Agent | Needs the plugin system first |
| ACP Agent Integration | After TUI decoupling |
| Team Memory Sync | Team use only |
| Remote Managed Settings | Enterprise |
| Worktree-aware sessions | Niche |
| Thinkback Marketplace | Too premature |
| Session Summary / Away Summary | Discarded — session continuation already covers it |

## See Also

- [Architecture](./architecture.md) — how the system works today
- [Decision Records](../adr/README.md) — why specific decisions were made
- [Implementation Directive](./implementation-directive.md) — strategic direction for continuous learning
- [Research Icebox](./research-icebox.md) — deferred research topics
- [Completed Features](./completed-features.md) — what has shipped
- [Linear project](https://linear.app/luksamuk/project/sprachspiel-05bb6fb1a5fd) — **the issue tracker**
