//! Decay calculations for the Content System
//!
//! Based on Ebbinghaus forgetting curve with access reinforcement.
//! Mirrors the facts system's decay pattern with content-specific half-lives.
//!
//! Content half-lives differ by type:
//! - Messages: 90 days (ephemeral conversational context)
//! - Notes: 60 days (personal notes, shorter-lived than documents)
//! - Documents: 120 days (imported reference material, longest retention)

use rusqlite::{params, Connection};

/// Half-life for message content items (days)
#[allow(dead_code)] // Consumed by content system (search/pruning)
pub const HALF_LIFE_MESSAGE: f32 = 90.0;

/// Half-life for note content items (days)
#[allow(dead_code)] // Consumed by content system
pub const HALF_LIFE_NOTE: f32 = 60.0;

/// Half-life for document content items (days)
#[allow(dead_code)] // Consumed by content system
pub const HALF_LIFE_DOCUMENT: f32 = 120.0;

/// Access boost per access (0.1 = 10%)
#[allow(dead_code)] // Consumed by content system
pub const CONTENT_ACCESS_BOOST: f32 = 0.1;

/// Maximum importance boost factor
#[allow(dead_code)] // Consumed by content system
pub const CONTENT_IMPORTANCE_BOOST: f32 = 0.5;

/// Minimum retention threshold for pruning (0.05 = 5%)
#[allow(dead_code)] // Consumed by content system
pub const MIN_CONTENT_RETENTION: f32 = 0.05;

/// Get the half-life for a content type.
///
/// Returns the Ebbinghaus half-life in days for the given content type.
/// Defaults to message half-life for unknown types.
#[allow(dead_code)] // Consumed by content system
pub fn get_content_half_life(content_type: &str) -> f32 {
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
#[allow(dead_code)] // Consumed by content system
pub fn compute_content_retention(
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

/// Record an access event for a content item.
///
/// Increments access_count, sets last_accessed to current Unix epoch,
/// and adds importance_boost to importance (clamped at 1.0).
///
/// # Arguments
/// * `conn` - SQLite connection
/// * `item_id` - ID of the content item to update
/// * `importance_boost` - Amount to add to importance (use 0.0 for no boost)
///
/// # Returns
/// Ok(()) on success, Err with message on failure
#[allow(dead_code)] // Consumed by content system
pub fn on_content_access(conn: &Connection, item_id: i64, importance_boost: f32) -> Result<(), String> {
    conn.execute(
        "UPDATE content_items SET access_count = access_count + 1, last_accessed = unixepoch('now'), importance = MIN(1.0, importance + ?1) WHERE id = ?2",
        params![importance_boost, item_id],
    )
    .map_err(|e| format!("Failed to update content access: {}", e))?;
    Ok(())
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
#[allow(dead_code)] // Consumed by content system
pub fn should_prune_content(importance: f32, retention: f32) -> bool {
    // Never prune high-importance items
    if importance >= 0.8 {
        return false;
    }

    // Prune items below minimum retention
    retention < MIN_CONTENT_RETENTION
}

/// Statistics from running the content decay cycle
#[derive(Debug, Clone)]
#[allow(dead_code)] // Consumed by content system
pub struct ContentDecayStats {
    /// Number of content items pruned
    pub pruned: usize,
    /// Number of content items remaining
    pub remaining: usize,
    /// Average retention of remaining items
    pub avg_retention: f32,
}

/// Run the content decay cycle.
///
/// Iterates over all non-pruned content items, computes retention,
/// and soft-deletes (pruned = 1) items that fall below the threshold.
///
/// # Arguments
/// * `conn` - SQLite connection
///
/// # Returns
/// ContentDecayStats with pruning results
#[allow(dead_code)] // Consumed by content system
pub fn run_content_decay_cycle(conn: &Connection) -> Result<ContentDecayStats, String> {
    let now = chrono::Utc::now().timestamp();

    // Get all non-pruned content items
    let mut stmt = conn
        .prepare(
            "SELECT id, importance, access_count, content_type, last_accessed, pruned
             FROM content_items WHERE pruned = 0",
        )
        .map_err(|e| format!("Failed to prepare decay query: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, f32>(1)?,
                row.get::<_, i32>(2)? as u32,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i32>(5)?,
            ))
        })
        .map_err(|e| format!("Failed to execute decay query: {}", e))?;

    let items: Vec<(i64, f32, u32, String, i64, i32)> = rows.filter_map(|r| r.ok()).collect();

    // Find items to prune
    let mut pruned_ids: Vec<i64> = Vec::new();
    let mut retention_sum: f32 = 0.0;

    for (id, importance, access_count, content_type, last_accessed, _pruned) in &items {
        let retention = compute_content_retention(
            *importance,
            *access_count,
            content_type,
            *last_accessed,
            now,
        );

        if should_prune_content(*importance, retention) {
            pruned_ids.push(*id);
        } else {
            retention_sum += retention;
        }
    }

    // Soft-delete pruned items
    for id in &pruned_ids {
        conn.execute(
            "UPDATE content_items SET pruned = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("Failed to prune content item {}: {}", id, e))?;
    }

    let pruned = pruned_ids.len();
    let remaining = items.len() - pruned;
    let avg_retention = if remaining > 0 {
        retention_sum / remaining as f32
    } else {
        0.0
    };

    Ok(ContentDecayStats {
        pruned,
        remaining,
        avg_retention,
    })
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

    // === on_content_access ===

    #[test]
    fn test_on_content_access_increments_count() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                conversation_id TEXT,
                role TEXT,
                message_type TEXT DEFAULT 'normal',
                previous_item_id INTEGER,
                prompt_tokens INTEGER,
                scope TEXT,
                source TEXT,
                title TEXT,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                decay_score REAL DEFAULT 1.0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL,
                has_embedding INTEGER DEFAULT 0,
                pruned INTEGER NOT NULL DEFAULT 0,
                project_id TEXT
            );
            INSERT INTO content_items (content_type, content, created_at, updated_at, last_accessed)
            VALUES ('note', 'test', 1000, 1000, 1000);",
        )
        .unwrap();

        // Initial access_count should be 0
        let count: i32 = conn
            .query_row(
                "SELECT access_count FROM content_items WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);

        on_content_access(&conn, 1, 0.0).unwrap();

        let count: i32 = conn
            .query_row(
                "SELECT access_count FROM content_items WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        on_content_access(&conn, 1, 0.0).unwrap();

        let count: i32 = conn
            .query_row(
                "SELECT access_count FROM content_items WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        // last_accessed should be updated to current time
        let last_accessed: i64 = conn
            .query_row(
                "SELECT last_accessed FROM content_items WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!(
            (last_accessed - now).abs() < 5,
            "last_accessed should be set to now, got {} vs {}",
            last_accessed,
            now
        );
    }

    #[test]
    fn test_on_content_access_with_importance_boost() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                conversation_id TEXT,
                role TEXT,
                message_type TEXT DEFAULT 'normal',
                previous_item_id INTEGER,
                prompt_tokens INTEGER,
                scope TEXT,
                source TEXT,
                title TEXT,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                decay_score REAL DEFAULT 1.0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL,
                has_embedding INTEGER DEFAULT 0,
                pruned INTEGER NOT NULL DEFAULT 0,
                project_id TEXT
            );"
        )
        .unwrap();

        // Insert a content item with importance 0.5
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('message', 'test', 0.5, 1.0, 1713600000, 1713600000, 1713600000)",
            rusqlite::params![],
        )
        .unwrap();
        let item_id = conn.last_insert_rowid();

        // Before: importance = 0.5, access_count = 0
        on_content_access(&conn, item_id, 0.001).unwrap();

        // After: importance = MIN(1.0, 0.5 + 0.001) = 0.501
        let importance: f32 = conn
            .query_row(
                "SELECT importance FROM content_items WHERE id = ?1",
                rusqlite::params![item_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            (importance - 0.501).abs() < 0.01,
            "Expected importance ~0.501, got {}",
            importance
        );

        // After: access_count = 1
        let access_count: i32 = conn
            .query_row(
                "SELECT access_count FROM content_items WHERE id = ?1",
                rusqlite::params![item_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(access_count, 1);
    }

    #[test]
    fn test_on_content_access_importance_capped_at_one() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                conversation_id TEXT,
                role TEXT,
                message_type TEXT DEFAULT 'normal',
                previous_item_id INTEGER,
                prompt_tokens INTEGER,
                scope TEXT,
                source TEXT,
                title TEXT,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                decay_score REAL DEFAULT 1.0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL,
                has_embedding INTEGER DEFAULT 0,
                pruned INTEGER NOT NULL DEFAULT 0,
                project_id TEXT
            );"
        )
        .unwrap();

        // Insert a content item with importance 0.999
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('message', 'test', 0.999, 1.0, 1713600000, 1713600000, 1713600000)",
            rusqlite::params![],
        )
        .unwrap();
        let item_id = conn.last_insert_rowid();

        // Boost by 0.01 → importance would be 1.009, but capped at 1.0
        on_content_access(&conn, item_id, 0.01).unwrap();

        let importance: f32 = conn
            .query_row(
                "SELECT importance FROM content_items WHERE id = ?1",
                rusqlite::params![item_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            (importance - 1.0).abs() < 0.01,
            "Expected importance capped at 1.0, got {}",
            importance
        );
    }
    // === run_content_decay_cycle ===

    #[test]
    fn test_run_decay_cycle_prunes_old_items() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                conversation_id TEXT,
                role TEXT,
                message_type TEXT DEFAULT 'normal',
                previous_item_id INTEGER,
                prompt_tokens INTEGER,
                scope TEXT,
                source TEXT,
                title TEXT,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                decay_score REAL DEFAULT 1.0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL,
                has_embedding INTEGER DEFAULT 0,
                pruned INTEGER NOT NULL DEFAULT 0,
                project_id TEXT
            );",
        )
        .unwrap();

        let now = chrono::Utc::now().timestamp();

        // Insert a very old message (2 years ago, should be pruned)
        let two_years_ago = now - (2 * 365 * 86400);
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, access_count, created_at, updated_at, last_accessed)
             VALUES ('message', 'old message', 0.3, 0, ?1, ?1, ?1)",
            params![two_years_ago],
        ).unwrap();

        // Insert a recent note (should NOT be pruned)
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, access_count, created_at, updated_at, last_accessed)
             VALUES ('note', 'recent note', 0.5, 0, ?1, ?1, ?1)",
            params![now],
        ).unwrap();

        // Insert a high-importance document (should NOT be pruned even if old)
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, access_count, created_at, updated_at, last_accessed)
             VALUES ('document', 'important doc', 0.9, 0, ?1, ?1, ?1)",
            params![two_years_ago],
        ).unwrap();

        let stats = run_content_decay_cycle(&conn).unwrap();
        assert_eq!(stats.pruned, 1, "Should prune 1 item (old message)");
        assert_eq!(stats.remaining, 2, "Should have 2 remaining items");

        // Verify the old message was soft-deleted
        let pruned: i32 = conn
            .query_row(
                "SELECT pruned FROM content_items WHERE content = 'old message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pruned, 1, "Old message should be pruned (soft-deleted)");

        // Verify the recent note is still active
        let pruned: i32 = conn
            .query_row(
                "SELECT pruned FROM content_items WHERE content = 'recent note'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pruned, 0, "Recent note should not be pruned");

        // Verify the high-importance document is still active
        let pruned: i32 = conn
            .query_row(
                "SELECT pruned FROM content_items WHERE content = 'important doc'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pruned, 0, "High-importance document should not be pruned");
    }

    #[test]
    fn test_run_decay_cycle_empty_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                conversation_id TEXT,
                role TEXT,
                message_type TEXT DEFAULT 'normal',
                previous_item_id INTEGER,
                prompt_tokens INTEGER,
                scope TEXT,
                source TEXT,
                title TEXT,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                decay_score REAL DEFAULT 1.0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL,
                has_embedding INTEGER DEFAULT 0,
                pruned INTEGER NOT NULL DEFAULT 0,
                project_id TEXT
            );",
        )
        .unwrap();

        let stats = run_content_decay_cycle(&conn).unwrap();
        assert_eq!(stats.pruned, 0);
        assert_eq!(stats.remaining, 0);
        assert_eq!(stats.avg_retention, 0.0);
    }

    #[test]
    fn test_run_decay_cycle_avg_retention() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                conversation_id TEXT,
                role TEXT,
                message_type TEXT DEFAULT 'normal',
                previous_item_id INTEGER,
                prompt_tokens INTEGER,
                scope TEXT,
                source TEXT,
                title TEXT,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                access_count INTEGER DEFAULT 0,
                decay_score REAL DEFAULT 1.0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL,
                has_embedding INTEGER DEFAULT 0,
                pruned INTEGER NOT NULL DEFAULT 0,
                project_id TEXT
            );",
        )
        .unwrap();

        let now = chrono::Utc::now().timestamp();

        // Insert a recent item (high retention)
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, access_count, created_at, updated_at, last_accessed)
             VALUES ('note', 'fresh note', 0.5, 0, ?1, ?1, ?1)",
            params![now],
        ).unwrap();

        let stats = run_content_decay_cycle(&conn).unwrap();
        assert_eq!(stats.pruned, 0);
        assert_eq!(stats.remaining, 1);
        assert!(
            stats.avg_retention > 0.9,
            "Avg retention for fresh item should be > 0.9, got {}",
            stats.avg_retention
        );
    }
}