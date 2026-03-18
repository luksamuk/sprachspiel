//! Message role constants
//!
//! Provides centralized constants for message roles to prevent
//! string duplication across the codebase.

/// Role string for user messages
pub const ROLE_USER: &str = "user";

/// Role string for assistant messages
pub const ROLE_ASSISTANT: &str = "assistant";

/// Role string for system messages
pub const ROLE_SYSTEM: &str = "system";

/// Role string for tool messages
pub const ROLE_TOOL: &str = "tool";

/// Display label for user role (with emoji)
pub const ROLE_LABEL_USER: &str = "👤 User";

/// Display label for assistant role (with emoji)
pub const ROLE_LABEL_ASSISTANT: &str = "🤖 Assistant";

/// Display label for system role (with emoji)
pub const ROLE_LABEL_SYSTEM: &str = "⚙️ System";

/// Display label for tool role (with emoji)
pub const ROLE_LABEL_TOOL: &str = "🔧 Tool";

/// Format a role string into a human-readable label with emoji.
///
/// # Arguments
/// * `role` - The role string ("user", "assistant", "system", "tool")
///
/// # Returns
/// A static string with emoji prefix (e.g., "👤 User")
///
/// # Example
/// ```ignore
/// use crate::consts::roles::format_role_label;
/// let label = format_role_label("user"); // "👤 User"
/// ```
pub fn format_role_label(role: &str) -> String {
    match role {
        ROLE_USER => ROLE_LABEL_USER.to_string(),
        ROLE_ASSISTANT => ROLE_LABEL_ASSISTANT.to_string(),
        ROLE_SYSTEM => ROLE_LABEL_SYSTEM.to_string(),
        ROLE_TOOL => ROLE_LABEL_TOOL.to_string(),
        _ => role.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_constants() {
        assert_eq!(ROLE_USER, "user");
        assert_eq!(ROLE_ASSISTANT, "assistant");
        assert_eq!(ROLE_SYSTEM, "system");
        assert_eq!(ROLE_TOOL, "tool");
    }

    #[test]
    fn test_format_role_label() {
        assert_eq!(format_role_label("user"), "👤 User");
        assert_eq!(format_role_label("assistant"), "🤖 Assistant");
        assert_eq!(format_role_label("system"), "⚙️ System");
        assert_eq!(format_role_label("tool"), "🔧 Tool");
        assert_eq!(format_role_label("unknown"), "unknown");
    }
}
