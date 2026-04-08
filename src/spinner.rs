//! Spinner/progress indicator for UX feedback
//!
//! Provides visual feedback while waiting for Ollama responses.
//! Supports suspend/resume for printing tool calls.

use indicatif::{ProgressBar, ProgressStyle};
use std::sync::RwLock;

static ACTIVE_SPINNER: RwLock<Option<ProgressBar>> = RwLock::new(None);
static ACTIVE_STATUS_BAR: RwLock<Option<String>> = RwLock::new(None);

/// RAII guard that automatically finishes the spinner when dropped
///
/// This is a convenience API for automatic cleanup in error paths.
/// Current code uses manual `create_spinner`/`finish_spinner` pairs,
/// but this guard can be adopted incrementally.
///
/// # Example
/// ```ignore
/// use crate::spinner::SpinnerGuard;
///
/// fn operation() -> Result<(), Error> {
///     let _spinner = SpinnerGuard::new("Working...");
///     // Do work that might fail...
///     Ok(()) // Spinner automatically finished here
/// } // Or finished here on early return
/// ```
#[allow(dead_code)]
pub struct SpinnerGuard(Option<ProgressBar>);

#[allow(dead_code)]
impl SpinnerGuard {
    /// Create a new spinner guard with the given message
    pub fn new(message: &str) -> Self {
        Self(Some(create_spinner(message)))
    }

    /// Manually finish the spinner early (before drop)
    pub fn finish(&mut self) {
        if let Some(spinner) = self.0.take() {
            finish_spinner(spinner);
        }
    }
}

impl Drop for SpinnerGuard {
    fn drop(&mut self) {
        if let Some(spinner) = self.0.take() {
            finish_spinner(spinner);
        }
    }
}

/// Create a spinner for indicating that the application is waiting for a response
///
/// # Arguments
/// * `message` - The message to display next to the spinner
///
/// # Returns
/// A ProgressBar instance that should be cleared when the response arrives
///
/// # Example
/// ```ignore
/// use ask_ai::spinner::create_spinner;
///
/// let spinner = create_spinner("Thinking...");
/// // Do work...
/// spinner.finish_and_clear();
/// ```
pub fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("Failed to set spinner style"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // Store as active spinner for suspend/resume
    if let Ok(mut guard) = ACTIVE_SPINNER.write() {
        *guard = Some(pb.clone());
    }

    pb
}

/// Finish and clear the active spinner
pub fn finish_spinner(spinner: ProgressBar) {
    spinner.finish_and_clear();
    // Clear from global state
    if let Ok(mut guard) = ACTIVE_SPINNER.write() {
        *guard = None;
    }
    if let Ok(mut guard) = ACTIVE_STATUS_BAR.write() {
        *guard = None;
    }
}

/// Suspend the active spinner to print something to stderr
/// If no spinner is active, executes the closure directly
pub fn suspend_for_print<F>(f: F)
where
    F: FnOnce(),
{
    if let Ok(guard) = ACTIVE_SPINNER.read()
        && let Some(spinner) = guard.as_ref()
    {
        spinner.suspend(f);
        return;
    }
    // No active spinner, just execute directly
    f();
}

/// Create a spinner with a custom style
///
/// Allows customizing the spinner appearance for different contexts
///
/// # Arguments
/// * `message` - The message to display
/// * `template` - A custom template string (see indicatif docs)
#[allow(dead_code)]
pub fn create_custom_spinner(message: &str, template: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template(template)
            .expect("Failed to set custom spinner style"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    if let Ok(mut guard) = ACTIVE_SPINNER.write() {
        *guard = Some(pb.clone());
    }

    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let spinner = create_spinner("Testing...");
        assert!(!spinner.is_finished());
        finish_spinner(spinner);
        if let Ok(guard) = ACTIVE_SPINNER.read() {
            assert!(guard.is_none());
        }
    }

    #[test]
    fn test_custom_spinner() {
        let spinner = create_custom_spinner("Custom...", "{spinner} {msg}");
        assert!(!spinner.is_finished());
        finish_spinner(spinner);
        if let Ok(guard) = ACTIVE_SPINNER.read() {
            assert!(guard.is_none());
        }
    }

    #[test]
    fn test_spinner_guard_auto_finish() {
        {
            let _spinner = SpinnerGuard::new("Auto cleanup test");
            // Spinner active
            if let Ok(guard) = ACTIVE_SPINNER.read() {
                assert!(guard.is_some());
            }
        } // Drop here
        if let Ok(guard) = ACTIVE_SPINNER.read() {
            assert!(guard.is_none());
        }
    }

    #[test]
    fn test_spinner_guard_manual_finish() {
        let mut spinner = SpinnerGuard::new("Manual finish test");
        spinner.finish();
        if let Ok(guard) = ACTIVE_SPINNER.read() {
            assert!(guard.is_none());
        }
        // Drop should not panic
        drop(spinner);
    }
}
