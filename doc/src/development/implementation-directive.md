# Implementation Directive: Continuous Learning Agent

**Status:** CANONICAL  
**Version:** 1.0  
**Created:** 2026-03-13  
**Sources:** OpenClaw-RL, MemOS, MemGPT, Unsloth/NVIDIA RL Environments

---

## Executive Summary

This document defines the **definitive implementation direction** for Ask-AI to become a **continuous learning personal agent**. It synthesizes insights from:

- **OpenClaw-RL** (arXiv:2603.10165) - Learning from next-state signals
- **MemOS** (arXiv:2507.03724) - Memory as manageable system resource  
- **MemGPT** (arXiv:2310.08560) - Virtual context management
- **Unsloth/NVIDIA** - RL environments for agent training

The core insight: **Every user interaction contains learning signals that are currently discarded.** By capturing, structuring, and utilizing these signals, Ask-AI can evolve from a stateless chat interface to a learning agent that improves with use.

---

## 1. The Unified Vision

### 1.1 Problem Statement

Current Ask-AI:
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

| Aspect | Traditional Chatbot | Ask-AI (Proposed) |
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
│                        ASK-AI UNIFIED ARCHITECTURE                  │
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
-- Current: messages + embeddings
-- Add: feedback_signals

CREATE TABLE feedback_signals (
    id INTEGER PRIMARY KEY,
    message_id INTEGER NOT NULL,          -- FK to messages
    session_id TEXT NOT NULL,             -- Session context
    signal_type TEXT NOT NULL,            -- 'good', 'bad', 'correction'
    signal_value REAL,                    -- +1.0, -1.0, or custom
    correction_text TEXT,                 -- For directive signals
    collected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    -- Temporal decay factor (computed at query time)
    -- decayed_weight = signal_value * exp(-age_days / half_life)
    
    FOREIGN KEY (message_id) REFERENCES messages(id)
);

-- Add weighting to messages for retrieval
ALTER TABLE messages ADD COLUMN feedback_weight REAL DEFAULT 1.0;
ALTER TABLE messages ADD COLUMN retrieval_boost REAL DEFAULT 0.0;
```

### 2.3 Component Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                         ASK-AI COMPONENTS                          │
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

**Goal:** Capture explicit and implicit feedback signals.

**Deliverables:**

| Task | Description | Effort |
|------|-------------|--------|
| `/feedback` command | Accept `good`, `bad`, `correction:<text>` | 2 days |
| Schema migration | Add `feedback_signals` table | 0.5 days |
| Weight propagation | Update `messages.feedback_weight` | 1 day |
| `/context` enhancement | Show feedback statistics | 0.5 days |

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
        self.base_value() * (-age_days / half_life).exp()
    }
    
    fn base_value(&self) -> f64 {
        match self.signal_type {
            FeedbackType::Good => 1.0,
            FeedbackType::Bad => -0.5,  // Negative but smaller magnitude
            FeedbackType::Correction(_) => 1.2, // Slightly higher for explicit correction
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
                let final_score = r.score * (1.0 + feedback_boost);
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

**Implementation:**

```rust
pub fn compute_retrieval_weight(
    base_weight: f64,
    feedback_signals: &[FeedbackSignal],
    last_accessed: DateTime<Utc>,
    now: DateTime<Utc>,
) -> f64 {
    let feedback_boost: f64 = feedback_signals
        .iter()
        .map(|s| s.decayed_weight(now))
        .sum();
    
    let recency_boost = {
        let age_hours = (now - last_accessed).num_hours() as f64;
        (-age_hours / 168.0).exp() // 1-week half-life for recency
    };
    
    base_weight * (1.0 + feedback_boost.max(0.0)) * (1.0 + recency_boost * 0.5)
}
```

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
# ~/.config/ask-ai/config.toml

[feedback]
enabled = true
implicit_capture = true          # Capture implicit signals
decay_half_life_good = 30        # Days
decay_half_life_bad = 7          # Days
decay_half_life_correction = 14  # Days
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

**Last Updated:** 2026-03-13