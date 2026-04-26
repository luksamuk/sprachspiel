//! Conflict detection and resolution for facts
//!
//! Detects conflicts between new facts and existing facts using FTS5 search,
//! and resolves them using heuristics.

use super::db::FactSearchResult;
use super::types::{Category, Fact};

/// Default threshold for conflict detection (similarity score 0.0-1.0)
///
/// Lowered from 0.85 to 0.75 to catch more FTS5 keyword matches.
/// BM25 normalization maps score -5 (good match) to ~0.83, which was
/// just below the old 0.85 threshold. At 0.75, most real duplicates
/// are caught while false positives remain rare due to the layered
/// dedup pipeline (exact match → normalized match → FTS5).
pub const CONFLICT_THRESHOLD: f32 = 0.75;

/// Type of conflict detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictType {
    /// High similarity, no contradiction (duplicate)
    Duplicate,
    /// High similarity with contradiction
    Contradiction,
}

/// A conflict between facts
#[derive(Debug, Clone)]
pub struct Conflict {
    /// The existing fact that conflicts
    pub existing_fact: Fact,
    /// Type of conflict
    pub conflict_type: ConflictType,
}

/// Resolution action for a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionAction {
    /// No conflict, add the new fact.
    /// This variant exists for enum completeness but is never instantiated
    /// in production code - when there's no conflict, we simply insert without
    /// returning an action.
    #[allow(dead_code)]
    Add,
    /// Duplicate, skip adding
    Skip,
    /// Contradiction, replace existing fact
    Update,
}

/// Check if two facts are contradictory
///
/// Uses simple heuristics to detect contradictions:
/// - "I like X" vs "I hate X" (preference conflict)
/// - "I prefer X" vs "I prefer Y" (preference override with different object)
/// - "X is A" vs "X is B" with negation
/// - PT equivalents of the above
pub fn is_contradiction(new: &str, existing: &str) -> bool {
    let new_lower = new.to_lowercase();
    let existing_lower = existing.to_lowercase();

    // Check for preference conflict patterns
    // "I like X" vs "I hate X"
    if (contains_preference_like(&new_lower) && contains_preference_hate(&existing_lower))
        || (contains_preference_hate(&new_lower) && contains_preference_like(&existing_lower))
    {
        return true;
    }

    // Check for preference override: "I prefer X" vs "I prefer Y"
    // Both talk about preferences but with different objects
    if is_preference_override(&new_lower, &existing_lower) {
        return true;
    }

    // Check for "not" / "não" contradiction
    if has_opposite_negation(&new_lower, &existing_lower) {
        return true;
    }

    false
}

/// Check if two statements are preference overrides.
///
/// Detects patterns like "I prefer dark mode" vs "I prefer light mode"
/// where both contain preference verbs but the objects differ.
/// This is a contradiction because the user's preference has changed.
fn is_preference_override(a: &str, b: &str) -> bool {
    // Both must contain preference verbs
    let a_has_pref = contains_preference_verb(a);
    let b_has_pref = contains_preference_verb(b);

    if !a_has_pref || !b_has_pref {
        return false;
    }

    // Extract the object of the preference (everything after the verb)
    let a_object = extract_preference_object(a);
    let b_object = extract_preference_object(b);

    // If both have objects and they differ, it's a preference override
    if let (Some(a_obj), Some(b_obj)) = (a_object, b_object) {
        // Must have significant word overlap to be about the same topic
        // but different specific preference
        let overlap = word_overlap_ratio(&a_obj, &b_obj);
        // Low overlap means completely different topics (not a contradiction)
        // High overlap means same topic, different preference
        overlap > 0.3 && overlap < 0.95 && a_obj != b_obj
    } else {
        false
    }
}

/// Check if content contains any preference verb
fn contains_preference_verb(s: &str) -> bool {
    s.contains("prefer")
        || s.contains("like ")
        || s.contains("love ")
        || s.contains("hate ")
        || s.contains("dislike")
        || s.contains("want ")
        || s.contains("wish ")
        || s.contains("enjoy ")
        || s.contains("gosto de")
        || s.contains("prefiro")
        || s.contains("prefere")
        || s.contains("adoro")
        || s.contains("adora")
        || s.contains("odeio")
        || s.contains("odeia")
        || s.contains("detesto")
        || s.contains("detesta")
}

/// Extract the object of a preference statement.
///
/// Given "i prefer dark mode" → Some("dark mode")
/// Given "user likes python" → Some("python")
/// Given "the project uses rust" → None
fn extract_preference_object(s: &str) -> Option<String> {
    // Preference verbs and their lengths
    let verbs: &[&str] = &[
        "i prefer ",
        "i like ",
        "i hate ",
        "i love ",
        "i dislike ",
        "i want ",
        "i enjoy ",
        "user prefers ",
        "user likes ",
        "user hates ",
        "user loves ",
        "user dislikes ",
        "user wants ",
        "user enjoys ",
        "prefiro ",
        "prefere ",
        "gosto de ",
        "gosta de ",
        "adoro ",
        "adora ",
        "odeio ",
        "odeia ",
        "detesto ",
        "detesta ",
    ];

    for verb in verbs {
        if s.starts_with(verb) {
            let rest = s.strip_prefix(verb).unwrap_or(s);
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

/// Calculate word overlap ratio between two strings.
fn word_overlap_ratio(a: &str, b: &str) -> f32 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();

    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let total = a_words.len().max(b_words.len());
    intersection as f32 / total as f32
}

/// Check if content contains preference-like patterns
fn contains_preference_like(s: &str) -> bool {
    s.contains("like ")
        || s.contains("love ")
        || s.contains("enjoy ")
        || s.contains("prefer")
        || s.contains("gosto de")
        || s.contains("prefiro")
        || s.contains("prefere")
        || s.contains("adoro")
        || s.contains("adora")
}

/// Check if content contains preference-hate patterns
fn contains_preference_hate(s: &str) -> bool {
    s.contains("hate ")
        || s.contains("dislike")
        || s.contains("odeio")
        || s.contains("odeia")
        || s.contains("não gosto")
        || s.contains("nao gosto")
        || s.contains("detesto")
        || s.contains("detesta")
}

/// Check if two strings have opposite negation
fn has_opposite_negation(a: &str, b: &str) -> bool {
    let a_neg = a.contains("não ") || a.contains("nao ") || a.contains("not ");
    let b_neg = b.contains("não ") || b.contains("nao ") || b.contains("not ");

    // If one has negation and the other doesn't, it might be a contradiction
    // But we need to be careful - this is a heuristic
    if a_neg != b_neg {
        // Check if they're talking about the same thing
        // This is a simplified check
        let a_words: Vec<&str> = a.split_whitespace().take(5).collect();
        let b_words: Vec<&str> = b.split_whitespace().take(5).collect();

        // If first few words are similar, might be contradiction
        let common_words = a_words.iter().filter(|w| b_words.contains(w)).count();

        if common_words >= 2 {
            return true;
        }
    }

    false
}

/// Resolve a conflict using heuristics
///
/// Resolution strategy:
/// - Duplicate → Skip
/// - Contradiction → Update (newer wins, unless high-importance preference)
pub fn resolve_conflict(conflict: Conflict) -> ResolutionAction {
    match conflict.conflict_type {
        ConflictType::Duplicate => ResolutionAction::Skip,
        ConflictType::Contradiction => {
            // For high-importance preferences, we might want LLM to adjudicate
            // But for now, we use temporal resolution: newer wins
            if conflict.existing_fact.category == Category::Preference
                && conflict.existing_fact.importance >= 0.8
            {
                // High-importance preference - still update, but log it
                // In the future, this could trigger LLM confirmation
                ResolutionAction::Update
            } else {
                ResolutionAction::Update
            }
        }
    }
}

/// Detect conflicts between a new fact and existing facts
///
/// Uses FTS5 search results to find similar facts, then checks for contradictions.
///
/// # Arguments
/// * `new_content` - The new fact content
/// * `search_results` - Search results from FTS5
/// * `threshold` - Similarity threshold (0.0 to 1.0)
///
/// # Returns
/// Vector of detected conflicts
pub fn detect_conflicts(
    new_content: &str,
    search_results: &[FactSearchResult],
    threshold: f32,
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for result in search_results {
        // Score is already normalized to [0, 1) by search_facts using reciprocal transform
        let similarity = result.score;

        if similarity >= threshold {
            let conflict_type = if is_contradiction(new_content, &result.fact.content) {
                ConflictType::Contradiction
            } else {
                // Any similarity >= threshold without contradiction = duplicate.
                // Previously had a gap (0.85-0.95) where facts were silently ignored,
                // causing exact duplicates to be inserted. Now all scores >= threshold
                // are treated as either contradiction or duplicate.
                ConflictType::Duplicate
            };

            conflicts.push(Conflict {
                existing_fact: result.fact.clone(),
                conflict_type,
            });
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::types::{Scope, Source};
    use chrono::Utc;

    fn create_test_fact_with_content(content: &str) -> FactSearchResult {
        FactSearchResult {
            fact: Fact {
                id: 1,
                scope: Scope::Project,
                category: Category::Fact,
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
            },
            score: 0.9, // High similarity by default
        }
    }

    #[test]
    fn test_is_contradiction_like_vs_hate() {
        assert!(is_contradiction("I like Python", "I hate Python"));
        assert!(is_contradiction("Gosto de café", "Odeio café"));
    }

    #[test]
    fn test_is_contradiction_not_contradiction() {
        // Similar but not contradictory
        assert!(!is_contradiction(
            "The project uses SQLite",
            "The project uses PostgreSQL"
        ));
    }

    #[test]
    fn test_detect_conficts_duplicate() {
        // Test that detect_conflicts works as expected
        // Any similarity >= threshold (0.75) without contradiction = duplicate
        let mut existing = create_test_fact_with_content("The project uses SQLite");
        existing.score = 0.80; // Above threshold (0.75), no contradiction = duplicate
        let results = vec![existing];

        let conflicts = detect_conflicts("The project uses SQLite", &results, CONFLICT_THRESHOLD);

        // With similarity > 0.95 and no contradiction, should be detected as duplicate
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict_type,
            ConflictType::Duplicate
        ));
    }

    #[test]
    fn test_resolve_conflict_duplicate() {
        let conflict = Conflict {
            existing_fact: Fact {
                id: 1,
                scope: Scope::Project,
                category: Category::Fact,
                content: "Test".to_string(),
                importance: 0.5,
                access_count: 0,
                decay_score: 1.0,
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                source: Source::User,
                invalidated_at: None,
                project_id: None,
                has_embedding: false,
            },
            conflict_type: ConflictType::Duplicate,
        };

        assert!(matches!(resolve_conflict(conflict), ResolutionAction::Skip));
    }

    #[test]
    fn test_resolve_conflict_contradiction() {
        let conflict = Conflict {
            existing_fact: Fact {
                id: 1,
                scope: Scope::Project,
                category: Category::Fact,
                content: "Test".to_string(),
                importance: 0.5,
                access_count: 0,
                decay_score: 1.0,
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                source: Source::User,
                invalidated_at: None,
                project_id: None,
                has_embedding: false,
            },
            conflict_type: ConflictType::Contradiction,
        };

        assert!(matches!(
            resolve_conflict(conflict),
            ResolutionAction::Update
        ));
    }

    #[test]
    fn test_detect_conflicts_contradiction() {
        // Test contradiction detection
        let existing = FactSearchResult {
            fact: Fact {
                id: 1,
                scope: Scope::Project,
                category: Category::Preference,
                content: "I like verbose responses".to_string(),
                importance: 0.5,
                access_count: 0,
                decay_score: 1.0,
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                source: Source::User,
                invalidated_at: None,
                project_id: None,
                has_embedding: false,
            },
            score: 0.90,
        };
        let results = vec![existing];

        let conflicts = detect_conflicts("I hate verbose responses", &results, CONFLICT_THRESHOLD);

        // Should detect contradiction because "like" vs "hate"
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict_type,
            ConflictType::Contradiction
        ));
    }

    #[test]
    fn test_detect_conflicts_no_conflict() {
        // Test no conflict when similarity is below threshold
        let existing = FactSearchResult {
            fact: Fact {
                id: 1,
                scope: Scope::Project,
                category: Category::Fact,
                content: "The project uses SQLite".to_string(),
                importance: 0.5,
                access_count: 0,
                decay_score: 1.0,
                created_at: Utc::now(),
                last_accessed: Utc::now(),
                source: Source::User,
                invalidated_at: None,
                project_id: None,
                has_embedding: false,
            },
            score: 0.50, // Below threshold (0.75)
        };
        let results = vec![existing];

        let conflicts =
            detect_conflicts("The project uses PostgreSQL", &results, CONFLICT_THRESHOLD);

        // No conflict because similarity is below threshold
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_detect_conflicts_gap_0_85_to_0_95_is_duplicate() {
        // Bug #1 fix: similarity between old threshold range was silently ignored.
        // Now all scores >= threshold (0.75) are treated as either duplicate or contradiction.
        let mut existing = create_test_fact_with_content("The project uses SQLite");
        existing.score = 0.80; // Above current threshold (0.75), no contradiction = duplicate
        let results = vec![existing];

        let conflicts = detect_conflicts("The project uses SQLite", &results, CONFLICT_THRESHOLD);

        // Should be detected as duplicate (not silently ignored)
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict_type,
            ConflictType::Duplicate
        ));
    }

    #[test]
    fn test_detect_conflicts_exact_match_is_duplicate() {
        // Exact similarity at threshold (0.75) is a duplicate
        let mut existing = create_test_fact_with_content("The project uses SQLite");
        existing.score = 0.75; // Exactly at threshold
        let results = vec![existing];

        let conflicts = detect_conflicts("The project uses SQLite", &results, CONFLICT_THRESHOLD);

        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict_type,
            ConflictType::Duplicate
        ));
    }

    // ── Preference Override Detection ─────────────────────────────────

    #[test]
    fn test_contradiction_preference_override_dark_light() {
        // "I prefer dark mode" vs "I prefer light mode" = contradiction
        assert!(is_contradiction(
            "I prefer dark mode",
            "I prefer light mode"
        ));
    }

    #[test]
    fn test_contradiction_preference_override_third_person() {
        // "User prefers dark mode" vs "User prefers light mode" = contradiction
        assert!(is_contradiction(
            "User prefers dark mode",
            "User prefers light mode"
        ));
    }

    #[test]
    fn test_contradiction_preference_like_hate() {
        assert!(is_contradiction("I like Python", "I hate Python"));
    }

    #[test]
    fn test_no_contradiction_different_topics() {
        // "I prefer dark mode" vs "I like Python" are NOT contradictions
        // (preferences about different topics)
        assert!(!is_contradiction("I prefer dark mode", "I like Python"));
    }

    #[test]
    fn test_no_contradiction_same_preference() {
        // "I prefer dark mode" vs "I prefer dark mode" are NOT contradictions
        // (same preference)
        assert!(!is_contradiction(
            "I prefer dark mode",
            "I prefer dark mode"
        ));
    }

    #[test]
    fn test_extract_preference_object() {
        assert_eq!(
            extract_preference_object("i prefer dark mode"),
            Some("dark mode".to_string())
        );
        assert_eq!(
            extract_preference_object("user likes python"),
            Some("python".to_string())
        );
        assert_eq!(extract_preference_object("the project uses rust"), None);
    }

    #[test]
    fn test_word_overlap_ratio() {
        assert!(word_overlap_ratio("dark mode", "light mode") > 0.3);
        assert!(word_overlap_ratio("dark mode", "python programming") < 0.3);
    }
}
