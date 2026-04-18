# Draft: Subagent Model Resolution + Prompt/Doc Fix

## Requirements (confirmed)
- Subagents MUST use models from config.toml (e.g., `[model.translate]` → `translategemma:4b`), NOT hardcoded model names
- Subagent prompts must clearly explain: they operate as CLI module replacements within chat, composing context as tools
- All 5 subagent types need verified prompts and documentation
- Chat commands (/ocr, /vision, /translate, /summarize) and spawn_subagent LLM tool both need proper documentation

## Critical Bugs Found

### Bug 1: Translate subagent gets wrong model
- `get_subcommand_config("translate")` returns `self.model.default` ("qwen3.5:4b") when `[model.translate].model` is not set
- The "translategemma" fallback only exists in `main.rs` CLI path, NOT in `get_subcommand_config()`
- Subagent calls `get_subcommand_config("translate")` → gets `"qwen3.5:4b"` instead of `"translategemma"`
- **FIX**: Add translate-specific fallback in `get_subcommand_config()` like OCR already has

### Bug 2: Subagent prompts reference non-existent tools
- `subagent_tools.rs` prompts say "use the `ocr_image` tool", "call `translate_text`", etc.
- But subagents run BARE — no tools are registered (except Document which has `run_command`)
- LLM subagent receives these instructions, tries to call tools that don't exist, wastes tokens
- **FIX**: Rewrite prompts to describe direct task execution, not tool calling

### Bug 3: Subagents don't use ModelConfig (temperature, context window)
- Subagents get model name from `get_subcommand_config()` then pass directly to Ollama
- They use `ModelOptions::default().temperature(0.0)` — ignoring per-model config
- `translategemma` has `temperature: 0.2` and `num_ctx: 4096` in built-in config
- But subagent sends `temperature(0.0)` and `num_ctx: default`
- **FIX**: Use `user_models::resolve_model_config()` to get ModelConfig, then `build_model_options()`

### Bug 4: Subagent prompts misrepresent their role
- Current: "You are an OCR specialist agent. Your task is to extract text from images using the `ocr_image` tool."
- Should be: They operate as lightweight CLI-like modules invoked within chat, processing input directly
- **FIX**: Rewrite all prompts to accurately describe what they do

### Bug 5: Missing delegation guidance in prompts/tools.rs
- `spawn_subagent` is listed as a tool but there's no guidance about WHEN to delegate vs doing directly
- No examples of effective delegation in the main agent's prompt
- **FIX**: Add delegation strategy section to prompts/tools.rs

## Current Model Resolution Comparison

| Path | translate model | Uses ModelConfig? | Temperature |
|------|----------------|-------------------|-------------|
| CLI `ask-ai translate` | "translategemma" (hardcoded fallback in main.rs) | YES via user_models | 0.2 (from ModelConfig) |
| Chat `/translate` | `get_subcommand_config("translate")` → "qwen3.5:4b" | NO | 0.0 (hardcoded) |
| spawn_subagent tool | `get_subcommand_config("translate")` → "qwen3.5:4b" | NO | 0.0 (hardcoded) |

**The subagent gets a DIFFERENT model AND wrong settings than the CLI path!**

## Technical Decisions
- Add "translategemma" fallback in `get_subcommand_config("translate")` — matches CLI behavior
- Use `user_models::resolve_model_config()` in SubagentRunner to get proper ModelConfig
- Rewrite subagent prompts: remove tool references, describe direct task execution
- Keep `SubagentConfig.model` as String (model name), add `SubagentConfig.model_options: ModelOptions`
- Or: pass ModelConfig to SubagentRunner instead of just model name

## Scope Boundaries
- INCLUDE: Fix model resolution for ALL 5 subagent types
- INCLUDE: Fix/verify prompts for ALL 5 subagent types
- INCLUDE: Fix/verify documentation for spawn_subagent and chat commands
- INCLUDE: Fix ModelConfig integration (temperature, num_ctx from config)
- EXCLUDE: Changing CLI subcommand model resolution (already works correctly)
- EXCLUDE: Changing config.toml structure (use existing structure)
- EXCLUDE: Changing OcrProcessor hardcoded model (separate concern, same result as get_subcommand_config)