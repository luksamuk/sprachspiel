//! Fact tools for the LLM to store and retrieve user/project information
//!
//! Provides tools for the LLM to autonomously store facts about the user
//! and project, enabling personalization across sessions.

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::facts::classify::classify_fact;
use crate::facts::conflict::{CONFLICT_THRESHOLD, detect_conflicts, resolve_conflict};
use crate::facts::extract::is_extractable_sentence;
use crate::facts::lang;
use crate::facts::types::{Category, Fact, MAX_FACT_CONTENT_SIZE, Scope, Source};
use crate::project::get_project_id;
use crate::tools::context::get_db;
use crate::tools::context::get_embedding;
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

    numeric_str.parse::<i64>().map_err(|_| {
        format!(
            "Invalid fact ID: '{}'. Use format 'fact:N' or just 'N'.",
            id
        )
    })
}

/// Store a fact or preference about the user or project.
///
/// **IMPORTANT: Content must be in English.** If the user speaks Portuguese
/// or any other language, translate the fact to English before storing.
/// Example: User says "Eu prefiro respostas curtas" → store "I prefer short responses".
///
/// The system automatically classifies facts as "preference" (user likes/dislikes)
/// or "fact" (objective information). You can override this with the category parameter.
///
/// Facts are stored globally by default. Use scope="project" for project-specific information.
///
/// # Arguments
/// * `content` - The fact to store in English (max 500 characters). Required.
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
/// // Translate from Portuguese before storing
/// // User said: "Eu prefiro respostas curtas"
/// fact_add(content="I prefer short responses")
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
            (
                "category".to_string(),
                category.clone().unwrap_or_else(|| "auto".to_string()),
            ),
            (
                "scope".to_string(),
                scope.clone().unwrap_or_else(|| "global".to_string()),
            ),
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

    // Validate content: reject fillers, commands, questions, and short text
    // This prevents the LLM from storing non-fact content that bypasses auto-extraction filtering.
    let lower = content.trim().to_lowercase();

    // Reject questions
    if content.trim().ends_with('?') {
        let msg = format!(
            "Skipped: '{}' appears to be a question, not a fact.",
            content.trim()
        );
        log_tool_result("fact_add", &msg);
        return Ok(msg);
    }

    // Reject conversational fillers
    if lang::filler_words().iter().any(|f| lower == *f) {
        let msg = format!(
            "Skipped: '{}' appears to be a conversational filler, not a fact.",
            content.trim()
        );
        log_tool_result("fact_add", &msg);
        return Ok(msg);
    }

    // Reject commands
    for starter in lang::command_starters() {
        if lower.starts_with(starter) {
            let msg = format!(
                "Skipped: '{}' appears to be a command, not a fact.",
                content.trim()
            );
            log_tool_result("fact_add", &msg);
            return Ok(msg);
        }
    }

    // Reject very short content (less than minimum extractable length)
    if content.trim().len() < 10 {
        let msg = "Skipped: Fact content is too short (minimum 10 characters).".to_string();
        log_tool_result("fact_add", &msg);
        return Ok(msg);
    }

    // Use is_extractable_sentence for additional validation.
    // This catches third-person statements, short phrases, and other non-fact content
    // that the simpler checks above might miss.
    if !is_extractable_sentence(&content) {
        let msg = format!(
            "Skipped: '{}' does not appear to be a personal fact or preference.",
            content.trim()
        );
        log_tool_result("fact_add", &msg);
        return Ok(msg);
    }

    // Normalize to storage format: PT→EN + EN first-person→third-person (ADR-E4 revised).
    // Auto-extraction already does this; fact_add must do the same for consistency.
    let content = lang::normalize_to_storage_format(&content);

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
            let err = format!("Error: Invalid scope '{}'. Use 'global' or 'project'.", s);
            log_tool_result("fact_add", &err);
            return Ok(err);
        }
    };

    // Create fact
    let fact = match Fact::for_insert(
        content.clone(),
        parsed_category,
        parsed_scope,
        project_id.clone(),
        Source::Llm,
    ) {
        Ok(f) => f,
        Err(e) => {
            let err = format!("Error creating fact: {}", e);
            log_tool_result("fact_add", &err);
            return Ok(err);
        }
    };

    // ====================================================================
    // Layer 1: Exact content match (case-insensitive, trimmed)
    // Catches obvious duplicates like "I prefer dark mode" == "i prefer dark mode"
    // ====================================================================
    let content_trimmed = content.trim().to_lowercase();
    match db.find_exact_fact(&content_trimmed) {
        Ok(Some(existing)) => {
            let result = format!(
                "Skipped: Exact duplicate already exists (fact:{}).\n\n\
                 Existing: {}\n\
                 New: {}\n\n\
                 Use fact_remove(id=\"{}\") first if you want to replace it.",
                existing.id, existing.content, content, existing.id
            );
            log_tool_result("fact_add", &result);
            return Ok(result);
        }
        Ok(None) => { /* No exact match, continue */ }
        Err(e) => {
            log::debug!("fact_add: Exact match query failed: {}", e);
        }
    }

    // ====================================================================
    // Layer 2: Normalized content match (strips pronouns/subjects)
    // Catches "I prefer dark mode" ≈ "User prefers dark mode"
    // ====================================================================
    let normalized_query = lang::normalize_for_comparison(&content);
    match db.find_normalized_fact(&normalized_query) {
        Ok(matches) if !matches.is_empty() => {
            // Found a normalized match — handle conflicts
            if parsed_scope == Scope::Global {
                // Global-wins-project: remove Project duplicates
                let mut global_match: Option<&crate::facts::types::Fact> = None;
                for fact in &matches {
                    if fact.scope == Scope::Project {
                        log::debug!(
                            "fact_add: Global fact overrides Project fact (id={}): '{}'",
                            fact.id,
                            fact.content
                        );
                        if let Err(e) = db.delete_fact(fact.id) {
                            log::debug!("fact_add: Failed to delete Project fact: {}", e);
                        }
                    } else {
                        global_match = Some(fact);
                    }
                }
                if let Some(existing) = global_match {
                    let result = format!(
                        "Skipped: Similar fact already exists (fact:{}).\n\n\
                         Existing: {}\n\
                         New: {}\n\n\
                         Use fact_remove(id=\"{}\") first if you want to replace it.",
                        existing.id, existing.content, content, existing.id
                    );
                    log_tool_result("fact_add", &result);
                    return Ok(result);
                }
                // All duplicates were Project-scope and removed — fall through to insert
            } else {
                // Project-scope: any existing match = skip
                let existing = &matches[0];
                let result = format!(
                    "Skipped: Similar fact already exists (fact:{}).\n\n\
                     Existing: {}\n\
                     New: {}\n\n\
                     Use fact_remove(id=\"{}\") first if you want to replace it.",
                    existing.id, existing.content, content, existing.id
                );
                log_tool_result("fact_add", &result);
                return Ok(result);
            }
        }
        Ok(_) => { /* No normalized match, continue */ }
        Err(e) => {
            log::debug!("fact_add: Normalized match query failed: {}", e);
        }
    }

    // ====================================================================
    // Layer 3: FTS5 keyword search with BM25 scoring
    // ====================================================================
    let conflicts = match db.search_facts(&normalized_query, None, 5) {
        Ok(results) => detect_conflicts(&content, &results, CONFLICT_THRESHOLD),
        Err(_) => {
            // If search fails, continue without conflict check
            Vec::new()
        }
    };

    // ====================================================================
    // Layer 3.5: Semantic embedding similarity (preference override detection)
    //
    // If FTS5 didn't find conflicts, check semantic similarity via embeddings.
    // This catches cases like "prefer dark mode" vs "prefer light mode"
    // where FTS5 BM25 score is too low but embeddings show high similarity.
    // ====================================================================
    if conflicts.is_empty()
        && parsed_category == Category::Preference
        && let Some(client) = get_embedding()
    {
        match crate::facts::embedding::generate_fact_embedding(&content, &client).await {
            Ok(candidate_embedding) => {
                match db.search_facts_semantic(&candidate_embedding, None, 5) {
                    Ok(semantic_results) => {
                        for result in &semantic_results {
                            if result.score < 0.90 {
                                continue; // Below semantic similarity threshold
                            }

                            if crate::facts::conflict::is_contradiction(
                                &content,
                                &result.fact.content,
                            ) {
                                log::debug!(
                                    "fact_add: Semantic contradiction found (cosine={:.3}): '{}' vs '{}'",
                                    result.score,
                                    content,
                                    result.fact.content
                                );
                                // Remove the old fact and insert the new one
                                if let Err(e) = db.delete_fact(result.fact.id) {
                                    log::debug!(
                                        "fact_add: Failed to delete contradicting fact: {}",
                                        e
                                    );
                                    continue;
                                }
                                // Fall through to insert below with the new fact
                                break;
                            }

                            // Not a contradiction but high similarity — it's a duplicate
                            log::debug!(
                                "fact_add: Semantic duplicate found (cosine={:.3}): '{}' vs '{}'",
                                result.score,
                                content,
                                result.fact.content
                            );
                            let result_msg = format!(
                                "Skipped: Similar fact already exists (fact:{}).\n\n\
                                 Existing: {}\n\
                                 New: {}\n\n\
                                 Use fact_remove(id=\"{}\") first if you want to replace it.",
                                result.fact.id, result.fact.content, content, result.fact.id
                            );
                            log_tool_result("fact_add", &result_msg);
                            return Ok(result_msg);
                        }
                    }
                    Err(e) => {
                        log::debug!("fact_add: Semantic search failed: {}", e);
                        // Fall through to insert
                    }
                }
            }
            Err(e) => {
                log::debug!(
                    "fact_add: Failed to generate embedding for semantic dedup: {}",
                    e
                );
                // Fall through to insert without semantic check
            }
        }
    }

    // Handle FTS5 conflicts
    if !conflicts.is_empty() {
        // Global-wins-project rule: when adding a Global-scope fact,
        // remove ALL conflicting Project-scope facts first, then resolve
        // remaining Global conflicts normally.
        if parsed_scope == Scope::Global {
            // Remove all conflicting Project-scope facts
            let mut remaining_conflicts = Vec::new();
            for conflict in &conflicts {
                if conflict.existing_fact.scope == Scope::Project {
                    log::debug!(
                        "fact_add: Global fact overrides Project fact (id={}): '{}'",
                        conflict.existing_fact.id,
                        conflict.existing_fact.content
                    );
                    if let Err(e) = db.delete_fact(conflict.existing_fact.id) {
                        log::debug!(
                            "fact_add: Failed to delete Project fact (id={}): {}",
                            conflict.existing_fact.id,
                            e
                        );
                    }
                } else {
                    remaining_conflicts.push(conflict);
                }
            }

            // If only Project conflicts existed, they've been removed — proceed to insert
            if remaining_conflicts.is_empty() {
                // Fall through to insert below
            } else {
                // Resolve remaining Global conflicts
                let conflict = remaining_conflicts[0];
                match resolve_conflict(conflict.clone()) {
                    crate::facts::conflict::ResolutionAction::Skip => {
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
                        if let Err(e) = db.delete_fact(conflict.existing_fact.id) {
                            let err = format!("Error resolving conflict: {}", e);
                            log_tool_result("fact_add", &err);
                            return Ok(err);
                        }
                        // Insert new fact below
                    }
                    crate::facts::conflict::ResolutionAction::Add => {
                        // No conflict - continue with insert below
                    }
                }
            }
        } else {
            // Project-scope fact: normal conflict resolution
            let conflict = &conflicts[0];

            match resolve_conflict(conflict.clone()) {
                crate::facts::conflict::ResolutionAction::Skip => {
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
                    if let Err(e) = db.delete_fact(conflict.existing_fact.id) {
                        let err = format!("Error resolving conflict: {}", e);
                        log_tool_result("fact_add", &err);
                        return Ok(err);
                    }
                    // Insert new fact below
                }
                crate::facts::conflict::ResolutionAction::Add => {
                    // No conflict - continue with insert below
                }
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

    // Generate embedding for the newly inserted fact (eager, fire-and-forget).
    // If Ollama is offline, has_embedding stays 0 and recovery generates on next startup.
    if let (Some(db_arc), Some(client)) = (get_db(), get_embedding()) {
        let scope_str = parsed_scope.to_string();
        let category_str = parsed_category.to_string();
        let pid = project_id.clone();
        let content_for_emb = content.clone();
        tokio::spawn(async move {
            match crate::facts::embedding::generate_fact_embedding(&content_for_emb, &client).await
            {
                Ok(emb) => {
                    if let Err(e) = db_arc.update_fact_embedding(
                        id,
                        &emb,
                        &scope_str,
                        &category_str,
                        pid.as_deref(),
                    ) {
                        log::debug!("fact_add: failed to store embedding: {}", e);
                    }
                }
                Err(e) => {
                    log::debug!("fact_add: failed to generate embedding: {}", e);
                }
            }
        });
    }

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
            (
                "category".to_string(),
                category.clone().unwrap_or_else(|| "all".to_string()),
            ),
            (
                "scope".to_string(),
                scope.clone().unwrap_or_else(|| "all".to_string()),
            ),
            (
                "limit".to_string(),
                limit.clone().unwrap_or_else(|| "5".to_string()),
            ),
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
        let result = format!(
            "No facts found matching '{}'.\n\n\
                              Tips:\n\
                              - Try different keywords\n\
                              - Use broader search terms\n\
                              - Facts may not have been stored yet",
            query
        );
        log_tool_result("fact_search", &result);
        return Ok(result);
    }

    // Filter by category if specified
    let filtered_results: Vec<_> = match category.as_deref() {
        Some("preference") => results
            .into_iter()
            .filter(|r| r.fact.category == Category::Preference)
            .collect(),
        Some("fact") => results
            .into_iter()
            .filter(|r| r.fact.category == Category::Fact)
            .collect(),
        Some(c) => {
            let err = format!(
                "Error: Invalid category '{}'. Use 'preference' or 'fact'.",
                c
            );
            log_tool_result("fact_search", &err);
            return Ok(err);
        }
        None => results,
    };

    if filtered_results.is_empty() {
        let category_label = category.as_deref().unwrap_or("all");
        let result = format!(
            "No {} facts found matching '{}'.\n\n\
                              Try searching without category filter.",
            category_label, query
        );
        log_tool_result("fact_search", &result);
        return Ok(result);
    }

    // Format results
    let mut output = format!(
        "**Found {} fact(s) matching '{}'**\n\n",
        filtered_results.len(),
        query
    );

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
pub async fn fact_remove(id: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
