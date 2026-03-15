//! Decay calculations for the Factual Memory System
//!
//! Based on Ebbinghaus forgetting curve with access reinforcement.

use super::types::{Category, Fact};
use chrono::{DateTime, Utc};

/// Half-life for preference facts (days)
pub const HALF_LIFE_PREFERENCE: f32 = 180.0;

/// Half-life for regular facts (days)
pub const HALF_LIFE_FACT: f32 = 30.0;

/// Access boost per access (0.1 = 10%)
pub const ACCESS_BOOST: f32 = 0.1;

/// Minimum retention threshold for pruning (0.05 = 5%)
pub const MIN_RETENTION: f32 = 0.05;

/// Maximum importance boost factor
pub const IMPORTANCE_BOOST: f32 = 0.5;

/// Get the half-life for a category
pub fn get_half_life(category: Category) -> f32 {
    match category {
        Category::Preference => HALF_LIFE_PREFERENCE,
        Category::Fact => HALF_LIFE_FACT,
    }
}

/// Compute retention score for a fact.
///
/// Uses Ebbinghaus forgetting curve with access reinforcement:
/// R = 2^(-t/half_life) * importance_mult * access_mult
///
/// # Arguments
/// * `fact` - The fact to compute retention for
/// * `now` - Current timestamp
///
/// # Returns
/// Retention score between 0.0 and 1.0
pub fn compute_retention(fact: &Fact, now: DateTime<Utc>) -> f32 {
    let half_life = get_half_life(fact.category);

    let days_since_access = (now - fact.last_accessed).num_days() as f32;

    // Exponential decay: R = 2^(-t / half_life)
    let decay = 2f32.powf(-days_since_access / half_life);

    // Importance multiplier (important facts retain longer)
    let importance_mult = 1.0 + fact.importance * IMPORTANCE_BOOST;

    // Access boost (frequently accessed facts retain longer)
    // log2(access_count) gives diminishing returns
    let access_mult = if fact.access_count > 0 {
        1.0 + ACCESS_BOOST * (fact.access_count as f32).log2().max(0.0)
    } else {
        1.0
    };

    (decay * importance_mult * access_mult).min(1.0).max(0.0)
}

/// Check if a fact should be pruned.
///
/// Facts below MIN_RETENTION are pruned, except:
/// - High-importance preferences (importance >= 0.8) are never pruned
///
/// # Arguments
/// * `fact` - The fact to check
/// * `now` - Current timestamp
///
/// # Returns
/// true if the fact should be pruned
pub fn should_prune(fact: &Fact, now: DateTime<Utc>) -> bool {
    // Never prune high-importance preferences
    if fact.category == Category::Preference && fact.importance >= 0.8 {
        return false;
    }

    // Already invalidated (soft deleted)
    if fact.invalidated_at.is_some() {
        return false;
    }

    compute_retention(fact, now) < MIN_RETENTION
}

/// Update fact access (increment count and update timestamp).
pub fn on_fact_access(fact: &mut Fact) {
    fact.access_count += 1;
    fact.last_accessed = Utc::now();

    // Optionally boost importance on access
    fact.importance = (fact.importance + 0.05).min(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_fact(category: Category, days_old: i64, access_count: u32) -> Fact {
        let now = Utc::now();
        let created_at = now - chrono::Duration::days(days_old);

        Fact {
            id: 1,
            scope: super::super::types::Scope::Project,
            category,
            content: "Test fact".to_string(),
            importance: 0.5,
            access_count,
            decay_score: 1.0,
            created_at,
            last_accessed: created_at,
            source: super::super::types::Source::User,
            invalidated_at: None,
            project_id: Some("test".to_string()),
        }
    }

    #[test]
    fn test_get_half_life() {
        assert_eq!(get_half_life(Category::Preference), 180.0);
        assert_eq!(get_half_life(Category::Fact), 30.0);
    }

    #[test]
    fn test_compute_retention_new_fact() {
        let fact = create_test_fact(Category::Fact, 0, 0);
        let now = Utc::now();

        // New fact should have retention close to 1.0
        let retention = compute_retention(&fact, now);
        assert!(
            retention > 0.99,
            "Retention should be > 0.99 for new fact, got {}",
            retention
        );
    }

    #[test]
    fn test_compute_retention_old_fact() {
        let fact = create_test_fact(Category::Fact, 60, 0); // 60 days old
        let now = Utc::now();

        // 60-day old fact with 30-day half-life
        // Base decay = 2^(-60/30) = 2^-2 = 0.25
        // With importance multiplier (1.0 + 0.5 * 0.5 = 1.25)
        // Retention = 0.25 * 1.25 = 0.3125
        let retention = compute_retention(&fact, now);
        assert!(
            retention < 0.35,
            "Retention should be < 0.35 for 60-day old fact, got {}",
            retention
        );
        assert!(
            retention > 0.2,
            "Retention should be > 0.2 for 60-day old fact, got {}",
            retention
        );
    }

    #[test]
    fn test_compute_retention_with_access() {
        let mut fact = create_test_fact(Category::Fact, 60, 10); // 60 days old, 10 accesses
        fact.importance = 0.5;

        let now = Utc::now();

        // With access boost: retention should be higher than without
        let retention = compute_retention(&fact, now);
        assert!(retention > 0.25, "Retention with access should be higher");
    }

    #[test]
    fn test_should_prune_new_fact() {
        let fact = create_test_fact(Category::Fact, 0, 0);
        let now = Utc::now();

        // New fact should not be pruned
        assert!(!should_prune(&fact, now));
    }

    #[test]
    fn test_should_prune_old_fact() {
        let fact = create_test_fact(Category::Fact, 365, 0); // 1 year old
        let now = Utc::now();

        // Very old fact with 30-day half-life should be pruned
        assert!(should_prune(&fact, now));
    }

    #[test]
    fn test_never_prune_high_importance_preference() {
        let mut fact = create_test_fact(Category::Preference, 3650, 0); // 10 years old
        fact.importance = 0.9; // High importance

        let now = Utc::now();

        // High-importance preference should never be pruned
        assert!(!should_prune(&fact, now));
    }

    #[test]
    fn test_preference_longer_retention() {
        let fact_fact = create_test_fact(Category::Fact, 365, 0);
        let fact_preference = create_test_fact(Category::Preference, 365, 0);

        let now = Utc::now();

        // Preference should retain longer than fact with same age
        let retention_fact = compute_retention(&fact_fact, now);
        let retention_preference = compute_retention(&fact_preference, now);

        assert!(
            retention_preference > retention_fact,
            "Preference retention ({}) should be > fact retention ({})",
            retention_preference,
            retention_fact
        );
    }

    #[test]
    fn test_on_fact_access() {
        let mut fact = create_test_fact(Category::Fact, 30, 5);
        let old_importance = fact.importance;

        on_fact_access(&mut fact);

        assert_eq!(fact.access_count, 6); // Incremented
        assert!(fact.importance > old_importance); // Boosted
        assert!(fact.importance <= 1.0); // Capped at 1.0
    }
}
