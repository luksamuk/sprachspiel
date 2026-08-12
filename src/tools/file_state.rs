//! Per-session file tracking for staleness detection.
//!
//! Tracks which files have been read during this session and their
//! mtime/size at read time. Write tools (`edit_file`, `write_file`) consult
//! this state before allowing an edit: a file must have been read first, and
//! must not have been externally modified since that read.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

/// Global session-scoped file state. Tools are stateless (proc-macro
/// generates unit structs), so this global carries the mutable session state.
/// Lock contention is nil: tool calls are sequential in the ReAct loop.
pub static FILE_SESSION_STATE: Lazy<Arc<Mutex<FileSessionState>>> =
    Lazy::new(|| Arc::new(Mutex::new(FileSessionState::default())));

/// Acquire the global state lock.
///
/// # Panics
/// Panics if the mutex is poisoned (a previous tool call panicked while
/// holding the lock). This is intentional: a poisoned file-state mutex
/// indicates a programming bug, and failing loudly is safer than silently
/// continuing with inconsistent tracking.
#[expect(clippy::expect_used)] // Mutex poisoning is a programming bug — panic is intentional.
pub fn file_session_state() -> MutexGuard<'static, FileSessionState> {
    FILE_SESSION_STATE
        .lock()
        .expect("file session state lock poisoned")
}

#[derive(Debug, Default)]
pub struct FileSessionState {
    /// Files read in this session: canonical path → metadata at read time.
    read_files: HashMap<PathBuf, ReadFileEntry>,
}

#[derive(Debug, Clone, Copy)]
pub struct ReadFileEntry {
    pub mtime: SystemTime,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReason {
    /// File mtime or size differs from the recorded read-time values.
    ModifiedExternally,
}

impl FileSessionState {
    /// Record that the file at `path` was read in this session with the
    /// given mtime and size. Subsequent reads overwrite the previous entry.
    pub fn record_read(&mut self, path: PathBuf, mtime: SystemTime, size: u64) {
        self.read_files.insert(path, ReadFileEntry { mtime, size });
    }

    /// True if the file at `path` has been read at least once in this session.
    pub fn has_been_read(&self, path: &Path) -> bool {
        self.read_files.contains_key(path)
    }

    /// Check whether the file at `path` has been modified externally since
    /// the last time this session read it.
    ///
    /// Returns `Err(StaleReason::ModifiedExternally)` if mtime or size
    /// differs from the recorded entry, `Ok(())` if the file is unchanged.
    ///
    /// # Note
    /// This function does not fail on filesystem errors — the caller is
    /// responsible for fetching current metadata. Call BEFORE checking:
    /// if the file has never been read, this call's result is meaningless
    /// (the must-read-before-edit check should have rejected the edit first).
    pub fn check_stale(
        &self,
        path: &Path,
        current_mtime: SystemTime,
        current_size: u64,
    ) -> Result<(), StaleReason> {
        match self.read_files.get(path) {
            None => Ok(()), // Caller handles the never-read case separately.
            Some(entry) => {
                if entry.mtime != current_mtime || entry.size != current_size {
                    Err(StaleReason::ModifiedExternally)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Clear all state. Used by tests to isolate cases.
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.read_files.clear();
    }
}

/// Build the error message returned when the LLM attempts to edit a file
/// that has never been read in this session.
pub fn must_read_first_error(path: &str) -> String {
    format!(
        "Error: File '{}' has not been read in this session. \
         Use read_file or read_file_segment to read the file first, \
         then try editing again.",
        path
    )
}

/// Build the error message returned when a file has been modified externally
/// since this session last read it.
pub fn stale_error(path: &str) -> String {
    format!(
        "Error: File '{}' has been modified since it was last read. \
         Re-read the file with read_file or read_file_segment to get \
         the latest content, then try editing again.",
        path
    )
}

/// Refresh the recorded state for `path` after this session wrote to it.
/// Fetches current mtime+size from disk and updates the entry.
/// If metadata fetch fails, the entry is left unchanged (next edit will
/// likely detect staleness and require a re-read, which is the safe path).
pub fn refresh_after_write(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path)
        && let Ok(mtime) = meta.modified()
    {
        file_session_state().record_read(path.to_path_buf(), mtime, meta.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn t0() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_000)
    }

    fn t1() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_001)
    }

    #[test]
    fn has_been_read_is_false_initially() {
        let mut state = FileSessionState::default();
        state.clear(); // idempotent — also exercises clear()
        assert!(!state.has_been_read(Path::new("/tmp/x")));
    }

    #[test]
    fn record_read_marks_file_as_read() {
        let mut state = FileSessionState::default();
        state.record_read(PathBuf::from("/tmp/x"), t0(), 100);
        assert!(state.has_been_read(Path::new("/tmp/x")));
        assert!(!state.has_been_read(Path::new("/tmp/y")));
    }

    #[test]
    fn check_stale_ok_when_never_read() {
        // Caller is expected to enforce must-read-first; check_stale on
        // unknown path is Ok (no staleness info available).
        let state = FileSessionState::default();
        assert!(state.check_stale(Path::new("/tmp/x"), t0(), 100).is_ok());
    }

    #[test]
    fn check_stale_ok_when_mtime_and_size_match() {
        let mut state = FileSessionState::default();
        state.record_read(PathBuf::from("/tmp/x"), t0(), 100);
        assert!(state.check_stale(Path::new("/tmp/x"), t0(), 100).is_ok());
    }

    #[test]
    fn check_stale_err_when_mtime_changed() {
        let mut state = FileSessionState::default();
        state.record_read(PathBuf::from("/tmp/x"), t0(), 100);
        assert_eq!(
            state.check_stale(Path::new("/tmp/x"), t1(), 100),
            Err(StaleReason::ModifiedExternally)
        );
    }

    #[test]
    fn check_stale_err_when_size_changed() {
        let mut state = FileSessionState::default();
        state.record_read(PathBuf::from("/tmp/x"), t0(), 100);
        assert_eq!(
            state.check_stale(Path::new("/tmp/x"), t0(), 101),
            Err(StaleReason::ModifiedExternally)
        );
    }

    #[test]
    fn record_read_overwrites_previous_entry() {
        let mut state = FileSessionState::default();
        state.record_read(PathBuf::from("/tmp/x"), t0(), 100);
        state.record_read(PathBuf::from("/tmp/x"), t1(), 200);
        // Newest entry: t1 + 200 — neither t0 nor 100 should still match.
        assert!(state.check_stale(Path::new("/tmp/x"), t0(), 100).is_err());
        assert!(state.check_stale(Path::new("/tmp/x"), t1(), 200).is_ok());
    }

    #[test]
    fn write_then_readless_edit_does_not_stale_positive() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("sprach_205_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("x.txt");
        let path_buf = path.clone();

        let mut file = std::fs::File::create(&path).expect("create file");
        file.write_all(b"initial").expect("write initial");
        drop(file);

        // First read: capture state.
        let meta1 = std::fs::metadata(&path).expect("meta");
        let mtime1 = meta1.modified().expect("mtime");
        file_session_state().record_read(path_buf.clone(), mtime1, meta1.len());

        // Write (bypasses atomic_write; we just want on-disk mtime to advance).
        // Sleep 2ms to ensure mtime resolution differs on filesystems with
        // coarse-grained mtimes (some filesystems only have second resolution).
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut file = std::fs::File::create(&path).expect("re-create file");
        file.write_all(b"modified").expect("write modified");
        drop(file);

        // Without refresh_after_write: stale (mtime differs, size differs).
        let meta2 = std::fs::metadata(&path).expect("meta2");
        let mtime2 = meta2.modified().expect("mtime2");
        assert!(
            file_session_state()
                .check_stale(&path_buf, mtime2, meta2.len())
                .is_err(),
            "expected ModifiedExternally before refresh"
        );

        // After refresh_after_write: not stale.
        refresh_after_write(&path_buf);
        let meta3 = std::fs::metadata(&path).expect("meta3");
        let mtime3 = meta3.modified().expect("mtime3");
        assert!(
            file_session_state()
                .check_stale(&path_buf, mtime3, meta3.len())
                .is_ok(),
            "expected Ok after refresh"
        );

        let _ = std::fs::remove_dir_all(&dir);
        file_session_state().clear();
    }
}
