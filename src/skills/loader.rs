//! Skill loading and directory scanning.
//!
//! Loads skills from three sources with priority:
//! 1. Project skills: .ask-ai/skills/<name>/SKILL.md
//! 2. User skills: ~/.config/ask-ai/skills/<name>/SKILL.md
//! 3. Builtin skills: embedded in binary via include_str!

use std::collections::HashMap;
use std::path::PathBuf;

use super::sanitize::{is_valid_skill_name, sanitize_skill_content, validate_skill_file};
use super::types::{Frontmatter, Skill, SkillIndex, SkillSource};

/// Default skills directory name.
const SKILLS_DIR_NAME: &str = "skills";
/// Skill file name (must be SKILL.md in skill directory).
const SKILL_FILE_NAME: &str = "SKILL.md";

/// Builtin skills embedded in binary.
/// These are always trusted and don't require sanitization.
static BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("pdf-processing", include_str!("builtin/pdf-processing.md")),
    ("ocr-images", include_str!("builtin/ocr-images.md")),
    ("code-analysis", include_str!("builtin/code-analysis.md")),
    ("web-scraping", include_str!("builtin/web-scraping.md")),
];

/// Load all skill indexes for system prompt.
///
/// Returns minimal metadata (name, description) for each skill.
/// Deduplication: project > user > builtin (project takes precedence).
pub fn load_skill_indexes() -> Vec<SkillIndex> {
    let mut skills: HashMap<String, SkillIndex> = HashMap::new();

    // Load builtin skills first (lowest priority)
    for (name, content) in BUILTIN_SKILLS {
        if let Some(index) = parse_builtin_skill_index(name, content) {
            skills.insert(name.to_string(), index);
        }
    }

    // Load user skills (medium priority)
    if let Some(user_dir) = get_user_skills_dir() {
        load_skills_indexes_from_dir(&user_dir, SkillSource::User, &mut skills);
    }

    // Load project skills (highest priority)
    if let Some(project_dir) = get_project_skills_dir() {
        load_skills_indexes_from_dir(&project_dir, SkillSource::Project, &mut skills);
    }

    // Sort by name for consistent ordering
    let mut indexes: Vec<_> = skills.into_values().collect();
    indexes.sort_by(|a, b| a.name.cmp(&b.name));

    indexes
}

/// Load full skill content by name.
///
/// Returns None if skill not found or validation fails.
/// Sanitizes content for user/project skills.
pub fn get_skill_content(name: &str) -> Option<Skill> {
    // Check name validity first
    if !is_valid_skill_name(name) {
        eprintln!("[SKILLS] Invalid skill name: {}", name);
        return None;
    }

    // Try project skills first (highest priority)
    if let Some(project_dir) = get_project_skills_dir() {
        if let Some(skill) = load_skill_from_dir(&project_dir, name, SkillSource::Project) {
            return Some(skill);
        }
    }

    // Try user skills (medium priority)
    if let Some(user_dir) = get_user_skills_dir() {
        if let Some(skill) = load_skill_from_dir(&user_dir, name, SkillSource::User) {
            return Some(skill);
        }
    }

    // Try builtin skills (lowest priority)
    for (builtin_name, content) in BUILTIN_SKILLS {
        if builtin_name == &name {
            return parse_builtin_skill(builtin_name, content);
        }
    }

    None
}

/// Get list of available skill names.
pub fn get_available_skill_names() -> Vec<String> {
    load_skill_indexes().into_iter().map(|i| i.name).collect()
}

// ============================================================================
// Private Functions
// ============================================================================

/// Get user skills directory (~/.config/ask-ai/skills/).
fn get_user_skills_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("ask-ai").join(SKILLS_DIR_NAME))
}

/// Get project skills directory (./.ask-ai/skills/).
fn get_project_skills_dir() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|p| p.join(".ask-ai").join(SKILLS_DIR_NAME))
}

/// Load skill indexes from a directory.
fn load_skills_indexes_from_dir(
    dir: &PathBuf,
    source: SkillSource,
    skills: &mut HashMap<String, SkillIndex>,
) {
    if !dir.exists() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Directory name becomes the skill name
            let skill_dir = path;
            let skill_file = skill_dir.join(SKILL_FILE_NAME);

            if skill_file.exists() {
                // Use directory name as skill name
                if let Some(name) = skill_dir.file_name().and_then(|n| n.to_str()) {
                    if is_valid_skill_name(name) {
                        // Try to read just the frontmatter for index
                        if let Ok(content) = std::fs::read_to_string(&skill_file) {
                            if validate_skill_file(&skill_file, &content).is_ok() {
                                if let Some(index) =
                                    parse_skill_index(&skill_file, name, &content, source)
                                {
                                    skills.insert(name.to_string(), index);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Load a single skill from a directory.
fn load_skill_from_dir(skills_dir: &PathBuf, name: &str, source: SkillSource) -> Option<Skill> {
    let skill_dir = skills_dir.join(name);
    let skill_file = skill_dir.join(SKILL_FILE_NAME);

    if !skill_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&skill_file).ok()?;

    // Validate
    validate_skill_file(&skill_file, &content).ok()?;

    // Parse frontmatter
    let (frontmatter, markdown_content) = Frontmatter::parse(&content).ok()?;

    // Sanitize content (user/project skills)
    let sanitized = sanitize_skill_content(&markdown_content, &format!("{}:{}", source, name))?;

    // Use directory name as skill name (frontmatter name is ignored for user/project)
    let skill_name = name.to_string();
    let description = frontmatter.description.unwrap_or_else(|| {
        // Use first line of content as description if not in frontmatter
        sanitized
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim()
            .to_string()
    });

    Some(Skill {
        name: skill_name,
        description,
        content: sanitized,
        source,
        path: Some(skill_file),
    })
}

/// Parse builtin skill index from embedded content.
fn parse_builtin_skill_index(name: &str, content: &str) -> Option<SkillIndex> {
    let (frontmatter, _) = Frontmatter::parse(content).ok()?;

    let skill_name = name.to_string();
    let description = frontmatter.description.unwrap_or_else(|| {
        // Use first line of content as description
        content
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim()
            .to_string()
    });

    Some(SkillIndex {
        name: skill_name,
        description,
        source: SkillSource::Builtin,
    })
}

/// Parse builtin skill from embedded content.
fn parse_builtin_skill(name: &str, content: &str) -> Option<Skill> {
    let (frontmatter, _) = Frontmatter::parse(content).ok()?;

    let skill_name = name.to_string();
    let description = frontmatter.description.unwrap_or_else(|| {
        content
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim()
            .to_string()
    });

    // Builtin skills are trusted, no sanitization needed
    // But we still strip invisible unicode for consistency
    let content = super::sanitize::remove_invisible_unicode(content);

    Some(Skill {
        name: skill_name,
        description,
        content,
        source: SkillSource::Builtin,
        path: None,
    })
}

/// Parse skill index from file content (for user/project skills).
fn parse_skill_index(
    _path: &PathBuf,
    default_name: &str,
    content: &str,
    source: SkillSource,
) -> Option<SkillIndex> {
    let (frontmatter, markdown_content) = Frontmatter::parse(content).ok()?;

    let name = default_name.to_string();
    let description = frontmatter.description.unwrap_or_else(|| {
        // Use first line after frontmatter as description
        markdown_content
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim()
            .to_string()
    });

    // Log warning if frontmatter name differs from directory name
    if let Some(frontmatter_name) = &frontmatter.name {
        if frontmatter_name != default_name {
            if crate::debug_tools::is_debug_enabled() {
                eprintln!("[SKILLS] Warning: {} skill directory name '{}' differs from frontmatter name '{}'", 
                    source, default_name, frontmatter_name);
            }
        }
    }

    Some(SkillIndex {
        name,
        description,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_count() {
        // We should have 4 builtin skills
        assert_eq!(BUILTIN_SKILLS.len(), 4);
    }

    #[test]
    fn test_load_skill_indexes_contains_builtins() {
        let indexes = load_skill_indexes();

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
        assert!(!skill.description.is_empty());
    }

    #[test]
    fn test_get_skill_content_nonexistent() {
        let skill = get_skill_content("nonexistent-skill");
        assert!(skill.is_none());
    }

    #[test]
    fn test_get_available_skill_names() {
        let names = get_available_skill_names();
        assert!(names.contains(&"pdf-processing".to_string()));
        assert!(names.contains(&"ocr-images".to_string()));
    }

    #[test]
    fn test_is_valid_skill_name() {
        assert!(is_valid_skill_name("pdf-processing"));
        assert!(is_valid_skill_name("ocr_images"));
        assert!(is_valid_skill_name("codeAnalysis"));

        // Invalid
        assert!(!is_valid_skill_name(""));
        assert!(!is_valid_skill_name("-invalid"));
        assert!(!is_valid_skill_name("skill name"));
        assert!(!is_valid_skill_name("skill!"));
    }

    #[test]
    fn test_builtin_skills_have_valid_content() {
        for (name, content) in BUILTIN_SKILLS {
            // Each builtin skill should parse successfully
            let result = Frontmatter::parse(content);
            assert!(result.is_ok(), "Failed to parse builtin skill: {}", name);

            // Content should not be empty
            let (_, markdown) = result.unwrap();
            assert!(
                !markdown.trim().is_empty(),
                "Empty content for builtin skill: {}",
                name
            );
        }
    }
}
