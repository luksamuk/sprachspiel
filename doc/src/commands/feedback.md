# Feedback Commands

Record feedback on assistant messages to influence future search ranking and retrieval.

## Synopsis

```
/feedback <good|bad|correction:text> [msg:id]
/fb <good|bad|correction:text> [msg:id]
/fg                                          # Shortcut: /feedback good
```

## Description

The feedback system lets you mark assistant messages as good, bad, or corrected. Feedback signals are stored persistently and affect how messages are ranked during semantic search — positively-feedbacked messages rank higher, negatively-feedbacked messages rank lower.

This is a **harness-only** system (ADR-001): feedback adjusts retrieval scores at inference time, it does not fine-tune the model.

## Subcommands

| Subcommand | Effect | Importance Adjustment |
|------------|--------|-----------------------|
| `good` | Mark message as helpful | +0.05 |
| `bad` | Mark message as unhelpful | -0.10 |
| `correction:<text>` | Provide a correction with text | +0.05 (with correction text stored) |

## Targeting Messages

By default, feedback applies to the **most recent assistant message**. To target a specific message:

```
/feedback good msg:42
/feedback bad msg:15
/feedback correction:The capital is Canberra msg:7
```

The `msg:N` format uses the same ID visible in `/context` output.

## Shortcuts

| Shortcut | Equivalent |
|----------|------------|
| `/fb good` | `/feedback good` |
| `/fb bad` | `/feedback bad` |
| `/fb correction:text` | `/feedback correction:text` |
| `/fg` | `/feedback good` |

## How Feedback Affects Search

When you search conversation history (`/search`), feedback signals are used to adjust ranking:

1. **Decay computation:** Each signal's weight decays over time using the half-life formula:
   - Good signals: 30-day half-life
   - Bad signals: 7-day half-life
   - Correction signals: 14-day half-life

2. **Source weighting:** User feedback (weight 1.0) influences ranking more than LLM self-feedback (weight 0.3, ADR-004).

3. **Score adjustment:** The search score is multiplied by a factor clamped between 0.1 and 3.0 (ADR-006):
   - Maximum positive feedback: 3× score amplification
   - Maximum negative feedback: 90% suppression (cannot eliminate results entirely)

## LLM Integration

The LLM can also submit feedback autonomously using the `feedback_submit` tool:

- LLM feedback is weighted at 30% (ADR-004) to counter self-approval bias
- The tool is available when `[feedback] enabled = true` (default) in config.toml

## Configuration

Feedback behavior can be configured in `config.toml` under the `[feedback]` section:

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `true` | Enable feedback system |
| `implicit_capture` | `true` | Allow LLM self-feedback |
| `llm_feedback_weight` | `0.3` | Weight for LLM-originated feedback |
| `decay_half_life_good` | `30.0` | Half-life for Good signals (days) |
| `decay_half_life_bad` | `7.0` | Half-life for Bad signals (days) |
| `decay_half_life_correction` | `14.0` | Half-life for Correction signals (days) |
| `content_decay` | `true` | Enable content retention decay |
| `access_reinforcement` | `true` | Reinforce retention on retrieval |
| `access_reinforcement_boost` | `0.001` | Importance boost per access |
| `content_prune_threshold` | `0.05` | Retention threshold for pruning |

## Error Cases

| Input | Error |
|-------|-------|
| `/feedback` (no subcommand) | `Usage: /feedback <good|bad|correction:text> [msg:id]` |
| `/feedback msg:abc good` | `Invalid message ID 'abc'. Use msg:<number> (e.g., msg:42).` |
| `/feedback correction:` | `Correction requires text. Usage: /feedback correction:<text>` |
| `/feedback good` (anonymous mode) | `Error: Cannot give feedback in anonymous mode.` |
| `/feedback good` (no assistant message yet) | `No assistant message to give feedback on.` |

## See Also

- [Chat Commands](./chat.md) - Full list of chat commands
- [Feedback Architecture](../development/feedback-architecture.md) - Technical design and formulas