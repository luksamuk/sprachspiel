# Ask-AI Documentation

Welcome to the Ask-AI documentation! This is a comprehensive guide for the Ask-AI command-line tool - a powerful Rust CLI for interacting with Ollama LLM models.

## What is Ask-AI?

Ask-AI is a feature-rich command-line interface that brings the power of Large Language Models (LLMs) to your terminal. Whether you need to:

- **Get quick answers** to questions
- **Translate text** between 50+ languages
- **Extract text from images** (OCR)
- **Summarize long documents**
- **Chain commands** for complex workflows

Ask-AI provides an elegant, markdown-rendered interface that makes working with AI models feel natural and efficient.

## Key Features

- **🚀 Multiple Models**: Support for 14+ model presets including LFM, Mistral, GPT-OSS, Llama, and cloud models
- **🛠️ Tool Integration**: Automatic capability detection with Pokémon, Weather, and Web Search tools
- **📝 Markdown Rendering**: Beautiful terminal output via termimad
- **🌐 Translation**: Translate between 50+ languages using TranslateGemma
- **🖼️ OCR**: Extract text, tables, formulas, and figures from images
- **📄 Summarization**: Create concise summaries with customizable styles
- **🔗 Pipelines**: Chain commands with stdin/pipe support
- **🧠 Think Mode**: Support for reasoning models
- **🐛 Debug Mode**: Detailed logging for troubleshooting

## Quick Example

```bash
# Ask a question
ask-ai "What is the capital of France?"

# Translate text
ask-ai translate en:pt "Hello, how are you?"

# Extract text from an image
ask-ai ocr document.png

# Summarize a document
cat long-article.txt | ask-ai summarize --style academic

# Chain commands for powerful workflows
ask-ai ocr japanese-document.png | ask-ai translate ja:pt | ask-ai summarize
```

## Getting Started

New to Ask-AI? Start here:

1. **[Installation](./installation.md)** - Install Ask-AI on your system
2. **[Quick Start](./quickstart.md)** - Get up and running in 5 minutes
3. **[Commands](./commands/README.md)** - Learn about all available commands

## Documentation Structure

This documentation is organized into sections:

- **User Guide**: Complete reference for using Ask-AI
  - Commands and subcommands
  - Available models and their capabilities
  - Tools and integrations
  - Pipelines and workflows
  
- **Development**: Information for contributors
  - Architecture and design decisions
  - Roadmap and planned features
  - Contributing guidelines

## Terminal Man Page

For quick reference while working in the terminal, consult the man page:

```bash
man ask-ai
```

Or use the built-in help:

```bash
ask-ai --help
ask-ai query --help
ask-ai translate --help
```

## Requirements

- **Ollama**: Must be running locally or accessible remotely
- **Rust**: Built with Rust for performance and reliability
- **Terminal**: Any modern terminal with UTF-8 support

## Support

- **Issues**: [GitHub Issues](https://github.com/luksamuk/ask-ai-rs/issues)
- **Documentation**: You're reading it! 📖

## License

Ask-AI is licensed under the MIT License. See the [LICENSE](../LICENSE.txt) file for details.

Copyright (c) 2026 Lucas S. Vieira

---

Ready to dive in? Start with the [Installation Guide](./installation.md) or jump to the [Quick Start](./quickstart.md)!
