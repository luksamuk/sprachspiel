# Tool-Calling Parameter Research & Recommendations

## Overview

This document contains optimized parameters for tool-calling (function calling) based on research from Unsloth, HuggingFace, Ollama, and academic sources.

## General Tool-Calling Best Practices

### Core Parameters for Tool Use

| Parameter | Default Range | Tool-Optimized Range | Notes |
|-----------|---------------|---------------------|-------|
| **temperature** | 0.7-1.0 | **0.1-0.3** | Lower = more deterministic JSON/tool output |
| **top_p** | 0.9-1.0 | **0.80-0.95** | Narrow sampling for structured outputs |
| **top_k** | 40-100 | **20-50** | Limit token choices for precision |
| **repeat_penalty** | 1.0-1.2 | **1.0-1.05** | Prevent loops without hurting tool names |
| **min_p** | 0.0 | **0.0-0.01** | Filter out unlikely tokens (experimental) |

### Key Insights from Research

1. **Temperature is Critical**: 
   - Tools/JSON: 0.1-0.3 (deterministic)
   - Agentic reasoning: 0.4-0.6 (balance)
   - Creative tasks: 0.7-1.0

2. **Top_p vs Top_k**:
   - Use top_p 0.80-0.95 for structured outputs
   - Lower top_k (20-40) for code/tool precision
   - Don't disable both (unpredictable results)

3. **Context Windows**:
   - Tool schemas take tokens
   - 64K recommended minimum for complex tools
   - 128K+ for multi-tool workflows

## Per-Model Optimized Parameters

### 1. **LFM 2.5 Thinking** (Default Model)
**Status**: ✅ Native tool support | **Best for**: Fast tool reasoning

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.1 | 0.1 | ✅ Already optimal |
| top_k | 50 | 20 | Narrow for tool precision |
| top_p | 0.1 | 0.8 | Balance determinism & flexibility |
| repeat_penalty | 1.05 | 1.02 | Lower for tool name variety |

**Recommendation**: Keep current parameters - they're already well-tuned.

---

### 2. **~~GPT-OSS 20B~~** (REMOVED)
**Status**: ❌ Removed in v0.14.0 due to tool calling issues

GPT-OSS was removed from ask-ai because it outputs special tokens (`<|call|>`, `<|channel|>`, `<|message|>`) after JSON tool calls, breaking the parser. This is a model-level issue that cannot be fixed at the application level.

**Alternatives**:
- `qwen3-coder` - Excellent tool support
- `qwen2.5-coder:7b` - Excellent tool support
- `qwen3.5:4b` - Reliable tool calling (multimodal)

---

### 3. **Mistral Small 3.2 24B**
**Status**: ✅ Native tool support | **Best for**: Production tool workflows

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.2 | 0.15 | Lower for reliable JSON |
| top_k | 40 | 40 | ✅ Optimal |
| top_p | 0.9 | 0.85 | Slightly narrow |
| repeat_penalty | 1.1 | 1.05 | Prevent loops |

**Research Notes**:
- Mistral advertises "best-in-class agentic capabilities"
- Native JSON mode available
- Low latency function calling optimized

**Recommendation**: Slightly lower temperature, reduce repeat_penalty

---

### 4. **Qwen3 Coder 30B**
**Status**: ✅ Excellent tool use | **Best for**: Code generation + tools

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.7 | 0.15-0.3 | 🔴 Too high for tools |
| top_k | 20 | 20 | ✅ Already good |
| top_p | 0.80 | 0.80 | ✅ Optimal |
| repeat_penalty | 1.05 | 1.05 | ✅ OK |

**Research Notes**:
- MoE architecture (30B total / 3.3B active)
- "State-of-the-art agentic tool-use capabilities"
- Execution-driven RL training
- Qwen team recommends 0.7 for coding, 0.3 for tools

**Recommendation**: Use 0.3 for tool workflows, keep 0.7 for pure coding

---

### 5. **SmolLM3 3B Q8_0**
**Status**: ⚠️ Small model | **Best for**: Edge deployment

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.2 | 0.1 | Lower for 3B model |
| top_k | 40 | 30 | Moderate limit |
| top_p | 0.9 | 0.85 | Slightly narrow |
| repeat_penalty | 1.1 | 1.0 | Small model needs less penalty |

**Research Notes**:
- 3B parameters = less capacity for complex tools
- Use simpler tool schemas
- Consider tool examples in system prompt

**Recommendation**: Lower temperature significantly for reliability

---

### 6. **Devstral Small 2 24B**
**Status**: ✅ Tool capable | **Best for**: Coding agents

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.15 | 0.15 | ✅ Already low |
| top_k | 40 | 40 | ✅ OK |
| top_p | 0.9 | 0.85 | Slightly narrow |
| repeat_penalty | 1.1 | 1.05 | Prevent loops |

**Research Notes**:
- Based on Mistral, inherits tool capabilities
- min_p=0.01 in modelfile (not available in ollama-rs yet)
- Excellent for coding + tool workflows

**Recommendation**: Keep current, very well tuned already

---

### 7. **GLM 4.7 Flash**
**Status**: ⚠️ Conversational | **Best for**: Fast inference

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.7 | 0.3 | 🔴 Too high for tools |
| top_k | 40 | 40 | ✅ OK |
| top_p | 1.0 | 0.9 | 🔴 Full sampling |
| repeat_penalty | 1.0 | 1.0 | ✅ OK |

**Research Notes**:
- Flash = optimized for speed
- Not specifically trained for tools
- Works but requires lower temperature

**Recommendation**: Lower temperature to 0.3 for tool reliability

---

### 8. **SEAD 14B**
**Status**: ⚠️ Unknown tool support | **Best for**: General tasks

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.2 | 0.15 | Lower slightly |
| top_k | 40 | 30 | Moderate limit |
| top_p | 0.9 | 0.85 | Narrow for structure |
| repeat_penalty | 1.1 | 1.05 | Standard adjustment |

**Research Notes**:
- Limited documentation on tool capabilities
- Research suggests BERT-like architecture = less tool-native
- Use conservative parameters

**Recommendation**: Conservative tool parameters

---

### 9. **TranslateGemma 12B**
**Status**: ⚠️ Translation focused | **Best for**: Translation tasks

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.2 | 0.15 | Lower for precision |
| top_k | 40 | 30 | Moderate limit |
| top_p | 0.9 | 0.85 | Narrow |
| repeat_penalty | 1.1 | 1.05 | Standard |

**Research Notes**:
- Gemma 3 based, multimodal capabilities
- Not optimized for tools but capable
- Use translation-specific tools only

**Recommendation**: Moderate tool parameters

---

### 10. **Assistant Pepe 8B**
**Status**: ⚠️ Character model | **Best for**: Fun interactions

| Parameter | Current | Tool-Optimized | Rationale |
|-----------|---------|-----------------|-----------|
| temperature | 0.7 | 0.4 | Balance personality & tools |
| top_k | 40 | 40 | ✅ OK |
| top_p | 0.9 | 0.9 | ✅ OK |
| repeat_penalty | 1.1 | 1.1 | ✅ OK |

**Research Notes**:
- Character model with sarcastic personality
- Based on Llama 3.1 Nemotron 8B
- High temperature = personality
- Lower for tool reliability while keeping character

**Recommendation**: Compromise at 0.4 to keep personality

---

## Tool-Calling Specific Recommendations

### When to Enable Tools

```rust
// Auto-enable based on capability detection
let use_tools = capabilities.tools || cli.tools;

// Override with conservative parameters for small models
if model_id.contains("3b") || model_id.contains("1.2b") {
    // Lower temperature for small models
    temperature = 0.1;
}
```

### Think Mode Parameters

Models with thinking capability:
- **LFM 2.5**: Always thinks (inherent), use temp=0.1
- **Qwen3**: Has thinking variants, use lower temp with tools

### JSON Output Optimization

For JSON/tool schemas:
1. Temperature: 0.1-0.2 (critical)
2. Top_p: 0.80-0.90
3. Repeat penalty: 1.0-1.05
4. System prompt: Include JSON example

Example system prompt addition:
```
When calling tools, output valid JSON only. Example:
{"tool": "fetch_pokemon", "parameters": {"pokemon_name": "pikachu"}}
```

## Implementation Plan

### Phase 1: Completed (v0.14.0)
1. ~~Lower GPT-OSS temperature~~ - Model removed due to tool calling issues
2. Lower Qwen3 Coder temp from 0.7 → 0.3 ✅
3. Custom models support via `~/.config/ask-ai/models.toml` ✅

### Phase 2: Fine-Tuning (Medium Priority)
1. Add per-model `tool_optimized` boolean flag
2. Implement automatic parameter adjustment when `--tools` flag used
3. Add `min_p` support when ollama-rs supports it

### Phase 3: Advanced Features (Low Priority)
1. Research `typical_p` and other advanced params
2. Add tool-specific system prompt fragments
3. Implement dynamic parameter adjustment based on query type

## References

- Unsloth documentation on tool use
- HuggingFace chat templating guide
- Ollama modelfile parameter documentation
- Mistral AI agentic capabilities documentation
- Qwen3 technical report (agentic tool use section)
- Various GGUF quantization best practices

---

Last Updated: 2026-02-19 - v0.14.0: GPT-OSS removed, custom models added
Status: Research Complete - Ready for Implementation
