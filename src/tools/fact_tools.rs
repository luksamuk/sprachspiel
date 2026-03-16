//! Fact tools for the LLM to store and retrieve user/project information
//!
//! Provides tools for the LLM to autonomously store facts about the user
//! and project, enabling personalization across sessions.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::facts::classify::classify_fact;
use crate::facts::conflict::{detect_conflicts, resolve_conflict, CONFLICT_THRESHOLD};
use crate::facts::types::{Category, Fact, Scope, Source, MAX_FACT_CONTENT_SIZE};
use crate::project::get_project_id;
use crate::tools::context::get_db;
use crate::utils::parse_bounded_number;
use ollama_rs::function;

/// Parse fact ID from various formats ("42" or "fact:42")
fn parse_fact_id(id: &str) -> Result<i64, String> {
    let id_str = id.trim();
    
    // Handle "fact:N" format
    let numeric_str = if id_str.starts_with("fact:") {
        id_str.strip_prefix("fact:").unwrap_or(id_str)
    } else {
        id_str
    };
    
    numeric_str
        .parse::<i64>()
        .map_err(|_| format!("Invalid fact ID: '{}'. Use format 'fact:N' or just 'N'.", id))
}

/// Store a fact or preference about the user or project.
///
/// The system automatically classifies facts as "preference" (user likes/dislikes)
/// or "fact" (objective information). You can override this with the category parameter.
///
/// Facts are stored globally by default. Use scope="project" for project-specific information.
///
/// # Arguments
/// * `content` - The fact to store (max 500 characters). Required.
///   - Preferences: "I prefer short responses", "I dislike verbose explanations"
///   - Facts: "Database uses PostgreSQL", "API endpoint is /api/v1/users"
/// * `category` - Override automatic classification: "preference" or "fact". Optional.
///   - If not specified, the system detects based on patterns like "I prefer", "I like"
/// * `scope` - Where this fact applies: "global" or "project". Default: "global".
///   - Use "project" for facts specific to the current project
///   - Use "global" for user preferences and general information
///
/// # Returns
/// Confirmation message with the fact ID and detected/specified category.
///
/// # Examples
/// ```ignore
/// // Store a global preference (auto-detected)
/// fact_add(content="I prefer concise responses")
///
/// // Store a project-specific fact
/// fact_add(content="Project uses SQLite for storage", scope="project")
///
/// // Override category detection
/// fact_add(content="User prefers Portuguese", category="fact")
/// ```
#[function]
pub async fn fact_add(
    content: String,
    category: Option<String>,
    scope: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "fact_add",
        &[
            ("content".to_string(), content.clone()),
            ("category".to_string(), category.clone().unwrap_or_else(|| "auto".to_string())),
            ("scope".to_string(), scope.clone().unwrap_or_else(|| "global".to_string())),
        ],
    );

    // Validate content length
    if content.is_empty() {
        let err = "Error: Fact content is empty. Provide a non-empty fact.";
        log_tool_result("fact_add", err);
        return Ok(err.to_string());
    }

    if content.len() > MAX_FACT_CONTENT_SIZE {
        let err = format!(
            "Error: Fact content exceeds {} characters (got {} characters). \
             Please shorten your fact or split it into multiple facts.",
            MAX_FACT_CONTENT_SIZE,
            content.len()
        );
        log_tool_result("fact_add", &err);
        return Ok(err);
    }

    // Get database
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Fact storage not available.\n\n\
                        This can happen if:\n\
                        1. You're in an anonymous session (--anonymous flag)\n\
                        2. The database is not initialized\n\n\
                        Start a regular chat session to store facts.";
            log_tool_result("fact_add", err);
            return Ok(err.to_string());
        }
    };

    // Parse category (auto-detect if not specified)
    let parsed_category = match category.as_deref() {
        Some("preference") => Category::Preference,
        Some("fact") => Category::Fact,
        Some(c) => {
            let err = format!(
                "Error: Invalid category '{}'. Use 'preference' or 'fact'.",
                c
            );
            log_tool_result("fact_add", &err);
            return Ok(err);
        }
        None => classify_fact(&content),
    };

    // Parse scope (default: global)
    let (parsed_scope, project_id) = match scope.as_deref() {
        Some("project") | Some("local") => {
            let pid = get_project_id();
            if pid.is_none() {
                let err = "Error: Project scope requires being in a project directory.\n\n\
                            Either:\n\
                            1. Use scope='global' for user-level facts\n\
                            2. Run ask-ai from a project directory (git repo or named folder)";
                log_tool_result("fact_add", err);
                return Ok(err.to_string());
            }
            (Scope::Project, pid)
        }
        Some("global") | None => (Scope::Global, None),
        Some(s) => {
            let err = format!(
                "Error: Invalid scope '{}'. Use 'global' or 'project'.",
                s
            );
            log_tool_result("fact_add", &err);
            return Ok(err);
        }
    };

    // Create fact
    let fact = match Fact::for_insert(
        content.clone(),
        parsed_category,
        parsed_scope,
        project_id,
        Source::Llm,
    ) {
        Ok(f) => f,
        Err(e) => {
            let err = format!("Error creating fact: {}", e);
            log_tool_result("fact_add", &err);
            return Ok(err);
        }
    };

    // Check for conflicts (Phase 0.7)
    // Search for similar facts
    let conflicts = match db.search_facts(&content, parsed_scope.into(), 5) {
        Ok(results) => detect_conflicts(&content, &results, CONFLICT_THRESHOLD),
        Err(_) => {
            // If search fails, continue without conflict check
            Vec::new()
        }
    };

    // Handle conflicts
    if !conflicts.is_empty() {
        let conflict = &conflicts[0]; // Take the most similar conflict

        match resolve_conflict(conflict.clone()) {
            crate::facts::conflict::ResolutionAction::Skip => {
                // Duplicate - skip adding
                let result = format!(
                    "Skipped: Similar fact already exists (fact:{}).\n\n\
                     Existing: {}\n\
                     New: {}\n\n\
                     Use fact_remove(id=\"{}\") first if you want to replace it.",
                    conflict.existing_fact.id,
                    conflict.existing_fact.content,
                    content,
                    conflict.existing_fact.id
                );
                log_tool_result("fact_add", &result);
                return Ok(result);
            }
            crate::facts::conflict::ResolutionAction::Update => {
                // Contradiction - update existing fact
                // Delete old fact first
                if let Err(e) = db.delete_fact(conflict.existing_fact.id) {
                    let err = format!("Error resolving conflict: {}", e);
                    log_tool_result("fact_add", &err);
                    return Ok(err);
                }

                // Insert new fact
                let id = match db.insert_fact(&fact) {
                    Ok(id) => id,
                    Err(e) => {
                        let err = format!("Error storing fact: {}", e);
                        log_tool_result("fact_add", &err);
                        return Ok(err);
                    }
                };

                let scope_label = match parsed_scope {
                    Scope::Global => "global",
                    Scope::Project => "project",
                };
                let category_label = match parsed_category {
                    Category::Preference => "preference",
                    Category::Fact => "fact",
                };

                let result = format!(
                    "Updated fact:{} (category: {}, scope: {})\n\
                     Replaced conflicting fact with: {}\n\
                     Previously: {}",
                    id, category_label, scope_label, content, conflict.existing_fact.content
                );
                log_tool_result("fact_add", &result);
                return Ok(result);
            }
            crate::facts::conflict::ResolutionAction::Add => {
                // No conflict - continue with insert below
            }
        }
    }

    // Insert into database (no conflicts)
    let id = match db.insert_fact(&fact) {
        Ok(id) => id,
        Err(e) => {
            let err = format!("Error storing fact: {}", e);
            log_tool_result("fact_add", &err);
            return Ok(err);
        }
    };

    let scope_label = match parsed_scope {
        Scope::Global => "global",
        Scope::Project => "project",
    };
    let category_label = match parsed_category {
        Category::Preference => "preference",
        Category::Fact => "fact",
    };

    let result = format!(
        "Stored fact:{} (category: {}, scope: {})\n\n\
         Content: {}",
        id, category_label, scope_label, content
    );
    log_tool_result("fact_add", &result);
    Ok(result)
}

/// Search stored facts using keyword search.
///
/// Uses full-text search (FTS5) to find facts matching the query.
/// Results are ranked by relevance score.
///
/// # Arguments
/// * `query` - Search query (keywords to find). Required.
///   - Example: "database", "prefer", "API endpoint"
/// * `category` - Filter by category: "preference" or "fact". Optional.
///   - Use to find only preferences or only facts
/// * `scope` - Filter by scope: "global" or "project". Optional.
///   - Use to find project-specific or global facts
/// * `limit` - Maximum results to return (default: 5, max: 20). Optional.
///
/// # Returns
/// List of matching facts with IDs, categories, scopes, and relevance scores.
/// Each result shows the fact content and metadata.
///
/// # Examples
/// ```ignore
/// // Search all facts containing "database"
/// fact_search(query="database")
///
/// // Search only preferences
/// fact_search(query="response", category="preference")
///
/// // Search project-specific facts
/// fact_search(query="API", scope="project", limit="10")
/// ```
#[function]
pub async fn fact_search(
    query: String,
    category: Option<String>,
    scope: Option<String>,
    limit: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "fact_search",
        &[
            ("query".to_string(), query.clone()),
            ("category".to_string(), category.clone().unwrap_or_else(|| "all".to_string())),
            ("scope".to_string(), scope.clone().unwrap_or_else(|| "all".to_string())),
            ("limit".to_string(), limit.clone().unwrap_or_else(|| "5".to_string())),
        ],
    );

    // Get database
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Fact storage not available.\n\n\
                        Start a regular chat session to search facts.";
            log_tool_result("fact_search", err);
            return Ok(err.to_string());
        }
    };

    // Parse limit
    let limit_num = parse_bounded_number(limit.as_deref(), 5, Some(20));

    // Parse scope
    let parsed_scope = match scope.as_deref() {
        Some("project") | Some("local") => {
            let pid = get_project_id();
            if pid.is_none() {
                let err = "Error: Project scope requires being in a project directory.".to_string();
                log_tool_result("fact_search", &err);
                return Ok(err);
            }
            Some(Scope::Project)
        }
        Some("global") => Some(Scope::Global),
        Some(s) => {
            let err = format!("Error: Invalid scope '{}'. Use 'global' or 'project'.", s);
            log_tool_result("fact_search", &err);
            return Ok(err);
        }
        None => None,
    };

    // Perform search
    let results = match db.search_facts(&query, parsed_scope, limit_num) {
        Ok(r) => r,
        Err(e) => {
            let err = format!("Error searching facts: {}", e);
            log_tool_result("fact_search", &err);
            return Ok(err);
        }
    };

    if results.is_empty() {
        let result = format!("No facts found matching '{}'.\n\n\
                              Tips:\n\
                              - Try different keywords\n\
                              - Use broader search terms\n\
                              - Facts may not have been stored yet", query);
        log_tool_result("fact_search", &result);
        return Ok(result);
    }

    // Filter by category if specified
    let filtered_results: Vec<_> = match category.as_deref() {
        Some("preference") => results.into_iter().filter(|r| r.fact.category == Category::Preference).collect(),
        Some("fact") => results.into_iter().filter(|r| r.fact.category == Category::Fact).collect(),
        Some(c) => {
            let err = format!("Error: Invalid category '{}'. Use 'preference' or 'fact'.", c);
            log_tool_result("fact_search", &err);
            return Ok(err);
        }
        None => results,
    };

    if filtered_results.is_empty() {
        let category_label = category.as_deref().unwrap_or("all");
        let result = format!("No {} facts found matching '{}'.\n\n\
                              Try searching without category filter.", category_label, query);
        log_tool_result("fact_search", &result);
        return Ok(result);
    }

    // Format results
    let mut output = format!("**Found {} fact(s) matching '{}'**\n\n", filtered_results.len(), query);

    for result in filtered_results {
        let category_label = match result.fact.category {
            Category::Preference => "preference",
            Category::Fact => "fact",
        };
        let scope_label = match result.fact.scope {
            Scope::Global => "global",
            Scope::Project => "project",
        };

        output.push_str(&format!(
            "**[id=fact:{}]** (category: {}, scope: {}, score: {:.2})\n{}\n\n",
            result.fact.id, category_label, scope_label, result.score, result.fact.content
        ));
    }

    output.push_str("Use fact_remove(id=\"N\") to remove a fact.");

    log_tool_result("fact_search", &output);
    Ok(output)
}

/// Remove a stored fact by its ID.
///
/// Permanently deletes a fact from storage. Use fact_search first to find the ID.
///
/// # Arguments
/// * `id` - The fact ID to remove. Required.
///   - Format: "N" or "fact:N" (both accepted)
///   - Example: "42" or "fact:42"
///
/// # Returns
/// Confirmation message if successful, error if fact not found.
///
/// # Examples
/// ```ignore
/// // Remove fact by ID
/// fact_remove(id="42")
///
/// // Also accepts "fact:" prefix
/// fact_remove(id="fact:42")
/// ```
#[function]
pub async fn fact_remove(
    id: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("fact_remove", &[("id".to_string(), id.clone())]);

    // Parse ID
    let parsed_id = match parse_fact_id(&id) {
        Ok(n) => n,
        Err(e) => {
            log_tool_result("fact_remove", &e);
            return Ok(e);
        }
    };

    // Get database
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Fact storage not available.\n\n\
                        Start a regular chat session to remove facts.";
            log_tool_result("fact_remove", err);
            return Ok(err.to_string());
        }
    };

    // Check if fact exists
    let existing = match db.get_fact(parsed_id) {
        Ok(f) => f,
        Err(e) => {
            let err = format!("Error checking fact: {}", e);
            log_tool_result("fact_remove", &err);
            return Ok(err);
        }
    };

    match existing {
        Some(fact) => {
            // Delete the fact
            if let Err(e) = db.delete_fact(parsed_id) {
                let err = format!("Error removing fact: {}", e);
                log_tool_result("fact_remove", &err);
                return Ok(err);
            }

            let result = format!(
                "Removed fact:{} (was: {})\n\n\
                 Content: {}",
                parsed_id, fact.category, fact.content
            );
            log_tool_result("fact_remove", &result);
            Ok(result)
        }
        None => {
            let err = format!(
                "Error: Fact {} not found.\n\n\
                 Use fact_search(query=\"...\") to find fact IDs.",
                parsed_id
            );
            log_tool_result("fact_remove", &err);
            Ok(err)
        }
    }
}