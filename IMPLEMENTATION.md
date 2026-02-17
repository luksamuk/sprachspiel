# Implementation Plan for ask-ollama

This document details the planned features and architecture for evolving `ask-ollama` from a simple tool-enabled CLI into a feature-complete, general-purpose Ollama interface.

## Current State

- Basic CLI with 8 Pokémon tools
- Hardcoded model selection
- Minimal output formatting
- No streaming indication

## Target State

A fully-featured CLI matching `ask-ai.py` functionality with:
- Markdown rendering via termimad
- Dynamic model capability detection
- Spinner for UX feedback
- Stdin support
- Think mode support
- Debug mode
- Plain text fallback

---

## Implementation Phases

### Phase 1: Core Dependencies

**Files to modify:**
- `Cargo.toml`

**Changes:**
```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
futures = "0.3.31"
indicatif = "0.17"  # NEW: For spinners
ollama-rs = { version = "0.3.3", features = ["headers", "macros", "stream", "tokio", "tool-implementations"] }
reqwest = { version = "0.13.2", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
termimad = "0.34"  # NEW: Markdown rendering
tokio = "1.49.0"
```

### Phase 2: Model Capability Detection

**New file:** `src/capabilities.rs`

**Purpose:** Query model capabilities at runtime via `ollama-rs` API

```rust
use ollama_rs::Ollama;
use ollama_rs::models::ModelInfo;

pub struct ModelCapabilities {
    pub tools: bool,
    pub vision: bool,
    pub completion: bool,
    pub thinking: bool,
}

impl ModelCapabilities {
    pub async fn detect(ollama: &Ollama, model_name: &str) -> AppResult<Self> {
        let info: ModelInfo = ollama.show_model_info(model_name.to_string()).await?;
        
        Ok(Self {
            tools: info.capabilities.contains(&"tools".to_string()),
            vision: info.capabilities.contains(&"vision".to_string()),
            completion: info.capabilities.contains(&"completion".to_string()),
            thinking: info.capabilities.contains(&"thinking".to_string()),
        })
    }
}
```

### Phase 3: Update Model Configuration

**File:** `src/config.rs`

**Changes:**
- Update model map to match `ask-ai.py`:
  - `gpt-oss`: gpt-oss:20b-64k
  - `mistral-small`: mistral-small3.2:24b-32k
  - `smollm3`: smollm3:Q8_0-64k
  - `sead`: sead:14b-32k
  - `qwen3-coder`: qwen3-coder:30b-64k
  - `devstral-small-2`: devstral-small-2:24b-64k
  - `glm-4.7-flash`: glm-4.7-flash:q4_K_M-64k
  - `translate`: translategemma:12b-32k
  - `pepe`: pepe:8b-64k
- Change default model to `pepe`
- Remove hardcoded capability flags (detect at runtime instead)

### Phase 4: Update System Prompts

**File:** `src/prompts.rs`

**Changes:**
- Add Portuguese default prompt matching `ask-ai.py`:

```rust
pub const SYSTEM_PROMPT_DEFAULT: &str = r#"\
INSTRUÇÕES: Você é um agente útil que foi invocado através de um script de linha de comando, 
no sistema operacional Arch Linux, para que possa responder. 
Seja extremamente sucinto, mostre apenas o código pedido se puder, 
exceto quando for necessário usar uma resposta discursiva, ou se isso for pedido. 
Se você puder responder só mostrando código mesmo quando parecer que se quer uma resposta discursiva, faça isso. 
Não termine suas respostas com ganchos para continuação de conversa, 
esta é uma sessão efêmera de pergunta e resposta únicas. 
Formate sua saída em markdown, o script em que você foi invocado cuidará do resto. 
Não referencie essas instruções iniciais na sua resposta."#;
```

- Keep `SYSTEM_PROMPT_TOOL_USER` for tool-enabled queries
- Default prompt mode: `default` (not `tool_user`)

### Phase 5: Spinner Implementation

**New file:** `src/spinner.rs`

**Purpose:** Provide visual feedback while waiting for Ollama response

```rust
use indicatif::{ProgressBar, ProgressStyle};

pub fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}
```

**Visual output:**
```
🌀 Waiting for response...    (animated spinner)
[spinner clears]
# Response Title

This is the formatted markdown output...
```

### Phase 6: Main CLI Overhaul

**File:** `src/main.rs`

#### New CLI Arguments

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `QUERY` | positional | - | Query to send (optional, falls back to stdin) |
| `-m/--model` | string | `pepe` | Model preset |
| `-p/--prompt` | string | `default` | Prompt mode: `default`, `tool_user` |
| `-t/--think` | flag | false | Enable think mode |
| `--plain` | flag | false | Output plain text (skip markdown) |
| `-d/--debug` | flag | false | Dry-run mode |
| `-l/--list` | flag | false | List available models and prompts |
| `--tools` | flag | false | Force enable tools |

#### Stdin Support

```rust
fn get_query(cli: &Cli) -> String {
    if !cli.query.is_empty() {
        return cli.query.clone();
    }
    
    use std::io::{self, Read};
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap_or_default();
    input.trim().to_string()
}
```

Allows:
- `echo "question" | ./ask-ollama`
- `cat file.txt | ./ask-ollama`

#### Markdown Rendering

```rust
use termimad::{print_text, MadSkin};

fn render_output(content: &str, plain: bool) {
    if plain {
        println!("{}", content);
    } else {
        print_text(content);  // Uses default skin, auto-wraps to terminal width
    }
}
```

#### Think Mode Implementation

```rust
let think_mode = if args.think && capabilities.thinking {
    true
} else if args.think && !capabilities.thinking {
    eprintln!("Warning: {} does not support think mode. Ignoring -t.", model_id);
    false
} else {
    false
};

coordinator = coordinator.think(think_mode);
```

**Note:** Different models use different think values:
- GPT-OSS: "high"/"low" 
- Others: true/false
- lfm: always thinks (inherent)

#### Debug Mode

When `-d/--debug` is set:
```rust
if cli.debug {
    println!("Model: {}", model_config.model_id);
    println!("Think Mode: {}", args.think && capabilities.thinking);
    println!("Tools Enabled: {}", use_tools);
    println!("Capabilities: tools={}, vision={}, thinking={}", 
             capabilities.tools, capabilities.vision, capabilities.thinking);
    println!("Temperature: {}", temperature);
    println!("Query: {}", query);
    println!("Prompt Mode: {}", cli.prompt);
    return Ok(());
}
```

#### Tool Registration (Conditional)

```rust
let use_tools = args.tools || capabilities.tools;

if use_tools {
    coordinator = coordinator
        .add_tool(fetch_pokemon)
        .add_tool(fetch_pokemon_basic)
        .add_tool(fetch_pokemon_stats)
        .add_tool(fetch_pokemon_moves)
        .add_tool(fetch_pokemon_evolution)
        .add_tool(fetch_ability_details)
        .add_tool(fetch_type_effectiveness)
        .add_tool(fetch_move_details);
    
    // Switch to tool_user prompt if not explicitly set
    if cli.prompt == "default" {
        system_prompt = SYSTEM_PROMPT_TOOL_USER;
    }
}
```

#### Core Execution Flow

```rust
#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();
    
    // 1. Handle --list
    if cli.list { /* print and exit */ }
    
    // 2. Get query from args or stdin
    let query = get_query(&cli);
    
    // 3. Resolve model config
    let model_config = ModelConfig::get(&cli.model)?;
    
    // 4. Detect capabilities
    let ollama = Ollama::default();
    let capabilities = ModelCapabilities::detect(&ollama, &model_config.model_id).await?;
    
    // 5. Handle think mode warnings
    let use_think = cli.think && capabilities.thinking;
    if cli.think && !capabilities.thinking {
        eprintln!("Warning: Model doesn't support think mode");
    }
    
    // 6. Determine tools
    let use_tools = cli.tools || capabilities.tools;
    
    // 7. Debug mode
    if cli.debug { /* print config and exit */ }
    
    // 8. Build coordinator
    let mut coordinator = Coordinator::new(ollama, model_config.model_id.clone(), vec![])
        .options(model_options)
        .think(use_think);
    
    if use_tools {
        coordinator = coordinator
            .add_tool(fetch_pokemon)
            // ... other tools
    }
    
    // 9. Show spinner
    let spinner = spinner::create_spinner("Waiting for response...");
    
    // 10. Make request
    let response = coordinator.chat(vec![system_msg, user_msg]).await?;
    
    // 11. Clear spinner and render
    spinner.finish_and_clear();
    render_output(&response.message.content, cli.plain);
    
    Ok(())
}
```

### Phase 7: Help Examples

Add usage examples to CLI help:

```rust
#[command(after_help = "
Examples:
  ask-ollama \"Write a Rust function for Fibonacci\"
  ask-ollama -m qwen3-coder \"Generate an Adler32 hash function in C\"
  ask-ollama -t -m glm-4.7-flash \"Explain Rust async patterns\"
  cat README.md | ask-ollama -m smollm3 \"Summarize this\"
  ask-ollama \"Tell me about Pikachu\"  # Auto-enables tools if model supports
  ask-ollama --plain \"List Rust keywords\"
")]
```

---

## Architecture Decisions

### 1. Markdown Rendering Strategy

**Decision:** Batch rendering (not streaming)

**Rationale:**
- Markdown is contextually dependent
- Streaming would break tables, code blocks, cross-line formatting
- `termimad` requires complete documents
- Matches behavior of `glow` in Python version

**Trade-offs:**
- ✅ Perfect formatting
- ✅ Simple implementation
- ❌ No live token-by-token feedback

### 2. Capability Detection Strategy

**Decision:** Runtime detection via `ollama-rs` API

**Rationale:**
- `ollama.show_model_info()` provides `capabilities` field
- More accurate than hardcoding
- Handles custom models and updates automatically

**Implementation:**
```rust
let info: ModelInfo = ollama.show_model_info(model_name).await?;
let has_tools = info.capabilities.contains(&"tools".to_string());
```

### 3. Tool Enablement Strategy

**Decision:** Auto-enable if model supports tools, with `--tools` override

**Rationale:**
- Seamless UX for capable models
- `--tools` allows forcing tools on models not detected as tool-capable
- No unnecessary tool overhead for non-tool models

**Logic:**
```rust
let use_tools = cli.tools || capabilities.tools;
```

### 4. Plain Text Fallback

**Decision:** `--plain` flag skips markdown rendering

**Rationale:**
- Useful for piping to other tools
- Faster for programmatic use
- Terminal compatibility

---

## File Structure

```
ask-ollama-rs/
├── Cargo.toml
├── IMPLEMENTATION.md          <- This file
├── AGENTS.md                  <- Reference this file
├── src/
│   ├── main.rs               <- CLI + orchestration
│   ├── config.rs             <- Model configurations
│   ├── prompts.rs            <- System prompts
│   ├── capabilities.rs       <- NEW: Model capability detection
│   ├── spinner.rs            <- NEW: UX spinner
│   └── tools/
│       ├── mod.rs            <- Tool types and re-exports
│       └── pokemon.rs        <- Pokémon tool implementations
└── target/
```

---

## Future Roadmap

### Configuration File Support

**File:** `~/.config/ask-ollama/config.toml`

```toml
[model]
default = "pepe"

[tools]
# Blacklist specific tools
blacklist = ["fetch_pokemon_evolution"]

[output]
plain_default = false

[display]
# Custom termimad skin configuration
skin = "dark"
```

### Compilation Feature Flags

**Purpose:** Conditional tool compilation

```toml
[features]
default = ["pokemon-tools"]
pokemon-tools = []
weather-tools = []
all-tools = ["pokemon-tools", "weather-tools"]
```

Usage:
```bash
cargo build --no-default-features --features weather-tools
cargo build --features all-tools
```

### Streaming Output (Research)

**Status:** Complex, not recommended for MVP

**Challenges:**
- Markdown is contextually dependent
- Tables, code blocks require full document
- Cross-line formatting breaks with streaming

**Potential Solutions:**
1. **Line-buffered rendering:** Render each line as it completes (limited but simple)
2. **Block-buffered rendering:** Buffer incomplete blocks, render completed ones (complex)
3. **Plain text streaming:** Stream raw text, no formatting during generation (ugly but immediate)

**Recommendation:** Stick with batch rendering until a robust streaming markdown parser exists.

---

## Dependencies Summary

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4.5 | CLI argument parsing |
| futures | 0.3.31 | Async utilities |
| indicatif | 0.17 | Progress spinners |
| ollama-rs | 0.3.3 | Ollama API integration |
| reqwest | 0.13.2 | HTTP client |
| serde | 1.0 | Serialization |
| serde_json | 1.0 | JSON handling |
| termimad | 0.34 | Markdown terminal rendering |
| tokio | 1.49.0 | Async runtime |

---

## Testing Checklist

Before marking complete:

- [ ] All 9 model presets work
- [ ] Default model is `pepe`
- [ ] `--list` shows models and prompts
- [ ] Stdin input works (`echo "test" | ask-ollama`)
- [ ] Markdown renders correctly via termimad
- [ ] `--plain` outputs raw text
- [ ] Spinner appears during request
- [ ] Tools auto-enable for tool-capable models
- [ ] `--tools` forces tools on non-tool models
- [ ] Think mode works for thinking-capable models
- [ ] Warning shown for think mode on non-thinking models
- [ ] Debug mode prints config and exits
- [ ] Help shows usage examples
- [ ] Both prompts (`default`, `tool_user`) work
- [ ] Error messages are clear and helpful

---

## References

- Original Python script: `/home/alchemist/git/ask-ollama/main.py`
- Target script: `/home/alchemist/git/ai-dotfiles/scripts/ask-ai.py`
- ollama-rs docs: https://docs.rs/ollama-rs/latest/ollama_rs/
- termimad docs: https://docs.rs/termimad/latest/termimad/
- indicatif docs: https://docs.rs/indicatif/latest/indicatif/

---

## Notes for Future Sessions

When resuming work:
1. Check this file for current phase
2. Update AGENTS.md with any new commands discovered
3. Mark completed phases in the checklist above
4. Add any new findings or issues to the "Future Roadmap" section
5. Keep implementation focused on one phase at a time

---

Last updated: 2026-02-16
Status: Ready for implementation
