# Sprachspiel for Termux (Android)

This package contains sprach compiled for Android/Termux (aarch64).

## Requirements

- **Termux** (latest version from F-Droid or Play Store)
- **Ollama** running on a separate machine (desktop/laptop/server)

## Quick Install

```bash
# Extract the tarball
tar -xzf sprach-VERSION-termux-aarch64.tar.gz
cd sprach-VERSION-termux-aarch64

# Run the installer
./install.sh
```

## Manual Install

If you prefer to install manually:

```bash
# Extract
tar -xzf sprach-VERSION-termux-aarch64.tar.gz

# Create directories
mkdir -p ~/bin
mkdir -p ~/.local/share/man/man1

# Install binary
cp sprach ~/bin/
chmod +x ~/bin/sprach

# Install manpage (optional)
cp sprach.1 ~/.local/share/man/man1/

# Add to PATH (add to ~/.bashrc or ~/.termux/boot/autoload.sh)
export PATH="$HOME/bin:$PATH"
```

## Configuring Ollama

sprach requires Ollama to be running. Since Termux cannot run Ollama directly, you need to connect to a remote Ollama instance.

### Step 1: Set up Ollama on your desktop/server

On your desktop or server machine:

```bash
# Start Ollama with network binding
OLLAMA_HOST=0.0.0.0:11434 ollama serve

# Or set in your shell config
export OLLAMA_HOST=0.0.0.0:11434
```

### Step 2: Configure sprach on Termux

Create or edit `~/.config/sprachspiel/config.toml`:

```bash
mkdir -p ~/.config/sprachspiel
cat > ~/.config/sprachspiel/config.toml << 'EOF'
# Sprachspiel configuration for Termux

[ollama]
host = "192.168.1.100:11434"  # Replace with your desktop/server IP
EOF
```

### Step 3: Test the connection

```bash
sprach "What is the capital of France?"
```

## Configuration File Location

```
~/.config/sprachspiel/config.toml     # Main configuration
~/.local/share/sprachspiel/           # Data directory (conversations, embeddings)
```

## Available Features

This build includes:

- Weather tools (`get_current_weather`, `get_weather_forecast`, `get_air_quality`)
- File tools (`read_file`, `list_directory`, `count_lines`, `read_file_segment`)
- Calculator (`calculate`)
- System information (`get_system_info`, `get_current_directory`)

**Note:** Web search tools (Serper, DuckDuckGo) are excluded from the default Termux build due to size constraints. Use the `-all-tools` build if you need them.

## All-Tools Build

If you need web search and Pokémon tools:

```bash
# Download the all-tools variant
tar -xzf sprach-VERSION-termux-aarch64-all-tools.tar.gz
cd sprach-VERSION-termux-aarch64-all-tools
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
3. **Use smaller models** - `qwen3.5:4b` works well with Termux latency

## Uninstalling

```bash
cd sprach-VERSION-termux-aarch64
./uninstall.sh
```

Or manually:

```bash
rm ~/bin/sprach
rm ~/.local/share/man/man1/sprach.1
# Optionally remove config if not needed
rm -rf ~/.config/sprachspiel
rm -rf ~/.local/share/sprachspiel
```

## Getting Help

```bash
sprach --help
man sprach  # If manpage is installed
```

## Links

- GitHub: https://github.com/luksamuk/sprachspiel
- Documentation: https://luksamuk.github.io/sprachspiel/
- Issues: https://github.com/luksamuk/sprachspiel/issues