//! Decay calculations for the Content System
//!
//! Based on Ebbinghaus forgetting curve with access reinforcement.
//! Mirrors the facts system's decay pattern with content-specific half-lives.
//!
//! Content half-lives differ by type:
//! - Messages: 90 days (ephemeral conversational context)
//! - Notes: 60 days (personal notes, shorter-lived than documents)
//! - Documents: 120 days (imported reference material, longest retention)
//!
//! SQL operations are in crate::db::content_decay_ops.

/// Half-life for message content items (days)
pub(crate) const HALF_LIFE_MESSAGE: f32 = 90.0;

/// Half-life for note content items (days)
pub(crate) const HALF_LIFE_NOTE: f32 = 60.0;

/// Half-life for document content items (days)
pub(crate) const HALF_LIFE_DOCUMENT: f32 = 120.0;

/// Access boost per access (0.1 = 10%)
pub(crate) const CONTENT_ACCESS_BOOST: f32 = 0.1;

/// Maximum importance boost factor
pub(crate) const CONTENT_IMPORTANCE_BOOST: f32 = 0.5;

/// Minimum retention threshold for pruning (0.05 = 5%)
pub(crate) const MIN_CONTENT_RETENTION: f32 = 0.05;

/// Get the half-life for a content type.
///
/// Returns the Ebbinghaus half-life in days for the given content type.
/// Defaults to message half-life for unknown types.
pub(crate) fn get_content_half_life(content_type: &str) -> f32 {
    match content_type {
        "message" => HALF_LIFE_MESSAGE,
        "note" => HALF_LIFE_NOTE,
        "document" => HALF_LIFE_DOCUMENT,
        _ => HALF_LIFE_MESSAGE,
    }
}

/// Compute retention score for a content item.
///
/// Uses Ebbinghaus forgetting curve with access reinforcement:
/// R = 2^(-days_since / half_life) * importance_mult * access_mult
///
/// # Arguments
/// * `importance` - Importance score (0.0 to 1.0)
/// * `access_count` - Number of times the item has been accessed
/// * `content_type` - Content type ("message", "note", or "document")
/// * `last_accessed` - Unix epoch timestamp of last access
/// * `now` - Current Unix epoch timestamp
///
/// # Returns
/// Retention score between 0.0 and 1.0
pub(crate) fn compute_content_retention(
    importance: f32,
    access_count: u32,
    content_type: &str,
    last_accessed: i64,
    now: i64,
) -> f32 {
    let half_life = get_content_half_life(content_type);

    let seconds_per_day: f32 = 86400.0;
    let days_since_access = ((now - last_accessed) as f32) / seconds_per_day;

    // Exponential decay: R = 2^(-t / half_life)
    let decay = 2f32.powf(-days_since_access / half_life);

    // Importance multiplier (important items retain longer)
    // Mirrors facts pattern: 1.0 + importance * IMPORTANCE_BOOST
    let importance_mult = 1.0 + importance * CONTENT_IMPORTANCE_BOOST;

    // Access boost (frequently accessed items retain longer)
    // log2(access_count) gives diminishing returns — same as facts
    let access_mult = if access_count > 0 {
        1.0 + CONTENT_ACCESS_BOOST * (access_count as f32).log2().max(0.0)
    } else {
        1.0
    };

    (decay * importance_mult * access_mult).clamp(0.0, 1.0)
}

/// Check if a content item should be pruned.
///
/// Items with importance >= 0.8 are never pruned.
/// Items with retention below MIN_CONTENT_RETENTION are pruned.
///
/// # Arguments
/// * `importance` - Importance score (0.0 to 1.0)
/// * `retention` - Computed retention score
///
/// # Returns
/// true if the item should be pruned
pub(crate) fn should_prune_content(importance: f32, retention: f32) -> bool {
    // Never prune high-importance items
    if importance >= 0.8 {
        return false;
    }

    // Prune items below minimum retention
    retention < MIN_CONTENT_RETENTION
}

/// Statistics from running the content decay cycle
#[derive(Debug, Clone)]
pub struct ContentDecayStats {
    /// Number of content items pruned
    pub pruned: usize,
    /// Number of content items remaining
    pub remaining: usize,
    /// Average retention of remaining items
    pub avg_retention: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // === get_content_half_life ===

    #[test]
    fn test_get_content_half_life_message() {
        assert_eq!(get_content_half_life("message"), 90.0);
    }

    #[test]
    fn test_get_content_half_life_note() {
        assert_eq!(get_content_half_life("note"), 60.0);
    }

    #[test]
    fn test_get_content_half_life_document() {
        assert_eq!(get_content_half_life("document"), 120.0);
    }

    #[test]
    fn test_get_content_half_life_unknown_defaults_to_message() {
        assert_eq!(get_content_half_life("unknown"), 90.0);
    }

    // === compute_content_retention ===

    #[test]
    fn test_retention_new_message_is_near_one() {
        let now = chrono::Utc::now().timestamp();
        // Just accessed: days_since = 0
        let retention = compute_content_retention(0.5, 0, "message", now, now);
        assert!(
            retention > 0.99,
            "Retention should be > 0.99 for new message, got {}",
            retention
        );
    }

    #[test]
    fn test_retention_message_at_half_life() {
        let now = 1_000_000_000i64; // arbitrary fixed timestamp
        let half_life_seconds = (HALF_LIFE_MESSAGE * 86400.0) as i64;
        let last_accessed = now - half_life_seconds;

        // At half-life: base decay = 2^(-1) = 0.5
        // With importance 0.5: importance_mult = 1.0 + 0.5 * 0.5 = 1.25
        // With access_count 0: access_mult = 1.0
        // retention = 0.5 * 1.25 * 1.0 = 0.625
        let retention = compute_content_retention(0.5, 0, "message", last_accessed, now);
        assert!(
            (retention - 0.625).abs() < 0.02,
            "Retention at message half-life should be ~0.625, got {}",
            retention
        );
    }

    #[test]
    fn test_retention_note_at_half_life() {
        let now = 1_000_000_000i64;
        let half_life_seconds = (HALF_LIFE_NOTE * 86400.0) as i64;
        let last_accessed = now - half_life_seconds;

        // At half-life: base decay = 0.5, importance_mult = 1.25, access_mult = 1.0
        // retention = 0.5 * 1.25 = 0.625
        let retention = compute_content_retention(0.5, 0, "note", last_accessed, now);
        assert!(
            (retention - 0.625).abs() < 0.02,
            "Retention at note half-life should be ~0.625, got {}",
            retention
        );
    }

    #[test]
    fn test_retention_document_at_half_life() {
        let now = 1_000_000_000i64;
        let half_life_seconds = (HALF_LIFE_DOCUMENT * 86400.0) as i64;
        let last_accessed = now - half_life_seconds;

        // At half-life: base decay = 0.5, importance_mult = 1.25, access_mult = 1.0
        // retention = 0.5 * 1.25 = 0.625
        let retention = compute_content_retention(0.5, 0, "document", last_accessed, now);
        assert!(
            (retention - 0.625).abs() < 0.02,
            "Retention at document half-life should be ~0.625, got {}",
            retention
        );
    }

    #[test]
    fn test_retention_with_access_higher_than_without() {
        let now = 1_000_000_000i64;
        let days_ago = 60 * 86400; // 60 days ago
        let last_accessed = now - days_ago;

        let no_access = compute_content_retention(0.5, 0, "note", last_accessed, now);
        let with_access = compute_content_retention(0.5, 10, "note", last_accessed, now);

        assert!(
            with_access > no_access,
            "Retention with access ({}) should be > without ({})",
            with_access,
            no_access
        );
    }

    #[test]
    fn test_retention_very_old_item_is_low() {
        let now = 1_000_000_000i64;
        let one_year_ago = now - (365 * 86400);

        let retention = compute_content_retention(0.5, 0, "message", one_year_ago, now);
        assert!(
            retention < 0.1,
            "Retention for 1-year-old message should be < 0.1, got {}",
            retention
        );
    }

    #[test]
    fn test_retention_high_importance_retains_longer() {
        let now = 1_000_000_000i64;
        let days_ago = 90 * 86400;
        let last_accessed = now - days_ago;

        let low_importance = compute_content_retention(0.2, 0, "message", last_accessed, now);
        let high_importance = compute_content_retention(0.8, 0, "message", last_accessed, now);

        assert!(
            high_importance > low_importance,
            "High importance retention ({}) should be > low ({})",
            high_importance,
            low_importance
        );
    }

    // === should_prune_content ===

    #[test]
    fn test_should_prune_high_importance_never() {
        // importance >= 0.8 → never prune
        assert!(!should_prune_content(0.8, 0.0));
        assert!(!should_prune_content(0.9, 0.01));
        assert!(!should_prune_content(1.0, 0.0));
    }

    #[test]
    fn test_should_prune_below_threshold() {
        // retention < 0.05 → prune
        assert!(should_prune_content(0.1, 0.04));
        assert!(should_prune_content(0.5, 0.01));
        assert!(should_prune_content(0.0, 0.0));
    }

    #[test]
    fn test_should_not_prune_above_threshold() {
        // retention >= 0.05 and importance < 0.8 → don't prune
        assert!(!should_prune_content(0.1, 0.06));
        assert!(!should_prune_content(0.5, 0.5));
        assert!(!should_prune_content(0.7, 0.1));
    }
}
