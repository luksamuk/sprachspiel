# Belief System Design: Post-PR 104 Refinement

**Date:** 2026-04-28
**Status:** Draft — not in current scope
**Related:** PR 104 (fact extraction + contradiction detection), `src/facts/conflict.rs`, `src/facts/lang.rs`
**Tracked as:** Draft B2 in IMPLEMENTATION.md

---

## The Insight

The fact contradiction engine (triples, exclusivity, polarity flips, word overlap) is **already generic** — it operates on `(subject, predicate, object)` triples extracted from any text. Today it's coupled to the fact store's decision policy ("delete old, insert new"), but the **detection** layer is separable.

If we abstract the detection engine from the fact store's action layer, we get a **Belief System**: a module that can track, detect conflicts in, and revise beliefs from ANY source — user facts, chat messages, project docs, even external knowledge bases.

---

## Current Architecture (PR 104)

```
┌──────────────────────┐     ┌──────────────────────────────┐
│  Fact Store          │     │  Content Store (messages)     │
│                      │     │                              │
│  INSERT path:        │     │  No dedup, no conflict       │
│  1. Exact match      │     │  detection — just appends    │
│  2. Normalized match │     │  chunks and searches them     │
│  3. Semantic search   │     │                              │
│  4. Triple cascade   │     │  Shares: vec0, embeddings,   │
│  5. FTS5 BM25       │     │  similarity thresholds        │
│  6. Insert/Replace   │     │                              │
└──────────────────────┘     └──────────────────────────────┘
```

Both stores share:
- `sqlite-vec` + `nomic-embed-text` for semantic search
- L2→cosine conversion (`1 - L2²/2`) at application level
- FTS5 BM25 for keyword search

But only the fact store has **conflict resolution**. Messages just accumulate.

---

## Proposed Abstraction

### Core Engine (extract from `conflict.rs`)

```rust
// belief_engine.rs — domain-independent belief revision

pub struct BeliefEngine {
    classifiers: PredicateClassifiers,  // from lang.rs
}

pub enum BeliefRelation {
    Exclusive,      // prefers, name_is — one winner
    Accumulative,   // likes, uses — can coexist
    PolarityFlip,   // likes vs hates — always contradicts
}

pub enum ConflictVerdict {
    Contradiction { reason: ConflictReason },
    Duplicate,
    Coexist,
}

impl BeliefEngine {
    /// Extract triple from text, classify predicate, detect conflict.
    /// Returns None if text doesn't match any known pattern.
    pub fn analyze(a: &str, b: &str) -> Option<ConflictVerdict>;

    /// Check if a predicate is mutually exclusive
    pub fn is_exclusive(predicate: &str) -> bool;

    /// Check if two predicates are polarity flips
    pub fn is_polarity_flip(a: &str, b: &str) -> bool;

    /// Word overlap between objects (for accumulative same-category detection)
    pub fn object_overlap(a: &str, b: &str) -> f32;
}
```

### Action Layer (stays in fact store)

```rust
// What to DO when BeliefEngine says "Contradiction" — this is fact-store-specific
fn handle_conflict(verdict: ConflictVerdict, existing: Fact, new: Fact) {
    match verdict {
        Contradiction { reason: PreferenceOverride } => {
            db.delete_fact(existing.id);
            db.insert_fact(&new);
        }
        Contradiction { reason: PolarityFlip } => {
            db.delete_fact(existing.id);
            db.insert_fact(&new);
        }
        Duplicate => skip!(),
        Coexist => db.insert_fact(&new),
    }
}
```

### What changes for messages?

```rust
// In content store — CONFLICTING beliefs from different conversations
fn on_new_message_chunk(chunk: &str) {
    let triples = BeliefEngine::extract_triples(chunk);
    for triple in triples {
        let existing = search_similar_beliefs(&triple);
        match BeliefEngine::analyze(&triple, &existing) {
            Contradiction(reason) => {
                // DON'T delete — mark old chunk as superseded
                // Surface to user: "Hey, you said X before, now you're saying Y"
                mark_superseded(existing, reason);
            }
            Duplicate => {}, // already known
            Coexist => store_belief(triple),
        }
    }
}
```

---

## Implementation Phases

### Phase 1 (near-term) → Draft B2.1 + B2.2

- Extract `BeliefEngine` from `conflict.rs` as standalone module
- Make `analyze()` return `ConflictVerdict` without taking action
- Fact store calls `BeliefEngine::analyze()` then decides policy
- Content store can optionally call `BeliefEngine::analyze()` for belief revision prompts
- No new dependencies, no Prolog, no Lisp

### Phase 2 (medium-term) → Draft B2.3

- Add missing verbs to `TRIPLE_*_PREFIXES` (favors, chooses, selected, etc.)
- PT irregular patterns (melhor, escolhe, etc.)
- Negation patterns with object-level negation ("usuário não é vegetariano")

### Phase 3 (exploratory) → Research Icebox R-05, R-06, R-10

- Evaluate LLM adjudication for gray-area cases (vim vs emacs) — See R-05
- Evaluate Crepe (Rust Datalog) as optional rule engine — See R-06
- Belief versioning (invalidated_at instead of delete) — Draft B2.4
- Multi-source belief reconciliation (user says X, docs say Y) — See R-10

> **Note:** Majestic Lisp as MCP server or rule engine was evaluated and explicitly excluded from scope. See Decision Record D-01 in the Research Icebox.

---

## Key Metrics (for reference)

| Approach | Latency | Binary Cost | Accuracy (pref override) | Current? |
|----------|---------|-------------|--------------------------|----------|
| Prefix triple + exclusivity/overlap | <1ms | 0 | ~95% | ✅ Yes |
| Prolog-style rule engine | ~1-5ms | ~50KB | ~98% | ❌ Future |
| LLM adjudication (local) | ~200ms-5s | 0 (Ollama) | ~70% | ❌ Rejected (Gokul 2025) |
| ONNX NLI cross-encoder | ~50-100ms | ~30MB | ~40% | ❌ Rejected |

---

## References

### Primary (this project)

- `src/facts/conflict.rs` — current implementation
- `src/facts/lang.rs` — predicate classification constants (source of truth)
- `doc/src/development/factual-memory-system.md` — fact system design
- `doc/src/development/feedback-architecture.md` — feedback-driven memory

### Competitive (from research)

See Research Icebox entries:
- C-01 (YantrikDB) — almost identical architecture, AGPL license concern
- C-02 (Kumiho) — AGM postulates, immutable revisions pattern adopted for B2.4
- C-03 (Crepe) — Rust Datalog, best candidate for rule engine if needed
- C-06 (Letta/MemGPT) — validates that LLM adjudication is unreliable

### Key Papers

- **Gokul et al. 2025** (arXiv:2504.00180): Contradiction detection in RAG. LLMs miss >30% of contradictions. Validates heuristic-first approach.
- **Li & Qin 2017** (MDPI Information): Antonyms have high cosine similarity. Our design uses this as a feature.
- **Kumiho (Mar 2026)**: Formal AGM belief revision on property graphs. Proves immutable revisions + mutable tags satisfy AGM postulates.