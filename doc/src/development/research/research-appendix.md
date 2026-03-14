# Research Appendix: Continuous Learning Agent Synthesis

**Status:** ARCHIVED REFERENCE  
**Created:** 2026-03-13  
**Purpose:** Document the research process that led to the Implementation Directive

---

## Overview

This appendix documents the research process that synthesized insights from multiple papers to create the Implementation Directive. It serves as a reference for understanding the decisions made.

---

## 1. Papers Analyzed

### 1.1 MemOS: A Memory OS for AI Systems (arXiv:2507.03724)

**Authors:** Li, Zhiyu et al. (38 authors)  
**Published:** July 2025  
**Citations:** 600+ (estimated)  
**Status:** Production implementation available (MemTensor/MemOS)

**Core Contribution:**
> "A memory operating system that treats memory as a manageable system resource, unifying plaintext, activation-based, and parameter-level memories."

**Key Concepts:**

| Concept | Description | Relevance to Ask-AI |
|---------|-------------|---------------------|
| MemCube | Basic unit of memory with content + metadata | Similar to messages with embeddings |
| Three-tier memory | Plaintext → Activation → Parameter | Roadmap for future evolution |
| Memory lifecycle | Compose, migrate, fuse | Missing - needs implementation |
| Continual learning | Foundation for learning and personalization | Core goal alignment |

**Direct Quote (MemOS Abstract):**
```
"LLMs face broader challenges arising from how information is distributed 
over time and context, requiring systems capable of managing heterogeneous 
knowledge spanning different temporal scales and sources."
```

---

### 1.2 OpenClaw-RL: Train Any Agent Simply by Talking (arXiv:2603.10165)

**Authors:** Wang, Yinjie et al. (5 authors)  
**Published:** March 2026  
**Citations:** New paper  
**Status:** Research implementation (Gen-Verse/OpenClaw-RL)

**Core Contribution:**
> "Every agent interaction generates a next-state signal... next-state signals are universal, and policy can learn from all of them simultaneously."

**Key Concepts:**

| Concept | Description | Relevance to Ask-AI |
|---------|-------------|---------------------|
| Next-state signals | User reply, tool output, GUI change | All present in Ask-AI |
| Evaluative signals | Scalar rewards (how well) | `/feedback` good/bad |
| Directive signals | Textual hints (how different) | `/feedback correction:` |
| PRM (Process Reward Model) | Learn from evaluative signals | Future: ML-based reward model |
| OPD (On-Policy Distillation) | Learn from directive signals | Future: pattern extraction |
| Asynchronous design | Serve + judge + train in parallel | Not applicable (no training) |

**Critical Distinction:**
OpenClaw-RL proposes **real model fine-tuning**. Ask-AI operates local-only without GPU infrastructure for training. We adapt to "pseudo-RL" - learning in retrieval/prompt space instead.

---

### 1.3 MemGPT: Towards LLMs as Operating Systems (arXiv:2310.08560)

**Authors:** Packer, Charles et al. (7 authors, UC Berkeley)  
**Published:** October 2023  
**Citations:** 1000+  
**Status:** Production (Letta AI, formerly MemGPT)

**Core Contribution:**
> "Virtual context management... MemGPT can create conversational agents that remember, reflect, and evolve dynamically."

**Key Concepts:**

| Concept | Description | Ask-AI Status |
|---------|-------------|---------------|
| Hierarchical memory | Main context ↔ Archive | ✅ Implementado (SQLite + embeddings) |
| Self-editing memory | LLM decides what to save | ⚠️ Auto-save, not LLM-decided |
| Interrupts | System interrupts for memory ops | ✅ ContinuationTag |
| Reflection | Agent reflects on knowledge | ❌ Not implemented |

**Gap:** MemGPT's "self-editing" is more aggressive than our proposal. We use explicit feedback rather than LLM self-direction.

---

### 1.4 Unsloth/NVIDIA Blog: RL Environments (March 2026)

**Authors:** Daniel Han, Michael Han (Unsloth); Shashank Verma et al. (NVIDIA)  
**Source:** https://unsloth.ai/blog/rl-environments

**Core Contribution:**
> "The environment defines the contract for intelligence... environments generate rollouts... verification determines success/failure."

**Key Concepts:**

| Concept | Description | Relevance to Ask-AI |
|---------|-------------|---------------------|
| Environment contract | State + Actions + Transition + Reward | Chat session is environment |
| Static vs Dynamic environments | Fixed state vs changing state | Dynamic (chat) |
| Verification | Deterministic success/failure check | Tool outcomes = verification |
| Rollouts | Complete interaction trajectories | Chat history = rollout |
| NeMo Gym | Framework for building RL environments | Architecture reference |

**Application:** Chat session with tools IS an RL environment. Every tool call success/failure is a verification signal.

---

### 1.5 CortexGraph: Temporal Memory for AI (GitHub: prefrontal-systems/cortexgraph)

**Authors:** Prefrontal Systems  
**Status:** Research artifact (PoC)  
**Stars:** 29 (niche project)

**Core Contribution:**
> "Memories naturally decay over time unless reinforced through use... Ebbinghaus forgetting curve."

**Key Concepts:**

| Concept | Description | Application |
|---------|-------------|-------------|
| Temporal decay | Memories fade exponentially | Weight old反馈 less |
| Reinforcement through use | Accessed memories strengthen | Retrieval boost |
| JSONL + Markdown storage | Human-readable formats | SQLite already used |
| Knowledge graphs | Structured relationships | Future: entity linking |

**Implementation:**
```rust
pub fn ebbinghaus_decay(age_days: f64, strength: f64) -> f64 {
    strength * (-age_days / HALF_LIFE).exp()
}
```

---

## 2. Cross-Paper Analysis

### 2.1 Converging Themes

All papers converge on these themes:

1. **Memory is a managed resource, not passive storage**
   - MemOS: "memory operating system"
   - MemGPT: "virtual context management"
   - CortexGraph: "temporal decay"

2. **Interactions contain learning signals**
   - OpenClaw-RL: "next-state signals are universal"
   - Unsloth: "verification determines success"
   - MemGPT: "reflect and evolve"

3. **Hierarchies improve efficiency**
   - MemOS: plaintext → activation → parameter
   - MemGPT: main context ↔ archive
   - All: Different levels for different time scales

### 2.2 Diverging Approaches

| Aspect | OpenClaw-RL | MemOS | MemGPT | Ask-AI Proposal |
|--------|-------------|-------|--------|-----------------|
| Learning method | Fine-tuning | Memory migration | Self-editing | Pseudo-RL (weight-based) |
| Feedback source | Explicit + next-state | Implicit (usage) | Self-directed | Explicit + implicit |
| Infrastructure | GPU cluster | Production system | Cloud + local | Local-first |
| Real-time training | Yes (async) | No | No | No |

**Our Innovation:** Combining OpenClaw-RL's signal taxonomy with MemOS's memory hierarchy in a local-first context.

---

## 3. Alternative Memory Systems Analyzed

### 3.1 mem0ai/mem0 (49K stars)

**Characteristics:**
- Universal memory layer
- Vector search + session management
- No implicit decay

**Gap:** No feedback weighting, no directive signals.

### 3.2 letta-ai/letta (MemGPT successor, 21K stars)

**Characteristics:**
- Stateful agents
- Advanced memory management
- Self-modifying behavior

**Gap:** Cloud-focused, no local-first priority.

### 3.3 MemoriLabs/Memori (12K stars)

**Characteristics:**
- SQL Native memory
- Long/short-term separation
- Decay mechanism

**Gap:** No feedback signal differentiation, no learning from corrections.

---

## 4. Research Methodology

### 4.1 Search Queries

Papers found via:
1. arXiv search: `memory agent learning RL`
2. GitHub topics: `memory-agent`, `llm-memory`, `agent-memory`
3. Unsloth blog: RL environments post

### 4.2 Selection Criteria

| Criterion | Weight |
|-----------|--------|
| Peer-reviewed (arXiv/venue) | 30% |
| Implementation available | 25% |
| Citations/adoption | 20% |
| Recency (2025+) | 15% |
| Relevance to local-first | 10% |

### 4.3 Papers Considered but Rejected

| Paper | Reason for Rejection |
|-------|---------------------|
| PolyNet (arXiv:2402.14048) | Neural CO, not memory |
| Neuroscience Network papers | Not LLM-specific |
| Various survey papers | Not novel contribution |

---

## 5. Validation Against Existing Ask-AI Architecture

### 5.1 What Ask-AI Already Has

| Feature | Ask-AI | MemOS | MemGPT | OpenClaw |
|---------|--------|-------|--------|----------|
| Persistent chat history | ✅ SQLite | ✅ | ✅ | N/A |
| Semantic search | ✅ BM25 + vector | ✅ | ✅ | N/A |
| Context management | ✅ Compaction | ✅ | ✅ | N/A |
| Overflow handling | ✅ ContinuationTag | ⚠️ | ✅ | N/A |
| Modular architecture | ✅ Separate modules | ✅ | ✅ | ✅ |

### 5.2 What Ask-AI Lacks

| Feature | Priority | Implementation Path |
|---------|----------|---------------------|
| Explicit feedback capture | P1 | `/feedback` command |
| Weighted retrieval | P2 | Feedback-weighted search |
| Tool outcome tracking | P3 | Success/failure signals |
| Implicit signal capture | P2 | Continuous/abandon patterns |
| Temporal decay | P2 | Ebbinghaus formula |

---

## 6. Key Decisions Made

### Decision 1: Pseudo-RL Instead of Real Fine-Tuning

**Rationale:** Local-first operation means no GPU infrastructure for gradient updates. Pseudo-RL (weight-based learning in retrieval space) provides benefit without infrastructure cost.

**Trade-off:** Slower "learning" (not updating model) but zero compute overhead.

### Decision 2: Separate Evaluative and Directive Signals

**Rationale:** OpenClaw-RL demonstrates these have different information content. Evaluative tells you "how much" to adjust, directive tells you "in what direction."

**Implementation:** `good/bad` for evaluative, `correction:text` for directive.

### Decision 3: Three-Tier Memory (Future-Proofed)

**Rationale:** MemOS demonstrates that full memory systems evolve through tiers. Ask-AI starts at Tier 1 (plaintext), architecture supports future Tiers 2-3.

**Future:** LoRA adapters for Tier 3 (parameter memory) when local GPU available.

### Decision 4: Temporal Decay with Reinforcement

**Rationale:** CortexGraph and Ebbinghaus research shows un-reinforced memories fade. Implementation uses exponential decay with reinforcement for accessed items.

**Formula:** `weight * exp(-age / half_life) * (1 + access_boost)`

---

## 7. Implementation Risks

| Risk | Mitigation |
|------|------------|
| User doesn't use `/feedback` | Capture implicit signals automatically |
| Feedback bias (only negative) | Default positive bias for continued sessions |
| Data accumulation | Decay + pruning old low-weight items |
| Performance overhead | Async capture, batch processing |
| Privacy concerns | Local-only storage, export controls |

---

## 8. Files in This Archive

```
__archived__/
├── papers/
│   ├── memos-paper.pdf           # MemOS paper (6.8MB)
│   ├── openclaw-rl-paper.pdf      # OpenClaw-RL paper (1.4MB)
│   └── memgpt-paper.pdf          # MemGPT paper (663KB)
├── openclaw-rl-analysis.md       # Original OpenClaw-RL analysis
├── effective-agents-analysis.md  # Effective agents research
├── context_management_research.md # Context management approaches
└── research-appendix.md          # This file
```

---

## 9. References

1. Li, Zhiyu et al. "MemOS: A Memory OS for AI System." arXiv:2507.03724, 2025.
2. Wang, Yinjie et al. "OpenClaw-RL: Train Any Agent Simply by Talking." arXiv:2603.10165, 2026.
3. Packer, Charles et al. "MemGPT: Towards LLMs as Operating Systems." arXiv:2310.08560, 2023.
4. Han, Daniel et al. "Reinforcement Learning environments and how to build them." Unsloth Blog, March 2026.
5. Prefrontal Systems. "CortexGraph: Temporal Memory for AI." GitHub, 2025.

---

**Document Status:** ARCHIVED - This is a reference document. For implementation guidance, see `implementation-directive.md`.