//! Vision subcommand CLI
//!
//! Defines the command-line interface for the vision subcommand
//! which describes and analyzes images using vision models.

use clap::Args;
use std::path::PathBuf;

/// Long help for `vision`, rendered by [`crate::consts::app::help_text`].
///
/// Declared here, not as `#[command(long_about = ...)]` on the args struct:
/// a doc-comment on a `Subcommand` variant takes precedence over the struct's
/// help attributes, so clap compiles that text in and never prints it.
/// The `Commands` variant references this constant.
pub const VISION_LONG_ABOUT: &str = r#"Analyze and describe images using vision models like qwen3.5, moondream, llava, or minicpm-v.

SUPPORTED FORMATS:
  - PNG (.png)
  - JPEG (.jpg, .jpeg)
  - WebP (.webp)
  - GIF (.gif) - first frame only

MODES:
  default   - Brief description of the image
  detailed  - Detailed analysis including composition, colors, subjects

MULTI-IMAGE:
  Multiple images are processed in a single API call using the images array.
  For best multi-image results, use minicpm-v:8b model with -m flag.

EXAMPLES:
  sprach vision photo.png                         # Brief description
  sprach vision --detailed photo.png              # Detailed analysis
  sprach vision screenshot.png -- "What UI elements are visible?"
  sprach vision img1.png img2.png -m minicpm-v    # Compare images
  sprach vision --json *.png > output.jsonl       # Batch with JSON output

MODELS:
  - qwen3.5:4b (recommended) - Multimodal, good quality, 128K context
  - moondream:1.8b - Lightweight alternative, 2K context
  - llava:7b - Better quality, good OCR
  - minicpm-v:8b - Best for multi-image tasks

MODEL SELECTION ORDER:
  -m/--model  >  [model.vision] in config.toml  >  global [model].default

REQUIREMENTS:
  - Ollama must be running locally or accessible remotely
  - Vision model must be downloaded: ollama pull qwen3.5:4b

CONFIGURATION:
  Default model can be set in ~/.config/sprachspiel/config.toml:
  [model.vision]
  model = "qwen3.5:4b"
"#;

#[derive(Args, Debug, Clone)]
#[command(about = "Analyze and describe images using vision models")]
pub struct VisionArgs {
    /// Image file(s) to analyze
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Custom prompt for analysis (overrides mode, must be last argument)
    #[arg(value_name = "PROMPT", last = true)]
    pub prompt: Option<String>,

    /// Detailed analysis mode
    #[arg(long)]
    pub detailed: bool,

    /// Output as JSON (one object per result)
    #[arg(long)]
    pub json: bool,

    /// Model to use (default: the global `[model].default` from config.toml)
    #[arg(short, long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Maximum tokens per image (default: 2048)
    #[arg(long, default_value = "2048")]
    pub max_tokens: u32,
}

impl VisionArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.files.is_empty() {
            return Err("No image files provided.\n\
                Usage: sprach vision [OPTIONS] <FILE>...\n\
                Try 'sprach vision --help' for more information."
                .to_string());
        }
        Ok(())
    }

    pub fn get_prompt(&self) -> String {
        if let Some(ref prompt) = self.prompt {
            return prompt.clone();
        }
        if self.detailed {
            return "Describe this image in detail, including composition, colors, subjects, and any notable elements.".to_string();
        }
        "Describe this image.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_args_validation() {
        let args = VisionArgs {
            files: vec![PathBuf::from("test.png")],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
        };
        assert!(args.validate().is_ok());

        let args_empty = VisionArgs {
            files: vec![],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
        };
        assert!(args_empty.validate().is_err());
    }

    #[test]
    fn test_get_prompt_default() {
        let args = VisionArgs {
            files: vec![PathBuf::from("test.png")],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
        };
        assert_eq!(args.get_prompt(), "Describe this image.");
    }

    #[test]
    fn test_get_prompt_detailed() {
        let args = VisionArgs {
            files: vec![PathBuf::from("test.png")],
            prompt: None,
            detailed: true,
            json: false,
            model: None,
            max_tokens: 2048,
        };
        assert!(args.get_prompt().contains("detail"));
    }

    #[test]
    fn test_get_prompt_custom() {
        let args = VisionArgs {
            files: vec![PathBuf::from("test.png")],
            prompt: Some("What objects are in this image?".to_string()),
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
        };
        assert_eq!(args.get_prompt(), "What objects are in this image?");
    }
}
