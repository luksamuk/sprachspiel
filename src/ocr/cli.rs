//! OCR subcommand CLI
//!
//! Defines the command-line interface for the OCR subcommand
//! which extracts text from images using GLM-OCR.

use clap::{Args, ValueEnum};
use std::path::PathBuf;

use super::mode::OcrMode;

/// Arguments for the OCR subcommand
#[derive(Args, Debug)]
#[command(
    about = "Extract text from images using GLM-OCR",
    long_about = r#"Extract text, tables, figures, or formulas from images using the GLM-OCR model.

SUPPORTED IMAGE FORMATS:
  - PNG (.png)
  - JPEG (.jpg, .jpeg)
  - WebP (.webp)
  - GIF (.gif) - first frame only

NOTE: PDF files are NOT supported directly. Convert to image first:
  pdftoppm -png input.pdf output
  convert -density 300 input.pdf[0] output.png

MODES:
  text     - General text recognition (default)
  table    - Extract tables with structure preservation
  figure   - Extract text from figures and diagrams
  formula  - Extract mathematical formulas (LaTeX)

EXAMPLES:
  ask ocr document.png                    # Extract text from image
  ask ocr --table spreadsheet.png       # Extract table structure
  ask ocr --formula equation.png          # Extract LaTeX formulas
  ask ocr --json *.png > output.jsonl   # Batch process with JSON output
  ask ocr page*.png > combined.txt        # Process multiple images

PIPELINES:
  ask ocr japanese.png | ask translate ja:pt    # OCR + translate
  ask ocr report.png | ask summarize             # OCR + summarize

REQUIREMENTS:
  - Ollama must be running locally or accessible remotely
  - GLM-OCR model must be downloaded: ollama pull glm-ocr:bf16
"#
)]
pub struct OcrArgs {
    /// Image file(s) to process
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Extraction mode (text, table, figure, formula)
    #[arg(short, long, value_enum, default_value = "text")]
    pub mode: OcrMode,

    /// Output as JSON (one object per line for batch)
    #[arg(long)]
    pub json: bool,

    /// Maximum tokens per image (default: 8192)
    #[arg(long, default_value = "8192")]
    pub max_tokens: u32,

    /// Enable debug mode with detailed logging
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub debug: Option<bool>,
}

/// Output format for OCR results
#[derive(ValueEnum, Clone, Debug, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain text output (default)
    #[default]
    Text,
    /// JSON format
    Json,
}

impl OcrArgs {
    /// Validate that files are provided
    pub fn validate(&self) -> Result<(), String> {
        if self.files.is_empty() {
            return Err("No image files provided.\n\
                Usage: ask ocr [OPTIONS] <FILE>...\n\
                Try 'ask ocr --help' for more information."
                .to_string());
        }
        Ok(())
    }

    /// Get output format
    #[allow(dead_code)]
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Text
        }
    }

    /// Check if batch mode (multiple files)
    #[allow(dead_code)]
    pub fn is_batch(&self) -> bool {
        self.files.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_ocr_args_validation() {
        let args = OcrArgs {
            files: vec![PathBuf::from("test.png")],
            mode: OcrMode::Text,
            json: false,
            max_tokens: 8192,
            debug: None,
        };
        assert!(args.validate().is_ok());

        let args_empty = OcrArgs {
            files: vec![],
            mode: OcrMode::Text,
            json: false,
            max_tokens: 8192,
            debug: None,
        };
        assert!(args_empty.validate().is_err());
    }

    #[test]
    fn test_is_batch() {
        let single = OcrArgs {
            files: vec![PathBuf::from("test.png")],
            mode: OcrMode::Text,
            json: false,
            max_tokens: 8192,
            debug: None,
        };
        assert!(!single.is_batch());

        let batch = OcrArgs {
            files: vec![PathBuf::from("1.png"), PathBuf::from("2.png")],
            mode: OcrMode::Text,
            json: false,
            max_tokens: 8192,
            debug: None,
        };
        assert!(batch.is_batch());
    }
}
