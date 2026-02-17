//! Translation module
//!
//! Provides functionality for translating text between languages using
//! TranslateGemma model with support for multiple languages, styles,
//! and auto-detection.

pub mod cli;
pub mod language;
pub mod prompt;
pub mod style;

// Re-export commonly used items
pub use cli::{Commands, QueryArgs, TranslateArgs};
pub use language::{parse_language_pair, LanguageCode, LanguageError, LanguageMapper};
pub use prompt::build_translation_prompt;
pub use style::TranslationStyle;

/// Type alias for translation results
pub type TranslationResult<T> = Result<T, TranslationError>;

/// Error type for translation operations
#[derive(Debug)]
pub enum TranslationError {
    Language(LanguageError),
    NoTextProvided,
    ModelNotFound(String),
    OllamaError(String),
    StdinError(std::io::Error),
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::Language(e) => write!(f, "Language error: {}", e),
            TranslationError::NoTextProvided => write!(f, "No text provided for translation"),
            TranslationError::ModelNotFound(m) => {
                write!(f, "Model '{}' not found in configuration", m)
            }
            TranslationError::OllamaError(e) => write!(f, "Ollama error: {}", e),
            TranslationError::StdinError(e) => write!(f, "Failed to read from stdin: {}", e),
        }
    }
}

impl std::error::Error for TranslationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TranslationError::StdinError(e) => Some(e),
            TranslationError::Language(e) => Some(e),
            _ => None,
        }
    }
}

impl From<LanguageError> for TranslationError {
    fn from(e: LanguageError) -> Self {
        TranslationError::Language(e)
    }
}

impl From<std::io::Error> for TranslationError {
    fn from(e: std::io::Error) -> Self {
        TranslationError::StdinError(e)
    }
}
