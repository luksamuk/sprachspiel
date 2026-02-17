# Configuration Guide

This guide covers how to configure Ask-AI for your specific needs.

## Environment Variables

### OLLAMA_HOST

Configure the Ollama server location:

```bash
# Default (local)
export OLLAMA_HOST="localhost:11434"

# Remote server
export OLLAMA_HOST="192.168.1.100:11434"

# Add to shell config
echo 'export OLLAMA_HOST="localhost:11434"' >> ~/.bashrc
```

## Model Configuration

Models are configured in `src/config.rs`. Each preset includes:

- Model ID
- Temperature
- Context window
- Sampling parameters

### Default Model

Change the default model in code:

```rust
// In src/main.rs or src/config.rs
const DEFAULT_MODEL: &str = "lfm";
```

### Custom Models

Add new model presets:

```rust
// In src/config.rs
pub fn get(model: &str) -> AppResult<ModelConfig> {
    match model {
        // ... existing models ...
        "my-custom" => Ok(ModelConfig {
            model_id: "custom-model".to_string(),
            temperature: 0.5,
            // ... other options ...
        }),
        _ => Err(format!("Unknown model: {}", model).into()),
    }
}
```

## Prompt Configuration

System prompts are defined in `src/prompts.rs`:

```rust
// Custom prompt
pub const SYSTEM_PROMPT_CUSTOM: &str = r#"\
Your custom instructions here..."#;
```

## Debug Configuration

### Enable Debug Logging

Set the `ASK_AI_DEBUG` environment variable:

```bash
export ASK_AI_DEBUG=1
```

### Debug Output

Debug mode shows:
- Model configuration
- Tool calls and results
- Timing information
- Raw responses

## Installation Paths

### Default Paths

- Binary: `/usr/local/bin/ask-ai`
- Man page: `/usr/local/share/man/man1/ask-ai.1`

### Custom Prefix

```bash
make install PREFIX=$HOME/.local
# Installs to:
# - $HOME/.local/bin/ask-ai
# - $HOME/.local/share/man/man1/ask-ai.1
```

## Shell Completion

### Bash

```bash
ask-ai --generate-completion bash > /etc/bash_completion.d/ask-ai
```

### Zsh

```bash
ask-ai --generate-completion zsh > /usr/local/share/zsh/site-functions/_ask-ai
```

### Fish

```bash
ask-ai --generate-completion fish > ~/.config/fish/completions/ask-ai.fish
```

## Performance Tuning

### Model Parameters

Adjust in `src/config.rs`:

```rust
ModelConfig {
    temperature: 0.1,  // Lower = more deterministic
    top_k: 20,         // Token sampling
    top_p: 0.9,        // Nucleus sampling
    repeat_penalty: 1.1, // Reduce repetition
}
```

### Timeout Settings

Set Ollama timeout:

```bash
export OLLAMA_TIMEOUT=120  # seconds
```

## Tips

1. **Keep models local** for offline use
2. **Use appropriate models** for tasks
3. **Monitor token usage** with debug mode
4. **Adjust temperature** for creativity vs determinism

## See Also

- [Models](./models.md) - Available models
- [Prompts](./prompts.md) - Prompt modes
- [Installation](./installation.md) - Setup guide
