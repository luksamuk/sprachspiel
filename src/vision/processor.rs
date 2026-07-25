//! Vision processor
//!
//! Handles image analysis using vision models via Ollama.
//! Uses /api/generate endpoint with images array for multi-image support.

#![expect(clippy::print_stdout)] // CLI subcommand output

use crate::provider::types::ProviderOptions;

use crate::spinner::{create_spinner, finish_spinner};
use crate::utils::read_file_as_base64;

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
        provider: &dyn crate::provider::LlmProvider,
        model_options: ProviderOptions,
        show_spinner: bool,
    ) -> VisionResult<VisionOutput> {
        if args.files.is_empty() {
            return Err(VisionError::NoImages);
        }

        for file in &args.files {
            crate::utils::validate_image_file(file).map_err(VisionError::FileNotFound)?;
        }

        let images = self.load_images(&args.files).await?;

        let prompt = args.get_prompt().to_string();
        let mut model_opts = model_options;
        model_opts.num_predict = Some(args.max_tokens as i32);

        let file_count = args.files.len();
        let spinner_msg = if file_count == 1 {
            format!("Analyzing {}...", args.files[0].display())
        } else {
            format!("Analyzing {} images...", file_count)
        };
        let spinner = if show_spinner {
            Some(create_spinner(&spinner_msg))
        } else {
            None
        };

        let result = self
            .call_vision_model(model, &prompt, images, model_opts, provider)
            .await?;

        if let Some(sp) = spinner {
            finish_spinner(sp);
        }

        Ok(VisionOutput {
            files: args
                .files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            prompt,
            content: result,
        })
    }

    /// Load multiple image files as base64 strings
    async fn load_images(&self, files: &[std::path::PathBuf]) -> VisionResult<Vec<String>> {
        let mut images = Vec::new();
        for file in files {
            let base64 = read_file_as_base64(file)
                .await
                .map_err(|e| VisionError::ReadFailed {
                    file: file.to_string_lossy().to_string(),
                    error: e,
                })?;
            log::debug!(
                "Loaded image: {} ({} bytes base64)",
                file.display(),
                base64.len()
            );
            images.push(base64);
        }
        Ok(images)
    }

    /// Call the vision model with images and prompt
    async fn call_vision_model(
        &self,
        model: &str,
        prompt: &str,
        images: Vec<String>,
        model_options: ProviderOptions,
        provider: &dyn crate::provider::LlmProvider,
    ) -> VisionResult<String> {
        let content = provider
            .generate(model, prompt, images, vec![], model_options)
            .await
            .map_err(|e| VisionError::OllamaError {
                message: format!("Failed to process image(s): {e}"),
            })?;

        Ok(content.trim().to_string())
    }
}

impl Default for VisionProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Output types and printing
// ---------------------------------------------------------------------------

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
    use std::path::Path;

    #[test]
    fn test_validate_image_file() {
        assert!(crate::utils::validate_image_file(Path::new("test.png")).is_err()); // doesn't exist
        assert!(crate::utils::validate_image_file(Path::new("test.jpg")).is_err());
        assert!(crate::utils::validate_image_file(Path::new("test.pdf")).is_err());
    }
}
