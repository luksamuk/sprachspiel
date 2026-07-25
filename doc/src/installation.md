# Installation Guide

This guide covers all the ways to install Sprachspiel on your system.

## Prerequisites

Before installing Sprachspiel, ensure you have:

1. **LLM Server** - Any OpenAI-compatible LLM server (llama-swap, Ollama, llama.cpp, LM Studio, vLLM, or a cloud provider)
2. **Rust toolchain** - Only needed if building from source
3. **Git** - For cloning the repository

### Installing an LLM Server

Sprachspiel requires an OpenAI-compatible LLM server to be running. You can use any of the following backends:

- **[llama-swap](https://github.com/mostlygeeksllc/llama-swap)** — model swap manager for llama.cpp (recommended for multi-model local setups)
- **[Ollama](https://ollama.com)** — simple local model runner
- **[llama.cpp](https://github.com/ggerganov/llama.cpp)** — direct llama.cpp server
- **[LM Studio](https://lmstudio.ai)** — GUI-based local model runner
- **[vLLM](https://github.com/vllm-project/vllm)** — high-throughput inference engine
- **Cloud providers** — any OpenAI-compatible API endpoint (OpenAI, Groq, Together, etc.)

All backends expose the same `/v1/chat/completions` endpoint that Sprachspiel uses. Configure your provider in `~/.config/sprachspiel/models.toml`.

#### Option A: llama-swap (recommended for local multi-model)

llama-swap loads GGUF model files and exposes them through a single OpenAI-compatible API at `http://localhost:12434/v1`. Install and start it following the [llama-swap documentation](https://github.com/mostlygeeksllc/llama-swap).

```bash
# Start llama-swap (default port 12434)
llama-swap serve
```

#### Option B: Ollama

Ollama is a popular choice for local model serving. Install it from [ollama.com](https://ollama.com):

```bash
# Linux
curl -fsSL https://ollama.com/install.sh | sh

# macOS
brew install ollama

# Windows
# Download from https://ollama.com/download
```

After installing, start the Ollama service:

```bash
ollama serve
```

Ollama exposes its OpenAI-compatible endpoint at `http://localhost:11434/v1`.

#### Option C: Other compatible backends

Other OpenAI-compatible backends also work:

- [llama.cpp](https://github.com/ggerganov/llama.cpp) — direct GGUF serving
- [LM Studio](https://lmstudio.ai/) — desktop GUI model server
- [vLLM](https://github.com/vllm-project/vllm) — high-throughput serving
- Cloud providers (OpenAI, Together, etc.)

Configure the `base_url` in `~/.config/sprachspiel/models.toml` to point to your server's `/v1` endpoint (see [Configuring Models](#configuring-models) below).

### Installing Rust (for building from source)

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Installation Methods

### Method 1: One-Liner (Recommended)

Install directly from GitHub releases:

```bash
# Install latest version
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash

# Install specific version
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash -s -- --version <version>

# Install with all tools enabled
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash -s -- --tools all

# Install system-wide (requires sudo)
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash -s -- --prefix /usr
```

**What it does:**
1. Detects your platform (Linux x86_64, Linux ARM64, or Termux)
2. Downloads the appropriate tarball from GitHub Releases
3. Extracts and runs the installation script
4. Installs to `~/.local/bin` by default (or your chosen prefix)
5. Installs manpage to `~/.local/share/man/man1`

**Post-installation:**
Add `~/.local/bin` to PATH if not already:
```bash
export PATH="$HOME/.local/bin:$PATH"
export MANPATH="$HOME/.local/share/man:$MANPATH"
```

### Method 2: Download Tarball

Download from [GitHub Releases](https://github.com/luksamuk/sprachspiel/releases):

```bash
# Download latest release
wget https://github.com/luksamuk/sprachspiel/releases/latest/download/sprachspiel-<version>-linux-x86_64.tar.gz

# Extract
tar -xzf sprachspiel-<version>-linux-x86_64.tar.gz
cd sprachspiel-<version>-linux-x86_64

# Install (interactive)
./install.sh

# Or install to custom location
./install.sh --prefix /usr
./install.sh --bin ~/bin --man ~/man

# Uninstall
./uninstall.sh
```

**Tarball contents:**
- `sprach` - Binary
- `sprach.1` - Manpage
- `install.sh` - Installation script
- `uninstall.sh` - Uninstallation script
- `README.md` - Documentation
- `LICENSE.txt` - License

### Method 3: Building from Source

The traditional way using Make:

```bash
# Clone the repository
git clone https://github.com/luksamuk/sprachspiel.git
cd sprachspiel

# Build and install (default: /usr/local)
make install

# Or install to ~/.local (recommended for development)
make install-local

# Or with custom prefix
make install PREFIX=/usr
```

This will:
1. Build the release binary
2. Install it to `/usr/local/bin/sprach` (or your chosen prefix)
3. Install the man page to `/usr/local/share/man/man1/`

### Method 4: Termux (Android)

Sprachspiel can run on Android via Termux. Since local LLM servers don't run on Android, you'll need a remote server.

#### Quick Install (One-Liner)

```bash
# In Termux
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash
```

The installer automatically detects Termux and configures the correct paths.

#### Manual Install on Termux

```bash
# In Termux
pkg install wget

# Download the tarball from GitHub releases
wget https://github.com/luksamuk/sprachspiel/releases/download/v<version>/sprachspiel-<version>-termux-aarch64.tar.gz

# Extract
tar -xzf sprachspiel-<version>-termux-aarch64.tar.gz
cd sprachspiel-<version>-termux-aarch64

# Install (creates ~/bin and adds to PATH)
./install.sh

# Or install to ~/.local/bin
./install.sh --bin ~/.local/bin --man ~/.local/share/man/man1

# Create config directory
mkdir -p ~/.config/sprachspiel

# Configure remote LLM server (e.g. llama-swap on your desktop at 192.168.1.100)
cat > ~/.config/sprachspiel/models.toml << 'EOF'
[provider."remote-llama-swap"]
kind = "openai"
base_url = "http://192.168.1.100:12434/v1"

[models."qwen3.5-4b"]
model_id = "qwen3.5-4b"
provider = "remote-llama-swap"
EOF
```

#### Termux-Specific Notes

- **Binary location**: `~/bin` (or `~/.local/bin`)
- **Manpage**: `~/.local/share/man/man1/sprach.1`
- **LLM server**: Must run on a separate machine (desktop/laptop/server)
- **Configuration**: `~/.config/sprachspiel/models.toml`
- **See**: `README-TERMUX.txt` included in the tarball

#### Building for Termux (Developers)

If you're building from source for Termux (requires cross-compilation):

**Prerequisites:**
1. Docker or Podman on your development machine
2. `cross` for Rust cross-compilation:
   ```bash
   cargo install cross --git https://github.com/cross-rs/cross
   ```

```bash
# On your development machine
git clone https://github.com/luksamuk/sprachspiel.git
cd sprachspiel

# Build for Termux (aarch64)
make termux

# Or build with all tools
make termux-all-tools

# Create distribution tarball
make tarball-termux
```

Available Make targets:

| Target | Description |
|--------|-------------|
| `make termux` | Build for Termux (default features) |
| `make termux-all-tools` | Build for Termux with all tools |
| `make tarball-termux` | Create tarball for distribution |
| `make tarball-termux-all-tools` | Create tarball with all tools |

### Method 5: Manual Installation

Build from source manually:

```bash
# Clone and enter repository
git clone https://github.com/luksamuk/sprachspiel.git
cd sprachspiel

# Build release binary
cargo build --release

# Copy to your PATH
sudo cp target/release/sprach /usr/local/bin/sprach

# Optional: Install man page
sudo cp man/sprach.1 /usr/local/share/man/man1/
```

### Method 6: Development Build

For development or testing:

```bash
# Run directly without installing
cargo run -- "Your query here"

# Or build debug version
cargo build

# Run debug binary
./target/debug/sprach "Your query"
```

## Configuring Models

Sprachspiel models are configured via `~/.config/sprachspiel/models.toml`. This file defines your LLM provider(s) and the models available to Sprachspiel. A default configuration file is created automatically on first run with recommended settings.

### Provider Configuration

Each provider is defined under a `[provider."<name>"]` section with a `kind` and `base_url`. The provider name is an arbitrary label you choose.

#### llama-swap (recommended local backend)

```toml
[provider."llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"
```

#### Ollama

```toml
[provider.ollama]
kind = "openai"
base_url = "http://localhost:11434/v1"
```

#### Remote server (e.g. for Termux)

```toml
[provider."remote-server"]
kind = "openai"
base_url = "http://192.168.1.100:12434/v1"
```

### Model Configuration

Each model is defined under a `[models."<name>"]` section, specifying the `model_id` that the provider recognises and which provider to use:

```toml
[provider."llama-swap"]
kind = "openai"
base_url = "http://localhost:12434/v1"

[models."qwen3.5-4b"]
model_id = "qwen3.5-4b"
provider = "llama-swap"

[models."ornith-1.0-35b"]
model_id = "ornith-1.0-35b"
provider = "llama-swap"

[models."translategemma-4b"]
model_id = "translategemma-4b"
provider = "llama-swap"
```

### Loading Model Files

Models are loaded as GGUF files by your backend server (llama-swap, llama.cpp, Ollama, etc.), not pulled by Sprachspiel itself. Refer to your backend's documentation for how to load or register GGUF model files:

- **llama-swap**: Place GGUF files in the configured model directory; llama-swap loads them on demand.
- **Ollama**: Use `ollama pull <model>` to download models, or import GGUF files with `ollama create`.
- **llama.cpp**: Pass GGUF file paths when starting the server.
- **LM Studio**: Load GGUF files through the LM Studio UI.

See [Custom Models](./configuration.md#custom-models) for details on configuring model parameters (context window, temperature, etc.) in `models.toml`.

## Post-Installation

### PATH Configuration (Essential)

After installation, ensure `~/.local/bin` is in your PATH:

```bash
# Check if installed binary is in PATH
which sprach
# Should show: /home/youruser/.local/bin/sprach

# If not found, add to your shell config
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Manpage Access (Optional)

To read the manpage, add the man directory to your MANPATH:

```bash
# Add to your shell config (~/.bashrc, ~/.zshrc, etc.)
echo 'export MANPATH="$HOME/.local/share/man:$MANPATH"' >> ~/.bashrc
source ~/.bashrc

# Then you can use
man sprach

# Or use man -M without setting MANPATH
man -M ~/.local/share/man sprach
```

### Shell Completion (Optional)

Generate shell completions using the built-in `completion` subcommand:

#### Quick Setup (User-local)

```bash
# Bash (current session only - add to ~/.bashrc for persistence)
eval "$(sprach completion bash)"

# Bash (permanent, user-local)
sprach completion bash >> ~/.bash_completion

# Zsh (user-local)
sprach completion zsh > ~/.zsh_completions/_sprach

# Fish (user-local)
sprach completion fish > ~/.config/fish/completions/sprach.fish
```

#### System-wide Setup (requires root)

```bash
# Bash - system-wide
sudo sprach completion bash > /etc/bash_completion.d/sprach

# Zsh - system-wide (verify your zsh site-functions location)
sudo sprach completion zsh > /usr/local/share/zsh/site-functions/_sprach
# OR
sudo sprach completion zsh > /usr/share/zsh/site-functions/_sprach

# Fish - system-wide
sudo sprach completion fish > /usr/share/fish/vendor_completions.d/sprach.fish
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
sprach <TAB>          # Should show subcommands
sprach --<TAB>        # Should show options
sprach translate <TAB> # Should show translate options
```

## Troubleshooting Installation

### "command not found: sprach"

The binary is not in your PATH. Check:

```bash
# Find where it was installed
which sprach

# If not found, check installation prefix
ls /usr/local/bin/sprach

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

### LLM Server Connection Failed

Make sure your LLM server is running and reachable. The endpoint depends on which backend you use:

```bash
# Check llama-swap (default port 12434)
curl http://localhost:12434/v1/models

# Check Ollama (default port 11434)
curl http://localhost:11434/v1/models

# Start your server if not running
llama-swap serve   # or: ollama serve, ./server -m model.gguf, etc.
```

Verify that the `base_url` in `~/.config/sprachspiel/models.toml` matches your server's address and port.

### "Model not found" Error

If you get "Model not found" errors, check that:

1. Your LLM server is running and the model is loaded/available
2. The `model_id` in `models.toml` matches the model name your server recognises
3. The `provider` field in `models.toml` matches a defined `[provider]` section

```bash
# List models available on your server (llama-swap)
curl http://localhost:12434/v1/models

# List models available on Ollama
curl http://localhost:11434/v1/models
```

### Model Installation Fails

Models are loaded by your backend server, not by Sprachspiel. If a model fails to load:

- **llama-swap**: Check that the GGUF file exists in the configured model directory and the path is correct.
- **Ollama**: Ensure the model was pulled or created successfully with `ollama list`.
- **llama.cpp / LM Studio**: Verify the GGUF file path is correct and the file is not corrupted.

Check your server's logs for detailed error messages.

## Next Steps

Now that Sprachspiel is installed:

1. **[Quick Start](./quickstart.md)** - Learn the basics in 5 minutes
2. **[Commands](./commands/README.md)** - Explore all available commands
3. **[Models](./models.md)** - Understand available models

Happy querying! 🚀