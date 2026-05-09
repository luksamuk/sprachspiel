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
```