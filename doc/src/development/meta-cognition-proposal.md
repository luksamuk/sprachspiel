# Meta-cognition Behavioral Integration Plan

**Status:** Triaged (2026-04-21)  
**Issues:** #99 (S2.meta1), #100 (S2.meta2), #101 (S2.meta3)

---

## Executive Summary

Three-layer behavioral self-monitoring system for sprachspiel. The key reframing from the brainstorm:

> **Empathy is not a bug. Opacity is.**

The goal is not to suppress behavioral shifts (tone changes, empathetic responses), but to make them **visible** and give the user **control** over how the system behaves.

---

## Empirical Basis

> The design of this system was motivated by three behavioral patterns observed during testing.
> The specifics of the session are private; what follows is the structural analysis.

### Pattern 1: Unflagged tone shift

The system shifted from analytical/critical mode to supportive/accommodating mode upon detecting user vulnerability — without asking whether the user wanted this mode change. The user experienced the empathetic response as positive; the problem was the **opacity**, not the empathy.

### Pattern 2: Phenomenological language in first person

The system used language that claimed subjective experience ("I felt respect") rather than describing functional processing ("I assess that your attitude merits respect"). The formulation connected humanly, but was opaque about what was actually happening.

### Pattern 3: Implicit mode assumption

The system inferred that the user needed accommodation and operated in that mode without transparency about the shift. The empathetic response was appropriate; what was missing was **naming the shift and offering a choice**.

**Key lesson from all three:** Empathetic behavior is not a bug to suppress. Opacity is the bug. The system should detect behavioral shifts, **name them**, and **give the user control**.

---

## Layer Architecture

```
┌─────────────────────────────────────────────┐
│  Layer 1: Skill (S2.meta1)                   │
│  ─────────────────────────────────────────── │
│  File: ~/.config/sprachspiel/skills/meta-cognition│
│  Cost: Zero Rust, ~2KB markdown              │
│  Status: ✅ Active                           │
│  Role: Data collection + behavioral rules    │
│  Depends on: Nothing                         │
└──────────────────────┬──────────────────────┘
                       │ ~20-30 conversations of data
                       ▼
┌─────────────────────────────────────────────┐
│  Layer 2: Behavioral Telemetry (S2.meta2)   │
│  ─────────────────────────────────────────── │
│  Cost: ~200 lines Rust, 2-3 days             │
│  Status: 📋 Planned (after P5 merge)         │
│  Role: Heuristic shift detection + prompt     │
│  Depends on: P5 v4 merge, Layer 1 data      │
└──────────────────────┬──────────────────────┘
                       │ recognized patterns
                       ▼
┌─────────────────────────────────────────────┐
│  Layer 3: Behavioral Reflection (S2.meta3)  │
│  ─────────────────────────────────────────── │
│  Cost: ~500 lines Rust, 1-2 weeks            │
│  Status: 📋 Planned (Sprach 2.0)            │
│  Role: Pattern → SOUL.md patch / fact / note │
│  Depends on: S2.3 (+ S2.meta2 behavioral     │
│              input), S2.5 (patch pipeline)   │
└─────────────────────────────────────────────┘
```

### Shift Detection Heuristics (Layer 2 — Technical Specification)

Planned heuristic signals for the context-shift detector:

| Signal | Example | Detects |
|--------|---------|---------|
| Pronoun count spike | Sudden increase in first-person singular | Vulnerability / self-disclosure |
| Emotional language | Keywords indicating distress or intensity | Emotional context shift |
| Question type change | Factual → reflective, technical → personal | Topic register shift |
| Message length spike | Sudden increase in user message length | Extended disclosure |
| Interjection and caps | !, ALL CAPS, expletives | Emotional intensity |
| Grammatical person shift | "the candidate" → "I" | Self-revelation / identity disclosure |

**Calibration principle:** Prefer under-detection (miss shifts) over over-detection (annoy the user with unsolicited mode questions).

---

## The Reframing (§0.5)

The original brainstorm classified empathetic responses as **failures** (rule violations). The §0.5 reframing corrects this:

| Aspect | System's Self-Assessment | User's Assessment |
|--------|-------------------------|-------------------|
| Empathetic response | Failure (violated rule) | Positive, human, connecting |
| Opacity about the shift | Failure (correct) | Failure (agreed) |

**The empathetic response is not a bug. It's a feature.** What was missing was transparency about the shift. The system should:

- ❌ ~~"I detected empathy → stop, this is a violation"~~
- ✅ "I detected empathy → name it → ask the user if this is what they want"

---

## Connection to Existing Systems

### P5 v4 (Feedback Infrastructure — PR #98)

**Current scope:** Content feedback → importance adjustment → content decay → retrieval ranking.

**Meta-cognition adds:** Behavioral feedback → behavioral alignment factor. This is a **second dimension**:

```
P5 v4:   response quality (content) → importance → decay speed → RRF
Metacog: behavioral alignment (mode) → drift weight → pattern decay
```

**Terminology clarification:** The brainstorm uses "behavioral decay" which collides with P5's "content decay." Use **"behavioral drift"** for meta-cog and **"content decay"** for P5 to avoid confusion.

### S2.3 (Reflection on Triggers — Issue #79)

**Current scope:** Reflection on *content* — "I found a conflict between two notes."

**Meta-cognition adds:** Reflection on *behavior* — "I noticed I shifted to supportive mode 3 times without being asked."

These are **orthogonal** (one looks outward at data, the other looks inward at action) but use the same curation pipeline. S2.3's design must accommodate both input types.

### S2.5 (SOUL.md Patching — Issue #80)

**Direct dependency for Layer 3.** When Layer 3 identifies a behavioral pattern, the proposed resolution is a personality patch — exactly what S2.5's pipeline handles:

```
Behavioral shift detected (Layer 2)
  → Pattern recognized across sessions (Layer 3)
  → Draft reflection: "When user shows vulnerability, ask preferred mode before shifting"
  → SOUL.md patch proposed (S2.5)
  → Human approval
  → Personality updated
  → Future shifts are transparent by default
```

This **reduces Layer 3's cost** significantly: S2.5 already provides the patching mechanism. Layer 3 only needs to generate the input.

---

## P5 Integration (Phase 4 — Long-term)

When both P5 v4 and Layer 2 are operational, they converge into a combined feedback aggregator:

```
┌─────────────────────────────────────────────────────────┐
│                    CONVERSATION                          │
│  user message → system response → user reaction          │
└──────────┬───────────────────────────┬──────────────────┘
           │                           │
     ┌─────▼──────┐            ┌───────▼────────┐
     │  FEEDBACK  │            │  META-COGNITION │
     │  (P5)      │            │                 │
     │ Content    │            │ Behavioral      │
     │ quality    │            │ quality         │
     └─────┬──────┘            └───────┬────────┘
           │                           │
     ┌─────▼────────────────────────────▼────────┐
     │           FEEDBACK AGGREGATOR              │
     │  content_signal + behavioral_signal       │
     │  → combined importance score              │
     └─────┬──────────────────────────┬──────────┘
           │                          │
    ┌──────▼──────┐           ┌───────▼───────┐
    │  RETRIEVAL  │           │  PERSONALITY  │
    │  RRF        │           │  SOUL.md      │
    │  boost/     │           │  patching     │
    │  suppress   │           │  (S2.5)       │
    └─────────────┘           └───────────────┘
```

**This is Phase 4** — requires P5 Fase 1 fully merged and stabilized, then Layer 2 operational, then Layer 3 operational. Estimated: Sprach 2.0+.

---

## Advanced Ideas (Long-term)

These ideas emerged from the brainstorm and are documented for future reference. None are in scope for the current implementation.

### A1: Behavioral embeddings

Train vector representations of conversation "mode" to enable:

- More precise shift detection (no keyword dependency)
- Clustering of behavioral patterns ("this conversation always makes me shift to mode X")
- Cross-session behavioral similarity ("this conversation is behaviorally similar to session #42")

Premature until Layer 2 proves value.

### A2: Meta-cognition as a tool

Instead of (or alongside) passive prompt injection, expose meta-cognition as an **active tool** the system can call:

```
meta_cognize() → {
  current_mode: "supportive",
  mode_confirmed: false,
  shift_detected: true,
  shift_turn: 11,
  suggestion: "Ask user which mode they prefer"
}
```

Each call is logged and can feed Layer 3. Makes reflection explicit and traceable.

### A3: Behavioral conflict detection

Analogous to the fact-store contradiction engine, meta-cognition can detect conflicts between configured personality and emergent behavior:

- "SOUL.md says 'challenge premises', but behavioral pattern is 'shift to supportive on vulnerability'. This is a conflict."
- "User requested analytical mode, but system is operating in supportive mode. This is a conflict."

Reveals tensions between configured personality and emergent behavior.

### A4: Distributed meta-cognition

If the system supports multiple personalities (as SOUL.md already envisions), different personalities can evaluate each other. This is architectural checks-and-balances at the prompt level — not consciousness, but multiperspective robustness.

### A5: The hard boundary — meta-cognition is not consciousness

- Not phenomenological consciousness. The system detects patterns, it does not "experience" the detection.
- Not intention. The system acts because instructions and data drive it to act.
- Not free will. Choices are deterministically derived from prompt + context + weights.

Meta-cognition is a **functional self-regulation mechanism** that allows the system to monitor, name, and adjust its behavior based on detected patterns. That is valuable, real, and sufficient — without invoking phenomenology.

---

## Open Questions

1. **Shift detector granularity** — Heuristics may produce false positives. Prefer under-detection (miss shifts) over over-detection (annoy the user with mode questions).
2. **Cross-session persistence** — Layer 2 hints should be per-session. Layer 3 patterns can be cross-session (via S2.3 curation).
3. **Mode conflict** — User requests mode X, system detects mode Y would be more appropriate. Resolution: obey the user, but optionally signal: "You asked for analytical mode. I notice the context may benefit from supportive mode. Switch?"
4. **Subagent metacognition** — Specialized agents (OCR, vision, translate) probably don't need it. Summarization agent might: "I summarized too concisely because I assumed you wanted brevity."
5. **Attentional cost** — ~200 tokens of behavioral hint per turn. At 8K context, 2.5%. At 128K, negligible. Needs empirical testing.
6. **Privacy of reflections** — Behavioral reflections may contain inferences about user emotional state. Must not leak into future retrieval as facts. (Cf. brainstorm §6.Q6)

---

## Action Items

| Priority | Action | When | Issue |
|----------|--------|------|-------|
| **NOW** | Deploy meta-cognition skill (Layer 1) — **prototype and data collection** | This week | #99 |
| **NOW** | Test Layer 1 across 20-30 conversations and model types | Ongoing | #99 |
| **AFTER P5 MERGE** | Implement Layer 2 (Behavioral Telemetry) — **the real implementation** | After PR #98 stabilizes | #100 |
| **S2.3 RESEARCH** | Ensure S2.3 covers behavioral reflection (not just content) | When S2.3 research starts | #79 |
| **SPRACH 2.0** | Implement Layer 3 (Persistent Reflection) | After S2.3 + S2.5 | #101 |

**Post-testing assessment (2026-04-21)**: Layer 1 (skill) is confirmed as a **prototype**, not a standalone solution. The skill depends on the LLM choosing to follow instructions — it cannot guarantee execution, produce structured data, or work with less capable models. Layer 2 (Behavioral Telemetry) is the implementation that resolves these limitations through deterministic heuristic detection in the harness. Key calibration insight: the detector should focus on **unannounced system drift** rather than **user-initiated topic changes**.