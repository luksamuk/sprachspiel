//! Decay calculations for the Feedback Signal System
//!
//! Implements time-based exponential decay for feedback signals using
//! the half-life formula: `2^(-days_since / half_life)` (ADR-002).
//!
//! First-stage clamp is ±`MAX_FEEDBACK_BOOST` (2.0) on accumulated
//! boost per item. The second-stage clamp (0.1–3.0) is applied later
//! as an RRF multiplier (Task 10).

use super::types::{FeedbackSignal, FeedbackSignalType};
use chrono::{DateTime, Utc};

/// Half-life for Good signals (days)
#[allow(dead_code)] // Consumed by db/prompt (Tasks 4-6)
pub const HALF_LIFE_GOOD: f32 = 30.0;

/// Half-life for Bad signals (days)
#[allow(dead_code)] // Consumed by db/prompt (Tasks 4-6)
pub const HALF_LIFE_BAD: f32 = 7.0;

/// Half-life for Correction signals (days)
#[allow(dead_code)] // Consumed by db/prompt (Tasks 4-6)
pub const HALF_LIFE_CORRECTION: f32 = 14.0;

/// Maximum accumulated boost per item (first-stage clamp, ±2.0)
#[allow(dead_code)] // Consumed by db/prompt (Tasks 4-6)
pub const MAX_FEEDBACK_BOOST: f32 = 2.0;

/// Returns the half-life for a signal type.
#[allow(dead_code)] // Consumed by prompt (Tasks 6+)
fn half_life(signal_type: FeedbackSignalType) -> f32 {
    match signal_type {
        FeedbackSignalType::Good => HALF_LIFE_GOOD,
        FeedbackSignalType::Bad => HALF_LIFE_BAD,
        FeedbackSignalType::Correction => HALF_LIFE_CORRECTION,
    }
}

/// Compute the decayed weight of a single feedback signal.
///
/// Formula: `base_value * 2^(-days_since / half_life) * source.weight_factor()`
///
/// # Arguments
/// * `signal` - The feedback signal
/// * `now` - Current timestamp
///
/// # Returns
/// Decayed weight value (positive for Good/Correction, negative for Bad)
#[allow(dead_code)] // Consumed by db/prompt (Tasks 4-6)
pub fn decayed_weight(signal: &FeedbackSignal, now: DateTime<Utc>) -> f32 {
    let signal_time = DateTime::<Utc>::from_timestamp(signal.created_at, 0).unwrap_or_default();
    let days_since = (now - signal_time).num_days() as f32;
    let hl = half_life(signal.signal_type);
    let decay_factor = 2f32.powf(-days_since / hl);
    signal.base_value * decay_factor * signal.source.weight_factor()
}

/// Compute the total accumulated boost for an item from all its signals.
///
/// Sums individual `decayed_weight()` values, then clamps to
/// `[-MAX_FEEDBACK_BOOST, MAX_FEEDBACK_BOOST]` (first-stage clamp).
///
/// # Arguments
/// * `signals` - All feedback signals for a single item
/// * `now` - Current timestamp
///
/// # Returns
/// Total boost clamped to ±2.0
#[allow(dead_code)] // Consumed by db/prompt (Tasks 4-6)
pub fn compute_total_boost(signals: &[FeedbackSignal], now: DateTime<Utc>) -> f32 {
    let total: f32 = signals.iter().map(|s| decayed_weight(s, now)).sum();
    total.clamp(-MAX_FEEDBACK_BOOST, MAX_FEEDBACK_BOOST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::types::{FeedbackSignalType, FeedbackSource};

    /// Helper: create a signal at a known unix timestamp.
    fn make_signal(
        signal_type: FeedbackSignalType,
        source: FeedbackSource,
        days_ago: i64,
    ) -> FeedbackSignal {
        let now = Utc::now();
        let created = now - chrono::Duration::days(days_ago);
        FeedbackSignal {
            item_id: 1,
            session_id: None,
            signal_type,
            base_value: signal_type.base_value(),
            correction_text: None,
            source,
            created_at: created.timestamp(),
        }
    }

    #[test]
    fn test_good_signal_half_life() {
        // Good signal at 30 days → decayed weight ≈ 0.5
        let signal = make_signal(FeedbackSignalType::Good, FeedbackSource::User, 30);
        let now = Utc::now();
        let weight = decayed_weight(&signal, now);
        assert!(
            (weight - 0.5).abs() < 0.01,
            "Good signal at 30 days should be ~0.5, got {}",
            weight
        );
    }

    #[test]
    fn test_bad_signal_half_life() {
        // Bad signal at 7 days → decayed weight ≈ -0.5
        let signal = make_signal(FeedbackSignalType::Bad, FeedbackSource::User, 7);
        let now = Utc::now();
        let weight = decayed_weight(&signal, now);
        assert!(
            (weight - (-0.5)).abs() < 0.01,
            "Bad signal at 7 days should be ~-0.5, got {}",
            weight
        );
    }

    #[test]
    fn test_correction_signal_half_life() {
        // Correction signal at 14 days → decayed weight ≈ 0.5
        let signal = make_signal(FeedbackSignalType::Correction, FeedbackSource::User, 14);
        let now = Utc::now();
        let weight = decayed_weight(&signal, now);
        assert!(
            (weight - 0.5).abs() < 0.01,
            "Correction signal at 14 days should be ~0.5, got {}",
            weight
        );
    }

    #[test]
    fn test_llm_vs_user_weight() {
        // LLM signal = 0.3 * user signal (same type, same age)
        let user_signal = make_signal(FeedbackSignalType::Good, FeedbackSource::User, 10);
        let llm_signal = make_signal(FeedbackSignalType::Good, FeedbackSource::Llm, 10);
        let now = Utc::now();
        let user_weight = decayed_weight(&user_signal, now);
        let llm_weight = decayed_weight(&llm_signal, now);
        let ratio = llm_weight / user_weight;
        assert!(
            (ratio - 0.3).abs() < 0.01,
            "LLM weight should be 0.3 * user weight, got ratio {}",
            ratio
        );
    }

    #[test]
    fn test_accumulation_capped_at_max() {
        // 5 good user signals at age 0 → total = 5.0, capped at 2.0
        let now = Utc::now();
        let signals: Vec<FeedbackSignal> = (0..5)
            .map(|_| make_signal(FeedbackSignalType::Good, FeedbackSource::User, 0))
            .collect();
        let boost = compute_total_boost(&signals, now);
        assert!(
            (boost - 2.0).abs() < f32::EPSILON,
            "5 fresh good signals should be clamped to 2.0, got {}",
            boost
        );
    }

    #[test]
    fn test_formula_matches_facts_system() {
        // Verify the formula matches facts/decay.rs pattern: 2^(-days/half_life)
        let signal = make_signal(FeedbackSignalType::Good, FeedbackSource::User, 30);
        let now = Utc::now();
        let weight = decayed_weight(&signal, now);
        // Manual calculation: base=1.0, decay=2^(-30/30)=0.5, source=1.0
        let expected = 1.0 * 2f32.powf(-30.0_f32 / 30.0) * 1.0;
        assert!(
            (weight - expected).abs() < f32::EPSILON,
            "Formula should match 2^(-days/half_life), got {} vs expected {}",
            weight,
            expected
        );
    }

    #[test]
    fn test_fresh_signal_full_weight() {
        // Signal at age 0 should have full weight
        let signal = make_signal(FeedbackSignalType::Good, FeedbackSource::User, 0);
        let now = Utc::now();
        let weight = decayed_weight(&signal, now);
        assert!(
            (weight - 1.0).abs() < 0.01,
            "Fresh good user signal should be ~1.0, got {}",
            weight
        );
    }

    #[test]
    fn test_negative_clamp() {
        // 5 bad user signals at age 0 → total = -5.0, clamped to -2.0
        let now = Utc::now();
        let signals: Vec<FeedbackSignal> = (0..5)
            .map(|_| make_signal(FeedbackSignalType::Bad, FeedbackSource::User, 0))
            .collect();
        let boost = compute_total_boost(&signals, now);
        assert!(
            (boost - (-2.0)).abs() < f32::EPSILON,
            "5 fresh bad signals should be clamped to -2.0, got {}",
            boost
        );
    }

    #[test]
    fn test_empty_signals_zero_boost() {
        let now = Utc::now();
        let boost = compute_total_boost(&[], now);
        assert!(
            boost.abs() < f32::EPSILON,
            "Empty signals should give 0.0 boost, got {}",
            boost
        );
    }

    #[test]
    fn test_mixed_signals_normal_accumulation() {
        // 1 Good + 1 Bad at age 0 from user → 1.0 + (-1.0) = 0.0
        let now = Utc::now();
        let signals = vec![
            make_signal(FeedbackSignalType::Good, FeedbackSource::User, 0),
            make_signal(FeedbackSignalType::Bad, FeedbackSource::User, 0),
        ];
        let boost = compute_total_boost(&signals, now);
        assert!(
            boost.abs() < 0.01,
            "1 Good + 1 Bad at age 0 should cancel to ~0.0, got {}",
            boost
        );
    }
}
