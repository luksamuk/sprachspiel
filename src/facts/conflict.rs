//! Conflict detection and resolution for facts
//!
//! Detects conflicts between new facts and existing facts using FTS5 search,
//! and resolves them using heuristics.
//!
//! # Layer 3.5: Semantic Contradiction Detection (with Triple Disambiguation)
//!
//! After Layer 2 (normalized match) and before Layer 3 (FTS5 BM25), semantic
//! search via embeddings finds candidate pairs at cosine ≥ 0.70. Triple
//! extraction then disambiguates: same predicate + different object = contradiction,
//! same triple = duplicate, different predicate = fall through to is_contradiction().

use super::db::FactSearchResult;
use super::lang::{
    EXCLUSIVE_PREDICATES, NEGATIVE_PREDICATES, POSITIVE_PREDICATES, STOP_WORDS,
    TRIPLE_IDENTITY_PREFIXES, TRIPLE_PREFERENCE_PREFIXES,
};
use super::types::{Category, Fact};

// === Triple Extraction for Semantic Contradiction Disambiguation ===

/// A semantic triple extracted from a fact for contradiction detection.
///
/// Represents `(subject, predicate, object)` — used to detect when two facts
/// share the same subject and predicate but differ in object, indicating
/// a preference override or identity change.
///
/// # Examples
///
/// ```ignore
/// use sprachspiel::facts::conflict::{FactTriple, extract_fact_triple};
///
/// let a = extract_fact_triple("User prefers dark mode").unwrap();
/// // FactTriple { subject: "user", predicate: "prefers", object: "dark mode" }
///
/// let b = extract_fact_triple("User prefers light mode").unwrap();
/// // FactTriple { subject: "user", predicate: "prefers", object: "light mode" }
///
/// assert!(a.contradicts(&b)); // Same predicate, different object
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactTriple {
    /// Always "user" for user facts
    pub subject: String,
    /// The predicate: "prefers", "likes", "name is", "is from", etc.
    pub predicate: String,
    /// The object: "dark mode", "Lucas", "Brazil", etc.
    pub object: String,
}

impl FactTriple {
    /// Check if two triples contradict each other.
    ///
    /// Two facts contradict if they share the same subject and a **mutually
    /// exclusive** predicate but have different objects. Predicates like
    /// "prefers", "always prefers", "name is" are exclusive — you can only
    /// have ONE preference or ONE name. Predicates like "likes", "loves",
    /// "hates", "uses" are **accumulative** — you can like both Python and Rust.
    ///
    /// # Examples
    ///
    /// - `(user, prefers, dark mode)` vs `(user, prefers, light mode)` → **true** (exclusive)
    /// - `(user, likes, python)` vs `(user, likes, rust)` → **false** (accumulative)
    /// - `(user, likes, python)` vs `(user, hates, python)` → **true** (polarity flip)
    /// - `(user, prefers, dark mode)` vs `(user, prefers, dark mode)` → **false** (same object)
    pub fn contradicts(&self, other: &FactTriple) -> bool {
        if self.subject != other.subject || self.object == other.object {
            return false;
        }

        // Check for polarity flip: like/hate on the same object
        if is_polarity_flip(&self.predicate, &other.predicate) {
            return true;
        }

        // Same predicate: contradiction depends on exclusivity
        if self.predicate == other.predicate {
            if is_exclusive_predicate(&self.predicate) {
                // Exclusive: any different object = contradiction (e.g., prefers X vs prefers Y)
                return true;
            }
            // Accumulative: only a contradiction if objects share significant words
            // (e.g., "likes dark mode" vs "likes light mode" share "mode" — same category)
            // but "likes Python" vs "likes Rust" share nothing — different topics, can coexist
            return object_word_overlap(&self.object, &other.object) > 0.3;
        }

        // Different predicates, different objects — not a contradiction
        // (e.g., "likes python" vs "prefers rust" can coexist)
        false
    }
}

/// Calculate word overlap ratio between two objects (used by `contradicts()`).
///
/// Returns the Jaccard-like overlap: `|intersection| / max(|a|, |b|)`.
/// Stop words (defined in `lang::STOP_WORDS`) are excluded to focus on
/// content words.
///
/// # Examples
///
/// - `"dark mode"` vs `"light mode"` → overlap = 1/2 = 0.5 (shared "mode") → contradiction
/// - `"Python"` vs `"Rust"` → overlap = 0/1 = 0.0 → no contradiction (can coexist)
/// - `"verbose output"` vs `"verbose errors"` → overlap = 1/2 = 0.5 (shared "verbose")
fn object_word_overlap(a: &str, b: &str) -> f32 {
    use std::collections::HashSet;

    fn content_words(s: &str) -> HashSet<String> {
        s.split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| !STOP_WORDS.contains(&w.as_str()))
            .collect()
    }

    let a_words = content_words(a);
    let b_words = content_words(b);

    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let total = a_words.len().max(b_words.len());
    intersection as f32 / total as f32
}

/// Check if a predicate is **mutually exclusive** — you can only have one.
///
/// Delegates to `lang::EXCLUSIVE_PREDICATES` as the source of truth.
fn is_exclusive_predicate(predicate: &str) -> bool {
    EXCLUSIVE_PREDICATES.contains(&predicate)
}

/// Check if two predicates form a polarity flip (like vs hate on same object).
///
/// "likes X" vs "hates X" is a contradiction regardless of predicate exclusivity.
fn is_polarity_flip(a: &str, b: &str) -> bool {
    let a_positive = is_positive_predicate(a) && is_negative_predicate(b);
    let b_positive = is_positive_predicate(b) && is_negative_predicate(a);
    a_positive || b_positive
}

/// Check if a predicate has **positive polarity** (affinity, enjoyment).
///
/// Delegates to `lang::POSITIVE_PREDICATES` as the source of truth.
fn is_positive_predicate(predicate: &str) -> bool {
    POSITIVE_PREDICATES.contains(&predicate)
}

/// Check if a predicate has **negative polarity** (aversion, dislike).
///
/// Delegates to `lang::NEGATIVE_PREDICATES` as the source of truth.
fn is_negative_predicate(predicate: &str) -> bool {
    NEGATIVE_PREDICATES.contains(&predicate)
}

/// Extract a semantic triple from fact content in storage format.
///
/// Uses `lang::TRIPLE_PREFERENCE_PREFIXES` and `lang::TRIPLE_IDENTITY_PREFIXES`
/// as the source of truth for prefix patterns (no string duplication).
///
/// Returns `None` for factual content that doesn't match user patterns
/// (e.g., "The project uses SQLite" — no recognizable user predicate).
///
/// # Prefix order
///
/// Preference prefixes are checked first (they're more specific and include
/// adverb+verb combos). Identity prefixes are checked second. Within each
/// category, longer prefixes are checked first to avoid partial matches
/// (e.g., "user is from " before "user is ").
///
/// # Legacy data
///
/// Covers both current third-person format ("User prefers X") and legacy
/// first-person format ("My name is X") from before the ADR-E4 fix.
/// Predicates map to canonical labels so triples are comparable across formats.
pub fn extract_fact_triple(content: &str) -> Option<FactTriple> {
    let lower = content.to_lowercase();

    // Try preference patterns first (more specific — longer prefixes)
    for (prefix, predicate) in TRIPLE_PREFERENCE_PREFIXES {
        if lower.starts_with(prefix) {
            let object = content[prefix.len()..].trim().to_string();
            if !object.is_empty() {
                return Some(FactTriple {
                    subject: "user".to_string(),
                    predicate: predicate.to_string(),
                    object,
                });
            }
        }
    }

    // Then identity patterns
    for (prefix, predicate) in TRIPLE_IDENTITY_PREFIXES {
        if lower.starts_with(prefix) {
            let object = content[prefix.len()..].trim().to_string();
            if !object.is_empty() {
                return Some(FactTriple {
                    subject: "user".to_string(),
                    predicate: predicate.to_string(),
                    object,
                });
            }
        }
    }

    None
}

/// Default threshold for FTS5 BM25 conflict detection (similarity score 0.0-1.0)
///
/// Lowered from 0.85 to 0.75 to catch more FTS5 keyword matches.
/// BM25 normalization maps score -5 (good match) to ~0.83, which was
/// just below the old 0.85 threshold. At 0.75, most real duplicates
/// are caught while false positives remain rare due to the layered
/// dedup pipeline (exact match → normalized match → semantic → FTS5).
pub const CONFLICT_THRESHOLD: f32 = 0.75;

/// Threshold for insert-time semantic search via embeddings (cosine similarity).
///
/// Used by Layer 3.5 in `dedup::deduplicate_and_insert()` (the centralized pipeline).
/// Intentionally broad (0.70) to catch contradictions that normalized match
/// and FTS5 BM25 miss (e.g., "prefer dark mode" vs "prefer light mode" at
/// cosine ~0.77). Triple-based disambiguation inside the semantic block
/// separates contradictions from duplicates and related facts.
///
/// Separate from `SEMANTIC_DEDUP_THRESHOLD = 0.90` in verify.rs, which is
/// for startup O(n²) pairwise dedup (intentionally strict — near-identical only).
pub const SEMANTIC_SEARCH_THRESHOLD: f32 = 0.70;

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
        || s.contains("likes ")
        || s.contains("love ")
        || s.contains("loves ")
        || s.contains("enjoy ")
        || s.contains("enjoys ")
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
        || s.contains("hates ")
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
    let a_neg = a.contains("não ")
        || a.contains("nao ")
        || a.contains("not ")
        || a.contains("doesn't ")
        || a.contains("don't ");
    let b_neg = b.contains("não ")
        || b.contains("nao ")
        || b.contains("not ")
        || b.contains("doesn't ")
        || b.contains("don't ");

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

    // ── FactTriple + extract_fact_triple (semantic disambiguation) ──────

    #[test]
    fn test_extract_triple_preference_simple() {
        let triple = extract_fact_triple("User prefers dark mode").unwrap();
        assert_eq!(triple.subject, "user");
        assert_eq!(triple.predicate, "prefers");
        assert_eq!(triple.object, "dark mode");
    }

    #[test]
    fn test_extract_triple_preference_likes() {
        let triple = extract_fact_triple("User likes Python").unwrap();
        assert_eq!(triple.predicate, "likes");
        assert_eq!(triple.object, "Python");
    }

    #[test]
    fn test_extract_triple_preference_adverb() {
        let triple = extract_fact_triple("User usually prefers dark mode").unwrap();
        assert_eq!(triple.predicate, "usually prefers");
        assert_eq!(triple.object, "dark mode");
    }

    #[test]
    fn test_extract_triple_preference_negation() {
        let triple = extract_fact_triple("User doesn't like verbose output").unwrap();
        assert_eq!(triple.predicate, "doesn't like");
        assert_eq!(triple.object, "verbose output");
    }

    #[test]
    fn test_extract_triple_preference_doesnt_want() {
        let triple = extract_fact_triple("User doesn't want to repeat").unwrap();
        assert_eq!(triple.predicate, "doesn't want");
        assert_eq!(triple.object, "to repeat");
    }

    #[test]
    fn test_extract_triple_adverb_really() {
        let triple = extract_fact_triple("User really likes Rust").unwrap();
        assert_eq!(triple.predicate, "really likes");
        assert_eq!(triple.object, "Rust");
    }

    #[test]
    fn test_extract_triple_adverb_always() {
        let triple = extract_fact_triple("User always prefers concise answers").unwrap();
        assert_eq!(triple.predicate, "always prefers");
        assert_eq!(triple.object, "concise answers");
    }

    #[test]
    fn test_extract_triple_adverb_never() {
        let triple = extract_fact_triple("User never wants verbose output").unwrap();
        assert_eq!(triple.predicate, "never wants");
        assert_eq!(triple.object, "verbose output");
    }

    #[test]
    fn test_extract_triple_adverb_strongly() {
        let triple = extract_fact_triple("User strongly prefers dark themes").unwrap();
        assert_eq!(triple.predicate, "strongly prefers");
        assert_eq!(triple.object, "dark themes");
    }

    #[test]
    fn test_extract_triple_identity_name() {
        let triple = extract_fact_triple("User's name is Lucas").unwrap();
        assert_eq!(triple.predicate, "name is");
        assert_eq!(triple.object, "Lucas");
    }

    #[test]
    fn test_extract_triple_identity_from() {
        let triple = extract_fact_triple("User is from Brazil").unwrap();
        assert_eq!(triple.predicate, "is from");
        assert_eq!(triple.object, "Brazil");
    }

    #[test]
    fn test_extract_triple_identity_lives_in() {
        let triple = extract_fact_triple("User lives in São Paulo").unwrap();
        assert_eq!(triple.predicate, "lives in");
        assert_eq!(triple.object, "São Paulo");
    }

    #[test]
    fn test_extract_triple_identity_works_at() {
        let triple = extract_fact_triple("User works at Google").unwrap();
        assert_eq!(triple.predicate, "works at");
        assert_eq!(triple.object, "Google");
    }

    #[test]
    fn test_extract_triple_identity_speaks() {
        let triple = extract_fact_triple("User speaks português").unwrap();
        assert_eq!(triple.predicate, "speaks");
        assert_eq!(triple.object, "português");
    }

    #[test]
    fn test_extract_triple_identity_language() {
        let triple = extract_fact_triple("User's language is inglês").unwrap();
        assert_eq!(triple.predicate, "language is");
        assert_eq!(triple.object, "inglês");
    }

    #[test]
    fn test_extract_triple_identity_is_a() {
        let triple = extract_fact_triple("User is a developer").unwrap();
        assert_eq!(triple.predicate, "is a");
        assert_eq!(triple.object, "developer");
    }

    #[test]
    fn test_extract_triple_identity_is() {
        // "User is from Brazil" should match "is from", not just "is"
        let triple = extract_fact_triple("User is from Brazil").unwrap();
        assert_eq!(triple.predicate, "is from"); // NOT "is"
    }

    #[test]
    fn test_extract_triple_no_match_factual() {
        // Non-user factual content — no recognizable triple
        assert!(extract_fact_triple("The project uses SQLite").is_none());
        assert!(extract_fact_triple("PostgreSQL is the database").is_none());
    }

    #[test]
    fn test_extract_triple_legacy_my_name() {
        // Legacy first-person identity (pre-ADR-E4-fix data)
        let triple = extract_fact_triple("My name is Lucas").unwrap();
        assert_eq!(triple.predicate, "name is"); // Same canonical predicate
        assert_eq!(triple.object, "Lucas");
    }

    #[test]
    fn test_extract_triple_legacy_i_live() {
        // Legacy first-person identity
        let triple = extract_fact_triple("I live in São Paulo").unwrap();
        assert_eq!(triple.predicate, "lives in"); // Same canonical predicate
        assert_eq!(triple.object, "São Paulo");
    }

    #[test]
    fn test_extract_triple_legacy_doesnt_like() {
        // Legacy bare negation (pre-ADR-E4-fix PT output)
        let triple = extract_fact_triple("Doesn't like verbose output").unwrap();
        assert_eq!(triple.predicate, "doesn't like");
        assert_eq!(triple.object, "verbose output");
    }

    #[test]
    fn test_triple_contradicts_same_predicate_different_object() {
        let a = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "light mode".into(),
        };
        assert!(a.contradicts(&b));
    }

    #[test]
    fn test_triple_contradicts_different_predicate() {
        let a = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "likes".into(),
            object: "light mode".into(),
        };
        assert!(!a.contradicts(&b)); // Different predicate — not a contradiction
    }

    #[test]
    fn test_triple_contradicts_same_object() {
        let a = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
        };
        assert!(!a.contradicts(&b)); // Same — duplicate, not contradiction
    }

    #[test]
    fn test_triple_contradicts_adverb_verb_same_category() {
        // "really likes dark mode" vs "really likes light mode" — accumulative verb
        // but objects share "mode" (word overlap 0.5 > 0.3) → IS contradiction (same category)
        let a = FactTriple {
            subject: "user".into(),
            predicate: "really likes".into(),
            object: "dark mode".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "really likes".into(),
            object: "light mode".into(),
        };
        assert!(a.contradicts(&b)); // shared "mode" → same category → override
    }

    #[test]
    fn test_triple_contradicts_adverb_verb_different_topics() {
        // "really likes Python" vs "really likes Rust" — accumulative verb
        // objects share NO words (overlap 0.0) → can coexist, NOT contradiction
        let a = FactTriple {
            subject: "user".into(),
            predicate: "really likes".into(),
            object: "Python".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "really likes".into(),
            object: "Rust".into(),
        };
        assert!(!a.contradicts(&b)); // no shared words → different topics → coexist
    }

    #[test]
    fn test_triple_contradicts_identity_change() {
        let a = FactTriple {
            subject: "user".into(),
            predicate: "name is".into(),
            object: "Lucas".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "name is".into(),
            object: "João".into(),
        };
        assert!(a.contradicts(&b));
    }

    #[test]
    fn test_triple_contradicts_not_across_categories() {
        // "User prefers X" vs "User likes Y" — different predicate, NOT contradiction
        // (handled by is_contradiction() in Layer 3 for like/hate pairs)
        let a = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
        };
        let b = FactTriple {
            subject: "user".into(),
            predicate: "likes".into(),
            object: "dark mode".into(),
        };
        assert!(!a.contradicts(&b)); // Different predicate
    }

    #[test]
    fn test_triple_contradicts_different_subject() {
        // Shouldn't happen in practice (subject is always "user"), but test for correctness
        let a = FactTriple {
            subject: "user".into(),
            predicate: "prefers".into(),
            object: "dark mode".into(),
        };
        let b = FactTriple {
            subject: "project".into(),
            predicate: "prefers".into(),
            object: "light mode".into(),
        };
        assert!(!a.contradicts(&b)); // Different subject
    }

    #[test]
    fn test_triple_cross_format_comparison() {
        // New third-person vs legacy first-person — same canonical predicate
        let a = extract_fact_triple("User's name is Lucas").unwrap();
        let b = extract_fact_triple("My name is João").unwrap();
        assert_eq!(a.predicate, "name is");
        assert_eq!(b.predicate, "name is");
        assert!(a.contradicts(&b)); // Same predicate, different object
    }

    #[test]
    fn test_extract_triple_case_insensitive() {
        // Prefix matching should be case-insensitive
        let triple = extract_fact_triple("USER PREFERS DARK MODE").unwrap();
        assert_eq!(triple.predicate, "prefers");
        // Object preserves original casing from the content string
    }

    // === Semantic Cascade Tests (Layer 3.5 reorder) ===

    /// Helper: simulate the semantic cascade logic used by Layer 3.5.
    /// Returns ("contradiction", predicate) | ("duplicate", "") | ("polarity", "")
    /// | ("neither", "")
    fn simulate_semantic_cascade(candidate: &str, existing: &str) -> (&'static str, String) {
        // Step 1: Triple-based disambiguation
        if let Some(candidate_triple) = extract_fact_triple(candidate)
            && let Some(existing_triple) = extract_fact_triple(existing)
        {
            if candidate_triple.contradicts(&existing_triple) {
                return ("contradiction", candidate_triple.predicate);
            }
            if candidate_triple.predicate == existing_triple.predicate
                && candidate_triple.object == existing_triple.object
            {
                return ("duplicate", String::new());
            }
            // Different predicate → fall through
        }
        // Step 2: Polarity opposition fallback
        if is_contradiction(candidate, existing) {
            return ("polarity", String::new());
        }
        ("neither", String::new())
    }

    #[test]
    fn test_cascade_preference_contradiction() {
        // "prefers dark mode" vs "prefers light mode" — same predicate
        let (action, predicate) =
            simulate_semantic_cascade("User prefers dark mode", "User prefers light mode");
        assert_eq!(action, "contradiction");
        assert_eq!(predicate, "prefers");
    }

    #[test]
    fn test_cascade_identity_contradiction() {
        // "name is Lucas" vs "name is Maria" — same predicate
        let (action, predicate) =
            simulate_semantic_cascade("User's name is Lucas", "User's name is Maria");
        assert_eq!(action, "contradiction");
        assert_eq!(predicate, "name is");
    }

    #[test]
    fn test_cascade_same_triple_duplicate() {
        // Same predicate, same object → semantic duplicate
        let (action, _) =
            simulate_semantic_cascade("User prefers dark mode", "User prefers dark mode");
        assert_eq!(action, "duplicate");
    }

    #[test]
    fn test_cascade_polarity_fallback_like_hate() {
        // "likes X" vs "hates X" — different predicates (triple skips),
        // but is_contradiction catches via like/hate polarity (with 3rd-person forms)
        let (action, _) =
            simulate_semantic_cascade("User likes verbose output", "User hates verbose output");
        assert_eq!(action, "polarity");
    }

    #[test]
    fn test_cascade_polarity_fallback_negation() {
        // "likes X" vs "doesn't like X" — triples extract different predicates,
        // but is_contradiction catches via opposite negation
        let (action, _) = simulate_semantic_cascade(
            "User likes verbose output",
            "User doesn't like verbose output",
        );
        assert_eq!(action, "polarity");
    }

    #[test]
    fn test_cascade_neither_different_topics() {
        // "likes Python" vs "prefers Rust" — different predicates, same-polarity, no contradiction
        let (action, _) = simulate_semantic_cascade("User likes Python", "User prefers Rust");
        assert_eq!(action, "neither");
    }

    #[test]
    fn test_cascade_neither_likes_same_object() {
        // "likes Python" vs "prefers Python" — different predicates, same object,
        // same polarity — NOT a contradiction (is_contradiction: both "like" polarity)
        let (action, _) = simulate_semantic_cascade("User likes Python", "User prefers Python");
        assert_eq!(action, "neither");
    }

    #[test]
    fn test_cascade_adverb_verb_different_topics() {
        // "really likes vim" vs "really likes emacs" — same adverb+accumulative verb
        // but "vim" and "emacs" share NO words → can coexist, NOT contradiction
        let (action, _) =
            simulate_semantic_cascade("User really likes vim", "User really likes emacs");
        assert_eq!(action, "neither"); // accumulative + no word overlap = coexist
    }

    #[test]
    fn test_cascade_adverb_verb_same_category() {
        // "really likes dark mode" vs "really likes light mode" — same adverb+accumulative verb
        // objects share "mode" → IS contradiction (same category, preference override)
        let (action, predicate) = simulate_semantic_cascade(
            "User really likes dark mode",
            "User really likes light mode",
        );
        assert_eq!(action, "contradiction");
        assert_eq!(predicate, "really likes");
    }

    #[test]
    fn test_cascade_negation_contradiction() {
        // "doesn't like verbose output" vs "doesn't like verbose errors"
        let (action, predicate) = simulate_semantic_cascade(
            "User doesn't like verbose output",
            "User doesn't like verbose errors",
        );
        assert_eq!(action, "contradiction");
        assert_eq!(predicate, "doesn't like");
    }

    #[test]
    fn test_cascade_location_change() {
        // "lives in São Paulo" vs "lives in Recife" — same predicate
        let (action, predicate) =
            simulate_semantic_cascade("User lives in São Paulo", "User lives in Recife");
        assert_eq!(action, "contradiction");
        assert_eq!(predicate, "lives in");
    }

    #[test]
    fn test_semantic_search_threshold_value() {
        // Verify threshold constant matches spec
        assert!((SEMANTIC_SEARCH_THRESHOLD - 0.70).abs() < f32::EPSILON);
        // Verify it's below the minimum contradiction cosine (0.7753)
        assert!(SEMANTIC_SEARCH_THRESHOLD < 0.77);
        // Verify it's above the maximum different-topic cosine (0.60)
        assert!(SEMANTIC_SEARCH_THRESHOLD > 0.60);
    }
}
