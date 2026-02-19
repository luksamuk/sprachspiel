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
git clone https://github.com/luksamuk/ask-ai-rs.git
cd ask-ai

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

### Method 2: Termux (Android)

Ask-AI can run on Android via Termux. Since Ollama doesn't run on Android, you'll need a remote Ollama server.

**Prerequisites:**
1. Install Termux from F-Droid (not Play Store - Play Store version is outdated)
2. Install Docker or Podman on your development machine
3. Install `cross` for cross-compilation:
   ```bash
   cargo install cross --git https://github.com/cross-rs/cross
   ```

**Build for Android:**

```bash
# On your development machine
git clone https://github.com/luksamuk/ask-ai-rs.git
cd ask-ai-rs

# Build for Termux (aarch64)
make termux

# Or build with all tools
make termux-all-tools

# Create distribution tarball
make tarball-termux
```

**Install on Termux:**

```bash
# In Termux (on your Android device)
pkg install wget

# Download the tarball from GitHub releases or transfer via scp
wget https://github.com/luksamuk/ask-ai-rs/releases/download/vX.Y.Z/ask-ai-X.Y.Z-termux-aarch64-linux-android.tar.gz

# Extract and install
tar -xzf ask-ai-*.tar.gz
chmod +x ask-ai
mv ask-ai $PREFIX/bin/

# Verify
ask-ai --version
```

**Configure Remote Ollama:**

Create the config file in Termux:

```bash
mkdir -p ~/.config/ask-ai
cat > ~/.config/ask-ai/config.toml << EOF
[model]
# IP address of your Ollama server (desktop/laptop)
ollama_host = "192.168.1.100"
ollama_port = 11434
EOF
```

Replace `192.168.1.100` with your Ollama server's IP address. The host can be specified as:
- IP address: `192.168.1.100`
- With scheme: `http://192.168.1.100`
- Hostname: `myserver.local`

**Available Make Targets:**

| Target | Description |
|--------|-------------|
| `make termux` | Build for Termux (default features) |
| `make termux-all-tools` | Build for Termux with all tools |
| `make tarball-termux` | Create tarball for distribution |
| `make tarball-termux-all-tools` | Create tarball with all tools |

### Method 3: Manual Installation

Build from source manually:

```bash
# Clone and enter repository
git clone https://github.com/luksamuk/ask-ai-rs.git
cd ask-ai

# Build release binary
cargo build --release

# Copy to your PATH
sudo cp target/release/ask-ai /usr/local/bin/ask-ai

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
./target/debug/ask-ai "Your query"
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

Install the three models required for basic functionality:

```bash
# Navigate to modelfiles directory
cd modelfiles

# Build and install all essential models
make models-essential
```

This builds and installs:
- **llama3.1:8b** - Default for general queries
- **translategemma:12b** - For translation command
- **glm-ocr:bf16** - For OCR/text extraction

Note: Context window sizes are configured in `~/.config/ask-ai/models.toml`, not in model tags.

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
make llama3.1            # Build Llama 3.1 8B (default)
make translategemma      # Build Translation model
make glm-ocr             # Pull OCR model

# Optional models for specialized tasks
make lfm                 # Build LFM 2.5 Thinking (reasoning)
make llama3.2            # Build Llama 3.2 3B (summarization, tools)
make mistral-small       # Build Mistral Small (tools)
make qwen3-coder         # Build Qwen3 Coder (code)

# See all available targets
make help
```

### Additional Models

Additional models (mistral-small, qwen3-coder, deepseek-coder-v2, etc.) are configured via `~/.config/ask-ai/models.toml`. A default configuration file is created automatically with recommended settings.

See [Custom Models](./configuration.md#custom-models) for details.

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
ollama rm llama3.1:8b
ollama rm translategemma:12b
ollama rm glm-ocr:bf16

# Remove optional models
ollama rm lfm2.5-thinking:1.2b
ollama rm llama3.2:3b

# Or remove all models at once
ollama rm $(ollama list | awk 'NR>1 {print $1}')
```

## Post-Installation

### Shell Completion (Optional)

Generate shell completions using the built-in `completion` subcommand:

#### Quick Setup (User-local)

```bash
# Bash (current session only - add to ~/.bashrc for persistence)
eval "$(ask-ai completion bash)"

# Bash (permanent, user-local)
ask-ai completion bash >> ~/.bash_completion

# Zsh (user-local)
ask-ai completion zsh > ~/.zsh_completions/_ask-ai

# Fish (user-local)
ask-ai completion fish > ~/.config/fish/completions/ask-ai.fish
```

#### System-wide Setup (requires root)

```bash
# Bash - system-wide
sudo ask-ai completion bash > /etc/bash_completion.d/ask-ai

# Zsh - system-wide (verify your zsh site-functions location)
sudo ask-ai completion zsh > /usr/local/share/zsh/site-functions/_ask-ai
# OR
sudo ask-ai completion zsh > /usr/share/zsh/site-functions/_ask-ai

# Fish - system-wide
sudo ask-ai completion fish > /usr/share/fish/vendor_completions.d/ask-ai.fish
```

#### Supported Shells

- `bash` - Bourne Again Shell
- `zsh` - Z Shell
- `fish` - Friendly Interactive Shell
- `powershell` - PowerShell
- `elvish` - Elvish Shell

#### Troubleshooting Completions

**Bash**: If completions don't work, ensure bash-completion is installed:
```bash
# Arch Linux
sudo pacman -S bash-completion

# Debian/Ubuntu
sudo apt-get install bash-completion

# Fedora
sudo dnf install bash-completion
```

Then reload your shell:
```bash
source ~/.bashrc
```

**Zsh**: Ensure the completions directory is in your `$fpath`. Add to `~/.zshrc`:
```bash
# Create directory if needed
mkdir -p ~/.zsh_completions

# Add to fpath (before compinit)
fpath+=(~/.zsh_completions)
autoload -Uz compinit && compinit
```

**Fish**: Completions should work immediately after placing in `~/.config/fish/completions/`. If not:
```bash
exec fish
```

**Test completions are working**:
```bash
ask-ai <TAB>          # Should show subcommands
ask-ai --<TAB>        # Should show options
ask-ai translate <TAB> # Should show translate options
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
