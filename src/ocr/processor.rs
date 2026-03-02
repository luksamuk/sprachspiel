//! OCR processor
//!
//! Handles the actual OCR processing using the GLM-OCR model via Ollama.
//! Uses /api/generate endpoint as recommended by GLM-OCR documentation.

use base64::Engine;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::images::Image;
use ollama_rs::models::ModelOptions;
use std::path::Path;

use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::utils::validate_image_file;

use super::cli::OcrArgs;
use super::error::{OcrError, OcrResult};
use super::mode::OcrMode;

/// OCR processor that handles image extraction
pub struct OcrProcessor;

impl OcrProcessor {
    /// Create a new OCR processor
    pub fn new() -> Self {
        Self
    }

    /// Process a single image file
    pub async fn process_file(
        &self,
        path: &Path,
        mode: OcrMode,
        settings: &Settings,
    ) -> OcrResult<OcrOutput> {
        validate_image_file(path).map_err(OcrError::FileNotFound)?;

        let image_bytes = tokio::fs::read(path)
            .await
            .map_err(|e| OcrError::ReadFailed {
                file: path.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_bytes);
        let image = Image::from_base64(base64_image);
        let prompt = mode.into_prompt();
        let ollama = settings.ollama_client();
        let model_options = ModelOptions::default().temperature(0.0);

        // Create generation request with the image attached
        let request = GenerationRequest::new("glm-ocr:bf16".to_string(), prompt)
            .options(model_options)
            .add_image(image); // <-- Actually sends the image data

        // Show spinner
        let spinner = create_spinner(&format!(
            "Extracting {} from {}...",
            mode.description(),
            path.display()
        ));

        // Send request to /api/generate
        let response = ollama
            .generate(request)
            .await
            .map_err(|e| OcrError::OllamaError {
                message: format!("Failed to process image: {}", e),
            })?;

        // Clear spinner
        finish_spinner(spinner);

        let content = response.response.trim().to_string();

        Ok(OcrOutput {
            file: path.to_string_lossy().to_string(),
            mode,
            content,
        })
    }

    /// Process multiple files
    pub async fn process_batch(
        &self,
        args: &OcrArgs,
        settings: &Settings,
    ) -> OcrResult<Vec<OcrOutput>> {
        let mut results = Vec::new();

        for file in &args.files {
            match self.process_file(file, args.mode, settings).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    eprintln!("Error processing {}: {}", file.display(), e);
                    // Continue with other files in batch mode
                }
            }
        }

        if results.is_empty() {
            return Err(OcrError::OllamaError {
                message: "All files failed to process".to_string(),
            });
        }

        Ok(results)
    }
}

impl Default for OcrProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// OCR result for a single file
#[derive(Debug, Clone)]
pub struct OcrOutput {
    pub file: String,
    pub mode: OcrMode,
    pub content: String,
}

/// Print OCR results
pub fn print_results(results: &[OcrOutput], json_output: bool) {
    if json_output {
        // JSON output (one per line for batch)
        for result in results {
            let json = serde_json::json!({
                "file": result.file,
                "mode": format!("{:?}", result.mode).to_lowercase(),
                "content": result.content,
            });
            println!("{}", json);
        }
    } else {
        // Plain text output with separators for batch
        let is_batch = results.len() > 1;

        for (i, result) in results.iter().enumerate() {
            if is_batch {
                // Print separator with filename
                println!("=== {} ===", result.file);
                println!();
            }

            println!("{}", result.content);

            // Add blank line between results in batch mode
            if is_batch && i < results.len() - 1 {
                println!();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::validate_image_file;

    #[test]
    fn test_validate_file_extension() {
        assert!(validate_image_file(Path::new("test.png")).is_err());
        assert!(validate_image_file(Path::new("test.jpg")).is_err());

        assert!(validate_image_file(Path::new("test.pdf")).is_err());

        assert!(validate_image_file(Path::new("test.txt")).is_err());
    }

    #[test]
    fn test_ocr_result_display() {
        let result = OcrOutput {
            file: "test.png".to_string(),
            mode: OcrMode::Text,
            content: "Hello World".to_string(),
        };

        assert_eq!(result.file, "test.png");
        assert!(matches!(result.mode, OcrMode::Text));
        assert_eq!(result.content, "Hello World");
    }
}
