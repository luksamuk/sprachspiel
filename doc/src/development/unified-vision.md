# Unified Vision: Sprachspiel Architecture Reconciliation

**Status:** Reference document (reconciled with actual implementation + research synthesis)
**Original Date:** 2026-03-13 (Portuguese, original in external research notes)
**Reconciled Date:** 2026-04-28
**Research Synthesis Date:** 2026-05-24

> This document consolidates the original "Visão Unificada" with the actual
> implementation decisions. Sections marked with ✅ have been implemented (possibly
> differently from the original proposal); sections marked with ❌ are pending
> and tracked as draft priorities in IMPLEMENTATION.md.
>
> **2026-05-24 Update:** Added Section 8 (Research Synthesis) consolidating findings
> from published research (see Section 8.9 for full citations). This section maps research insights
> to the project roadmap, identifies synergies, and proposes new cards. Key decision:
> Cultural Grounding moved to M4 (important but not the current priority; memory is).

---

## 1. The Central Problem

### 1.1 Current State

| Component | Original Vision | Actual Implementation | Status |
|-----------|----------------|----------------------|--------|
| Persistent chat | ✅ SQLite complete | Same — `content_items` table with `content_type='message'` | ✅ Match |
| Semantic search | ✅ BM25 + Vector + RRF | Same — hybrid retrieval with RRF fusion | ✅ Match |
| Tools (28+) | ✅ Feature flags | Same — now 50+ tools in 14 categories | ✅ Match |
| Context management | ✅ Auto compaction | Same — 4-tier percentage-based thresholds | ✅ Match |
| Project-aware | ✅ AGENTS.md injection | Same | ✅ Match |
| Document import | ❌ Planned | ✅ Implemented as P3 (v0.39.0) — `content_type='document'` in `content_items` | ✅ Diverged |
| Notes system | ❌ Planned | ✅ Implemented as P4 priority — `content_type='note'` in `content_items` | ✅ Diverged |
| Study sessions | ❌ **NEW** | ❌ Pending — tracked as Draft B3 | ❌ Pending |
| Memory with cross-session | ❌ Critical gap | ✅ Factual Memory (P0) + Feedback (P5) + Content Decay (P5) | ✅ Diverged |
| Feedback-driven decay | ❌ Not in original | ✅ Implemented (P5 V4) — Ebbinghaus curves with feedback signals | ✅ Beyond original |
| Retrieval-reinforced retention | ❌ Not in original | ✅ Implemented (P5 V4) — `on_content_access()` reinforces retention | ✅ Beyond original |

### 1.2 Reconciled Conflicts

**Notes System (planned) vs. Memory System (needed)**

Both unified under `content_items` with `content_type` enum. Notes are user-created items with 60-day half-life — not "never decay" as originally proposed. See P4 in IMPLEMENTATION.md.

**Document Import (planned) vs. Study Sessions (new)**

Document Import ✅ DONE (P3). Study Sessions share chunking/embedding infrastructure but add a verification layer. Tracked as Draft B3.

**"Forget" vs. Decay Temporal**

Decay implemented, not delete. Ebbinghaus formula `2^(-t/half_life)` with per-type half-lives: messages=90d, notes=60d, documents=120d, facts=30/180d. `prune` command offers manual override. See P5 in IMPLEMENTATION.md and feedback-architecture.md.

**Skills System vs. AGENTS.md vs. Study Templates**

Complementary, not conflicting. Skills = behavioral instructions (markdown). AGENTS.md = project context (injected automatically). Study Templates = pending (Draft B3).

---

## 2. Unified Knowledge Store — As Implemented

The original proposal used a `knowledge_items` table. The actual implementation uses `content_items`:

```sql
-- ACTUAL SCHEMA (v12, simplified)
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
    importance REAL DEFAULT 0.5,          -- from P5 feedback system
    decay_score REAL DEFAULT 1.0,         -- from P5 Ebbinghaus decay
    access_count INTEGER DEFAULT 0,        -- from P5 retrieval-reinforced retention
    last_accessed INTEGER,                 -- from P5
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    project_id TEXT,
    has_embedding INTEGER DEFAULT 0,

    -- Document-specific (v8)
    filename TEXT,
    file_type TEXT,
    word_count INTEGER
);
```

**Key differences from original `knowledge_items`:**

| Aspect | Original `knowledge_items` | Actual `content_items` |
|--------|---------------------------|----------------------|
| Table name | `knowledge_items` | `content_items` |
| `source_type` enum | msg/note/doc/study/extract | message/note/document (study pending as B3) |
| `importance` | Single field, 0.0-1.0 | Same, but driven by feedback signals (good +0.05, bad -0.1) |
| `feedback_score` | Accumulated from feedback | Replaced by `importance` adjustments — feedback modifies importance directly |
| `decay_score` | Not in original | Added in P5 — Ebbinghaus `2^(-t/half_life)` |
| `access_count` | Not in original | Added in P5 — retrieval reinforces retention |
| Decay formula | `importance * access_count * POW(0.95, days)` | `2^(-t/half_life)` with per-type half-lives |
| Note decay | "Never (user control)" | 60 days (Ebbinghaus, but user can always re-access to reinforce) |
| Document decay | 90 days | 120 days (longer, since documents are explicitly saved) |

---

## 3. Reconciled Features

### 3.1 What Was COMBINED (as proposed)

| Original Feature | Proposed Partner | Action | Status |
|-----------------|------------------|--------|--------|
| Notes System | Memory System | **Combined** — notes ARE memory items with `content_type='note'` | ✅ Done |
| Document Import | Knowledge Ingest | **Combined** — documents ARE memory items with `content_type='document'` | ✅ Done |
| Feedback System | Decay Temporal | **Integrated** — feedback adjusts decay speed via importance | ✅ Done (P5 V4) |
| AGENTS.md | Context Injection | **Kept separate** — AGENTS.md is project context, not memory | ✅ Working well |

### 3.2 What Was SEPARATED (as proposed)

| Feature | Reason to Keep Separate | Status |
|---------|------------------------|--------|
| Verification Layer | Independent of knowledge — about output quality | ❌ Pending → Draft B3 |
| System Reminders | Operational, doesn't affect persistence | ❌ Pending → Draft B6.1 |
| Skills System | Instructional, not data | ✅ Done (P3) |

### 3.3 What Was ADDED (beyond original)

| Feature | Description | Status |
|---------|-------------|--------|
| Feedback-driven decay | `2^(-t/h)` with feedback adjusting importance → decay speed | ✅ Done (P5) |
| Content-type half-lives | messages=90d, notes=60d, documents=120d, facts=30/180d | ✅ Done (P5) |
| Retrieval-reinforced retention | `on_content_access()` increments `access_count` and boosts importance | ✅ Done (P5) |
| Fact contradiction engine | 6-layer dedup pipeline with heuristic triple-based detection | ✅ Done (P6.1/P6.7) |
| Unified content storage | `content_items` table for messages, notes, documents | ✅ Done (schema v7+) |

---

## 4. Unified Phases — Reconciled

### Phase 1: Unified Knowledge Store → ✅ DONE

Original proposal → Actual implementation:
- Schema migration: ✅ Done (`content_items` table, schema v7→v12)
- Decay calculator: ✅ Done (Ebbinghaus `2^(-t/h)` in `content/decay.rs` and `facts/decay.rs`)
- Memory injection in context: ✅ Done (`remember()` tool, hybrid search)
- Feedback system basic: ✅ Done (P5 V4 — `/feedback good/bad/correction`)

### Phase 2: Document Import → ✅ DONE

- Chunking infrastructure: ✅ Done (dynamic chunk sizing, `content_chunks` table)
- Document sources: ✅ Done (TXT, MD, ORG builtin; PDF, EPUB via skills)
- Import commands: ✅ Done (`/doc import`, `/doc list`, `/doc show`, `/doc delete`)
- Source attribution: ✅ Done (`filename`, `file_type`, `word_count` columns)

### Phase 3: Notes System → ✅ DONE

- Note commands: ✅ Done (`/note add/list/show/edit/delete`)
- Note storage: ✅ Done (`content_type='note'`, scope project/global)
- Note retrieval: ✅ Done (hybrid search includes notes, `remember()` discovers notes)

### Phase 4: Verification Layer → ❌ PENDING (Draft B3)

- Status: Tracked as Draft B3 in IMPLEMENTATION.md
- `study` source type: Planned for `content_items` with `content_type='study'`
- Verifier trait: Planned (CodeVerifier, CrossModelVerifier, StudyVerifier)
- Verified knowledge boost: Planned (importance 0.9, 180d half-life)

### Phase 5: Advanced Features → PARTIALLY DONE

| Feature | Original Plan | Actual Status |
|---------|---------------|---------------|
| System Reminders | ReminderTrigger enum + templates | ❌ Pending → Draft B6.1 |
| Auto-extraction | Extract facts from conversations | ✅ Done (P6.1 — `fact_add` auto-extraction) |
| Learned Patterns | Detect usage patterns, adapt prompts | ❌ Pending → Draft B6.2 |
| Decay Management UI | `/memory stats/forget/prune` | ❌ Pending → Draft B6.3 |

---

## 5. Source Type Decay Rates — As Implemented

| Source Type | Original Half-Life | Actual Half-Life | Rationale |
|-------------|------------------|-----------------|-----------|
| `message` | 30 days | 90 days | Conversations carry nuance and context; original was too aggressive |
| `note` | Never (user control) | 60 days | User-curated but not permanent; re-access reinforces retention |
| `document` | 90 days | 120 days | Explicitly saved, longest retention |
| `fact` (preference) | Not in original | 180 days | Self-reinforcing preferences persist longer |
| `fact` (regular) | Not in original | 30 days | Project-contextual, more ephemeral |

---

## 6. Diagrams

The following diagrams illustrate the key architectural flows and data structures of Sprachspiel. Each diagram is a standalone HTML file with dark-themed SVG, versioned alongside the documentation source.

### 6.1 Data Flow Architecture

End-to-end pipeline from user input through ingestion, storage, retrieval, context building, verification, and feedback loop.

<img src="../assets/diagrams/data-flow.svg" alt="Sprachspiel Data Flow Architecture" style="width:100%; max-width:1200px;">

### 6.2 Content Type Decay Rates

Half-life reference table and Ebbinghaus decay curves (R(t) = 2^(-t/h)) for each content type.

<img src="../assets/diagrams/decay-rates.svg" alt="Content Type Decay Rates" style="width:100%; max-width:1200px;">

### 6.3 Source Types and Storage

Content_items unified schema (v12) with type-specific fields, shared columns, and auxiliary tables.

<img src="../assets/diagrams/source-types.svg" alt="Source Types and Storage" style="width:100%; max-width:1200px;">

### 6.4 Dedup Pipeline

6-layer fact deduplication pipeline: Exact Match → Normalized → Semantic + Triple Disambiguation → FTS5 → Startup Verification → Global-Wins-Project.

<img src="../assets/diagrams/dedup-pipeline.svg" alt="Fact Dedup Pipeline" style="width:100%; max-width:1200px;">

### 6.5 Feedback-Driven Memory Lifecycle

Complete feedback loop: /feedback signals → importance adjustment → Ebbinghaus decay → hybrid RRF retrieval → retrieval reinforces retention.

<img src="../assets/diagrams/feedback-loop.svg" alt="Feedback-Driven Memory Lifecycle" style="width:100%; max-width:1200px;">

### 6.6 Belief Engine Abstraction

Proposed domain-independent BeliefEngine with ConflictVerdict types (Contradiction/Duplicate/Coexist) and divergent store policies.

<img src="../assets/diagrams/belief-engine.svg" alt="Belief Engine Abstraction" style="width:100%; max-width:1200px;">

### 6.7 Milestone Map

Reconciled milestone progression: M1 Core Evolution → M2 UX & Pre-Launch → M3 Sprach 2.0 → M4 Future.

<img src="../assets/diagrams/milestone-map.svg" alt="Sprachspiel Milestone Map" style="width:100%; max-width:1200px;">

---

## 8. Research Synthesis — From Papers to Roadmap

**Added:** 2026-05-24
**Sources:** Arabzadeh et al. 2026 (arXiv:2605.03344), Gu et al. 2026 (arXiv:2605.19932), Diógenes et al. 2026 (PRW-5188-2880), Zhang et al. 2025 (arXiv:2512.24601), Zandieh et al. 2026 (ICLR 2026, arXiv:2504.19874), Gao & Long 2024 (SIGMOD 2024, arXiv:2405.12497), and analysis of passive models (BusyBeaver, Privacy Filter, LlamaFirewall, WebWorld, Needle). See Section 8.9 for full citations.
**Key Decision:** Cultural Grounding (R-24) moved to **M4** — important but not the current priority. Memory pipeline (TAP, PEEK, feedback) is the priority.

### 8.1 Source Map

| # | Paper/Source | Focus | Roadmap Impact |
|---|-------------|-------|----------------|
| 1 | T3 (arXiv:2605.03344) | RAG over thinking traces | P0-CRITICAL (M1) |
| 2 | PEEK (arXiv:2605.19932) | Context Map as Orientation Cache | P1-P2 (M3) |
| 3 | Trace Analysis Pipeline (synthesis) | Unified offline analysis pipeline | P0-P2 (M1→M3) |
| 4 | Passive Models (BusyBeaver, Privacy Filter, etc.) | Passive models as middleware | P2-P3 (M3/M4) |
| 5 | NLP Historical Errors (PRW-5188-2880) | Cultural grounding, TTR, invisibility principle | P2-P3 (M4) |
| 6 | RAG Vector Index (TurboQuant, RaBitQ) | Norm correction, d_eff | P1 (M1) |
| 7 | Recursion/RLM (arXiv:2512.24601) | Context-offload, session variables | P2 (M3, conditional) |
| 8 | Translation Models (Hy-MT2, TranslateGemma) | Canary for cultural fragility | P2-informative (M3+) |

### 8.2 Memory Architecture: From 5 Layers to 7 + 2 Cross-Cutting

**Current (5 layers):**

```
Layer 1: Session Memory (volatile, RAM)
Layer 2: Conversation Memory (SQLite + BM25 + Vector + RRF)
Layer 3: Factual Memory (facts with decay, dedup, conflict resolution)
Layer 4: Context Assembly (system → facts → retrieved → recent → query)
Layer 5: Feedback Memory (quality signals + weighted decay)
```

**Proposed (7 layers + 2 cross-cutting):**

```
Layer 1: Session Memory (volatile, RAM)
Layer 2: Conversation Memory (+ thinking-aware retrieval, if enabled)
Layer 3: Factual Memory (+ auto-extracted facts, if enabled)
Layer 4: Context Assembly (with Orientation Cache between AGENTS.md and Facts)
Layer 5: Feedback Memory (+ auto-feedback source='auto', if enabled)
Layer 6: Orientation Cache 📋 (persistent context map, prompt-resident)
Layer 7: Trace Analysis Pipeline 📋 (orchestrates 4→6 and 3→5)

CROSS-CUTTING A: Cultural Grounding
  - SOUL.md with cultural anchoring (NLP-Historical §7)
  - S2.5 Patching + S2.meta2/S2.meta3 (personality evolution)
  - Passive models for calque/pragmatics detection (invisibility principle)
  - Translation fleet as cultural fragility canary
  - ⚠️ MOVED TO M4 — memory is the current priority

CROSS-CUTTING B: Resource Efficiency
  - Norm correction in embeddings (RAG-Vector §2)
  - Context-offload before compaction (RLM §3)
  - Session variables on demand (RLM §4)
  - Context Strategy Comparison Benchmark B1.5 (RLM §7.5, #124)
```

### 8.3 Key Synergies

**Synergy 1: T3-Reflect → PEEK Distiller → Behavioral Telemetry (S2.meta2)**

The Reflect pipeline extracts failure patterns. The Distiller needs those patterns to classify orientation vs. task. Behavioral telemetry detects drift in real-time. Flow:

```
S2.meta2: heuristic detects conservative drift (low TTR, §9.3)
  → TAP-Reflect: processes trace offline, confirms pattern
    → PEEK Distiller: uses Reflect to identify stall points
      → Orientation Cache: stall_points feeds diagnosis
        → S2.5: if pattern persists, proposes SOUL.md patch
```

**Synergy 2: Passive Models × Curatorial Invisibility**

The invisibility principle (NLP-Historical §9.7): prioritize curatorial middleware by how invisible the error is to the model:

| Priority | Passive Model | Detects | Invisibility |
|----------|--------------|---------|-------------|
| 1 | Confidence scorer | Competence illusion | Total |
| 2 | Pragmatics classifier | pt-BR subtext loss | High |
| 3 | Calque detector | "Como posso ajudá-lo?" | Medium |
| 4 | TTR monitor | Conservative vocabulary | Low (measurable) |

**Synergy 3: Translation Fleet × Cultural Fragility Canary**

If specialized translation models (Hy-MT2, TranslateGemma) fail on pt-BR slang, general models fail worse. SOUL.md anchoring is the first mitigation (~40-60%). S2.5 patches are the second. Cultural RAG is the third (M4+).

**Synergy 4: Norm Correction × d_eff × TAP-2 (RAG-Vector × T3 × #136)**

`norm_correction` from TurboVec/TurboQuant improves TAP-2 (thinking-aware retrieval) directly. When `d_eff < 0.7` (768→256 Matryoshka truncation), cosine similarity is biased. One float per vector corrects this at zero query-time cost. This is a **prerequisite of TAP-2**.

**Synergy 5: Context-Offload × TAP × Compaction (RLM × TAP)**

Offload via sub-agent resolves 1 of 3 context pressure sources (large tool results). Compaction remains inevitable for long history and system prompt. But the compaction summary is a **free Distiller** — TAP runs after compaction using the summary as additional input.

**Synergy 6: AGENTS.md → Context Map × Dynamic SOUL.md (PEEK × NLP-Historical)**

Context Map (PEEK) gradually replaces AGENTS.md. Dynamic SOUL.md (S2.5 + NLP-Historical) gradually replaces static SOUL.md. Same trajectory: static → semi-structured → dynamic.

**Synergy 7: Empathy ≠ Failure × Curatorial Transparency (NLP-Historical §10)**

The "empathy ≠ failure" reframing establishes: behavioral shifts are not bugs, but opacity is. The goal is not to suppress shifts, but make them visible. This orients all of S2.meta1-3 (#99/#100/#101). Flow:

```
S2.meta2: detects behavioral change (not failure)
  → Asks user about preferred mode (transparency > suppression)
    → If pattern persists: S2.5 patch proposal (with human approval)
```

### 8.4 Unified Roadmap — Research Informed

#### M1 (Core Evolution) — No Structural Changes

Only one addition:

| Item | Phase | Source | Depends On | Issue |
|------|-------|--------|-----------|-------|
| **NEW:** Norm correction in embeddings | W4.x addendum | RAG-Vector §2 | #133 (diag) | To create |

Everything else is already planned in the 7 waves. TAP-0 (#151), TAP-1 (#152), TAP-2 (#153 + #137) remain P0-CRITICAL/P0-HIGH.

#### M2 (UX & Benchmarks) — One Addition

| Item | Phase | Source | Depends On | Issue |
|------|-------|--------|-----------|-------|
| LoCoMo benchmark adaptation | B1.1 | T3 | W7.1 | #124 |
| Feedback-driven decay benchmark | B1.2 | FadeMem | W3 | #124 |
| RAG quality benchmark | B1.3 | — | B1.1 | #124 |
| **NEW:** Context Strategy Comparison | B1.5 | RLM §7.5 | B1.1 harness | To create |

B1.5 tests H1-H6: offload vs compaction vs importance eviction vs minimal injection with local models. Tier 2 initially; promotes to Tier 1 if H6 confirmed (small models can't manage their own search).

#### M3 (Sprach 2.0) — Reordered Priorities

**Ordered by dependency and impact:**

| # | Item | Phase | Source | Depends On | Priority |
|---|------|-------|--------|-----------|----------|
| 1 | Empathy ≠ Failure (principle, not code) | — | NLP-Hist §10 | None | P1 (immediate doc) |
| 2 | OC-1a: Static Context Map | OC-1a | PEEK | None (no LLM) | P1-high |
| 3 | OC-1b: Compaction summary → TAP | OC-1b | DESIGN §12.4 | TAP-1 (#152) | P1-high |
| 4 | TAP-BENCH: Model evaluation benchmark | — | DESIGN §11.5 | TAP-1 | P1 |
| 5 | TAP-3: Unified prompt + batch + auto-feedback | TAP-3 | T3+PEEK+Feedback | TAP-2, W3 | P1 |
| 6 | OC-2: Dynamic Distiller | OC-2 | PEEK | TAP-3 | P1 |
| 7 | TTR/Entropy monitoring as Reflect input | — | NLP-Hist §9.3 | #100, #153 | P2 |
| 8 | Context-Offload via sub-agent (feature flag) | — | RLM §3 | B1.5 (validate H1) | P2 (conditional) |
| 9 | Session Variables on-demand | — | RLM §4 | OC-1a or offload | P2 |
| 10 | Passive Models pipeline design | — | Passive Models + NLP-Hist | #15 (Plugin System) | P2-low |
| 11 | Translation Fleet canary test | — | Translation + NLP-Hist §9.2 | N/A | P2-informative |

**Note:** Cultural Grounding (SOUL.md enraizamento) was originally proposed for M3 but is **moved to M4** per user decision. Memory pipeline is the current M3 priority.

#### M4 (Future) — With Cultural Grounding

| Item | Phase | Source | Depends On | Priority |
|------|-------|--------|-----------|----------|
| **NEW:** SOUL.md cultural anchoring section | Phase 1 (doc only) | NLP-Hist §7 | None | P2 (immediate in M4) |
| **NEW:** S2.5 Patching + S2.meta2/S2.meta3 for cultural evolution | Phase 2 | NLP-Hist §8 | #80, #100, #101 | P2 |
| OC-3: Cartographer + Evictor | OC-3 | PEEK | OC-2 | P2 |
| AGENTS.md → Context Map transition | — | PEEK × DESIGN §12.6 | OC-1a, OC-2 | P2 |
| **NEW:** Passive cultural evolution (auto-detection) | Phase 3 | NLP-Hist §8 | S2.5, #15, Passive Models | P3 |
| **NEW:** Passive Models as curatorial middleware | Phase 3 | R-28 | #15 | P3 |
| **NEW:** pt-BR cultural knowledge RAG | Phase 4 | NLP-Hist §9.2 | Plugin system | P3 |
| Behavioral embeddings | R-13 | NLP-Hist §9.7 | #100 data | P3 |
| TurboQuant-style quantization in sqlite-vec | — | RAG-Vector §3 | 50K+ vectors | P3 |

### 8.5 Important Constraints (Cross-Cutting)

1. **Benchmark-Driven Validation:** No context architecture change (offload, importance eviction, minimal injection, session variables) may be implemented without controlled benchmarks. Memory of the user is clear on this.

2. **Small Model Skepticism:** Models <10B are probably inadequate for trajectory analysis. The same-model/CPU-fallback cascade needs controlled benchmarking (DESIGN §11.5) before assuming any small model works.

3. **Compaction Is Inevitable:** Offload defers compaction for tool results. Context Map reduces retrieval. Importance eviction improves what compaction preserves. TAP recovers information offline. **But none eliminates compaction.** The goal: see less, lose less, recover later.

4. **No Python Sidecar:** Any auxiliary model runs via Ollama (already integrated) or as separate binary with IPC — not as coupled Python process. Philosophy is single-binary with extensions via plugin system (WASM or Ollama sidecars).

5. **Translation ≠ Passive:** Translation models are **active** (generate text), not passive (classify). They don't enter the middleware pipeline as classifiers — but serve as canary test and fleet specialist.

### 8.6 New Cards to Create (GitHub Issues)

| # | Title | Priority | Milestone | Depends On | Type |
|---|-------|----------|-----------|------------|------|
| N1 | SOUL.md — Cultural Grounding Section | P2 | M4 | None | Documentation |
| N2 | Norm Correction in Embedding Tables | P0 (addendum) | M1/W4.x | #133 | Code (~20 lines) |
| N3 | B1.5 — Context Strategy Comparison Benchmark | P2 (Tier 2) | M2 | #124 B1.1 | Benchmark |
| N4 | Empathy ≠ Failure — Meta-cognition Reframing | P1 (principle) | M3 | None | Documentation |
| N5 | TTR/Entropy Monitoring as Reflect Input | P2 | M3 | #100, #153 | Code |
| N6 | Context-Offload via Sub-Agent (Feature Flag) | P2 (conditional) | M3 | B1.5 (H1) | Code |
| N7 | Session Variables On-Demand | P2 | M3 | OC-1a or N6 | Code |
| N8 | Passive Models Pipeline Design | P2-low | M3/M4 | #15 | Design |
| N9 | Translation Fleet as Cultural Fragility Canary | P2-informative | M3+ | R-24 Phase 2+ | Test pattern |

### 8.7 Existing Cards Needing Updates

| Issue | Update Needed | Source |
|-------|---------------|--------|
| #99 (S2.meta1) | Add "empathy ≠ failure" principle and phenomenology guardrail | NLP-Hist §10 |
| #100 (S2.meta2) | Add TTR/entropy as heuristic input signals for drift detection | NLP-Hist §9.3 |
| #101 (S2.meta3) | Reinforce: detection → ask user → recalibrate (never skip human approval) | NLP-Hist §10.3-10.4 |
| #80 (S2.5 SOUL.md Patching) | Connect with R-24 Phase 2: patches can include calques, pt-BR pragmatics | R-24 |
| #124 (Benchmarks) | Add B1.5 as sub-item: Context Strategy Comparison | RLM §7.5 |
| #152 (TAP-1) | Document synergy with PEEK Distiller (T3-Reflect → stall points → Orientation Cache) | PEEK |
| #153 (TAP-2) | Add norm_correction (R-25) as prerequisite — d_eff bias affects thinking-aware retrieval | RAG-Vector §2 |
| #15 (Plugin System) | Include passive models as plugin type (WASM or Ollama sidecar), prioritized by invisibility | R-28 |

### 8.8 Research Icebox Additions (R-21 to R-28)

Added to `doc/src/development/research-icebox.md`:

- **R-21:** Orientation Cache (PEEK) — phased: OC-1a → OC-1b → OC-2 → OC-3
- **R-22:** Trace Analysis Pipeline (unified background analysis) — TAP-0 through TAP-4
- **R-23:** Compaction-Triggered Analysis — free Distiller from compaction summaries
- **R-24:** Cultural Grounding (Moved to M4) — 4 phases from SOUL.md anchoring to RAG
- **R-25:** Norm Correction for Embeddings — ~20 lines, 1 float per vector
- **R-26:** Context-Offload via Sub-Agent — conditional on B1.5 validating H1
- **R-27:** Translation Fleet as Cultural Fragility Canary — test pattern, not code
- **R-28:** Passive Models as Curatorial Middleware — prioritized by invisibility principle

---

## 7. Open Questions

1. **Study source type:** Should `content_type` enum add `'study'` or should study items be notes with a `verified` flag? Decision pending B3 design.

2. **Verification vs. Feedback:** Are verified items (passed quiz) fundamentally different from good-feedback items? Current design: yes — verified gets higher starting importance (0.9 vs 0.5+0.05). But should verification decay the same way?

3. **Decay formula unification:** `facts/decay.rs` and `content/decay.rs` share structural patterns. A future refactoring could extract a shared `Decayable` trait. Tracked as a note in IMPLEMENTATION.md (P5 section).

4. **Belief Engine vs. Fact Duplicates:** The belief engine (Draft B2) will extract contradiction detection from `conflict.rs` into a domain-independent module. This is a prerequisite for content-store-level belief revision (marking superseded content).

---

*This document was originally created in Portuguese on 2026-03-13 as "Visão Unificada do Sprachspiel: Confronto de Ideias e Roadmap Final". It was translated, reconciled with actual implementation decisions, and updated on 2026-04-28. Sections that diverged from the original proposal are clearly marked.*