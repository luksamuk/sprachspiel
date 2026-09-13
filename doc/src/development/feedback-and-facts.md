# Feedback Signals & Fact Deduplication

> **Status:** documentação técnica viva, extraída de `IMPLEMENTATION.md` durante a
> consolidação do backlog (LUC-140 / `refactor/backlog-consolidation`). O conteúdo é
> **verbatim** da fonte; apenas os cabeçalhos foram normalizados (eram marcados como
> "PRIORITY N … COMPLETED", resíduo de tracker).

Decaimento de feedback, o pipeline de dedup em seis camadas, e a extração automática de fatos.

## Feedback Infrastructure

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
| ADR-010 | Empathy ≠ Failure — Meta-cognition Reframing | Behavioral shifts are not bugs; opacity is. S2.meta1-3 (LUC-81/82/83) detect change (not failure), measure drift as neutral observation, make shifts visible with human approval — never auto-suppress. Source: NLP-Historical §10 (Diógenes et al. 2026). See Linear LUC-103 for full ADR body. |

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

---

## Fact Embeddings & Semantic Dedup

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

---

## Auto Fact Extraction (autoDream-lite)

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

---
