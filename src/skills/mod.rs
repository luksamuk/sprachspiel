//! Skills System for ask-ai.
//!
//! Skills are Markdown files with YAML frontmatter that define AI behaviors.
//! They are loaded on-demand when the LLM requests them via the skill_view() tool.
//!
//! # Architecture
//!
//! ```text
//! System Prompt
//! ├── SKILLS INDEX (names + descriptions)
//! │   └── <available_skills> section
//! └── Tools section
//!
//! On-demand Loading:
//! ├── LLM sees relevant skill in INDEX
//! ├── LLM calls skill_view(name="pdf-processing")
//! └── System returns full SKILL.md content
//! ```
//!
//! # Skill Sources (priority)
//!
//! 1. **Project**: `.ask-ai/skills/<name>/SKILL.md` (highest priority)
//! 2. **User**: `~/.config/ask-ai/skills/<name>/SKILL.md`
//! 3. **Builtin**: Embedded in binary via `include_str!` (lowest priority)
//!
//! # Example
//!
//! ```markdown
//! ---
//! name: pdf-processing
//! description: Extract text from PDF files using external tools
//! ---
//!
//! # PDF Processing
//!
//! When asked to process PDF files:
//! 1. Check tool availability with check_tool_availability
//! 2. Use run_command("pdftotext", ...) for text extraction
//! ```

mod loader;
mod sanitize;
mod types;

pub use loader::{get_available_skill_names, get_skill_content, load_skill_indexes};
pub use sanitize::{
    is_valid_skill_name, sanitize_skill_content, validate_skill_file, MAX_SKILL_NAME_LENGTH,
    MAX_SKILL_SIZE,
};
pub use types::{Frontmatter, Skill, SkillError, SkillIndex, SkillSource};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_skill_indexes_returns_builtins() {
        let indexes = load_skill_indexes();
        assert!(!indexes.is_empty());

        // Builtin skills should always be present
        let names: Vec<&str> = indexes.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"pdf-processing"));
        assert!(names.contains(&"ocr-images"));
        assert!(names.contains(&"code-analysis"));
        assert!(names.contains(&"web-scraping"));
    }

    #[test]
    fn test_get_skill_content_builtin() {
        let skill = get_skill_content("pdf-processing");
        assert!(skill.is_some());

        let skill = skill.unwrap();
        assert_eq!(skill.name, "pdf-processing");
        assert_eq!(skill.source, SkillSource::Builtin);
        assert!(skill.path.is_none());
    }

    #[test]
    fn test_get_available_skill_names() {
        let names = get_available_skill_names();
        assert!(names.len() >= 4); // At least 4 builtin skills
    }
}
