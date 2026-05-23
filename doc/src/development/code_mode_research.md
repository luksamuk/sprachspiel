# Code Mode Enhancement - Research Summary

> **HISTORICAL NOTE (April 2026):** This research evaluated deepseek-coder-v2.
> Since then, **qwen2.5-coder:7b** has become the recommended model for code mode due to:
> - Better tool calling support (BFCL ~75%)
> - Smaller size (4.7 GB vs 16 GB)
> - Fits in GPU memory (6GB VRAM)
> - Faster response times
>
> **Note on devstral-small-2:** This model (24B) was evaluated and showed good performance,
> but is not recommended for typical use due to its large size. It remains a valid option
> for users with sufficient hardware resources.
>
> This document is kept for historical reference.

## Executive Summary

Research completed on code mode improvements, including model evaluation, system prompt optimization, and tool integration strategy. DeepSeek-Coder-V2:16b-32k showed superior performance compared to devstral-small-2:24b-64k for code generation tasks.

**Status:** ✅ COMPLETED (Feb 2026)

---

## 1. Model Evaluation Results

### Models Tested

| Model | Size | Context | Architecture | Load Time | Response Quality |
|-------|------|---------|--------------|-----------|------------------|
| **deepseek-coder-v2:16b-32k** | 16B (2.4B active) | 32K | MoE | **2.6s** | ✅ Excellent |
| devstral-small-2:24b-64k | 24B | 64K | Dense | 19.6s | ✅ Good |

### Test Query (Portuguese)
```
"Como eu faço para mostrar a saída do comando 'ollama list', 
porém ignorando a primeira linha e pegando todos os dados da primeira coluna, 
e depois repassar cada um dos nomes de modelos recuperados para 'ollama show'?"
```

### Results

**deepseek-coder-v2:16b-32k:**
```bash
ollama list | tail -n +2 | cut -d' ' -f1 | while read model; do ollama show "$model"; done
```
- **Time:** 2.6s total (1.8s eval)
- **Approach:** `cut -d' ' -f1` (simpler, assumes single-space delimiter)
- **Verdict:** Fast and concise

**devstral-small-2:24b-64k:**
```bash
ollama list | tail -n +2 | awk '{print $1}' | while read model; do ollama show "$model"; done
```
- **Time:** 19.6s total (8.1s eval)
- **Approach:** `awk '{print $1}'` (more robust for any whitespace)
- **Verdict:** Slower but more robust

### Recommendation
**Use deepseek-coder-v2:16b-32k as default for code mode** due to:
- 7.5x faster response time
- MoE architecture (only 2.4B active params per token)
- Better for quick shell command generation

---

## 2. Optimized System Prompt (English)

Based on testing with both models, the following prompt produced the best results:

```rust
pub const SYSTEM_PROMPT_CODE: &str = r#"\
You are a senior developer invoked through a command-line script on Arch Linux to provide code.

ABSOLUTE RULES:
- Answer ONLY with code, no discursive explanations
- No introductions like "Here is the code" or "This code does..."
- No conclusions like "Hope this helps" or "You can use it like this..."
- No unnecessary explanatory comments (only docstrings if essential)
- Use correct syntax and appropriate languages for the requested task
- Include only the code necessary to solve the problem
- Format code correctly with markdown (```language)
- This is an ephemeral session - no conversation continuation

If the user explicitly asks for explanations, then provide them succinctly.
Otherwise, code only."#;
```

**Key improvements over previous version:**
- Removed Portuguese (consistency with other prompts)
- Removed "Você é..." introspection
- Clearer "ABSOLUTE RULES" section
- Better formatting for markdown blocks
- Explicit ephemeral session notice

---

## 3. Tool-Enhanced Code Mode

### Use Case: Code + File Operations

When code mode is used WITH tools enabled, the model can:
- Inspect project structure before suggesting commands
- Read configuration files (package.json, Cargo.toml, etc.)
- Check existing code patterns before generating new code
- Search for relevant files in the codebase

### System Prompt with Tools

```rust
pub const SYSTEM_PROMPT_CODE_WITH_TOOLS: &str = r#"\
You are a senior developer invoked through a command-line script on Arch Linux to provide code.

You have access to tools that can inspect the local filesystem. Use them when you need to:
- Understand the project structure before suggesting commands
- Read configuration files to understand the environment
- Check existing files before generating code that depends on them
- List directories to understand the codebase layout

ABSOLUTE RULES:
- Answer ONLY with code, no discursive explanations
- No introductions like "Here is the code" or "This code does..."
- No conclusions like "Hope this helps" or "You can use it like this..."
- No unnecessary explanatory comments (only docstrings if essential)
- Use correct syntax and appropriate languages for the requested task
- Include only the code necessary to solve the problem
- Format code correctly with markdown (```language)
- This is an ephemeral session - no conversation continuation

TOOL USAGE GUIDELINES:
- Use list_directory to understand project structure
- Use read_file to inspect configuration files
- Use search_files to find relevant code patterns
- Call tools BEFORE generating final code if needed

If the user explicitly asks for explanations, then provide them succinctly.
Otherwise, code only."#;
```

### Implementation Strategy

**Current logic in main.rs:**
```rust
let prompt_name = if args.code && use_tools {
    "code_with_tools"
} else if args.code {
    "code"
} else if use_tools {
    "tool_user"
} else {
    &args.prompt
};
```

**Blacklisting Impact:**
- Tools blacklisted via config file won't appear in the system prompt
- Models won't know blacklisted tools exist
- Code mode with tools automatically falls back to code-only mode if file tools are blacklisted

---

## 4. Model Configuration

### deepseek-coder-v2:16b-32k Parameters

From modelfile testing:

```toml
PARAMETER num_ctx 32768        # 32K context window
PARAMETER temperature 0.15   # Low for deterministic code
PARAMETER top_k 40
PARAMETER top_p 0.85
PARAMETER min_p 0.02         # Prevents low-probability tokens
PARAMETER repeat_penalty 1.05
```

**Why these values:**
- `temperature: 0.15` - Low for consistent, predictable code
- `min_p: 0.02` - DeepSeek-specific recommendation
- `num_ctx: 32768` - User-specified memory limit

---

## 5. Implementation Plan

### Phase 1: Configuration System Enhancement

**Status:** ⏳ PENDING

**Objective:** Per-subcommand model configuration

**Config Schema:**
```toml
[model]
default = "lfm"                    # Global default

[model.code]                        # NEW: Code mode settings
default = "deepseek-coder-v2"      # Use new model
thinking = false
tools = true                        # Enable file operations

[model.query]
default = "lfm"
thinking = true

[model.summarize]
default = "qwen3.5:4b"
thinking = false

[model.ocr]
# Fixed: glm-ocr:bf16 (not configurable)

[model.translate]
# Fixed: translategemma:4b (not configurable)
```

**Tasks:**
1. [ ] Extend `Settings` struct to support per-subcommand configs
2. [ ] Update `ModelSettings` with subcommand variants
3. [ ] Update each subcommand handler to use respective config
4. [ ] Test backward compatibility (existing configs without subcommand sections)

### Phase 2: Code Mode Model Assignment

**Status:** ⏳ PENDING

**Tasks:**
1. [ ] Add deepseek-coder-v2:16b-32k to `src/config.rs`
2. [ ] Set as default for `model.code` in config
3. [ ] Update code mode prompt logic to use new model
4. [ ] Test with and without tools enabled

### Phase 3: Documentation Update

**Status:** ⏳ PENDING

**Tasks:**
1. [ ] Update `doc/src/configuration.md` with per-subcommand config
2. [ ] Update `doc/src/models.md` with deepseek-coder-v2 info
3. [ ] Update `modelfiles/README.md` if exists
4. [ ] Update man page with new code mode behavior

---

## 6. Testing Checklist

### Before Implementation

- [ ] Test deepseek-coder-v2:16b-32k with various code queries
- [ ] Compare output quality with devstral-small-2
- [ ] Verify tool calls work in code_with_tools mode
- [ ] Check blacklist filtering works correctly

### After Implementation

- [ ] Config file loads correctly with new structure
- [ ] Backward compatibility: old configs still work
- [ ] Code mode uses deepseek-coder-v2 by default
- [ ] Code + tools mode works with file operations
- [ ] Blacklisted tools don't appear in prompts
- [ ] Debug mode shows correct model selection
- [ ] All existing tests pass

---

## 7. Files to Modify

### Core Implementation

| File | Changes |
|------|---------|
| `src/settings.rs` | Add per-subcommand model config |
| `src/config.rs` | Add deepseek-coder-v2 model entry |
| `src/main.rs` | Update subcommand handlers to use config |
| `src/prompts.rs` | ✅ Already updated with new prompts |

### Build System

| File | Changes |
|------|---------|
| `modelfiles/Makefile` | ✅ Already added deepseek-coder-v2 target |
| `modelfiles/deepseek-coder-v2.modelfile` | ✅ Already created |

### Documentation

| File | Changes |
|------|---------|
| `doc/src/configuration.md` | Document per-subcommand config |
| `doc/src/models.md` | Add deepseek-coder-v2 info |
| `doc/src/tools.md` | Document code_with_tools mode |
| `man/sprach.1` | Update man page |

---

## 8. Key Decisions

### ✅ Decided

1. **Code mode default model:** deepseek-coder-v2:16b-32k
2. **System prompt language:** English (consistency)
3. **Tool support in code mode:** Yes, via `code_with_tools` prompt
4. **Configuration approach:** Per-subcommand in config file
5. **Context window:** 32K (user-specified memory constraint)

### ⏳ Pending Discussion

1. **Should code mode EVER use tools?**
   - Pros: Can inspect files before generating code
   - Cons: Slower, may be unexpected
   - **Current approach:** Yes, but controlled via config

2. **Should devstral-small-2 be kept as alternative?**
   - Pros: More robust awk usage
   - Cons: Much slower
   - **Current approach:** Keep both, deepseek as default

3. **Tool blacklisting in code mode:**
   - If file tools are blacklisted, should code mode still try to use them?
   - **Current approach:** Automatically falls back to code-only prompt

---

## 9. Next Steps

**Immediate:**
1. Add this document to roadmap
2. Implement configuration system enhancement
3. Add deepseek-coder-v2 to config.rs
4. Test end-to-end

**Before Tool Robustness Phase:**
1. Ensure all code mode features work correctly
2. Document new configuration options
3. Update examples in documentation

---

## 10. References

- DeepSeek-Coder-V2 Paper: https://arxiv.org/abs/2406.11931
- HuggingFace: https://huggingface.co/deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct
- Ollama Library: https://ollama.com/library/deepseek-coder-v2
- Test results: See conversation history for detailed outputs

---

## Appendix: Test Commands

### To test deepseek-coder-v2:
```bash
# Build modelfile
ollama create deepseek-coder-v2:16b-32k -f modelfiles/deepseek-coder-v2.modelfile

# Test via API
curl http://localhost:11434/api/chat -d '{
  "model": "deepseek-coder-v2:16b-32k",
  "messages": [
    {"role": "system", "content": "[SYSTEM_PROMPT_CODE]"},
    {"role": "user", "content": "Your query here"}
  ],
  "stream": false
}'
```

### To compare with devstral-small-2:
```bash
# Test via sprachspiel
sprach query -c -m devstral-small-2 "Your query"
sprach query -c -m deepseek-coder-v2 "Your query"
```

---

*Document created: 2026-02-17*
*Status: Research complete, ready for implementation*
