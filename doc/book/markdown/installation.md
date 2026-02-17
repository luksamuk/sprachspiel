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
git clone <your-repo-url>
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
git clone <your-repo-url>
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

## Verifying Installation

After installation, verify Ask-AI is working:

```bash
# Check version
ask-ai --version

# Show help
ask-ai --help

# List available models
ask-ai --list

# Test with a simple query
ask-ai "Hello, are you working?"
```

## Installing Required Models

Ask-AI uses several Ollama models for different tasks. Install the ones you need:

### Default Query Model (LFM)

```bash
ollama pull lfm2.5-thinking:1.2b-32k
```

### Translation Model

```bash
ollama pull translategemma:12b-32k
```

### OCR Model

```bash
ollama pull glm-ocr:bf16
```

### Summarization Model (optional)

```bash
ollama pull llama3.2:3b-32k
```

### Other Useful Models

```bash
# For tool calling
ollama pull mistral-small3.2:24b-32k

# For coding
ollama pull qwen3-coder:30b-64k

# For general use
ollama pull pepe:8b-64k
```

## Uninstallation

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

## Post-Installation

### Shell Completion (Optional)

Add shell completions for your shell:

```bash
# Bash
ask-ai --generate-completion bash > /etc/bash_completion.d/ask-ai

# Zsh
ask-ai --generate-completion zsh > /usr/local/share/zsh/site-functions/_ask-ai

# Fish
ask-ai --generate-completion fish > ~/.config/fish/completions/ask-ai.fish
```

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

## Next Steps

Now that Ask-AI is installed:

1. **[Quick Start](./quickstart.md)** - Learn the basics in 5 minutes
2. **[Commands](./commands/README.md)** - Explore all available commands
3. **[Models](./models.md)** - Understand available models

Happy querying! 🚀
