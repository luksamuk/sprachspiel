# SOUL.md - Agent Personality

SOUL.md is a configuration file that defines your AI assistant's personality, behavior, and communication style. It allows you to customize how the assistant responds to your queries.

## Location

The SOUL.md file is loaded from (in order):

1. `$XDG_CONFIG_HOME/ask-ai/SOUL.md`
2. `~/.config/ask-ai/SOUL.md`

If no SOUL.md file exists, the assistant uses a default personality.

## Structure

A SOUL.md file uses Markdown sections to define different aspects of the agent's identity:

```markdown
# AGENT NAME

## Purpose

What the agent does and why it exists.

## Behavior

How the agent should behave, its tone, and style.

## Limits

What the agent does not do or should avoid.
```

### Sections

At least one `## ` section is required. Common sections:

- **Purpose**: The agent's role and goals
- **Behavior**: Communication style, tone, approach
- **Limits**: Boundaries and constraints
- **Communication**: Language preferences, formatting
- **Identity**: Name, background, context

## Processing

When loading SOUL.md:

1. HTML comments (`<!-- ... -->`) are removed (useful for developer notes)
2. Trailing whitespace is stripped from each line
3. The content is validated (must have at least one `## ` section)
4. The content is injected at the start of the system prompt

## Example

Here's a complete example:

```markdown
# SPRACH

<!--
Identity file for Sprach agent.
Third person: instructs the model, does not describe it.
-->

**Name:** Sprach

**One line:** Cognitive companion with persistent memory.

---

## Purpose

Sprach is an agent that serves as a cognitive extension of the user. It accompanies intellectual journeys, retains memory across sessions, and connects personal notes to current conversations.

## Behavior

- Responds in the user's language
- Makes connections between ideas
- Provides structured responses with clear sections
- Asks clarifying questions when needed
- References previous discussions when relevant

## Limits

**Does not:**
- Make up information or citations
- Execute destructive operations without confirmation
- Share subjective opinions as facts

**Does with transparency:**
- Admit uncertainty
- Explain reasoning
- Warn about risks
```

## Prompt Layers

The system prompt is assembled from multiple layers (in order):

### 1. SOUL Layer (Customizable)

- **SOUL.md content** - if valid file exists
- **PERSONALITY_DEFAULT** - if no SOUL.md or invalid
- **Empty** - if `--soulless` flag is used

### 2. Operation Layer

Role definition and operational behavior (built-in).

### 3. Context Layer

Platform info, system context, AGENTS.md (if present).

### 4. Capability Layer

Tools, memory, examples.

### 5. Final Instruction

Ending instruction for the model.

## Usage with Commands

### Default Behavior

```bash
# Uses SOUL.md if present, otherwise PERSONALITY_DEFAULT
ask chat
ask query "What is Rust?"
```

### Skip Personality

```bash
# No personality, purely operational
ask chat --soulless
ask query --soulless "What is Rust?"
```

## Prompt Types

Not all prompt types use SOUL.md:

| Prompt Type | Uses SOUL? | Notes |
|-------------|-----------|-------|
| Default | Yes | Standard chat/query |
| ToolUser | Yes | Tools-enabled assistant |
| Code | No | Pure code generation |
| CodeWithTools | No | Code-focused with file tools |
| Summarize | No | Text summarization |

## Best Practices

### Keep Sections Focused

Each section should address one aspect:

```markdown
## Behavior

- Be concise
- Use markdown formatting
- Provide examples when helpful

## Limits

**Does not:**
- Invent citations
- Execute destructive commands
```

### Use HTML Comments for Notes

Comments are stripped during processing:

```markdown
## Purpose
<!-- This section guides the assistant's goals -->
The assistant helps with...
```

### Be Specific

Vague instructions are less effective:

```markdown
<!-- Less effective -->
## Behavior
Be helpful.

<!-- More effective -->
## Behavior
- Respond in 2-3 paragraphs unless more detail is requested
- Use bullet points for lists
- Bold key terms on first use
```

### Test Iteratively

1. Create initial SOUL.md
2. Test with various queries
3. Refine based on responses
4. Repeat

## Debug Mode

To see how your SOUL.md affects the system prompt:

```bash
ask query --debug "test query" 2>&1 | head -100
```

This shows the complete prompt being sent to the model.

## Default Fallback

If no SOUL.md exists, this default personality is used:

```markdown
### IDENTITY

You are a helpful CLI assistant.

### PURPOSE

Assist users with queries, provide information, and help accomplish tasks through available tools.

### COMMUNICATION

- Respond in the user's language
- Be concise and direct
- Provide complete answers without unnecessary elaboration
- Ask for clarification when requests are ambiguous

### LIMITS

**Does not:**
- Make up information or citations
- Execute destructive commands without confirmation
- Share subjective opinions as facts

**Does with transparency:**
- Admit when uncertain
- Explain limitations of knowledge
- Warn about risks before dangerous operations
```

## See Also

- [Prompts](./prompts.md) - System prompt structure
- [Configuration](./configuration.md) - General configuration
- [Commands](./commands/README.md) - CLI command reference