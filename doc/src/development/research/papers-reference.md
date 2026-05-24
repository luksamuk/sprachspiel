# Research Papers Reference

Papers that informed the Implementation Directive. **PDFs are not stored in the repository** - use the arXiv links below.

## Papers

| Paper | Authors | arXiv | Year |
|-------|---------|-------|------|
| **MemOS: A Memory OS for AI System** | Li et al. (38 authors) | [arXiv:2507.03724](https://arxiv.org/abs/2507.03724) | 2025 |
| **OpenClaw-RL: Train Any Agent Simply by Talking** | Wang et al. (5 authors) | [arXiv:2603.10165](https://arxiv.org/abs/2603.10165) | 2026 |
| **MemGPT: Towards LLMs as Operating Systems** | Packer et al. (7 authors) | [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) | 2023 |
| **Contradiction Detection with Contradiction-Specific Word Embedding** | Li, Qin & Liu | [DOI: 10.3390/info10020059](https://www.mdpi.com/1999-4893/10/2/59) | 2017 |
| **Contradiction Detection in RAG Systems** | Gokul, Tenneti & Nakkiran | [arXiv:2504.00180](https://arxiv.org/abs/2504.00180) | 2025 |
| **DRAGged into Conflicts (CONFLICTS benchmark)** | Cattan et al. | [arXiv:2506.08500](https://arxiv.org/abs/2506.08500) | 2025 |
| **LongMemEval: Benchmarking Chat Assistants on Long-Term Interactive Memory** | Wu et al. | [arXiv:2410.10813](https://arxiv.org/abs/2410.10813) | 2024 |
| **WikiContradict: Evaluating LLMs on Real-World Contradictory Knowledge** | Wasserblat et al. | [arXiv:2406.13805](https://arxiv.org/abs/2406.13805) | 2024 |
| **HaluMem: Evaluating Hallucinations in Memory Systems of Agents** | MemTensor team | [arXiv:2511.03506](https://arxiv.org/abs/2511.03506) | 2025 |
| **Beyond Cosine Similarity: Antonym Intrusion in Synonym Graphs** | Tosun & Buldur | [arXiv:2601.13251](https://arxiv.org/abs/2601.13251) | 2026 |
| **On the Theoretical Limitations of Embedding-Based Retrieval** | Boratko et al. | [arXiv:2508.21038](https://arxiv.org/abs/2508.21038) | 2025 |
| **How Small Transformations Expose Weakness of Similarity Measures** | — | [arXiv:2509.09714](https://arxiv.org/abs/2509.09714) | 2025 |
| **Sparse Contrastive Learning for Contradiction Retrieval (SparseCL)** | — | [arXiv:2406.10746](https://arxiv.org/abs/2406.10746) | 2025 |
| **RAG over Thinking Traces Can Improve Reasoning Tasks (T3)** | Arabzadeh, Ma, Min, Zaharia | [arXiv:2605.03344](https://arxiv.org/abs/2605.03344) | 2026 |
| **PEEK: Context Map as an Orientation Cache for Long-Context LLM Agents** | Gu et al. | [arXiv:2605.19932](https://arxiv.org/abs/2605.19932) | 2026 |
| **Gated DeltaNet-2** | Hatamizadeh et al. | [arXiv:2605.22791](https://arxiv.org/abs/2605.22791) | 2026 |
| **UniMem: Towards a Unified View of Memory Architectures** | Fang et al. | [arXiv:2402.03009](https://arxiv.org/abs/2402.03009) | 2024 |
| **Pichay: Demand Paging for LLM Context** | Mason | [arXiv:2603.09023](https://arxiv.org/abs/2603.09023) | 2026 |
| **Titans: Learning to Memorize at Test Time** | Behrouz et al. | [arXiv:2501.00663](https://arxiv.org/abs/2501.00663) | 2024 |
| **Context Cartography** | Wu & Gartner | [arXiv:2603.20578](https://arxiv.org/abs/2603.20578) | 2026 |

## Key Contributions

### MemOS (2025)
- Memory as manageable system resource
- Three-tier memory hierarchy (plaintext → activation → parameter)
- MemCube abstraction with lifecycle management
- Foundation for continual learning and personalization

### OpenClaw-RL (2026)
- Next-state signals are universal learning sources
- Separation of evaluative signals (scalar reward) and directive signals (textual hints)
- Asynchronous training pipeline
- PRM (Process Reward Model) and OPD (On-Policy Distillation)

### MemGPT (2023)
- Virtual context management
- Hierarchical memory (main context ↔ archive)
- Self-editing memory
- Interrupts for memory operations

### Contradiction & Conflict Detection

#### Li, Qin & Liu (2017) — Contradiction-Specific Word Embedding
- Standard embeddings (Word2Vec, GloVe) map antonyms to near-identical vectors
- Proves semantic similarity alone cannot detect contradictions
- **Sprachspiel implication:** Two-step approach needed — embeddings for retrieval, triples for disambiguation

#### Gokul, Tenneti & Nakkiran (2025) — Contradiction Detection in RAG
- LLMs achieve at most 71% F1 on contradiction detection in RAG
- Three contradiction types: self, pair, conditional
- **Sprachspiel implication:** LLM-based contradiction detection is unreliable; deterministic approach preferred

#### Cattan et al. (2025) — CONFLICTS Benchmark
- First benchmark for knowledge conflicts in search-augmented LLMs
- Taxonomy of conflict categories with expected model behaviors
- **Sprachspiel implication:** Our exclusive/accumulative predicate classification aligns with their broader taxonomy

#### LongMemEval (2024)
- Evaluates 5 core long-term memory abilities of chat assistants
- Includes knowledge update testing (contradiction handling)
- **Sprachspiel implication:** Validates need for memory update with conflict resolution

#### WikiContradict (NeurIPS 2024)
- 253 human-annotated instances of contradictory Wikipedia passages
- Benchmarks LLM performance under conflicting evidence
- **Sprachspiel implication:** Real-world contradiction benchmark; our facts are shorter/structured (easier)

#### HaluMem (2025)
- First operation-level hallucination benchmark for memory systems
- Three tasks: memory extraction, updating, question answering
- **Sprachspiel implication:** Memory updating (where contradictions occur) is a known hallucination vector

#### Tosun & Buldur (2026) — Beyond Cosine Similarity
- Antonym intrusion is language-agnostic (observed in Turkish + English)
- Cosine similarity cannot distinguish semantic drift from genuine synonymy
- **Sprachspiel implication:** Confirms our two-step approach (embeddings + triples) is the correct architecture

#### T3 — RAG over Thinking Traces (Arabzadeh et al. 2026)

- **Paper:** arXiv:2605.03344 — "RAG over Thinking Traces Can Improve Reasoning Tasks"
- **Key findings:**
  - Thinking traces are a superior RAG corpus for reasoning tasks vs. conventional documents
  - General-purpose corpora frequently HURT performance on reasoning tasks
  - T3 method: Struct (7-step cheatsheet), Semantic (3-level distillation), Reflect (failure patterns)
  - Gemini-2.5-Flash on AIME: 53.3% → 83.3% (+56.3%) with T3-Semantic
  - 5-15x compression of raw traces; optimal k=3 retrieval; quality > quantity
- **Sprachspiel implication (P0-CRITICAL):**
  - `strip_thinking_tags()` permanently deletes thinking content before storage (~80% of traces lost)
  - Pre-tool messages store thinking inline (accidental); normal messages delete it entirely
  - **T3 Phase 0:** Preserve thinking in `thinking_content` column (bug fix, #151)
  - **T3 Phase 1:** Struct transform + ThinkingTrace pipeline (#152)
  - **T3 Phase 2:** Thinking-aware retrieval with RRF fusion (#153 + #137)
  - **T3 Phase 3:** Semantic/Reflect transforms, facts from Reflect (M3)
- **Hardware:** CPU-only fallback (LFM 2.5 1.2B) for transforms; same-model cascade when loaded

#### PEEK — Context Map as Orientation Cache (Gu et al. 2026)

- **Paper:** arXiv:2605.19932
- **Key findings:** +6.3–34.0% quality gains on reasoning/aggregation tasks with constant map
- **Sprachspiel implication:** Orientation Cache (OC-1a through OC-3) replaces AGENTS.md static injection with dynamic, session-persistent context map. See R-21 in research-icebox.md.

#### RLM — Recursive Language Models (Zhang et al. 2025)

- **Paper:** arXiv:2512.24601
- **Key findings:** Sub-agents with isolated context preserve 100% of information vs. compaction which loses details. But slower (2-5x) and compaction remains inevitable for long history.
- **Sprachspiel implication:** Context-offload via sub-agent (R-26) resolves 1 of 3 context pressure sources. Session variables (R-26 §4) add on-demand injection. Benchmark-driven validation required (B1.5). See RECURSION-SPRACHSPIEL.md analysis.

#### NLP Historical Errors — Cultural Grounding (Diógenes et al. 2026)

- **Paper:** PRW-5188-2880
- **Key findings:** 
  - Models trained dominantly in English fail in Global South regionalisms and pragmatics
  - SOUL.md solves ~40-60% at linguistic register level, but not deep semantic loss
  - Invisibility principle: curatorial priority should follow how invisible errors are to the model
  - Empathy ≠ failure: behavioral shifts are not bugs, opacity is
- **Sprachspiel implication:** Cultural Grounding (R-24, moved to M4). Empathy ≠ Failure principle orients S2.meta1-3 (#99/#100/#101). See unified-vision.md §8.

#### TurboQuant / RaBitQ — Norm Correction for Embeddings (Zandieh et al. 2026, Gao & Long 2024)

- **Papers:** ICLR 2026 (arXiv:2504.19874), SIGMOD 2024 (arXiv:2405.12497)
- **Key findings:** Scalar quantization systematically underestimates cosine similarity. 1 float per vector corrects the bias at zero query-time cost.
- **Sprachspiel implication:** Norm correction (R-25) as ~20-line Rust addendum to W4.x. Critical when d_eff < 0.7 (Matryoshka truncation). Prerequisite of TAP-2.

#### Passive Models as Middleware

- **Papers/Models:** BusyBeaver-50M (DJLougen/GestaltLabs), OpenAI Privacy Filter, LlamaFirewall (Meta), PII Shield, WebWorld (Qwen), Needle (Cactus-Compute), Dreamer4 (Hafner et al.)
- **Key findings:** Three archetypes: Classifiers (Privacy Filter, PromptGuard 2), Policy Models (BusyBeaver), World Simulators (WebWorld, Dreamer4). Small models (26M-50M) can route tools, detect PII, predict next state.
- **Sprachspiel implication:** Passive models as curatorial middleware (R-28), prioritized by invisibility (NLP-Historical §9.7): Confidence scorer > Pragmatics classifier > Calque detector > TTR monitor. Requires plugin system (#15).

#### Translation Models — Cultural Fragility Canary

- **Models:** Hy-MT2-1.8B (Tencent), TranslateGemma-4B (Google)
- **Key findings:** Even specialized translation models fail on pt-BR slang. Hy-MT2: literal translations. TranslateGemma: better but imperfect. If even these fail, general models fail more.
- **Sprachspiel implication:** Translation fleet as canary test for cultural fragility (R-27). Not a code feature — a testing pattern that guides where SOUL.md needs patches.

#### Gated DeltaNet-2 (Hatamizadeh et al. 2026)

- **Paper:** arXiv:2605.22791
- **Key findings:** Decoupled gates (separate forget gate and update gate) outperform monolithic gates in linear attention. Fine-grained, independent control over what to forget vs. what to update provides better information routing than a single combined gate.
- **Sprachspiel implication:** Informs multi-head retrieval design (#137) and multi-signal compaction (R-23). The key insight: **decoupled, fine-grained gates outperform monolithic ones** — applied to RRF fusion (separate weights per head) and compaction (separate signals for recency/relevance/importance instead of a single heuristic).

#### UniMem: Towards a Unified View of Memory Architectures (Fang et al. 2024)

- **Paper:** arXiv:2402.03009
- **Key findings:** Proposes a unified framework for understanding diverse memory architectures (attention, retrieval, compression, persistent storage) as instances of the same abstract operation with different hyperparameters.
- **Sprachspiel implication:** Validates the information routing abstraction (#179) — our compaction, RAG检索, and memory systems share the same underlying pattern. UniMem provides the academic framing for why thinking traces, facts, and context all benefit from the same gate/retain/evict pattern.

#### Pichay: Demand Paging for LLM Context (Mason 2026)

- **Paper:** arXiv:2603.09023
- **Key findings:** LLM context can be managed like virtual memory, with demand paging bringing in information on demand. The cost model (latency of page fault vs. latency of lost information) directly informs the offload vs. compaction tradeoff.
- **Sprachspiel implication:** Validates the session variables concept (R-26, RLM §4) and the B1.5 benchmark design (#158) — demand paging is another "Gate mechanism" in the information routing abstraction, routing information between "in-context" and "on-disk" capacity levels.

#### Titans: Learning to Memorize at Test Time (Behrouz et al. 2024)

- **Paper:** arXiv:2501.00663
- **Key findings:** Neural memory with learned gates that decide what to store, what to forget, and what to retrieve. The surprise metric (how unexpected new information is) drives the gate function.
- **Sprachspiel implication:** TAP-Reflect (R-22) is analogous to Titans' learned gate — extracting "what was surprising" from thinking traces. The surprise metric maps to TAP-Reflect's failure pattern extraction. Our compaction quality metric (R-30) serves a similar purpose: measuring how much "memorable" information was retained vs. discarded.

#### Context Cartography (Wu & Gartner 2026)

- **Paper:** arXiv:2603.20578
- **Key findings:** LLMs have predictable "attention deserts" — regions of context that receive systematically less attention. The middle of context is especially neglected. Mapping these deserts enables strategic information placement.
- **Sprachspiel implication:** Validates our middle-compaction strategy (keep first N + last N). Informs R-04 (attention-based prompt optimization) and R-29 (information routing mapping) — attention distribution is part of the Capacity allocation in the routing abstraction.

## Related Blog Posts

- **Unsloth/NVIDIA**: "Reinforcement Learning environments and how to build them" (March 2026)
  - https://unsloth.ai/blog/rl-environments

## Citation

```bibtex
@article{memos2025,
  title={MemOS: A Memory OS for AI System},
  author={Li, Zhiyu and Xi, Chenyang and Li, Chunyu and others},
  journal={arXiv preprint arXiv:2507.03724},
  year={2025}
}

@article{openclaw2026,
  title={OpenClaw-RL: Train Any Agent Simply by Talking},
  author={Wang, Yinjie and Chen, Xuyang and Jin, Xiaolong and Wang, Mengdi and Yang, Ling},
  journal={arXiv preprint arXiv:2603.10165},
  year={2026}
}

@article{memgpt2023,
  title={MemGPT: Towards LLMs as Operating Systems},
  author={Packer, Charles and Wooders, Sarah and Lin, Kevin and others},
  journal={arXiv preprint arXiv:2310.08560},
  year={2023}
}

@article{li2017contradiction,
  title={Contradiction Detection with Contradiction-Specific Word Embedding},
  author={Li, Qin and Liu, Yang},
  journal={Information},
  volume={10},
  number={2},
  pages={59},
  year={2017},
  doi={10.3390/info10020059}
}

@article{gokul2025contradiction,
  title={Contradiction Detection in RAG Systems: Evaluating LLMs as Context Validators},
  author={Gokul, Tenneti and Nakkiran},
  journal={arXiv preprint arXiv:2504.00180},
  year={2025}
}

@article{cattan2025conflicts,
  title={DRAGged into Conflicts: Detecting and Addressing Conflicting Sources in Search-Augmented LLMs},
  author={Cattan, Arie and others},
  journal={arXiv preprint arXiv:2506.08500},
  year={2025}
}

@article{longmemeval2024,
  title={LongMemEval: Benchmarking Chat Assistants on Long-Term Interactive Memory},
  author={Wu, Di and others},
  journal={arXiv preprint arXiv:2410.10813},
  year={2024}
}

@article{wikicontradict2024,
  title={WikiContradict: A Benchmark for Evaluating LLMs on Real-World Contradictory Knowledge},
  author={Wasserblat, Moshe and others},
  journal={arXiv preprint arXiv:2406.13805},
  year={2024}
}

@article{halumem2025,
  title={HaluMem: Evaluating Hallucinations in Memory Systems of Agents},
  author={MemTensor team},
  journal={arXiv preprint arXiv:2511.03506},
  year={2025}
}

@article{tosun2026beyond,
  title={Beyond Cosine Similarity: Taming Semantic Drift and Antonym Intrusion in a 15-Million Node Turkish Synonym Graph},
  author={Tosun, Ebubekir and Buldur, Mehmet Emin},
  journal={arXiv preprint arXiv:2601.13251},
  year={2026}
}

@article{sparsecl2025,
  title={Sparse Contrastive Learning for Contradiction Retrieval},
  journal={arXiv preprint arXiv:2406.10746},
  year={2025}
}

@article{t3_2026,
  title={RAG over Thinking Traces Can Improve Reasoning Tasks},
  author={Arabzadeh, Negar and Ma, Wentai and Min, Sewon and Zaharia, Matei},
  journal={arXiv preprint arXiv:2605.03344},
  year={2026}
}

@article{peek_2026,
  title={PEEK: Context Map as an Orientation Cache for Long-Context LLM Agents},
  author={Gu, Zijian and others},
  journal={arXiv preprint arXiv:2605.19932},
  year={2026}
}

@article{rlm_2025,
  title={Recursive Language Models},
  author={Zhang, Xiang and Kraska, Tim and Khattab, Omar},
  journal={arXiv preprint arXiv:2512.24601},
  year={2025}
}

@article{nlp_historical_2026,
  title={Evolu\c{c}\~ao do uso de tags e marcadores em Processamento de Linguagem Natural (PLN)},
  author={Di\'ogenes, F.H.P. and Souza, I.O. and Guelpeli, M.V.C.},
  journal={Peer Review, PRW-5188-2880},
  year={2026},
  doi={10.53660/PRW-5188-2880}
}

@article{turboquant_2026,
  title={TurboQuant: Productive Quantization for Vector Search},
  author={Zandieh, Amin and others},
  journal={ICLR 2026},
  year={2026},
  note={arXiv:2504.19874}
}

@article{rabitq_2024,
  title={RaBitQ: Quantization for Vector Search},
  author={Gao, Jianhao and Long, Cheng},
  journal={SIGMOD 2024},
  year={2024},
  note={arXiv:2405.12497}
}

@article{fademem_2026,
  title={FadeMem: Dual-Layer Ebbinghaus Decay},
  journal={arXiv:2601.18642},
  year={2026}
}

@article{llamafirewall_2025,
  title={LlamaFirewall: An Open Source Guardrail System for Building Secure AI Agents},
  author={Chennabasappa, Sahana and Nikolaidis, Cyrus and Song, Daniel and Molnar, David and others},
  journal={arXiv preprint arXiv:2505.03574},
  year={2025}
}

@article{piishield_2026,
  title={PII Shield: A Browser-Level Overlay for User-Controlled Personal Identifiable Information (PII) Management in AI Interactions},
  author={Holschneider, Max and LeeYouk, Saetbyeol},
  journal={arXiv preprint arXiv:2603.24895},
  note={Accepted at CHI 2026 Workshop: Ethics at the Front-End},
  year={2026}
}

@article{guran2024middleware,
  title={Towards a Middleware for Large Language Models},
  author={Guran, Narcisa and Knauf, Florian and Ngo, Man and Petrescu, Stefan and Rellermeyer, Jan S.},
  journal={arXiv preprint arXiv:2411.14513},
  year={2024}
}

@article{webworld_2026,
  title={WebWorld: World Model for Web Agents},
  author={Xiao, Zikai and Tu, Jianhong and Zou, Chuhang and others},
  journal={arXiv preprint arXiv:2602.14721},
  year={2026}
}

@article{dreamer4_2025,
  title={Mastering Diverse Domains through World Models},
  author={Hafner, Danijar and Yan, Wilson and Lillicrap, Timothy},
  journal={arXiv preprint arXiv:2509.24527},
  year={2025}
}

@article{shaukat2026chunking,
  title={Document Chunking Strategies},
  author={Shaukat, H. and others},
  journal={arXiv:2603.06976},
  year={2026}
}

@article{antoinelli2025complexity,
  title={Desafios de grandes modelos de linguagem generativa na reprodu\c{c}\~ao de complexidade textual},
  author={Antonelli, A.L.},
  journal={Texto Livre},
  year={2025}
}

@article{bender2021parrots,
  title={On the Dangers of Stochastic Parrots: Can Language Models Be Too Big?},
  author={Bender, E.M. and Gebru, T. and McMillan-Major, A. and Shmitchell, S.},
  journal={FAccT '21},
  year={2021}
}

@article{nunes2008nlp,
  title={Processamento de l\^\{i\}nguas naturais: para qu\^e e para quem?},
  author={Nunes, M.G.V.},
  journal={EBLC},
  year={2008}
}

@article{gated_deltanet_2026,
  title={Gated DeltaNet-2},
  author={Hatamizadeh, Ali and others},
  journal={arXiv preprint arXiv:2605.22791},
  year={2026}
}

@article{unimem_2024,
  title={UniMem: Towards a Unified View of Memory Architectures},
  author={Fang, Xiang and others},
  journal={arXiv preprint arXiv:2402.03009},
  year={2024}
}

@article{pichay_2026,
  title={Pichay: Demand Paging for LLM Context},
  author={Mason, David},
  journal={arXiv preprint arXiv:2603.09023},
  year={2026}
}

@article{titans_2024,
  title={Titans: Learning to Memorize at Test Time},
  author={Behrouz, Ali and Delavari, Pezhman and Bighash, Simin},
  journal={arXiv preprint arXiv:2501.00663},
  year={2024}
}

@article{context_cartography_2026,
  title={Context Cartography: Mapping Attention Deserts in LLM Context},
  author={Wu, Zhiyuan and Gartner, Jan},
  journal={arXiv preprint arXiv:2603.20578},
  year={2026}
}
```