//! Translation style definitions
//!
//! Supports predefined styles (formal, casual, technical, literary)
//! and custom style instructions.

use crate::utils::normalize_input;

/// Translation style enum with predefined options and custom support
#[derive(Debug, Clone, Default)]
pub enum TranslationStyle {
    /// Formal, professional language with proper etiquette
    #[default]
    Formal,
    /// Informal, conversational language with colloquialisms
    Casual,
    /// Technical documentation style preserving terminology
    Technical,
    /// Literary translation preserving style and flow
    Literary,
    /// Custom user-provided instruction
    Custom(String),
}

impl TranslationStyle {
    /// Parse string to TranslationStyle
    /// Recognizes: formal, casual, technical, literary
    /// Anything else becomes Custom(style_instruction)
    pub fn parse(s: &str) -> Self {
        let normalized = normalize_input(s);

        match normalized.as_str() {
            "formal" => Self::Formal,
            "casual" => Self::Casual,
            "technical" => Self::Technical,
            "literary" => Self::Literary,
            _ => Self::Custom(s.to_string()),
        }
    }

    /// Get the style instruction for the prompt
    pub fn to_instruction(&self) -> &str {
        match self {
            Self::Formal => {
                "Use formal, professional language with proper etiquette and polite forms. \
                Maintain a respectful and business-appropriate tone."
            }
            Self::Casual => {
                "Use informal, conversational language with natural colloquialisms. \
                Keep it friendly and accessible like everyday speech."
            }
            Self::Technical => {
                "Preserve all technical terminology, function names, APIs, and code snippets exactly. \
                Use precise technical language appropriate for documentation."
            }
            Self::Literary => {
                "Preserve the poetic and literary qualities of the original. \
                Focus on style, flow, rhythm, and cultural nuances. \
                Maintain the emotional tone and artistic intent."
            }
            Self::Custom(instr) => instr,
        }
    }

    /// Get display name
    #[allow(dead_code)]
    pub fn display_name(&self) -> String {
        match self {
            Self::Formal => "Formal".to_string(),
            Self::Casual => "Casual".to_string(),
            Self::Technical => "Technical".to_string(),
            Self::Literary => "Literary".to_string(),
            Self::Custom(_) => "Custom".to_string(),
        }
    }

    /// Check if this is a custom style
    #[allow(dead_code)]
    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }

    /// Get the raw custom string if custom, None otherwise
    #[allow(dead_code)]
    pub fn custom_instruction(&self) -> Option<&str> {
        match self {
            Self::Custom(s) => Some(s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_predefined() {
        assert!(matches!(
            TranslationStyle::parse("formal"),
            TranslationStyle::Formal
        ));
        assert!(matches!(
            TranslationStyle::parse("FORMAL"),
            TranslationStyle::Formal
        ));
        assert!(matches!(
            TranslationStyle::parse("Formal"),
            TranslationStyle::Formal
        ));
        assert!(matches!(
            TranslationStyle::parse("casual"),
            TranslationStyle::Casual
        ));
        assert!(matches!(
            TranslationStyle::parse("technical"),
            TranslationStyle::Technical
        ));
        assert!(matches!(
            TranslationStyle::parse("literary"),
            TranslationStyle::Literary
        ));
    }

    #[test]
    fn test_parse_custom() {
        let style = TranslationStyle::parse("use Brazilian slang");
        assert!(matches!(style, TranslationStyle::Custom(_)));
        assert_eq!(style.custom_instruction(), Some("use Brazilian slang"));
    }

    #[test]
    fn test_to_instruction() {
        let formal = TranslationStyle::Formal;
        assert!(formal.to_instruction().contains("formal"));
        assert!(formal.to_instruction().contains("professional"));

        let casual = TranslationStyle::Casual;
        assert!(casual.to_instruction().contains("informal"));
        assert!(casual.to_instruction().contains("conversational"));

        let technical = TranslationStyle::Technical;
        assert!(technical.to_instruction().contains("technical terminology"));
        assert!(technical.to_instruction().contains("code snippets"));

        let literary = TranslationStyle::Literary;
        assert!(literary.to_instruction().contains("poetic"));
        assert!(literary.to_instruction().contains("artistic intent"));
    }

    #[test]
    fn test_custom_instruction() {
        let custom = TranslationStyle::Custom("use very formal academic language".to_string());
        assert_eq!(custom.to_instruction(), "use very formal academic language");
        assert!(custom.is_custom());
        assert_eq!(
            custom.custom_instruction(),
            Some("use very formal academic language")
        );
    }

    #[test]
    fn test_display_name() {
        assert_eq!(TranslationStyle::Formal.display_name(), "Formal");
        assert_eq!(TranslationStyle::Casual.display_name(), "Casual");
        assert_eq!(TranslationStyle::Technical.display_name(), "Technical");
        assert_eq!(TranslationStyle::Literary.display_name(), "Literary");
        assert_eq!(
            TranslationStyle::Custom("anything".to_string()).display_name(),
            "Custom"
        );
    }
}
