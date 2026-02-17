//! OCR extraction modes
//!
//! Defines the different extraction modes supported by GLM-OCR:
//! - Text: General text recognition
//! - Table: Table structure extraction
//! - Figure: Figure and diagram recognition
//! - Formula: Mathematical formula extraction (LaTeX)

use clap::ValueEnum;

/// OCR extraction mode
#[derive(ValueEnum, Clone, Debug, Copy, PartialEq, Eq, Default)]
pub enum OcrMode {
    /// General text recognition (default)
    #[default]
    Text,
    /// Table structure extraction
    Table,
    /// Figure and diagram recognition
    Figure,
    /// Mathematical formula extraction
    Formula,
}

impl OcrMode {
    /// Get the prompt prefix for this mode
    pub fn into_prompt(self) -> &'static str {
        match self {
            OcrMode::Text => "Text Recognition:",
            OcrMode::Table => "Table Recognition:",
            OcrMode::Figure => "Figure Recognition:",
            OcrMode::Formula => "Formula Recognition:",
        }
    }

    /// Get a description of this mode
    pub fn description(&self) -> &'static str {
        match self {
            OcrMode::Text => "Extract general text content",
            OcrMode::Table => "Extract tables preserving structure",
            OcrMode::Figure => "Extract text from figures and diagrams",
            OcrMode::Formula => "Extract mathematical formulas in LaTeX",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_prompts() {
        assert_eq!(OcrMode::Text.into_prompt(), "Text Recognition:");
        assert_eq!(OcrMode::Table.into_prompt(), "Table Recognition:");
        assert_eq!(OcrMode::Figure.into_prompt(), "Figure Recognition:");
        assert_eq!(OcrMode::Formula.into_prompt(), "Formula Recognition:");
    }

    #[test]
    fn test_default() {
        assert!(matches!(OcrMode::default(), OcrMode::Text));
    }
}
