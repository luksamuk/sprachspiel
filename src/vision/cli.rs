//! Vision subcommand CLI
//!
//! Defines the command-line interface for the vision subcommand
//! which describes and analyzes images using vision models.

use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
#[command(
    about = "Analyze and describe images using vision models",
    long_about = r#"Analyze and describe images using vision models like qwen3.5, moondream, llava, or minicpm-v.

SUPPORTED FORMATS:
  Images:
  - PNG (.png)
  - JPEG (.jpg, .jpeg)
  - WebP (.webp)
  - GIF (.gif) - first frame only

  Documents:
  - PDF (.pdf) - each page converted to images and analyzed

MODES:
  default   - Brief description of the image
  detailed  - Detailed analysis including composition, colors, subjects

PDF SUPPORT:
  When a PDF file is provided, each page is converted to a PNG image using
  pdftoppm (from poppler-utils) and processed individually by the vision model.
  Use --pages to select a specific page range (e.g., "1-5" or "1,3,5").

  For large PDFs, processing happens sequentially to avoid overwhelming Ollama.
  Progress is saved every 20 pages so processing can be resumed if interrupted.

  Requires: poppler-utils (pdftoppm, pdfinfo, pdftotext)
  Install:  sudo apt install poppler-utils    (Debian/Ubuntu)
            sudo pacman -S poppler             (Arch)
            sudo dnf install poppler-utils     (Fedora)

MULTI-IMAGE:
  Multiple images are processed in a single API call using the images array.
  For best multi-image results, use minicpm-v:8b model with -m flag.

EXAMPLES:
  ask vision photo.png                        # Brief description
  ask vision --detailed photo.png             # Detailed analysis
  ask vision screenshot.png "What UI elements are visible?"
  ask vision img1.png img2.png -m minicpm-v   # Compare images
  ask vision --json *.png > output.jsonl     # Batch with JSON output
  ask vision document.pdf                     # Analyze all PDF pages
  ask vision --pages 1-5 document.pdf         # Analyze pages 1-5
  ask vision --pages 1,3,7 document.pdf       # Analyze specific pages

MODELS:
  - qwen3.5:4b (default) - Multimodal, good quality, 128K context
  - moondream:1.8b - Lightweight alternative, 2K context
  - llava:7b - Better quality, good OCR
  - minicpm-v:8b - Best for multi-image tasks

REQUIREMENTS:
  - Ollama must be running locally or accessible remotely
  - Vision model must be downloaded: ollama pull qwen3.5:4b
  - For PDF: poppler-utils must be installed

CONFIGURATION:
  Default model can be set in ~/.config/ask-ai/config.toml:
  [model.vision]
  model = "qwen3.5:4b"
"#
)]
pub struct VisionArgs {
    /// Image or PDF file(s) to analyze
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

    /// Model to use (default: moondream)
    #[arg(short, long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Maximum tokens per image (default: 2048)
    #[arg(long, default_value = "2048")]
    pub max_tokens: u32,

    /// Page range for PDF files (e.g., "1-5" or "1,3,5"). Default: all pages
    #[arg(long, value_name = "PAGES")]
    pub pages: Option<String>,
}

impl VisionArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.files.is_empty() {
            return Err("No image files provided.\n\
                Usage: ask vision [OPTIONS] <FILE>...\n\
                Try 'ask vision --help' for more information."
                .to_string());
        }
        if let Some(ref pages) = self.pages
            && let Err(e) = parse_page_range(pages)
        {
            return Err(format!("Invalid --pages value '{}': {}", pages, e));
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

/// Parse a page range string into a list of 1-based page numbers.
///
/// Supports:
/// - Single page: "5"
/// - Range: "1-5" (inclusive)
/// - Comma-separated: "1,3,5"
/// - Combination: "1-3,7,10-12"
pub fn parse_page_range(input: &str) -> Result<Vec<usize>, String> {
    let mut pages = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(dash_pos) = part.find('-') {
            let start_str = &part[..dash_pos].trim();
            let end_str = &part[dash_pos + 1..].trim();
            let start: usize = start_str
                .parse()
                .map_err(|_| format!("invalid page number: '{}'", start_str))?;
            let end: usize = end_str
                .parse()
                .map_err(|_| format!("invalid page number: '{}'", end_str))?;
            if start == 0 || end == 0 {
                return Err("page numbers must be 1-based (use 1 for the first page)".to_string());
            }
            if start > end {
                return Err(format!("invalid range: {}-{} (start > end)", start, end));
            }
            for p in start..=end {
                if !pages.contains(&p) {
                    pages.push(p);
                }
            }
        } else {
            let page: usize = part
                .parse()
                .map_err(|_| format!("invalid page number: '{}'", part))?;
            if page == 0 {
                return Err("page numbers must be 1-based (use 1 for the first page)".to_string());
            }
            if !pages.contains(&page) {
                pages.push(page);
            }
        }
    }
    pages.sort();
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_vision_args_validation() {
        let args = VisionArgs {
            files: vec![PathBuf::from("test.png")],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
            pages: None,
        };
        assert!(args.validate().is_ok());

        let args_empty = VisionArgs {
            files: vec![],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
            pages: None,
        };
        assert!(args_empty.validate().is_err());
    }

    #[test]
    fn test_vision_args_pages_validation() {
        let args = VisionArgs {
            files: vec![PathBuf::from("test.pdf")],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
            pages: Some("1-5".to_string()),
        };
        assert!(args.validate().is_ok());

        let args_bad = VisionArgs {
            files: vec![PathBuf::from("test.pdf")],
            prompt: None,
            detailed: false,
            json: false,
            model: None,
            max_tokens: 2048,
            pages: Some("0-5".to_string()),
        };
        assert!(args_bad.validate().is_err());
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
            pages: None,
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
            pages: None,
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
            pages: None,
        };
        assert_eq!(args.get_prompt(), "What objects are in this image?");
    }

    #[test]
    fn test_parse_page_range_single() {
        assert_eq!(parse_page_range("5").unwrap(), vec![5]);
    }

    #[test]
    fn test_parse_page_range_range() {
        assert_eq!(parse_page_range("1-5").unwrap(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_page_range_comma() {
        assert_eq!(parse_page_range("1,3,5").unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn test_parse_page_range_combined() {
        assert_eq!(parse_page_range("1-3,7,10-12").unwrap(), vec![1, 2, 3, 7, 10, 11, 12]);
    }

    #[test]
    fn test_parse_page_range_dedup() {
        assert_eq!(parse_page_range("1-3,2,3").unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_page_range_zero() {
        assert!(parse_page_range("0-5").is_err());
    }

    #[test]
    fn test_parse_page_range_inverted() {
        assert!(parse_page_range("5-1").is_err());
    }

    #[test]
    fn test_parse_page_range_invalid() {
        assert!(parse_page_range("abc").is_err());
    }

    #[test]
    fn test_parse_page_range_whitespace() {
        assert_eq!(parse_page_range(" 1 - 3 , 5 ").unwrap(), vec![1, 2, 3, 5]);
    }
}