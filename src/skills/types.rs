//! Types for the skills system.
//!
//! Skills are Markdown files with YAML frontmatter that define AI behaviors.
//! They are loaded on-demand when the LLM requests them via skill_view().

use std::path::PathBuf;

/// Source of a skill file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillSource {
    /// Embedded in binary via include_str!
    Builtin,
    /// ~/.config/ask-ai/skills/<name>/SKILL.md - user-controlled
    User,
    /// .ask-ai/skills/<name>/SKILL.md - project-level, potentially shared
    Project,
}

impl std::fmt::Display for SkillSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillSource::Builtin => write!(f, "builtin"),
            SkillSource::User => write!(f, "user"),
            SkillSource::Project => write!(f, "project"),
        }
    }
}

/// Skill metadata for the INDEX section in system prompt.
///
/// This is what gets included in the prompt - just name and description.
/// The full content is loaded on-demand via skill_view().
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillIndex {
    /// Skill name (from frontmatter, defaults to directory name).
    /// Max 64 characters, alphanumeric + hyphen + underscore.
    pub name: String,
    /// Brief description for LLM to decide relevance.
    pub description: String,
    /// Where the skill was loaded from.
    pub source: SkillSource,
}

/// Full skill content returned by skill_view().
#[derive(Debug, Clone)]
pub struct Skill {
    /// Skill name.
    pub name: String,
    /// Brief description.
    pub description: String,
    /// Full Markdown content (after frontmatter).
    pub content: String,
    /// Source: builtin, user, or project.
    pub source: SkillSource,
    /// File path (None for builtin).
    #[allow(dead_code)]
    pub path: Option<PathBuf>,
}

/// YAML frontmatter parsed from SKILL.md files.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Frontmatter {
    /// Required: skill identifier (max 64 chars).
    #[serde(default)]
    pub name: Option<String>,
    /// Required: brief description for INDEX.
    #[serde(default)]
    pub description: Option<String>,
}

impl Frontmatter {
    /// Parse frontmatter from SKILL.md content.
    ///
    /// Format:
    /// ```markdown
    /// ---
    /// name: skill-name
    /// description: Brief description
    /// ---
    /// # Skill content
    /// ```
    pub fn parse(content: &str) -> Result<(Self, String), String> {
        // Find frontmatter delimiters
        let content = content.trim_start();

        if !content.starts_with("---") {
            return Err("SKILL.md must start with YAML frontmatter (---)".to_string());
        }

        // Find closing ---
        let content_after_first = &content[3..]; // Skip opening ---
        let end_marker = content_after_first.find("\n---");

        let (frontmatter_str, markdown_content) = match end_marker {
            Some(pos) => {
                let fm = &content_after_first[..pos];
                let md = &content_after_first[pos + 4..]; // Skip \n---
                (fm, md.trim())
            }
            None => {
                return Err("YAML frontmatter must end with ---".to_string());
            }
        };

        // Parse YAML
        let frontmatter: Self = serde_yaml::from_str(frontmatter_str).unwrap_or_else(|e| {
            // Try to salvage with default values
            eprintln!(
                "[SKILLS] Warning: Failed to parse frontmatter: {}, using defaults",
                e
            );
            Self::default()
        });

        Ok((frontmatter, markdown_content.to_string()))
    }
}

/// Error types for skill operations.
#[allow(dead_code)]
#[derive(Debug)]
pub enum SkillError {
    /// Skill file not found.
    NotFound(String),
    /// Invalid frontmatter.
    InvalidFrontmatter(String),
    /// File too large (max 256KB).
    FileTooLarge {
        path: PathBuf,
        size: usize,
        max: usize,
    },
    /// Binary content detected (null bytes).
    BinaryContent(PathBuf),
    /// Invalid skill name (not alphanumeric + hyphen + underscore).
    InvalidName(String),
    /// I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillError::NotFound(name) => {
                write!(
                    f,
                    "Skill '{}' not found. Use skill_list to see available skills.",
                    name
                )
            }
            SkillError::InvalidFrontmatter(msg) => {
                write!(f, "Invalid frontmatter: {}", msg)
            }
            SkillError::FileTooLarge { path, size, max } => {
                write!(
                    f,
                    "Skill file too large: {} ({} bytes, max {} bytes)",
                    path.display(),
                    size,
                    max
                )
            }
            SkillError::BinaryContent(path) => {
                write!(f, "Skill file contains binary content: {}", path.display())
            }
            SkillError::InvalidName(name) => {
                write!(f, "Invalid skill name: '{}'. Name must be alphanumeric with hyphens or underscores.", name)
            }
            SkillError::Io(e) => {
                write!(f, "I/O error: {}", e)
            }
        }
    }
}

impl std::error::Error for SkillError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SkillError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SkillError {
    fn from(e: std::io::Error) -> Self {
        SkillError::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_source_display() {
        assert_eq!(SkillSource::Builtin.to_string(), "builtin");
        assert_eq!(SkillSource::User.to_string(), "user");
        assert_eq!(SkillSource::Project.to_string(), "project");
    }

    #[test]
    fn test_frontmatter_parse_valid() {
        let content = r#"---
name: pdf-processing
description: Extract text from PDF files
---
# PDF Processing

Instructions here...
"#;
        let (fm, md) = Frontmatter::parse(content).unwrap();
        assert_eq!(fm.name, Some("pdf-processing".to_string()));
        assert_eq!(
            fm.description,
            Some("Extract text from PDF files".to_string())
        );
        assert!(md.contains("# PDF Processing"));
    }

    #[test]
    fn test_frontmatter_parse_missing_delimiter() {
        let content = "name: test\ndescription: test\n# Content";
        let result = Frontmatter::parse(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_frontmatter_parse_unclosed() {
        let content = "---\nname: test\n# Content";
        let result = Frontmatter::parse(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_frontmatter_parse_defaults() {
        let content = "---\n---\n# Content";
        let (fm, _) = Frontmatter::parse(content).unwrap();
        assert_eq!(fm.name, None);
        assert_eq!(fm.description, None);
    }

    #[test]
    fn test_skill_error_not_found() {
        let err = SkillError::NotFound("test-skill".to_string());
        assert!(err.to_string().contains("test-skill"));
        assert!(err.to_string().contains("skill_list"));
    }

    #[test]
    fn test_skill_error_invalid_name() {
        let err = SkillError::InvalidName("test skill!".to_string());
        assert!(err.to_string().contains("test skill!"));
        assert!(err.to_string().contains("alphanumeric"));
    }
}
