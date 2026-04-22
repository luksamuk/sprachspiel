//! Database operations for feedback signals
//!
//! Provides insert, retrieval, and boost computation for the Feedback Signal System.
//! Feedback signals target content_items with content_type='message' only (ADR-003).

use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::str::FromStr;

use crate::feedback::types::{FeedbackSignal, FeedbackSignalType, FeedbackSource};

/// Half-life in days for each signal type (inline decay, refactored to feedback::decay in Task 6)
fn half_life_days(signal_type: FeedbackSignalType) -> f32 {
    match signal_type {
        FeedbackSignalType::Good => 30.0,
        FeedbackSignalType::Bad => 7.0,
        FeedbackSignalType::Correction => 14.0,
    }
}

/// Insert a feedback signal into the database.
///
/// Validates that the target item exists and has content_type='message' (ADR-003).
/// Returns the inserted row ID on success.
#[allow(clippy::too_many_arguments)] // 8 params: unavoidable for feedback insert
pub fn insert_feedback_signal(
    conn: &Connection,
    item_id: i64,
    session_id: Option<&str>,
    signal_type: FeedbackSignalType,
    base_value: f32,
    correction_text: Option<&str>,
    source: FeedbackSource,
    created_at: i64,
) -> Result<i64, String> {
    // Validate content_type = 'message' (ADR-003)
    let content_type: Option<String> = match conn.query_row(
        "SELECT content_type FROM content_items WHERE id = ?1",
        params![item_id],
        |row| row.get(0),
    ) {
        Ok(ct) => Some(ct),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => {
            return Err(format!("Error checking content item {}: {}", item_id, e));
        }
    };

    match content_type {
        None => {
            return Err(format!(
                "Error: Content item with id {} does not exist. Cannot add feedback to a non-existent item.",
                item_id
            ));
        }
        Some(ct) if ct != "message" => {
            return Err(format!(
                "Error: Feedback is only allowed on messages (content_type='message'), but item {} has content_type='{}'. Use /fact or /note commands for non-message content.",
                item_id, ct
            ));
        }
        _ => {}
    }

    conn.execute(
        "INSERT INTO feedback_signals (item_id, session_id, signal_type, base_value, correction_text, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            item_id,
            session_id,
            signal_type.as_str(),
            base_value,
            correction_text,
            source.as_str(),
            created_at,
        ],
    )
    .map_err(|e| format!("Error inserting feedback signal for item {}: {}", item_id, e))?;

    Ok(conn.last_insert_rowid())
}

/// Retrieve all feedback signals for a specific item.
///
/// Returns signals ordered by creation time (oldest first).
#[allow(dead_code)] // Used by tests only
pub fn get_feedback_signals_for_item(
    conn: &Connection,
    item_id: i64,
) -> Result<Vec<FeedbackSignal>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT item_id, session_id, signal_type, base_value, correction_text, source, created_at
             FROM feedback_signals
             WHERE item_id = ?1
             ORDER BY created_at ASC",
        )
        .map_err(|e| format!("Error preparing feedback query for item {}: {}", item_id, e))?;

    let rows = stmt
        .query_map(params![item_id], |row| {
            let signal_type_str: String = row.get(2)?;
            let source_str: String = row.get(5)?;

            let signal_type = FeedbackSignalType::from_str(&signal_type_str)
                .map_err(rusqlite::Error::InvalidParameterName)?;
            let source = FeedbackSource::from_str(&source_str)
                .map_err(rusqlite::Error::InvalidParameterName)?;

            Ok(FeedbackSignal {
                item_id: row.get(0)?,
                session_id: row.get(1)?,
                signal_type,
                base_value: row.get(3)?,
                correction_text: row.get(4)?,
                source,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| {
            format!(
                "Error querying feedback signals for item {}: {}",
                item_id, e
            )
        })?;

    let mut results = Vec::new();
    for row in rows {
        let signal = row.map_err(|e| format!("Error reading feedback signal row: {}", e))?;
        results.push(signal);
    }

    Ok(results)
}

/// Compute decayed feedback boost for multiple items.
///
/// For each item_id, sums up all feedback signals with exponential decay applied:
/// `decay = 2^(-days_since / half_life)` where half_life depends on signal_type,
/// then applies `source.weight_factor()`, and clamps the total to [-2.0, 2.0].
///
/// Items with no feedback signals get no entry in the returned map.
pub fn compute_feedback_boost(
    conn: &Connection,
    item_ids: &[i64],
    now: i64,
) -> Result<HashMap<i64, f32>, String> {
    if item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Build parameterized IN clause
    let placeholders: Vec<String> = item_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT item_id, signal_type, base_value, source, created_at
         FROM feedback_signals
         WHERE item_id IN ({})",
        placeholders.join(", ")
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Error preparing feedback boost query: {}", e))?;

    let params: Vec<Box<dyn rusqlite::types::ToSql>> = item_ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
        .collect();

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            let signal_type_str: String = row.get(1)?;
            let source_str: String = row.get(3)?;
            Ok((
                row.get::<_, i64>(0)?, // item_id
                signal_type_str,       // signal_type
                row.get::<_, f32>(2)?, // base_value
                source_str,            // source
                row.get::<_, i64>(4)?, // created_at
            ))
        })
        .map_err(|e| format!("Error executing feedback boost query: {}", e))?;

    let mut boosts: HashMap<i64, f32> = HashMap::new();

    for row in rows {
        let (item_id, signal_type_str, base_value, source_str, created_at) =
            row.map_err(|e| format!("Error reading feedback boost row: {}", e))?;

        let signal_type = match FeedbackSignalType::from_str(&signal_type_str) {
            Ok(st) => st,
            Err(e) => {
                // Skip invalid signal types rather than failing the whole computation
                eprintln!(
                    "Warning: skipping invalid signal_type '{}': {}",
                    signal_type_str, e
                );
                continue;
            }
        };

        let source = match FeedbackSource::from_str(&source_str) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: skipping invalid source '{}': {}", source_str, e);
                continue;
            }
        };

        // Inline decay computation (refactored to feedback::decay in Task 6)
        let seconds_per_day: f64 = 86400.0;
        let days_since = ((now - created_at) as f64 / seconds_per_day).max(0.0) as f32;
        let hl = half_life_days(signal_type);
        let decay = 2f32.powf(-days_since / hl);
        let weighted = base_value * decay * source.weight_factor();

        *boosts.entry(item_id).or_insert(0.0) += weighted;
    }

    // Clamp each total to [-2.0, 2.0]
    for boost in boosts.values_mut() {
        *boost = boost.clamp(-2.0, 2.0);
    }

    Ok(boosts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Helper: insert a content item with content_type='message' and return its ID
    fn insert_message_item(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('message', 'test message', 0.5, 1.0, 1713600000, 1713600000, 1713600000)",
            rusqlite::params![],
        )
        .expect("insert message item");
        conn.last_insert_rowid()
    }

    /// Helper: insert a content item with content_type='note' and return its ID
    fn insert_note_item(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('note', 'test note', 0.5, 1.0, 1713600000, 1713600000, 1713600000)",
            rusqlite::params![],
        )
        .expect("insert note item");
        conn.last_insert_rowid()
    }

    /// Create an in-memory Database and run a test closure with a Connection.
    fn with_test_db<F, R>(f: F) -> R
    where
        F: FnOnce(&Connection) -> R,
    {
        let db = Database::in_memory().expect("Failed to create in-memory DB");
        db.with_connection(|conn| Ok(f(conn)))
            .expect("with_connection failed")
    }

    #[test]
    fn test_insert_feedback_signal_good() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let result = insert_feedback_signal(
                conn,
                item_id,
                Some("sess_test"),
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                1713600000,
            );
            assert!(result.is_ok(), "Expected success, got: {:?}", result);
            let id = result.unwrap();
            assert!(id > 0, "Expected positive row ID, got: {}", id);
        });
    }

    #[test]
    fn test_insert_feedback_signal_bad() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let result = insert_feedback_signal(
                conn,
                item_id,
                None,
                FeedbackSignalType::Bad,
                -1.0,
                None,
                FeedbackSource::User,
                1713600000,
            );
            assert!(result.is_ok(), "Expected success, got: {:?}", result);
        });
    }

    #[test]
    fn test_insert_feedback_signal_correction_with_text() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let result = insert_feedback_signal(
                conn,
                item_id,
                Some("sess_abc"),
                FeedbackSignalType::Correction,
                1.0,
                Some("The capital is Canberra, not Sydney"),
                FeedbackSource::Llm,
                1713600000,
            );
            assert!(result.is_ok(), "Expected success, got: {:?}", result);
        });
    }

    #[test]
    fn test_insert_rejects_non_message_content_type() {
        with_test_db(|conn| {
            let note_id = insert_note_item(conn);
            let result = insert_feedback_signal(
                conn,
                note_id,
                None,
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                1713600000,
            );
            assert!(result.is_err(), "Expected error for non-message item");
            let err = result.unwrap_err();
            assert!(
                err.contains("content_type='message'"),
                "Error should mention message constraint, got: {}",
                err
            );
            assert!(
                err.contains("note"),
                "Error should mention note content_type, got: {}",
                err
            );
        });
    }

    #[test]
    fn test_insert_rejects_nonexistent_item() {
        with_test_db(|conn| {
            let result = insert_feedback_signal(
                conn,
                99999, // nonexistent item
                None,
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                1713600000,
            );
            assert!(result.is_err(), "Expected error for nonexistent item");
            let err = result.unwrap_err();
            assert!(
                err.contains("does not exist"),
                "Error should mention nonexistent item, got: {}",
                err
            );
        });
    }

    #[test]
    fn test_get_feedback_signals_for_item() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            insert_feedback_signal(
                conn,
                item_id,
                Some("sess_1"),
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                1713600000,
            )
            .unwrap();
            insert_feedback_signal(
                conn,
                item_id,
                Some("sess_1"),
                FeedbackSignalType::Bad,
                -1.0,
                None,
                FeedbackSource::User,
                1713600100,
            )
            .unwrap();

            let signals = get_feedback_signals_for_item(conn, item_id).unwrap();
            assert_eq!(signals.len(), 2);
            assert_eq!(signals[0].signal_type, FeedbackSignalType::Good);
            assert_eq!(signals[1].signal_type, FeedbackSignalType::Bad);
            assert_eq!(signals[0].item_id, item_id);
            assert_eq!(signals[0].session_id, Some("sess_1".to_string()));
            assert_eq!(signals[0].source, FeedbackSource::User);
        });
    }

    #[test]
    fn test_get_feedback_signals_empty() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let signals = get_feedback_signals_for_item(conn, item_id).unwrap();
            assert!(signals.is_empty());
        });
    }

    #[test]
    fn test_compute_feedback_boost_empty_ids() {
        with_test_db(|conn| {
            let boosts = compute_feedback_boost(conn, &[], 1713600000).unwrap();
            assert!(boosts.is_empty());
        });
    }

    #[test]
    fn test_compute_feedback_boost_no_signals() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let boosts = compute_feedback_boost(conn, &[item_id], 1713600000).unwrap();
            assert!(boosts.is_empty(), "No signals means no entries in map");
        });
    }

    #[test]
    fn test_compute_feedback_boost_with_decay() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            insert_feedback_signal(
                conn,
                item_id,
                None,
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                created_at,
            )
            .unwrap();

            // At the same time as creation, decay = 2^0 = 1.0, weight = 1.0
            let boosts = compute_feedback_boost(conn, &[item_id], created_at).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (boost - 1.0).abs() < 0.01,
                "Expected ~1.0 for fresh Good signal, got: {}",
                boost
            );

            // 30 days later (one half-life for Good), decay = 0.5, boost = 0.5
            let now_30d = created_at + 30 * 86400;
            let boosts = compute_feedback_boost(conn, &[item_id], now_30d).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (boost - 0.5).abs() < 0.01,
                "Expected ~0.5 after one Good half-life, got: {}",
                boost
            );

            // 60 days later (two half-lives for Good), decay = 0.25, boost = 0.25
            let now_60d = created_at + 60 * 86400;
            let boosts = compute_feedback_boost(conn, &[item_id], now_60d).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (boost - 0.25).abs() < 0.01,
                "Expected ~0.25 after two Good half-lives, got: {}",
                boost
            );
        });
    }

    #[test]
    fn test_compute_feedback_boost_bad_signal_decay() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            insert_feedback_signal(
                conn,
                item_id,
                None,
                FeedbackSignalType::Bad,
                -1.0,
                None,
                FeedbackSource::User,
                created_at,
            )
            .unwrap();

            // 7 days later (one half-life for Bad), decay = 0.5, boost = -0.5
            let now_7d = created_at + 7 * 86400;
            let boosts = compute_feedback_boost(conn, &[item_id], now_7d).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (boost - (-0.5)).abs() < 0.01,
                "Expected ~-0.5 after one Bad half-life, got: {}",
                boost
            );
        });
    }

    #[test]
    fn test_compute_feedback_boost_correction_decay() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            insert_feedback_signal(
                conn,
                item_id,
                None,
                FeedbackSignalType::Correction,
                1.0,
                Some("Fix the capital"),
                FeedbackSource::Llm,
                created_at,
            )
            .unwrap();

            // 14 days later (one half-life for Correction), decay = 0.5
            // Llm weight = 0.3, boost = 1.0 * 0.5 * 0.3 = 0.15
            let now_14d = created_at + 14 * 86400;
            let boosts = compute_feedback_boost(conn, &[item_id], now_14d).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (boost - 0.15).abs() < 0.01,
                "Expected ~0.15 after one Correction half-life with Llm weight, got: {}",
                boost
            );
        });
    }

    #[test]
    fn test_compute_feedback_boost_multiple_signals_accumulate() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            // Insert a Good and a Bad signal at the same time
            insert_feedback_signal(
                conn,
                item_id,
                None,
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                created_at,
            )
            .unwrap();
            insert_feedback_signal(
                conn,
                item_id,
                None,
                FeedbackSignalType::Bad,
                -1.0,
                None,
                FeedbackSource::User,
                created_at,
            )
            .unwrap();
            // Both at full decay: 1.0 + (-1.0) = 0.0
            let boosts = compute_feedback_boost(conn, &[item_id], created_at).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                boost.abs() < 0.01,
                "Expected ~0.0 for equal Good+Bad, got: {}",
                boost
            );
        });
    }

    #[test]
    fn test_compute_feedback_boost_clamp_at_max() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            // Insert 5 Good User signals at same time: 5.0, should clamp to 2.0
            for _ in 0..5 {
                insert_feedback_signal(
                    conn,
                    item_id,
                    None,
                    FeedbackSignalType::Good,
                    1.0,
                    None,
                    FeedbackSource::User,
                    created_at,
                )
                .unwrap();
            }
            let boosts = compute_feedback_boost(conn, &[item_id], created_at).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (*boost - 2.0).abs() < 0.01,
                "Expected clamped to 2.0, got: {}",
                boost
            );
        });
    }

    #[test]
    fn test_compute_feedback_boost_clamp_at_min() {
        with_test_db(|conn| {
            let item_id = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            // Insert 5 Bad User signals: -5.0, should clamp to -2.0
            for _ in 0..5 {
                insert_feedback_signal(
                    conn,
                    item_id,
                    None,
                    FeedbackSignalType::Bad,
                    -1.0,
                    None,
                    FeedbackSource::User,
                    created_at,
                )
                .unwrap();
            }
            let boosts = compute_feedback_boost(conn, &[item_id], created_at).unwrap();
            let boost = boosts.get(&item_id).expect("should have boost");
            assert!(
                (*boost - (-2.0)).abs() < 0.01,
                "Expected clamped to -2.0, got: {}",
                boost
            );
        });
    }

    #[test]
    fn test_compute_feedback_boost_multiple_items() {
        with_test_db(|conn| {
            let item_a = insert_message_item(conn);
            let item_b = insert_message_item(conn);
            let created_at: i64 = 1713600000;
            insert_feedback_signal(
                conn,
                item_a,
                None,
                FeedbackSignalType::Good,
                1.0,
                None,
                FeedbackSource::User,
                created_at,
            )
            .unwrap();
            insert_feedback_signal(
                conn,
                item_b,
                None,
                FeedbackSignalType::Bad,
                -1.0,
                None,
                FeedbackSource::User,
                created_at,
            )
            .unwrap();
            let boosts = compute_feedback_boost(conn, &[item_a, item_b], created_at).unwrap();
            assert_eq!(boosts.len(), 2);
            assert!(
                (boosts[&item_a] - 1.0).abs() < 0.01,
                "Item A boost should be ~1.0"
            );
            assert!(
                (boosts[&item_b] - (-1.0)).abs() < 0.01,
                "Item B boost should be ~-1.0"
            );
        });
    }
}
