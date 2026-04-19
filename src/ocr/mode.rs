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
            OcrMode::Text => "text",
            OcrMode::Table => "tables",
            OcrMode::Figure => "figures",
            OcrMode::Formula => "formulas",
        }
    }

    /// Get the descriptive prompt for vision models
    pub fn into_descriptive_prompt(self) -> &'static str {
        match self {
            OcrMode::Text => "Extract all text from this image. Preserve layout and structure. Output ONLY the extracted text, no analysis or commentary.",
            OcrMode::Table => "Extract the table structure from this image. Preserve rows and columns. Output ONLY the table data in markdown format, no analysis or commentary.",
            OcrMode::Figure => "Extract and describe the figure or diagram in this image. Output ONLY a description of what is depicted, no analysis or commentary beyond the figure content.",
            OcrMode::Formula => "Extract all mathematical formulas from this image. Output ONLY the formulas in LaTeX notation, no analysis or commentary.",
        }
    }
}

/// Returns true if the given model_id refers to a GLM-OCR model variant.
pub fn is_glm_ocr_model(model_id: &str) -> bool {
    model_id.starts_with("glm-ocr")
}

/// Parse an OCR mode from an optional input string.
/// Returns `OcrMode::Text` for None or empty input.
pub fn parse_ocr_mode(input: Option<String>) -> Result<OcrMode, String> {
    let input = input.filter(|s| !s.is_empty());
    match input {
        None => Ok(OcrMode::Text),
        Some(s) => OcrMode::from_str(&s, true).map_err(|_| {
            format!(
                "Error: Invalid OCR mode '{}'. Valid modes: text, table, figure, formula",
                s
            )
        }),
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

    #[test]
    fn test_descriptive_prompts() {
        assert!(
            OcrMode::Text.into_descriptive_prompt().contains("ONLY")
                && OcrMode::Text.into_descriptive_prompt().contains("no")
                && OcrMode::Text
                    .into_descriptive_prompt()
                    .contains("commentary")
        );
        assert!(
            OcrMode::Table.into_descriptive_prompt().contains("ONLY")
                && OcrMode::Table.into_descriptive_prompt().contains("no")
                && OcrMode::Table
                    .into_descriptive_prompt()
                    .contains("commentary")
        );
        assert!(
            OcrMode::Figure.into_descriptive_prompt().contains("ONLY")
                && OcrMode::Figure.into_descriptive_prompt().contains("no")
                && OcrMode::Figure
                    .into_descriptive_prompt()
                    .contains("commentary")
        );
        assert!(
            OcrMode::Formula.into_descriptive_prompt().contains("ONLY")
                && OcrMode::Formula.into_descriptive_prompt().contains("no")
                && OcrMode::Formula
                    .into_descriptive_prompt()
                    .contains("commentary")
        );
    }

    #[test]
    fn test_is_glm_ocr_model() {
        assert!(is_glm_ocr_model("glm-ocr:bf16"));
        assert!(!is_glm_ocr_model("qwen3.5:cloud"));
        assert!(is_glm_ocr_model("glm-ocr"));
        assert!(is_glm_ocr_model("glm-ocr-custom:q4"));
        assert!(!is_glm_ocr_model("minicpm-v"));
    }

    #[test]
    fn test_parse_ocr_mode() {
        assert_eq!(parse_ocr_mode(None).unwrap(), OcrMode::Text);
        assert_eq!(
            parse_ocr_mode(Some("table".to_string())).unwrap(),
            OcrMode::Table
        );
        assert_eq!(
            parse_ocr_mode(Some("FIGURE".to_string())).unwrap(),
            OcrMode::Figure
        );
        assert_eq!(
            parse_ocr_mode(Some("formula".to_string())).unwrap(),
            OcrMode::Formula
        );
        assert!(parse_ocr_mode(Some("invalid".to_string())).is_err());
        let err = parse_ocr_mode(Some("invalid".to_string())).unwrap_err();
        assert!(err.contains("text, table, figure, formula"));
        assert_eq!(parse_ocr_mode(Some("".to_string())).unwrap(), OcrMode::Text);
    }
}
