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

- **Source:** Meta-cognition brainstorm (internal analysis, §4.1)
- **Current state:** Layer 2 telemetry (#100) uses heuristics (pronoun count, keyword detection) for shift detection
- **Why deferred:** No calibration data from real conversations. Heuristic detection in Layer 2 needs to run first and produce the training data that embeddings would replace.
- **Prerequisite:** Layer 2 (#100) producing shift detection data from 20-30+ real conversations
- **Revisit when:** Heuristic shift detection proves too noisy (>30% false positives) or misses shifts that users flag

---

### R-14: Meta-cognition as Active Tool (meta_cognize)

- **Source:** Meta-cognition brainstorm (internal analysis, §4.2)
- **Current state:** Board draft "meta_cognize() Active Behavioral Tool [M3]" — LLM-callable tool returning behavioral state (mode, shift detection, suggestions)
- **Why deferred to M3:** Depends on Layer 2 (#100) telemetry producing structured data. Complementary to passive telemetry — makes reflection explicit and traceable.
- **Prerequisite:** #100 (Behavioral Telemetry) producing shift data
- **Revisit when:** Layer 2 is in production and producing reliable shift signals

---

### R-15: Behavioral Conflict Detection

- **Source:** Meta-cognition brainstorm (internal analysis, §4.3)
- **Current state:** Board draft "Behavioral Conflict Detection [M3]" — detect tensions between SOUL.md (configured personality) and emergent behavior
- **Why deferred to M3:** Depends on #77/#78 (Visualize Connections / Relations Graph) for the structural foundation to represent personality-behavior tensions as a graph problem.
- **Prerequisite:** #77 and #78 implemented with relation extraction working
- **Revisit when:** Relations graph is functional and can represent personality vs behavior edges

---

### R-16: Distributive Meta-cognition (Multi-Personality Evaluation)

- **Source:** Meta-cognition brainstorm (internal analysis, §4.4)
- **Current state:** Not on board — too speculative even for a draft
- **Why deferred:** Requires SOUL.md multi-personality support (multiple configurable personas evaluating each other). Currently sprachspiel has a single personality.
- **Prerequisite:** Extended Personalities System (#49) implemented; Layer 3 (#101) producing reflections that can be compared across personalities
- **Revisit when:** Multi-personality is mature enough that multiple evaluations of the same response are feasible

---

### R-17: RAPTOR-like Hierarchical Retrieval

- **Source:** RAG improvement research (internal analysis, §5); Sarthi et al. 2024 (arXiv:2401.18059)
- **Current state:** Flat retrieval (BM25 + cosine + RRF). No hierarchical summarization.
- **Why deferred:** Requires Metadata Enrichment, HyDE-like pairing, and Semantic Dedup (board drafts) to be stabilized first. RAPTOR adds a summarization layer on top of functioning chunk-level retrieval.
- **Prerequisite:** Context-Aware Chunking, Metadata Enrichment, Semantic Dedup all in production
- **Revisit when:** Flat retrieval with attention priming and metadata boosting still shows quality gaps at scale (>10k chunks)

---

### R-18: Privacy Filter as Rust-Native Classifier

- **Source:** Privacy filter proposal (internal analysis, §10)
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
  - **Phase 0 (P0-CRITICAL):** Preserve thinking content in `thinking_content` column. Fix asymmetric storage bug (5 data loss paths: streaming response, non-streaming response, pre-tool concatenation, continuation turns, compaction summary — only first 4 are in scope; compaction is by design, see D-08). Add `[t3] enabled = false` feature flag. Includes continuation thinking fix. **Decoupled from #136** — standalone PR with simple `ALTER TABLE` migration v13→v14 (no vec0 changes). See Decision Records D-08, D-09, D-11.
  - **Phase 1 (P0-HIGH):** ThinkingTrace pipeline + Struct transform. Background job, same-model/CPU-fallback cascade. New `thinking_traces` table.
  - **Phase 2 (P0-HIGH):** Thinking-aware retrieval. RRF fusion of content + traces. k=3 retrieval.
  - **Phase 3 (P1):** Semantic/Reflect transforms. Facts from Reflect. Feedback-weighted RRF for traces.
  - **Phase 4 (P2):** Quality > quantity scoring. Compression (delete raw after T3). Caching.
- **Prerequisite:** Phase 0 has no prerequisites (was falsely dependent on #107, now resolved). Phase 1 depends on #106 (Configurable Embedding). Phase 3 depends on #107 (EmbeddingProvider). Phase 1 also needs `t3_status` column (deferred from Phase 0, see D-09).
- **Revisit when:** Phase 0 is next in W4.5 (unblocked — no dependency on #107 or #136)

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
  2. In `content_items`, thinking content is an attribute of a message (the thinking that preceded the assistant's response), not a separate content type. The `thinking_content` column handles this in the existing `ContentType::Message` path. The `t3_status` column is deferred to Phase 1 (see D-09) where the T3 transform pipeline needs state tracking — in Phase 0, `thinking_content IS NOT NULL` is equivalent to "has thinking."
  3. Adding a content type for something that is never independently searched/retrieved by users (it's an attribute, not standalone content) would create confusion in the type system.
- **Alternative path:** `thinking_content TEXT` column in `content_items` (Phase 0); `thinking_traces` table with `transform_type TEXT` (Phase 1)
- **Revisit only if:** T3 traces need to be independently searchable by users via commands like `/search traces`, at which point `ContentType::ThinkingTrace` could be reconsidered

---

### D-08: Compaction Summary Does NOT Preserve Thinking

- **Decision:** Compaction summaries are excluded from thinking preservation (2026-05-31)
- **Reasons:**
  1. Compaction summaries are **content generated by the LLM**, not original thinking traces. The LLM produces a summary without access to the original thinking (which was stripped before storage).
  2. T3's goal is to retrieve **original thinking traces** — the reasoning the model produced during its response. A compaction summary's "thinking" would be meta-reasoning about what to summarize, not the original reasoning trace.
  3. The compaction summary is already a lossy transformation. Adding thinking to it would create a false impression that the original reasoning is preserved, when only the LLM's summary-level meta-reasoning would be captured.
  4. Multi-level compaction (compaction of already-compacted ranges) would require accumulating thinking across summaries, further removing it from the original traces.
- **Alternative path:** T3-Phase1 TAP-Reflect processes `thinking_content` from original messages (not summaries). Compaction-Triggered Analysis (R-23) saves the summary itself (without thinking) and feeds it to the Reflect pipeline as input.
- **Revisit only if:** Research shows that meta-reasoning from summaries has retrieval value distinct from original traces, at which point a separate `summary_thinking` column could be considered.

---

### D-09: thinking_trace_status Deferred from T3-Phase0 to T3-Phase1

- **Decision:** `thinking_trace_status` column and `ThinkingTraceStatus` enum are deferred to T3-Phase1 (2026-05-31)
- **Reasons:**
  1. In Phase 0, there are only 2 meaningful states: "has thinking" and "no thinking." This is equivalent to `thinking_content IS NOT NULL`, making a separate column redundant.
  2. The 3+ state machine (`None` → `Raw` → `Pending` → `Done`) only becomes necessary in Phase 1 when the Thinking Trace Transform pipeline needs to track processing state.
  3. Adding the column now would be YAGNI — no code reads `thinking_trace_status` in Phase 0 because no pipeline exists to process it.
  4. Adding later is cheap: `ALTER TABLE content_items ADD COLUMN thinking_trace_status INTEGER DEFAULT 0` (1 byte per row, backward compatible, no vec0 changes).
- **Phase 1 scope:** Add `thinking_trace_status INTEGER DEFAULT 0` column. Implement `ThinkingTraceStatus` enum: `None=0` (no thinking), `Raw=1` (thinking preserved, awaiting transform), `Pending=2` (transform in progress), `Done=3` (transform completed, trace in `thinking_traces` table).
- **Revisit when:** T3-Phase1 (#152) implementation begins

---

### D-10: #136 Scope Rewrite — Embedding Configuration and Model Registry

- **Decision:** #136 is rewritten from "Geometry-Aware Default Dimensions Formula" to "Geometry-Aware Embedding Configuration and Model Registry" (2026-05-31)
- **Reasons:**
  1. The original scope mixed configuration policy (`recommended_dimensions()`) with benchmark infrastructure (d_eff storage) and database migration (vec0 resize). These are different concerns that should be layered.
  2. `d_eff` is a property of the **embedding model**, not of an individual document. Storing it as a vec0 auxiliary column is redundant — all rows from the same model have the same `d_eff`. A model registry table (`embedding_models`) centralizes this information.
  3. The `embedding_models` table supports multiple future features: configurable models (#106), benchmarks (#135), geometry-aware RRF (#137), and embedding provider abstraction (#107). Scattering `d_eff` across config.toml fields doesn't serve these downstream features.
  4. `sprach diagnostics --save` integration provides a natural workflow: measure → save → configure. Without it, `d_eff` is a manual step that users will skip.
- **New phases:** (1) `embedding_models` table, (2) diagnostics --save integration, (3) `recommended_dimensions()` function, (4) dynamic vec0 dimensions (only if dimensions change)
- **Dependency change:** Now depends on #106 and #135 (was positioned before them — triage error)
- **Revisit when:** #106 complete

---

### D-11: #136 Decoupled from #151

- **Decision:** #136 and #151 are no longer a joint PR (2026-05-31)
- **Reasons:**
  1. The joint PR was proposed for a "shared migration v12→v13" that added both `thinking_content` and `d_eff` columns. Since #136 now needs `embedding_models` table (not just columns in `content_items`), and #151 only needs `ALTER TABLE content_items ADD COLUMN thinking_content TEXT`, the migrations are completely different.
  2. #136 depends on #106 and #135 (configurable model + d_eff measurements). #151 depends on nothing. Bundling them would block the P0-CRITICAL bug fix behind unrelated prerequisites.
  3. Without #136 in the joint PR, the #151 migration is simple: one `ALTER TABLE` on `content_items`. No vec0 changes, no recovery pipeline, no embedding regeneration. This dramatically reduces risk for the P0-CRITICAL fix.
  4. #136 may require DROP+reCREATE of vec0 tables (Phase 4) — an expensive operation that should not be coupled with a data preservation fix.
- **New positions:** #151 is W4.5 (standalone), #136 is W4.7 (after #106 and #135)
- **Revisit if:** #106 and #135 complete before #151, at which point a combined PR could be reconsidered (but #151 should not wait for them)

---

### R-21: Orientation Cache (PEEK)

- **Source:** Gu et al. 2026 (arXiv:2605.19932)
- **Current state:** No cached orientation between sessions. AGENTS.md provides static context only.
- **Why significant:** +6.3–34.0% quality gains on reasoning/aggregation tasks with constant map. Orientation survives session boundaries, reducing cold-start problems.
- **Implementation phases:**
  - **OC-1a (P1-high):** Static Context Map composition — compose from AGENTS.md + facts + retrieved content. No LLM needed. Can start in parallel with W6/W7.
  - **OC-1b (P1-high):** Compaction summary as TAP input — save summaries that already exist and feed them to the Reflect pipeline. Minimum effort.
  - **OC-2 (P1):** Dynamic Distiller — LLM-powered orientation extraction. Depends on TAP-3.
  - **OC-3 (P2):** Cartographer + Evictor — automatic map maintenance and stale item removal.
- **Prerequisite:** TAP-2 for meaningful feedback; #136 for embedding quality
- **Cross-refs:** DESIGN.md §15.5, PEEK cross-reference, UNIFIED-RESEARCH-VISION §2
- **Revisit when:** TAP-2 complete; OC-1a can start independently
- **Information routing reframing:** PEEK maps to **persistent K/V store at session level** — the Orientation Cache is a context map that persists across sessions, providing K (key concepts) and V (orientation summaries) that are always available, with Gate=open for the current session and Capacity=session context window. See R-29 for the full mapping.

---

### R-22: Trace Analysis Pipeline (Unified Background Analysis)

- **Source:** Synthesis of PEEK + T3 + existing feedback
- **Current state:** T3 phases 0-2 planned (W4.5, W7.0, W7.1). No unified pipeline.
- **Why significant:** 1 analysis, multiple destinations. Amortizes LLM call cost.
- **Implementation phases:** See DESIGN.md §4.1 (TAP-0 through TAP-4)
- **Prerequisite:** TAP-1 as base; model benchmark (§11.5) for fallback
- **Cross-refs:** DESIGN.md §4.1, UNIFIED-RESEARCH-VISION §3
- **Information routing reframing:** TAP maps to **offline gate function learning** — the pipeline learns which information to GATE in/out based on Q=current context, K=past content, V=extracted facts/patterns. TAP-Reflect = learning the Gate function from failure patterns. TAP-Struct = learning what V format maximizes downstream retrieval quality. See R-29 for the full mapping.
- **Revisit when:** W7.0 complete

---

### R-23: Compaction-Triggered Analysis

- **Source:** DESIGN.md §12.4 crossover
- **Current state:** Compaction generates summary but discards it.
- **Why significant:** Summary is a free Distiller — already produced, currently thrown away.
- **Implementation:** Save compaction summary to DB; trigger TAP-Reflect on saved summary.
- **Prerequisite:** TAP-3 starting
- **Cross-refs:** RECURSION-SPRACHSPIEL.md §5, UNIFIED-RESEARCH-VISION §3 Sinergy 5
- **Information routing reframing:** Compaction-triggered analysis maps to **compressing V** — compaction produces a summary (compressed V), which is currently discarded. TAP-Reflect processes this compressed V to extract orientation and failure patterns. The quality of compression determines how much information is preserved (see R-30 for the metric). See R-29 for the full mapping.
- **Revisit when:** TAP-3 begins

---

### R-24: Enraizamento Cultural (Cultural Grounding)

- **Source:** Diógenes, Souza, Guelpeli 2026 (PRW-5188-2880)
- **Current state:** SOUL.md has communication instructions but no explicit cultural grounding. No mechanism to detect or correct calques, pragmatic loss, or competence illusion ("stochastic parrot" — Bender et al. 2021).
- **Why significant:** Models trained dominantly in English fail in regionalisms and Global South pragmatics. SOUL.md solves ~40-60% at the linguistic register level, but not deep semantic loss, slang anachronisms, or factual cultural knowledge gaps.
- **Implementation phases:**
  - **Phase 1 (M4, immediate documentation):** Add cultural grounding section to SOUL.md (NLP-Historical §7, A4). Principle of invisibility, transparent confidence, no calquing.
  - **Phase 2 (M4):** S2.5 Patching + S2.meta2/S2.meta3 for personality evolution based on cultural failures
  - **Phase 3 (M4+):** Passive models (calque classifier, pragmatics classifier) as pipeline middleware, prioritized by invisibility principle (§9.7)
  - **Phase 4 (M4+):** pt-BR cultural knowledge RAG (C1, NLP-Hist §9.2)
- **Cross-refs:** Translation fleet as canary test (NLP-Hist §9.2); Passive models as curatorial immune system (NLP-Hist §9.5); TTR monitor as Reflect input (NLP-Hist §9.3); UNIFIED-RESEARCH-VISION §3 Sinergy 7
- **Key insight:** "Empathy ≠ failure" — behavioral shifts are not bugs, but must be transparent. Suppression is not the goal; visibility and user choice are. (NLP-Historical §10)
- **Revisit when:** Immediate (Phase 1 is documentation only); Phase 2 depends on S2.5 operational

---

### R-25: Norm Correction for Embeddings (TurboVec Technique)

- **Source:** TurboQuant (Zandieh et al., ICLR 2026); TurboVec (Codrai 2026); RaBitQ (Gao & Long, SIGMOD 2024)
- **Current state:** Cosine similarity in sqlite-vec has systematic underestimation bias, amplified when d_eff is low (Matryoshka 768→256).
- **Why significant:** 1 float per vector corrects the bias at zero query-time cost. Impacts TAP-2 (thinking-aware retrieval), fact dedup, and all semantic retrieval. Especially important when d_eff < 0.7.
- **Implementation:** ALTER tables add `norm_correction REAL`; calculate on insert; multiply in scoring.
- **Effort:** ~20 lines Rust, 1 SQL migration
- **Prerequisite:** #133 (embedding diagnostics) to measure d_eff and confirm bias
- **Cross-refs:** RAG-Vector §2, UNIFIED-RESEARCH-VISION §3 Sinergy 4
- **Milestone:** M1/W4.x (addendum to #136)
- **Information routing reframing:** Norm correction improves **K accuracy in the semantic retrieval head** — systematic underestimation of cosine similarity from scalar quantization means K vectors are less distinguishable. One float per vector corrects the bias, making the Gate function more accurate at its job. See R-29 for the full mapping, R-30 for the quality metric that measures downstream impact.
- **Revisit when:** #133 complete; add as addendum of W4.x

---

### R-26: Context-Offload via Sub-Agent

- **Source:** RLM (Zhang et al. 2025, arXiv:2512.24601); RECURSION-SPRACHSPIEL.md
- **Current state:** Compaction is the only mechanism when context fills. Offload resolves 1 of 3 pressure sources (large tool results).
- **Why significant:** Preserves 100% of information (sub-agent sees everything), vs compaction which loses details. But slower (2-5x) and compaction remains inevitable for long history.
- **Depends on:** B1.5 benchmark (validate H1: offload preserves more facts than compaction)
- **Implementation:** Offload threshold between 85% and 88%, SubagentRunner with config, SourceType::ToolOffload
- **Cross-refs:** TAP offline vs offload inline (complementary); Session variables as on-demand session vars (RLM §4)
- **Critical constraint:** Benchmark-driven validation required before ANY architecture change. Models <10B are probably inadequate for managing their own search. H6 must be tested.
- **Information routing reframing:** Context-offload maps to an **alternative Gate mechanism** — instead of compressing within context (compaction Gate: discard low-g_i turns), offload routes entire tool-result V to a sub-agent (Gate: route to different Capacity). This preserves 100% of information for the offloaded content but adds latency. The choice between compaction and offload is a Gate design decision. See R-29 for the full mapping.
- **Milestone:** M3 (conditional on B1.5 confirming H1)
- **Revisit when:** B1.5 implemented; if H1 confirmed, feature flag `[context] offload_enabled = true`

---

### R-27: Translation Fleet as Cultural Fragility Canary Test

- **Source:** NLP-Historical §9.2 + Translation Models research
- **Current state:** Hy-MT2 (1.8B) and TranslateGemma (4B) are specialized translation models. If even specialized models fail on pt-BR slang, general models fail more.
- **Why significant:** The translation fleet is a low-cost detector for cultural fragility. Can be used as an automatic test: if the translation model fails on a term, the general model likely fails worse. This guides where SOUL.md needs patches and where curation is most urgent.
- **Implementation:** Not a code feature — it's a testing pattern. Run translations of critical pt-BR terms via fleet and verify quality.
- **Cross-refs:** R-24 (Cultural Grounding) Phase 2+; NLP-Hist §9.2
- **Milestone:** M3+ (informational, when cultural curation is active)
- **Revisit when:** R-24 Phase 2+ (curatorial workflow active)

---

### R-28: Passive Models as Curatorial Middleware

- **Source:** Passive Models research (BusyBeaver, Privacy Filter, LlamaFirewall, WebWorld, Needle, Nandi, Dreamer4)
- **Current state:** Three archetypes identified (Classifiers, Policy Models, World Simulators), but none integrated into Sprachspiel.
- **Why significant:** Lightweight classifiers (calque detector, pragmatics classifier, confidence scorer, TTR monitor) form a "curatorial immune system" — detecting where human intervention is needed and reducing review volume to what matters (NLP-Hist §9.5).
- **Key constraint:** Models with custom architectures (BusyBeaver QDelta, Needle encoder-decoder) cannot run on llama.cpp. They need their own runtime or Wasm. Most viable integration is via Ollama (ShieldGemma, LlamaGuard) or as sidecar process.
- **Priority ordering (by invisibility, NLP-Hist §9.7):** Confidence scorer > Pragmatics classifier > Calque detector > TTR monitor
- **Implementation phases:** M3 plugin system design (WASM or Ollama sidecars); M4+ for models with custom runtime
- **Cross-refs:** UNIFIED-RESEARCH-VISION §3 Sinergy 2 and §7; Passive Models README §9-11
- **Milestone:** M3 (design) / M4 (implementation)
- **Revisit when:** Plugin system operational (#15); S2.5 proving curatorial value

---

### R-29: Information Routing Consistency Mapping

- **Source:** Synthesis of attention, retrieval, and memory management research (Gated DeltaNet-2, UniMem, Pichay, Titans, Context Cartography)
- **Current state:** No formal mapping between Sprachspiel components and the information routing abstraction. Components (context compaction, RAG retrieval, RRF fusion) are designed independently.
- **Why significant:** Research reveals that token attention, RAG retrieval, context compaction, and persistent memory are instances of the same abstract operation: `L = (Q, K, V, Gate, Capacity, Heads)`. Making this structure explicit enables:
  1. Identifying where Sprachspiel uses 1-signal heuristics that could benefit from multi-signal gates
  2. Designing adaptive weights with theoretical grounding instead of arbitrary tuning
  3. Enabling quantitative measurement of information loss during compaction
  4. Allowing cross-scale signal flow (retrieval scores → compaction importance)
- **Biggest gap identified:** Context compaction uses 1-signal heuristic (recency) vs. multi-signal gate (relevance + recency + importance). See R-30 for the quality metric that quantifies this gap.
- **Deliverable:** `doc/src/development/information-routing-mapping.md` — formal mapping document. Zero code, purely architectural.
- **Implementation:** Research document only. No code changes.
- **Cross-refs:** #179 (formal issue); R-21 (Orientation Cache = persistent K/V at session level); R-22 (TAP = offline gate function); R-23 (Compaction = compressed V with quality metric); R-25 (Norm Correction = improving K accuracy); R-26 (Offload = alternative gate mechanism)
- **Information routing reframing notes:**
  - R-21 → PEEK maps to **persistent K/V store at the session level** (Gate=open for current session, Capacity=session context window)
  - R-22 → TAP maps to **offline gate function learning** (learning which information to Gates in/out based on relevance/recency/importance)
  - R-23 → Compaction-triggered analysis maps to **compressing V** (keeping summaries, discarding raw content, tracked by quality metric R-30)
  - R-25 → Norm correction maps to **improving K accuracy in the semantic retrieval head** (reducing systematic bias in similarity computation)
  - R-26 → Context-offload maps to **alternative Gate mechanism** (routing to sub-agent instead of compressing within context)
- **Milestone:** M3 (research) / M4 (informing design)
- **Revisit when:** When adaptive RRF (#137) or multi-signal compaction design begins; OC-1a can proceed independently

---

### R-30: Compaction Quality Metric

- **Source:** Synthesis of information routing research; compaction as rank-reduction operation
- **Current state:** No quantitative measure of information loss during compaction. "Keep first N + last N" has no quality score.
- **Why significant:** Without a quality metric, we cannot optimize compaction without running full end-to-end benchmarks every time. The metric enables:
  1. Fast iteration on compaction strategies (lightweight proxy for task performance)
  2. Quantitative comparison in B1.5 (offload preserves X% of gated information vs compaction Y%)
  3. Validation that multi-signal gates improve over recency-only (H1.1 in B1.5)
- **Formula:**
  ```
  quality = 1 - Σ(discarded g_i) / Σ(all g_i)
  ```
  Where `g_i` is the gate/retention weight of each turn. Phase A (recency-only): `g_i = 1` for kept, `0` for discarded → quality = kept_turns / total_turns. Phase B (multi-signal): `g_i = σ(α·relevance + β·recency + γ·importance)`.
- **Implementation phases:**
  - **Phase A (with TAP-1, ~5 lines):** Log recency-weighted quality on every compaction event. Add `/context quality` command. Trivially computable with current system.
  - **Phase B (after M3.γ multi-signal gate):** Enrich metric with relevance and importance signals. Depends on B1.5 validating H1 (multi-signal > recency-only).
- **Phase A is tiny scope** — can be added as an addendum to #152 (TAP-1) without needing a separate issue.
- **Cross-refs:** #152 (TAP-1 — add Phase A as sub-item); #158 (B1.5 — uses metric for quantitative comparison); #179 (Information Routing Mapping — formal framing)
- **Information routing reframing note:** Quality metric measures how much "V" (information value) is preserved vs discarded by the compaction Gate. Currently the Gate uses recency-only (binary: keep/discard), making quality = kept/total. Multi-signal Gate produces continuous g_i values, enabling weighted quality scores.
- **Milestone:** M1 (Phase A, with TAP-1) / M3 (Phase B, after multi-signal gate)
- **Revisit when:** Phase A can start immediately with TAP-1; Phase B after B1.5

---

### R-31: System Prompt Architecture Documentation

- **Source:** Auto-diagnosis of Sprach system prompt (system-prompt-analysis.md)
- **Current state:** Prompt architecture (SOUL → OPERATION → CONTEXT → CAPABILITY) is implicit in code but not documented. Auto-diagnosis flagged "redundancy" between layers, but analysis confirmed this is intentional layering — each layer serves a different purpose (identity, behavior, semantics, syntax).
- **Why deferred to M2:** TUI (#16) will require prompt restructuring for interactive modes (different views need different prompt compositions). Documenting now and changing later wastes effort.
- **Implementation:** `doc/src/development/prompt-architecture.md` — explains the four layers, why "redundancy" is intentional, and how to add new sections correctly.
- **Key insight from triage:** The auto-diagnosis identified 3x "redundancy" in Behavior instructions, Memory instructions, and File Operations. Analysis confirmed these are NOT redundant — they are different layers serving different purposes:
  - SOUL.md "Search first" = behavioral motivation (WHY use memory)
  - MEMORY section = semantic instruction (WHAT retrieved_context means)
  - MEMORY TOOLS = syntactic guide (HOW to use remember tool)
  - Consolidating these would lose the ability to disable SOUL.md independently with `--soulless`.
- **Cross-refs:** #182 (prompt clarifications — first formalization of hierarchy); #16 (TUI will change prompt structure); #180 (MCP Client Phase 1 — tool discovery will change how tool descriptions are delivered)
- **Milestone:** M2 (documentation, no code)
- **Revisit when:** TUI implementation starts and prompt structure needs redesign

---

### R-32: ratatui-cheese Widget Adoption (M2 TUI Components)

- **Source:** M2 TUI library evaluation (ratatui-cheese v0.7.0, MIT license, ratatui 0.30)
- **Current state:** W6 (Responsive Chat Rebuild) provides Ratatui foundation — `RatatuiView`, `CrosstermInput`, `tui-markdown`, `rattles` spinner, `StatusBar` widget. M2 TUI (#16) needs panel system, selection widgets, help display, and theming.
- **Why significant:** ratatui-cheese provides 9 widgets + Palette system that cover 4-5 of the TUI widget needs at low integration cost. Minimal dependency depth (ratatui + unicode-width only). Compatible version (ratatui 0.30 matches Sprachspiel).
- **Component assessment:**
  - ✅ **Adopt — Help:** Drop-in keybinding display with short mode (1-line) and full mode (multi-column). Replaces ad-hoc keybinding display. Wired to `?` key. Effort: Low — no overlap with existing code.
  - ✅ **Adopt — Fieldset:** Container with decorated borders (`── Title ──`) and 7 fill styles. Essential for sidebar/panel visual sectioning in multipane layout. Replaces manual `Block::bordered()` styling. Effort: Low.
  - ✅ **Adopt — Select / MultiSelect:** Single/multi selection pickers with cursor, disabled options, validation. Needed for `/steer` (model picker), `/queue` (session list), tool selection, config menus. Effort: Medium — needs integration with event loop and state management.
  - ✅ **Adopt — List + Paginator:** Paginated list with custom headers, selection indicators, item spacing. Natural fit for `/queue` (conversation history), tool list, context window display. Paginator provides `get_slice_bounds()` helper for clean pagination UX. Effort: Medium — state management + Paginator integration.
  - ⏸️ **Evaluate — Palette:** 5 presets (Dark, Light, Charm, Ocean, Sunset). Need comparison against existing Catppuccin-based styles in `src/chat/tui/styles.rs`. Decision depends on whether Palette simplifies or fragments the theming layer. Added as sub-item of #16.
  - ❌ **Skip — Spinner:** Sprachspiel uses `rattles 0.2` for spinner animation. Switching to cheese-spinner is a lateral move (12 presets vs rattles' presets, no functional gain). Keep rattles.
  - ❌ **Skip — Input:** Sprachspiel uses `ratatui-textarea 0.9.1` which supports multi-line editing, undo/redo, selection, clipboard. The cheese Input is single-line only — a downgrade. Keep ratatui-textarea.
  - ❌ **Skip — Tree:** No tree-structured data in M2 scope. Niche use case for M3+. Revisit if file/document trees are needed.
- **Risk assessment:**
  - **Maturity:** Young (1 month), single maintainer, 27 stars, 0 forks — but MIT licensed so forkable
  - **Breaking changes:** Rapid release cadence (7 versions in 5 weeks) — API may shift
  - **Mitigation:** Pin exact version in `Cargo.toml` (`ratatui-cheese = "0.7"`). Widgets are simple enough to vendor if the library stagnates.
- **Adoption plan:**
  1. Add `ratatui-cheese = "0.7"` to `Cargo.toml` as direct dependency (no feature flag)
  2. Help widget — replace ad-hoc keybinding display, wire to `?` key
  3. Fieldset — use for sidebar/panel borders in multipane layout
  4. Select / MultiSelect — implement for `/steer` and config menus
  5. List + Paginator — implement for `/queue` and conversation display
  6. Evaluate Palette against existing Catppuccin styles — adopt if it simplifies theming
  7. Skip Spinner, Input, Tree (rattles, ratatui-textarea, and no use case)
- **Dependencies:** ratatui 0.30 — compatible with Sprachspiel stack
- **Cross-refs:** #16 (TUI — primary consumer), #117 (Interaction Modes — Select/MultiSelect for `/steer`), R-33 (Onboarding Mode — uses Help, Fieldset, Select, Paginator)
- **Milestone:** M2 (with #16)
- **Revisit when:** M2 TUI implementation starts

---

### R-33: First-Run Onboarding Mode (OnboardingWizard)

- **Source:** M2 UX design evaluation — discoverability analysis of Sprachspiel's 30+ slash commands, feature-gated tools, 3 config files, and invisible capabilities
- **Current state:** Zero onboarding. `Settings::default()` provides sensible defaults (qwen3.5:4b, localhost:11434, dark skin, all features on). Only mechanism: `--init-config` creates a commented sample config, and a banner hint "Type /help for commands". The following features are invisible to new users:
  - 30+ slash commands (discoverable only via `/help`)
  - Keybindings (Ctrl+C copy/cancel, Ctrl+U undo, etc.) — undocumented in TUI
  - Tool system (weather, file, calc, etc.) — shown in banner as ✅/❌ but not explained
  - Thinking mode (`/think`) — not prompted
  - Model switching (`/model`) — not prompted
  - Facts system (auto-extraction) — invisible until it fires
  - Notes, documents, skills, feedback — only via `/help`
  - Per-project session auto-save, `models.toml`, `tools.toml` — not communicated
  - Anonymous mode (`--anonymous`) — CLI flag only
- **Why significant:** Discoverability is a core UX gap. Sprachspiel has significant functionality that users never find without prior knowledge. An onboarding flow that respects the "zero-config works" principle (app launches immediately even without config) while progressively revealing features would dramatically improve first-run experience.
- **Architecture:** `OnboardingWizard` state machine composing ratatui-cheese widgets, invoked when `config.toml` is missing or via `/setup` command at any time.
- **OnboardingStep enum:**
  - `Welcome` — Intro text + Help widget (navigation keys) + Paginator (step 1/N)
  - `Connection` — Fieldset("Connection") + Input(Ollama host/port) + Select(protocol)
  - `Model` — Fieldset("Model") + Select(default model) + List(available models)
  - `Preferences` — Fieldset("Preferences") + Select(skin: dark/light/mono) + MultiSelect(enable features) + Select(thinking mode)
  - `Commands` — Interactive Help showing essential keycommands + `/` commands
  - `Confirm` — Fieldset("Review") + summary display + Select(confirm/edit/start)
- **Design principles:**
  1. **Never block the user** — onboarding is optional, skippable at every step, and re-invocable via `/setup`
  2. **Zero-config works** — smart defaults mean the app launches immediately even without config
  3. **Progressive disclosure** — show minimal UI first, reveal complexity via `?` and commands
  4. **Auto-detect over ask** — check if Ollama is running, detect available models, test connectivity before asking
  5. **Persist progress** — if user quits mid-wizard, save partial config so they can resume
- **Config surface covered:** 3 TOML files (~30 settings with defaults): `config.toml` (connection, skin, features), `models.toml` (custom models), `tools.toml` (external tools, timeouts)
- **Integration points:** `Settings::load()` for first-run detection (config.toml missing), `/setup` command for re-invocation, `show_welcome()` hook
- **ratatui-cheese widget mapping by step:**
  | Step | Widgets Used |
  |------|-------------|
  | Welcome | `Help` (short mode) + `Paginator` |
  | Connection | `Fieldset` + `Input` + `Select` |
  | Model | `Fieldset` + `Select` + `List` + `Paginator` |
  | Preferences | `Fieldset` + `Select` + `MultiSelect` |
  | Commands | `Help` (full mode) |
  | Confirm | `Fieldset` + summary display |
- **Note:** Also evaluated `ratatui-form` (v0.1.1) for field validation within each wizard step. Single-screen only, immature — not recommended for adoption at this time. Could complement cheese for field validation later.
- **Issue tracking:** Sub-item of #16 (TUI), not a separate issue. Added as checklist item within the TUI issue.
- **Prerequisite:** #16 (TUI with panel system and ratatui-cheese integration)
- **Cross-refs:** #16 (TUI — parent issue), #117 (Interaction Modes — uses Select/MultiSelect), R-32 (ratatui-cheese widgets)
- **Milestone:** M2 (design phase; implementation in parallel with TUI)
- **Revisit when:** TUI panel system (#16) has basic layout working