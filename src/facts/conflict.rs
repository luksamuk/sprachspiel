//! Conflict detection and resolution for facts
//!
//! Detects conflicts between new facts and existing facts using FTS5 search,
//! and resolves them using heuristics.

use super::db::FactSearchResult;
use super::types::{Category, Fact};

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
    /// Similarity score (0.0 to 1.0, higher = more similar)
    pub similarity: f32,
}

/// Resolution action for a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionAction {
    /// No conflict, add the new fact
    Add,
    /// Duplicate, skip adding
    Skip,
    /// Contradiction, replace existing fact
    Update,
}

/// Check if two facts are contradictory
///
/// Uses simple heuristics to detect contradictions:
/// - "I like X" vs "I hate X"
/// - "X is A" vs "X is B" with negation
pub fn is_contradiction(new: &str, existing: &str) -> bool {
    let new_lower = new.to_lowercase();
    let existing_lower = existing.to_lowercase();

    // Check for negation patterns
    // "I like X" vs "I hate X"
    if (contains_preference_like(&new_lower) && contains_preference_hate(&existing_lower))
        || (contains_preference_hate(&new_lower) && contains_preference_like(&existing_lower))
    {
        return true;
    }

    // Check for "not" / "não" contradiction
    // This is a simplified check - real contradiction detection would need more sophisticated NLP
    if has_opposite_negation(&new_lower, &existing_lower) {
        return true;
    }

    false
}

/// Check if content contains preference-like patterns
fn contains_preference_like(s: &str) -> bool {
    s.contains("like ") || s.contains("gosto de") || s.contains("prefiro")
}

/// Check if content contains preference-hate patterns
fn contains_preference_hate(s: &str) -> bool {
    s.contains("hate ") || s.contains("odeio") || s.contains("não gosto")
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
        // Normalize score to 0.0 - 1.0 range
        // FTS5 BM25 scores are negative, we negate and cap
        let similarity = result.score.clamp(0.0, 1.0);

        if similarity >= threshold {
            let conflict_type = if is_contradiction(new_content, &result.fact.content) {
                ConflictType::Contradiction
            } else if similarity > 0.95 {
                // Very high similarity but no contradiction = likely duplicate
                ConflictType::Duplicate
            } else {
                continue; // Not a conflict
            };

            conflicts.push(Conflict {
                existing_fact: result.fact.clone(),
                conflict_type,
                similarity,
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
        // A duplicate needs similarity > 0.95 to be detected
        let mut existing = create_test_fact_with_content("The project uses SQLite");
        existing.score = 0.96; // High enough to be detected as duplicate
        let results = vec![existing];

        let conflicts = detect_conflicts("The project uses SQLite", &results, 0.8);

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
            },
            conflict_type: ConflictType::Duplicate,
            similarity: 0.98,
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
            },
            conflict_type: ConflictType::Contradiction,
            similarity: 0.9,
        };

        assert!(matches!(
            resolve_conflict(conflict),
            ResolutionAction::Update
        ));
    }
}
