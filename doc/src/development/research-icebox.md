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

### R-09: MCP Server (Memory as Service) — See B5 and B8

- **Source:** Roadmap gap analysis; competitive landscape
- **Current state:** Draft priority B5 (MCP Server) and B8 (ACP Agent Integration) in IMPLEMENTATION.md. B8 subsumes B5's use case via MCP-over-ACP.
- **Why deferred to M4:** B8 (ACP) exposes sprachspiel as a complete agent to editors, which is a more valuable integration than exposing individual memory tools. Implementing B8 first eliminates the need for a standalone MCP server.
- **Prerequisite:** P14 TUI (decoupling via ApplicationBackend trait), then B8 (ACP adapter)
- **Revisit when:** TUI is implemented with ApplicationBackend decoupling; evaluate based on user demand for standalone memory API vs full agent experience

---

---

### R-10: Multi-Source Belief Reconciliation

- **Source:** Belief system design research (Phase 3)
- **Current state:** Facts are scoped per-project or global. Content items (messages, notes, docs) have no cross-source contradiction detection.
- **Why deferred:** Requires B2 (Belief Engine) to be stable first. Multi-source reconciliation (user says X, docs say Y) is a fundamentally harder problem that needs clear UX for conflict surfacing.
- **Prerequisite:** B2 (Belief Engine) implemented and proven for single-source contradictions
- **Revisit when:** B2 is production-stable and users report cross-source contradictions that the current system misses

---

### R-11: ACP (Agent Client Protocol) Integration

- **Source:** OpenCode ACP support, Zed ACP integration, ACP specification (agentclientprotocol.com)
- **Current state:** Draft priority B8 in IMPLEMENTATION.md. ACP is the emerging standard for editor↔agent communication (like LSP for language servers).
- **Why M3/M4:** Requires TUI decoupling (P14 ApplicationBackend trait). ACP exposes sprachspiel as a complete agent — sessions, memory, tools — rather than individual MCP tools.
- **Key insight:** ACP replaces MCP Server (B5) as the primary integration path. MCP-over-ACP provides tool-level access when needed. 30+ ACP agents and 20+ ACP clients (editors) already exist.
- **Prerequisite:** P14 TUI with ApplicationBackend decoupling (B8.1, B8.2)
- **Revisit when:** ACP v1.0 SDK (SACP) stabilizes; Zed/JetBrains ACP support matures
- **Reference:** https://agentclientprotocol.com/, https://opencode.ai/docs/acp/

---

### R-12: ApplicationBackend Decoupling (TUI/ACP Prerequisite)

- **Source:** P14 (TUI) and B8 (ACP) architectural requirement
- **Current state:** `InputBackend` and `ChatView` traits exist but are thin abstractions (not yet an ApplicationBackend). `ChatCore` and `repl.rs` are tightly coupled.
- **Why important:** Both TUI and ACP need the same decoupling. Without it, each new I/O backend requires duplicating REPL logic. With it, CLI/TUI/ACP share the same core through `ApplicationBackend` trait.
- **Target architecture:** `ApplicationBackend` trait with event stream (`send_message() → EventStream`), session management, and cancel support — used by CLI, TUI, and ACP backends
- **Revisit when:** P14 (TUI) implementation starts — this must be the first architectural step

---

### R-13: Behavioral Embeddings (Conversation Mode Vectors)

- **Source:** Meta-cognition brainstorm (~/meta-cognition-brainstorm.md §4.1)
- **Current state:** Layer 2 telemetry (#100) uses heuristics (pronoun count, keyword detection) for shift detection
- **Why deferred:** No calibration data from real conversations. Heuristic detection in Layer 2 needs to run first and produce the training data that embeddings would replace.
- **Prerequisite:** Layer 2 (#100) producing shift detection data from 20-30+ real conversations
- **Revisit when:** Heuristic shift detection proves too noisy (>30% false positives) or misses shifts that users flag

---

### R-14: Meta-cognition as Active Tool (meta_cognize)

- **Source:** Meta-cognition brainstorm (~/meta-cognition-brainstorm.md §4.2)
- **Current state:** Board draft "meta_cognize() Active Behavioral Tool [M3]" — LLM-callable tool returning behavioral state (mode, shift detection, suggestions)
- **Why deferred to M3:** Depends on Layer 2 (#100) telemetry producing structured data. Complementary to passive telemetry — makes reflection explicit and traceable.
- **Prerequisite:** #100 (Behavioral Telemetry) producing shift data
- **Revisit when:** Layer 2 is in production and producing reliable shift signals

---

### R-15: Behavioral Conflict Detection

- **Source:** Meta-cognition brainstorm (~/meta-cognition-brainstorm.md §4.3)
- **Current state:** Board draft "Behavioral Conflict Detection [M3]" — detect tensions between SOUL.md (configured personality) and emergent behavior
- **Why deferred to M3:** Depends on #77/#78 (Visualize Connections / Relations Graph) for the structural foundation to represent personality-behavior tensions as a graph problem.
- **Prerequisite:** #77 and #78 implemented with relation extraction working
- **Revisit when:** Relations graph is functional and can represent personality vs behavior edges

---

### R-16: Distributive Meta-cognition (Multi-Personality Evaluation)

- **Source:** Meta-cognition brainstorm (~/meta-cognition-brainstorm.md §4.4)
- **Current state:** Not on board — too speculative even for a draft
- **Why deferred:** Requires SOUL.md multi-personality support (multiple configurable personas evaluating each other). Currently sprachspiel has a single personality.
- **Prerequisite:** Extended Personalities System (#49) implemented; Layer 3 (#101) producing reflections that can be compared across personalities
- **Revisit when:** Multi-personality is mature enough that multiple evaluations of the same response are feasible

---

### R-17: RAPTOR-like Hierarchical Retrieval

- **Source:** RAG improvement research (~/RAG-IMPROVEMENT-ROADMAP.md §5); Sarthi et al. 2024 (arXiv:2401.18059)
- **Current state:** Flat retrieval (BM25 + cosine + RRF). No hierarchical summarization.
- **Why deferred:** Requires Metadata Enrichment, HyDE-like pairing, and Semantic Dedup (board drafts) to be stabilized first. RAPTOR adds a summarization layer on top of functioning chunk-level retrieval.
- **Prerequisite:** Context-Aware Chunking, Metadata Enrichment, Semantic Dedup all in production
- **Revisit when:** Flat retrieval with attention priming and metadata boosting still shows quality gaps at scale (>10k chunks)

---

### R-18: Privacy Filter as Rust-Native Classifier

- **Source:** Privacy filter proposal (~/privacy-filter-integration-proposal.md §10)
- **Current state:** Board draft "Privacy Filter Integration [M3]" uses Python sidecar (only viable path currently)
- **Why deferred:** ONNX runtime (`ort` + `tokenizers`) would add ~30MB binary + heavy deps, violating single-binary/Termux philosophy. A Rust-native Viterbi classifier without ONNX deps is theoretically possible but requires significant implementation effort.
- **Prerequisite:** Python sidecar proves value in production; enough usage data to justify Rust-native investment
- **Decision path:** (1) Python sidecar in M3 → (2) evaluate usage patterns → (3) if PII redaction is critical path, invest in Rust-native Viterbi
- **Revisit when:** Sidecar has 3+ months of production data showing consistent usage

---

## Competitive Research

### C-01: YantrikDB (Rust, Embedded Graph + Vec + Temporal)

- **Language:** Rust
- **Approach:** Embedded graph database with vector search and temporal queries
- **Relevance:** CRITICAL — almost identical architecture to sprachspiel (SQLite + sqlite-vec + temporal queries)
- **Key difference:** AGPL-3.0 license; requires ML runtime for embeddings
- **Key feature:** Built-in `scan_conflicts()` and `resolve_conflict()` — similar to our `conflict.rs` but graph-based
- **Lesson:** Our approach (heuristic triples + FTS5 + semantic) avoids the AGPL problem and runs without additional ML runtime, but YantrikDB's graph traversal could inform future S2.2 (Content Relations) design
- **Reference:** Project tracked for architectural comparison

---

### C-02: Kumiho (AGM Belief Revision on Property Graphs)

- **Language:** Python (research paper, March 2026)
- **Approach:** Formal AGM belief revision postulates (K*2-K*6) on property graphs
- **Key pattern adopted:** Immutable revisions + mutable tag pointers
- **Influence on sprachspiel:** B2.4 (Belief versioning) will use `invalidated_at` timestamp instead of delete, inspired by Kumiho's immutable revision pattern
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
- **Relevance to sprachspiel:** Validates our "Lost in the Middle" mitigation strategy (middle compaction preserves first N + last N)
- **Revisit for:** R-04 (attention-based prompt optimization) — if we can measure attention distributions

---

### C-08: Recurrent Context Compression (arXiv:2406.06110)

- **Institution:** Stanford NLP
- **Key finding:** Learned compression outperforms static windowing in maintaining key facts
- **Relevance to sprachspiel:** Our structured summary template (Goal/Instructions/Progress/Discoveries/Files) is a manual version of learned compression
- **Revisit for:** R-01 (multi-stage pipeline) — if our compaction quality degrades

---

### C-09: Speculative Actions (arXiv:2510.04371)

- **Institution:** DeepMind/Stanford (ICLR 2026)
- **Key finding:** Pre-executing likely next actions reduces latency by ~40%
- **Relevance to sprachspiel:** Pattern detection could pre-compact when approaching threshold, pre-embed after messages
- **Revisit for:** R-03 (speculative execution) — requires usage data

---

### C-10: FadeMem (arXiv:2601.18642)

- **Key finding:** Dual-layer Ebbinghaus decay with different half-lives for different memory categories
- **Relevance to sprachspiel:** Confirms our design (messages 90d, notes 60d, documents 120d, facts 30d/180d). Our feedback-driven variant goes beyond FadeMem.
- **Use in:** B1.2 (custom feedback-decay benchmark) — FadeMem comparison as baseline

---

### C-11: OpenAI Privacy Filter Model

- **Institution:** OpenAI (Apache 2.0)
- **Key specs:** 1.4B params (50M active, Sparse MoE), bidirectional encoder, 8 PII categories, ~0.4s on CPU, 2.7GB
- **Key finding:** Token classification (not text generation) — cannot run on llama-swap. Must run as separate pipeline (sidecar or native).
- **Relevance to sprachspiel:** Board draft "Privacy Filter Integration [M3]" — fact redaction, log sanitization, tool output scrubbing. ONNX rejected per project philosophy.
- **PT-BR testing:** Good detection for person/address/email/phone, minor boundary issues, no dedicated CPF/RG label (falls under account_number). Fine-tuning possible.
- **Reference:** https://huggingface.co/openai/privacy-filter, https://github.com/openai/privacy-filter

---

### C-12: Shaukat et al. 2026 (Document Chunking Strategies)

- **Institution:** arXiv:2603.06976
- **Key finding:** Paragraph Group Chunking reaches nDCG@5 of 0.459 vs <0.244 for fixed-size chunking. Evaluated 36 methods across 6 domains with 5 embedding models.
- **Relevance to sprachspiel:** Board draft "Context-Aware Chunking [M4]" — validates semantic chunking over fixed-size
- **Reference:** arXiv:2603.06976

---

### C-13: ClashEval (Wu et al. 2024)

- **Institution:** arXiv:2404.10198
- **Key finding:** LLMs overwrite correct internal knowledge with incorrect retrieved evidence in >60% of cases. Without metadata authority distinctions, models treat all chunks equally.
- **Relevance to sprachspiel:** Board draft "Metadata Enrichment [M4]" — authority and recency metadata enables RRF boosting, preventing stale/wrong information from drowning current/correct information
- **Reference:** arXiv:2404.10198

---

### C-14: HyDE (Gao et al. 2022) and Dense X Retrieval (Chen et al. 2023)

- **Institutions:** arXiv:2212.10496, arXiv:2312.06648
- **Key findings:** HyDE moves query embeddings closer to relevant documents by generating hypothetical answers. Propositions (Q&A pairs) as retrieval granularity surpass passage-level.
- **Relevance to sprachspiel:** Board draft "Q&A Pairing / HyDE-like Embedding [M4]" — embedding questions instead of raw text at ingestion time
- **References:** arXiv:2212.10496, arXiv:2312.06648

---

### R-19: Thinking Trace Retrieval (T3 Pipeline)

- **Source:** Arabzadeh et al. 2026 — arXiv:2605.03344 (RAG over Thinking Traces)
- **Current state:** `strip_thinking_tags()` permanently deletes thinking content before storage. Pre-tool messages store thinking inline accidentally. No dedicated thinking trace storage, transformation, or retrieval.
- **Why significant (P0-CRITICAL):** ~80% of thinking traces are permanently lost. The paper demonstrates thinking traces are the most valuable RAG corpus for reasoning tasks, outperforming conventional documents. General-purpose corpora frequently HURT reasoning performance.
- **Implementation phases:**
  - **Phase 0 (P0-CRITICAL):** Preserve thinking content in `thinking_content` column. Fix asymmetric storage bug. Joint PR with #136 (geometry-aware dimensions). `[t3] enabled = false` feature flag.
  - **Phase 1 (P0-HIGH):** ThinkingTrace pipeline + Struct transform. Background job, same-model/CPU-fallback cascade. New `thinking_traces` table.
  - **Phase 2 (P0-HIGH):** Thinking-aware retrieval. RRF fusion of content + traces. k=3 retrieval.
  - **Phase 3 (P1):** Semantic/Reflect transforms. Facts from Reflect. Feedback-weighted RRF for traces.
  - **Phase 4 (P2):** Quality > quantity scoring. Compression (delete raw after T3). Caching.
- **Prerequisite:** #106 (Configurable Embedding) before Phase 1; #107 (EmbeddingProvider) before Phase 3
- **Revisit when:** Phase 0 is next in W4.5 (after W4.0-4.4)

---

### R-20: Thinking-Aware Benchmark Suite

- **Source:** T3 paper benchmarks (AIME, GPQA-Diamond, LiveCodeBench); competitive analysis (YourMemory)
- **Current state:** No benchmarks for memory-augmented reasoning. Zero published metrics.
- **Why deferred to M2/M3:** Requires T3 Phase 2 (thinking-aware retrieval) and Benchmark Infrastructure (#124) to be in place first.
- **Planned benchmarks:**
  - **LoCoMo Adaptation:** Long-context memory benchmark (adapt YourMemory template). Requires W7.1 complete.
  - **RAGAS Evaluation Pipeline:** Framework for evaluating RAG quality. Requires #124.
  - **Feedback-Driven Decay Metrics:** First-mover — no standard benchmark exists. Requires W3 data.
- **Prerequisite:** #124 (Benchmark Infrastructure), W7.1 (Thinking-Aware Retrieval)
- **Revisit when:** W7.1 complete + #124 ready

---

### Decision Records (Why NOT)

### D-01: Majestic Lisp as Rule Engine / MCP Server

- **Decision:** Explicitly rejected (2026-04-28)
- **Reasons:**
  1. Circular dependency risk (sprachspiel depends on majestic-lisp which might depend on sprachspiel)
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

---

### D-06: ONNX Runtime for Privacy Filter

- **Decision:** Explicitly rejected (2026-05-07)
- **Reasons:**
  1. `ort` + `tokenizers` crate dependency chain inflates binary by ~30MB and adds heavy deps
  2. Violates single-binary, Termux-compatible philosophy
  3. Previously evaluated and rejected during NLI cross-encoder investigation for contradiction detection
- **Alternative path:** Python sidecar for M3 (board draft "Privacy Filter Integration"); Rust-native Viterbi classifier as long-term goal (R-18) if usage justifies
- **Revisit only if:** A lightweight Rust-native inference solution emerges that doesn't require ort/tokenizers deps

---

### D-07: ContentType::ThinkingTrace as Rust Enum Variant

- **Decision:** Rejected (2026-05-15)
- **Reasons:**
  1. Thinking traces transformed by T3 are stored in a separate `thinking_traces` table with `transform_type` as a string discriminator — not in `content_items`. String discriminators are more flexible for extensibility (adding `struct`, `semantic_l1`, etc. without Rust enum changes).
  2. In `content_items`, thinking content is an attribute of a message (the thinking that preceded the assistant's response), not a separate content type. The `thinking_content` column and `t3_status` column handle this in the existing `ContentType::Message` path.
  3. Adding a content type for something that is never independently searched/retrieved by users (it's an attribute, not standalone content) would create confusion in the type system.
- **Alternative path:** `thinking_content TEXT` column in `content_items` (Phase 0); `thinking_traces` table with `transform_type TEXT` (Phase 1)
- **Revisit only if:** T3 traces need to be independently searchable by users via commands like `/search traces`, at which point `ContentType::ThinkingTrace` could be reconsidered