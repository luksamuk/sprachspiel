# Research Papers Reference

Papers that informed the Implementation Directive. **PDFs are not stored in the repository** - use the arXiv links below.

## Papers

| Paper | Authors | arXiv | Year |
|-------|---------|-------|------|
| **MemOS: A Memory OS for AI System** | Li et al. (38 authors) | [arXiv:2507.03724](https://arxiv.org/abs/2507.03724) | 2025 |
| **OpenClaw-RL: Train Any Agent Simply by Talking** | Wang et al. (5 authors) | [arXiv:2603.10165](https://arxiv.org/abs/2603.10165) | 2026 |
| **MemGPT: Towards LLMs as Operating Systems** | Packer et al. (7 authors) | [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) | 2023 |

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
```