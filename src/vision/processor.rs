//! Vision processor
//!
//! Handles image analysis using vision models via Ollama.
//! Uses /api/generate endpoint with images array for multi-image support.

use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::images::Image;
use ollama_rs::models::ModelOptions;

use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::utils::{read_file_as_base64, validate_image_file};

use super::cli::VisionArgs;
use super::error::{VisionError, VisionResult};

pub struct VisionProcessor;

impl VisionProcessor {
    pub fn new() -> Self {
        Self
    }

    pub async fn process(
        &self,
        args: &VisionArgs,
        model: &str,
        settings: &Settings,
    ) -> VisionResult<VisionOutput> {
        if args.files.is_empty() {
            return Err(VisionError::NoImages);
        }

        let mut images = Vec::new();
        for file in &args.files {
            validate_image_file(file).map_err(VisionError::FileNotFound)?;

            let base64_image =
                read_file_as_base64(file)
                    .await
                    .map_err(|e| VisionError::ReadFailed {
                        file: file.to_string_lossy().to_string(),
                        error: e,
                    })?;

            images.push(Image::from_base64(base64_image));
        }

        let prompt = args.get_prompt().to_string();

        let ollama = settings.ollama_client();

        let model_options = ModelOptions::default()
            .temperature(0.1)
            .num_predict(args.max_tokens as i32);

        let mut request = GenerationRequest::new(model.to_string(), prompt).options(model_options);

        for image in images {
            request = request.add_image(image);
        }

        let file_count = args.files.len();
        let spinner_msg = if file_count == 1 {
            format!("Analyzing {}...", args.files[0].display())
        } else {
            format!("Analyzing {} images...", file_count)
        };
        let spinner = create_spinner(&spinner_msg);

        let response = ollama
            .generate(request)
            .await
            .map_err(|e| VisionError::OllamaError {
                message: format!("Failed to process image(s): {}", e),
            })?;

        finish_spinner(spinner);

        let content = response.response.trim().to_string();

        Ok(VisionOutput {
            files: args
                .files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            prompt: args.get_prompt(),
            content,
        })
    }
}

impl Default for VisionProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct VisionOutput {
    pub files: Vec<String>,
    pub prompt: String,
    pub content: String,
}

pub fn print_results(result: &VisionOutput, json_output: bool) {
    if json_output {
        let json = serde_json::json!({
            "files": result.files,
            "prompt": result.prompt,
            "content": result.content,
        });
        println!("{}", json);
    } else {
        let is_multi = result.files.len() > 1;

        if is_multi {
            println!("Files: {}", result.files.join(", "));
            println!();
        }

        println!("{}", result.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_validate_image_file() {
        assert!(validate_image_file(Path::new("test.png")).is_err());
        assert!(validate_image_file(Path::new("test.jpg")).is_err());
        assert!(validate_image_file(Path::new("test.pdf")).is_err());
    }
}
