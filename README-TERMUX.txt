# ask-ai for Termux (Android)

This package contains ask-ai compiled for Android/Termux (aarch64).

## Requirements

- **Termux** (latest version from F-Droid or Play Store)
- **Ollama** running on a separate machine (desktop/laptop/server)

## Quick Install

```bash
# Extract the tarball
tar -xzf ask-ai-VERSION-termux-aarch64.tar.gz
cd ask-ai-VERSION-termux-aarch64

# Run the installer
./install.sh
```

## Manual Install

If you prefer to install manually:

```bash
# Extract
tar -xzf ask-ai-VERSION-termux-aarch64.tar.gz

# Create directories
mkdir -p ~/bin
mkdir -p ~/.local/share/man/man1

# Install binary
cp ask-ai ~/bin/
chmod +x ~/bin/ask-ai

# Install manpage (optional)
cp ask-ai.1 ~/.local/share/man/man1/

# Add to PATH (add to ~/.bashrc or ~/.termux/boot/autoload.sh)
export PATH="$HOME/bin:$PATH"
```

## Configuring Ollama

ask-ai requires Ollama to be running. Since Termux cannot run Ollama directly, you need to connect to a remote Ollama instance.

### Step 1: Set up Ollama on your desktop/server

On your desktop or server machine:

```bash
# Start Ollama with network binding
OLLAMA_HOST=0.0.0.0:11434 ollama serve

# Or set in your shell config
export OLLAMA_HOST=0.0.0.0:11434
```

### Step 2: Configure ask-ai on Termux

Create or edit `~/.config/ask-ai/config.toml`:

```bash
mkdir -p ~/.config/ask-ai
cat > ~/.config/ask-ai/config.toml << 'EOF'
# Ask AI configuration for Termux

[ollama]
host = "192.168.1.100:11434"  # Replace with your desktop/server IP
EOF
```

### Step 3: Test the connection

```bash
ask-ai "What is the capital of France?"
```

## Configuration File Location

```
~/.config/ask-ai/config.toml     # Main configuration
~/.local/share/ask-ai/           # Data directory (conversations, embeddings)
```

## Available Features

This build includes:

- Weather tools (`get_current_weather`, `get_weather_forecast`, `get_air_quality`)
- File tools (`read_file`, `list_directory`, `search_files`, `count_lines`, `read_file_segment`)
- Calculator (`calculate`)
- System information (`get_system_info`, `get_current_directory`)

**Note:** Web search tools (Serper, DuckDuckGo) are excluded from the default Termux build due to size constraints. Use the `-all-tools` build if you need them.

## All-Tools Build

If you need web search and Pokémon tools:

```bash
# Download the all-tools variant
tar -xzf ask-ai-VERSION-termux-aarch64-all-tools.tar.gz
cd ask-ai-VERSION-termux-aarch64-all-tools
./install.sh
```

## Troubleshooting

### "Connection refused" or timeout

1. **Check if Ollama is running** on your desktop/server:
   ```bash
   curl http://YOUR_DESKTOP_IP:11434/api/tags
   ```

2. **Check firewall** - Allow port 11434:
   ```bash
   # On the desktop/server (ufw)
   sudo ufw allow 11434
   
   # Or (firewalld)
   sudo firewall-cmd --add-port=11434/tcp --permanent
   sudo firewall-cmd --reload
   ```

3. **Check IP address** - Make sure you're using the correct IP:
   ```bash
   # On desktop/server
   ip addr show | grep inet
   ```

### "command not found"

Make sure `~/bin` is in your PATH:

```bash
echo $PATH
# Should show /data/data/com.termux/files/home/bin

# If not, add it:
echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Performance tips

1. **Use WiFi** - Avoid mobile data for better latency
2. **Keep Ollama warm** - Models that haven't been loaded recently take longer to start
3. **Use smaller models** - `llama3.2:3b` works well with Termux latency

## Uninstalling

```bash
cd ask-ai-VERSION-termux-aarch64
./uninstall.sh
```

Or manually:

```bash
rm ~/bin/ask-ai
rm ~/.local/share/man/man1/ask-ai.1
# Optionally remove config if not needed
rm -rf ~/.config/ask-ai
rm -rf ~/.local/share/ask-ai
```

## Getting Help

```bash
ask-ai --help
man ask-ai  # If manpage is installed
```

## Links

- GitHub: https://github.com/luksamuk/ask-ai-rs
- Documentation: https://anomalyco.github.io/ask-ai/
- Issues: https://github.com/luksamuk/ask-ai-rs/issues