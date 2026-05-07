//! Spinner/progress indicator for UX feedback
//!
//! Provides visual feedback while waiting for Ollama responses.
//! Supports suspend/resume for printing tool calls.
//! Uses rattles presets for randomized spinner animations.

use indicatif::{ProgressBar, ProgressStyle};
use rattles::Rattle;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

static ACTIVE_SPINNER: RwLock<Option<ProgressBar>> = RwLock::new(None);
static ACTIVE_STATUS_BAR: RwLock<Option<String>> = RwLock::new(None);

/// Extract all animation frames from a Rattler as static string slices.
/// Uses TickedRattler to iterate without depending on the global clock.
fn extract_all_frames<T: Rattle>(rattler: rattles::Rattler<T>) -> Vec<&'static str> {
    let len = rattler.len();
    let mut ticked = rattler.into_ticked();
    let mut frames = Vec::with_capacity(len);
    for _ in 0..len {
        frames.push(ticked.tick()[0]);
    }
    frames
}

/// Return frames from a random rattles preset for use as indicatif tick_strings.
///
/// Uses the system timestamp for randomness (no `rand` dependency).
/// The last frame is always `" "` (space) — shown when the spinner finishes.
/// Only single-line presets are included (no emoji, no multi-line braille).
fn random_spinner_frames() -> Vec<&'static str> {
    let presets: Vec<fn() -> Vec<&'static str>> = vec![
        // --- braille ---
        || extract_all_frames(rattles::presets::braille::dots()),
        || extract_all_frames(rattles::presets::braille::dots2()),
        || extract_all_frames(rattles::presets::braille::dots3()),
        || extract_all_frames(rattles::presets::braille::dots4()),
        || extract_all_frames(rattles::presets::braille::dots5()),
        || extract_all_frames(rattles::presets::braille::dots6()),
        || extract_all_frames(rattles::presets::braille::dots7()),
        || extract_all_frames(rattles::presets::braille::dots8()),
        || extract_all_frames(rattles::presets::braille::dots9()),
        || extract_all_frames(rattles::presets::braille::dots10()),
        || extract_all_frames(rattles::presets::braille::dots11()),
        || extract_all_frames(rattles::presets::braille::dots12()),
        || extract_all_frames(rattles::presets::braille::bounce()),
        || extract_all_frames(rattles::presets::braille::breathe()),
        || extract_all_frames(rattles::presets::braille::snake()),
        || extract_all_frames(rattles::presets::braille::wave()),
        || extract_all_frames(rattles::presets::braille::waverows()),
        || extract_all_frames(rattles::presets::braille::pulse()),
        || extract_all_frames(rattles::presets::braille::orbit()),
        || extract_all_frames(rattles::presets::braille::helix()),
        || extract_all_frames(rattles::presets::braille::sparkle()),
        || extract_all_frames(rattles::presets::braille::rain()),
        || extract_all_frames(rattles::presets::braille::sand()),
        || extract_all_frames(rattles::presets::braille::scan()),
        || extract_all_frames(rattles::presets::braille::cascade()),
        || extract_all_frames(rattles::presets::braille::fillsweep()),
        // --- ascii ---
        || extract_all_frames(rattles::presets::ascii::dqpb()),
        || extract_all_frames(rattles::presets::ascii::arc()),
        || extract_all_frames(rattles::presets::ascii::balloon()),
        || extract_all_frames(rattles::presets::ascii::circle_halves()),
        || extract_all_frames(rattles::presets::ascii::circle_quarters()),
        || extract_all_frames(rattles::presets::ascii::toggle()),
        || extract_all_frames(rattles::presets::ascii::triangle()),
        || extract_all_frames(rattles::presets::ascii::grow_horizontal()),
        || extract_all_frames(rattles::presets::ascii::grow_vertical()),
        || extract_all_frames(rattles::presets::ascii::noise()),
        || extract_all_frames(rattles::presets::ascii::point()),
        || extract_all_frames(rattles::presets::ascii::simple_dots()),
        || extract_all_frames(rattles::presets::ascii::simple_dots_scrolling()),
        || extract_all_frames(rattles::presets::ascii::square_corners()),
        || extract_all_frames(rattles::presets::ascii::rolling_line()),
        // --- arrows ---
        || extract_all_frames(rattles::presets::arrows::arrow()),
    ];

    let idx = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        % (presets.len() as u128)) as usize;

    let mut frames = presets[idx]();
    // indicatif shows the last frame when the spinner finishes
    frames.push(" ");
    frames
}

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
/// use sprachspiel::spinner::create_spinner;
///
/// let spinner = create_spinner("Thinking...");
/// // Do work...
/// spinner.finish_and_clear();
/// ```
/// Check if spinners should be shown (hidden in quiet mode)
///
/// Returns `true` unless the log level is explicitly set to `Error` (quiet mode).
/// When no logger is initialized (`Off`), spinners are enabled (default behavior).
pub fn is_spinner_enabled() -> bool {
    let level = log::max_level();
    // Off = no logger initialized yet (default: spinners enabled)
    // Error = quiet mode (spinners suppressed)
    level != log::LevelFilter::Error
}

/// Create a new progress spinner with random animation frames.
///
/// In quiet mode (error level only), returns a hidden spinner that does nothing.
/// This ensures spinners are suppressed when the user only wants the final output.
pub fn create_spinner(message: &str) -> ProgressBar {
    if !is_spinner_enabled() {
        // In quiet mode, return a hidden spinner but still track it
        let pb = ProgressBar::hidden();
        pb.set_message(message.to_string());
        if let Ok(mut guard) = ACTIVE_SPINNER.write() {
            *guard = Some(pb.clone());
        }
        return pb;
    }

    let pb = ProgressBar::new_spinner();
    let frames = random_spinner_frames();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&frames)
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
    let frames = random_spinner_frames();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&frames)
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
