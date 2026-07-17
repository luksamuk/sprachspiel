# Feature Status

Per-issue tracking for planned and in-progress features. For completed features, see [Completed Features](./completed-features.md). For the milestone overview, see [Roadmap](./roadmap.md).

### Skills System

**Priority:** Medium  
**Status:** Research needed

**Goal:** Load custom behaviors from `.sprachspiel/skills/` or `~/.config/sprachspiel/skills/`.

**Tasks:**
- [ ] Research: Skill systems in other agents
- [ ] Design: Skill file format
- [ ] Implement: Skill parser
- [ ] Implement: `--skill` flag

---

### Effective AI Coding Agents Analysis

**Priority:** Medium  
**Status:** Research Complete

**Goal:** Apply lessons learned from academic research on terminal-native AI agents.

Analysis of the paper "Building Effective AI Coding Agents for the Terminal" (OPENDEV, arXiv:2603.05344v2) comparing best practices with Sprachspiel architecture.

**Key Findings:**
- Sprachspiel implements ~60-70% of recommended patterns
- Strong alignment: Context Engineering (hybrid retrieval), Session Management, Tool System
- Gaps: Memory System (structured facts), System Reminders, Adaptive Compaction

**Recommendations:**
- Memory System for extracted facts (integrates with planned Notes + Document Import)
- System Reminders for instruction fade-out mitigation
- Per-workflow model selection for resource optimization

**Full Analysis:** See [Effective Agents Analysis](./effective-agents-analysis.md) for detailed comparison, code references, and implementation roadmap.

---

### Core Enhancements [M1]

**Status:** 📋 PLANNED / 🟡 RESEARCH

| Card # | Feature | Status | Effort |
|--------|---------|--------|--------|
| #116 | Retry Threshold with Backoff | ✅ Completed | 1.5-2 days |
| #118 | Tool Trait + `#[sprachspiel::tool]` Proc Macro | ✅ Completed | 1-1.5 weeks |
| #119 | Agnostic Provider Types | ✅ Completed | 1 week |
| #120 | OllamaProvider (reqwest direct) | ✅ Completed | 2-3 weeks |
| #121 | Consumer Migration | ✅ Completed | 2-3 weeks |
| #122 | OpenAI-Compatible Provider | ✅ Completed | 2 weeks |
| #123 | Remove ollama-rs from Cargo.toml | ✅ Completed | 2-3 days |
| #72 | Multi-Provider Support (parent issue) | 📋 Planned | 10-12 weeks |
| #107 | Embedding Provider Abstraction | 📋 Planned | 1-2 weeks |
| #106 | Configurable Embedding Model + Matryoshka | 📋 Ready | 1 week |
| #74 | Context Pinning | 🟡 Research | 2-4 days |
| #75 | Dynamic Context Limits | 🟡 Research | 1-2 days |
| #76 | Secret Scanning (Content) | 📋 Planned | 1-2 days |
| #105 | Config Upgrade Command | ✅ Completed | 5 days |

**T3 — Thinking Trace Pipeline & Retrieval:**

| Card # | Feature | Status | Priority | Milestone |
|--------|---------|--------|----------|-----------|
| #151 | T3-Phase0: Preserve Thinking Content + Schema Foundation | ✅ Completed (PR #189) | 🔴 P0-CRITICAL | M1/W4.5 |
| #152 | T3-Phase1: ThinkingTrace Pipeline + Struct Transform | 📋 Not Started | P0-HIGH | M1/W7.0 |
| #153 | T3-Phase2: Thinking-Aware Retrieval + RRF Fusion | 📋 Not Started | P0-HIGH | M1/W7.1 |

**T3 Phases (deferred to M3/M4):**

| Phase | Feature | Priority | Milestone |
|-------|---------|----------|-----------|
| Phase 3 | T3-Phase3: Semantic/Reflect + Facts Integration | P1 | M3 |
| Phase 4 | T3-Phase4: Optimization (Quality > Quantity, Compression, Caching) | P2 | M4 |

**Reference:** Arabzadeh et al. 2026, arXiv:2605.03344 — "RAG over Thinking Traces Can Improve Reasoning Tasks"

---

### Sprach 2.0: CAS Research [M3]

**Status:** 🟡 RESEARCH NEEDED  
**Full Design:** See [Sprach 2.0 Research](./sprach-2-0-research.md) for open questions, code analysis, and implementation details.

Self-analysis identifying sprachspiel as a Complex Adaptive System (CAS) with emergent properties but limited open-endedness. Proposals aim to increase emergent connectivity and adaptive behavior.

| ID | Proposal | Depends On | Status | Effort |
|----|----------|------------|--------|--------|
| S2.1 | Visualize Connections Tool | None | 🟡 Research | 2-3 days |
| S2.2 | Content Relations Graph (2-layer) | S2.1 | 🟡 Research | 5-8 days |
| S2.3 | Reflection on Triggers + Curation | S2.1, S2.2 | 🟡 Research | 4-7 days |
| S2.4 | Plugin System (WASM) | — | 📋 #15 (existing) | 3-4 weeks |
| S2.5 | SOUL.md Patching + `/apply-patch` | S2.3 | 🟡 Research | 3-5 days |
| S2.6 | Skills Auto-Registration (Meta) | S2.1-S2.5 | 🕐 Wait | TBD |
| S2.meta1 | Meta-cognition Skill (Layer 1) | None | 🟡 Prototype (Issue #99) | 1h |
| S2.meta2 | Behavioral Telemetry (Layer 2) | Feedback system, S2.meta1 data | 📋 Planned (Issue #100) | 2-3 days |
| S2.meta3 | Behavioral Reflection + Personality (Layer 3) | S2.3, S2.5, S2.meta2 | 📋 Planned (Issue #101) | 1-2 weeks |

**Meta-cognition Behavioral Proposals (S2.meta1-S2.meta3):**

Three-layer approach to behavioral self-monitoring. **Layer 1** (skill) is a prototype and data collection instrument — testing confirmed it works with high-reasoning models but cannot guarantee execution, produce structured data, or work with smaller models. **Layer 2** (behavioral telemetry in system prompt) is the real implementation — deterministic heuristic detection in the harness, not dependent on the LLM self-monitoring. **Layer 3** (persistent personality adjustment) requires S2.3 and S2.5. Key reframing: **empathy is not a bug — opacity is.** The goal is not to suppress behavioral shifts, but to make them visible and give the user control. Calibration insight from testing: the detector should focus on **unannounced system drift** rather than **user-initiated topic changes**. See [Meta-cognition Proposal](./meta-cognition-proposal.md) for the integration plan.

**Validated Decisions (DEC-001 to DEC-007):**

| Decision | Ruling | Validation |
|----------|--------|------------|
| DEC-001 | Cache incremental for relations (on-demand, not pre-computed) | GraphSeek 2026, Graph RAG 2026 |
| DEC-002 | Reflection triggers (not periodic) | ICML 2025, MeCo arXiv 2025 |
| DEC-003 | Curation with human approval (drafts, not auto-publish) | Rewire.it, "Human-in-the-loop" |
| DEC-004 | WASM sandbox by capabilities (allowed/denied, not total isolation). **CRITICAL:** DEC-007 extends this — `process_spawn` deny is meaningless when MCP STDIO *is* process spawning. STDIO MCP servers require explicit allowlist + sandbox. | The New Stack 2026, MCP-SandboxScan, OX Security 2026 |
| DEC-005 | Semantic versioning for plugins (major equal, minor ≥) | OpenFang, "Semver + manifest signing" |
| DEC-006 | SOUL.md patches require human approval (suggestions, not automatic) | MetaMind NeurIPS 2025, "Human oversight" |
| DEC-007 | MCP STDIO: no untrusted command execution (explicit approval + allowlist + sandbox) | OX Security 2026, CVE-2025-65720, CVE-2026-30623, CVE-2026-30624, CVE-2026-30618, CVE-2026-33224, CVE-2026-30625, CVE-2026-30615, CVE-2026-26015, CVE-2026-40933, CVE-2025-49596, CVE-2026-22252, CVE-2026-22688, CVE-2025-54994, CVE-2025-54136 |

**Competitors:** Joplin GSoC 2026 (note graphs with AI), OpenClaw (WASM sandbox for community skills)

---

## Low Priority

### Plugin System

**Priority:** Low  
**Status:** Not started

User-defined tools via dynamic loading or compilation.

**Security Note (ADR-007):** MCP STDIO transport has a by-design RCE vulnerability (CVE-2025-65720 et al.). When implementing MCP client integration, STDIO servers MUST use an explicit command allowlist in `config.toml` and require user approval. Prefer HTTP/SSE transport. See IMPLEMENTATION.md ADR-007 for full details.

---

### Responsive Chat Rebuild with Ratatui [M1, W6]

**Status:** ✅ COMPLETE (v0.44.0, PR #155)

**Goal:** Rebuild the chat REPL using Ratatui as the rendering framework. Same chat UX, but responsive layout that adapts to terminal width. Replaces `println!` + hardcoded ANSI with declarative rendering.

**This is NOT the full TUI (#16).** This is the foundation — rendering engine, event loop, and crossterm input. The full TUI (sidebars, /queue, /steer, multi-pane) builds ON TOP of this in M2.

**Problem:** Chat only rendered correctly at 80 columns. Any resize produces broken output. Root cause: 600+ `println!` calls with hardcoded widths across 222 ANSI escape sequences.

**Architecture:** The `ChatView` and `InputBackend` traits already exist for this migration. We implement `RatatuiView` and `CrosstermInput` as the new backends.

```
┌─ Responsive Chat Architecture ─────────────────────────┐
│                                                         │
│  App (event loop)                                       │
│  ├── CrosstermInput ── implements InputBackend           │
│  │   └── tab completion, history, crossterm key events  │
│  ├── RatatuiView ──── implements ChatView                │
│  │   └── responsive layout (chat area + status + input) │
│  ├── mpsc channel ──── LLM streaming tokens             │
│  └── ratatui terminal.draw() ── declarative rendering    │
│                                                         │
│  Existing traits (no changes):                          │
│  ├── ChatView trait (view/mod.rs)                       │
│  └── InputBackend trait (input/mod.rs)                  │
└─────────────────────────────────────────────────────────┘
```

**Delivered in 4 PRs (each left the codebase functional and testable):**

| PR | Scope | Key Deliverable | Issue |
|----|-------|-----------------|-------|
| PR #152 | CommandResult — decouple logic from presentation | All output goes through `CommandResult` enum + `ChatView` | #145 ✅ Completed |
| PR #153 | Ratatui infrastructure + responsive rendering | `RatatuiView` with `--tui` flag for visual testing | #146 ✅ Completed |
| PR #154 | Crossterm input + event loop + streaming | Full chat, commands, streaming in `--tui` mode | #147 ✅ Completed |
| PR #155 | Final transition — remove rustyline, make ratatui default | Single rendering mode, responsive at any width | #148 ✅ Completed |

**Dependencies Added:**
- `ratatui = "0.29"` — TUI rendering framework
- `crossterm = { version = "0.28", features = ["event-stream"] }` — terminal backend + input
- `ratatui-textarea` — full-featured text editing widget
- `tui-markdown = "0.2"` — markdown rendering in ratatui widgets
- `unicode-segmentation = "1.11"` — cursor movement in input editing

**Dependencies Kept:**
- `indicatif` — subcommand spinners (non-chat)
- `rattles` — animation frames (ratatui widget + non-chat, more natural integration)

**Dependencies Removed:**
- `rustyline` — input now via crossterm
- `termimad` — replaced by standalone monochrome markdown renderer

**Prerequisite for:** Full TUI (#16, M2) — `/queue`, `/steer`, sidebars, multi-pane layout all build on top of this infrastructure.

---

### TUI (Terminal User Interface) [M2]

**Status:** 📋 PLANNED (W6 foundation complete, awaiting UX design)

**Goal:** Build the full TUI experience on top of the Responsive Chat Rebuild (W6): sidebars, /queue, /steer, multi-pane layout, UX design, and formal ApplicationBackend abstraction.

**Depends on:** Responsive Chat Rebuild (M1, W6) ✅ COMPLETE — the Ratatui rendering engine, event loop, CrosstermInput, and CommandResult enum are now available.

**What W6 already delivers (no need to re-implement):**

| Item | W6 Deliverable | PR |
|------|---------------|-----|
| Chat pane with markdown rendering | `RatatuiView` + `tui-markdown` | PR 2 |
| Input pane with history | `CrosstermInput` + tab completion | PR 3 |
| Status bar (model, context, tokens) | Ratatui widget, responsive | PR 2 |
| Ratatui research | Architecture defined | PR 1-4 |
| Terminal resize handling | `AppEvent::Resize` | PR 3 |
| `InputBackend` → crossterm impl | `CrosstermInput` | PR 3 |
| `ChatView` → ratatui impl | `RatatuiView` | PR 2 |
| CommandResult enum | Decoupled logic from presentation | PR 1 |
| Concurrent input channel (mpsc) | Event loop with tokio | PR 3 |
| Responsive layout at any width | Declarative ratatui layout | PR 2 |

**What #16 still needs to build:**

| Item | Description | Effort |
|------|-------------|--------|
| Sidebar for tools/messages | Multi-pane layout with tool call details | 1-2 weeks |
| `/queue` and `/steer` busy-input modes | Concurrent input during LLM execution (#117) | 2-3 weeks |
| `ApplicationBackend` trait | Formal decoupling for CLI/TUI/ACP backends | 1-2 weeks |
| UX design mockups | Full TUI wireframes with sidebars, scrollback | 1 week |
| PageUp/PageDown scrollback | History navigation in chat area | 2-3 days |
| Mascote ASCII indicator | Visual state indicator | 1-2 days |

**Milestone split:**
- **M2 (UX & TUI Design):** UX research, design mockups, prototyping, feedback rounds. Includes Interaction Modes Design (`/queue`, `/steer`) as a core UX feature.
- **M3 (TUI Implementation):** Coding sidebars, /queue, /steer, multi-pane layout on top of W6 infrastructure. Happens alongside Sprach 2.0 research.

**Architecture Preparation (delivered by W6):**
- ✅ `InputBackend` trait — abstracts input handling
- ✅ `ChatView` trait — abstracts output rendering
- ✅ `ReplState` struct — separates state from I/O
- ✅ `CommandResult` enum — decouples logic from presentation
- ✅ `RatatuiView` — ratatui rendering backend
- ✅ `CrosstermInput` — crossterm input backend
- ✅ `App` event loop — tokio + mpsc for async communication
- 📋 `ApplicationBackend` trait — formal decoupling for ACP

**Architectural Requirement (ACP Prerequisite):**

The TUI implementation MUST create a clean `ApplicationBackend` trait that decouples core logic from the I/O layer. This decoupling is required for B8 (ACP Agent Integration).

```
ApplicationBackend (trait) — #16 creates this
   ├── TUI (RatatuiView + CrosstermInput) — already delivered by W6
   └── ACP (stdio JSON-RPC) — B8
```

Note: The CLI backend (RustylineInput + TerminalView) is removed in W6 PR 4, so ACP becomes the second backend, not the third.

**Interaction Modes Design (#117):**

Three busy-input modes for the TUI, inspired by Hermes Agent's `/queue` and `/steer` commands:

| Mode | Input during execution | UX | Use case |
|------|----------------------|-----|----------|
| `interrupt` (default) | Ctrl+C kills current run | Current CLI behavior | "Stop everything" |
| `queue` | `/queue <prompt>` enqueues for next turn | `"Queued: check logs"` | Sequential tasks: "do A, then B" |
| `steer` | `/steer <prompt>` injects guidance mid-run | `"⏩ Steer: focus on errors"` | Mid-course correction |

**Why TUI-only:** The current rustyline input is blocking — it cannot receive input while the LLM is running. `/queue` and `/steer` require a concurrent input channel (`mpsc`), which the TUI naturally provides via its event loop.

**Config:**
```toml
[tui]
busy_input_mode = "steer"  # "interrupt" | "queue" | "steer"
```

**Reference:** See `IMPLEMENTATION.md` - #16 TUI for detailed interaction modes architecture.

---

### Multilingual Injection Detection

**Priority:** Low  
**Status:** Not started

**Tasks:**
- [ ] Research: Injection patterns in non-English languages
- [ ] Implement: Multilingual pattern detection

---

## Low Priority

### Multi-Provider Support (OpenAI-Compatible Backends) — #72 [M1]

**Status:** 📋 PLANNED (full ollama-rs removal)  
**Issue:** #72

**Prerequisites:**

| Issue | What | Status | Note |
|-------|------|--------|------|
| #116 | Retry Threshold with Backoff | ✅ Completed | First in dependency chain |
| #118 | Tool Trait + Proc Macro | ✅ Completed | Can start in parallel with #116 |
| #106 | Configurable Embedding Model | 📋 Ready | Required before embedding provider swap |
| #107 | Embedding Provider Abstraction | 📋 Planned | Sub-task of provider migration |

> **NOTE:** This feature is tracked in IMPLEMENTATION.md under the "Core Enhancements" section. The detailed dependency chain (#116→#118→#119→#120→#121→#122→#123) is documented there. This roadmap section is kept for architectural reference.

**Goal:** Abstract provider differences to support both Ollama (local) and OpenAI-compatible APIs (cloud/local) through a unified interface.

**Motivation:**
- **Performance:** llama.cpp server with OpenAI-compatible endpoints can be faster than Ollama for local models
- **Primary Target:** OpenAI API - enable cloud-based models for users without local GPU
- **Compatibility Targets:**
  - llama.cpp (via OpenAI-compatible `/v1/chat/completions` endpoint)
  - LM Studio (same pathway)
- **Deferred:** vLLM support postponed until testing resources available

**Current State:**
- `ollama-rs` is tightly coupled to Ollama's native API
- Tool calling uses `ollama-rs::generation::tools::Tool` trait
- Embeddings use `/api/embeddings` endpoint
- Model capabilities detected via `/api/show_model_info`
- Context overflow depends on `prompt_eval_count` from responses

**Architecture:**

```rust
/// Provider abstraction for LLM backends
trait LlmProvider {
    /// Send chat request with optional tools
    async fn chat(&self, messages: Vec<ChatMessage>, tools: Vec<Tool>) -> Result<ChatResponse>;
    
    /// Generate completion (for non-chat use cases)
    async fn generate(&self, prompt: &str, options: GenerateOptions) -> Result<GenerateResponse>;
    
    /// Get model capabilities (tools, vision, thinking)
    async fn get_model_capabilities(&self, model: &str) -> Result<ModelCapabilities>;
    
    /// Generate embeddings for text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    
    /// Check provider health/availability
    async fn is_available(&self) -> bool;
}

/// Ollama native provider (current implementation)
struct OllamaProvider { 
    client: ollama_rs::Ollama,
    base_url: String,
}

/// OpenAI-compatible provider (OpenAI API, llama.cpp, LM Studio)
struct OpenAICompatibleProvider { 
    client: reqwest::Client, 
    base_url: String,
    api_key: Option<String>,
}

/// Provider selector based on model configuration
enum Provider {
    Ollama(OllamaProvider),
    OpenAI(OpenAICompatibleProvider),
}
```

**Key Design Decisions:**

| Decision | Rationale |
|----------|-----------|
| Adapter pattern | Clean separation, existing Ollama code unchanged |
| Enabled by default | No feature flag complexity |
| Per-model provider | User can mix local (Ollama) + cloud (OpenAI) |
| Config-based capabilities | OpenAI-compatible servers don't have `/api/show_model_info` |

**Impact Analysis:**

| Component | Ollama API | OpenAI-Compatible | Effort | Notes |
|-----------|------------|-------------------|--------|-------|
| Chat/Query | `/api/chat` | `/v1/chat/completions` | LOW | Same message format |
| Tool Calling | Native tools | Function calling | HIGH | Format conversion required |
| Embeddings | `/api/embeddings` | `/v1/embeddings` | HIGH | Response format differs |
| Vision/OCR | `/api/generate` + images | Vision messages | MEDIUM | Image encoding differs |
| Capabilities | `/api/show_model_info` | Config-based | LOW | No API for this |
| Context Overflow | `prompt_eval_count` | `usage.prompt_tokens` | LOW | Minor parsing change |

**Configuration:**

```toml
# ~/.config/sprachspiel/models.toml

# Default provider (ollama or openai)
default_provider = "ollama"

# Provider-specific settings
[providers.ollama]
base_url = "http://localhost:11434"

[providers.openai]
base_url = "https://api.openai.com/v1"  # or "http://localhost:8080/v1" for llama.cpp
api_key = "${OPENAI_API_KEY}"  # Environment variable

# Per-model provider override
[models."gpt-4o"]
provider = "openai"
tools = true
vision = true
thinking = false

[models."qwen3.5:4b"]
provider = "ollama"  # Explicit, but default anyway
tools = true
vision = true
thinking = false
```

**Implementation Phases:**

| Phase | Status | Description | Effort |
|-------|--------|-------------|--------|
| 1. Provider Trait | 📋 Planned | Define `LlmProvider` trait | 1 day |
| 2. Ollama Adapter | 📋 Planned | Wrap existing `ollama-rs` in adapter | 2 days |
| 3. OpenAI Adapter | 📋 Planned | Implement OpenAI-compatible client | 3 days |
| 4. Tool Conversion | 📋 Planned | Ollama ↔ OpenAI tool format conversion | 2 days |
| 5. Embeddings | 📋 Planned | Unified embedding interface | 2 days |
| 6. Vision/OCR | 📋 Planned | Image support through providers | 1 day |
| 7. Configuration | 📋 Planned | Provider per model in `models.toml` | 1 day |
| 8. Testing | 📋 Planned | Integration tests with real providers | 2 days |
| **Total** | | | **14 days** |

**Key Files to Modify:**

| File | Change |
|------|--------|
| `src/provider/mod.rs` | NEW - Provider trait and registry |
| `src/provider/ollama.rs` | NEW - OllamaProvider adapter |
| `src/provider/openai.rs` | NEW - OpenAICompatibleProvider |
| `src/settings.rs` | Provider configuration parsing |
| `src/capabilities.rs` | Config-based capability detection |
| `src/chat/custom_coordinator.rs` | Use `LlmProvider` trait instead of `ollama-rs` directly |
| `src/embeddings/client.rs` | Provider abstraction for embeddings |

**Dependencies:**
- None (adapter abstraction, enabled by default)

**Notes:**
- OpenAI API is the **primary implementation target** for cloud-based models
- llama.cpp and LM Studio gain compatibility **through** OpenAI-compatible adapter
- vLLM support **postponed** until testing resources available
- Ollama remains primary **local** provider; OpenAI for **cloud** models
- Users can mix providers: Ollama for local + OpenAI for specific cloud models
- Tool calling conversion is bidirectional (Ollama ↔ OpenAI format)
- No feature flag - abstraction is internal architecture detail

**References:**
- OpenAI API Reference: https://platform.openai.com/docs/api-reference/chat
- llama.cpp OpenAI-compatible server: https://github.com/ggerganov/llama.cpp/blob/master/examples/server/README.md
- LM Studio API: https://lmstudio.ai/docs/python
- Current `settings.rs` - configuration structure
- Current `chat/custom_coordinator.rs` - tool execution flow
- Current `embeddings/client.rs` - embedding client
- Current `capabilities.rs` - model capability detection

---

## [M4] Future — Deferred

Features explicitly deferred with no current priority:

| Feature | Reason |
|---------|--------|
| AutoDream full daemon (4-phase) | After Sprach 2.0 |
| Cost Tracking | After Sprach 2.0 |
| Multi-scope Memory (team/private) | Not applicable — harness is not code-focused |
| Context Collapse | Observe, don't implement |
| VCR Testing | When CI is robust |
| Speculation | Indefinite deferral |
| Remote Agent | #15+ |
| ACP Agent Integration | B8 — after TUI decoupling (#16) |
| Team Memory Sync | Team use only |
| Remote Managed Settings | Enterprise |
| Worktree-aware sessions | Niche |
| Thinkback Marketplace | Too premature |
| Session Summary / Away Summary | Descarted — session continuation already works |

---

## Future Tools

### Document Processing
- PDF text extraction
- Document format conversion
- Batch processing

### Data Analysis
- CSV/JSON analysis
- Statistical tools
- ASCII visualization

### Code Tools
- Repository analysis
- Code quality checks
- Documentation generation

---

## Testing

### Test Coverage
- [ ] Unit tests for all commands
- [ ] Integration tests with mock Ollama
- [ ] Tool testing framework
- [ ] OCR/Translation testing

### CI/CD
- [x] GitHub Actions for testing
- [x] Release automation
- [ ] Documentation deployment

---

## See Also

- [Roadmap](./roadmap.md) — Milestone overview
- [Completed Features](./completed-features.md) — Implemented features
- [Implementation Status](./implementation-status.md) — Current snapshot