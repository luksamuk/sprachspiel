# Evaluated Models

Models evaluated for local inference on **RTX 3050 Laptop 6GB VRAM**, served via [llama-swap](https://github.com/mostlygeeksllc/llama-swap) (an OpenAI-compatible model swap manager for llama.cpp). These models were benchmarked using Sprachspiel's OpenAI-compatible provider.

> **Note:** These are the models *we* tested. Sprachspiel accepts any OpenAI-compatible backend — you are not limited to these models or to llama-swap. See [Available Models](../models.md) for how to configure your own.

These models have been benchmarked for tool calling, reasoning, vision, and specialized tasks. This catalog tracks what we've tested, what works, and what doesn't.

## Evaluation Status

- ✅ Running in llama-swap, tested via OpenAI API
- ⏳ GGUF available, not yet in llama-swap
- ❌ Failed / incompatible
- 🔜 Not yet evaluated

## Sprachspiel Sub-Agent Roles

Models are assigned to sub-agent roles based on their strengths. Current role
taxonomy:

| Role | Requirements | Best Candidates |
|------|-------------|-----------------|
| **Coordinator** | Tool calling, reasoning, JSON compliance | lfm2.5-230m, lfm2.5-8b-a1b, qwen3.5-4b, qwen3.5-9b |
| **Coder** | Code generation, tool calling, agentic loops | ornith-1.0-35b, agents-a1-35b, qwen3.6-35b-a3b, north-mini-code |
| **Translator** | Translation quality, multi-language | hy-mt2-1.8b, translategemma-4b |
| **Vision** | Image understanding, OCR | lfm2.5-vl-450m, lfm2.5-vl-1.6b, minicpm-v-4.6, qwen3-vl-4b, glm-ocr |
| **Reasoner** | Deep thinking, math, logic | gemma4-e4b, qwen3.5-9b:think, glm-4.7-flash |
| **Lightweight** | Fast, low VRAM, simple tasks | lfm2.5-230m, lfm2.5-8b-a1b, gemma4-e2b |
| **Embedding** | Vector embeddings for retrieval | nomic-embed-text-v2-moe, lfm2.5-embedding-350m |

---

## Models

### gemma4-e4b — Gemma 4 E4B Dense

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/gemma-4-E4B-it-GGUF](https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF) |
| License | Gemma Terms of Use |
| Architecture | Gemma4ForCausalLM (gemma4) — dense 4B |
| Params | 4B |
| Quant | UD-Q3_K_XL Dynamic 2.0 (~4.5 GB) |
| Context | 64K–128K (dynamic) |
| Backend | ik_llama.cpp (v4542) — hadamard KV cache |
| KV cache | q8_0 (K) + q4_0 (V) + hadamard (-khad/-vhad) |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ✅ Yes |
| Vision | ✅ Yes (model supports vision; mmproj crash was specific to one CUDA build) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Reasoner** ✅ — Good reasoning quality at 4B, hadamard KV preserves cache quality
- **Coder** ⚠️ — Decent but Qwen3.6-35B-MoE is stronger for code
- ⚠️ E4B MTP OOMs on 6GB — no speculative decoding

**Recommended params:** temp=0.7/top-p=0.9 (chat), temp=0.6/top-p=0.9 (think)

---

### gemma4-e2b — Gemma 4 E2B Dense

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/gemma-4-E2B-it-GGUF](https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF) |
| License | Gemma Terms of Use |
| Architecture | Gemma4ForCausalLM (gemma4) — dense 2B |
| Params | 2B |
| Quant | UD-Q3_K_XL Dynamic 2.0 (~2.72 GB) |
| Context | 32K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) |
| KV cache | q4_0 + attn_rot (iSWA fix) |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ✅ Yes |
| Vision | ✅ Yes (model supports vision) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Lightweight Reasoner** — Small but capable
- ⚠️ E2B MTP is 3.6× slower than baseline — no speculative decoding benefit

**Recommended params:** temp=0.7/top-p=0.9 (chat), temp=0.6/top-p=0.9 (think)

---

### qwen3.6-35b-a3b — Qwen3.6 35B A3B MoE (Primary Coder)

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [mudler/Qwen3.6-35B-A3B-APEX-GGUF](https://huggingface.co/mudler/Qwen3.6-35B-A3B-APEX-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen35moe) — MoE 35B, 33 experts, top-8 routing, 3B active/token |
| Params | 35B total / 3B active per token |
| Quant | APEX I-Compact (~17.3 GB) — mixed-precision MoE offline |
| Context | 16K–128K (dynamic) |
| Backend | ik_llama.cpp (v4542) — pinned memory + hadamard |
| KV cache | q4_0 K + q4_0 V + hadamard + attn_rot |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ✅ Yes (--parallel-tool-calls) |
| Vision | ✅ Yes (Qwen3.6 is multimodal) |
| attn_rot | ✅ head_dim=256 |
| MTP | ⚠️ NOT active — APEX GGUFs lack MTP tensors, and MTP OOMs on 6GB |

**Sprachspiel Sub-Agent Fit:**
- **Coder** ✅✅ — Primary coding model, strong tool calling, agentic loops
- **Reasoner** ✅ — 3B active params with expert routing for reasoning
- ⚠️ Heavy offload (~10GB RAM) — ~20 tok/s, use for quality not speed
- ❌ MTP not viable on 6GB (APEX ceiling)

**Recommended params:** temp=0.6/top-p=0.95 (code-optimized)

**Aliases:** "Lain" (primary Sprachspiel model via `lain` alias)

---

### qwen3.5-9b — Qwen3.5 9B Dense

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/Qwen3.5-9B-GGUF](https://huggingface.co/unsloth/Qwen3.5-9B-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen3.5) — dense 9B, Gated Delta Net |
| Params | 9B |
| Quant | UD-Q3_K_XL (~5.05 GB) |
| Context | Dynamic (--fit --fit-margin 512) |
| Backend | ik_llama.cpp (v4542) — hadamard KV cache |
| KV cache | q8_0 (K) + q4_0 (V) + hadamard |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ✅ Yes (--parallel-tool-calls) |
| Vision | ❌ No (mmproj exists but blocks partial expert offload) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Coordinator** ✅ — Strong tool calling, good reasoning
- **Reasoner** ✅ — 9B dense with thinking mode
- ⚠️ BeeLlama segfaults with this model — ik backend required

**Recommended params:** default_temp/top_p/top_k/min_p

---

### qwen3.5-4b — Qwen3.5 4B Dense (DFlash)

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/Qwen3.5-4B-GGUF](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) (abliterated i1-Q4_K_M) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen3.5) — dense 4B |
| Params | 4B |
| Quant | i1-Q4_K_M (~2.6 GB) + DFlash drafter Q4_K_M (~313 MB) |
| Context | 98304 (DFlash speculative decoding, BeeLlama backend) |
| Backend | BeeLama.cpp (v0.3.2) — DFlash speculative decoding |
| KV cache | q4_0 K + V |
| Thinking | ✅ Yes (dual mode: base + :think, reasoning_content field) |
| Tool Calling | ✅ Yes (parallel tool calls via Jinja template) |
| Vision | ❌ No |
| attn_rot | ✅ head_dim=256 |
| DFlash | flat DFlash (branch-budget=0), adaptive draft-max (profit), ~71% acceptance, GPU cross ring 5 layers x 512 slots |

**Sprachspiel Sub-Agent Fit:**
- **Coordinator** ✅ — Good tool calling, DFlash gives 1.64x speedup (86 tok/s)
- **Lightweight** ✅ — Fast decode with block diffusion drafting
- ⚠️ Bee only — no --fit, uses -ngl all. MoQ-3.75 incompatible (missing tokenizer merges)
- ⚠️ --parallel 1 only (DFlash requires single slot)

**Recommended params:** default_temp/top_p/top_k/min_p

**Benchmarks:**
- DFlash: 86 tok/s avg (vs 53 tok/s on ik_llama+MoQ-3.75) — 1.64x speedup
- Acceptance rate: ~71% (37/52 tokens accepted)
- GPU cross ring: 5 layers x 512 slots x 2560 embd

---

### lfm2.5-8b-a1b — LFM2.5 8B A1B MoE

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [LiquidAI/LFM2.5-8B-A1B-Instruct-GGUF](https://huggingface.co/LiquidAI/LFM2.5-8B-A1B-Instruct-GGUF) |
| License | LFM2 Community License |
| Architecture | lfm2 (Liquid AI) — hybrid MoE |
| Params | 8.3B total / 1.5B active per token |
| Quant | Q4_K_M (~4.8 GB) |
| Context | 32K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) |
| Thinking | ✅ Yes |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Coordinator** ✅ — Good balance MoE, strong tool calling with 1.5B active params
- ⚠️ MoE — 8.3B total weight, but only 1.5B active keeps inference fast

**Recommended params:** temp=0.3/top-p=0.9/min-p=0.15/repeat-penalty=1.05

---

### lfm2.5-230m — LFM2.5 230M Ultra-Small

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [LiquidAI/LFM2.5-230M-Instruct-GGUF](https://huggingface.co/LiquidAI/LFM2.5-230M-Instruct-GGUF) |
| License | LFM2 Community License |
| Architecture | lfm2 (Liquid AI) — hybrid (conv + GQA) |
| Params | 230M |
| Quant | Q8_0 (~0.3 GB) |
| Context | dynamic |
| Backend | llama.cpp upstream (v674) |
| Thinking | ❌ No |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Lightweight** ✅✅ — Ultra-light, fast tool calling, minimal VRAM (~0.3 GB)
- Designed for robotics and edge cases — works for tool calling but very small
- ⚠️ Very small — limited reasoning capability, best for simple tool loops

**Recommended params:** temp=0.3/top-p=0.9/min-p=0.15/repeat-penalty=1.05

---

### ornith-1.0-35b — Ornith 1.0 35B Agentic Coder

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [mudler/Ornith-1.0-35B-APEX-GGUF](https://huggingface.co/mudler/Ornith-1.0-35B-APEX-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen35moe) — MoE 35B, 33 experts, top-8 routing, 3B active/token |
| Params | 35B total / 3B active per token |
| Quant | APEX I-Compact (~16.5 GB) |
| Context | 262144 |
| Backend | ik_llama.cpp (v4542) |
| Thinking | ✅ Yes |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Coder** ✅✅ — Self-improving agentic coder with self-scaffolding RL
- **Reasoner** ✅ — 3B active params with expert routing
- ⚠️ Heavy offload — ~10GB RAM, slower than pure GPU models

**Recommended params:** temp=0.6/top-p=0.95 (code-optimized)

---

### agents-a1-35b — Qwen AgentWorld 35B

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [mudler/Agents-A1-APEX-GGUF](https://huggingface.co/mudler/Agents-A1-APEX-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen35moe) — MoE 35B, 33 experts, top-8 routing, 3B active/token |
| Params | 35B total / 3B active per token |
| Quant | APEX I-Compact (~16.5 GB) |
| Context | 131072 |
| Backend | ik_llama.cpp (v4542) |
| Thinking | ✅ Yes |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Description:** Qwen's native language world model. 35B params, 3B active. Simulates 7 agent environments (MCP, Search, Terminal, SWE, Android, Web, OS) via long chain-of-thought reasoning.

**Sprachspiel Sub-Agent Fit:**
- **Coder** ✅ — Long-horizon agentic tasks, simulates 7 agent environments
- **Reasoner** ✅ — Long chain-of-thought reasoning across agent environments
- ⚠️ Heavy offload — ~10GB RAM

**Recommended params:** temp=0.6/top-p=0.95

---

### glm-4.7-flash — GLM 4.7 Flash

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/GLM-4.7-Flash-GGUF](https://huggingface.co/unsloth/GLM-4.7-Flash-GGUF) |
| License | GLM License |
| Architecture | GLM — MLA MoE, 30B total, 3.6B active, 64 experts top-4 |
| Params | 30B total / 3.6B active per token |
| Quant | APEX I-Compact (~17 GB) |
| Context | 128K |
| Backend | ik_llama.cpp (v4542) |
| Thinking | ✅ Yes |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Benchmarks:** IFEval 71.71, BFCLv3 43.26

**Sprachspiel Sub-Agent Fit:**
- **Reasoner** ✅ — Fast reasoning with MoE efficiency
- **Coordinator** ✅ — Good tool calling (BFCLv3 43.26)
- ⚠️ Heavy offload — ~10GB RAM

**Recommended params:** temp=0.6/top-p=0.95

---

### north-mini-code — North Mini Code 1.0

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/North-Mini-Code-1.0-GGUF](https://huggingface.co/unsloth/North-Mini-Code-1.0-GGUF) |
| License | Apache 2.0 |
| Architecture | cohere2moe — MoE 30B, 3B active |
| Params | 30B total / 3B active per token |
| Quant | APEX (~18.6 GB) |
| Context | 131072 |
| Backend | ik_llama.cpp (v4542) |
| Thinking | ❌ No |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Coder** ✅ — Code generation specialist, Cohere2MoE architecture
- ⚠️ Heavy offload — ~12GB RAM
- ⚠️ No thinking mode — pure code generation

**Recommended params:** temp=0.6/top-p=0.95

---

### lfm2.5-vl-450m — LFM2.5-VL 450M Vision

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [LiquidAI/LFM2.5-VL-450M-GGUF](https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF) |
| License | LFM2 Community License |
| Architecture | lfm2 (Liquid AI) — vision-language |
| Params | ~450M total (0.22B model + 0.18B mmproj) |
| Quant | Q8_0 (model) + F16 (mmproj) |
| Context | 32K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) |
| KV cache | f16 (vision, precision priority) |
| Thinking | ❌ No (--reasoning off) |
| Tool Calling | ❌ No |
| Vision | ✅ Yes (mmproj included) |

**Sprachspiel Sub-Agent Fit:**
- **Vision** ✅ — Ultra-light VLM for object detection, captioning, basic image understanding
- Fast and tiny (~0.5GB VRAM total)
- ⚠️ Limited reasoning — use for simple vision tasks only

---

### lfm2.5-vl-1.6b — LFM2.5-VL 1.6B Vision

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [LiquidAI/LFM2.5-VL-1.6B-GGUF](https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF) |
| License | LFM2 Community License |
| Architecture | lfm2 (Liquid AI) — vision-language |
| Params | ~1.6B |
| Quant | Q8_0 (~1.6 GB) |
| Context | dynamic |
| Backend | llama.cpp upstream (v674) |
| Thinking | ❌ No |
| Tool Calling | ❌ No |
| Vision | ✅ Yes (grounding, bounding boxes) |

**Sprachspiel Sub-Agent Fit:**
- **Vision** ✅ — Better vision quality than 450M, supports bounding box grounding
- ⚠️ Larger than 450M but significantly better image understanding quality

---

### minicpm-v-4.6 — MiniCPM-V 4.6 Vision-Language

| Field | Value |
|-------|-------|
| Status | ✅ Running (unlisted, vision-only) |
| Source | [openbmb/MiniCPM-V-4.6-GGUF](https://huggingface.co/openbmb/MiniCPM-V-4.6-GGUF) |
| License | Apache 2.0 |
| Architecture | Custom (MiniCPM vision) — Qwen3.5-0.8B backbone + SigLIP2-400M vision |
| Params | ~1.3B total (0.55GB model Q5_K_M + 1.1GB mmproj F16) |
| Context | 8K–256K (dynamic) |
| Backend | llama.cpp upstream (v674) — mmproj uses libmtmd |
| KV cache | f16 (vision, precision priority) |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ❌ No |
| Vision | ✅ Yes (video+image+text, LLaVA-UHD v4 compression) |

**Sprachspiel Sub-Agent Fit:**
- **Vision** ✅ — Higher quality than LFM2.5-VL-450M for detailed image understanding, OCR, document analysis
- ⚠️ Larger and slower than LFM2.5-VL-450M
- ✅ Video support, 256K context

---

### qwen3-vl-4b — Qwen3-VL 4B Vision-Language

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [Qwen/Qwen3-VL-4B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF) |
| License | Apache 2.0 |
| Architecture | qwen3 (vision) — vision-language |
| Params | 4B |
| Quant | Q4_K_M (~2.5 GB) |
| Context | dynamic |
| Backend | llama.cpp upstream (v674) |
| Thinking | ❌ No |
| Tool Calling | ❌ No |
| Vision | ✅ Yes (grounding, 0–1000 bounding boxes) |

**Sprachspiel Sub-Agent Fit:**
- **Vision** ✅ — Vision-language with bounding box detection (up to 1000 boxes)
- ✅ Good balance of size and capability — 4B with Q4_K_M fits in ~2.5 GB
- ⚠️ No tool calling — vision-only role

---

### glm-ocr — GLM OCR Specialist

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [zai-org/GLM-OCR](https://huggingface.co/zai-org/GLM-OCR) |
| License | GLM License |
| Architecture | GLM — OCR specialist |
| Params | ~1.4B |
| Quant | BF16 (~1.4 GB) |
| Context | auto |
| Backend | llama.cpp upstream (v674) |
| Thinking | ❌ No |
| Tool Calling | ❌ No |
| Vision | ✅ Yes |

**Sprachspiel Sub-Agent Fit:**
- **Vision** ✅ — Document understanding, text/table/formula extraction
- ✅ Specialized OCR — better text extraction than general VLMs
- ⚠️ Not a chatbot — specialized for OCR only

---

### hy-mt2-1.8b — Hunyuan MT2 1.8B Translation

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [tencent/Hy-MT2-1.8B-GGUF](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF) |
| License | Tencent Hunyuan License |
| Architecture | hunyuan_v1_dense — upstream llama.cpp ONLY |
| Params | 1.8B |
| Quant | Q4_K_M (~1.1 GB) |
| Context | 32K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) — ik doesn't support hunyuan_v1_dense |
| KV cache | q8_0 + attn_rot |
| Thinking | ❌ No |
| Tool Calling | ❌ No |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Translator** ✅✅ — Dedicated translation model (33 languages + 5 dialects)
- Alternative to TranslateGemma for Sprachspiel translate mode
- No system prompt needed (per Hy-MT2 docs)

**Recommended params:** temp=0.7/top-p=0.6/top-k=20/repeat-penalty=1.05

---

### translategemma-4b — TranslateGemma 4B Translation

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [mradermacher/TranslateGemma-4B-GGUF](https://huggingface.co/mradermacher/TranslateGemma-4B-GGUF) |
| License | Gemma Terms of Use |
| Architecture | gemma3 (Google) — translation-specialized |
| Params | 4B |
| Quant | Q4_K_M (~2.7 GB) |
| Context | 32K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) |
| KV cache | (default) |
| Thinking | ❌ No |
| Tool Calling | ❌ No |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Translator** ✅ — 55 languages, currently Sprachspiel's default translation model
- ⚠️ Custom Jinja template issues (crashes llama.cpp parser) — uses --no-jinja --chat-template gemma

**Recommended params:** temp=0.2 (deterministic translation)

**Alias:** "Geminái" in Sprachspiel TTS pronunciation guide

---

### nomic-embed-text-v2-moe — Nomic Embed v2 MoE

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [nomic-ai/nomic-embed-text-v2-moe](https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe) |
| License | Apache 2.0 |
| Architecture | nomic-bert — MoE embedding model |
| Params | 475M |
| Quant | Q4_K_M (~1 GB) |
| Context | 512 |
| Dimensions | 768 |
| Backend | llama.cpp (--embeddings --pooling cls) |
| Thinking | ❌ No (embedding-only) |
| Tool Calling | ❌ No (embedding-only) |
| Vision | ❌ No (embedding-only) |

**Sprachspiel Sub-Agent Fit:**
- **Embedding** ✅✅ — General-purpose embeddings, Matryoshka truncation
- ✅ Small and fast (~1 GB), supports dimension truncation
- ⚠️ Embedding-only — not for chat, tool calling, or reasoning

---

### lfm2.5-embedding-350m — LFM2.5 Embedding 350M

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [LiquidAI/LFM2.5-Embedding-350M](https://huggingface.co/LiquidAI/LFM2.5-Embedding-350M) |
| License | LFM2 Community License |
| Architecture | lfm2-bidir — dense bi-encoder |
| Params | 354M |
| Quant | Q8_0 (~0.4 GB) |
| Context | 512 |
| Dimensions | 1024 |
| Backend | llama.cpp (--embeddings --pooling cls --cont-batching, CPU-only -ngl 0) |
| Thinking | ❌ No (embedding-only) |
| Tool Calling | ❌ No (embedding-only) |
| Vision | ❌ No (embedding-only) |

**Sprachspiel Sub-Agent Fit:**
- **Embedding** ✅✅ — Fast multilingual retrieval, 11 languages
- ✅ Best-in-class dense embedder at 350M scale, ultra-small (~0.4 GB)
- ⚠️ CPU-only (-ngl 0) — no GPU offload
- ⚠️ Embedding-only — not for chat, tool calling, or reasoning

---

## Benchmark Plan

These models are benchmarked via Sprachspiel's OpenAI-compatible provider:

### Tool Calling (all chat models)
- BFCL v3/v4 function calling benchmark
- Multi-step tool calling accuracy
- JSON format compliance

### Reasoning (think-capable models)
- Math (GSM8K, MATH)
- Code generation (HumanEval, MBPP)
- Multi-step reasoning

### Vision (vision models)
- Image captioning quality
- Document OCR accuracy
- Chart/diagram understanding

### Specialized (extraction/translation/embedding)
- Translation quality: Hy-MT2 vs TranslateGemma
- Embedding quality: Nomic v2 MoE vs LFM2.5 Embedding 350M
- OCR quality: GLM-OCR vs MiniCPM-V-4.6

### Throughput (all models)
- TTFT (time to first token)
- Tokens/second
- VRAM usage at various context lengths