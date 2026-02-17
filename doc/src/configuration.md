# Configuration Guide

This guide covers how to configure Ask-AI for your specific needs.

## Configuration File

Ask-AI supports a user configuration file for persistent settings. This is the recommended way to customize the tool.

### Creating the Config File

Generate a sample configuration file:

```bash
ask-ai --init-config
```

This creates `~/.config/ask-ai/config.toml` with all available options commented out.

### Config File Location

Ask-AI looks for the config file in this order:

1. `$XDG_CONFIG_HOME/ask-ai/config.toml` (if XDG_CONFIG_HOME is set)
2. `~/.config/ask-ai/config.toml` (default)

### Configuration Options

```toml
# Ask-AI Configuration File
# Located at ~/.config/ask-ai/config.toml

[model]
# Default model preset to use when none is specified
# See available models with: ask-ai --list-models
default = "lfm"

# Ollama server connection settings
# Default: localhost on port 11434
ollama_host = "127.0.0.1"
ollama_port = 11434

[tools]
# Tools to disable (blacklist)
# Available tools: web_search, web_search_news, web_instant_answer,
#   get_weather, get_current_weather, get_weather_forecast,
#   fetch_pokemon, fetch_pokemon_basic, fetch_pokemon_stats,
#   fetch_pokemon_moves, fetch_pokemon_evolution, fetch_ability_details,
#   fetch_type_effectiveness, fetch_move_details,
#   read_file, list_directory, search_files
blacklist = ["web_search", "web_instant_answer"]

# Sandboxing for file operations tools
# When enabled, file tools can only access files in the current directory
# and its subdirectories. Disable only if you understand the risks.
file_sandbox = true

[output]
# Use plain text output by default (no markdown rendering)
plain_default = false

# Enable debug output by default
# Shows all tool calls, model parameters, and raw responses
debug_default = false

[display]
# Terminal skin for markdown rendering
# Options: dark, light, or mono
skin = "dark"
```

### Configuration Precedence

Settings are applied in this priority order (highest first):

1. **Command-line arguments** - Override everything
2. **Config file** - Persistent user preferences
3. **Default values** - Built-in defaults

Example: If config file sets `default = "gpt-oss"` but you run `ask-ai -m lfm "query"`, the CLI argument wins.

### Remote Ollama Server

To connect to a remote Ollama instance:

```toml
[model]
ollama_host = "192.168.1.100"
ollama_port = 11434
```

Or use environment variables:

```bash
export OLLAMA_HOST="192.168.1.100:11434"
```

## Tool Configuration

### How Tool Filtering Works

Ask-AI uses a two-layer filtering system for tools:

1. **Compile-time (Feature Flags)**: Tools are included/excluded at build time
   - See [Tools documentation](./tools.md#compilation-features) for feature flags
   - Pokémon tools are disabled by default to save context window space

2. **Runtime (Blacklist)**: Tools can be disabled per-session via configuration
   - Blacklisted tools won't be registered AND won't appear in prompts
   - The model won't know these tools exist

**Example workflow:**
- Build with: `cargo build --release` (no Pokémon tools by default)
- Blacklist in config: `blacklist = ["web_search"]` (hides web search from model)

### Disabling Tools at Runtime

Some tools may not work in your environment or you may want to disable them:

```toml
[tools]
# Disable web search (currently broken)
blacklist = ["web_search", "web_search_news", "web_instant_answer"]
```

**Important**: When a tool is blacklisted, it's completely hidden from the model. The system prompt won't mention the tool, and the model won't try to use it. This saves context window space.

### Enabling Pokémon Tools

Pokémon tools are **disabled by default** at compile time to avoid polluting the context window. To enable them:

```bash
# Build with Pokémon tools
cargo build --release --features pokemon-tools

# Or enable all tools
cargo build --release --features all-tools
```

Then, to use them:
```bash
ask-ai --tools "Tell me about Pikachu"
```

### File Tool Sandboxing

File operation tools (`read_file`, `list_directory`, `search_files`) are sandboxed by default to the current working directory for security. This prevents the AI from accessing files outside your project.

To disable sandboxing (not recommended):

```toml
[tools]
file_sandbox = false
```

## Environment Variables

### OLLAMA_HOST

Configure the Ollama server location (overrides config file):

```bash
# Default (local)
export OLLAMA_HOST="localhost:11434"

# Remote server
export OLLAMA_HOST="192.168.1.100:11434"

# Add to shell config
echo 'export OLLAMA_HOST="localhost:11434"' >> ~/.bashrc
```

### ASK_AI_DEBUG

Enable debug logging globally:

```bash
export ASK_AI_DEBUG=1
```

## Model Configuration

### Default Model

Set in config file:

```toml
[model]
default = "gpt-oss"  # Your preferred default
```

Or via environment variable (highest priority):

```bash
ask-ai -m gpt-oss "your query"
```

### Custom Model Presets

Models are configured in `src/config.rs`. Each preset includes:

- Model ID
- Temperature
- Context window size
- Sampling parameters (top_k, top_p)
- Repeat penalty

To add a custom model, you need to modify the source code and rebuild:

```rust
// In src/config.rs
configs.insert(
    "my-model",
    ModelConfig {
        model_id: "my-custom-model:latest".to_string(),
        num_ctx: 32768,
        temperature: 0.5,
        top_k: 40,
        top_p: 0.9,
        repeat_penalty: 1.1,
    },
);
```

## Prompt Configuration

System prompts are defined in `src/prompts.rs`. To customize prompts, edit the source code:

```rust
// In src/prompts.rs
pub const SYSTEM_PROMPT_CUSTOM: &str = r#"\
Your custom instructions here..."#;
```

Then reference it with `-p custom` flag.

## Debug Configuration

### Debug Output

Debug mode shows:
- Model configuration (temperature, context, etc.)
- Tool calls and their results
- Model capabilities detection
- Raw responses

Enable via:
- Config file: `debug_default = true`
- CLI flag: `ask-ai -d "query"`
- Environment: `ASK_AI_DEBUG=1`

## Performance Tuning

### Model Parameters

Adjust in your config file (or model presets in `src/config.rs`):

```toml
# Lower temperature = more deterministic, less creative
# Range: 0.0 to 2.0
temperature = 0.1

# Top-k sampling limits token selection to k most likely
# Lower = more focused, higher = more diverse
top_k = 40

# Top-p (nucleus) sampling: consider tokens with cumulative prob < p
top_p = 0.9

# Penalty for repeating tokens
repeat_penalty = 1.1
```

### Timeout Settings

Set Ollama timeout via environment variable:

```bash
export OLLAMA_TIMEOUT=120  # seconds
```

## Installation Paths

### Default Paths

- Binary: `/usr/local/bin/ask-ai`
- Man page: `/usr/local/share/man/man1/ask-ai.1`
- Config: `~/.config/ask-ai/config.toml`

### Custom Prefix

```bash
make install PREFIX=$HOME/.local
# Installs to:
# - $HOME/.local/bin/ask-ai
# - $HOME/.local/share/man/man1/ask-ai.1
```

## Shell Completion

Generate completions for your shell:

```bash
# Bash
ask-ai completion bash >~/.bash_completion

# Zsh
ask-ai completion zsh > ~/.zsh_completions/_ask-ai
# Add to ~/.zshrc: fpath+=(~/.zsh_completions)

# Fish
ask-ai completion fish > ~/.config/fish/completions/ask-ai.fish
```

See [Installation Guide](./installation.md#shell-completions) for more details.

## Tips

1. **Use config file for persistent preferences** - CLI args for one-off changes
2. **Keep models local** for offline use
3. **Use appropriate models** for specific tasks (translation, coding, etc.)
4. **Monitor token usage** with debug mode enabled
5. **Adjust temperature** based on whether you need creativity or determinism
6. **Sandbox file tools** in untrusted environments

## Troubleshooting

### Config file not being read

Check that the file exists and has correct permissions:

```bash
ls -la ~/.config/ask-ai/config.toml
```

Test with debug mode to see active configuration:

```bash
ask-ai -d --init-config  # Shows where config was created
ask-ai -d "test query"   # Shows active settings
```

### Changes not taking effect

Remember precedence: CLI args > Config file > Defaults. You may be overriding the config with a CLI argument.

## See Also

- [Models](./models.md) - Available models and their characteristics
- [Prompts](./prompts.md) - Prompt modes and customization
- [Tools](./tools.md) - Available tools and configuration
- [Installation](./installation.md) - Setup and installation guide
- [Troubleshooting](./troubleshooting.md) - Common issues and solutions
