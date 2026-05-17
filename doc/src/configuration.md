# Configuration Guide

This guide covers how to configure Sprachspiel for your specific needs.

## Configuration File

Sprachspiel supports a user configuration file for persistent settings. This is the recommended way to customize the tool.

### Creating the Config File

Generate a sample configuration file:

```bash
sprach --init-config
```

This creates `~/.config/sprachspiel/config.toml` with all available options commented out.

### Config File Location

Sprachspiel looks for the config file in this order:

1. `$XDG_CONFIG_HOME/sprachspiel/config.toml` (if XDG_CONFIG_HOME is set)
2. `~/.config/sprachspiel/config.toml` (default)

### Configuration Options

```toml
# Sprachspiel Configuration File
# Location: ~/.config/sprachspiel/config.toml
# 
# This is a complete example configuration showing all available options.
# Lines starting with '#' are comments and are ignored.
# Remove the '#' to enable an option, or modify values as needed.
# 
# After editing, the configuration takes effect immediately on the next run.

# =============================================================================
# MODEL CONFIGURATION
# =============================================================================
# Configure which AI models to use for different tasks.

[model]

# The default model preset to use for general queries.
# See all available models with: sprach --list-models
# Default: "qwen3.5:4b"
default = "qwen3.5:4b"

# Global default for thinking mode (optional).
# This is used as a fallback for all subcommands that don't have their own setting.
# Subcommand-specific settings (model.query.thinking, etc.) override this.
# Model capability takes precedence: if the model doesn't support thinking, this is ignored.
# thinking = false

# LLM server connection settings.
# Change these if your LLM server is not running on the default localhost.
# Default: "127.0.0.1"
ollama_host = "127.0.0.1"
# Default: 11434
ollama_port = 11434

# -----------------------------------------------------------------------------
# PER-SUBCOMMAND MODEL OVERRIDES (Optional)
# -----------------------------------------------------------------------------
# You can use different models for different subcommands.
# This allows you to use lightweight models for simple tasks and 
# powerful models for complex ones, optimizing for speed and cost.

# --- QUERY SUBCOMMAND ---
[model.query]
# The model to use for 'ask query' or 'ask q'.
# If not specified, falls back to the global [model] default.
# model = "qwen3.5:4b"

# Enable thinking mode for queries. Some models show their reasoning process.
# If not specified, defaults to: true for query
# thinking = true

# Enable tool calling for queries (weather, file operations, etc.).
# If not specified, defaults to: true for query
# tools = true

# --- CHAT SUBCOMMAND ---
[model.chat]
# The model to use for 'ask chat'.
# If not specified, falls back to the global [model] default.
# model = "qwen3.5:4b"

# Enable thinking mode for chat. Some models show their reasoning process.
# If not specified, defaults to: false for chat
# thinking = false

# Enable tool calling for chat (weather, file operations, etc.).
# If not specified, defaults to: true for chat
# tools = true

# --- SUMMARIZE SUBCOMMAND ---
[model.summarize]
# The model to use for 'ask summarize'.
# Recommended: a lightweight model for speed.
# If not specified, falls back to the global [model] default.
# model = "qwen3.5:4b"

# Summarization typically doesn't need thinking mode.
# If not specified, defaults to: false for summarize
# thinking = false

# Summarization doesn't use external tools.
# If not specified, defaults to: false for summarize
# tools = false

# --- CODE MODE ---
[model.code]
# The model to use when the code flag (-c) is active.
# Default: qwen2.5-coder:7b (optimized for coding with function calling)
# If not specified, falls back to the code default (qwen2.5-coder:7b).
# model = "qwen2.5-coder:7b"

# Code generation typically doesn't need thinking mode.
# If not specified, defaults to: false for code
# thinking = false

# Enable tool calling in code mode. This allows the model to inspect 
# your project files (read_file, list_directory, search_files) before 
# generating code, leading to more accurate suggestions.
# If not specified, defaults to: true for code
# tools = true

# =============================================================================
# TOOLS CONFIGURATION
# =============================================================================
# Control which AI tools are available and how they behave.

[tools]

# A list of tools to disable (blacklist).
# Blacklisted tools won't be available to the AI, saving context window space.
#
# Available tools include:
#   - get_current_datetime, get_project_context (System information)
#   - get_weather, get_current_weather, get_weather_forecast (Weather)
#   - read_file, list_directory, search_files (File operations)
#   - fetch_pokemon, fetch_pokemon_stats, etc. (Pokémon data)
#   - serper_search, serper_search_news (Serper API web search - requires SERPER_API_KEY)
#   - web_search, web_search_news, web_instant_answer (DuckDuckGo - may fail due to CAPTCHA)
#
# Note: DuckDuckGo tools may be blocked by CAPTCHA. Use Serper tools for reliable web search.
# Default: [] (all enabled tools are available)
blacklist = []

# =============================================================================
# OUTPUT CONFIGURATION
# =============================================================================
# Control how responses are displayed.

[output]

# Use plain text output by default, disabling markdown rendering.
# If true, responses will be plain text instead of formatted markdown.
# Default: false
plain_default = false

# Verbosity level for log output (optional).
# Controls how much diagnostic information is shown alongside the LLM response.
#
# Options:
#   "quiet"   — Errors only. No spinner, no tool calls. Ideal for scripting/pipes.
#   "normal"  — Tool calls (compact), warnings, errors. Good default for interactive use.
#   "verbose" — Detailed tool calls with full parameters and results. For debugging.
#   "trace"   — Everything including embedding internals, token budgets. Maximum info.
#
# Priority: CLI flags (-v/-q) > RUST_LOG env var > this setting > default
# Default: "normal" (info level)
# verbosity = "normal"

# =============================================================================
# DISPLAY CONFIGURATION
# =============================================================================
# Customize the terminal appearance.

[display]

# The color theme for markdown rendering.
# Options: "dark" (Catppuccin Mocha), "light" (Catppuccin Latte), or "mono" (monochrome)
# Default: "dark"
skin = "dark"
```

### Configuration Precedence

Settings are applied in this priority order (highest first):

1. **Command-line arguments** - Override everything
2. **Config file** - Persistent user preferences
3. **Default values** - Built-in defaults

#### Model Selection Precedence

When choosing which model to use for a subcommand:

1. **CLI flag** (`-m/--model`) - Always takes precedence
2. **Subcommand config** (`model.chat.model`, `model.query.model`, etc.)
3. **Global default** (`model.default`)

#### Thinking Mode Precedence

When determining if thinking mode is enabled:

1. **Model capability** - Some models don't support thinking (checked first)
2. **CLI flag** (`-t/--think`) - User override
3. **Subcommand config** (`model.chat.thinking`, `model.query.thinking`, etc.)
4. **Global config** (`model.thinking`)
5. **Model default** (`thinking = true` in models.toml)
6. **Hardcoded default** (true for query, false for others)

Note: If thinking is enabled in config but the model doesn't support it, a warning is displayed and thinking is disabled.

## Custom Models

Sprachspiel supports user-defined models via a TOML file. This allows you to:

- Add new models not included in the built-in presets
- Override parameters for existing models (partial override)

### Creating Custom Models

Create `~/.config/sprachspiel/models.toml`:

```toml
# Custom model definitions
# Location: ~/.config/sprachspiel/models.toml

# Add a new model
[models.my-coder]
model_id = "phi3:mini-4k"    # Required: Model ID (as recognized by the backend)
num_ctx = 4096                # Optional: context window (default: 4096)
temperature = 0.3             # Optional: temperature (default: 0.2)
top_k = 40                    # Optional: top-k sampling (default: 40)
top_p = 0.9                   # Optional: top-p sampling (default: 0.9)
repeat_penalty = 1.1          # Optional: repeat penalty (default: 1.0)

# Add another model with minimal config
[models.simple]
model_id = "llama3:8b"        # Only model_id required

# Override built-in model (partial override)
[models.lfm]
temperature = 0.15            # Only override what you want to change
```

### Using Custom Models

```bash
# Use a custom model
sprach -m my-coder "Write a function"

# Use in chat mode
sprach chat -m simple

# Override built-in model parameters
sprach -m lfm "query"  # Uses modified temperature from models.toml
```

### Model Parameter Defaults

When defining a custom model without all parameters, these defaults are used:

| Parameter    | Default |
|-------------|---------|
| `num_ctx`    | 32768 (32K) |
| `temperature`| 0.8     |
| `top_k`      | not set (uses backend default) |
| `top_p`      | not set (uses backend default) |
| `repeat_penalty` | 1.1 |

**Note**: If `num_ctx` is not specified, the default is 32K tokens. For cloud models or models where you want Ollama to automatically manage context, you can omit `num_ctx` entirely.

### Enabling Thinking for Cloud Models

Cloud models with `:cloud` tag can have thinking enabled by setting `thinking = true` in the model config:

```toml
[models.glm-5]
model_id = "glm-5:cloud"
thinking = true

[models.kimi-k2.5]
model_id = "kimi-k2.5:cloud"
thinking = true
```

When `thinking = true` is set:
- The model will use thinking mode by default
- You can still disable it with `-t false` or via CLI flag

### Listing All Models

```bash
sprach --list
```

This shows both built-in models and user-defined models (marked with `[user]`).


### Remote LLM Server

To connect to a remote LLM server (such as Ollama):

```toml
[model]
# Both formats work - with or without http://
ollama_host = "192.168.1.100"
# Or explicitly: ollama_host = "http://192.168.1.100"
ollama_port = 11434
```

This is useful for:
- **Termux/Android** - Connect to an Ollama server running on your desktop
- **Remote servers** - Connect to an LLM server on a different machine
- **Docker/containers** - Connect to an LLM server in a container

Or use environment variables:

```bash
export OLLAMA_HOST="192.168.1.100:11434"
```

## Per-Subcommand Configuration

You can configure different models for different subcommands. This allows you to use lightweight models for simple tasks and powerful models for complex ones.

### Available Subcommand Sections

- `[model.query]` - Configuration for `ask query` (or `ask q`)
- `[model.summarize]` - Configuration for `ask summarize`
- `[model.code]` - Configuration for code mode (`ask query -c`)

### Example Configuration

```toml
[model]
# Global default model
default = "ministral"

[model.query]
# Model for general queries
model = "ministral"
thinking = false
tools = true

[model.summarize]
# Model for summarization
model = "qwen3"
thinking = false
tools = false

[model.code]
# Model for code generation
model = "qwen3-coder"
tools = true
```

### Configuration Precedence

For subcommands, the priority is:

1. **CLI flags** - `-m`, `-t`, `--tools`
2. **Subcommand-specific config** - e.g., `[model.code]` settings
3. **Global default** - from `[model]` section

Example: If you run `ask query -c "function"` with the above config:
- It will use `qwen3-coder` (from `[model.code]`)
- Not `ministral` (global default)

### Options

Each subcommand section supports:

- `model` - Model preset name (optional, uses global default if not set)
- `thinking` - Enable think mode (optional, defaults: true for query, false for others)
- `tools` - Enable tools (optional, defaults: true for query/code, false for summarize)

## Tool Configuration

### How Tool Filtering Works

Sprachspiel uses a two-layer filtering system for tools:

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
# Disable specific tools
blacklist = ["fetch_pokemon", "get_pokemon_ability"]
```

**Important**: When a tool is blacklisted, it's completely hidden from the model. The system prompt won't mention the tool, and the model won't try to use it. This saves context window space.

**Note**: DuckDuckGo web search tools (`web_search`, `web_search_news`, `web_instant_answer`) may fail due to CAPTCHA. For reliable web search, use Serper tools (`serper_search`, `serper_search_news`) which require the `SERPER_API_KEY` environment variable.

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
sprach --tools "Tell me about Pikachu"
```

## Environment Variables

### OLLAMA_HOST

Configure the LLM server location (overrides config file):

```bash
# Default (local)
export OLLAMA_HOST="localhost:11434"

# Remote server
export OLLAMA_HOST="192.168.1.100:11434"

# Add to shell config
echo 'export OLLAMA_HOST="localhost:11434"' >> ~/.bashrc
```

### RUST_LOG

Enable debug logging via the standard Rust environment variable:

```bash
export RUST_LOG=debug
```

## Model Configuration

### Default Model

Set in config file:

```toml
[model]
```

Or via environment variable (highest priority):

```bash
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

### Verbose Logging

Verbose logging shows:
- Model configuration (temperature, context, etc.)
- Tool calls and their results
- Model capabilities detection
- Raw responses (when verbose level is enabled)

Enable via:
- CLI flag: `sprach -v "query"` (verbose), `sprach -vv "query"` (trace)
- Environment: `RUST_LOG=trace sprach command` or `RUST_LOG=debug sprach command)`

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

- Binary: `/usr/local/bin/sprach`
- Man page: `/usr/local/share/man/man1/sprach.1`
- Config: `~/.config/sprachspiel/config.toml`

### Custom Prefix

```bash
make install PREFIX=$HOME/.local
# Installs to:
# - $HOME/.local/bin/sprach
# - $HOME/.local/share/man/man1/sprach.1
```

## Shell Completion

Generate completions for your shell:

```bash
# Bash
sprach completion bash >~/.bash_completion

# Zsh
sprach completion zsh > ~/.zsh_completions/_sprach
# Add to ~/.zshrc: fpath+=(~/.zsh_completions)

# Fish
sprach completion fish > ~/.config/fish/completions/sprach.fish
```

See [Installation Guide](./installation.md#shell-completions) for more details.

## Tips

1. **Use config file for persistent preferences** - CLI args for one-off changes
2. **Keep models local** for offline use
3. **Use appropriate models** for specific tasks (translation, coding, etc.)
4. **Monitor token usage** with debug mode enabled
5. **Adjust temperature** based on whether you need creativity or determinism
6. **Sandbox file tools** in untrusted environments
7. **Use AGENTS.md** for project-specific context

## AGENTS.md Context

Sprachspiel automatically loads `AGENTS.md` from the current directory to provide project-specific context to the model.

### How It Works

When you run a query from a directory containing `AGENTS.md`:

1. The file is read and sanitized for security
2. Content is framed as project context
3. It's injected into the system prompt
4. The model uses this context for better responses

### Security Measures

Content is sanitized to prevent prompt injection:

- **Size limit:** 1000 lines max (warning at 500+)
- **Pattern removal:** Injection patterns like "ignore previous instructions"
- **Tag removal:** Fake system tags like `[SYSTEM]`, `<instruction>`
- **Code block removal:** Executable blocks (bash, python, javascript, etc.)

### Example AGENTS.md

```markdown
# Project Guidelines

## Build Commands
- `cargo build --release` - Build the project
- `cargo test` - Run tests

## Code Style
- Use snake_case for functions
- Maximum line length: 100 characters
- Run `cargo fmt` before commits

## Notes
- This project uses Tokio for async runtime
- All errors should return AppResult<T>
```

### Disabling Context

Use `--ignore-agents` to skip loading:

```bash
sprach --ignore-agents "General programming question"
```

## Troubleshooting

### Config file not being read

Check that the file exists and has correct permissions:

```bash
ls -la ~/.config/sprachspiel/config.toml
```

Test with debug mode to see active configuration:

```bash
sprach -d --init-config  # Shows where config was created
sprach -d "test query"   # Shows active settings
```

### Changes not taking effect

Remember precedence: CLI args > Config file > Defaults. You may be overriding the config with a CLI argument.

## See Also

- [Models](./models.md) - Available models and their characteristics
- [Prompts](./prompts.md) - Prompt modes and customization
- [Tools](./tools.md) - Available tools and configuration
- [Installation](./installation.md) - Setup and installation guide
- [Troubleshooting](./troubleshooting.md) - Common issues and solutions
