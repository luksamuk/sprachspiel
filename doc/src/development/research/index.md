# Research Background

This directory contains research documents that informed the development of Sprachspiel's continuous learning system.

## Documents

| Document | Description |
|----------|-------------|
| [Papers Reference](./papers-reference.md) | arXiv links and citations for MemOS, OpenClaw-RL, MemGPT, and contradiction detection papers |
| [Research Synthesis](./research-appendix.md) | Complete synthesis of all research |
| [OpenClaw-RL Analysis](./openclaw-rl-analysis.md) | Original analysis of OpenClaw-RL framework |
| [Effective Agents Analysis](./effective-agents-analysis.md) | Research on effective agent architectures |
| [Context Management Research](./context_management_research.md) | Research on context management approaches |
| [OpenAI Streaming Tool Calls](./openai-streaming-tool-calls.md) | Investigation of tool call streaming differences between Ollama native (`/api/chat`) and OpenAI-compatible (`/v1/chat/completions`) — informs #122 design |

## Research Summary

The research in this directory led to the **Implementation Directive** for continuous learning:

1. **Memory is a managed resource** (MemOS, MemGPT)
   - Not passive storage, but active lifecycle management
   
2. **Interactions contain learning signals** (OpenClaw-RL, Unsloth)
   - Evaluative (good/bad) and directive (how to improve) signals
   
3. **Temporal decay improves relevance** (CortexGraph, Ebbinghaus)
   - Memories fade exponentially unless reinforced
   
4. **Pseudo-RL works for local-first** (Our synthesis)
   - Weight-based learning without GPU fine-tuning

## Key Papers

| Paper | Source | Key Insight |
|-------|--------|-------------|
| MemOS | [arXiv:2507.03724](https://arxiv.org/abs/2507.03724) | Memory as manageable system resource |
| OpenClaw-RL | [arXiv:2603.10165](https://arxiv.org/abs/2603.10165) | Next-state signals are universal learning sources |
| MemGPT | [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) | Virtual context management |

## Canonical Document

For implementation guidance based on this research, see:

**[Implementation Directive](../implementation-directive.md)** - The definitive implementation direction.