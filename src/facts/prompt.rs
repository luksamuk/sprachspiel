//! Build facts section for system prompt
//!
//! Constructs the "## User Facts" section that gets injected into the system prompt,
//! with Unicode-safe truncation if the total exceeds the limit.
//!
//! # Third-Person Normalization (ADR-E4)
//!
//! Facts stored in the database preserve the user's original wording (e.g.,
//! "I prefer dark mode"). When rendered in the system prompt, first-person
//! pronouns are converted to third-person attribution (e.g., "User prefers
//! dark mode") to prevent the LLM from confusing user facts with its own
//! identity. This follows the pattern used by Mem0 and Claude Code, which
//! store facts in third-person or with explicit attribution blocks.
//!
//! Staleness labels are appended to facts when they indicate age or decay:
//! - `(stale)` — decay_score < 0.3 (badly decayed)
//! - `(N days ago)` — last_accessed > 30 days (not recently used)
//! - `(unused)` — access_count == 0 and age > 7 days (never retrieved)

use super::lang;
use super::types::{Category, Fact, MAX_TOTAL_FACTS_CHARS, Scope};
use crate::utils::truncate_chars;
use chrono::Utc;

/// Threshold for decay_score below which a fact is considered stale.
const STALE_DECAY_THRESHOLD: f32 = 0.3;

/// Number of days without access before showing "days ago" label.
const DAYS_AGO_THRESHOLD: i64 = 30;

/// Number of days since creation (with zero accesses) before showing "unused" label.
const UNUSED_AGE_THRESHOLD: i64 = 7;

/// Normalize fact content from first-person to third-person for prompt rendering.
///
/// This prevents the LLM from confusing user preferences with its own identity.
/// For example, "I prefer dark mode" becomes "User prefers dark mode".
///
/// The normalization is applied only during prompt rendering — the database
/// stores the original text as entered by the user or extracted heuristically.
///
/// # Rules
///
/// - "I prefer" → "User prefers"
/// - "I like" → "User likes"
/// - "I hate" → "User hates"
/// - "I want" → "User wants"
/// - "I don't want" → "User doesn't want"
/// - "I don't like" → "User doesn't like"
/// - "I love" → "User loves"
/// - "I dislike" → "User dislikes"
/// - "I'm" / "I am" → "User is"
/// - "My" → "User's"
/// - Sentences not starting with first-person pronouns are left unchanged
///
/// # ADR Reference
///
/// ADR-E4: Third-person normalization in prompt rendering. Research shows LLMs
/// can misinterpret first-person facts in system prompts as self-descriptions.
/// See: Mem0 (third-person extraction), Claude Code (third-person with headers),
/// MemGPT (labeled `<human>` blocks).
pub fn normalize_to_third_person(content: &str) -> String {
    let lower = content.to_lowercase();

    for (from, to) in lang::normalize_replacements() {
        if lower.starts_with(from) {
            // Preserve original casing for the rest of the sentence
            let rest = &content[from.len()..];
            return format!("{}{}", to, rest);
        }
    }

    // No first-person pattern matched — return as-is
    content.to_string()
}

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
/// Groups facts by scope (Global first, then Project) and by category
/// (preferences first, then facts) within each scope. Truncates to
/// MAX_TOTAL_FACTS_CHARS if necessary.
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

    // Separate by scope: Global first, then Project
    let global_facts: Vec<&Fact> = facts.iter().filter(|f| f.scope == Scope::Global).collect();

    let project_facts: Vec<&Fact> = facts.iter().filter(|f| f.scope == Scope::Project).collect();

    // Render Global scope facts
    if !global_facts.is_empty() {
        render_scope_section(&mut section, "Global", &global_facts);
    }

    // Render Project scope facts
    if !project_facts.is_empty() {
        render_scope_section(&mut section, "Project", &project_facts);
    }

    // Truncate if over limit (Unicode-safe)
    if section.len() > MAX_TOTAL_FACTS_CHARS {
        // Use chars count, not bytes
        section = truncate_chars(&section, MAX_TOTAL_FACTS_CHARS);
    }

    section
}

/// Render a scope section (Global or Project) with preferences and facts.
fn render_scope_section(section: &mut String, scope_label: &str, facts: &[&Fact]) {
    let preferences: Vec<&&Fact> = facts
        .iter()
        .filter(|f| f.category == Category::Preference)
        .collect();

    let fact_list: Vec<&&Fact> = facts
        .iter()
        .filter(|f| f.category == Category::Fact)
        .collect();

    section.push_str(&format!("### {} Preferences\n", scope_label));
    if preferences.is_empty() {
        section.push_str("(none)\n");
    } else {
        for fact in preferences {
            let staleness = get_staleness_label(fact);
            let display = normalize_to_third_person(&fact.content);
            section.push_str(&format!("- {}{}\n", display, staleness));
        }
    }
    section.push('\n');

    section.push_str(&format!("### {} Facts\n", scope_label));
    if fact_list.is_empty() {
        section.push_str("(none)\n");
    } else {
        for fact in fact_list {
            let staleness = get_staleness_label(fact);
            let display = normalize_to_third_person(&fact.content);
            section.push_str(&format!("- {}{}\n", display, staleness));
        }
    }
    section.push('\n');
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
            has_embedding: false,
        }
    }

    fn create_test_fact_with_scope(content: &str, category: Category, scope: Scope) -> Fact {
        Fact {
            id: 1,
            scope,
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
            has_embedding: false,
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

        // Within each scope section, preferences should come before facts
        let pref_pos = result
            .find("### Project Preferences")
            .expect("Should have project preferences");
        let fact_pos = result
            .find("### Project Facts")
            .expect("Should have project facts");

        assert!(pref_pos < fact_pos, "Preferences should come before facts");
    }

    #[test]
    fn test_only_preferences() {
        let facts = vec![
            create_test_fact("I prefer dark mode", Category::Preference),
            create_test_fact("I like short responses", Category::Preference),
        ];

        let result = build_facts_section(&facts);

        assert!(result.contains("### Project Preferences"));
        // Facts section is present but empty
        assert!(result.contains("### Project Facts"));
    }

    #[test]
    fn test_only_facts() {
        let facts = vec![
            create_test_fact("The project uses SQLite", Category::Fact),
            create_test_fact("API endpoint is /api/v1", Category::Fact),
        ];

        let result = build_facts_section(&facts);

        // Preferences section is present but empty
        assert!(result.contains("### Project Preferences"));
        assert!(result.contains("### Project Facts"));
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

        // Result should be well-formed with scope sections
        assert!(result.contains("### Project Facts"));

        // Should end at valid char boundary
        assert!(result.is_char_boundary(result.len()));
    }

    #[test]
    fn test_format() {
        // Fresh facts should have no staleness label
        // Note: build_facts_section now normalizes to third-person and groups by scope
        let facts = vec![
            create_test_fact_with_scope("I prefer Portuguese", Category::Preference, Scope::Global),
            create_test_fact_with_scope("The project uses Rust", Category::Fact, Scope::Project),
        ];

        let result = build_facts_section(&facts);

        // Global preferences come first
        assert!(result.contains("### Global Preferences"));
        assert!(result.contains("- User prefers Portuguese"));
        // Project facts come second
        assert!(result.contains("### Project Facts"));
        assert!(result.contains("- The project uses Rust"));
    }

    // --- Third-person normalization tests ---

    #[test]
    fn test_normalize_preference_english() {
        assert_eq!(
            normalize_to_third_person("I prefer dark mode"),
            "User prefers dark mode"
        );
        assert_eq!(
            normalize_to_third_person("I like Python"),
            "User likes Python"
        );
        assert_eq!(
            normalize_to_third_person("I hate verbose errors"),
            "User hates verbose errors"
        );
        assert_eq!(
            normalize_to_third_person("I want short responses"),
            "User wants short responses"
        );
        assert_eq!(
            normalize_to_third_person("I love concise code"),
            "User loves concise code"
        );
        assert_eq!(
            normalize_to_third_person("I dislike complexity"),
            "User dislikes complexity"
        );
    }

    #[test]
    fn test_normalize_negation_english() {
        assert_eq!(
            normalize_to_third_person("I don't want to repeat myself"),
            "User doesn't want to repeat myself"
        );
        assert_eq!(
            normalize_to_third_person("I don't like verbose messages"),
            "User doesn't like verbose messages"
        );
        assert_eq!(
            normalize_to_third_person("I can't stand slow responses"),
            "User can't stand slow responses"
        );
    }

    #[test]
    fn test_normalize_identity_english() {
        assert_eq!(
            normalize_to_third_person("My name is Lucas"),
            "User's name is Lucas"
        );
        assert_eq!(
            normalize_to_third_person("I'm a developer"),
            "User is a developer"
        );
        assert_eq!(
            normalize_to_third_person("I am from Brazil"),
            "User is from Brazil"
        );
        assert_eq!(
            normalize_to_third_person("I work at Google"),
            "User works at Google"
        );
        assert_eq!(
            normalize_to_third_person("I live in São Paulo"),
            "User lives in São Paulo"
        );
    }

    #[test]
    fn test_normalize_third_person_unchanged() {
        // Facts already in third person should not be changed
        assert_eq!(
            normalize_to_third_person("The project uses Rust"),
            "The project uses Rust"
        );
        assert_eq!(
            normalize_to_third_person("Database is PostgreSQL"),
            "Database is PostgreSQL"
        );
        assert_eq!(
            normalize_to_third_person("API endpoint is /api/v1"),
            "API endpoint is /api/v1"
        );
    }

    #[test]
    fn test_normalize_portuguese_preference() {
        // PT preferences now normalize to English
        assert_eq!(
            normalize_to_third_person("eu prefiro respostas curtas"),
            "User prefers respostas curtas"
        );
        assert_eq!(
            normalize_to_third_person("Eu gosto de café"),
            "User likes café"
        );
    }

    #[test]
    fn test_normalize_portuguese_identity() {
        // PT identity patterns normalize to English third-person
        assert_eq!(
            normalize_to_third_person("Meu nome é Lucas"),
            "User's name is Lucas"
        );
        assert_eq!(
            normalize_to_third_person("Eu moro em São Paulo"),
            "User lives in São Paulo"
        );
    }

    #[test]
    fn test_normalize_portuguese_negation() {
        assert_eq!(
            normalize_to_third_person("Eu não gosto de código ruins"),
            "User doesn't like código ruins"
        );
        assert_eq!(
            normalize_to_third_person("Eu não quero repetir"),
            "User doesn't want repetir"
        );
    }

    #[test]
    fn test_normalize_my_possessive() {
        assert_eq!(
            normalize_to_third_person("My language is Portuguese"),
            "User's language is Portuguese"
        );
    }

    #[test]
    fn test_normalize_in_section_with_third_person() {
        // Verify that build_facts_section produces third-person output
        let facts = vec![
            create_test_fact("I prefer dark mode", Category::Preference),
            create_test_fact("The project uses SQLite", Category::Fact),
        ];

        let result = build_facts_section(&facts);

        assert!(
            result.contains("- User prefers dark mode"),
            "Third-person normalization should apply to preferences"
        );
        assert!(
            result.contains("- The project uses SQLite"),
            "Third-person facts should be unchanged"
        );
    }

    #[test]
    fn test_scope_separation() {
        // Facts are grouped by scope: Global first, then Project
        let facts = vec![
            create_test_fact_with_scope("I prefer dark mode", Category::Preference, Scope::Global),
            create_test_fact_with_scope("The project uses Rust", Category::Fact, Scope::Project),
            create_test_fact_with_scope(
                "I like short responses",
                Category::Preference,
                Scope::Global,
            ),
            create_test_fact_with_scope("API is REST", Category::Fact, Scope::Project),
        ];

        let result = build_facts_section(&facts);

        // Global section comes first
        let global_pos = result
            .find("### Global Preferences")
            .expect("Should have global preferences");
        // Project section comes second
        let project_pos = result
            .find("### Project Preferences")
            .expect("Should have project preferences");

        assert!(
            global_pos < project_pos,
            "Global section should come before Project section"
        );

        // Each scope has its own preferences and facts
        assert!(result.contains("### Global Preferences"));
        assert!(result.contains("### Global Facts"));
        assert!(result.contains("### Project Preferences"));
        assert!(result.contains("### Project Facts"));
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
            has_embedding: false,
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
            has_embedding: false,
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
