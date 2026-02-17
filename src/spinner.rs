//! Spinner/progress indicator for UX feedback
//!
//! Provides visual feedback while waiting for Ollama responses

use indicatif::{ProgressBar, ProgressStyle};

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
    pb
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
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let spinner = create_spinner("Testing...");
        assert!(!spinner.is_finished());
        spinner.finish_and_clear();
        assert!(spinner.is_finished());
    }

    #[test]
    fn test_custom_spinner() {
        let spinner = create_custom_spinner("Custom...", "{spinner} {msg}");
        assert!(!spinner.is_finished());
        spinner.finish_and_clear();
        assert!(spinner.is_finished());
    }
}
