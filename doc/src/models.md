# Available Models

Ask-AI uses a two-tier model system:

1. **Built-in models** - Essential models required for basic functionality
2. **Recommended models** - Additional models configured via `~/.config/ask-ai/models.toml`

## Built-in Models (Required)

These models are required for basic functionality:

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| **llama3.1** | llama3.1:8b | 4K | General queries (default) |
| translategemma | translategemma:12b | 4K | Translation |
| glm-ocr | glm-ocr:bf16 | Auto | OCR/image text extraction |

### Installation

```bash
# Required models
ollama pull llama3.1:8b
ollama pull translategemma:12b
ollama pull glm-ocr:bf16
```

## Recommended Models

These optional models can be configured in `~/.config/ask-ai/models.toml`:

### General Purpose

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| ministral | ministral-3:14b | 32K | General queries, fast |
| qwen3 | qwen3:8b | 32K | General with thinking |

### Code & Development

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| qwen3-coder | qwen3-coder:30b | 32K | Code generation with tools |
| qwen3-coder-next | qwen3-coder-next:cloud | 260K | Cloud code generation |
| nemotron | nemotron-3-nano:30b | 32K | Thinking mode |

### Cloud Models

High-capability models with large context windows:

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| glm-5 | glm-5:cloud | 200K | Complex reasoning, thinking |
| kimi-k2.5 | kimi-k2.5:cloud | 200K | Multimodal, thinking |
| minimax-m2.5 | minimax-m2.5:cloud | 200K | Coding, agentic |
| qwen3.5 | qwen3.5:cloud | 260K | Vision-language |

### Character Models

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| assistant-pepe | assistant-pepe:8b | 64K | Entertainment |

## Sample Configuration

Create `~/.config/ask-ai/models.toml`:

```toml
# General purpose models
[models.ministral]
model_id = "ministral-3:14b"
num_ctx = 32768
temperature = 0.2

[models.qwen3]
model_id = "qwen3:8b"
num_ctx = 32768
temperature = 1.0
thinking = true

# Code models
[models.qwen3-coder]
model_id = "qwen3-coder:30b"
num_ctx = 32768
temperature = 0.3

# Cloud models with thinking
[models.glm-5]
model_id = "glm-5:cloud"
num_ctx = 202752
thinking = true

[models.kimi-k2.5]
model_id = "kimi-k2.5:cloud"
num_ctx = 202144
thinking = true
```

## Per-Command Model Configuration

Configure different models for different tasks in `~/.config/ask-ai/config.toml`:

```toml
[model]
default = "ministral"      # Global default

[model.query]
model = "ministral"        # For queries
thinking = false
tools = true

[model.summarize]
model = "qwen3"            # For summarization
thinking = false
tools = false

[model.code]
model = "qwen3-coder"      # For code
tools = true
```

## Model Capabilities

| Model | Tools | Vision | Think | Local |
|-------|-------|--------|-------|-------|
| llama3.1 | Yes | No | No | Yes |
| translategemma | No | No | No | Yes |
| glm-ocr | No | Yes | No | Yes |
| ministral | Yes | No | No | Yes |
| qwen3 | No | No | Yes | Yes |
| qwen3-coder | Yes | No | No | Yes |
| nemotron | No | No | Yes | Yes |
| glm-5 | Yes | No | Yes* | No |
| kimi-k2.5 | Yes | Yes | Yes* | No |
| minimax-m2.5 | Yes | No | Yes* | No |
| qwen3.5 | Yes | Yes | Yes* | No |

\* Cloud models support thinking via `thinking = true` in config.

## Choosing a Model

### For General Queries
```bash
ask-ai "Your question"           # Default model from config
ask-ai -m ministral "question"   # Fast, capable
ask-ai -m qwen3 -t "reasoning"   # With thinking
```

### For Code
```bash
ask-ai -m qwen3-coder "Write a Rust function"
ask-ai -p code "Optimize this code"  # Code prompt mode
```

### For Cloud Models
```bash
ask-ai -m glm-5 "Complex analysis"
ask-ai -m kimi-k2.5 "Multimodal task"
ask-ai -m minimax-m2.5 "Coding task"
```

## Custom Models

Add your own models in `~/.config/ask-ai/models.toml`:

```toml
[models.my-model]
model_id = "my-model:7b"    # Required
num_ctx = 32768             # Optional: context window
temperature = 0.7           # Optional: temperature
top_k = 40                  # Optional: top-k
top_p = 0.9                 # Optional: top-p
repeat_penalty = 1.1        # Optional: repeat penalty
thinking = true             # Optional: for cloud models
```

### Model Parameter Defaults

| Parameter | Default | Notes |
|-----------|---------|-------|
| num_ctx | 32768 (32K) | Omit for auto-detect |
| temperature | 0.8 | From docs.ollama.com |
| top_k | not set | Uses Ollama default |
| top_p | not set | Uses Ollama default |
| repeat_penalty | 1.1 | From docs.ollama.com |

## Listing Models

```bash
ask-ai --list
```

Shows built-in models and user-defined models (marked with `[user]`).

## See Also

- [query](./commands/query.md) - Using models for queries
- [summarize](./commands/summarize.md) - Summarization command
- [Configuration](./configuration.md) - Custom models setup