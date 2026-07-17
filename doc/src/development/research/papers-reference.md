# Research Papers Reference

Papers that informed the Implementation Directive. **PDFs are not stored in the repository** - use the arXiv links below.

## Papers

| Paper | Authors | arXiv | Year |
|-------|---------|-------|------|
| **MemOS: A Memory OS for AI System** | Li et al. (38 authors) | [arXiv:2507.03724](https://arxiv.org/abs/2507.03724) | 2025 |
| **OpenClaw-RL: Train Any Agent Simply by Talking** | Wang et al. (5 authors) | [arXiv:2603.10165](https://arxiv.org/abs/2603.10165) | 2026 |
| **MemGPT: Towards LLMs as Operating Systems** | Packer et al. (7 authors) | [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) | 2023 |
| **Contradiction Detection with Contradiction-Specific Word Embedding** | Li, Luyang; Qin, Bing; Liu, Ting | [DOI: 10.3390/a10020059](https://www.mdpi.com/1999-4893/10/2/59) | 2017 |
| **Contradiction Detection in RAG Systems** | Gokul, Vignesh; Tenneti, Srikanth; Nakkiran, Alwarappan | [arXiv:2504.00180](https://arxiv.org/abs/2504.00180) | 2025 |
| **DRAGged into Conflicts (CONFLICTS benchmark)** | Cattan et al. | [arXiv:2506.08500](https://arxiv.org/abs/2506.08500) | 2025 |
| **LongMemEval: Benchmarking Chat Assistants on Long-Term Interactive Memory** | Wu et al. | [arXiv:2410.10813](https://arxiv.org/abs/2410.10813) | 2024 |
| **WikiContradict: A Benchmark for Evaluating LLMs on Real-World Knowledge Conflicts from Wikipedia** | Hou et al. (8 authors) | [arXiv:2406.13805](https://arxiv.org/abs/2406.13805) | 2024 |
| **HaluMem: Evaluating Hallucinations in Memory Systems of Agents** | Chen, Ding et al. (9 authors) | [arXiv:2511.03506](https://arxiv.org/abs/2511.03506) | 2025 |
| **Beyond Cosine Similarity: Antonym Intrusion in Synonym Graphs** | Tosun & Buldur | [arXiv:2601.13251](https://arxiv.org/abs/2601.13251) | 2026 |
| **On the Theoretical Limitations of Embedding-Based Retrieval** | Weller, Boratko et al. | [arXiv:2508.21038](https://arxiv.org/abs/2508.21038) | 2025 |
| **How Small Transformation Expose the Weakness of Semantic Similarity Measures** | Nikiema et al. | [arXiv:2509.09714](https://arxiv.org/abs/2509.09714) | 2025 |
| **SparseCL: Sparse Contrastive Learning for Contradiction Retrieval** | Xu et al. | [arXiv:2406.10746](https://arxiv.org/abs/2406.10746) | 2024 |
| **RAG over Thinking Traces Can Improve Reasoning Tasks (T3)** | Arabzadeh, Ma, Min, Zaharia | [arXiv:2605.03344](https://arxiv.org/abs/2605.03344) | 2026 |
| **PEEK: Context Map as an Orientation Cache for Long-Context LLM Agents** | Gu et al. | [arXiv:2605.19932](https://arxiv.org/abs/2605.19932) | 2026 |
| **Gated DeltaNet-2: Decoupling Erase and Write in Linear Attention** | Hatamizadeh, Choi, Kautz | [arXiv:2605.22791](https://arxiv.org/abs/2605.22791) | 2026 |
| **UniMem: Towards a Unified View of Long-Context Large Language Models** | Fang et al. (15 authors) | [arXiv:2402.03009](https://arxiv.org/abs/2402.03009) | 2024 |
| **The Missing Memory Hierarchy: Demand Paging for LLM Context Windows** | Mason, Tony | [arXiv:2603.09023](https://arxiv.org/abs/2603.09023) | 2026 |
| **Titans: Learning to Memorize at Test Time** | Behrouz et al. | [arXiv:2501.00663](https://arxiv.org/abs/2501.00663) | 2024 |
| **Context Cartography: Toward Structured Governance of Contextual Space** | Wu, Zihua; Gartner, Georg | [arXiv:2603.20578](https://arxiv.org/abs/2603.20578) | 2026 |
| **Aha Moment Revisited: Are VLMs Truly Capable of Self Verification in Inference-time Scaling?** | Wu, Li, Yang, Jiang, Yan, Li, Yu, Zhang & Nahrstedt | [arXiv:2506.17417](https://arxiv.org/abs/2506.17417) | 2025 |
| **Self-Verification Dilemma: Experience-Driven Suppression of Overused Checking in LLM Reasoning** | Long, Jiang, Chen, Guo, Gan & Wang | [arXiv:2602.03485](https://arxiv.org/abs/2602.03485) | 2026 |
| **Diverse Inference and Verification for Advanced Reasoning** | Drori, Longhitano, Mao, Hyun, Zhang, Park, Meeks, Zhang, Segev, Yong, Verma, Shporer, Amit, Udell | [arXiv:2502.09955](https://arxiv.org/abs/2502.09955) | 2025 |

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

#### Gokul, Vignesh; Tenneti, Srikanth; Nakkiran, Alwarappan (2025) — Contradiction Detection in RAG
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

#### TurboQuant / RaBitQ — Norm Correction for Embeddings (Zandieh et al. 2025, Gao & Long 2024)

- **Papers:** arXiv:2504.19874 (2025), SIGMOD 2024 (arXiv:2405.12497)
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

#### UniMem: Towards a Unified View of Long-Context Large Language Models (Fang et al. 2024)

- **Paper:** arXiv:2402.03009
- **Key findings:** Proposes a unified framework for understanding diverse memory architectures (attention, retrieval, compression, persistent storage) as instances of the same abstract operation with different hyperparameters.
- **Sprachspiel implication:** Validates the information routing abstraction (#179) — our compaction, RAG检索, and memory systems share the same underlying pattern. UniMem provides the academic framing for why thinking traces, facts, and context all benefit from the same gate/retain/evict pattern.

#### The Missing Memory Hierarchy (Mason 2026)

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

#### Aha Moment Revisited (Wu et al. 2025)

- **Paper:** arXiv:2506.17417 — "Aha Moment Revisited: Are VLMs Truly Capable of Self Verification in Inference-time Scaling?"
- **Key findings:** Simple majority voting consistently and substantially outperforms verification-centric strategies such as best-of-N with self-verification. Self-verification up to -16.7% worse than baseline. Recovery rate from self-correction attempts: only 2.7–19.5%. Visual information is not effectively integrated into self-verification.
- **Sprachspiel implication:** ADR-004 — LLM self-feedback discounted to 30% weight. LLM self-verification is an unreliable signal; user feedback is the ground truth.

#### Self-Verification Dilemma (Long et al. 2026)

- **Paper:** arXiv:2602.03485 — "Self-Verification Dilemma: Experience-Driven Suppression of Overused Checking in LLM Reasoning"
- **Key findings:** A substantial fraction of reflective steps consist of self-verification (recheck) that repeatedly confirm intermediate results. These rechecks rarely identify errors or alter reasoning outcomes — the vast majority are confirmatory rather than corrective. Reducing overused verification saves up to 20.3% tokens while maintaining accuracy.
- **Sprachspiel implication:** ADR-004 — reinforces the 30% discount on LLM self-feedback. Verification steps rarely change outcomes, confirming that LLM self-approval is an unreliable quality signal. Also informs TAP-Reflect design: filter out confirmatory rechecks from thinking traces.

#### Diverse Inference and Verification (Drori et al. 2025)

- **Paper:** arXiv:2502.09955 — "Diverse Inference and Verification for Advanced Reasoning"
- **Key findings:** Strict binary verification (Lean formal proofs for math, code execution for ARC puzzles) provides unambiguous 0/1 correctness signals. A proof either type-checks or it doesn't; code either produces the correct output or it doesn't. This strict verification combined with rejection sampling and RL with inference feedback significantly improves reasoning: IMO combinatorics 33.3% → 77.8%.
- **Sprachspiel implication:** ADR-005 — validates binary Good/Bad feedback signals (±1.0) with no partial credit. The strict verification paradigm confirms that granularity should come from temporal decay, not from base_value magnitude.

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
  author={Li, Luyang and Qin, Bing and Liu, Ting},
  journal={Algorithms},
  volume={10},
  number={2},
  pages={59},
  year={2017},
  doi={10.3390/a10020059}
}

@article{gokul2025contradiction,
  title={Contradiction Detection in RAG Systems: Evaluating LLMs as Context Validators for Improved Information Consistency},
  author={Gokul, Vignesh and Tenneti, Srikanth and Nakkiran, Alwarappan},
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

@article{hou2024wikicontradict,
  title={WikiContradict: A Benchmark for Evaluating LLMs on Real-World Knowledge Conflicts from Wikipedia},
  author={Hou, Yufang and Pascale, Alessandra and Carnerero-Cano, Javier and Tchrakian, Tigran and Marinescu, Radu and Daly, Elizabeth and Padhi, Inkit and Sattigeri, Prasanna},
  journal={arXiv preprint arXiv:2406.13805},
  year={2024}
}

@article{chen2025halumem,
  title={HaluMem: Evaluating Hallucinations in Memory Systems of Agents},
  author={Chen, Ding and Niu, Simin and Li, Kehang and Liu, Peng and Zheng, Xiangping and Tang, Bo and Li, Xinchi and Xiong, Feiyu and Li, Zhiyu},
  journal={arXiv preprint arXiv:2511.03506},
  year={2025}
}

@article{tosun2026beyond,
  title={Beyond Cosine Similarity: Taming Semantic Drift and Antonym Intrusion in a 15-Million Node Turkish Synonym Graph},
  author={Tosun, Ebubekir and Buldur, Mehmet Emin and Ezerceli, {\"O}zay and ElHussieni, Mahmoud},
  journal={arXiv preprint arXiv:2601.13251},
  year={2026}
}

@article{xu2024sparsecl,
  title={SparseCL: Sparse Contrastive Learning for Contradiction Retrieval},
  author={Xu, Haike and Lin, Zongyu and Sun, Yizhou and Chang, Kai-Wei and Indyk, Piotr},
  journal={arXiv preprint arXiv:2406.10746},
  year={2024}
}

@article{arabzadeh2026t3,
  title={RAG over Thinking Traces Can Improve Reasoning Tasks},
  author={Arabzadeh, Negar and Ma, Wenjie and Min, Sewon and Zaharia, Matei},
  journal={arXiv preprint arXiv:2605.03344},
  year={2026}
}

@article{gu2026peek,
  title={PEEK: Context Map as an Orientation Cache for Long-Context LLM Agents},
  author={Gu, Zhuohan and Zhang, Qizheng and Khattab, Omar and Madden, Samuel},
  journal={arXiv preprint arXiv:2605.19932},
  year={2026}
}

@article{zhang2025recursive,
  title={Recursive Language Models},
  author={Zhang, Alex L. and Kraska, Tim and Khattab, Omar},
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

@article{zandieh2025turboquant,
  title={TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate},
  author={Zandieh, Amir and Daliri, Majid and Hadian, Majid and Mirrokni, Vahab},
  journal={arXiv preprint arXiv:2504.19874},
  year={2025}
}

@article{gao2024rabitq,
  title={RaBitQ: Quantizing High-Dimensional Vectors with a Theoretical Error Bound for Approximate Nearest Neighbor Search},
  author={Gao, Jianyang and Long, Cheng},
  journal={SIGMOD 2024},
  year={2024},
  note={arXiv:2405.12497}
}

@article{wei2026fademem,
  title={FadeMem: Biologically-Inspired Forgetting for Efficient Agent Memory},
  author={Wei, Lei and Peng, Xiao and Dong, Xu and Xie, Niantao and Wang, Bin},
  journal={arXiv preprint arXiv:2601.18642},
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

@article{xiao2026webworld,
  title={WebWorld: A Large-Scale World Model for Web Agent Training},
  author={Xiao, Zikai and Tu, Jianhong and Zou, Chuhang and Zuo, Yuxin and Li, Zhi and Wang, Peng and Yu, Bowen and Huang, Fei and Lin, Junyang and Liu, Zuozhu},
  journal={arXiv preprint arXiv:2602.14721},
  year={2026}
}

@article{hafner2025dreamer,
  title={Training Agents Inside of Scalable World Models},
  author={Hafner, Danijar and Yan, Wilson and Lillicrap, Timothy},
  journal={arXiv preprint arXiv:2509.24527},
  year={2025}
}

@article{shaukat2026chunking,
  title={A Systematic Investigation of Document Chunking Strategies and Embedding Sensitivity},
  author={Shaukat, Muhammad Arslan and others},
  journal={arXiv preprint arXiv:2603.06976},
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

@article{hatamizadeh2026gateddeltanet2,
  title={Gated DeltaNet-2: Decoupling Erase and Write in Linear Attention},
  author={Hatamizadeh, Ali and Choi, Yejin and Kautz, Jan},
  journal={arXiv preprint arXiv:2605.22791},
  year={2026}
}

@article{fang2024unimem,
  title={UniMem: Towards a Unified View of Long-Context Large Language Models},
  author={Fang, Junjie and Tang, Likai and Bi, Hongzhe and Qin, Yujia and Sun, Si and Li, Zhenyu and Li, Haolun and Li, Yongjian and Cong, Xin and Lin, Yankai and Yan, Yukun and Shi, Xiaodong and Song, Sen and Liu, Zhiyuan and Sun, Maosong},
  journal={arXiv preprint arXiv:2402.03009},
  year={2024},
  note={COLM 2024}
}

@article{mason2026missing,
  title={The Missing Memory Hierarchy: Demand Paging for LLM Context Windows},
  author={Mason, Tony},
  journal={arXiv preprint arXiv:2603.09023},
  year={2026}
}

@article{behrouz2024titans,
  title={Titans: Learning to Memorize at Test Time},
  author={Behrouz, Ali and Zhong, Peilin and Mirrokni, Vahab},
  journal={arXiv preprint arXiv:2501.00663},
  year={2024}
}

@article{wu2026cartography,
  title={Context Cartography: Toward Structured Governance of Contextual Space in Large Language Model Systems},
  author={Wu, Zihua and Gartner, Georg},
  journal={arXiv preprint arXiv:2603.20578},
  year={2026}
}

@article{aha_moment_revisited_2025,
  title={Aha Moment Revisited: Are VLMs Truly Capable of Self Verification in Inference-time Scaling?},
  author={Wu, Mingyuan and Li, Meitang and Yang, Jingcheng and Jiang, Jize and Yan, Kaizhuo and Li, Zhaoheng and Yu, Hanchao and Zhang, Minjia and Nahrstedt, Klara},
  journal={arXiv preprint arXiv:2506.17417},
  year={2025},
  note={NeurIPS 2025 Multimodal Algorithmic Reasoning Workshop Oral}
}

@article{self_verification_dilemma_2026,
  title={Self-Verification Dilemma: Experience-Driven Suppression of Overused Checking in LLM Reasoning},
  author={Long, Quanyu and Jiang, Kai Jie and Chen, Jianda and Guo, Xu and Gan, Leilei and Wang, Wenya},
  journal={arXiv preprint arXiv:2602.03485},
  year={2026}
}

@article{drori2025diverse,
  title={Diverse Inference and Verification for Advanced Reasoning},
  author={Drori, Iddo and Longhitano, Gaston and Mao, Mao and Hyun, Seunghwan and Zhang, Yuke and Park, Sungjun and Meeks, Zachary and Zhang, Xin-Yu and Segev, Ben and Yong, Howard and Verma, Nakul and Shporer, Avi and Amit, Alon and Udell, Madeleine},
  journal={arXiv preprint arXiv:2502.09955},
  year={2025}
}

@article{liu2023lostinmiddle,
  title={Lost in the Middle: How Language Models Use Long Contexts},
  author={Liu, Nelson F. and Lin, Kevin and Hewitt, John and Paranjape, Ashwin and Bevilacqua, Michele and Petroni, Fabio and Liang, Percy},
  journal={arXiv preprint arXiv:2307.03172},
  year={2023},
  note={TACL 2023}
}

@article{cuconasu2024trust,
  title={A Tale of Trust and Accuracy: Base vs. Instruct LLMs in RAG Systems},
  author={Cuconasu, Florin and Trappolini, Giovanni and Tonellotto, Nicola and Silvestri, Fabrizio},
  journal={arXiv preprint arXiv:2406.14972},
  year={2024}
}

@article{weller2025limitations,
  title={On the Theoretical Limitations of Embedding-Based Retrieval},
  author={Weller, Orion and Boratko, Michael and Naim, Iftekhar and Lee, Jinhyuk},
  journal={arXiv preprint arXiv:2508.21038},
  year={2025}
}

@article{nikiema2025transformation,
  title={How Small Transformation Expose the Weakness of Semantic Similarity Measures},
  author={Nikiema, Serge Lionel and Djire, Albérick Euraste and Bonkoungou, Abdoul Aziz and Moumoula, Micheline Bénédicte and Samhi, Jordan and Kabore, Abdoul Kader and Klein, Jacques and Bissyande, Tegawendé F.},
  journal={arXiv preprint arXiv:2509.09714},
  year={2025}
}
```