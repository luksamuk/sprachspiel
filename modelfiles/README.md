# Model Files for Ask-AI

This directory contains Ollama modelfiles for models supported by Ask-AI.

## What are Modelfiles?

Modelfiles define how Ollama creates and configures models. They specify:
- Base model to pull from Ollama or Hugging Face
- Context window sizes
- Sampling parameters (temperature, top_k, top_p)
- Stop tokens and other configurations

**Note**: Since v0.14.0, context window size is configured in `~/.config/ask-ai/models.toml`, not in model tags. This allows Ollama to auto-detect context based on available memory.

## Quick Start

### Install Essential Models (Required)

These are the models required for basic functionality:

```bash
cd modelfiles
make models-essential
```

This installs:
- **qwen3.5:4b** - Default for general queries (131K context, multimodal)
- **translategemma:4b** - For translation (4K context)
- **glm-ocr:bf16** - For OCR
- **moondream:1.8b** - For vision/image description (alternative)

### Install Optional Models (Recommended)

For enhanced functionality:

```bash
make models-optional
```

This installs:
- **lfm2.5-thinking:1.2b** - Reasoning with thinking mode
- **llama3.2:3b** - Fast summarization with tools
- **mistral-small3.2:24b** - Tool-capable
- **qwen3-coder:30b** - Code generation

### Install All Models

```bash
make models-all        # All local models
make models            # Everything
```

## Available Make Targets

### Essential Models
- `qwen3.5:4b` - Qwen 3.5 4B (default for query, multimodal)
- `translategemma` - Translation model
- `glm-ocr` - OCR model (pull-only)
- `llama3.1` - Llama 3.1 8B (alternative for general queries)

### Optional Models
- `lfm` - LFM 2.5 Thinking (reasoning)
- `llama3.2` - Llama 3.2 3B (summarization, tools)
- `mistral-small` - Tool-capable
- `qwen3-coder` - Code generation
- `pepe` - Character model
- `devstral-small-2` - Coding focused
- `smollm3` - Lightweight
- `sead` - General purpose

### Cloud Models
Cloud models are configured in `~/.config/ask-ai/models.toml` and don't require modelfiles.

### Combined Targets
- `models-essential` - Essential models only
- `models-optional` - Recommended optional models
- `models-all` - All local models
- `models` - Everything
- `list` - Show installed models

## Modelfile Format

Each `.modelfile` contains:

```
FROM <base-model>
PARAMETER num_ctx <context-size>
PARAMETER temperature <temp>
PARAMETER top_k <top-k>
PARAMETER top_p <top-p>
PARAMETER repeat_penalty <penalty>
PARAMETER stop <stop-token>
```

Example: `lfm2.5-thinking.modelfile`
```
FROM lfm2.5-thinking:1.2b
PARAMETER num_ctx 32768
PARAMETER temperature 0.1
PARAMETER top_k 50
PARAMETER top_p 0.1
PARAMETER repeat_penalty 1.05
PARAMETER stop 
```

## How It Works

The Makefile:
1. Pulls the base model from Ollama/Hugging Face
2. Creates a new model with the modelfile configuration
3. Removes the temporary base model
4. The result is a model with the exact name and parameters Ask-AI expects

## Manual Installation

If you prefer not to use modelfiles, pull directly:

```bash
# Essential models (just pull, no modelfile needed)
ollama pull qwen3.5:4b
ollama pull translategemma:4b
ollama pull glm-ocr:bf16
# Optional vision alternative:
ollama pull moondream:1.8b
# Optional general alternative:
ollama pull llama3.1:8b

# Optional - some need modelfiles for custom config
ollama pull lfm2.5-thinking:1.2b
ollama pull llama3.2:3b
```

**Note:** Models without modelfiles use Ollama's default parameters. For context window sizes, configure in `~/.config/ask-ai/models.toml`.

## Context Window Configuration

Since v0.14.0, context window sizes are defined in `~/.config/ask-ai/models.toml`:

```toml
[models."qwen3.5:4b"]
model_id = "qwen3.5:4b"
num_ctx = 131072
temperature = 1.0
thinking = true

[models.lfm]
model_id = "lfm2.5-thinking:1.2b"
num_ctx = 32768  # 32K context
```

This approach:
- Lets Ollama optimize context based on available memory
- Avoids duplicate models with different context tags
- Allows easy tuning without rebuilding models

## Troubleshooting

### Model Installation Fails

1. Check Ollama is running: `ollama serve`
2. Try pulling base model manually first
3. Check internet connection for HF models

### Model Not Found

```bash
# Check installed models
ollama list

# Install missing model
make llama3.1
```

### Cleanup

Remove temporary files (not installed models):
```bash
make clean-models
```

Remove installed models:
```bash
ollama rm <model-name>
```

## Source Information

- **Hugging Face**: Many models come from `hf.co/unsloth/`, `hf.co/ggml-org/`, etc.
- **Ollama Library**: Some models from `ollama.com/library/`
- **Cloud Models**: Configured in `~/.config/ask-ai/models.toml`

See individual modelfiles for specific source URLs.