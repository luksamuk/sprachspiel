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
/// - File tools
pub const TOOL_EXAMPLES: &str = r#"### EXAMPLES

User: What's the weather in Tokyo?
Action: get_weather(location="Tokyo")
Response: **Tokyo Weather**: Currently 23°C, partly cloudy with light winds. The forecast shows clear skies for the next few days.

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
        // Count by --- separators; the last example has no trailing one.
        let count = TOOL_EXAMPLES.matches("---").count() + 1;
        assert!(count >= 3, "Expected at least 3 examples, found {}", count);
    }

    #[test]
    fn test_examples_cover_tool_categories() {
        // Weather example
        assert!(
            TOOL_EXAMPLES.contains("get_weather"),
            "Missing weather example"
        );

        // File example
        assert!(
            TOOL_EXAMPLES.contains("list_directory"),
            "Missing file example"
        );

    }
}
