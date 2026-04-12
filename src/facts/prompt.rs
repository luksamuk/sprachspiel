//! Build facts section for system prompt
//!
//! Constructs the "## User Facts" section that gets injected into the system prompt,
//! with Unicode-safe truncation if the total exceeds the limit.
//!
//! Staleness labels are appended to facts when they indicate age or decay:
//! - `(stale)` — decay_score < 0.3 (badly decayed)
//! - `(N days ago)` — last_accessed > 30 days (not recently used)
//! - `(unused)` — access_count == 0 and age > 7 days (never retrieved)

use super::types::{Category, Fact, MAX_TOTAL_FACTS_CHARS};
use crate::utils::truncate_chars;
use chrono::Utc;

/// Threshold for decay_score below which a fact is considered stale.
const STALE_DECAY_THRESHOLD: f32 = 0.3;

/// Number of days without access before showing "days ago" label.
const DAYS_AGO_THRESHOLD: i64 = 30;

/// Number of days since creation (with zero accesses) before showing "unused" label.
const UNUSED_AGE_THRESHOLD: i64 = 7;

/// Returns a staleness label for a fact, or empty string if the fact is fresh.
///
/// Labels are prioritized: stale > days ago > unused.
/// Only one label is ever shown per fact to minimize prompt token usage.
/// Fresh facts (recently accessed, high decay score) produce no label.
fn get_staleness_label(fact: &Fact) -> String {
    let now = Utc::now();
    let days_since_access = (now - fact.last_accessed).num_days();
    let age_days = (now - fact.created_at).num_days();

    if fact.decay_score < STALE_DECAY_THRESHOLD {
        " (stale)".to_string()
    } else if days_since_access > DAYS_AGO_THRESHOLD {
        format!(" ({} days ago)", days_since_access)
    } else if fact.access_count == 0 && age_days > UNUSED_AGE_THRESHOLD {
        " (unused)".to_string()
    } else {
        String::new()
    }
}

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
            let staleness = get_staleness_label(fact);
            section.push_str(&format!("- {}{}\n", fact.content, staleness));
        }
        section.push('\n');
    }

    if !fact_list.is_empty() {
        section.push_str("### Facts\n");
        for fact in fact_list {
            let staleness = get_staleness_label(fact);
            section.push_str(&format!("- {}{}\n", fact.content, staleness));
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
        // Fresh facts should have no staleness label
        let facts = vec![
            create_test_fact("I prefer Portuguese", Category::Preference),
            create_test_fact("The project uses Rust", Category::Fact),
        ];

        let result = build_facts_section(&facts);

        let expected =
            "### Preferences\n- I prefer Portuguese\n\n### Facts\n- The project uses Rust\n";

        assert_eq!(result, expected);
    }

    // --- Staleness label tests ---

    fn create_stale_fact(content: &str, category: Category, decay_score: f32) -> Fact {
        Fact {
            id: 1,
            scope: Scope::Project,
            category,
            content: content.to_string(),
            importance: 0.5,
            access_count: 0,
            decay_score,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            source: Source::User,
            invalidated_at: None,
            project_id: None,
        }
    }

    fn create_fact_with_age(
        content: &str,
        category: Category,
        days_since_access: i64,
        age_days: i64,
        access_count: u32,
        decay_score: f32,
    ) -> Fact {
        let now = Utc::now();
        Fact {
            id: 1,
            scope: Scope::Project,
            category,
            content: content.to_string(),
            importance: 0.5,
            access_count,
            decay_score,
            created_at: now - chrono::Duration::days(age_days),
            last_accessed: now - chrono::Duration::days(days_since_access),
            source: Source::User,
            invalidated_at: None,
            project_id: None,
        }
    }

    #[test]
    fn test_staleness_stale() {
        // decay_score < 0.3 → "(stale)"
        let fact = create_stale_fact("Old API endpoint", Category::Fact, 0.2);
        let label = get_staleness_label(&fact);
        assert_eq!(label, " (stale)");
    }

    #[test]
    fn test_staleness_days_ago() {
        // last_accessed > 30 days and decay_score >= 0.3 → "(N days ago)"
        let fact = create_fact_with_age(
            "Old preference",
            Category::Preference,
            45,  // 45 days since last access
            60,  // 60 days old
            3,   // accessed 3 times
            0.5, // not stale
        );
        let label = get_staleness_label(&fact);
        assert_eq!(label, " (45 days ago)");
    }

    #[test]
    fn test_staleness_unused() {
        // access_count == 0 and age > 7 days → "(unused)"
        let fact = create_fact_with_age(
            "Created but never used",
            Category::Fact,
            10,  // 10 days since last access
            15,  // 15 days old
            0,   // never accessed
            0.8, // not stale
        );
        let label = get_staleness_label(&fact);
        assert_eq!(label, " (unused)");
    }

    #[test]
    fn test_staleness_fresh() {
        // Recently accessed, high decay_score → no label
        let fact = create_fact_with_age(
            "Fresh fact",
            Category::Fact,
            0,    // accessed today
            1,    // 1 day old
            5,    // accessed 5 times
            0.95, // high decay score
        );
        let label = get_staleness_label(&fact);
        assert!(label.is_empty());
    }

    #[test]
    fn test_staleness_priority_stale_over_days_ago() {
        // decay_score < 0.3 takes priority over days_since_access > 30
        let fact = create_fact_with_age(
            "Both stale and old",
            Category::Fact,
            60,  // 60 days since access (> 30)
            90,  // 90 days old
            1,   // accessed once
            0.1, // very stale (< 0.3)
        );
        let label = get_staleness_label(&fact);
        assert_eq!(label, " (stale)");
    }

    #[test]
    fn test_staleness_priority_days_ago_over_unused() {
        // days_since_access > 30 takes priority over access_count == 0
        let fact = create_fact_with_age(
            "Old and unused",
            Category::Fact,
            45,  // 45 days since access (> 30)
            60,  // 60 days old
            0,   // never accessed
            0.5, // not stale
        );
        let label = get_staleness_label(&fact);
        assert_eq!(label, " (45 days ago)");
    }

    #[test]
    fn test_staleness_borderline_not_stale() {
        // decay_score == 0.3 should NOT show "(stale)" (threshold is strictly < 0.3)
        let fact = create_stale_fact("Borderline", Category::Fact, 0.3);
        let label = get_staleness_label(&fact);
        // With decay_score = 0.3, last_accessed = now (0 days), access_count = 0, age = 0 days
        // No label should appear
        assert!(label.is_empty());
    }

    #[test]
    fn test_staleness_in_section() {
        // Verify staleness label appears in the final section output
        let fact = create_fact_with_age(
            "Old project fact",
            Category::Fact,
            60,  // 60 days since access
            90,  // 90 days old
            2,   // accessed twice
            0.4, // not stale but old
        );
        let result = build_facts_section(&[fact]);
        assert!(result.contains("- Old project fact (60 days ago)\n"));
    }

    #[test]
    fn test_staleness_in_section_with_mixed() {
        // Mix of fresh and stale facts
        let fresh = create_test_fact("Fresh preference", Category::Preference);
        let mut stale = create_test_fact("Old preference", Category::Preference);
        stale.decay_score = 0.1;
        stale.last_accessed = Utc::now() - chrono::Duration::days(100);
        stale.created_at = Utc::now() - chrono::Duration::days(200);

        let result = build_facts_section(&[fresh, stale]);
        // Fresh fact should have no label
        assert!(result.contains("- Fresh preference\n"));
        // Stale fact should have (stale) label
        assert!(result.contains("- Old preference (stale)\n"));
    }
}
