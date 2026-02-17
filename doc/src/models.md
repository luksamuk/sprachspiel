# Available Models

Ask-AI supports multiple model presets optimized for different tasks. Each preset configures the appropriate model, temperature, context window, and sampling parameters.

## Model List

| Preset | Model ID | Context | Best For |
|--------|----------|---------|----------|
| **lfm** | lfm2.5-thinking:1.2b-32k | 32K | General queries (default) |
| gpt-oss | gpt-oss:20b-64k | 64K | Tool calling |
| mistral-small | mistral-small3.2:24b-32k | 32K | Agentic tasks |
| smollm3 | smollm3:Q8_0-64k | 64K | Edge deployment |
| sead | sead:14b-32k | 32K | General purpose |
| qwen3-coder | qwen3-coder:30b-64k | 64K | Code generation |
| devstral-small-2 | devstral-small-2:24b-64k | 64K | Coding with min_p |
| llama3.2 | llama3.2:3b-32k | 32K | Summarization |
| glm-5 | glm-5:cloud | 198K | Cloud reasoning |
| kimi-k2.5 | kimi-k2.5:cloud | 256K | Multimodal agentic |
| minimax-m2.5 | minimax-m2.5:cloud | 198K | Coding and agentic |
| qwen3.5 | qwen3.5:cloud | 256K | Vision-language |
| translate | translategemma:12b-32k | 32K | Translation (fixed) |
| pepe | pepe:8b-64k | 64K | Sarcastic personality |

## Model Categories

### General Purpose

These models work well for everyday queries:

#### lfm (Default)
```bash
ask-ai -m lfm "Your question"
```
- **Model**: lfm2.5-thinking:1.2b-32k
- **Context**: 32K tokens
- **Temperature**: 0.3
- **Best for**: General questions, reasoning, explanations
- **Think mode**: Supported

#### sead
```bash
ask-ai -m sead "Your question"
```
- **Model**: sead:14b-32k
- **Context**: 32K tokens
- **Temperature**: 0.7
- **Best for**: General purpose tasks
- **Think mode**: Not supported

#### pepe (Easter Egg)
```bash
ask-ai -m pepe "Your question"
```
- **Model**: pepe:8b-64k
- **Context**: 64K tokens
- **Temperature**: 0.7
- **Best for**: General queries with sarcastic personality
- **Special**: Injects sarcastic personality into responses
- **Think mode**: Not supported

### Code-Focused

Optimized for programming tasks:

#### qwen3-coder
```bash
ask-ai -m qwen3-coder "Write a function"
```
- **Model**: qwen3-coder:30b-64k
- **Context**: 64K tokens
- **Temperature**: 0.1
- **Best for**: Code generation, debugging, code review
- **Think mode**: Not supported
- **Tools**: Yes

#### devstral-small-2
```bash
ask-ai -m devstral-small-2 "Implement algorithm"
```
- **Model**: devstral-small-2:24b-64k
- **Context**: 64K tokens
- **Temperature**: 0.1
- **Best for**: Coding with min_p sampling
- **Think mode**: Not supported

### Tool-Capable

Models that support tool calling:

#### mistral-small
```bash
ask-ai -m mistral-small "What's the weather?"
```
- **Model**: mistral-small3.2:24b-32k
- **Context**: 32K tokens
- **Temperature**: 0.2
- **Best for**: Agentic tasks with tools
- **Tools**: Full support
- **Think mode**: Not supported

#### gpt-oss
```bash
ask-ai -m gpt-oss "Tell me about Pikachu"
```
- **Model**: gpt-oss:20b-64k
- **Context**: 64K tokens
- **Temperature**: 0.2
- **Best for**: Tool calling (note: may have issues with some tool calls)
- **Tools**: Full support
- **Think mode**: Not supported

### Specialized

Models for specific tasks:

#### llama3.2 (Summarization)
```bash
ask-ai summarize -m llama3.2 "Text..."
```
- **Model**: llama3.2:3b-32k
- **Context**: 32K tokens
- **Temperature**: 0.1
- **Best for**: Summarization (default for summarize command)
- **Tools**: No
- **Think mode**: Not supported

#### translate (Translation)
```bash
ask-ai translate en:pt "Text"  # Fixed model
```
- **Model**: translategemma:12b-32k
- **Context**: 32K tokens
- **Temperature**: 0.1
- **Best for**: Translation tasks (always used for translate)
- **Tools**: No
- **Think mode**: Not supported

### Cloud Models

Remote/cloud-based models (requires internet):

#### glm-5
```bash
ask-ai -m glm-5 "Complex reasoning"
```
- **Model**: glm-5:cloud
- **Context**: 198K tokens
- **Temperature**: 0.7
- **Best for**: Complex reasoning, large context
- **Think mode**: Supported

#### kimi-k2.5
```bash
ask-ai -m kimi-k2.5 "Multimodal task"
```
- **Model**: kimi-k2.5:cloud
- **Context**: 256K tokens
- **Temperature**: 0.7
- **Best for**: Multimodal agentic tasks
- **Think mode**: Not supported

#### minimax-m2.5
```bash
ask-ai -m minimax-m2.5 "Coding task"
```
- **Model**: minimax-m2.5:cloud
- **Context**: 198K tokens
- **Temperature**: 0.7
- **Best for**: Coding and agentic tasks
- **Think mode**: Not supported

#### qwen3.5
```bash
ask-ai -m qwen3.5 "Vision task"
```
- **Model**: qwen3.5:cloud
- **Context**: 256K tokens
- **Temperature**: 0.7
- **Best for**: Vision-language tasks
- **Think mode**: Not supported

## Capability Matrix

| Model | Tools | Vision | Think | Local | Size |
|-------|-------|--------|-------|-------|------|
| lfm | No | No | Yes | Yes | 1.2B |
| mistral-small | Yes | No | No | Yes | 24B |
| gpt-oss | Yes | No | No | Yes | 20B |
| qwen3-coder | Yes | No | No | Yes | 30B |
| sead | No | No | No | Yes | 14B |
| smollm3 | No | No | No | Yes | 3B |
| devstral-small-2 | No | No | No | Yes | 24B |
| llama3.2 | No | No | No | Yes | 3B |
| pepe | No | No | No | Yes | 8B |
| glm-5 | No | No | Yes | No | Cloud |
| kimi-k2.5 | No | No | No | No | Cloud |
| minimax-m2.5 | No | No | No | No | Cloud |
| qwen3.5 | No | No | No | No | Cloud |

## Configuration Details

Each model preset includes:

### Temperature
- **Low (0.1-0.2)**: More deterministic, good for code and technical tasks
- **Medium (0.3-0.7)**: Balanced, good for general queries

### Context Window
- **32K**: Standard size for most tasks
- **64K**: Large documents, codebases
- **198K-256K**: Cloud models for very large contexts

### Sampling Parameters
- **Top K**: 20-50
- **Top P**: 0.1-0.95
- **Repeat Penalty**: 1.0-1.1

## Choosing a Model

### For General Queries
```bash
ask-ai -m lfm "Your question"  # Default, good reasoning
ask-ai -m sead "Your question"  # General purpose
```

### For Coding
```bash
ask-ai -m qwen3-coder "Write a Rust function"
ask-ai -m devstral-small-2 "Implement algorithm"
ask-ai -p code "Optimize this code"
```

### For Tool Usage
```bash
ask-ai -m mistral-small "Get weather in Tokyo"
ask-ai --tools "Tell me about Pikachu"  # Force tools
```

### For Summarization
```bash
ask-ai summarize "Text..."  # Uses llama3.2 by default
ask-ai summarize -m smollm3 "Text..."  # Faster
```

### For Translation
```bash
ask-ai translate en:pt "Text"  # Fixed: translategemma
```

### For Reasoning
```bash
ask-ai -m lfm -t "Complex problem"  # Think mode
ask-ai -m glm-5 -t "Deep reasoning"  # Cloud model
```

## Installation

Before using a model, pull it with Ollama:

```bash
# Essential models
ollama pull lfm2.5-thinking:1.2b-32k
ollama pull translategemma:12b-32k
ollama pull glm-ocr:bf16
ollama pull llama3.2:3b-32k

# Optional models
ollama pull mistral-small3.2:24b-32k
ollama pull qwen3-coder:30b-64k
ollama pull pepe:8b-64k
# ... etc
```

## Model Commands

### List Available Models

```bash
ask-ai --list
```

Shows:
- All model presets
- Model IDs
- Context sizes
- Available prompt modes

### Check Model Status

```bash
# See installed models
ollama list

# Check if specific model exists
ollama show lfm2.5-thinking:1.2b-32k
```

## Best Practices

1. **Use default for most tasks** - `lfm` works well for general queries
2. **Match model to task** - Use code models for coding, etc.
3. **Consider context size** - Large documents need large context
4. **Local vs Cloud** - Local models are faster and work offline
5. **Tool support** - Only certain models support tool calling

## See Also

- [query](./commands/query.md) - Using models for queries
- [summarize](./commands/summarize.md) - Summarization command
- [Configuration](./configuration.md) - Customizing defaults
