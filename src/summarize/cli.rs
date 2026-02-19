//! Summarize subcommand CLI
//!
//! Defines the command-line interface for the summarize subcommand
//! which creates concise summaries using AI with tools disabled.

use clap::{Args, ValueEnum};

/// Arguments for the summarize subcommand
#[derive(Args, Debug, Clone)]
#[command(
    about = "Summarize text using AI",
    long_about = r#"Create concise summaries of provided text while preserving key information.

This subcommand uses a specialized summarization prompt and automatically
disables tool usage for security and efficiency.

MODEL:
  Uses mistral-small by default for optimal summarization quality.

EXAMPLES:
  ask summarize "Long text here..."
  echo "Long text" | ask summarize
  ask ocr document.png | ask summarize
  cat article.txt | ask summarize

  # With length limit
  ask summarize --max-length 200 "Very long text..."

  # Bullet points only
  ask summarize --format bullets "Text..."

  # Technical content
  ask summarize --style technical documentation.txt
"#
)]
pub struct SummarizeArgs {
    /// Text to summarize (optional, reads from stdin if not provided)
    #[arg(value_name = "TEXT")]
    pub text: Option<String>,

    /// Maximum length of summary in words (approximate)
    #[arg(short, long, default_value = "300")]
    pub max_length: u32,

    /// Output format: paragraph, bullets, or both
    #[arg(short, long, value_enum, default_value = "both")]
    pub format: SummaryFormat,

    /// Focus area: general, technical, academic, business
    #[arg(long, value_enum, default_value = "general")]
    pub style: SummaryStyle,
}

/// Output format for summaries
#[derive(ValueEnum, Clone, Debug, Copy, PartialEq, Eq, Default)]
pub enum SummaryFormat {
    /// Single paragraph summary
    Paragraph,
    /// Bullet points only
    Bullets,
    /// Both paragraph and bullets (default)
    #[default]
    Both,
}

impl SummaryFormat {
    /// Get format instruction for prompt
    pub fn into_instruction(self) -> &'static str {
        match self {
            SummaryFormat::Paragraph => {
                "Provide a concise paragraph summary that captures the main points."
            }
            SummaryFormat::Bullets => {
                "Provide a list of bullet points covering the key information."
            }
            SummaryFormat::Both => {
                "Provide both a concise paragraph summary followed by bullet points of key takeaways."
            }
        }
    }
}

/// Summary style/focus
#[derive(ValueEnum, Clone, Debug, Copy, PartialEq, Eq, Default)]
pub enum SummaryStyle {
    /// General purpose summarization
    #[default]
    General,
    /// Technical documentation style
    Technical,
    /// Academic paper style
    Academic,
    /// Business report style
    Business,
}

impl SummaryStyle {
    /// Get style instruction for prompt
    pub fn into_instruction(self) -> &'static str {
        match self {
            SummaryStyle::General => "Create a general summary suitable for any audience.",
            SummaryStyle::Technical => {
                "Create a technical summary preserving key technical details, terminology, and concepts. Focus on implementation details, APIs, and code structure where relevant."
            }
            SummaryStyle::Academic => {
                "Create an academic summary highlighting methodology, findings, and conclusions. Preserve citations and theoretical frameworks."
            }
            SummaryStyle::Business => {
                "Create a business-focused summary emphasizing actionable insights, key metrics, strategic implications, and recommendations."
            }
        }
    }
}

impl SummarizeArgs {
    /// Get text from args or stdin
    #[allow(dead_code)]
    pub fn get_text(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // If text provided as argument, use it
        if let Some(ref text) = self.text {
            return Ok(text.trim().to_string());
        }

        // Read from stdin
        use std::io::{self, Read};
        let mut input = String::new();
        match io::stdin().read_to_string(&mut input) {
            Ok(_) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    Err("Input from stdin is empty.".into())
                } else {
                    Ok(trimmed.to_string())
                }
            }
            Err(e) => Err(format!("Failed to read from stdin: {}", e).into()),
        }
    }

    /// Validate that text is provided
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), String> {
        match self.get_text() {
            Ok(text) if !text.is_empty() => Ok(()),
            Ok(_) => Err("No text provided for summarization.\n\
                Usage: ask summarize [OPTIONS] <TEXT>\n\
                   or: echo \"text\" | ask summarize\n\
                Try 'ask summarize --help' for more information."
                .to_string()),
            Err(e) => Err(format!("Failed to read input: {}", e)),
        }
    }

    /// Build the complete prompt based on format and style
    pub fn build_prompt(&self, base_prompt: &str) -> String {
        let length_instruction = format!(
            "Create a summary of approximately {} words.",
            self.max_length
        );

        let format_instruction = self.format.into_instruction();
        let style_instruction = self.style.into_instruction();

        format!(
            "{}\n\n{}\n{}\n{}",
            base_prompt, length_instruction, format_instruction, style_instruction
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_format_instructions() {
        assert!(
            SummaryFormat::Paragraph
                .into_instruction()
                .contains("paragraph")
        );
        assert!(
            SummaryFormat::Bullets
                .into_instruction()
                .contains("bullet points")
        );
        assert!(SummaryFormat::Both.into_instruction().contains("paragraph"));
        assert!(
            SummaryFormat::Both
                .into_instruction()
                .contains("bullet points")
        );
    }

    #[test]
    fn test_summary_style_instructions() {
        assert!(SummaryStyle::General.into_instruction().contains("general"));
        assert!(
            SummaryStyle::Technical
                .into_instruction()
                .contains("technical")
        );
        assert!(
            SummaryStyle::Academic
                .into_instruction()
                .contains("academic")
        );
        assert!(
            SummaryStyle::Business
                .into_instruction()
                .contains("business")
        );
    }

    #[test]
    fn test_build_prompt() {
        let args = SummarizeArgs {
            text: None,
            max_length: 300,
            format: SummaryFormat::Both,
            style: SummaryStyle::General,
        };

        let base_prompt = "You are a summarizer.";
        let prompt = args.build_prompt(base_prompt);

        assert!(prompt.contains(base_prompt));
        assert!(prompt.contains("300 words"));
        assert!(prompt.contains("paragraph"));
        assert!(prompt.contains("general"));
    }
}
