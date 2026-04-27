//! Types for the Factual Memory System
//!
//! Defines the core types: Category, Scope, Source, and Fact.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum content length per fact (characters)
pub const MAX_FACT_CONTENT_SIZE: usize = 500;

/// Maximum total facts characters in prompt
pub const MAX_TOTAL_FACTS_CHARS: usize = 2200;

/// Fact categories with different decay half-lives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// User preferences, likes/dislikes (180 days half-life)
    Preference,
    /// Objective facts about environment/project (30 days half-life)
    Fact,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Preference => write!(f, "preference"),
            Category::Fact => write!(f, "fact"),
        }
    }
}

impl std::str::FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "preference" => Ok(Category::Preference),
            "fact" => Ok(Category::Fact),
            _ => Err(format!("Invalid category: {}", s)),
        }
    }
}

/// Fact scope (visibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Facts specific to current project
    Project,
    /// Facts that apply to all projects
    Global,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scope::Project => write!(f, "project"),
            Scope::Global => write!(f, "global"),
        }
    }
}

impl std::str::FromStr for Scope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "project" => Ok(Scope::Project),
            "global" => Ok(Scope::Global),
            _ => Err(format!("Invalid scope: {}", s)),
        }
    }
}

/// Fact source (who added it)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Added by user via /fact command
    User,
    /// Added autonomously by LLM
    Llm,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::User => write!(f, "user"),
            Source::Llm => write!(f, "llm"),
        }
    }
}

impl std::str::FromStr for Source {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Source::User),
            "llm" => Ok(Source::Llm),
            _ => Err(format!("Invalid source: {}", s)),
        }
    }
}

/// A fact stored in the memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    /// Unique identifier
    pub id: i64,
    /// Scope (project or global)
    pub scope: Scope,
    /// Category (preference or fact)
    pub category: Category,
    /// Fact content (max 500 chars)
    pub content: String,
    /// Importance (0.0 to 1.0, affects decay)
    pub importance: f32,
    /// Access count (incremented on retrieval)
    pub access_count: u32,
    /// Current decay score (0.0 to 1.0)
    pub decay_score: f32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
    /// Who added this fact
    pub source: Source,
    /// When this fact was invalidated (soft delete)
    pub invalidated_at: Option<DateTime<Utc>>,
    /// Project ID (for project-scoped facts)
    pub project_id: Option<String>,
    /// Whether this fact has a vector embedding in fact_embeddings
    pub has_embedding: bool,
}

impl Fact {
    /// Create a new fact with default values
    pub fn new(
        content: String,
        category: Category,
        scope: Scope,
        project_id: Option<String>,
        source: Source,
    ) -> Result<Self, String> {
        // Validate content length
        if content.len() > MAX_FACT_CONTENT_SIZE {
            return Err(format!(
                "Fact content exceeds {} characters (got {})",
                MAX_FACT_CONTENT_SIZE,
                content.len()
            ));
        }

        // Validate UTF-8 boundary
        if !content.is_char_boundary(content.len()) {
            return Err("Fact content has invalid unicode".to_string());
        }

        // Global facts must not have a project_id
        let project_id = if scope == Scope::Global {
            None
        } else {
            project_id
        };

        let now = Utc::now();

        Ok(Fact {
            id: 0, // Will be set by database
            scope,
            category,
            content,
            importance: 0.5,
            access_count: 0,
            decay_score: 1.0,
            created_at: now,
            last_accessed: now,
            source,
            invalidated_at: None,
            project_id,
            has_embedding: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_display() {
        assert_eq!(Category::Preference.to_string(), "preference");
        assert_eq!(Category::Fact.to_string(), "fact");
    }

    #[test]
    fn test_category_from_str() {
        use std::str::FromStr;
        assert!(matches!(
            Category::from_str("preference"),
            Ok(Category::Preference)
        ));
        assert!(matches!(Category::from_str("fact"), Ok(Category::Fact)));
        assert!(Category::from_str("invalid").is_err());
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(Scope::Project.to_string(), "project");
        assert_eq!(Scope::Global.to_string(), "global");
    }

    #[test]
    fn test_scope_from_str() {
        use std::str::FromStr;
        assert!(matches!(Scope::from_str("project"), Ok(Scope::Project)));
        assert!(matches!(Scope::from_str("global"), Ok(Scope::Global)));
        assert!(Scope::from_str("invalid").is_err());
    }

    #[test]
    fn test_source_display() {
        assert_eq!(Source::User.to_string(), "user");
        assert_eq!(Source::Llm.to_string(), "llm");
    }

    #[test]
    fn test_source_from_str() {
        use std::str::FromStr;
        assert!(matches!(Source::from_str("user"), Ok(Source::User)));
        assert!(matches!(Source::from_str("llm"), Ok(Source::Llm)));
        assert!(Source::from_str("invalid").is_err());
    }

    #[test]
    fn test_fact_new_validates_length() {
        let long_content = "x".repeat(501);
        let result = Fact::new(
            long_content,
            Category::Fact,
            Scope::Project,
            None,
            Source::User,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds"));
    }

    #[test]
    fn test_fact_new_validates_unicode() {
        // Content ending in middle of multibyte character
        let mut content = "Hello 世界".to_string();
        // Truncate to byte position that's not a char boundary
        content.push_str("🌍");
        let result = Fact::new(content, Category::Fact, Scope::Project, None, Source::User);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fact_global_forces_project_id_none() {
        // Global facts must always have project_id = None
        let fact = Fact::new(
            "Test fact".to_string(),
            Category::Fact,
            Scope::Global,
            Some("my-project".to_string()), // Should be ignored
            Source::User,
        )
        .unwrap();
        assert_eq!(fact.scope, Scope::Global);
        assert_eq!(
            fact.project_id, None,
            "Global facts must have project_id = None"
        );
    }

    #[test]
    fn test_fact_project_keeps_project_id() {
        // Project facts should keep their project_id
        let fact = Fact::new(
            "Test fact".to_string(),
            Category::Fact,
            Scope::Project,
            Some("my-project".to_string()),
            Source::User,
        )
        .unwrap();
        assert_eq!(fact.scope, Scope::Project);
        assert_eq!(fact.project_id, Some("my-project".to_string()));
    }
}
