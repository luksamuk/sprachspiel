# Model Guide

Sprachspiel supports any OpenAI-compatible LLM backend. You can run models locally via [llama-swap](https://github.com/mostlygeeksllc/llama-swap), [llama.cpp](https://github.com/ggerganov/llama.cpp), [Ollama](https://ollama.com), [LM Studio](https://lmstudio.ai), or [vLLM](https://github.com/vllm-project/vllm), or use a cloud provider that exposes an OpenAI-compatible `/v1/chat/completions` endpoint. Sprachspiel communicates with all of them through the provider-agnostic `LlmProvider` trait.

Sprachspiel does not bundle or distribute any models — you download and configure your own. The models listed below have been evaluated on consumer hardware (RTX 3050 Laptop, 6GB VRAM) and are known to work well. You are not limited to these; any model served by an OpenAI-compatible backend will work.

## Configuration Overview

Sprachspiel uses two config files:

1. **`~/.config/sprachspiel/models.toml`** — Provider endpoints and model definitions (`[provider.*]` and `[models.*]` sections).
2. **`~/.config/sprachspiel/config.toml`** — Per-subcommand defaults (`[model]`, `[model.query]`, `[model.code]`, etc.).

### Provider Configuration (`models.toml`)

The `[provider]` section defines the transport — how Sprachspiel reaches the LLM backend. Example with llama-swap on `localhost:12434`:

```toml
# ~/.config/sprachspiel/models.toml

[provider."llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"
# connect_timeout_secs = 5
# read_timeout_secs = 900
# stream_idle_timeout_secs = 900
# ttfb_timeout_secs = 120
# max_retries = 3
```

Each model entry references its provider by name and can override inference parameters:

```toml
[models."qwen3.5-4b"]
model_id = "qwen3.5-4b"
provider = "llama-swap"
tools = true
vision = true
thinking = true
temperature = 0.6
top_p = 0.95

# Embedding models require embeddings = true + dimensions
[models."nomic"]
model_id = "nomic-embed-text-v2-moe"
provider = "llama-swap"
embeddings = true
dimensions = 768
```

### Per-Command Defaults (`config.toml`)

The `[model]` section in `config.toml` sets which model each subcommand uses:

```toml
# ~/.config/sprachspiel/config.toml

[model]
default = "qwen3.5-4b"       # Global default

[model.query]
model = "qwen3.5-4b"
thinking = true
tools = true

[model.summarize]
model = "qwen3.5-4b"
thinking = false
tools = false

[model.code]
model = "qwen3.6-35b-a3b"
tools = true

[model.vision]
model = "lfm2.5-vl-1.6b"
tools = false
```

## Recommended Models for Local Inference

### Chat / General

| Model | Size | Tools | Thinking | Vision | Best For |
|-------|------|-------|----------|--------|----------|
| [Qwen3.5-4B](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) | 3.4 GB | ✅ | ✅ | ✅ | **Default** — best balance of size, tool calling, thinking, multimodal |
| [Gemma 4 E2B](https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF) | 2.7 GB | ✅ | ✅ | ✅ | Google 2B dense, native function calling, dual-mode thinking |
| [LFM2.5-8B-A1B](https://huggingface.co/LiquidAI/LFM2.5-8B-A1B-GGUF) | 4.8 GB | ✅ | ✅ | ❌ | MoE 8B (1.5B active), good balance |
| [Qwen3.5-9B](https://huggingface.co/unsloth/Qwen3.5-9B-GGUF) | 5.1 GB | ✅ | ✅ | ✅ | **Recommended upgrade** — better quality, multimodal |
| [Gemma 4 E4B](https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF) | 4.5 GB | ✅ | ✅ | ✅ | Google 4B dense, native function calling, thinking |
| [Qwen3.6-35B-A3B](https://huggingface.co/mudler/Qwen3.6-35B-A3B-APEX-GGUF) | 17.3 GB | ✅ | ✅ | ✅ | MoE 35B (3B active), primary coder |
| [Ornith 1.0-35B](https://huggingface.co/mudler/Ornith-1.0-35B-APEX-GGUF) | 16.5 GB | ✅ | ✅ | ❌ | Large MoE, self-improving agentic coder |
| [Agents-A1-35B](https://huggingface.co/mudler/Agents-A1-APEX-GGUF) | 16.5 GB | ✅ | ✅ | ❌ | Agentic model, long-horizon tasks |
| [GLM-4.7-Flash](https://huggingface.co/unsloth/GLM-4.7-Flash-GGUF) | ~17 GB | ✅ | ✅ | ❌ | Fast reasoning (30B MoE, 3.6B active) |
| [North-Mini-Code](https://huggingface.co/unsloth/North-Mini-Code-1.0-GGUF) | 18.6 GB | ✅ | ❌ | ❌ | Code specialist (Cohere2MoE, 30B A3B) |

### OCR

| Model | Size | Vision | Best For |
|-------|------|--------|----------|
| [GLM-OCR](https://huggingface.co/zai-org/GLM-OCR) | ~1.4 GB | ✅ | **Recommended** — document understanding, text/table/formula extraction |

> **Note:** GLM-OCR is purpose-built for OCR, but any general-purpose model with vision capability (e.g., Qwen3.5-4B, Gemma 4 E2B) can also handle basic image text extraction. For best results with complex documents, use a dedicated OCR model.

### Vision

> **Note:** Any general-purpose model with `vision = ✅` in the Chat/General table above (e.g., Qwen3.5-4B, Qwen3.5-9B, Gemma 4 E2B/E4B, Qwen3.6-35B-A3B) can also be used for image analysis via `sprach vision`. The models listed below are dedicated vision-language models optimized for image understanding.

| Model | Size | Vision | Grounding | Thinking | Best For |
|-------|------|--------|-----------|----------|----------|
| [LFM2.5-VL-450M](https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF) | ~0.5 GB | ✅ | ✅ | ❌ | Ultra-light VLM, bounding boxes |
| [LFM2.5-VL-1.6B](https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF) | ~1.6 GB | ✅ | ✅ | ❌ | Better vision quality, grounding |
| [MiniCPM-V-4.6](https://huggingface.co/openbmb/MiniCPM-V-4.6-GGUF) | ~1.7 GB | ✅ | ✅ | ✅ | VLM with grounding + video support |
| [Qwen3-VL-4B](https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF) | ~2.5 GB | ✅ | ✅ | ❌ | Vision-language, bounding boxes (0–1000) |

### Translation

> **Note:** Any general-purpose model can translate text, but the models listed below are translation specialists trained to handle many languages and preserve nuance.

| Model | Size | Best For |
|-------|------|----------|
| [TranslateGemma-4B](https://huggingface.co/mradermacher/TranslateGemma-4B-GGUF) | ~3 GB | 50+ languages, better nuance and context understanding **(recommended)** |
| [Hy-MT2-1.8B](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF) | 1.1 GB | 33 languages + 5 dialects, lightweight, no system prompt needed |

### Embedding

Embedding models generate vector representations of text for semantic search (`/search`), fact matching, and conversation enrichment. They are declared with `embeddings = true` and `dimensions` in `models.toml`, and selected via `[indexing].model` in `config.toml`. They cannot be used for chat (the `-m` and `/model` commands reject them).

| Model | Size | Dims | Context | Languages | Prefix | Best For |
|-------|------|------|---------|-----------|--------|----------|
| [Nomic Embed v2](https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe) | ~1 GB | 768 | 8192 | 100+ | `search_document: ` | **Default** — general-purpose, MoE (305M active), Matryoshka 64-768 |
| [Snowflake Arctic Embed M v2](https://huggingface.co/Snowflake/snowflake-arctic-embed-m-v2.0) | ~200 MB | 768 | 8192 | 74 (pt-BR) | `query: ` / none | **Recommended alternative** — 5× smaller, multilingual with Portuguese, d_eff ~4% |
| [Qwen3 Embedding 0.6B](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B) | ~400 MB | 4096 | 8192 | Multilingual | instruction-aware | High d_eff potential, but 4096d = more storage |
| [mxbai-embed-large](https://huggingface.co/mixedbread-ai/mxbai-embed-large-v1) | ~670 MB | 1024 | **512** | English | `Represent this sentence...` | Best MTEB (64.68) but 512-token context is a hard blocker for RAG |
| [LFM2.5-Embedding-350M](https://huggingface.co/LiquidAI/LFM2.5-Embedding-350M) | ~230 MB | 1024 | 32768 | 11 (pt) | `query: ` / `document: ` | Half nomic's size, 32K context, hybrid conv+attn, efficient on CPU |
| [Nemotron-3-Embed-1B](https://huggingface.co/nvidia/Nemotron-3-Embed-1B-BF16) | ~700 MB Q4 | 2048 | 32768 | 34 (pt) | `query: ` / `document: ` | **#1 RTEB at 1B scale** (72.4%), agent memory, multilingual cross-lingual, GGUF available |

#### Model Ranking

The following ranking is weighted for Sprachspiel's use case: local inference on consumer hardware (6GB VRAM), Portuguese language support, RAG with 512+ character chunks, and hybrid BM25+vector retrieval.

| Rank | Model | Retrieval | Size | Context | Languages | d_eff (est.) | Matryoshka | Why this rank |
|------|-------|-----------|------|---------|-----------|--------------|------------|---------------|
| 🥇 1 | [Nemotron-3-Embed-1B](https://huggingface.co/nvidia/Nemotron-3-Embed-1B-BF16) | 72.4 RTEB | ~700 MB Q4 | 32K | 34 (pt) | TBD | ✅ | #1 RTEB at 1B scale, 34 langs incl pt, 32K context, agent memory, GGUF available |
| 🥈 2 | [Nomic Embed v2](https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe) | ~62 MTEB | ~1 GB | 8K | 100+ | 2.74% (measured) | ✅ 64-768 | Current default, well-tested, MoE (305M active), low d_eff but BM25 compensates |
| 🥉 3 | [Snowflake Arctic Embed M v2](https://huggingface.co/Snowflake/snowflake-arctic-embed-m-v2.0) | ~57 MTEB | ~200 MB | 8K | 74 (pt-BR) | ~4% (est.) | ✅ 256 | 5× smaller, higher estimated d_eff, 74 langs incl pt-BR, but lower MTEB |
| 4 | [LFM2.5-Embedding-350M](https://huggingface.co/LiquidAI/LFM2.5-Embedding-350M) | TBD | ~230 MB | 32K | 11 (pt) | TBD | TBD | 32K context, half nomic's size, CPU-efficient, but no known MTEB score |
| 5 | [Qwen3 Embedding 0.6B](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B) | ~60 MTEB | ~400 MB | 8K | Multilingual | ~1-3% (est.) | ✅ 32-4096 | Potentially high d_eff, but 4096d = significantly more storage |
| 6 | [mxbai-embed-large](https://huggingface.co/mixedbread-ai/mxbai-embed-large-v1) | 64.68 MTEB | ~670 MB | **512** | English | ~3-6% (est.) | ✅ 64-1024 | Best local MTEB, but 512-token context is a hard blocker for RAG chunks |

**Ranking weights:**

- **Retrieval quality (RTEB/MTEB)** — 30%. The primary purpose of the embedding model.
- **Context length** — 20%. 512 tokens blocks RAG; 8K is acceptable; 32K is ideal.
- **Local viability** — 20%. Size, GGUF availability, llama.cpp compatibility.
- **Multilingual / pt-BR** — 15%. Sprachspiel is Brazilian; Portuguese support matters.
- **d_eff** — 10%. Vector discrimination (estimated for untested models).
- **Matryoshka** — 5%. Nice-to-have for server-side truncation.

**Caveats:**

- d_eff is **estimated** for all models except Nomic (measured at 7/256 = 2.74%). Run `sprach diagnostics` after switching models to measure actual d_eff — the ranking may shift.
- Nemotron-3-Embed-1B is #1 by RTEB + context + languages, but d_eff is unknown. If d_eff turns out very low, Snowflake (#3) may overtake it.
- Nomic is #2 by stability, not by quality — it's the current default, already integrated and tested. After #106 lands and benchmarks run, it may drop.
- mxbai is last despite the highest local MTEB — 512-token context makes it unusable for Sprachspiel's 512+ character chunks.
- Nemotron-3-Embed-8B (RTEB 78.5%, #1 overall) is excluded from this ranking — at 8B BF16 it's too heavy for local consumer hardware. The 1B variant brings most of the quality at a fraction of the footprint.

#### Prefix Configuration

Each embedding model expects a specific text prefix. Configure it via `[indexing].prefix` in `config.toml`:

```toml
[indexing]
model = "nomic"                    # alias from models.toml
prefix = "search_document: "       # nomic prefix (default)
# prefix = ""                       # for models that don't need a prefix (BGE, GTE)
# prefix = "query: "                # for snowflake-arctic-embed queries
```

| Model | Prefix | Notes |
|-------|--------|-------|
| nomic-embed-text-v2-moe | `search_document: ` | Default; also supports `search_query: ` for queries |
| snowflake-arctic-embed-m-v2.0 | `query: ` for queries, none for passages | Asymmetric |
| qwen3-embedding | instruction-aware | Follows `Instruct: ...\nQuery: ` format |
| mxbai-embed-large | `Represent this sentence for searching relevant passages: ` | Long prefix |
| LFM2.5-Embedding-350M | `query: ` / `document: ` | Asymmetric, similar to nomic |
| Nemotron-3-Embed-1B | `query: ` / `document: ` | Asymmetric, same as LFM2.5; 34 languages including pt |
| BGE / GTE / embeddinggemma | `""` (none) | No prefix needed |

#### Dimensions and Matryoshka Truncation

Embedding models output vectors at their **nominal dimensions** (e.g., 768 for Nomic). Sprachspiel can store the full vector or truncate it to a smaller dimension via **Matryoshka Representation Learning (MRL)** — the model aligns the most important information in the first N dimensions, so truncating to 256 retains most quality with 3× storage savings.

Configure the storage dimension via the model alias's `dimensions` field in `models.toml`:

```toml
[models."nomic"]
model_id = "nomic-embed-text-v2-moe"
dimensions = 256          # Matryoshka-truncated (recommended: 3× less storage, ~2-3% quality loss)
# dimensions = 768        # Full dimensions (no truncation, maximum quality)
```

When you change `dimensions`, Sprachspiel automatically recreates the vec0 tables and regenerates all embeddings on the next startup via the background recovery pipeline.

#### Embedding Geometry (d_eff)

Sprachspiel's `sprach diagnostics` subcommand measures **effective dimensionality (d_eff)** — the number of dimensions that actually carry signal, as opposed to the nominal storage dimensions. Low d_eff means vector search is weak and BM25 compensates silently.

- **Current (Nomic v2, 256d):** d_eff ≈ 7 (2.74%) — SPREAD regime, BM25 compensates
- **Snowflake Arctic M v2 (estimated):** d_eff ≈ 20-35 (~4%) — potentially better discrimination
- **Nemotron-3-Embed-1B:** d_eff unknown — needs measurement via `sprach diagnostics`; RTEB 72.4% suggests strong retrieval but geometry is model-dependent
- **Recommended:** Run `sprach diagnostics` after changing models to verify d_eff and adjust RRF weights if needed

## Quick Recommendations

| Use Case | Model | Why |
|----------|-------|-----|
| Daily use | Qwen3.5-4B | Default, multimodal, good balance of size and capability |
| **Better quality** | **Qwen3.5-9B** | **Recommended upgrade, worth the VRAM** |
| Code | Ornith 1.0-35B | Self-improving agentic coder |
| Fast reasoning | GLM-4.7-Flash | 30B MoE, fast decode |
| OCR | GLM-OCR | Document extraction (recommended) |
| Vision (light) | LFM2.5-VL-450M | Ultra-light VLM |
| Vision (quality) | Qwen3-VL-4B | Best dedicated vision quality |
| Translation (quality) | TranslateGemma-4B | 50+ languages, better nuance |
| Translation (fast) | Hy-MT2-1.8B | 33+ languages, lightweight |
| Embedding | Nomic Embed v2 | 768d, Matryoshka, general-purpose |
| Embedding (best retrieval) | Nemotron-3-Embed-1B | #1 RTEB 1B (72.4%), 34 languages, 32K context |
| Embedding (small + pt-BR) | Snowflake Arctic M v2 | 200MB, 74 languages, d_eff ~4% |

## Choosing a Model

### For General Queries

```bash
sprach "Your question"                    # Default (Qwen3.5-4B)
sprach -m qwen3.5-9b "question"           # Better quality
sprach -m lfm2.5-8b-a1b "question"        # Good balance
sprach -m glm-4.7-flash "question" -t     # Fast reasoning
```

### For Code

```bash
sprach -m qwen3.6-35b-a3b "Write a Rust function"  # Primary coder
sprach -m north-mini-code "Refactor this module"  # Code specialist
sprach -m ornith-1.0-35b "Complex agentic task"   # Agentic coder
```

### For Vision

```bash
sprach vision photo.png                     # Default (LFM2.5-VL-1.6B)
sprach vision -m lfm2.5-vl-450m photo.png   # Lightweight
sprach vision -m minicpm-v-4.6 photo.png    # Higher quality
sprach vision -m qwen3-vl-4b photo.png     # Bounding boxes
sprach vision -m qwen3.5-4b photo.png      # General-purpose with vision
```

### For OCR

```bash
sprach ocr document.png                     # Default (GLM-OCR)
sprach ocr document.png --mode table         # Table extraction
sprach ocr document.png --formula            # Formula extraction
```

### For Translation

```bash
sprach translate "Hello world"               # Default (TranslateGemma)
sprach translate -m hy-mt2-1.8b "Hello world"  # Lightweight (Hy-MT2)
```

## Custom Models

Add your own models in `~/.config/sprachspiel/models.toml`:

```toml
[models."my-model"]
model_id = "my-model-7b"       # Required: Model ID recognized by backend
provider = "llama-swap"        # Required: References a [provider.*] section
tools = true                    # Optional: tool calling capability
vision = false                  # Optional: vision capability
thinking = true                 # Optional: thinking/reasoning mode
temperature = 0.7              # Optional: temperature
top_p = 0.9                    # Optional: top-p
seed = 42                      # Optional: reproducible outputs
embeddings = false             # Optional: marks as embedding-only
dimensions = 768               # Required if embeddings = true
```

### Model Parameters

| Parameter | Default | Notes |
|-----------|---------|-------|
| `temperature` | 0.8 | From docs.ollama.com |
| `top_p` | not set | Uses backend default |
| `seed` | not set | Optional, for reproducible outputs |
| `num_ctx` | 32768 (32K) | Omit for auto-detect |
| `thinking` | not set | Tri-state: `true`/`false`/probe |

## Listing Models

```bash
sprach --list
```

Shows built-in models and user-defined models (marked with `[user]`).

## Sample `models.toml`

A complete configuration example:

```toml
# ~/.config/sprachspiel/models.toml

# ── Provider ──────────────────────────────────────────────

[provider."llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

# ── Chat / General ─────────────────────────────────────────

[models."qwen3.5-4b"]
model_id = "qwen3.5-4b"
provider = "llama-swap"
tools = true
thinking = true
temperature = 0.6
top_p = 0.95

[models."gemma4-e2b"]
model_id = "gemma4-e2b"
provider = "llama-swap"
tools = true
thinking = true
temperature = 0.7

[models."lfm2.5-8b-a1b"]
model_id = "lfm2.5-8b-a1b"
provider = "llama-swap"
tools = true
thinking = true

[models."qwen3.5-9b"]
model_id = "qwen3.5-9b"
provider = "llama-swap"
tools = true
thinking = true
temperature = 0.6
top_p = 0.95

[models."gemma4-e4b"]
model_id = "gemma4-e4b"
provider = "llama-swap"
tools = true
thinking = true
temperature = 0.7

[models."qwen3.6-35b-a3b"]
model_id = "qwen3.6-35b-a3b"
provider = "llama-swap"
tools = true
thinking = true

[models."ornith-1.0-35b"]
model_id = "ornith-1.0-35b"
provider = "llama-swap"
tools = true
thinking = true

[models."agents-a1-35b"]
model_id = "agents-a1-35b"
provider = "llama-swap"
tools = true
thinking = true

[models."glm-4.7-flash"]
model_id = "glm-4.7-flash"
provider = "llama-swap"
tools = true
thinking = true

[models."north-mini-code"]
model_id = "north-mini-code"
provider = "llama-swap"
tools = true

# ── OCR ─────────────────────────────────────────────────────

[models."glm-ocr"]
model_id = "glm-ocr"
provider = "llama-swap"
vision = true

# ── Vision ─────────────────────────────────────────────────

[models."lfm2.5-vl-450m"]
model_id = "lfm2.5-vl-450m"
provider = "llama-swap"
vision = true

[models."lfm2.5-vl-1.6b"]
model_id = "lfm2.5-vl-1.6b"
provider = "llama-swap"
vision = true

[models."minicpm-v-4.6"]
model_id = "minicpm-v-4.6"
provider = "llama-swap"
vision = true
thinking = true

[models."qwen3-vl-4b"]
model_id = "qwen3-vl-4b"
provider = "llama-swap"
vision = true

# ── Translation ────────────────────────────────────────────

[models."hy-mt2-1.8b"]
model_id = "hy-mt2-1.8b"
provider = "llama-swap"
temperature = 0.7
top_p = 0.6

# ── Embedding ──────────────────────────────────────────────

[models."nomic"]
model_id = "nomic-embed-text-v2-moe"
provider = "llama-swap"
embeddings = true
dimensions = 768

[models."lfm2.5-embedding-350m"]
model_id = "lfm2.5-embedding-350m"
provider = "llama-swap"
embeddings = true
dimensions = 1024
```

## See Also

- [query](./commands/query.md) — Using models for queries
- [summarize](./commands/summarize.md) — Summarization command
- [vision](./commands/vision.md) — Vision command
- [Configuration](./configuration.md) — Full configuration reference
- [Provider Architecture](./development/provider-architecture.md) — LlmProvider trait design