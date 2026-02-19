# Available Models

Ask-AI uses a two-tier model system:

1. **Built-in models** - Essential models included with ask-ai
2. **User-defined models** - Custom models via `~/.config/ask-ai/models.toml`

## Built-in Models

These models are configured by default and always available:

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| **llama3.1** | llama3.1:8b | 4K | General queries (default) |
| translategemma | translategemma:12b | 4K | Translation |
| glm-ocr | glm-ocr:bf16 | Auto | OCR/image text extraction |

## User-Defined Models

Additional models are defined in `~/.config/ask-ai/models.toml`. The default file includes:

### General Purpose Models

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| lfm | lfm2.5-thinking:1.2b | 32K | Reasoning with thinking mode |
| llama3.2 | llama3.2:3b | 32K | Fast summarization, tools |
| sead | sead:14b | 32K | General purpose |
| smollm3 | smollm3:Q8_0 | 64K | Edge deployment |

### Tool-Capable Models

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| mistral-small | mistral-small3.2:24b | 32K | Agentic tasks with tools |
| qwen3-coder | qwen3-coder:30b | 64K | Code generation + tools |
| nemotron | nemotron-3-nano:30b | 64K | Code + tools |

### Code Models

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| deepseek-coder-v2 | deepseek-coder-v2:16b | 32K | Code generation (MoE) |
| devstral-small-2 | devstral-small-2:24b | 64K | Coding |

### Cloud Models

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| glm-5 | glm-5:cloud | Auto | Complex reasoning, coding |
| kimi-k2.5 | kimi-k2.5:cloud | Auto | Multimodal agentic |
| minimax-m2.5 | minimax-m2.5:cloud | Auto | Coding, agentic |
| qwen3.5 | qwen3.5:cloud | Auto | Vision-language |

### Character Models

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| pepe | pepe:8b | 64K | Sarcastic entertainment |

User-defined models appear with `[user]` marker in `--list` output.

## Model Categories

### General Purpose

#### llama3.1 (Default)
```bash
ask-ai "Your question"
```
- **Model**: llama3.1:8b
- **Context**: 4K tokens (uses Ollama default if not specified)
- **Temperature**: 0.2
- **Best for**: General queries, coding, explanations
- **Tools**: Supported

#### lfm (User-defined)
```bash
ask-ai -m lfm "Your question"
```
- **Model**: lfm2.5-thinking:1.2b
- **Context**: 32K tokens
- **Temperature**: 0.1
- **Best for**: Reasoning with visible thinking
- **Think mode**: Supported

#### llama3.2 (User-defined)
```bash
ask-ai -m llama3.2 "Your question"
```
- **Model**: llama3.2:3b
- **Context**: 32K tokens
- **Temperature**: 0.2
- **Best for**: Fast summarization, general tasks
- **Tools**: Supported

### Translation

#### translategemma
```bash
ask-ai translate en:pt "Hello world"
```
- **Model**: translategemma:12b
- **Context**: 4K tokens
- **Temperature**: 0.2
- **Best for**: Translation tasks

### Cloud Models

High-capability models with large context windows:

#### glm-5 (User-defined)
```bash
ask-ai -m glm-5 "Complex reasoning task"
```
- **Model**: glm-5:cloud
- **Best for**: Complex reasoning, coding, agentic tasks
- **Tools + Think**: Supported

#### kimi-k2.5 (User-defined)
```bash
ask-ai -m kimi-k2.5 "Multimodal task"
```
- **Model**: kimi-k2.5:cloud
- **Best for**: Multimodal agentic tasks
- **Tools + Vision + Think**: Supported

#### minimax-m2.5 (User-defined)
```bash
ask-ai -m minimax-m2.5 "Coding task"
```
- **Model**: minimax-m2.5:cloud
- **Best for**: Coding, agentic tasks

#### qwen3.5 (User-defined)
```bash
ask-ai -m qwen3.5 "Vision-language task"
```
- **Model**: qwen3.5:cloud
- **Best for**: Vision-language tasks

### Tool-Capable Models (User-defined)

#### mistral-small
```bash
ask-ai -m mistral-small "Search for Rust tutorials"
```
- **Model**: mistral-small3.2:24b
- **Context**: 32K tokens
- **Temperature**: 0.2
- **Best for**: Agentic tasks with tools
- **Tools**: Native support

#### qwen3-coder
```bash
ask-ai -m qwen3-coder "Write a Rust function"
```
- **Model**: qwen3-coder:30b
- **Context**: 64K tokens
- **Temperature**: 0.3
- **Best for**: Code generation + tools
- **Tools**: Supported

### Code Models (User-defined)

#### deepseek-coder-v2
```bash
ask-ai -m deepseek-coder-v2 "Implement an algorithm"
```
- **Model**: deepseek-coder-v2:16b
- **Context**: 32K tokens
- **Temperature**: 0.15
- **Best for**: Fast code generation (MoE: 2.4B active)

#### devstral-small-2
```bash
ask-ai -m devstral-small-2 "Write tests"
```
- **Model**: devstral-small-2:24b
- **Context**: 64K tokens
- **Temperature**: 0.15
- **Best for**: Coding

### OCR

#### glm-ocr
```bash
ask-ai ocr document.png
```
- **Model**: glm-ocr:bf16
- **Best for**: Text extraction from images

### Character Models (User-defined)

#### pepe (Easter Egg)
```bash
ask-ai -m pepe "Tell me a joke"
```
- **Model**: pepe:8b
- **Context**: 64K tokens
- **Temperature**: 1.0
- **Best for**: Sarcastic entertainment
- **Note**: Not for serious use!

## Model Capabilities

| Model | Tools | Vision | Think | Local | Size |
|-------|-------|--------|-------|-------|------|
| llama3.1 | Yes | No | No | Yes | 8B |
| translategemma | No | No | No | Yes | 12B |
| glm-ocr | No | Yes | No | Yes | - |
| lfm | No | No | Yes | Yes | 1.2B |
| llama3.2 | Yes | No | No | Yes | 3B |
| glm-5 | Yes | No | Yes* | No | Cloud |
| kimi-k2.5 | Yes | Yes | Yes* | No | Cloud |
| minimax-m2.5 | Yes | No | Yes* | No | Cloud |
| qwen3.5 | Yes | Yes | Yes* | No | Cloud |
| mistral-small | Yes | No | No | Yes | 24B |
| qwen3-coder | Yes | No | No | Yes | 30B |
| deepseek-coder-v2 | No | No | No | Yes | 16B |

\* Cloud models support thinking via `thinking = true` in `models.toml`

## Choosing a Model

### For General Queries
```bash
ask-ai "Your question"           # Default llama3.1, fast and capable
ask-ai -m lfm "Complex reasoning" # Thinking model for reasoning
```

### For Coding
```bash
ask-ai -m deepseek-coder-v2 "Write a function"  # Fast, efficient
ask-ai -m qwen3-coder "Implement feature"        # With tools
ask-ai -p code "Optimize code"                   # Code prompt mode
```

### For Tool Usage
```bash
ask-ai -m mistral-small "Search for docs"     # Native tool support
ask-ai -m qwen3-coder "Read file and fix"     # Code + tools
```

### For Large Context
```bash
ask-ai -m kimi-k2.5 "Analyze large document"   # Cloud, large context
ask-ai -m glm-5 "Long reasoning task"          # Cloud, reasoning
```

## Custom Models

Add or override models via `~/.config/ask-ai/models.toml`:

```toml
# Add a new model
[models.my-coder]
model_id = "phi3:mini"    # Required: Ollama model ID
num_ctx = 32768           # Optional: context window (default: 32K)
temperature = 0.3         # Optional: temperature (default: 0.8)
top_k = 40                # Optional: top-k sampling (omit to use Ollama default)
top_p = 0.9               # Optional: top-p sampling (omit to use Ollama default)
repeat_penalty = 1.1      # Optional: repeat penalty (default: 1.1)
thinking = true           # Optional: enable thinking mode by default

# Add another model with minimal config (uses defaults)
[models.simple]
model_id = "llama3:8b"    # Only model_id required

# Cloud model - enable thinking
[models.my-cloud]
model_id = "my-model:cloud"
thinking = true           # Enable thinking for cloud models

# Override built-in model (partial override)
[models.llama3.1]
temperature = 0.15        # Only override what you want to change
```

### Model Parameter Defaults

When defining a custom model without all parameters, these defaults are used:

| Parameter    | Default |
|-------------|---------|
| `num_ctx`    | 32768 (32K) |
| `temperature`| 0.2     |
| `top_k`      | 40      |
| `top_p`      | 0.9     |
| `repeat_penalty` | 1.0 |

**Note**: If `num_ctx` is not specified, the default is 32K tokens. For cloud models, you can omit all parameters to let Ollama handle them automatically.

### Listing All Models

```bash
ask-ai --list
```

This shows both built-in models and user-defined models (marked with `[user]`).

### Check Model Status

```bash
# See installed models
ollama list

# Check if specific model exists
ollama show llama3.1:8b
```

## Best Practices

1. **Use default for most tasks** - `llama3.1` works well for general queries
2. **Match model to task** - Use code models for coding, etc.
3. **Consider context size** - Large documents need large context
4. **Local vs Cloud** - Local models are faster and work offline
5. **Tool support** - Only certain models support tool calling

## See Also

- [query](./commands/query.md) - Using models for queries
- [summarize](./commands/summarize.md) - Summarization command
- [Configuration](./configuration.md) - Custom models setup