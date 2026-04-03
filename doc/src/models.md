# Available Models

Ask-AI uses a two-tier model system:

1. **Built-in models** - Essential models for core functionality
2. **Recommended models** - Additional models configured via `~/.config/ask-ai/models.toml`

## Built-in Models

These models are configured by default and provide core functionality:

| Preset | Model ID | Size | Context | Best For |
|--------|----------|------|---------|----------|
| **qwen3.5:4b** | qwen3.5:4b | 3.4 GB | 131K | General queries, code, vision (multimodal) |
| translategemma | translategemma:4b | ~3 GB | 4K | Translation |
| glm-ocr | glm-ocr:bf16 | 2.2 GB | Auto | OCR/image text extraction |

### Installation

```bash
# Required models (built-in)
ollama pull qwen3.5:4b       # Default model (multimodal)
ollama pull translategemma:4b # Translation
ollama pull glm-ocr:bf16      # OCR
```

### Recommended Upgrades

For users who want better quality, the same model family offers larger variants:

| Model ID | Size | Context | Best For |
|----------|------|---------|----------|
| **qwen3.5:9b** | 6.6 GB | 131K | **Better quality** — recommended for daily use |
| qwen3.5:27b | 17 GB | 64K | **Full agent experience** — overkill for most tasks |

**Recommendation:**
- **qwen3.5:9b** — Good balance between quality and speed. Worth it if you have the RAM.
- **qwen3.5:27b** — Overkill. Only for users who need maximum reasoning capability and have lots of RAM.

To use these models, configure them in `~/.config/ask-ai/config.toml`:

```toml
[model]
default = "qwen3.5:9b"  # Upgrade from 4b

[model.query]
model = "qwen3.5:9b"

[model.code]
model = "qwen3.5:9b"  # Or "qwen3.5:27b" for complex tasks
```

And define the model in `~/.config/ask-ai/models.toml`:

```toml
[models."qwen3.5:9b"]
model_id = "qwen3.5:9b"
num_ctx = 131072
temperature = 0.6
top_p = 0.95
top_k = 20
thinking = true

[models."qwen3.5:27b"]
model_id = "qwen3.5:27b"
num_ctx = 65536
temperature = 0.6
top_p = 0.95
top_k = 20
thinking = true
```

## Alternative Models

These models can be installed for specific use cases:

| Model ID | Size | Context | Best For |
|----------|------|---------|----------|
| llama3.1:8b | 4.9 GB | 4K | General queries (alternative) |
| moondream:1.8b | 1.7 GB | 2K | Vision (lightweight alternative) |

```bash
# Alternative models (optional)
ollama pull llama3.1:8b      # Alternative general model
ollama pull moondream:1.8b   # Alternative vision model
```

## Recommended Models

These optional models can be configured in `~/.config/ask-ai/models.toml`:

### General Purpose

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| ministral | ministral-3:14b | 32K | General queries, fast |
| qwen3 | qwen3:8b | 32K | General with thinking |
| **nemotron-3-nano** | nemotron-3-nano:4b | 131K | **Efficient tool calling**, 2.8GB |
| **gemma4:e2b** | gemma4:e2b | 131K | **Native FC**, Google's compact model |
| nanbeige4.1 | nanbeige4.1:3b | 64K | Tool calling + thinking |
| ministral-3 | ministral-3:3b | 256K | Fast tool calling (temp=0.3) |

### Code & Development

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| **qwen2.5-coder:7b** | qwen2.5-coder:7b | 128K | **Default for code mode**, function calling |
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

### Vision Models

These models are used by the `ask vision` command for image analysis:

| Model ID | Size | Context | Multi-Image | Best For |
|----------|------|---------|-------------|----------|
| qwen3.5:4b | 3.4 GB | 131K | Yes | Default, multimodal |
| moondream:1.8b | 1.7 GB | 2K | No | Lightweight alternative |
| llava:13b | 8.0 GB | 4K | No | Better quality |
| llama3.2-vision:11b | 7.8 GB | 128K | No | Large context |
| ministral-3:14b | 7.5 GB | 32K | Yes | Multi-image, general purpose |

**Note:** The default model (qwen3.5:4b) is multimodal and can handle vision tasks.

```bash
# Install vision models
ollama pull qwen3.5:4b           # Default (multimodal, also handles vision)
ollama pull llava:13b            # Optional, better quality
ollama pull llama3.2-vision:11b  # Optional, large context
ollama pull ministral-3:14b      # Optional, multi-image support
```

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
default = "qwen3.5:4b"      # Global default

[model.query]
model = "qwen3.5:4b"        # For queries
thinking = true
tools = true

[model.summarize]
model = "qwen3.5:4b"        # For summarization
thinking = false
tools = false

[model.code]
model = "qwen3.5:9b"        # For code (larger model)
tools = true

[model.vision]
model = "qwen3.5:4b"        # For vision (multimodal)
tools = false
```

## Model Capabilities

| Model | Tools | Vision | Think | Local | Size | Notes |
|-------|-------|--------|-------|-------|------|-------|
| qwen3.5:4b | Yes | Yes | Yes | Yes | 3.4 GB | Default, multimodal |
| qwen3.5:9b | Yes | Yes | Yes | Yes | 6.6 GB | **Recommended upgrade** |
| qwen3.5:27b | Yes | Yes | Yes | Yes | 17 GB | Overkill for most |
| translategemma | No | No | No | Yes | ~3 GB | Translation specialist |
| glm-ocr | No | Yes | No | Yes | 2.2 GB | OCR specialist |
| llama3.1:8b | Yes | No | No | Yes | 4.9 GB | Alternative general |
| moondream:1.8b | No | Yes | No | Yes | 1.7 GB | Alternative vision (light) |
| llava:13b | No | Yes | No | Yes | 8.0 GB | Better vision quality |
| llama3.2-vision:11b | No | Yes | No | Yes | 7.8 GB | Large context vision |
| ministral | Yes | Yes | No | Yes | 7.5 GB | Multi-image support |
| qwen3 | No | No | Yes | Yes | ~5 GB | Thinking support |
| qwen3-coder | Yes | No | No | Yes | ~17 GB | Code specialist |
| glm-5 | Yes | No | Yes* | No | Cloud | Complex reasoning |
| kimi-k2.5 | Yes | Yes | Yes* | No | Cloud | Multimodal cloud |
| minimax-m2.5 | Yes | No | Yes* | No | Cloud | Agentic tasks |
| qwen3.5:cloud | Yes | Yes | Yes* | No | Cloud | Large context |
| **nemotron-3-nano:4b** | Yes | No | No | Yes | 2.8 GB | **Efficient tool calling** |
| **gemma4:e2b** | Yes | No | No | Yes | 7.2 GB | **Native FC (Google)** |
| nanbeige4.1:3b | Yes | No | Yes | Yes | 2.4 GB | Tools + thinking |
| ministral-3:3b | Yes | Yes | No | Yes | 3.0 GB | Fast FC (temp=0.3) |

\* Cloud models support thinking via `thinking = true` in config.

## Choosing a Model

### For General Queries
```bash
ask-ai "Your question"           # Default model (qwen3.5:4b)
ask-ai -m "qwen3.5:9b" "question" # Better quality (recommended)
ask-ai -m ministral "question"   # Fast, capable
ask-ai -m qwen3 -t "reasoning"   # With thinking
```

### For Code
```bash
ask-ai -m "qwen3.5:9b" "Write a Rust function"  # Good for code
ask-ai -m "qwen3.5:27b" "Complex refactoring"   # Overkill for simple tasks
ask-ai -m qwen3-coder "Write a Rust function"   # Code specialist
```

### For Vision
```bash
ask vision photo.png                        # Default (qwen3.5:4b)
ask vision -m "qwen3.5:9b" photo.png        # Better quality
ask vision -m moondream photo.png           # Lightweight alternative
ask vision -m llava:13b photo.png --detailed  # Better quality
```

### For Cloud Models
```bash
ask-ai -m glm-5 "Complex analysis"
ask-ai -m kimi-k2.5 "Multimodal task"
ask-ai -m minimax-m2.5 "Coding task"
```

### Quick Recommendations

| Use Case | Recommended Model | Why |
|----------|-------------------|-----|
| Daily use | qwen3.5:4b | Default, multimodal, good balance |
| **Better quality** | **qwen3.5:9b** | **Recommended upgrade, worth the RAM** |
| Complex tasks | qwen3.5:27b | Overkill, only if you need max reasoning |
| Translation | translategemma | Specialist |
| OCR | glm-ocr | Specialist |
| Vision | qwen3.5:4b | Multimodal (same as default) |

## Custom Models

Add your own models in `~/.config/ask-ai/models.toml`:

```toml
[models."my-model"]
model_id = "my-model:7b"    # Required
num_ctx = 32768             # Optional: context window
temperature = 0.7           # Optional: temperature
top_k = 40                  # Optional: top-k
top_p = 0.9                 # Optional: top-p
repeat_penalty = 1.1        # Optional: repeat penalty
thinking = true             # Optional: for models that support it
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
- [vision](./commands/vision.md) - Vision command
- [Configuration](./configuration.md) - Custom models setup