//! Vision error types
//!
//! Provides detailed error messages for vision operations with helpful
//! suggestions for common issues.

use std::fmt;

#[derive(Debug, Clone)]
pub enum VisionError {
    FileNotFound(String),
    #[allow(dead_code)]
    InvalidExtension(String),
    #[allow(dead_code)]
    UnsupportedFormat {
        found: String,
        supported: String,
    },
    ReadFailed {
        file: String,
        error: String,
    },
    OllamaError {
        message: String,
    },
    NoImages,
    /// PDF-specific errors (tool not found, conversion failed, etc.)
    PdfConversionError {
        message: String,
    },
    /// PDF support is not available (missing tools or other issues)
    PdfSupport {
        message: String,
    },
}

impl fmt::Display for VisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisionError::FileNotFound(path) => {
                writeln!(f, "Error: File not found: {}", path)?;
                write!(f, "Make sure the file exists and the path is correct.")
            }
            VisionError::InvalidExtension(ext) => {
                writeln!(f, "Error: Invalid file extension: {}", ext)?;
                write!(f, "Please provide a valid image file.")
            }
            VisionError::UnsupportedFormat { found, supported } => {
                writeln!(f, "Error: Unsupported image format: {}", found)?;
                writeln!(f, "Supported formats: {}", supported)?;
                writeln!(f)?;
                write!(f, "Tip: Convert your file using ImageMagick:")
            }
            VisionError::ReadFailed { file, error } => {
                writeln!(f, "Error: Failed to read file: {}", file)?;
                write!(f, "Reason: {}", error)
            }
            VisionError::OllamaError { message } => {
                writeln!(f, "Error: {}", message)?;
                writeln!(f)?;
                writeln!(f, "Common causes:")?;
                writeln!(f, "  1. Ollama daemon is not running (run: ollama serve)")?;
                writeln!(
                    f,
                    "  2. Vision model not downloaded (run: ollama pull qwen3.5:4b)"
                )?;
                write!(
                    f,
                    "  3. Connection refused - check if OLLAMA_HOST is set correctly"
                )
            }
            VisionError::NoImages => {
                writeln!(f, "Error: No image files provided.")?;
                writeln!(f)?;
                write!(
                    f,
                    "Usage: ask vision [OPTIONS] <FILE>...\nTry 'ask vision --help' for more information."
                )
            }
            VisionError::PdfConversionError { message } => {
                writeln!(f, "Error: PDF conversion failed: {}", message)?;
                writeln!(f)?;
                writeln!(f, "Common causes:")?;
                writeln!(f, "  1. poppler-utils is not installed")?;
                writeln!(f, "  2. The PDF file may be corrupted or password-protected")?;
                writeln!(f)?;
                write!(
                    f,
                    "Install poppler-utils:\n  Arch: sudo pacman -S poppler\n  Debian: sudo apt install poppler-utils\n  Fedora: sudo dnf install poppler-utils"
                )
            }
            VisionError::PdfSupport { message } => {
                writeln!(f, "Error: PDF support unavailable: {}", message)?;
                writeln!(f)?;
                write!(
                    f,
                    "PDF processing requires poppler-utils (pdftoppm, pdfinfo, pdftotext).\nInstall with:\n  Arch: sudo pacman -S poppler\n  Debian: sudo apt install poppler-utils\n  Fedora: sudo dnf install poppler-utils\n  Termux: pkg install poppler"
                )
            }
        }
    }
}

impl std::error::Error for VisionError {}

pub type VisionResult<T> = Result<T, VisionError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = VisionError::FileNotFound("test.png".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("File not found"));
        assert!(msg.contains("test.png"));

        let err = VisionError::UnsupportedFormat {
            found: "pdf".to_string(),
            supported: "png, jpg".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("pdf"));
        assert!(msg.contains("png, jpg"));

        let err = VisionError::OllamaError {
            message: "Connection refused".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Connection refused"));
        assert!(msg.contains("ollama serve"));
    }

    #[test]
    fn test_pdf_conversion_error() {
        let err = VisionError::PdfConversionError {
            message: "pdftoppm failed".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("PDF conversion failed"));
        assert!(msg.contains("pdftoppm failed"));
        assert!(msg.contains("poppler-utils"));
    }

    #[test]
    fn test_pdf_support_error() {
        let err = VisionError::PdfSupport {
            message: "pdftoppm not found".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("PDF support unavailable"));
        assert!(msg.contains("pdftoppm not found"));
        assert!(msg.contains("poppler-utils"));
    }
}