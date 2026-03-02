# /context Command

Show context metrics and token usage for the current chat session.

## Synopsis

```
/context
/ctx
```

## Description

The `/context` command displays detailed information about token usage in the current chat session, including:

- **System prompt tokens**: Estimated tokens used by the system prompt
- **Tool definitions tokens**: Estimated tokens for tool definitions (when tools are enabled)
- **Conversation tokens**: Estimated tokens for the conversation history
- **Context utilization**: Percentage of context window used

This information helps you understand how much of the model's context window is being used and plan for context management.

## Output Format

```
Context Information:
  Model:          llama3.1:8b (32K context)

  Token Breakdown:
    System prompt:    ~890 tokens
    Tool definitions: ~450 tokens (23 tools)
    Conversation:     ~1,250 tokens (15 messages)
    ────────────────────────────────────────────
    Total used:       ~2,590 tokens
    Available:        ~29,506 tokens
    Utilization:      8.1%

  Session:
    Total:           15 messages
```

## Token Estimation

Token counts are estimates based on:

- **Text estimation**: ~0.75 words per token (GPT-style)
- **Message overhead**: ~4 tokens per message (role markers, formatting)
- **Code estimation**: ~0.5 tokens per character (higher density)

Actual token usage may vary depending on the model's tokenizer.

## Related Commands

- `/info` - Show session information including model and settings
- `/compact` - Compact conversation history to reduce token usage

## See Also

- [Chat Mode](./chat.md) - Interactive chat documentation
- [Context Management](../development/context_management_research.md) - Research on context handling