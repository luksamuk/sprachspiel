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
# read_timeout_secs = 300
# stream_idle_timeout_secs = 300
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

| Model | Size | Dimensions | Best For |
|-------|------|------------|----------|
| [Nomic Embed v2](https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe) | ~1 GB | 768 | General-purpose embeddings, MoE, supports Matryoshka truncation **(recommended)** |
| [LFM2.5-Embed-350M](https://huggingface.co/LiquidAI/LFM2.5-Embedding-350M) | ~0.4 GB | 1024 | Dense bi-encoder, fast multilingual retrieval, 11 languages |

> Embedding models are declared with `embeddings = true` and `dimensions` in `models.toml`. They cannot be used for chat (the `-m` and `/model` commands reject them).

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