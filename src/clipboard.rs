//! Clipboard abstraction with no-op fallback for Android/Termux.
//!
//! On Linux, macOS, and Windows, this delegates to `cli-clipboard`.
//! On Android (Termux), clipboard operations are no-ops since
//! `cli-clipboard` does not compile for `target_os = "android"`.

/// Set the system clipboard contents.
///
/// Returns `Ok(())` on success, `Err` if clipboard is unavailable.
/// On Android, always returns `Err` (clipboard not available).
#[cfg(not(target_os = "android"))]
pub fn set_contents(text: String) -> Result<(), String> {
    cli_clipboard::set_contents(text).map_err(|e| e.to_string())
}

/// Set the system clipboard contents (Android no-op).
#[cfg(target_os = "android")]
pub fn set_contents(_text: String) -> Result<(), String> {
    Err("clipboard not available on android".to_string())
}

/// Get the system clipboard contents.
///
/// Returns the clipboard text on success, `Err` if clipboard is unavailable.
/// On Android, always returns `Err` (clipboard not available).
#[cfg(not(target_os = "android"))]
pub fn get_contents() -> Result<String, String> {
    cli_clipboard::get_contents().map_err(|e| e.to_string())
}

/// Get the system clipboard contents (Android no-op).
#[cfg(target_os = "android")]
pub fn get_contents() -> Result<String, String> {
    Err("clipboard not available on android".to_string())
}
