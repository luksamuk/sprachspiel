---
name: model-switching
description: Centralized model switching in ask-ai. All model changes MUST go through switch_model() to prevent inconsistent state between think, tools, and capabilities.
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: model-config
---

## What I do

I enforce the SINGLE POINT OF FAILURE rule for model switching in ask-ai. All model changes MUST go through the centralized `switch_model()` function.

## When to use me

Use this skill when implementing model switching functionality, modifying the `/model` command, or any code that changes the active LLM model.

## The Rule

**CRITICAL:** All model switching MUST go through `src/chat/model_switch.rs`.

The `switch_model()` function is the ONLY place that handles:
- Model validation
- Config resolution
- Capability detection
- Think/tools state adjustment
- Warning generation

## Correct Usage

```rust
// ✅ CORRECT - Use the centralized function
match super::model_switch::switch_model(
    name,
    &ollama,
    &capabilities,
    session.think,
    session.tools,
).await {
    Ok(result) => {
        session.set_model(result.model_name.clone());
        session.think = result.think_active;
        session.tools = result.tools_active;
        // ... update other state
    }
    Err(e) => eprintln!("{}", e),
}
```

## What NEVER to Do

```rust
// ❌ WRONG - Never duplicate this logic
if !user_models::is_model_valid(name) { ... }
let config = user_models::resolve_model_config(name);
let caps = ModelCapabilities::detect(...).await;
// ... etc
```

**Why this matters:**
- Prevents inconsistent state between `session.think`, `session.tools`, and `tools_active`
- Ensures capabilities are always detected and warnings are consistent
- Single place to fix bugs related to model switching

## Related Code Paths

| File | What it does |
|------|-------------|
| `src/chat/model_switch.rs` | Central `switch_model()` function |
| `src/chat/custom_coordinator.rs` | Model passed to coordinator |
| `src/capabilities.rs` | `ModelCapabilities::detect_or_default()` |
| `src/user_models.rs` | Model validation and config resolution |
| `src/chat/session.rs` | `ChatSession::set_model()` |

## Common Patterns to Use Instead of Duplicating

1. **Model configuration building** — Use `ModelConfig::build_model_options()`
2. **Capability detection** — Use `ModelCapabilities::detect_or_default()`
3. **Thinking display** — Use `display_thinking()`
4. **Model resolution** — Use `resolve_model_config()`
5. **Think mode validation** — Use `resolve_think_mode()`
6. **Model switching** — Use `model_switch::switch_model()` (this skill)