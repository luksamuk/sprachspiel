# Implementation Directive: Continuous Learning Agent

**Status:** CANONICAL  
**Version:** 1.0  
**Created:** 2026-03-13  
**Updated:** 2026-04-19  
**Sources:** OpenClaw-RL, MemOS, MemGPT, Unsloth/NVIDIA RL Environments

---

## Executive Summary

This document defines the **definitive implementation direction** for Sprachspiel to become a **continuous learning personal agent**. It synthesizes insights from:

- **OpenClaw-RL** (arXiv:2603.10165) - Learning from next-state signals
- **MemOS** (arXiv:2507.03724) - Memory as manageable system resource  
- **MemGPT** (arXiv:2310.08560) - Virtual context management
- **Unsloth/NVIDIA** - RL environments for agent training

The core insight: **Every user interaction contains learning signals that are currently discarded.** By capturing, structuring, and utilizing these signals, Sprachspiel can evolve from a stateless chat interface to a learning agent that improves with use.

---

## 1. The Unified Vision

### 1.1 Problem Statement

Current Sprachspiel:
- ✅ Remembers conversations (SQLite + embeddings)
- ✅ Searches context effectively (hybrid retrieval)
- ✅ Handles context overflow gracefully (continuation tags)
- ❌ **Does not learn from interactions**
- ❌ **Feedback is not captured or utilized**
- ❌ **No mechanism for personalization over time**

### 1.2 The Core Thesis (Validated by Literature)

> **User interactions contain two types of learning signals that should be captured and utilized:**
>
> 1. **Evaluative signals** - "How well did the response perform?" (positive/negative feedback)
> 2. **Directive signals** - "How should the response have been different?" (corrections, hints)
>
> These signals exist in:
> - Explicit feedback (`/feedback good/bad`)
> - User corrections (re-querying with different phrasing)
> - Conversation continuation patterns (did user continue or abandon?)
> - Tool success/failure outcomes

**Literature Corroboration:**

| Paper | Key Insight | Corroboration |
|-------|-------------|---------------|
| OpenClaw-RL | Next-state signals are universal learning sources | ✅ Core thesis |
| MemOS | Memory needs lifecycle management (not just storage) | ✅ Foundation |
| MemGPT | Agents can self-manage memory hierarchies | ✅ Architecture |
| CortexGraph | Memories should decay unless reinforced | ✅ Decay logic |

### 1.3 What Makes This Different

| Aspect | Traditional Chatbot | Sprachspiel (Proposed) |
|--------|---------------------|-------------------|
| Memory | Store everything equally | Weighted by feedback & recency |
| Learning | None from user | Implicit + explicit signals |
| Personalization | None | User-specific patterns reinforced |
| Context | Fixed retrieval | Adaptive based on success history |
| Evolution | Static | Improves with use |

---

## 2. Architecture Overview

### 2.1 Three-Tier Memory Model (Inspired by MemOS + MemGPT)

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SPRACHSPIEL UNIFIED ARCHITECTURE                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────┐  ┌──────────────────────────────────┐   │
│  │   TIER 3: PARAMETER   │  │   TIER 2: ACTIVATION (Future)   │   │
│  │   (Future: LoRA)      │  │   KV-Cache / Adapters            │   │
│  │                      │  │                                  │   │
│  │   - Skill adapters    │  │   - Session-level patterns       │   │
│  │   - User embeddings   │  │   - Working memory              │   │
│  │   - Long-term prefs   │  │   - Context compression         │   │
│  │                      │  │                                  │   │
│  │   ↑ OFFLINE BATCH    │  │   ↑ ONLINE LEARNING              │   │
│  └──────────────────────┘  └──────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────────┤
│  │                    TIER 1: PLAINTEXT (Current)                  │
│  │                                                                 │
│  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   │  WORKING     │  │  EPISODIC   │  │  FACTUAL            │  │
│  │   │  CONTEXT     │  │  (Chat DB)  │  │  (AGENTS.md/SOUL)   │  │
│  │   │              │  │             │  │                     │  │
│  │   │  - Recent    │  │  - Messages │  │  - Project context  │  │
│  │  │  - Retrieved │  │  - Embeddings│  │  - Personality      │  │
│  │  │  - Decays    │  │  - Feedback  │  │  - Skills loaded    │  │
│  │  │              │  │             │  │                     │  │
│  │  │  ↑ Session   │  │  ↑ Weighted │  │  ↑ Static            │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  │                                                                 │
│  └─────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────────┤
│  │                    LEARNING SIGNAL PIPELINE                     │
│  │                                                                 │
│  │   User Interaction → Signal Capture → Weight Application        │
│  │         ↓                  ↓                ↓                   │
│  │   ┌──────────┐      ┌──────────┐      ┌──────────┐           │
│  │   │ Explicit │      │ Implicit │      │  SQLite  │           │
│  │   │ /feedback│      │ Continue │      │  Update  │           │
│  │   │          │      │ Abandon  │      │          │           │
│  │   │ Good/Bad │      │ Re-query │      │ Weight   │           │
│  │   │ Correct  │      │ Repeat   │      │ Adjust   │           │
│  │   └──────────┘      └──────────┘      └──────────┘           │
│  │                                                                 │
│  └─────────────────────────────────────────────────────────────────┤
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Data Model Additions

```sql
-- Current: content_items + embeddings
-- Add: feedback_signals (separate table for auditability — ADR-003: messages-only in Phase 1)
-- NOTE: sprachspiel uses content_items table (NOT a separate messages table).
--       Messages are content_items with content_type='message'.

CREATE TABLE IF NOT EXISTS feedback_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL,                         -- FK to content_items (messages only in Phase 1)
    session_id TEXT,                                  -- Session context (nullable — metadata only)
    signal_type TEXT NOT NULL CHECK(signal_type IN ('good', 'bad', 'correction')),
    base_value REAL NOT NULL,                         -- Good=+1.0, Bad=-1.0, Correction=+1.0 (ADR-005)
    correction_text TEXT,                              -- For directive signals
    source TEXT NOT NULL DEFAULT 'user' CHECK(source IN ('user', 'llm')),
    created_at INTEGER NOT NULL,
    
    FOREIGN KEY (item_id) REFERENCES content_items(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_feedback_item ON feedback_signals(item_id);
CREATE INDEX IF NOT EXISTS idx_feedback_type ON feedback_signals(signal_type);
CREATE INDEX IF NOT EXISTS idx_feedback_created ON feedback_signals(created_at DESC);
```

> **Design decision (ADR-003):** Feedback targets `content_items.id` (messages only in Phase 1).
> When Unified Knowledge Store is implemented, `feedback_signals.item_id` can reference
> `knowledge_items.id` for other content types (facts, notes, documents).
>
> **Rejected approach:** Adding `feedback_weight` column to `content_items` would conflate
> signal storage with content storage. Separate table preserves auditability and enables
> Phase 2 extension. No `ALTER TABLE content_items` is needed.

### 2.3 Component Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                       SPRACHSPIEL COMPONENTS                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────────┐    │
│   │   User      │────▶│   REPL      │────▶│  Coordinator    │    │
│   │   Interface │     │   (chat)    │     │  (ollama-rs)    │    │
│   └─────────────┘     └─────────────┘     └────────┬────────┘    │
│                                                      │             │
│                              ┌───────────────────────┼─────────┐   │
│                              │                       │         │   │
│                              ▼                       ▼         ▼   │
│                      ┌──────────────┐      ┌──────────────┐       │
│                      │  Feedback    │      │  Context     │       │
│                      │  Collector   │      │  Builder     │       │
│                      │  (NEW)       │      │  (existing)  │       │
│                      └──────┬───────┘      └──────┬───────┘       │
│                             │                     │               │
│                             ▼                     ▼               │
│                      ┌─────────────────────────────────────┐      │
│                      │           SQLite DB                  │      │
│                      │  - messages (weighted by feedback)   │      │
│                      │  - embeddings (weighted retrieval)   │      │
│                      │  - feedback_signals (NEW)            │      │
│                      └─────────────────────────────────────┘      │
│                                            │                      │
│                                            ▼                      │
│                      ┌─────────────────────────────────────┐      │
│                      │       Feedback-Aware Retrieval       │      │
│                      │  - BM25 + Vector + Feedback Weight   │      │
│                      │  - Decay: recent > old, good > bad   │      │
│                      └─────────────────────────────────────┘      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Implementation Phases

### Phase 1: Feedback Infrastructure (PRIORITY 1)

**Goal:** Capture **explicit** feedback signals. Implicit feedback is deferred to Phase 2.

**Deliverables:**

| Task | Description | Effort |
|------|-------------|--------|
| `/feedback` command | Accept `good`, `bad`, `correction:<text>` (explicit only) | 2 days |
| Schema migration | Add `feedback_signals` table (schema v10) | 0.5 days |
| RRF boost injection | Apply feedback weight in `content_reciprocal_rank_fusion()` | 1 day |
| `/context` enhancement | Show feedback statistics | 0.5 days |
| LLM feedback tool | `feedback_submit()` with `[feedback].llm_feedback = true` config | 1 day |

> **Note:** Implicit feedback capture (continuation signals, requery detection, session
> abandonment) is deferred to Phase 2. The `[feedback].implicit_capture` config field is
> reserved for forward compatibility but has no effect in Phase 1.

**Implementation:**

```rust
// src/chat/feedback.rs (NEW MODULE)

pub enum FeedbackType {
    Good,           // +1.0 evaluative
    Bad,            // -1.0 evaluative
    Correction(String), // directive signal
}

pub struct FeedbackSignal {
    pub message_id: i64,
    pub signal_type: FeedbackType,
    pub timestamp: DateTime<Utc>,
}

impl FeedbackSignal {
    /// Compute decayed weight based on age
    pub fn decayed_weight(&self, now: DateTime<Utc>) -> f64 {
        let age_days = (now - self.timestamp).num_days() as f64;
        let half_life = match self.signal_type {
            FeedbackType::Good => 30.0,      // Good: 30-day half-life
            FeedbackType::Bad => 7.0,        // Bad: 7-day half-life (forget faster)
            FeedbackType::Correction(_) => 14.0, // Lessons: 14-day half-life
        };
        self.base_value() * 2f64.powf(-age_days / half_life)
    }
    
    fn base_value(&self) -> f64 {
        match self.signal_type {
            FeedbackType::Good => 1.0,
            FeedbackType::Bad => -1.0,  // Symmetric magnitude (ADR-005: Drori 'no partial credit')
            FeedbackType::Correction(_) => 1.0, // Correction value is in metadata, not weight
        }
    }
}
```

**Implicit Feedback Capture:**

```rust
// Detect implicit signals from conversation patterns

pub fn detect_implicit_feedback(session: &ChatSession) -> Vec<ImplicitSignal> {
    let mut signals = Vec::new();
    
    // User continued the conversation -> likely positive
    if session.has_continuation_after_last_assistant_message() {
        signals.push(ImplicitSignal::Continuation { weight: 0.1 });
    }
    
    // User rephrased the same question -> likely negative
    if session.has_requery_within_n_messages(3) {
        signals.push(ImplicitSignal::Requery { weight: -0.3 });
    }
    
    // User abandoned session mid-task -> likely negative
    if session.was_abandoned_during_task() {
        signals.push(ImplicitSignal::Abandonment { weight: -0.5 });
    }
    
    signals
}
```

---

### Phase 2: Feedback-Aware Retrieval (PRIORITY 2)

**Goal:** Use feedback history to improve context retrieval.

**Deliverables:**

| Task | Description | Effort |
|------|-------------|--------|
| Weighted retrieval | Modify hybrid search to consider feedback | 3 days |
| Decay implementation | Implement temporal decay for feedback | 1 day |
| Context composition | Weight retrieved results by feedback | 1 day |

**Implementation:**

```rust
// Modify retrieval to incorporate feedback weight

impl HybridRetriever {
    pub fn retrieve_weighted(
        &self,
        query: &str,
        session_id: &str,
        limit: usize,
        feedback_weights: &FeedbackWeights,
    ) -> Vec<WeightedResult> {
        // 1. Standard hybrid search
        let results = self.hybrid_search(query, limit * 2);
        
        // 2. Apply feedback-based weighting
        let weighted: Vec<_> = results
            .into_iter()
            .map(|r| {
                let feedback_boost = feedback_weights.get(r.message_id);
                let final_score = (r.score * (1.0 + feedback_boost)).clamp(0.1, 3.0);
                WeightedResult { result: r, final_score }
            })
            .collect();
        
        // 3. Re-rank by final score
        let mut weighted = weighted;
        weighted.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());
        
        weighted.into_iter().take(limit).collect()
    }
}
```

---

### Phase 3: Verification Signals (PRIORITY 3)

**Goal:** Capture success/failure signals from tool execution.

**Deliverables:**

| Task | Description | Effort |
|------|-------------|--------|
| Tool outcome tracking | Track successful/failed tool calls | 2 days |
| Skill verification | Per-Skill success rate tracking | 2 days |
| Process waypoints | Track progress in multi-step tasks | 3 days |

**Implementation:**

```rust
// Track tool outcomes as pseudo-feedback

pub struct ToolOutcome {
    pub tool_name: String,
    pub success: bool,
    pub error_type: Option<String>,
    pub execution_time_ms: u64,
}

impl ToolOutcome {
    /// Convert to feedback signal
    pub fn to_feedback(&self) -> Option<FeedbackSignal> {
        if self.success {
            // Successful tool use = small positive signal
            Some(FeedbackSignal {
                signal_type: FeedbackType::Good,
                weight: 0.05, // Small weight for implicit success
            })
        } else {
            // Failed tool use = negative signal
            Some(FeedbackSignal {
                signal_type: FeedbackType::Bad,
                weight: -0.1,
            })
        }
    }
}
```

---

### Phase 4: Skill-Level Learning (PRIORITY 4)

**Goal:** Apply feedback patterns to skill selection and execution.

**Deliverables:**

| Task | Description | Effort |
|------|-------------|--------|
| Skill success tracking | Track which skills work well for user | 2 days |
| Adaptive skill suggestions | Prefer skills with better track record | 2 days |
| User-specific patterns | Learn user's preferred workflow patterns | 3 days |

---

## 4. Key Design Decisions

### 4.1 Why "Pseudo-RL" Instead of Real Fine-Tuning?

**Literature Gap:** OpenClaw-RL and similar frameworks propose true RL with parameter updates. However:

| Constraint | Reality | Solution |
|------------|---------|----------|
| Local-first | No GPU cluster for training | Pseudo-RL: learn in retrieval/prompt space |
| Real-time | Training blocks inference | Async collection, batch application |
| User control | User owns their data | Local feedback DB, optional export |

**Our Approach:**

Instead of updating model weights, we update:
1. **Retrieval weights** - Good interactions rank higher
2. **Context composition** - Successful patterns prioritized
3. **Skill suggestions** - High-success skills recommended first

This achieves "learning" without requiring fine-tuning infrastructure.

### 4.1.1 ADR-001: Harness-Only, No Fine-Tuning (Architecture Decision Record)

**Decision:** The feedback system operates entirely within the retrieval/prompt harness. No model weight updates (LoRA, QLoRA, or full fine-tuning) are performed.

**Rationale:**
- Local-first constraint: users run on consumer hardware without training GPUs.
- Latency: fine-tuning blocks inference; harness adjustments complete in O(1).
- Reproducibility: prompt-level changes are inspectable and auditable by the user.
- The CortexGraph/MemOS literature confirms that memory management (not parameter updates) drives most short-term personalization gains.

**Consequence:** Long-term skill internalization (Tier 2/3) is deferred to a future offline batch pipeline, not the online feedback loop.

### 4.2 Why Separate Evaluative and Directive Signals?

From OpenClaw-RL:

> "Next-state signals encode two forms of information: evaluative signals (how well action performed) and directive signals (how action should have been different)."

**Implementation:**

| Signal Type | Stored As | Used For |
|-------------|-----------|----------|
| Good/Bad (evaluative) | Scalar weight | Retrieval ranking |
| Correction (directive) | Text + context | Pattern learning |

Directive signals are richer - they tell us *how* to improve, not just *whether* to improve. We preserve the correction text for future pattern analysis.

### 4.3 Temporal Decay Strategy

From CortexGraph (Ebbinghaus):

> "Memories naturally decay over time unless reinforced through use."

**Implementation (ADR-002: aligned with existing Facts system):**

Feedback decay uses Ebbinghaus `2^(-t/h)` — the same formula as `facts::compute_retention()` in `src/facts/decay.rs`. This ensures consistency across all memory systems in sprachspiel.

```rust
const MAX_FEEDBACK_BOOST: f32 = 2.0;

/// Compute total feedback boost for a message (applied in RRF fusion)
pub fn compute_feedback_boost(
    signals: &[FeedbackSignal],
    now: DateTime<Utc>,
) -> f32 {
    let total: f32 = signals.iter()
        .map(|s| decayed_weight(s, now))
        .sum();
    total.clamp(-MAX_FEEDBACK_BOOST, MAX_FEEDBACK_BOOST)
}

/// Decay a single signal — Ebbinghaus curve consistent with Facts system
pub fn decayed_weight(signal: &FeedbackSignal, now: DateTime<Utc>) -> f32 {
    let half_life = match signal.signal_type {
        FeedbackType::Good => 30.0,
        FeedbackType::Bad => 7.0,
        FeedbackType::Correction(_) => 14.0,
    };
    
    let age_days = (now - signal.timestamp).num_days() as f32;
    let decay = 2f32.powf(-age_days / half_life);  // Same as facts::compute_retention
    
    // ADR-004: LLM self-feedback weighted at 30% of user feedback
    let source_factor = match signal.source {
        FeedbackSource::User => 1.0,
        FeedbackSource::Llm => 0.3,
    };
    
    signal.base_value() * decay * source_factor
}
```

**Application in RRF fusion (not a retrieval_weight function):**

Feedback boost is applied inside `content_reciprocal_rank_fusion()` (in `src/content/db.rs`), not as a separate `compute_retrieval_weight()` function. The RRF fusion already handles recency through BM25 scoring and vector similarity, so adding a separate `recency_boost` would double-count recency signals.

```rust
// In content_reciprocal_rank_fusion():
let feedback_boost = compute_feedback_boost(feedback_signals_for_item, now);
// ADR-006: clamp prevents negative scores
let multiplier = (1.0 + feedback_boost).clamp(0.1, 3.0);
let final_score = r.score * multiplier;
```

> **Removed:** The original `compute_retrieval_weight()` function with `recency_boost` was
> replaced by direct RRF injection. RRF already encodes recency; a separate boost was redundant.

### 4.3.1 Content Item Decay (NEW — ADR-008/ADR-009)

Content items (messages, notes, documents) now decay using the same Ebbinghaus model as facts:

```rust
// src/content/decay.rs (NEW)
pub const HALF_LIFE_MESSAGE: f32 = 90.0;
pub const HALF_LIFE_NOTE: f32 = 60.0;
pub const HALF_LIFE_DOCUMENT: f32 = 120.0;

pub fn compute_content_retention(
    importance: f32, access_count: u32, content_type: &str,
    last_accessed: DateTime<Utc>, now: DateTime<Utc>
) -> f32 {
    let half_life = match content_type {
        "message" => HALF_LIFE_MESSAGE,
        "note" => HALF_LIFE_NOTE,
        "document" => HALF_LIFE_DOCUMENT,
        _ => HALF_LIFE_MESSAGE,
    };
    let days_since_access = (now - last_accessed).num_days() as f32;
    let decay = 2f32.powf(-days_since_access / half_life);
    let importance_mult = 1.0 + importance * CONTENT_IMPORTANCE_BOOST;
    let access_mult = if access_count > 0 {
        1.0 + CONTENT_ACCESS_BOOST * (access_count as f32).log2().max(0.0)
    } else {
        1.0
    };
    (decay * importance_mult * access_mult).clamp(0.0, 1.0)
}
```

Retrieval reinforces retention (ADR-009):
```rust
/// Called when search_content_hybrid() returns results
pub fn on_content_access(conn: &Connection, item_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE content_items SET access_count = access_count + 1, last_accessed = unixepoch('now') WHERE id = ?1",
        params![item_id],
    )?;
    Ok(())
}
```

### 4.4 ADR-004: LLM Self-Feedback Weight Discount

**Decision:** When the LLM uses the `feedback_submit()` tool to provide feedback on messages (including its own responses), that signal receives **30% of the weight** of an equivalent user-provided explicit feedback signal.

**Rationale:**
- LLM self-verification is consistently beaten by simple majority voting (Wu et al. 2025: up to -16.7% worse than baseline)
- Only ~3% of decisions change per reflection step (Chan et al. 2025), and some correct→incorrect reversions occur
- Recovery rate from self-correction attempts: 2.7-19.5% (Wu et al. 2025) — weak signal
- User signals remain the ground truth; LLM signals are supplementary
- A 30% discount factor (configurable via `llm_feedback_weight = 0.3`) balances signal contribution against noise, consistent with empirical range of 20-40% from both papers

**Implementation:** The `llm_feedback_weight` config field scales all LLM-originated feedback signals via `source_factor` in `decayed_weight()`. The discount is applied at query time, not at insert time — the DB stores the raw signal with `source='llm'`, and the weight factor is applied when computing `compute_feedback_boost()`.

### 4.5 ADR-008: Content Decay Activation

**Decision:** The `content_items` ghost fields (`decay_score`, `access_count`, `last_accessed`) are activated with the same Ebbinghaus model used by the facts system. Create `src/content/decay.rs` with `compute_content_retention()`, `on_content_access()`, `should_prune_content()`, and `run_content_decay_cycle()`.

**Rationale:**
- User's stated goal: "irrelevant things are forgotten faster" — but content items are never forgotten
- `content_items` already have the required fields (inserted as `1.0`, `0`, `created_at`), they just need functional update logic
- The facts system provides the proven model; content items should follow the same pattern
- Content-type-specific half-lives: messages=90d, notes=60d, documents=120d (longer than facts because conversational memory carries nuance)
- Items with `importance >= 0.8` are never pruned (same protection as high-importance preferences)

**Feedback → Importance Adjustment:**
- `/feedback good` → `importance = min(1.0, importance + 0.05)` — slow boost, rewarded content decays slower
- `/feedback bad` → `importance = max(0.0, importance - 0.1)` — faster penalty, bad content decays faster
- `/feedback correction` → no importance change

This creates a **feedback-driven forgetting loop**: bad feedback → lower importance → faster decay → sooner pruning → forgotten faster.

### 4.6 ADR-009: Retrieval Reinforces Retention

**Decision:** When `search_content_hybrid()` returns results, call `on_content_access()` for each returned item. This increments `access_count`, updates `last_accessed`, and applies a tiny importance boost (+0.001 per retrieval).

**Rationale:**
- Same pattern as `on_fact_access()` in facts system — accessed items retain longer
- `access_count` multiplier in `compute_retention()` uses `1.0 + ACCESS_BOOST * log2(access_count)` — already proven in facts
- RRF boost (immediate ranking) and access_count (future retention) operate on different timescales — NOT double-counting
- Performance: N small writes per search (2 columns each) — negligible for SQLite

---

## 5. Metrics for Success

| Metric | Baseline | Target (3 months) | Target (6 months) |
|--------|----------|--------------------|--------------------|
| Feedback commands used | 0 | 30% of sessions | 60% of sessions |
| Implicit signals captured | 0 | 80% of sessions | 95% of sessions |
| Retrieval accuracy (user rating) | N/A | 70% positive | 85% positive |
| Task completion rate | Baseline | +5% | +15% |
| Context relevance correlation | N/A | 0.3 | 0.5 |

---

## 6. Migration Path

### 6.1 No Breaking Changes

- All feedback tables are **additive**
- Existing sessions work unchanged
- New tables are populated lazily

### 6.2 Opt-In Features

```toml
# ~/.config/sprachspiel/config.toml

[feedback]
enabled = true
implicit_capture = true          # Capture implicit signals
llm_feedback_weight = 0.3        # ADR-004: LLM self-feedback discounted to 30% of user weight
decay_half_life_good = 30        # Days
decay_half_life_bad = 7          # Days
decay_half_life_correction = 14  # Days
content_decay = true             # NEW (ADR-008): enable content item decay
access_reinforcement = true      # NEW (ADR-009): enable access tracking on retrieval
access_reinforcement_boost = 0.01  # NEW: importance += this per 10 accesses
content_prune_threshold = 0.05   # NEW: below this retention → prune
```

### 6.3 Data Export

Users can export their feedback history:

```
/export-feedback              # Export to JSON
/export-feedback --format csv # Export to CSV
```

---

## 7. Appendix: Reference Documents

### 7.1 Research Papers

| Paper | File | Status |
|-------|------|--------|
| MemOS: A Memory OS for AI Systems | `__archived__/papers/memos-paper.pdf` | ✅ Downloaded |
| OpenClaw-RL: Train Any Agent Simply by Talking | `__archived__/papers/openclaw-rl-paper.pdf` | ✅ Downloaded |
| MemGPT: Towards LLMs as Operating Systems | `__archived__/papers/memgpt-paper.pdf` | ✅ Downloaded |
| Krishnamurthy et al. 2026 | Referenced | LLM self-assessment noise analysis |
| Drori 2025 | Referenced | Binary feedback: no partial credit for negative signals |
| Wu et al. 2025 | Referenced | LLM confidence calibration and over-estimation |
| Chan et al. 2025 | Referenced | Calibrated self-evaluation in agent systems |

### 7.2 Analysis Documents

| Document | File | Description |
|----------|------|-------------|
| OpenClaw-RL Analysis | `__archived__/openclaw-rl-analysis.md` | Detailed analysis of OpenClaw-RL framework |
| Effective Agents Analysis | `__archived__/effective-agents-analysis.md` | Research on effective agent architectures |
| Context Management Research | `__archived__/context_management_research.md` | Research on context management approaches |
| Skills System Design | `skills-system-design.md` | Skills architecture (maintained - relevant) |

### 7.3 Key Insights Summary

| Source | Key Insight | Application |
|--------|-------------|-------------|
| OpenClaw-RL | Next-state signals are universal | Capture all interaction signals |
| MemOS | Memory hierarchy (plaintext → activation → parameter) | Three-tier architecture |
| MemGPT | Virtual context management | Already implemented (continuation) |
| CortexGraph | Ebbinghaus decay | Temporal weighting of feedback |
| Unsloth/NVIDIA | Environments define intelligence contract | Tool outcomes as verification |
| Krishnamurthy 2026 | LLM self-assessment is noisy | Discount LLM-originated feedback (ADR-004) |
| Drori 2025 | No partial credit for negative signals | Symmetric Good/Bad magnitude (ADR-005) |
| Wu 2025 | LLM confidence systematically over-estimated | Weight user signals above LLM signals |
| Chan 2025 | Calibrated self-evaluation possible with discounting | 0.3 weight factor for LLM self-feedback |

---

## 8. Next Steps for Implementation Agent

When this directive is passed to the coding agent:

1. **Read this document** to understand the unified vision
2. **Review `__archived__/` files** for detailed research context
3. **Create GitHub issues** for each Phase 1 task
4. **Implement in dependency order** (Feedback → Retrieval → Verification → Skills)
5. **Write integration tests** for feedback capture and retrieval weighting
6. **Update documentation** in `doc/src/` for user-facing changes

---

**Document Status:** CANONICAL - This is the definitive implementation direction for the continuous learning feature. All implementation decisions should reference this document.

**Last Updated:** 2026-04-19