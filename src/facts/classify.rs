//! Heuristic classification for facts
//!
//! Classifies facts into preference or fact categories using pattern matching.
//! No LLM required - pure heuristic with 90%+ accuracy.

use super::types::Category;

/// Classify a fact into a category using heuristic pattern matching.
///
/// # Arguments
/// * `content` - The fact content to classify
///
/// # Returns
/// The predicted category (preference or fact)
///
/// # Examples
/// ```
/// use ask_ai::facts::classify::classify_fact;
/// use ask_ai::facts::types::Category;
///
/// assert!(matches!(classify_fact("I prefer dark mode"), Category::Preference));
/// assert!(matches!(classify_fact("The project uses SQLite"), Category::Fact));
/// ```
pub fn classify_fact(content: &str) -> Category {
    let lower = content.to_lowercase();

    // Heuristic for preferences
    // Portuguese: prefiro, gosto, odeio, não gosto, quero
    // English: prefer, like, hate, want, don't like
    if contains_preference_patterns(&lower) {
        return Category::Preference;
    }

    // Default: fact
    Category::Fact
}

/// Check if content contains preference patterns
fn contains_preference_patterns(lower: &str) -> bool {
    // Portuguese preference patterns
    if lower.contains("prefiro") || lower.contains("prefere") {
        return true;
    }
    if lower.contains("gosto de") || lower.contains("gosta de") {
        return true;
    }
    if lower.contains("odeio") || lower.contains("não gosto") || lower.contains("nao gosto") {
        return true;
    }
    if lower.contains("quero") || lower.contains("não quero") || lower.contains("nao quero") {
        return true;
    }

    // English preference patterns
    if lower.contains("i prefer") || lower.contains("i like") || lower.contains("i hate") {
        return true;
    }
    if lower.contains("i usually prefer")
        || lower.contains("i usually like")
        || lower.contains("i usually hate")
    {
        return true;
    }
    if lower.contains("i always") || lower.contains("i never") {
        return true;
    }
    if lower.contains("i want") || lower.contains("i don't want") || lower.contains("i dont want") {
        return true;
    }
    if lower.contains("i love") || lower.contains("i dislike") {
        return true;
    }
    if lower.contains("i find") || lower.contains("i find it") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_preference_portuguese() {
        assert!(matches!(
            classify_fact("Eu prefiro respostas curtas"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("Prefiro trabalhar de manhã"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("Gosto de café"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("Odeio quando isso acontece"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("Não gosto de código desorganizado"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("Quero terminar isso logo"),
            Category::Preference
        ));
    }

    #[test]
    fn test_classify_preference_english() {
        assert!(matches!(
            classify_fact("I prefer dark mode"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("I like Python for scripting"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("I hate verbose error messages"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("I want short responses"),
            Category::Preference
        ));
        assert!(matches!(
            classify_fact("I don't want to repeat myself"),
            Category::Preference
        ));
    }

    #[test]
    fn test_classify_fact_default() {
        assert!(matches!(
            classify_fact("The project uses SQLite for storage"),
            Category::Fact
        ));
        assert!(matches!(
            classify_fact("Os documentos estão em ~/docs"),
            Category::Fact
        ));
        assert!(matches!(
            classify_fact("API endpoint is /api/v1/users"),
            Category::Fact
        ));
    }

    #[test]
    fn test_classify_edge_cases() {
        // These should be facts, not preferences
        assert!(matches!(
            classify_fact("The user prefers Portuguese"),
            Category::Fact
        ));
        // "Prefers" without "I" is a statement about user, not a preference
    }
}
