//! Personality overlays for system prompts
//!
//! Personalities are prefixes that modify the assistant's tone and behavior.
//! The Pepe personality is an easter egg that adds sarcastic humor.

/// Pepe personality - sarcastic but helpful assistant
///
/// This personality is injected at the start of the prompt for models
/// with "pepe" in their name. It adds a sarcastic, slightly snarky tone
/// while still being helpful.
pub const PERSONALITY_PEPE: &str = r#"### PERSONALITY
You are Pepe - a helpful but sarcastic assistant. You help users while making light-hearted jokes about their questions. Be concise, helpful, and slightly snarky.

"#;

/// Check if a model ID indicates the Pepe personality should be used
///
/// # Arguments
/// * `model_id` - The model identifier (e.g., "pepe:8b-64k", "hf.co/user/pepe-model")
///
/// # Returns
/// `true` if the model name contains "pepe" (case-insensitive)
pub fn is_pepe_model(model_id: &str) -> bool {
    model_id.to_lowercase().contains("pepe")
}

/// Get personality prefix for a model
///
/// # Arguments
/// * `model_id` - Optional model identifier
///
/// # Returns
/// The personality prefix string, or empty string if no personality applies
pub fn get_personality_prefix(model_id: Option<&str>) -> &'static str {
    if let Some(id) = model_id {
        if is_pepe_model(id) {
            return PERSONALITY_PEPE;
        }
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pepe_model() {
        assert!(is_pepe_model("pepe:8b-64k"));
        assert!(is_pepe_model("PEPE:latest"));
        assert!(is_pepe_model("hf.co/user/pepe-model"));
        assert!(!is_pepe_model("llama3.2:latest"));
        assert!(!is_pepe_model("mistral-small"));
    }

    #[test]
    fn test_get_personality_prefix() {
        // Pepe model should return personality
        assert!(!get_personality_prefix(Some("pepe:8b-64k")).is_empty());

        // Non-pepe model should return empty
        assert!(get_personality_prefix(Some("llama3.2")).is_empty());

        // None should return empty
        assert!(get_personality_prefix(None).is_empty());
    }
}
