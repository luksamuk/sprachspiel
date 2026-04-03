//! Document types for the Content System
//!
//! Documents are imported files (TXT, MD, ORG, PDF, EPUB) that are
//! stored for semantic search and retrieval. Unlike notes (LLM-created),
//! documents are user-imported files that get chunked and embedded.
//!
//! # File Size Limit
//!
//! Maximum file size is 2.5 MB (2,500,000 bytes). Larger files are rejected
//! with a helpful error message.
//!
//! # Feature Dependencies
//!
//! - TXT/MD/ORG: Builtin support, no dependencies
//! - PDF/EPUB: Requires `skills-tools` feature (uses document-processing skill)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;

use super::types::{ContentScope, ContentSource};

/// Maximum document file size in bytes
/// Larger documents should be split before import to ensure proper chunking.
pub const MAX_DOCUMENT_SIZE: usize = 2_500_000; // 2.5 MB = 2,500,000 bytes

/// Supported document file types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    /// Plain text file
    Txt,
    /// Markdown file
    Md,
    /// Org-mode file
    Org,
    /// PDF document (requires skills-tools)
    Pdf,
    /// EPUB ebook (requires skills-tools)
    Epub,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Txt => write!(f, "txt"),
            FileType::Md => write!(f, "md"),
            FileType::Org => write!(f, "org"),
            FileType::Pdf => write!(f, "pdf"),
            FileType::Epub => write!(f, "epub"),
        }
    }
}

impl std::str::FromStr for FileType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "txt" => Ok(FileType::Txt),
            "md" | "markdown" => Ok(FileType::Md),
            "org" => Ok(FileType::Org),
            "pdf" => Ok(FileType::Pdf),
            "epub" => Ok(FileType::Epub),
            _ => Err(format!(
                "Unsupported file type: {}. Supported: txt, md, org, pdf, epub",
                s
            )),
        }
    }
}

impl FileType {
    /// Get file extension
    pub fn extension(&self) -> &'static str {
        match self {
            FileType::Txt => "txt",
            FileType::Md => "md",
            FileType::Org => "org",
            FileType::Pdf => "pdf",
            FileType::Epub => "epub",
        }
    }

    /// Check if this file type requires skills-tools feature
    /// Only compiled when skills-tools is NOT enabled
    #[cfg(not(feature = "skills-tools"))]
    pub fn requires_skills(&self) -> bool {
        matches!(self, FileType::Pdf | FileType::Epub)
    }
}

/// Detect file type from path extension
pub fn detect_file_type(path: &Path) -> Result<FileType, String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| format!("File has no extension: {}", path.display()))?;

    FileType::from_str(extension)
}

/// A document stored in the content system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier
    pub id: i64,
    /// Scope (project or global)
    pub scope: ContentScope,
    /// Source (always User for documents)
    pub source: ContentSource,
    /// Document title (extracted from filename or first heading)
    pub title: String,
    /// Original filename
    pub filename: String,
    /// File type
    pub file_type: FileType,
    /// Document content (full text)
    pub content: String,
    /// Word count
    pub word_count: usize,
    /// Importance (0.0 to 1.0, affects decay)
    pub importance: f32,
    /// Access count (incremented on retrieval)
    pub access_count: u32,
    /// Current decay score (0.0 to 1.0)
    pub decay_score: f32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
    /// Project ID (for project-scoped documents)
    pub project_id: Option<String>,
}

impl Document {
    /// Create a new document with default values
    pub fn new(
        content: String,
        title: String,
        filename: String,
        file_type: FileType,
        scope: ContentScope,
        project_id: Option<String>,
    ) -> Result<Self, String> {
        if content.is_empty() {
            return Err("Document content cannot be empty".to_string());
        }

        if content.len() > MAX_DOCUMENT_SIZE {
            let size_mb = content.len() as f64 / 1_000_000.0;
            let limit_mb = MAX_DOCUMENT_SIZE as f64 / 1_000_000.0;
            return Err(format!(
                "Document too large: {:.1} MB ({:.0} bytes) exceeds the {:.1} MB limit ({:.0} bytes).\n\
                 \n\
                 File: {}\n\
                 \n\
                 To import large documents, ask the user to split the file externally,\n\
                 or import a smaller file. The LLM cannot split files automatically.",
                size_mb,
                content.len(),
                limit_mb,
                MAX_DOCUMENT_SIZE,
                filename
            ));
        }

        if !content.is_char_boundary(content.len()) {
            return Err("Document content has invalid unicode".to_string());
        }

        let word_count = content.split_whitespace().count();
        let now = Utc::now();

        Ok(Document {
            id: 0,
            scope,
            source: ContentSource::User,
            title,
            filename,
            file_type,
            content,
            word_count,
            importance: 0.5,
            access_count: 0,
            decay_score: 1.0,
            created_at: now,
            updated_at: now,
            last_accessed: now,
            project_id,
        })
    }

    /// Extract title from content (first heading) or filename
    pub fn extract_title(content: &str, filename: &str) -> String {
        // Try to find first heading in content
        for line in content.lines().take(20) {
            let trimmed = line.trim();

            // Org-mode #+TITLE: directive (highest priority)
            if let Some(title) = trimmed.strip_prefix("#+TITLE:") {
                let title = title.trim();
                if !title.is_empty() {
                    return title.to_string();
                }
                // Empty #+TITLE: - skip to next line
                continue;
            }

            // Markdown heading
            if trimmed.starts_with('#') {
                // But skip org-mode directives (already handled above)
                // This handles case where # is at start but it's actually org content
                let title = trimmed.trim_start_matches('#').trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }

            // Org-mode heading
            if trimmed.starts_with('*') {
                let title = trimmed.trim_start_matches('*').trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }

            // Underlined heading (Markdown/Text)
            if !trimmed.is_empty() && line.len() > trimmed.len() {
                // Next line is underline (=== or ---)
                continue;
            }
        }

        // Fall back to filename without extension
        Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename)
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_extension() {
        assert_eq!(FileType::Txt.extension(), "txt");
        assert_eq!(FileType::Md.extension(), "md");
        assert_eq!(FileType::Org.extension(), "org");
        assert_eq!(FileType::Pdf.extension(), "pdf");
        assert_eq!(FileType::Epub.extension(), "epub");
    }

    #[test]
    fn test_detect_file_type() {
        use std::path::PathBuf;

        assert!(matches!(
            detect_file_type(&PathBuf::from("test.txt")),
            Ok(FileType::Txt)
        ));
        assert!(matches!(
            detect_file_type(&PathBuf::from("test.md")),
            Ok(FileType::Md)
        ));
        assert!(matches!(
            detect_file_type(&PathBuf::from("test.pdf")),
            Ok(FileType::Pdf)
        ));
        assert!(detect_file_type(&PathBuf::from("test")).is_err());
    }

    #[test]
    fn test_extract_title_from_markdown() {
        let content = "# My Document\n\nSome content here.";
        let title = Document::extract_title(content, "file.md");
        assert_eq!(title, "My Document");
    }

    #[test]
    fn test_extract_title_from_org() {
        let content = "* My Document\n\nSome content here.";
        let title = Document::extract_title(content, "file.org");
        assert_eq!(title, "My Document");
    }

    #[test]
    fn test_extract_title_fallback_to_filename() {
        let content = "No heading here\nJust plain text.";
        let title = Document::extract_title(content, "my_file.txt");
        assert_eq!(title, "my_file");
    }

    #[test]
    fn test_extract_title_org_directive() {
        let content = "#+TITLE: My Org Title\n\n* Heading\nSome content.";
        let title = Document::extract_title(content, "file.org");
        assert_eq!(title, "My Org Title");
    }

    #[test]
    fn test_extract_title_org_directive_priority() {
        // #+TITLE: should take priority over * heading
        let content = "#+TITLE: Title from Directive\n\n* Different Heading\nContent.";
        let title = Document::extract_title(content, "file.org");
        assert_eq!(title, "Title from Directive");
    }

    #[test]
    fn test_extract_title_org_empty_directive() {
        // Empty #+TITLE: should fall back to heading
        let content = "#+TITLE:\n\n* My Heading\nContent.";
        let title = Document::extract_title(content, "file.org");
        assert_eq!(title, "My Heading");
    }

    #[test]
    fn test_extract_title_org_whitespace_directive() {
        // Whitespace after #+TITLE: should be trimmed
        let content = "#+TITLE:   Spaced Title   \n\n* Heading\nContent.";
        let title = Document::extract_title(content, "file.org");
        assert_eq!(title, "Spaced Title");
    }

    #[test]
    fn test_document_new_validates_size() {
        let long_content = "x".repeat(MAX_DOCUMENT_SIZE + 1);
        let result = Document::new(
            long_content,
            "Title".to_string(),
            "test.txt".to_string(),
            FileType::Txt,
            ContentScope::Project,
            Some("test-project".to_string()),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("too large") || err.contains("exceeds"),
            "Error message should mention size limit"
        );
    }

    #[test]
    fn test_document_size_just_under_limit() {
        let content = "x".repeat(MAX_DOCUMENT_SIZE); // Exactly at limit
        let result = Document::new(
            content,
            "Title".to_string(),
            "test.txt".to_string(),
            FileType::Txt,
            ContentScope::Project,
            Some("test-project".to_string()),
        );
        assert!(
            result.is_ok(),
            "Document at exact size limit should be accepted"
        );
    }

    #[test]
    fn test_document_new_valid_content() {
        let content = "This is a valid document with some words.".to_string();
        let doc = Document::new(
            content.clone(),
            "Test Document".to_string(),
            "test.md".to_string(),
            FileType::Md,
            ContentScope::Project,
            Some("test-project".to_string()),
        )
        .expect("Valid document should succeed");

        assert_eq!(doc.title, "Test Document");
        assert_eq!(doc.filename, "test.md");
        assert_eq!(doc.file_type, FileType::Md);
        assert_eq!(doc.scope, ContentScope::Project);
        assert_eq!(doc.source, ContentSource::User);
        assert_eq!(doc.word_count, 8); // "This is a valid document with some words."
    }
}
