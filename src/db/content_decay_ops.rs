//! Database operations for content decay
//!
//! SQL-based operations for content decay cycle: access reinforcement,
//! decay-based pruning, and statistics queries.
//! Pure logic functions (half-life computation, retention calculation, pruning threshold)
//! remain in `crate::content::decay`.

use rusqlite::{Connection, params};

use crate::content::decay::{ContentDecayStats, compute_content_retention, should_prune_content};

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
pub fn on_content_access(
    conn: &Connection,
    item_id: i64,
    importance_boost: f32,
) -> Result<(), String> {
    conn.execute(
        "UPDATE content_items SET access_count = access_count + 1, last_accessed = unixepoch('now'), importance = MIN(1.0, importance + ?1) WHERE id = ?2",
        params![importance_boost, item_id],
    )
    .map_err(|e| format!("Failed to update content access: {}", e))?;
    Ok(())
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

    // Find items to prune and update decay_score for all items
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

        // Persist decay_score so /context "items at risk" query works
        conn.execute(
            "UPDATE content_items SET decay_score = ?1 WHERE id = ?2",
            params![retention, id],
        )
        .map_err(|e| format!("Failed to update decay_score for item {}: {}", id, e))?;

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

/// Overview statistics for the content decay system, used in /context display.
///
/// Provides a snapshot of content memory health.
/// `decay_score` is persisted during `run_content_decay_cycle()`, so
/// "items at risk" queries are accurate after each cycle run.
pub struct ContentDecayOverview {
    /// Total non-pruned content items
    pub total_items: usize,
    /// Average importance across non-pruned items (0.0–1.0)
    pub avg_importance: f64,
    /// Items with decay_score < 0.3 (at risk of pruning)
    pub items_at_risk: usize,
    /// Total number of feedback signals
    pub total_feedback_signals: usize,
}

/// Get content decay statistics for display in /context command.
///
/// Returns aggregated stats from the content_items and feedback_signals tables.
/// `decay_score` values are persisted by `run_content_decay_cycle()`,
/// so "items at risk" is accurate after each cycle run.
///
/// # Arguments
/// * `conn` - SQLite connection
///
/// # Returns
/// ContentDecayOverview with aggregate statistics, or error message on failure
pub fn get_content_decay_stats(conn: &Connection) -> Result<ContentDecayOverview, String> {
    // Total non-pruned items and average importance
    let (total_items, avg_importance): (i64, f64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(AVG(importance), 0.0) FROM content_items WHERE pruned = 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Failed to query content stats: {}", e))?;

    // Items at risk (low decay_score, not high importance)
    let items_at_risk: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM content_items WHERE pruned = 0 AND decay_score < 0.3 AND importance < 0.8",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to query at-risk items: {}", e))?;

    // Total feedback signals
    let total_feedback_signals: i64 = conn
        .query_row("SELECT COUNT(*) FROM feedback_signals", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Failed to query feedback signals count: {}", e))?;

    Ok(ContentDecayOverview {
        total_items: total_items as usize,
        avg_importance,
        items_at_risk: items_at_risk as usize,
        total_feedback_signals: total_feedback_signals as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            );",
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
            );",
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

        // Verify decay_score was persisted for non-pruned items
        let note_decay: f32 = conn
            .query_row(
                "SELECT decay_score FROM content_items WHERE content = 'recent note'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            note_decay > 0.9,
            "Recent note decay_score should be > 0.9, got {}",
            note_decay
        );

        let doc_decay: f32 = conn
            .query_row(
                "SELECT decay_score FROM content_items WHERE content = 'important doc'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // Old document with high importance: decay is low but importance multiplier boosts it
        assert!(
            doc_decay > 0.0,
            "Important doc decay_score should be > 0, got {}",
            doc_decay
        );
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

        // Verify decay_score was persisted (fresh item should have high score)
        let decay_score: f32 = conn
            .query_row(
                "SELECT decay_score FROM content_items WHERE content = 'fresh note'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            decay_score > 0.9,
            "Fresh note decay_score should be > 0.9 after cycle, got {}",
            decay_score
        );
    }

    #[test]
    fn test_decay_score_persisted_after_cycle() {
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

        // Insert item with default decay_score = 1.0 but old last_accessed
        // (should get a lower decay_score after the cycle)
        let six_months_ago = now - (180 * 86400);
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, access_count, created_at, updated_at, last_accessed)
             VALUES ('note', 'old note', 0.3, 0, ?1, ?1, ?1)",
            params![six_months_ago],
        )
        .unwrap();

        // Verify initial decay_score is 1.0 (default)
        let initial_decay: f32 = conn
            .query_row(
                "SELECT decay_score FROM content_items WHERE content = 'old note'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            (initial_decay - 1.0).abs() < 0.01,
            "Initial decay_score should be 1.0, got {}",
            initial_decay
        );

        // Run decay cycle
        let stats = run_content_decay_cycle(&conn).unwrap();
        assert_eq!(
            stats.pruned, 0,
            "Old note should not be pruned (importance < 0.8 but retention > 0.05)"
        );

        // Verify decay_score was updated (should be < 1.0 for an old item)
        let updated_decay: f32 = conn
            .query_row(
                "SELECT decay_score FROM content_items WHERE content = 'old note'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            updated_decay < 1.0,
            "Old note decay_score should be < 1.0 after cycle, got {}",
            updated_decay
        );
        assert!(
            updated_decay > 0.0,
            "Old note decay_score should be > 0 (not pruned), got {}",
            updated_decay
        );
    }

    // === get_content_decay_stats ===

    #[test]
    fn test_get_content_decay_stats_empty() {
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
            CREATE TABLE feedback_signals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                session_id TEXT,
                signal_type TEXT NOT NULL CHECK(signal_type IN ('good', 'bad', 'correction')),
                base_value REAL NOT NULL,
                correction_text TEXT,
                source TEXT NOT NULL DEFAULT 'user' CHECK(source IN ('user', 'llm')),
                created_at INTEGER NOT NULL,
                FOREIGN KEY (item_id) REFERENCES content_items(id) ON DELETE CASCADE
            );",
        )
        .unwrap();

        let stats = get_content_decay_stats(&conn).unwrap();
        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.avg_importance, 0.0);
        assert_eq!(stats.items_at_risk, 0);
        assert_eq!(stats.total_feedback_signals, 0);
    }

    #[test]
    fn test_get_content_decay_stats_with_items() {
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
            CREATE TABLE feedback_signals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                session_id TEXT,
                signal_type TEXT NOT NULL CHECK(signal_type IN ('good', 'bad', 'correction')),
                base_value REAL NOT NULL,
                correction_text TEXT,
                source TEXT NOT NULL DEFAULT 'user' CHECK(source IN ('user', 'llm')),
                created_at INTEGER NOT NULL,
                FOREIGN KEY (item_id) REFERENCES content_items(id) ON DELETE CASCADE
            );",
        )
        .unwrap();

        let now = chrono::Utc::now().timestamp();

        // Insert 2 content items with different importance and decay_score
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('note', 'important note', 0.7, 0.8, ?1, ?1, ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('message', 'at-risk msg', 0.1, 0.2, ?1, ?1, ?1)",
            params![now],
        ).unwrap();

        // Insert 1 feedback signal for the first item
        conn.execute(
            "INSERT INTO feedback_signals (item_id, signal_type, base_value, source, created_at)
             VALUES (1, 'good', 1.0, 'user', ?1)",
            params![now],
        )
        .unwrap();

        let stats = get_content_decay_stats(&conn).unwrap();
        assert_eq!(stats.total_items, 2);
        assert!(
            (stats.avg_importance - 0.4).abs() < 0.01,
            "Expected avg_importance ~0.4, got {}",
            stats.avg_importance
        );
        assert_eq!(stats.items_at_risk, 1); // decay_score 0.2 < 0.3 and importance 0.1 < 0.8
        assert_eq!(stats.total_feedback_signals, 1);
    }

    #[test]
    fn test_get_content_decay_stats_excludes_pruned() {
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
            CREATE TABLE feedback_signals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                session_id TEXT,
                signal_type TEXT NOT NULL CHECK(signal_type IN ('good', 'bad', 'correction')),
                base_value REAL NOT NULL,
                correction_text TEXT,
                source TEXT NOT NULL DEFAULT 'user' CHECK(source IN ('user', 'llm')),
                created_at INTEGER NOT NULL,
                FOREIGN KEY (item_id) REFERENCES content_items(id) ON DELETE CASCADE
            );",
        )
        .unwrap();

        let now = chrono::Utc::now().timestamp();

        // Insert an active item
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, created_at, updated_at, last_accessed)
             VALUES ('note', 'active note', 0.5, 0.8, ?1, ?1, ?1)",
            params![now],
        ).unwrap();

        // Insert a pruned item (should be excluded from stats)
        conn.execute(
            "INSERT INTO content_items (content_type, content, importance, decay_score, pruned, created_at, updated_at, last_accessed)
             VALUES ('note', 'pruned note', 0.3, 0.1, 1, ?1, ?1, ?1)",
            params![now],
        ).unwrap();

        let stats = get_content_decay_stats(&conn).unwrap();
        assert_eq!(stats.total_items, 1); // excludes pruned
        assert!(
            (stats.avg_importance - 0.5).abs() < 0.01,
            "Expected avg_importance ~0.5, got {}",
            stats.avg_importance
        );
        assert_eq!(stats.items_at_risk, 0); // active item has decay_score 0.8 >= 0.3
    }
}
