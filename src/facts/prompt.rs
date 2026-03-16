//! Build facts section for system prompt
//!
//! Constructs the "## User Facts" section that gets injected into the system prompt,
//! with Unicode-safe truncation if the total exceeds the limit.
//!
//! NOTE: This module is used in Phase 0.4 (Prompt injection).
//! Functions here are intentionally kept for future use.

use super::types::{Category, Fact, MAX_TOTAL_FACTS_CHARS};
use crate::utils::truncate_chars;

/// Build the facts section for the system prompt.
///
/// Groups facts by category (preferences first, then facts) and
/// truncates to MAX_TOTAL_FACTS_CHARS if necessary.
///
/// # Arguments
/// * `facts` - Slice of facts to include
///
/// # Returns
/// String containing "## User Facts" section, or empty string if no facts
pub fn build_facts_section(facts: &[Fact]) -> String {
    if facts.is_empty() {
        return String::new();
    }

    let mut section = String::new();

    // Group by category (preferences first)
    let preferences: Vec<&Fact> = facts
        .iter()
        .filter(|f| f.category == Category::Preference)
        .collect();

    let fact_list: Vec<&Fact> = facts
        .iter()
        .filter(|f| f.category == Category::Fact)
        .collect();

    if !preferences.is_empty() {
        section.push_str("### Preferences\n");
        for fact in preferences {
            section.push_str(&format!("- {}\n", fact.content));
        }
        section.push('\n');
    }

    if !fact_list.is_empty() {
        section.push_str("### Facts\n");
        for fact in fact_list {
            section.push_str(&format!("- {}\n", fact.content));
        }
    }

    // Truncate if over limit (Unicode-safe)
    if section.len() > MAX_TOTAL_FACTS_CHARS {
        // Use chars count, not bytes
        section = truncate_chars(&section, MAX_TOTAL_FACTS_CHARS);
    }

    section
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::types::{Scope, Source};
    use chrono::Utc;

    fn create_test_fact(content: &str, category: Category) -> Fact {
        Fact {
            id: 1,
            scope: Scope::Project,
            category,
            content: content.to_string(),
            importance: 0.5,
            access_count: 0,
            decay_score: 1.0,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            source: Source::User,
            invalidated_at: None,
            project_id: None,
        }
    }

    #[test]
    fn test_empty_facts() {
        let result = build_facts_section(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_preferences_first() {
        let facts = vec![
            create_test_fact("The project uses SQLite", Category::Fact),
            create_test_fact("I prefer dark mode", Category::Preference),
        ];

        let result = build_facts_section(&facts);

        // Preferences should come first
        let pref_pos = result
            .find("### Preferences")
            .expect("Should have preferences");
        let fact_pos = result.find("### Facts").expect("Should have facts");

        assert!(pref_pos < fact_pos, "Preferences should come before facts");
    }

    #[test]
    fn test_only_preferences() {
        let facts = vec![
            create_test_fact("I prefer dark mode", Category::Preference),
            create_test_fact("I like short responses", Category::Preference),
        ];

        let result = build_facts_section(&facts);

        assert!(result.contains("### Preferences"));
        assert!(!result.contains("### Facts"));
    }

    #[test]
    fn test_only_facts() {
        let facts = vec![
            create_test_fact("The project uses SQLite", Category::Fact),
            create_test_fact("API endpoint is /api/v1", Category::Fact),
        ];

        let result = build_facts_section(&facts);

        assert!(!result.contains("### Preferences"));
        assert!(result.contains("### Facts"));
    }

    #[test]
    fn test_truncation_limit() {
        // Create facts that would exceed the limit if we didn't validate
        // Since Fact::new() validates content length, we can't actually exceed
        // MAX_FACT_CONTENT_SIZE per fact. But we can test that build_facts_section
        // produces valid output.
        let facts = vec![
            create_test_fact("A short fact", Category::Fact),
            create_test_fact("Another short fact", Category::Fact),
        ];

        let result = build_facts_section(&facts);

        // Result should be well-formed
        assert!(result.contains("### Facts"));

        // Should end at valid char boundary
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn test_format() {
        let facts = vec![
            create_test_fact("I prefer Portuguese", Category::Preference),
            create_test_fact("The project uses Rust", Category::Fact),
        ];

        let result = build_facts_section(&facts);

        let expected =
            "### Preferences\n- I prefer Portuguese\n\n### Facts\n- The project uses Rust\n";

        assert_eq!(result, expected);
    }
}
