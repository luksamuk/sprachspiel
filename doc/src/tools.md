# Available Tools

Ask-AI provides 14 tools that enhance queries with real-time data from external sources. Tools are automatically enabled for capable models.

## Tool Overview

| Category | Count | Source | Status |
|----------|-------|--------|--------|
| Pokémon | 8 | PokéAPI | ✅ Working |
| Weather | 3 | Open-Meteo | ✅ Working |
| Web Search | 3 | DuckDuckGo | ⚠️ Currently blocked |

## Pokémon Tools (8)

Powered by [PokéAPI](https://pokeapi.co/).

### fetch_pokemon

Get comprehensive Pokémon data including stats, abilities, moves, and evolution chain.

```
Function: fetch_pokemon
Args: name (string)
Example: fetch_pokemon(name: "pikachu")
```

### fetch_pokemon_basic

Get basic Pokémon information (types, height, weight, abilities).

```
Function: fetch_pokemon_basic
Args: name (string)
Example: fetch_pokemon_basic(name: "charizard")
```

### fetch_pokemon_stats

Get base stats (HP, Attack, Defense, etc.).

```
Function: fetch_pokemon_stats
Args: name (string)
Example: fetch_pokemon_stats(name: "mewtwo")
```

### fetch_pokemon_moves

Get learnable moves with optional limit.

```
Function: fetch_pokemon_moves
Args: name (string), limit (optional integer)
Example: fetch_pokemon_moves(name: "pikachu", limit: 10)
```

### fetch_pokemon_evolution

Get evolution chain information.

```
Function: fetch_pokemon_evolution
Args: name (string)
Example: fetch_pokemon_evolution(name: "eevee")
```

### fetch_ability_details

Get ability descriptions and which Pokémon have it.

```
Function: fetch_ability_details
Args: ability (string)
Example: fetch_ability_details(ability: "lightning-rod")
```

### fetch_type_effectiveness

Get type weaknesses, resistances, and immunities.

```
Function: fetch_type_effectiveness
Args: type_name (string)
Example: fetch_type_effectiveness(type_name: "electric")
```

### fetch_move_details

Get move information (power, accuracy, type, effect).

```
Function: fetch_move_details
Args: move (string)
Example: fetch_move_details(move: "thunderbolt")
```

## Weather Tools (3)

Powered by [Open-Meteo](https://open-meteo.com/) (free, no API key required).

### get_weather

Get current weather conditions and 3-day forecast.

```
Function: get_weather
Args: city (string), country (optional string)
Example: get_weather(city: "Tokyo", country: "Japan")
```

### get_current_weather

Get current conditions only (simpler response).

```
Function: get_current_weather
Args: city (string), country (optional string)
Example: get_current_weather(city: "London")
```

### get_weather_forecast

Get extended 7-day forecast.

```
Function: get_weather_forecast
Args: city (string), country (optional string), days (optional integer)
Example: get_weather_forecast(city: "Paris", days: 7)
```

## Web Search Tools (3)

⚠️ **Currently blocked by DuckDuckGo CAPTCHA**. Alternative needed.

Powered by DuckDuckGo Lite.

### web_search

General web search with results.

```
Function: web_search
Args: query (string), max_results (optional integer)
Example: web_search(query: "Rust programming language", max_results: 5)
```

### web_search_news

News-specific search.

```
Function: web_search_news
Args: query (string), max_results (optional integer)
Example: web_search_news(query: "technology", max_results: 5)
```

### web_instant_answer

Quick facts and definitions.

```
Function: web_instant_answer
Args: query (string)
Example: web_instant_answer(query: "What is photosynthesis?")
```

## Using Tools

### Automatic Tool Detection

Tools are automatically enabled for capable models:

```bash
# Tools auto-enabled for mistral-small
ask-ai -m mistral-small "Tell me about Pikachu"

# Tools auto-enabled for gpt-oss
ask-ai -m gpt-oss "What's the weather in Tokyo?"
```

### Force Enable Tools

Force tools on any model:

```bash
ask-ai --tools "Tell me about Pikachu"
```

### Tool User Prompt

Use enhanced prompt for better tool selection:

```bash
ask-ai -p tool_user "What's the weather?"
```

## Tool Examples

### Pokémon Queries

```bash
# Comprehensive data
ask-ai "Tell me everything about Charizard"

# Specific information
ask-ai "What are Pikachu's stats?"
ask-ai "Show me Eevee's evolution chain"
ask-ai "What type is super effective against Water?"

# Compare Pokémon
ask-ai "Compare Blastoise and Charizard stats"

# Move information
ask-ai "Tell me about Thunderbolt"
ask-ai "What moves can Pikachu learn?"
```

### Weather Queries

```bash
# Current weather
ask-ai "What's the weather in Tokyo?"

# Forecast
ask-ai "Weather forecast for Paris"

# Specific queries
ask-ai "Is it raining in London?"
ask-ai "What's the temperature in New York?"

# With country
ask-ai "Weather in Sydney, Australia"
```

### Web Search (Currently Blocked)

```bash
# Note: Web search is currently blocked by DuckDuckGo CAPTCHA

# General search
ask-ai "Search for Rust async patterns"

# News
ask-ai "Latest technology news"

# Quick facts
ask-ai "What is quantum computing?"
```

## Tool Selection

The model automatically selects appropriate tools based on your query:

```mermaid
graph TD
    A[User Query] --> B{Contains Pokémon?}
    B -->|Yes> C[Use Pokémon tools]
    B -->|No> D{Contains Weather?}
    D -->|Yes> E[Use Weather tools]
    D -->|No> F{Needs Web Search?}
    F -->|Yes> G[Use Web Search tools]
    F -->|No> H[Answer directly]
```

## Known Issues

### DuckDuckGo Web Search Blocked

**Status**: ⚠️ Currently blocked

**Problem**: DuckDuckGo Lite endpoint blocks automated requests with CAPTCHA

**Error**: "Unfortunately, bots use DuckDuckGo too"

**Workaround**: None currently

**Solution**: Alternative search provider needed
- SerpAPI (paid)
- Bing API (paid)
- Searx (self-hosted)
- Local LLM web search

### GPT-OSS Tool Calling

**Status**: ⚠️ Under investigation

**Problem**: Models like `gpt-oss:120b` may fail with `invalid character '<'` error

**Cause**: Likely HTML entity encoding issue

**Workaround**: Use other models for tool calls:
- `mistral-small`
- `pepe`
- `lfm`

## Tool Capable Models

| Model | Tools | Notes |
|-------|-------|-------|
| mistral-small | ✅ | Best for tools |
| gpt-oss | ⚠️ | May have issues |
| qwen3-coder | ✅ | Code + tools |

## Debug Mode

See tool calls in debug mode:

```bash
ask-ai -d "Tell me about Pikachu"

# Output includes:
# - Tool calls with arguments
# - Tool results
# - Model configuration
```

## Best Practices

1. **Use capable models** - mistral-small works best
2. **Be specific** - "Pikachu stats" vs "Tell me about Pikachu"
3. **Use tool_user prompt** - For complex queries
4. **Check debug mode** - If tools aren't working
5. **Weather doesn't need API key** - Always available

## Future Tools

Planned additions:

- **File Operations**: Read files, list directories
- **System Tools**: Execute commands (configurable whitelist)
- **Web Scraping**: Extract content from URLs

## See Also

- [query](./commands/query.md) - Using tools with queries
- [Models](./models.md) - Tool-capable models
- [Prompts](./prompts.md) - Tool user prompt mode
