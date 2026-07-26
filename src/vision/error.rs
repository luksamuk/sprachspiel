//! Error types for vision subsystem.

use std::fmt;

/// Errors that can occur during vision analysis.
#[derive(Debug)]
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
    /// The selected model does not support vision capabilities.
    NoVisionCapability {
        model: String,
    },
}

impl fmt::Display for VisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_message())
    }
}

impl VisionError {
    fn format_message(&self) -> String {
        match self {
            VisionError::FileNotFound(path) => format_file_not_found(path),
            VisionError::InvalidExtension(ext) => format_invalid_extension(ext),
            VisionError::UnsupportedFormat { found, supported } => {
                format_unsupported_format(found, supported)
            }
            VisionError::ReadFailed { file, error } => format_read_failed(file, error),
            VisionError::OllamaError { message } => format_ollama_error(message),
            VisionError::NoImages => format_no_images(),
            VisionError::NoVisionCapability { model } => format_no_vision_capability(model),
        }
    }
}

fn format_file_not_found(path: &str) -> String {
    format!(
        "Error: File not found: {}\nMake sure the file exists and the path is correct.",
        path
    )
}

fn format_invalid_extension(ext: &str) -> String {
    format!(
        "Error: Invalid file extension: {}\nPlease provide a valid image file.",
        ext
    )
}

fn format_unsupported_format(found: &str, supported: &str) -> String {
    format!(
        "Error: Unsupported image format: {}\nSupported formats: {}\n\nTip: Convert your file using ImageMagick:",
        found, supported
    )
}

fn format_read_failed(file: &str, error: &str) -> String {
    format!("Error: Failed to read file: {}\nReason: {}", file, error)
}

fn format_ollama_error(message: &str) -> String {
    format!(
        "Error: {}\n\nCommon causes:\n  1. {}\n  2. Vision model not downloaded (run: ollama pull qwen3.5:4b)\n  3. Connection refused - check if OLLAMA_HOST is set correctly",
        message,
        crate::consts::app::ERR_LLM_NOT_RUNNING
    )
}

fn format_no_images() -> String {
    "Error: No image files provided.\n\nUsage: ask vision [OPTIONS] <FILE>...\nTry 'ask vision --help' for more information.".to_string()
}

fn format_no_vision_capability(model: &str) -> String {
    format!(
        "Error: Model '{}' does not support vision capabilities.\n\nVision-capable models you can use:\n  - qwen3.5:4b    (recommended, local, multimodal)\n  - qwen3.5:cloud  (cloud, multimodal)\n  - moondream:1.8b (lightweight, local)\n  - minicpm-v:8b   (best for multi-image)\n\nSet a vision model in config.toml:\n  [model.vision]\n  model = \"qwen3.5:4b\"",
        model
    )
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
        assert!(msg.contains("LLM server is not running"));

        let err = VisionError::NoVisionCapability {
            model: "llama3.2:3b".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("llama3.2:3b"));
        assert!(msg.contains("vision"));
        assert!(msg.contains("qwen3.5:4b"));
    }
}
