//! Feedback boost map computation and display sections for /context.
//!
//! Provides:
//! - `compute_feedback_boost_map` — queries feedback_signals and computes
//!   decay-weighted boost per item for RRF fusion
//! - `build_feedback_section` — formats basic feedback stats for /context
//! - `build_decay_section` — (V4) formats decay stats for /context
//!
//! # Design Decisions
//!
//! - Feedback metadata is NOT injected into the system prompt (Phase 1 is RRF-only)
//! - `build_feedback_section` and `build_decay_section` are pure formatting
//!   functions (no DB calls)
//! - `compute_feedback_boost_map` delegates decay calculation to `feedback::decay`

use chrono::{DateTime, Utc};

use std::collections::HashMap;
use std::str::FromStr;

use super::decay::compute_total_boost;
use super::types::{FeedbackSignal, FeedbackSignalType, FeedbackSource};
use crate::db::Database;

/// Compute feedback boost map for a set of content items.
///
/// For each item_id in `item_ids`, queries feedback_signals from the DB,
/// groups them by item, then delegates to `feedback::decay::compute_total_boost`
/// for decay-weighted accumulation with first-stage clamping.
///
/// # Arguments
/// * `db` - Database connection
/// * `item_ids` - Slice of content_item IDs to compute boosts for
/// * `now` - Current timestamp (for decay calculation)
///
/// # Returns
/// HashMap mapping item_id → boost value, only for items that have feedback.
/// Items with no feedback are absent from the map (implicit 0.0 boost).
#[allow(dead_code)] // Consumed by Tasks 7, 10 (RRF fusion + /context)
pub fn compute_feedback_boost_map(
    db: &Database,
    item_ids: &[i64],
    now: DateTime<Utc>,
) -> Result<HashMap<i64, f32>, String> {
    if item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Build a parameterized IN clause dynamically.
    // item_ids is bounded by retrieval limits, not user input, so this is safe.
    let placeholders: Vec<String> = item_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let in_clause = placeholders.join(",");

    let sql = format!(
        "SELECT item_id, signal_type, base_value, source, created_at, \
         correction_text, session_id \
         FROM feedback_signals \
         WHERE item_id IN ({}) \
         ORDER BY item_id",
        in_clause
    );

    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&sql)?;

        // Build parameter values from item_ids
        let param_values: Vec<rusqlite::types::Value> = item_ids
            .iter()
            .map(|id| rusqlite::types::Value::Integer(*id))
            .collect();

        // Group signals by item_id
        let mut signals_by_item: HashMap<i64, Vec<FeedbackSignal>> = HashMap::new();

        let mut rows =
            stmt.query(rusqlite::params_from_iter(param_values.iter()))?;

        while let Some(row) = rows.next()? {
            let item_id: i64 = row.get(0)?;
            let signal_type_str: String = row.get(1)?;
            let base_value: f32 = row.get(2)?;
            let source_str: String = row.get(3)?;
            let created_at: i64 = row.get(4)?;
            let correction_text: Option<String> = row.get(5)?;
            let session_id: Option<String> = row.get(6)?;

            // Parse signal type — skip malformed rows
            let signal_type = match FeedbackSignalType::from_str(&signal_type_str) {
                Ok(st) => st,
                Err(_) => continue,
            };

            // Parse source — skip malformed rows
            let source = match FeedbackSource::from_str(&source_str) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let signal = FeedbackSignal {
                item_id,
                session_id,
                signal_type,
                base_value,
                correction_text,
                source,
                created_at,
            };

            signals_by_item.entry(item_id).or_default().push(signal);
        }

        // Compute boost per item using feedback::decay::compute_total_boost
        let mut boost_map: HashMap<i64, f32> = HashMap::new();
        for (item_id, signals) in signals_by_item {
            let boost = compute_total_boost(&signals, now);
            boost_map.insert(item_id, boost);
        }

        Ok(boost_map)
    })
    .map_err(|e| format!("Database error computing feedback boost map: {}", e))
}

/// Build a feedback stats section for /context display.
///
/// This is a pure formatting function — it does NOT query the database.
///
/// # Format
/// ```text
/// 📊 Feedback: N signals, avg boost: +0.XX
/// ```
///
/// # Arguments
/// * `session_id` - Current session ID (reserved for future session-scoped stats)
/// * `signal_count` - Total number of feedback signals
/// * `avg_boost` - Average boost value across all items with feedback
#[allow(dead_code)] // Consumed by Task 7 (/context command)
pub fn build_feedback_section(session_id: &str, signal_count: usize, avg_boost: f32) -> String {
    // session_id is reserved for future session-scoped feedback display
    let _ = session_id;

    if signal_count == 0 {
        return "📊 Feedback: no signals".to_string();
    }

    let sign = if avg_boost >= 0.0 { "+" } else { "" };
    format!(
        "📊 Feedback: {} signals, avg boost: {}{:.2}",
        signal_count, sign, avg_boost
    )
}

/// Build a decay stats section for /context display (V4 ADDITION).
///
/// This is a pure formatting function — it does NOT query the database.
///
/// # Format
/// ```text
/// 📉 Decay: N items (M pruned), avg retention: 0.XX, K at risk
/// ```
///
/// # Arguments
/// * `total_items` - Total number of content items
/// * `pruned_count` - Number of items pruned by decay cycle
/// * `avg_retention` - Average retention score across all items
/// * `at_risk_count` - Number of items with retention below a danger threshold
#[allow(dead_code)] // Consumed by Task 7 (/context command, V4)
pub fn build_decay_section(
    total_items: usize,
    pruned_count: usize,
    avg_retention: f32,
    at_risk_count: usize,
) -> String {
    if total_items == 0 {
        return "📉 Decay: no items".to_string();
    }

    format!(
        "📉 Decay: {} items ({} pruned), avg retention: {:.2}, {} at risk",
        total_items, pruned_count, avg_retention, at_risk_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::decay::{
        HALF_LIFE_BAD, HALF_LIFE_CORRECTION, HALF_LIFE_GOOD, MAX_FEEDBACK_BOOST,
    };
    use crate::feedback::types::{FeedbackSource, FeedbackSignalType};
    use rusqlite::params;

    // === build_feedback_section tests ===

    #[test]
    fn test_build_feedback_section_no_signals() {
        let result = build_feedback_section("sess_abc", 0, 0.0);
        assert_eq!(result, "📊 Feedback: no signals");
    }

    #[test]
    fn test_build_feedback_section_positive_boost() {
        let result = build_feedback_section("sess_abc", 5, 0.35);
        assert_eq!(result, "📊 Feedback: 5 signals, avg boost: +0.35");
    }

    #[test]
    fn test_build_feedback_section_negative_boost() {
        let result = build_feedback_section("sess_abc", 3, -0.50);
        assert_eq!(result, "📊 Feedback: 3 signals, avg boost: -0.50");
    }

    #[test]
    fn test_build_feedback_section_zero_boost() {
        let result = build_feedback_section("sess_abc", 2, 0.0);
        assert_eq!(result, "📊 Feedback: 2 signals, avg boost: +0.00");
    }

    // === build_decay_section tests ===

    #[test]
    fn test_build_decay_section_no_items() {
        let result = build_decay_section(0, 0, 0.0, 0);
        assert_eq!(result, "📉 Decay: no items");
    }

    #[test]
    fn test_build_decay_section_normal() {
        let result = build_decay_section(100, 5, 0.78, 12);
        assert_eq!(
            result,
            "📉 Decay: 100 items (5 pruned), avg retention: 0.78, 12 at risk"
        );
    }

    #[test]
    fn test_build_decay_section_no_pruned() {
        let result = build_decay_section(50, 0, 0.95, 0);
        assert_eq!(
            result,
            "📉 Decay: 50 items (0 pruned), avg retention: 0.95, 0 at risk"
        );
    }

    #[test]
    fn test_build_decay_section_low_retention() {
        let result = build_decay_section(30, 10, 0.15, 8);
        assert_eq!(
            result,
            "📉 Decay: 30 items (10 pruned), avg retention: 0.15, 8 at risk"
        );
    }

    // === compute_feedback_boost_map tests ===

    #[test]
    fn test_compute_feedback_boost_map_empty_ids() {
        let db = Database::in_memory().expect("Failed to create in-memory DB");
        let now = Utc::now();
        let result = compute_feedback_boost_map(&db, &[], now);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_compute_feedback_boost_map_no_signals() {
        let db = Database::in_memory().expect("Failed to create in-memory DB");
        let now = Utc::now();
        let result = compute_feedback_boost_map(&db, &[1, 2, 3], now);
        assert!(result.is_ok());
        // No feedback signals exist in a fresh DB, so map should be empty
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_compute_feedback_boost_map_with_signals() {
        let db = Database::in_memory().expect("Failed to create in-memory DB");
        let now = Utc::now();

        // Insert a content item and feedback signals
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, content, importance, created_at, updated_at, last_accessed) \
                 VALUES ('message', 'test content', 0.5, ?1, ?1, ?1)",
                params![now.timestamp()],
            )?;
            let item_id = conn.last_insert_rowid();

            // User good signal: weight 1.0
            conn.execute(
                "INSERT INTO feedback_signals (item_id, session_id, signal_type, base_value, source, created_at) \
                 VALUES (?1, 'sess_test', 'good', 1.0, 'user', ?2)",
                params![item_id, now.timestamp()],
            )?;

            // LLM good signal: weight 0.3
            conn.execute(
                "INSERT INTO feedback_signals (item_id, session_id, signal_type, base_value, source, created_at) \
                 VALUES (?1, 'sess_test', 'good', 1.0, 'llm', ?2)",
                params![item_id, now.timestamp()],
            )?;

            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test data");

        let result =
            compute_feedback_boost_map(&db, &[1], now).expect("Failed to compute boost map");

        // Should have exactly one entry for item_id=1
        assert_eq!(result.len(), 1);
        let boost = result.get(&1).expect("Expected boost for item 1");

        // User good = 1.0 (base) * 1.0 (decay) * 1.0 (source) = 1.0
        // LLM good = 1.0 (base) * 1.0 (decay) * 0.3 (source) = 0.3
        // Total = 1.3
        let expected = FeedbackSignalType::Good.base_value() * FeedbackSource::User.weight_factor()
            + FeedbackSignalType::Good.base_value() * FeedbackSource::Llm.weight_factor();
        assert!(
            (boost - expected).abs() < 0.01,
            "Expected boost {}, got {}",
            expected,
            boost
        );
    }

    #[test]
    fn test_compute_feedback_boost_map_clamping() {
        let db = Database::in_memory().expect("Failed to create in-memory DB");
        let now = Utc::now();

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO content_items (content_type, content, importance, created_at, updated_at, last_accessed) \
                 VALUES ('message', 'test content', 0.5, ?1, ?1, ?1)",
                params![now.timestamp()],
            )?;
            let item_id = conn.last_insert_rowid();

            // Insert 3 user "good" signals = 3.0 unclamped, should clamp to 2.0
            for _ in 0..3 {
                conn.execute(
                    "INSERT INTO feedback_signals (item_id, session_id, signal_type, base_value, source, created_at) \
                     VALUES (?1, 'sess_test', 'good', 1.0, 'user', ?2)",
                    params![item_id, now.timestamp()],
                )?;
            }

            Ok::<(), rusqlite::Error>(())
        })
        .expect("Failed to insert test data");

        let result =
            compute_feedback_boost_map(&db, &[1], now).expect("Failed to compute boost map");
        let boost = result.get(&1).expect("Expected boost for item 1");

        // 3 * 1.0 = 3.0, clamped to MAX_FEEDBACK_BOOST (2.0)
        assert!(
            (boost - MAX_FEEDBACK_BOOST).abs() < 0.01,
            "Expected clamped boost {}, got {}",
            MAX_FEEDBACK_BOOST,
            boost
        );
    }

    #[test]
    fn test_decay_constants_match() {
        // Verify prompt.rs uses the same constants as decay.rs
        assert!((HALF_LIFE_GOOD - 30.0).abs() < f32::EPSILON);
        assert!((HALF_LIFE_BAD - 7.0).abs() < f32::EPSILON);
        assert!((HALF_LIFE_CORRECTION - 14.0).abs() < f32::EPSILON);
        assert!((MAX_FEEDBACK_BOOST - 2.0).abs() < f32::EPSILON);
    }
}