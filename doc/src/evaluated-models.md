# Evaluated Models (llama-swap)

Models evaluated for local inference via llama-swap on **RTX 3050 Laptop 6GB VRAM**.
Target: Sprachspiel multi-provider support (OpenAI-compatible endpoints).

When Sprachspiel gains OpenAI endpoint support, these models will be benchmarked
for tool calling, reasoning, vision, and specialized tasks. This catalog tracks
what we've tested, what works, and what doesn't.

## Evaluation Status

- ✅ Running in llama-swap, tested via OpenAI API
- ⏳ GGUF available, not yet in llama-swap
- ❌ Failed / incompatible
- 🔜 Not yet evaluated

## Sprachspiel Sub-Agent Roles

When Sprachspiel gains multi-model support, models will be assigned to sub-agent roles
based on their strengths. Current role taxonomy:

| Role | Requirements | Best Candidates |
|------|-------------|-----------------|
| **Coordinator** | Tool calling, reasoning, JSON compliance | lfm2.5-1.2b, qwen3.5-4b, qwen3.5-9b |
| **Coder** | Code generation, tool calling, agentic loops | qwopus-coder-9b, gpt-oss-20b, qwen3.6-35b-moe |
| **Translator** | Translation quality, multi-language | hy-mt2-1.8b, translategemma-4b |
| **Vision** | Image understanding, OCR | lfm2.5-vl-450m, minicpm-v-4.6 |
| **Reasoner** | Deep thinking, math, logic | gemma4-26b-moe, gemma4-e4b, qwen3.5-9b:think |
| **Lightweight** | Fast, low VRAM, simple tasks | lfm2.5-1.2b, littlelamb-0.3b-tc |
| **World Model** | Web interaction prediction | webworld-8b |

> **Note:** None of these models have been tested directly in Sprachspiel yet.
> Sprachspiel currently only supports Ollama as backend. Multi-provider support
> (OpenAI endpoints via llama-swap) is planned but not yet implemented.

---

## Models

### gemma4-26b-moe — Gemma 4 26B A4B MoE

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [mudler/gemma-4-26B-A4B-it-APEX-GGUF](https://huggingface.co/mudler/gemma-4-26B-A4B-it-APEX-GGUF) |
| License | Gemma Terms of Use |
| Architecture | Gemma4ForCausalLM (gemma4) — MoE 26B, 128 experts, top-8 routing, 4B active/token |
| Params | 26B total / 4B active per token |
| Quant | APEX I-Compact (~15.5 GB) — edges Q4_K, middle Q3_K, shared Q6_K, attn Q4_K |
| Context | 16K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) |
| KV cache | q4_0 + attn_rot (iSWA fix) |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ✅ Yes |
| Vision | ❌ Disabled (mmproj crash on CUDA #21402) |
| attn_rot | ✅ head_dim=256 (256%64==0) |

**Sprachspiel Sub-Agent Fit:**
- **Reasoner** ✅ — 4B active params with deep MoE reasoning, best for complex logic/math
- ⚠️ Heavy offload to RAM (~10GB) — high latency, use only when quality > speed
- ❌ Vision not available

**Recommended params:** temp=1.0/top-p=0.95 (think), temp=0.7/top-p=0.95 (chat)

**Issues:** mmproj crash on CUDA, APEX without MTP is hardware ceiling (6GB VRAM soldered)

---

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
| Vision | ❌ Disabled (mmproj crash on CUDA #21402) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Reasoner** ✅ — Good reasoning quality at 4B, hadamard KV preserves cache quality
- **Coder** ⚠️ — Decent but Qwen3.6-35B-MoE and Qwopus are stronger for code
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
| Vision | ❌ Disabled (--no-mmproj) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Lightweight Reasoner** — Small but capable
- ⚠️ E2B MTP is 3.6× slower than baseline — no speculative decoding benefit

**Recommended params:** temp=0.7/top-p=0.9 (chat), temp=0.6/top-p=0.9 (think)

---

### qwen3.6-35b-moe — Qwen3.6 35B A3B MoE (Primary Coder)

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
| Vision | ❌ Disabled (--no-mmproj to free VRAM for offload) |
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

### qwopus-coder-9b — Qwopus 3.5-9B Coder

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [jackrong/Qwopus3.5-9B-coder](https://huggingface.co/jackrong/Qwopus3.5-9B-coder) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen3.5) — dense 9B, fine-tuned with Trace Inversion + agent traces |
| Params | 9B |
| Quant | Q4_K_M (~5.63 GB) |
| Context | 131072 |
| Backend | ik_llama.cpp (v4542) — pinned memory |
| KV cache | --fit + --fit-margin 512 |
| Thinking | ✅ Yes |
| Tool Calling | ✅ Yes (--parallel-tool-calls) |
| Vision | ❌ No (--no-mmproj: mmproj blocks offload) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Coder** ✅ — Specialized for agentic coding + tool calling, stronger than base Qwen3.5-9B

**Recommended params:** temp=0.6/top-p=0.95 (code-optimized)

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

### qwen3.5-4b — Qwen3.5 4B Dense (TurboQuant)

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/Qwen3.5-4B-GGUF](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3.5ForCausalLM (qwen3.5) — dense 4B |
| Params | 4B |
| Quant | UD-Q3_K_XL (~2.27 GB) |
| Context | 131072 (fixed, --n-gpu-layers 99) |
| Backend | BeeLlama.cpp (v9459/v0.2.0) — TurboQuant KV cache |
| KV cache | turbo3_tcq K + V (~5× compression, PPL ≈ fp16) |
| Thinking | ✅ Yes (dual mode: base + :think) |
| Tool Calling | ✅ Yes |
| Vision | ❌ No (--no-mmproj) |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Coordinator** ✅ — Good tool calling, fits entirely in VRAM with TurboQuant for long context
- **Lightweight** ✅ — Fast at short context, TurboQuant excels at 8K+ context
- ⚠️ Bee only — no --fit, must use --n-gpu-layers 99
- ✅ turbo3_tcq enables 64K-128K context with near-fp16 quality

**Recommended params:** default_temp/top_p/top_k/min_p

**Benchmarks:**
- Bee turbo4: +29-35% speedup at 8-16K context vs upstream q4_0
- Bee turbo4: ~2× speedup at 32K+ context vs upstream q4_0 (long-context king)

---

### lfm2.5-1.2b — LFM2.5 1.2B Instruct

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [LiquidAI/LFM2.5-1.2B-Instruct-GGUF](https://huggingface.co/LiquidAI/LFM2.5-1.2B-Instruct-GGUF) |
| License | LFM2 Community License |
| Architecture | lfm2 (Liquid AI) — dense hybrid (conv + GQA) |
| Params | 1.2B |
| Quant | Q8_0 (~1.25 GB) |
| Context | 32K–128K (dynamic) |
| Backend | llama.cpp upstream (v674) |
| KV cache | q8_0 + attn_rot |
| Thinking | ❌ No |
| Tool Calling | ✅ Yes (consistent, best LFM2 for agentic tasks) |
| Vision | ❌ No |
| attn_rot | ✅ head_dim=64 (64%64==0) |

**Sprachspiel Sub-Agent Fit:**
- **Lightweight Coordinator** ✅✅ — Best tool-calling model under 2B, consistent JSON format
- Fast and reliable for simple agentic loops
- ⚠️ No thinking mode — not suitable for complex reasoning

**Recommended params:** temp=0.3/top-p=0.9/min-p=0.15/repeat-penalty=1.05

---

### littlelamb-0.3b-tc — LittleLamb 0.3B Tool Calling

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [mradermacher/LittleLamb-ToolCalling-GGUF](https://huggingface.co/mradermacher/LittleLamb-ToolCalling-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3 (qwen3) — compressed from 0.6B to 290M via CompactifAI |
| Params | 290M (compressed) |
| Quant | Q8_0 (~303 MB) |
| Context | 8K–40K (dynamic) |
| Backend | llama.cpp upstream (v674) — ik_llama segfaults with this model |
| KV cache | q8_0 + attn_rot |
| Thinking | ✅ Yes (dual mode) |
| Tool Calling | ✅ Yes (Qwen3-style JSON, BFCL v4 51.5%) |
| Vision | ❌ No |
| attn_rot | ✅ head_dim=256 |

**Sprachspiel Sub-Agent Fit:**
- **Ultra-lightweight Coordinator** ⚠️ — BFCL 51.5% is mediocre
- Useful for extremely VRAM-constrained scenarios or simple tool loops
- ⚠️ Not suitable for complex agentic tasks (low accuracy)

**Issues:** ik_llama.cpp segfaults with qwen3 arch — upstream only

**Recommended params:** uses code_temp/code_top_p/code_top_k

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

### gpt-oss-20b — GPT-OSS 20B MoE Coding

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [unsloth/gpt-oss-20b-GGUF](https://huggingface.co/unsloth/gpt-oss-20b-GGUF) |
| License | Apache 2.0 |
| Architecture | OPENAI_MOE (gpt_oss) — 20.91B, top-4/8 experts, ~3.5B active |
| Params | 20.91B total / ~3.5B active |
| Quant | Q4_K_M (~10.8 GB) |
| Context | 16K–128K (dynamic) |
| Backend | ik_llama.cpp (v4542) — pinned memory + hadamard |
| KV cache | q4_0 + hadamard + attn_rot |
| Thinking | ✅ Yes (ALWAYS — Harmony format, not optional) |
| Tool Calling | ✅ Yes |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **Coder** ✅ — Strong coding model with always-on reasoning (Harmony format)
- ⚠️ Heavy — ~6GB RAM offload, slower than pure GPU models
- ⚠️ Always thinks internally — no way to disable reasoning

**Recommended params:** temp=0.6/top-p=0.9, --ubatch-size 2048 --batch-size 2048

---

### webworld-8b — WebWorld 8B World Model

| Field | Value |
|-------|-------|
| Status | ✅ Running |
| Source | [Qwen/WebWorld-8B](https://huggingface.co/Qwen/WebWorld-8B) |
| License | Apache 2.0 |
| Architecture | qwen3 — previously segfaulted in ik, FIXED in v4524+ |
| Params | 8B |
| Quant | i1-Q5_K_M |
| Context | 40960 |
| Backend | ik_llama.cpp (v4542) |
| KV cache | (default) |
| Thinking | ❌ No |
| Tool Calling | ❌ No |
| Vision | ❌ No |

**Sprachspiel Sub-Agent Fit:**
- **World Model** ✅ — Predicts next web page state given current state + action
- NOT a chatbot — requires specialized prompt format (world model system prompt + state/action)
- Action space: click(bid), fill(bid,text), goto(url), scroll(dx,dy), keyboard_press(key)

---

### MiniCPM5-1B (Not in active roster)

| Field | Value |
|-------|-------|
| Status | ⏳ GGUF available, not in llama-swap |
| Source | [openbmb/MiniCPM5-1B-GGUF](https://huggingface.co/openbmb/MiniCPM5-1B-GGUF) |
| License | Apache 2.0 |
| Architecture | LlamaForCausalLM |
| Params | 1.08B (680M non-embedding) |
| Quant | Q8_0 (~1.15 GB) |
| Context | 131,072 (128K) |
| Thinking | ✅ Yes (enable_thinking) |
| Tool Calling | ❌ XML-style — incompatible with OpenAI tool_calls format |
| Vision | ❌ No |
| attn_rot | ❌ head_dim=96, 96%64≠0 |

**Sprachspiel Sub-Agent Fit:**
- ❌ NOT suitable for tool calling (XML format incompatible)
- ❌ Think mode too verbose for tool-calling tasks
- ⚠️ #1 on Artificial Analysis index for small models (17.9) — good for non-tool reasoning

**Issues:** llama.cpp autoparser TAG_WITH_TAGGED boundary bug — XML tool calls not parsed as structured tool_calls. SGLang OOM on 6GB. Awaiting upstream fix.

---

### NuExtract3 (Not in active roster)

| Field | Value |
|-------|-------|
| Status | ⏳ GGUF available, not in llama-swap |
| Source | [numind/NuExtract3-GGUF](https://huggingface.co/numind/NuExtract3-GGUF) |
| License | Apache 2.0 |
| Architecture | Qwen3_5ForConditionalGeneration (Gated Delta Net hybrid) |
| Params | 4B (vision-language) |
| Thinking | ✅ Yes (enable_thinking) |
| Tool Calling | ❌ No (specialized extraction model) |
| Vision | ✅ Yes (document understanding) |

**Sprachspiel Sub-Agent Fit:**
- **Extraction Specialist** ⏳ — Structured extraction from text/images + JSON templates
- ⚠️ Same Qwen3.5 VL architecture that crashes mmproj on CUDA
- Not a general chatbot — specialized for extraction/OCR only

---

## Benchmark Plan

When Sprachspiel gains OpenAI endpoint support, we need to benchmark:

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

### Specialized (extraction/translation)
- NuExtract structured extraction with JSON templates
- Translation quality: Hy-MT2 vs TranslateGemma

### Throughput (all models)
- TTFT (time to first token)
- Tokens/second
- VRAM usage at various context lengths

## Format

When adding a new model, copy the template below:

```
### [Model Name]

| Field | Value |
|-------|-------|
| Status | ⏳/✅/❌ |
| Source | [HF link] |
| License | [license] |
| Architecture | [arch] |
| Params | [size] |
| Quant | [quantization] |
| Context | [max context] |
| Backend | [engine + version] |
| KV cache | [cache type] |
| Thinking | Yes/No (method) |
| Tool Calling | [format or No] |
| Vision | Yes/No |
| attn_rot | ✅/❌ (head_dim info) |

**Sprachspiel Sub-Agent Fit:**
- [Role] ✅/⚠️/❌ — [reasoning]

**Recommended params:** temp=X, top_p=X, ...

**Issues:** [any known problems]
```