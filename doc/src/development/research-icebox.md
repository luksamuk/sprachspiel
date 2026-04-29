# Research Icebox

> Ideas, competitive insights, and deferred topics that are not active priorities but should not be forgotten. Items here are candidates for future milestones when prerequisites are met or when research validates them.
>
> Items are organized into three categories:
>
> - **Refinement Topics (R-XX):** Technical ideas that may become priorities after their prerequisites are met.
> - **Competitive Research (C-XX):** External systems, papers, and patterns worth remembering.
> - **Decision Records (Why NOT):** Explicitly rejected items with reasons, so we don't re-evaluate them without new information.

## Refinement Topics

### R-01: Multi-Stage Compression Pipeline

- **Source:** Context management research (Recurrent Context Compression, arXiv:2406.06110)
- **Current state:** Single-stage compaction with structured template (works well in practice)
- **Why deferred:** No data showing multi-stage compression provides significant improvement over our current approach. Complexity cost is high (5 levels vs 1).
- **Prerequisite:** Collect real compaction data (summary sizes, compression ratios, user satisfaction) to justify additional stages
- **Revisit when:** Context compression becomes a measured bottleneck; users report quality degradation after compaction

---

### R-02: Exact Token Counting (tiktoken or equivalent)

- **Source:** Context management research; Issue #103
- **Current state:** Word-based estimation (~15-20% error); approaching via P6.0d (`prompt_eval_count` from Ollama API) and B4.1 (anchor-based estimation)
- **Why deferred:** tiktoken adds a Rust dependency for a problem we're solving with simpler approaches. `prompt_eval_count` (from P6.0d) gives exact counts post-hoc, and anchor-based estimation (B4.1) improves pre-flight estimates without a tokenizer.
- **Prerequisite:** B4.1 + P6.0d implemented; evaluate if remaining estimation gap justifies adding tiktoken
- **Decision path:** tiktoken only if (1) anchor-based estimation proves insufficient AND (2) `prompt_eval_count` is unavailable from the provider

---

### R-03: Speculative Execution

- **Source:** arXiv:2510.04371 (Speculative Actions, ICLR 2026, DeepMind/Stanford)
- **Current state:** No pattern detection or predictive pre-execution
- **Why deferred:** Without collected usage patterns, speculative execution is guesswork. No telemetry data to validate which patterns are worth pre-executing.
- **Prerequisite:** Telemetry system collecting tool usage patterns; evidence of repeated sequential patterns
- **Revisit when:** We have usage data showing repetitive tool sequences (e.g., read-then-edit > 60% of multi-tool calls)

---

### R-04: Attention-Based Prompt Optimization

- **Source:** arXiv:2603.20578 (Context Cartography, UC Berkeley)
- **Current state:** Static prompt composition (system → facts → retrieved → recent)
- **Why deferred:** Requires attention weight data from models, which local Ollama models don't easily expose. "Lost in the Middle" research already informs our middle-compaction approach.
- **Prerequisite:** Access to model attention weights; tool for measuring attention distribution across prompt sections
- **Revisit when:** We can extract attention maps from the models we support, or when Ollama exposes attention APIs

---

### R-05: LLM Adjudication for Edge Cases (Belief System)

- **Source:** Belief system design research; Gokul et al. 2025 (arXiv:2504.00180)
- **Current state:** Heuristic triple-based dedup covers ~85% of contradiction cases. Accumulative predicates without word overlap ("likes vim" vs "likes emacs") fall through to coexist (not flagged as contradiction).
- **Why deferred:** LLM adjudication is expensive (200ms-5s), unreliable (misses >30% per Gokul et al.), and our heuristic approach covers the common cases well. The 15% edge cases are qualitatively minor.
- **Prerequisite:** B2 (Belief Engine) implemented and in production; measurable data on edge-case failures
- **Revisit when:** Edge cases cause real user-visible problems; a lightweight verifier model (<100ms) becomes available

---

### R-06: Rule Engine for Belief System (Prolog/Datalog)

- **Source:** Belief system design research
- **Candidates evaluated:**
  - **Crepe** (Rust Datalog procedural macro) — best candidate: native Rust, MIT license, compile-time verification, μs-level latency
  - **Scryer Prolog** (Rust ISO Prolog) — too heavy as dependency, evolving API
  - **Tulisp** (Rust Emacs-Lisp interpreter) — viable but overkill for current scope
- **Current state:** Pattern matching on `TRIPLE_*_PREFIXES` in `lang.rs` with exclusivity/polarity/accumulative classification in `conflict.rs`
- **Why deferred:** Current prefix-based approach handles ~85% of cases in <1ms. Adding a rule engine is O(1) expressiveness gain for O(dependency) complexity cost. Not justified until edge cases (R-05) become a real problem.
- **Prerequisite:** B2 (Belief Engine) implemented; R-05 evaluated; evidence that pattern matching is insufficient
- **Revisit when:** We have 3+ categories of belief rules that can't be expressed as prefix patterns AND heuristic classification accuracy drops below 90%

---

### R-07: Non-Destructive Context Collapse

- **Source:** Context management research; MemGPT architecture (arXiv:2310.08560)
- **Current state:** Compaction is destructive — original messages are replaced with a summary. Context archival is not recovering past details.
- **Why deferred:** Adds schema complexity (archive layer, recovery index) and storage cost. Current hybrid search + `/search` command already provides retrieval of past content.
- **Prerequisite:** B4.1 (improved token estimation) to size archive correctly; schema migration for archive tables
- **Revisit when:** Users report losing important context after compaction that `/search` cannot recover

---

### R-08: Importance-Based Eviction Strategy

- **Source:** arXiv:2601.06007v2 (Don't Break the Cache, MIT CSAIL)
- **Current state:** Simple middle-compaction (preserve first N + last N, summarize middle). Content decay and feedback adjust importance, but eviction during compaction doesn't use importance scores.
- **Why deferred:** Need to understand how `importance_score` and `feedback_score` interact with real eviction decisions. Current compaction works; changing the strategy risks regressions.
- **Prerequisite:** Production data on importance score distribution; understanding of how feedback-weighted retrieval interacts with importance-weighted eviction
- **Revisit when:** We have data showing compaction loses high-importance content; feedback system (P5) has been in production long enough to accumulate representative scores

---

### R-09: MCP Server (Memory as Service)

- **Source:** Roadmap gap analysis; competitive landscape
- **Current state:** Not started. P15 covers MCP Client only (consuming external MCP servers).
- **Why deferred:** Building an MCP server is a different product direction (API/library vs CLI tool). Needs careful design around what to expose (facts? content? search? decay?) and authentication model.
- **Prerequisite:** P15 (MCP Client) implemented; understanding of which memory primitives other tools would consume
- **Revisit when:** MCP Client is stable; at least 2 external tools express interest in consuming ask-ai's memory

---

### R-10: Multi-Source Belief Reconciliation

- **Source:** Belief system design research (Phase 3)
- **Current state:** Facts are scoped per-project or global. Content items (messages, notes, docs) have no cross-source contradiction detection.
- **Why deferred:** Requires B2 (Belief Engine) to be stable first. Multi-source reconciliation (user says X, docs say Y) is a fundamentally harder problem that needs clear UX for conflict surfacing.
- **Prerequisite:** B2 (Belief Engine) implemented and proven for single-source contradictions
- **Revisit when:** B2 is production-stable and users report cross-source contradictions that the current system misses

---

## Competitive Research

### C-01: YantrikDB (Rust, Embedded Graph + Vec + Temporal)

- **Language:** Rust
- **Approach:** Embedded graph database with vector search and temporal queries
- **Relevance:** CRITICAL — almost identical architecture to ask-ai (SQLite + sqlite-vec + temporal queries)
- **Key difference:** AGPL-3.0 license; requires ML runtime for embeddings
- **Key feature:** Built-in `scan_conflicts()` and `resolve_conflict()` — similar to our `conflict.rs` but graph-based
- **Lesson:** Our approach (heuristic triples + FTS5 + semantic) avoids the AGPL problem and runs without additional ML runtime, but YantrikDB's graph traversal could inform future S2.2 (Content Relations) design
- **Reference:** Project tracked for architectural comparison

---

### C-02: Kumiho (AGM Belief Revision on Property Graphs)

- **Language:** Python (research paper, March 2026)
- **Approach:** Formal AGM belief revision postulates (K*2-K*6) on property graphs
- **Key pattern adopted:** Immutable revisions + mutable tag pointers
- **Influence on ask-ai:** B2.4 (Belief versioning) will use `invalidated_at` timestamp instead of delete, inspired by Kumiho's immutable revision pattern
- **Reference:** arXiv:2603.10165

---

### C-03: Crepe (Rust Datalog Procedural Macro)

- **Language:** Rust
- **Approach:** Datalog as Rust procedural macros — compile-time verification
- **Relevance:** GREEN — Best candidate for declarative rule engine if R-06 becomes active
- **License:** MIT
- **Performance:** μs-level, no runtime dependency
- **Why not now:** Current prefix matching is sufficient; Crepe integration would be the natural choice if rule complexity grows

---

### C-04: Mem0 (Python, Memory for AI Agents)

- **Language:** Python
- **Approach:** LLM extraction + vector + graph
- **Key lesson:** Their v2 is ADD-only (no UPDATE path — issue #4904), which validates our dedup pipeline design. Their failure to handle contradictions is instructive.
- **Relevance:** Validates the problem space; their architecture failures confirm our design decisions

---

### C-05: Zep/Graphiti (Python/TypeScript, Temporal Knowledge Graph)

- **Language:** Python/TypeScript
- **Approach:** LLM extraction + temporal knowledge graph
- **Relevance:** Medium — temporal anchoring is interesting for future S2.2 (Content Relations) and B2 (Belief versioning)
- **Lesson:** Their time-anchored approach could inform how we timestamp beliefs and content relationships
- **Reference:** Tracked for temporal model design patterns

---

### C-06: Letta/MemGPT (Python, LLM Self-Editing Memory)

- **Language:** Python
- **Approach:** LLM manages text blocks with core/archive partitioning
- **Relevance:** LOW — validates that LLM-based memory management is unreliable for contradiction detection
- **Key lesson:** Their approach misses >30% of contradictions (Gokul et al. 2025), justifying our heuristic-first approach in `conflict.rs`
- **License:** AGPL-3.0

---

### C-07: Context Cartography Research (arXiv:2603.20578)

- **Institution:** UC Berkeley AI Research
- **Key finding:** LLMs have predictable attention patterns — beginning and end of context get more attention than middle
- **Relevance to ask-ai:** Validates our "Lost in the Middle" mitigation strategy (middle compaction preserves first N + last N)
- **Revisit for:** R-04 (attention-based prompt optimization) — if we can measure attention distributions

---

### C-08: Recurrent Context Compression (arXiv:2406.06110)

- **Institution:** Stanford NLP
- **Key finding:** Learned compression outperforms static windowing in maintaining key facts
- **Relevance to ask-ai:** Our structured summary template (Goal/Instructions/Progress/Discoveries/Files) is a manual version of learned compression
- **Revisit for:** R-01 (multi-stage pipeline) — if our compaction quality degrades

---

### C-09: Speculative Actions (arXiv:2510.04371)

- **Institution:** DeepMind/Stanford (ICLR 2026)
- **Key finding:** Pre-executing likely next actions reduces latency by ~40%
- **Relevance to ask-ai:** Pattern detection could pre-compact when approaching threshold, pre-embed after messages
- **Revisit for:** R-03 (speculative execution) — requires usage data

---

### C-10: FadeMem (arXiv:2601.18642)

- **Key finding:** Dual-layer Ebbinghaus decay with different half-lives for different memory categories
- **Relevance to ask-ai:** Confirms our design (messages 90d, notes 60d, documents 120d, facts 30d/180d). Our feedback-driven variant goes beyond FadeMem.
- **Use in:** B1.2 (custom feedback-decay benchmark) — FadeMem comparison as baseline

---

## Decision Records (Why NOT)

### D-01: Majestic Lisp as Rule Engine / MCP Server

- **Decision:** Explicitly rejected (2026-04-28)
- **Reasons:**
  1. Circular dependency risk (ask-ai depends on majestic-lisp which might depend on ask-ai)
  2. Performance overhead for what's currently <1ms pattern matching
  3. Scope creep — adding a full language runtime for 15% edge case coverage
- **Alternative path:** R-06 (Crepe/Datalog) is the natural evolution if rules become complex
- **Revisit only if:** Crepe also proves insufficient AND a separate rule engine becomes a hard requirement

---

### D-02: tiktoken as Primary Token Counter

- **Decision:** Deferred; not rejected but lowest priority approach (2026-04-28)
- **Preferred solutions, in order:**
  1. `prompt_eval_count` from Ollama API (P6.0d) — exact, no dependency
  2. Anchor-based estimation (B4.1) — good enough for pre-flight checks, no dependency
  3. tiktoken — only if both above fail to close the estimation gap
- **Reason:** Minimizing dependencies is a project priority; tiktoken adds a heavy Rust crate for marginal improvement

---

### D-03: LLM Adjudication as Primary Dedup Method

- **Decision:** Rejected as primary method (validated by Gokul et al. 2025)
- **Data:** LLMs miss >30% of contradictions; heuristic approach covers ~85% with <1ms latency
- **Allowed as:** Supplement for edge cases (R-05) — if and when edge cases become a real problem
- **Revisit only if:** A lightweight verifier model (<100ms, >95% accuracy) becomes available locally

---

### D-04: Full Multi-Stage Compression Pipeline

- **Decision:** Deferred indefinitely (our single-stage with structured template works well)
- **Reason:** Complexity cost (5 compression levels, each needing tuning) doesn't justify the gain without data showing quality problems
- **Revisit only if:** User data shows compaction quality is a top-3 complaint

---

### D-05: Speculative Execution

- **Decision:** Deferred — no usage data to validate pattern detection
- **Reason:** Without collected tool usage patterns, speculative execution is guesswork. The 40% latency reduction claim requires repetitive sequential patterns we haven't measured.
- **Revisit only if:** Telemetry shows repetitive tool sequences in >60% of multi-tool calls