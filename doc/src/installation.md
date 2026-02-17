# Installation Guide

This guide covers all the ways to install Ask-AI on your system.

## Prerequisites

Before installing Ask-AI, ensure you have:

1. **Ollama** - The LLM server that Ask-AI communicates with
2. **Rust toolchain** - Only needed if building from source
3. **Git** - For cloning the repository

### Installing Ollama

Ask-AI requires Ollama to be running. Install it from [ollama.ai](https://ollama.ai):

```bash
# Linux
curl -fsSL https://ollama.ai/install.sh | sh

# macOS
brew install ollama

# Windows
# Download from https://ollama.ai/download
```

After installing, start the Ollama service:

```bash
ollama serve
```

### Installing Rust (for building from source)

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Installation Methods

### Method 1: Using Make (Recommended)

The easiest way to install Ask-AI is using the provided Makefile:

```bash
# Clone the repository
git clone https://github.com/luksamuk/ask-ollama-rs.git
cd ask-ollama-rs

# Build and install (default: /usr/local)
make install

# Or install to a custom location
make install PREFIX=/usr
make install PREFIX=$HOME/.local
```

This will:
1. Build the release binary
2. Install it to `/usr/local/bin/ask-ai` (or your chosen prefix)
3. Install the man page to `/usr/local/share/man/man1/`

### Method 2: Manual Installation

Build from source manually:

```bash
# Clone and enter repository
git clone https://github.com/luksamuk/ask-ollama-rs.git
cd ask-ollama-rs

# Build release binary
cargo build --release

# Copy to your PATH
sudo cp target/release/ask-ollama /usr/local/bin/ask-ai

# Optional: Install man page
sudo cp man/ask-ai.1 /usr/local/share/man/man1/
```

### Method 3: Development Build

For development or testing:

```bash
# Run directly without installing
cargo run -- "Your query here"

# Or build debug version
cargo build

# Run debug binary
./target/debug/ask-ollama "Your query"
```

## Installing Models

⚠️ **Important:** Ask-AI models **must be built** using the provided modelfiles. Simply pulling models directly from Ollama won't work because our models require custom parameters (context window, temperature, etc.) that are configured in the modelfiles.

### How Model Building Works

Each modelfile:
1. Pulls the base model from Ollama or Hugging Face
2. Creates a new model with **custom parameters** optimized for Ask-AI
3. Names it with the exact ID that Ask-AI expects

**Always use the Makefile targets** - never use `ollama pull` directly.

### Quick Install (Essential Models Only)

Install the four models required for basic functionality:

```bash
# Navigate to modelfiles directory
cd modelfiles

# Build and install all essential models
make models-essential
```

This builds and installs:
- **lfm2.5-thinking:1.2b-32k** - Default for general queries (32K context)
- **translategemma:12b-32k** - For translation command
- **llama3.2:3b-32k** - For summarization command  
- **glm-ocr:bf16** - For OCR/text extraction

### Installing Optional Models

For enhanced functionality with tools and specialized tasks:

```bash
cd modelfiles

# Build and install recommended optional models
make models-optional
```

This builds and installs:
- **mistral-small3.2:24b-32k** - Tool-capable model (32K context)
- **gpt-oss:20b-64k** - Tool calling model (64K context)
- **qwen3-coder:30b-64k** - Code generation (64K context)
- **pepe:8b-64k** - Character model with personality (64K context)

### Installing All Local Models

To build and install all local models (both essential and optional):

```bash
cd modelfiles
make models-all
```

### Installing Cloud Models

Cloud models are pulled directly (no build needed) from remote APIs:

```bash
cd modelfiles
make models-cloud
```

This pulls cloud-based models with large context windows (198K-256K tokens).

### Installing Individual Models

Build individual models as needed:

```bash
cd modelfiles

# Essential models (must have)
make lfm                 # Build LFM 2.5 Thinking (default)
make translategemma      # Build Translation model
make llama3.2            # Build Summarization model
make glm-ocr             # Pull OCR model

# Optional models
make mistral-small       # Build tool-capable model
make gpt-oss            # Build tool calling model
make qwen3-coder        # Build code generation model
make pepe               # Build character model

# See all available targets
make help
```

### About Modelfiles

The `modelfiles/` directory contains `.modelfile` definitions that:
- Specify the base model to pull from Ollama or Hugging Face
- Configure context window sizes (32K, 64K, etc.)
- Set optimized parameters (temperature, top_k, top_p, etc.)
- Define stop tokens and other model-specific settings

Each modelfile creates a customized model with the correct name and configuration for Ask-AI.

### Manual Model Installation (Not Recommended)

**⚠️ Warning:** Direct `ollama pull` will NOT work correctly. Models must be built with custom parameters.

If you attempt manual installation, models will have wrong configuration:
- Wrong context window sizes
- Wrong temperature settings
- Missing stop tokens

The application will fail or behave unexpectedly.

**Always use the Makefile:**
```bash
cd modelfiles
make models-essential
```

## Verifying Installation

After installation, verify Ask-AI is working:

```bash
# Check version
ask-ai --version

# Show help
ask-ai --help

# List available models
ask-ai --list

# Test with a simple query (requires lfm model)
ask-ai "Hello, are you working?"
```

## Uninstallation

### Uninstall Ask-AI Binary

If you installed with Make:

```bash
make uninstall

# Or with custom prefix
make uninstall PREFIX=/usr
```

If you installed manually:

```bash
sudo rm /usr/local/bin/ask-ai
sudo rm /usr/local/share/man/man1/ask-ai.1
```

### Remove Installed Models

To remove models installed via modelfiles:

```bash
# List installed models
ollama list

# Remove specific models
ollama rm lfm2.5-thinking:1.2b-32k
ollama rm translategemma:12b-32k
ollama rm llama3.2:3b-32k
ollama rm glm-ocr:bf16
ollama rm mistral-small3.2:24b-32k
ollama rm gpt-oss:20b-64k
ollama rm qwen3-coder:30b-64k
ollama rm pepe:8b-64k

# Or remove all models at once
ollama rm $(ollama list | awk 'NR>1 {print $1}')
```

## Post-Installation

### Shell Completion (Optional)

Generate shell completions using the built-in `completion` subcommand:

```bash
# Bash
ask-ai completion bash > /etc/bash_completion.d/ask-ai

# Zsh
ask-ai completion zsh > /usr/local/share/zsh/site-functions/_ask-ai

# Fish
ask-ai completion fish > ~/.config/fish/completions/ask-ai.fish

# PowerShell
ask-ai completion powershell

# Elvish
ask-ai completion elvish
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`

### Environment Variables

You can configure Ask-AI with environment variables:

```bash
# Ollama server location (default: localhost:11434)
export OLLAMA_HOST="localhost:11434"

# Add to your shell config (.bashrc, .zshrc, etc.)
echo 'export OLLAMA_HOST="localhost:11434"' >> ~/.bashrc
```

## Troubleshooting Installation

### "command not found: ask-ai"

The binary is not in your PATH. Check:

```bash
# Find where it was installed
which ask-ai

# If not found, check installation prefix
ls /usr/local/bin/ask-ai

# Add to PATH if needed
export PATH="/usr/local/bin:$PATH"
```

### "cargo: command not found"

Rust is not installed. Install it first:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build Errors

Ensure you have the latest Rust:

```bash
rustup update
```

### Ollama Connection Failed

Make sure Ollama is running:

```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Start Ollama if not running
ollama serve
```

### "Model not found" Error

If you get "Model not found" errors:

```bash
# Check if Ollama has the model
ollama list | grep lfm2.5

# Install missing models
cd modelfiles
make models-essential
```

### Model Installation Fails

If model installation via modelfiles fails:

```bash
# Check Ollama is running
ollama serve

# Try installing the base model manually first
ollama pull lfm2.5-thinking:1.2b

# Then retry the modelfile installation
cd modelfiles
make lfm
```

## Next Steps

Now that Ask-AI is installed:

1. **[Quick Start](./quickstart.md)** - Learn the basics in 5 minutes
2. **[Commands](./commands/README.md)** - Explore all available commands
3. **[Models](./models.md)** - Understand available models

Happy querying! 🚀
