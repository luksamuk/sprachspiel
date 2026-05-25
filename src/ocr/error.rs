//! OCR error types
//!
//! Provides detailed error messages for OCR operations with helpful
//! suggestions for common issues.

use std::fmt;

/// Error type for OCR operations
#[derive(Debug, Clone)]
pub enum OcrError {
    /// File not found
    FileNotFound(String),
    /// Invalid file extension
    #[allow(dead_code)]
    InvalidExtension(String),
    /// Unsupported image format
    #[allow(dead_code)]
    UnsupportedFormat { found: String, supported: String },
    /// Failed to read file
    ReadFailed { file: String, error: String },
    /// Empty file
    #[allow(dead_code)]
    EmptyFile(String),
    /// Ollama communication error
    OllamaError { message: String },
    /// Model not available
    #[allow(dead_code)]
    ModelNotAvailable(String),
    /// Invalid image data
    #[allow(dead_code)]
    InvalidImage(String),
}

impl fmt::Display for OcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OcrError::FileNotFound(path) => {
                writeln!(f, "Error: File not found: {}", path)?;
                write!(f, "Make sure the file exists and the path is correct.")
            }
            OcrError::InvalidExtension(ext) => {
                writeln!(f, "Error: Invalid file extension: {}", ext)?;
                write!(f, "Please provide a valid image file.")
            }
            OcrError::UnsupportedFormat { found, supported } => {
                writeln!(f, "Error: Unsupported image format: {}", found)?;
                writeln!(f, "Supported formats: {}", supported)?;
                writeln!(f)?;
                write!(f, "Tip: Convert your file using ImageMagick:")
            }
            OcrError::ReadFailed { file, error } => {
                writeln!(f, "Error: Failed to read file: {}", file)?;
                write!(f, "Reason: {}", error)
            }
            OcrError::EmptyFile(path) => {
                writeln!(f, "Error: Empty file: {}", path)?;
                write!(f, "The file appears to be empty or corrupted.")
            }
            OcrError::OllamaError { message } => {
                writeln!(f, "Error: {}", message)?;
                writeln!(f)?;
                writeln!(f, "Common causes:")?;
                writeln!(f, "  1. {}", crate::consts::app::ERR_LLM_NOT_RUNNING)?;
                writeln!(
                    f,
                    "  2. GLM-OCR model not downloaded (run: ollama pull glm-ocr:bf16)"
                )?;
                write!(
                    f,
                    "  3. Connection refused - check if OLLAMA_HOST is set correctly"
                )
            }
            OcrError::ModelNotAvailable(model) => {
                writeln!(f, "Error: Model '{}' not available", model)?;
                write!(f, "Run: ollama pull {}", model)
            }
            OcrError::InvalidImage(path) => {
                writeln!(f, "Error: Invalid or corrupted image: {}", path)?;
                write!(f, "The file may be corrupted or not a valid image.")
            }
        }
    }
}

impl std::error::Error for OcrError {}

/// Type alias for OCR results
pub type OcrResult<T> = Result<T, OcrError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = OcrError::FileNotFound("test.png".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("File not found"));
        assert!(msg.contains("test.png"));

        let err = OcrError::UnsupportedFormat {
            found: "pdf".to_string(),
            supported: "png, jpg".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("pdf"));
        assert!(msg.contains("png, jpg"));

        let err = OcrError::OllamaError {
            message: "Connection refused".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Connection refused"));
        assert!(msg.contains("LLM server is not running"));
    }
}
