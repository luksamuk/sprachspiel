# Model Files for Ask-AI

This directory contains Ollama modelfiles for all models supported by Ask-AI.

## What are Modelfiles?

Modelfiles define how Ollama creates and configures models. They specify:
- Base model to pull from Ollama or Hugging Face
- Context window sizes
- Sampling parameters (temperature, top_k, top_p)
- Stop tokens and other configurations

## Quick Start

### Install Essential Models (Required)

These are the four models required for basic functionality:

```bash
cd modelfiles
make models-essential
```

This installs:
- **lfm2.5-thinking:1.2b-32k** - Default for general queries
- **translategemma:12b-32k** - For translation
- **llama3.2:3b-32k** - For summarization
- **glm-ocr:bf16** - For OCR

### Install Optional Models (Recommended)

For enhanced functionality:

```bash
make models-optional
```

This installs:
- mistral-small3.2:24b-32k (tool-capable)
- gpt-oss:20b-64k (tool calling)
- qwen3-coder:30b-64k (code generation)
- pepe:8b-64k (character model)

### Install All Models

```bash
make models-all        # All local models
make models-cloud      # Cloud-based models
make models            # Everything (local + cloud)
```

## Available Make Targets

### Essential Models
- `lfm` - LFM 2.5 Thinking (default for query)
- `translategemma` - Translation model
- `llama3.2` - Summarization model
- `glm-ocr` - OCR model (pull-only)

### Optional Models
- `mistral-small` - Tool-capable
- `gpt-oss` - Tool calling
- `qwen3-coder` - Code generation
- `pepe` - Character model
- `devstral-small-2` - Coding focused
- `smollm3` - Lightweight
- `sead` - General purpose
- `zephyr` - General
- `llava` - Vision

### Cloud Models
- `cloud-glm-5` - GLM-5 Cloud (198K)
- `cloud-kimi` - Kimi K2.5 Cloud (256K)
- `cloud-minimax` - MiniMax M2.5 Cloud (198K)
- `cloud-qwen35` - Qwen3.5 Cloud (256K)

### Combined Targets
- `models-essential` - Essential models only
- `models-optional` - Recommended optional models
- `models-all` - All local models
- `models-cloud` - Cloud models only
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
PARAMETER stop <|endoftext|>
```

## How It Works

The Makefile:
1. Pulls the base model from Ollama/Hugging Face
2. Creates a new model with the modelfile configuration
3. Removes the temporary base model
4. The result is a model with the exact name and parameters Ask-AI expects

## Manual Installation

If you prefer not to use modelfiles:

```bash
ollama pull lfm2.5-thinking:1.2b-32k
ollama pull translategemma:12b-32k
ollama pull glm-ocr:bf16
ollama pull llama3.2:3b-32k
```

**Note:** Manual installation uses default parameters. Modelfiles provide optimized configurations.

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
make lfm
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
- **Cloud Models**: Pulled from remote APIs via Ollama

See individual modelfiles for specific source URLs.
