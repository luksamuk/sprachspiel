//! Spinner/progress indicator for UX feedback
//!
//! Provides visual feedback while waiting for Ollama responses.
//! Supports suspend/resume for printing tool calls.

use indicatif::{ProgressBar, ProgressStyle};
use std::sync::RwLock;

static ACTIVE_SPINNER: RwLock<Option<ProgressBar>> = RwLock::new(None);

/// Create a spinner for indicating that the application is waiting for a response
///
/// # Arguments
/// * `message` - The message to display next to the spinner
///
/// # Returns
/// A ProgressBar instance that should be cleared when the response arrives
///
/// # Example
/// ```
/// use ask_ollama::spinner::create_spinner;
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
}
