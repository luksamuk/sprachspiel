# Troubleshooting

Common issues and their solutions.

## Installation Issues

### "command not found: ask-ai"

**Problem:** Binary not in PATH

**Solution:**

```bash
# Check installation
which ask-ai

# If not found, check location
ls /usr/local/bin/ask-ai

# Add to PATH
export PATH="/usr/local/bin:$PATH"

# Or reinstall
make install PREFIX=$HOME/.local
export PATH="$HOME/.local/bin:$PATH"
```

### "cargo: command not found"

**Problem:** Rust not installed

**Solution:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Ollama Connection Issues

### "Failed to connect to Ollama"

**Problem:** Ollama not running

**Solution:**

```bash
# Check if running
curl http://localhost:11434/api/tags

# Start Ollama
ollama serve

# Or start as service
sudo systemctl start ollama
```

### "Model not found"

**Problem:** Model not pulled

**Solution:**

```bash
# Pull required models
ollama pull llama3.1:8b
ollama pull translategemma:4b
ollama pull glm-ocr:bf16
ollama pull moondream:1.8b
```

## Model Issues

### "Model doesn't support think mode"

**Problem:** Trying to use think mode on incompatible model

**Solution:**

```bash
# Use think-capable model
ask-ai -m lfm -t "Question"

# Check capabilities
ask-ai -d "Test query"
```

### "Tools not working"

**Problem:** Model doesn't support tools or tools not enabled

**Solution:**

```bash
# Use tool-capable model
ask-ai -m mistral-small "Tell me about Pikachu"

# Force enable tools
ask-ai --tools "Tell me about Pikachu"

# Check debug output
ask-ai -d "Query"
```

**Problem:** `invalid character '\u003c'` error

**Solution:**

```bash
# Use alternative model
ask-ai -m mistral-small "Query"
ask-ai -m pepe "Query"
```

## Web Search Issues

### "DuckDuckGo CAPTCHA Block"

**Problem:** Web search blocked

**Status:** Currently blocked

**Workaround:**

```bash
# Use query without web search
ask-ai "Question using model knowledge only"

# Try code_with_tools for web research
ask-ai -p code_with_tools "Latest Rust patterns"
```

## Translation Issues

### "Language not found"

**Problem:** Invalid language code

**Solution:**

```bash
# List available languages
ask-ai translate --list

# Check specific language
ask-ai translate --list pt
```

### "Translation failed"

**Problem:** Model issue or timeout

**Solution:**

```bash
# Try with specific model
ask-ai translate --model smollm3 en:pt "Text"

# Debug mode
ask-ai translate -d en:pt "Text"
```

## OCR Issues

### "Image not found"

**Problem:** File doesn't exist or wrong path

**Solution:**

```bash
# Verify file exists
ls -la image.png

# Check supported formats
ask-ai ocr --help
```

### "OCR failed"

**Problem:** Image quality or model issue

**Solution:**

```bash
# Pull required model
ollama pull glm-ocr:bf16

# Try with higher quality image
# Ensure text is clear and well-lit
```

## Performance Issues

### "Slow responses"

**Problem:** Large model or complex query

**Solution:**

```bash
# Use smaller model
ask-ai -m smollm3 "Question"

# Reduce context
ask-ai summarize --max-length 100 "Text"
```

### "Out of memory"

**Problem:** Model too large for available RAM

**Solution:**

```bash
# Use smaller model
ask-ai -m llama3.2 "Question"  # 3B model
ask-ai -m smollm3 "Question"   # 3B model
```

## Debug Mode

Use debug mode to diagnose issues:

```bash
# Enable debug
ask-ai -d "Query"
ask-ai translate -d en:pt "Text"
ask-ai ocr -d image.png
ask-ai summarize -d "Text"
```

## Getting Help

### GitHub Issues

Report bugs at: https://github.com/luksamuk/ask-ai-rs/issues

### Debug Information

When reporting issues, include:

```bash
# Version
ask-ai --version

# List command
ask-ai --list

# Debug output
ask-ai -d "Test query" 2> debug.log
cat debug.log
```

## Known Limitations

1. **Web search**: Currently blocked by DuckDuckGo CAPTCHA
## See Also

- [Installation](./installation.md) - Setup guide
- [Configuration](./configuration.md) - Configuration options
- [Models](./models.md) - Model reference
