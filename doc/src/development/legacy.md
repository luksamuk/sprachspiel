# Legacy Documentation

This section contains documentation that has been superseded by newer, more comprehensive documents.

## What is Legacy Documentation?

Legacy documentation represents earlier design iterations that have been replaced or consolidated. These documents are kept for:

- **Historical reference** — Understanding design evolution
- **Research context** — Preserving decision rationale
- **Transition period** — Allowing gradual migration to new docs

## Current Status

| Legacy Document | Replaced By | Reason |
|-----------------|-------------|--------|
| [Context Composition Design](./context_composition_design.md) | [Memory Architecture](./memory-architecture.md) + [Context Anatomy](./context-anatomy.md) | Consolidated into unified memory architecture |
| [Retrieval Design](./retrieval-design.md) | [Memory Architecture](./memory-architecture.md) | Conversation memory is now documented as Layer 2 |

## When to Use

- **Use new documents** for current implementation details and architecture understanding
- **Use legacy documents** for historical context or when referenced by older issues/PRs

## Migration Notes

If you're updating links or references:

1. **Context composition** → Link to [Memory Architecture](./memory-architecture.md) for overview, or [Context Anatomy](./context-anatomy.md) for prompt assembly details
2. **Retrieval design** → Link to [Memory Architecture](./memory-architecture.md) Layer 2 section

## Archived Documents

Documents may be removed from this section after 6 months if no longer referenced.