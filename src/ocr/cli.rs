//! OCR subcommand CLI
//!
//! Defines the command-line interface for the OCR subcommand
//! which extracts text from images using an OCR model.

use clap::Args;
use std::path::PathBuf;

use super::mode::OcrMode;

/// Long help for `sprach ocr`, rendered by [`crate::consts::app::help_text`].
///
/// Lives here rather than in a `#[command(long_about = ...)]` on [`OcrArgs`]
/// because a doc-comment on a `Subcommand` variant takes precedence over the
/// struct's help attributes — clap would compile this text in and never print
/// it (LUC-142). The `Commands::Ocr` variant references this constant.
pub const OCR_LONG_ABOUT: &str = "Extract text, tables, figures, or formulas from images.

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
  {app} ocr document.png                 # Extract text from image
  {app} ocr --mode table sheet.png       # Extract table structure
  {app} ocr --mode formula eq.png        # Extract LaTeX formulas
  {app} ocr --json *.png > output.jsonl  # Batch process with JSON output
  {app} ocr page*.png > combined.txt     # Process multiple images

PIPELINES:
  {app} ocr japanese.png | {app} translate ja:pt    # OCR + translate
  {app} ocr report.png | {app} summarize            # OCR + summarize

REQUIREMENTS:
  - The LLM server must be running locally or accessible remotely
  - An OCR model must be available: ollama pull glm-ocr:bf16";

/// Arguments for the OCR subcommand
#[derive(Args, Debug, Clone)]
#[command(about = "Extract text from images")]
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
}

impl OcrArgs {
    /// Validate that files are provided.
    ///
    /// This message is printed to stderr and **is** reachable — `sprach ocr`
    /// with no arguments exits through this path. It used to say `Usage: ask ocr`,
    /// naming the binary from before the rename; that reached users because it
    /// is a runtime error string, not help text (LUC-142).
    pub fn validate(&self) -> Result<(), String> {
        if self.files.is_empty() {
            return Err(format!(
                "No image files provided.\nUsage: {app} ocr [OPTIONS] <FILE>...\n\
                 Try '{app} ocr --help' for more information.",
                app = crate::consts::app::APP_NAME
            ));
        }
        Ok(())
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
        };
        assert!(args.validate().is_ok());

        let args_empty = OcrArgs {
            files: vec![],
            mode: OcrMode::Text,
            json: false,
            max_tokens: 8192,
        };
        assert!(args_empty.validate().is_err());
    }

    /// The no-arguments error is user-visible (`sprach ocr` prints it to stderr),
    /// so it must name the real binary. It previously said `ask ocr`.
    #[test]
    fn validation_error_names_the_real_binary() {
        let args = OcrArgs {
            files: vec![],
            mode: OcrMode::Text,
            json: false,
            max_tokens: 8192,
        };
        let err = args.validate().expect_err("empty files must be rejected");
        assert!(
            err.contains("sprach ocr"),
            "error must name the real binary: {err}"
        );
        assert!(
            !err.contains("ask ocr"),
            "error must not name the pre-rename binary: {err}"
        );
    }

    /// `long_about` is only printed if the `Commands` variant references it, and
    /// the text must render `{app}` — a literal `{app}` leaking into `--help`
    /// would be worse than the stale name it replaced.
    #[test]
    fn long_about_renders_app_name() {
        let rendered = crate::consts::app::help_text(OCR_LONG_ABOUT);
        assert!(
            rendered.contains("sprach ocr"),
            "rendered help must name the binary"
        );
        assert!(
            !rendered.contains("{app}"),
            "placeholder must be substituted: {rendered}"
        );
    }
}
