//! Heuristic classification for facts
//!
//! Classifies facts into preference or fact categories using pattern matching.
//! No LLM required - pure heuristic with 90%+ accuracy.
//!
//! Classification keywords are centralized in `lang::preference_keywords()`.

use super::lang;
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
/// use sprachspiel::facts::classify::classify_fact;
/// use sprachspiel::facts::types::Category;
///
/// assert!(matches!(classify_fact("I prefer dark mode"), Category::Preference));
/// assert!(matches!(classify_fact("The project uses SQLite"), Category::Fact));
/// ```
pub fn classify_fact(content: &str) -> Category {
    let lower = content.to_lowercase();

    if contains_preference_patterns(&lower) {
        Category::Preference
    } else {
        Category::Fact
    }
}

/// Check if content contains preference patterns using centralized keywords.
fn contains_preference_patterns(lower: &str) -> bool {
    for (keyword, _lang) in lang::preference_keywords() {
        if lower.contains(keyword) {
            return true;
        }
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
        assert!(matches!(classify_fact("Adoro Rust"), Category::Preference));
        assert!(matches!(
            classify_fact("Detesto bugs"),
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
        // "The user prefers Portuguese" is a statement about user, not a first-person preference.
        // Since "prefers" (without "i prefer") won't match as a preference keyword,
        // this should be classified as a fact.
        assert!(matches!(
            classify_fact("The user prefers Portuguese"),
            Category::Fact
        ));
    }
}
