//! Few-shot examples for tool usage
//!
//! Uses ReAct-style trajectories (User → Action → Response) to demonstrate
//! proper tool usage. These examples help the model understand when and how
//! to call each tool.

/// Few-shot examples for tool usage using ReAct-style format
///
/// Each example follows the pattern:
/// 1. User question
/// 2. Action (tool call)
/// 3. Response (answer based on tool results)
///
/// The examples cover different tool categories:
/// - Weather tools
/// - Pokémon tools
/// - Web search tools
/// - File tools
pub const TOOL_EXAMPLES: &str = r#"### EXAMPLES

User: What's the weather in Tokyo?
Action: get_weather(location="Tokyo")
Response: **Tokyo Weather**: Currently 23°C, partly cloudy with light winds. The forecast shows clear skies for the next few days.

---

User: Compare Pikachu and Charizard base stats
Action: fetch_pokemon_stats(pokemon_name="Pikachu")
Action: fetch_pokemon_stats(pokemon_name="Charizard")
Response: **Stat Comparison**:

| Stat | Pikachu | Charizard |
|------|---------|-----------|
| HP | 35 | 78 |
| Attack | 55 | 84 |
| Defense | 40 | 78 |
| Sp. Atk | 50 | 109 |
| Sp. Def | 50 | 85 |
| Speed | 90 | 100 |

Charizard has significantly higher stats overall, especially in Special Attack.

---

User: What is quantum computing?
Action: web_search(query="quantum computing explanation")
Response: **Quantum Computing**: A computing paradigm that uses quantum mechanics principles like superposition and entanglement to process information. Unlike classical computers that use bits (0 or 1), quantum computers use qubits that can exist in multiple states simultaneously, enabling parallel computation for certain problems.

---

User: Show me the project structure
Action: list_directory(path=".")
Response: **Project Structure**:
```
project/
├── src/ (156 KB)
│   ├── main.rs (45 KB)
│   ├── lib.rs (3 KB)
│   └── utils.rs (12 KB)
├── tests/ (28 KB)
├── Cargo.toml (2 KB)
└── README.md (8 KB)
```

---

User: What type is Pikachu weak against?
Action: fetch_type_effectiveness(type_name="electric")
Response: **Electric Type Weaknesses**: Electric-type Pokémon are weak only to Ground-type moves (2x damage). They are resistant to Electric, Flying, and Steel (0.5x damage), and immune to Steel-type moves when combined with other factors.

---

User: Read lines 10-20 of config.yaml
Action: count_lines(path="config.yaml")
Action: read_file_segment(path="config.yaml", start_line="10", num_lines="11")
Response:
```yaml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4
database:
  url: "postgres://localhost/mydb"
  pool_size: 10
```
"#;

/// Get the tool examples string
pub fn get_tool_examples() -> &'static str {
    TOOL_EXAMPLES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_examples_contain_required_patterns() {
        // Should have User/Action/Response pattern (ReAct format)
        assert!(TOOL_EXAMPLES.contains("User:"), "Missing User: pattern");
        assert!(TOOL_EXAMPLES.contains("Action:"), "Missing Action: pattern");
        assert!(
            TOOL_EXAMPLES.contains("Response:"),
            "Missing Response: pattern"
        );
    }

    #[test]
    fn test_examples_count() {
        // Count by --- separators
        let count = TOOL_EXAMPLES.matches("---").count();
        assert!(count >= 5, "Expected at least 5 examples, found {}", count);
    }

    #[test]
    fn test_examples_cover_tool_categories() {
        // Weather example
        assert!(
            TOOL_EXAMPLES.contains("get_weather"),
            "Missing weather example"
        );

        // Pokémon example
        assert!(
            TOOL_EXAMPLES.contains("fetch_pokemon"),
            "Missing Pokémon example"
        );

        // Web search example
        assert!(
            TOOL_EXAMPLES.contains("web_search"),
            "Missing web search example"
        );

        // File example
        assert!(
            TOOL_EXAMPLES.contains("list_directory"),
            "Missing file example"
        );
    }
}
