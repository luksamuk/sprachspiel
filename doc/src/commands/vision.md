# vision Command

The `vision` command analyzes and describes images using vision models. Unlike OCR which extracts text, vision provides general image understanding and description.

## Synopsis

```bash
sprach [GLOBAL OPTIONS] vision <FILE>... [-- <PROMPT>]
```

## Description

Vision provides general image understanding capabilities. It can describe images, answer questions about visual content, and compare multiple images. This is distinct from OCR which focuses specifically on text extraction.

## Arguments

| Argument | Description |
|----------|-------------|
| `FILE` | One or more image files to analyze |
| `PROMPT` | Optional custom prompt (overrides modes) |

| Option | Description |
|--------|-------------|
| `-v` | Verbose logging |
| `-vv` | Trace logging |
| `--help` | Show help |

## Subcommand Options

These options are specific to the vision subcommand:

| Option | Description | Default |
|--------|-------------|---------|
| `--detailed` | Detailed analysis mode | disabled |
| `--json` | Output as JSON | disabled |
| `--max-tokens` | Maximum tokens per image | `2048` |

## Analysis Modes

| Mode | Flag | Description |
|------|------|-------------|
| default | (none) | Brief image description |
| detailed | `--detailed` | Comprehensive analysis with composition, colors, subjects |
| custom | (prompt arg) | User-defined question or task |

## Supported Image Formats

- **PNG** (`.png`)
- **JPEG/JPG** (`.jpg`, `.jpeg`)
- **WebP** (`.webp`)
- **GIF** (`.gif`) - First frame only

## Examples

### Basic Description

```bash
# Describe an image
sprach vision photo.png

```

### Detailed Analysis

```bash
# Get comprehensive image analysis
sprach vision --detailed artwork.png

# Output includes:
# - Composition and layout
# - Color palette
# - Main subjects
# - Notable elements
# - Style and mood
```

### Plain Text Output

```bash
# Plain text without markdown rendering
sprach --plain vision photo.png

# Useful for piping to other commands
sprach --plain vision screenshot.png | grep "button"
```

### Custom Prompts

```bash
# Ask specific questions (use -- before prompt)
sprach vision photo.png -- "What objects are visible in this image?"
sprach vision screenshot.png -- "What UI components are used?"
sprach vision chart.png -- "Describe the data visualization"
sprach vision diagram.png -- "Explain the workflow shown"
```

### Multi-Image Analysis

```bash
# Analyze multiple images together
sprach vision img1.png img2.png

# Compare images with custom prompt (use -- before prompt)
sprach vision before.png after.png -- "What changed between these images?"

# For best multi-image results, use minicpm-v
sprach vision -m minicpm-v:8b img1.png img2.png -- "Compare these"
```

### JSON Output

```bash
# Output as JSON for programmatic use
sprach vision --json photo.png

# Example output:
# {"files": ["photo.png"], "prompt": "Describe this image.", "content": "A cat sitting on..."}

# Batch processing
for img in *.png; do
    sprach vision --json "$img" >> results.jsonl
done
```

### Model Selection

```bash
# Use specific model
sprach vision -m llava:13b photo.png
sprach vision -m qwen3.5:4b screenshot.png
sprach vision -m ministral-3:14b img1.png img2.png -- "Compare these"

# Via config file (~/.config/sprachspiel/config.toml):
# [model.vision]
# model = "qwen3.5:4b"
```

## Vision Models

| Model | Size | Context | Multi-Image | Best For |
|-------|------|---------|-------------|----------|
| `qwen3.5:4b` | 3.4 GB | 131K | Yes | Default, multimodal, good quality |
| `moondream:1.8b` | 1.7 GB | 2K | No | Lightweight alternative |
| `llava:13b` | 8.0 GB | 4K | No | Better quality |
| `ministral-3:14b` | 7.5 GB | 32K | Yes | Multi-image, general purpose |

**Note:** 8K context is sufficient for most vision tasks.

## Use Cases

### 1. Image Description

```bash
sprach vision photo.png
```

### 2. UI Analysis

```bash
sprach vision screenshot.png -- "What UI framework might this be using?"
sprach vision mockup.png -- "Describe the user interface"
```

### 3. Content Moderation

```bash
sprach vision image.png -- "Is this image appropriate for a general audience?"
```

### 4. Accessibility

```bash
sprach vision --detailed photo.png
# Generate alt text for web images
```

### 5. Visual Q&A

```bash
sprach vision diagram.png -- "Explain what this diagram shows"
sprach vision chart.png -- "What trends does this chart show?"
```

### 6. Comparison Tasks

```bash
# Multi-image comparison (requires model with multi-image support)
sprach vision -m ministral-3:14b v1.png v2.png -- "What are the differences?"
```

## Configuration

Default model can be set in `~/.config/sprachspiel/config.toml`:

```toml
[model.vision]
model = "moondream"
thinking = false
tools = false
```

## Model Resolution Order

1. CLI flag `-m` → use specified model
2. Config file `[model.vision].model` → use configured model
3. Default → `qwen3.5:4b`

## Pipelines

```bash
# Vision → Summarize
sprach vision --detailed photo.png | sprach summarize

# Vision → Translate
sprach vision photo.png "Describe in Portuguese"

# Multiple images with JSON for processing
sprach vision --json *.png | jq '.content' > descriptions.txt
```

## Tips for Better Results

### Image Quality

- Use clear, well-lit images
- Higher resolution works better
- PNG preferred for screenshots
- Crop unnecessary borders

### Prompt Engineering

```bash
# Be specific
sprach vision photo.png "List all visible objects"

# Ask for structure
sprach vision diagram.png "Describe this as a numbered list"

# Request format
sprach vision chart.png "Extract the data as a markdown table"
```

### Multi-Image Tasks

- Use `minicpm-v:8b` for comparing images
- Be explicit about comparison type
- Limit to 2-4 images for best results

## Limitations

- Requires vision model (run `ollama pull qwen3.5:4b`)
- Context varies by model (moondream: 2K, llava: 32K)
- Multi-image support varies by model
- Complex images may need `--detailed` or custom prompts

## Error Handling

Common errors and solutions:

```bash
# Model not found
ollama pull qwen3.5:4b

# File not found
sprach vision /path/to/existing/file.png

# Unsupported format
# Convert to PNG: convert image.bmp image.png
```

## See Also

- [ocr](./ocr.md) - Text extraction from images
- [query](./query.md) - General LLM queries
- [translate](./translate.md) - Language translation
- [Models](../models.md) - Available models