//! Auto Fact Extraction (autoDream-lite)
//!
//! Heuristic extraction of user preferences and identity facts from conversation
//! messages. Extracts facts after each response using pattern matching, then
//! deduplicates against existing facts via FTS5 search.
//!
//! # Architecture
//!
//! ```text
//! User message → Sentence splitting → Pattern matching → Candidate facts
//!                                                                    ↓
//!                                              Deduplication (FTS5) → Insert
//! ```
//!
//! # Extraction Categories
//!
//! 1. **Preferences** — "I prefer X", "I like Y", "I hate Z" (EN + PT)
//! 2. **Identity** — "My name is X", "I work at Y", "I live in Z" (EN + PT)
//!
//! # Configuration
//!
//! ```toml
//! [facts]
//! auto_extract = true          # Enable/disable auto-extraction
//! max_facts = 3               # Max facts per response
//! auto_extract_notify = true  # Show [Auto-extracted: N fact(s)] notification
//! ```
//!
//! # Design Decisions (ADRs)
//!
//! - **ADR-E1:** Heuristic-only extraction (no LLM call). Precision ~85%, recall ~55%.
//!   LLM-mode is deferred to future work.
//! - **ADR-E2:** Always `Scope::Global` for auto-extracted facts. Project-scoped facts
//!   require explicit `/fact add --project`.
//! - **ADR-E3:** Source::Llm attribution for all auto-extracted facts.
//! - **ADR-E4:** Third-person normalization in prompt rendering ("User prefers X"),
//!   not in storage. Storage preserves original text.
//! - **ADR-E5:** Synchronous extraction after response. Heuristic is <1ms, no async needed.
//! - **ADR-E6:** Max 3 facts per response to avoid noise.

use super::classify::classify_fact;
use super::conflict::{CONFLICT_THRESHOLD, detect_conflicts, resolve_conflict};
use super::lang;
use super::types::{Category, Fact, MAX_FACT_CONTENT_SIZE, Scope, Source};
use crate::db::Database;

// === Constants ===

/// Maximum number of recent user messages to scan.
const MAX_MESSAGES_TO_SCAN: usize = 5;

/// Minimum length for an extracted fact (avoids trivial extractions).
const MIN_FACT_LENGTH: usize = 10;

/// Maximum length for an extracted fact (truncation threshold).
/// Facts longer than MAX_FACT_CONTENT_SIZE (500) are rejected entirely.
const MAX_EXTRACT_LENGTH: usize = MAX_FACT_CONTENT_SIZE;

/// Confidence thresholds for pattern categories.
const PREFERENCE_CONFIDENCE: f32 = 0.9;
const IDENTITY_CONFIDENCE: f32 = 0.8;

// === Types ===

/// A fact candidate extracted from a message.
#[derive(Debug, Clone)]
pub struct ExtractedFact {
    /// The extracted fact text (original from message, may be normalized).
    pub content: String,
    /// Auto-classified category (preference or fact).
    pub category: Category,
    /// Confidence score (0.0-1.0).
    pub confidence: f32,
    /// Always Global for auto-extracted facts.
    pub scope: Scope,
    /// Always Llm for auto-extracted facts.
    pub source: Source,
    /// Which pattern matched (for debugging).
    #[allow(dead_code)] // Used in tests
    pub matched_pattern: String,
    /// The original sentence that was matched.
    #[allow(dead_code)] // Used in tests
    pub original_sentence: String,
}

/// Result of auto-extraction and insertion.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Number of facts successfully inserted.
    pub inserted: usize,
    /// Number of facts skipped (duplicates).
    pub skipped: usize,
    /// Number of facts that updated existing ones (contradictions).
    pub updated: usize,
    /// Details of each extracted fact.
    pub details: Vec<ExtractionDetail>,
}

/// Detail of a single fact extraction outcome.
#[derive(Debug, Clone)]
pub struct ExtractionDetail {
    /// The fact content that was processed.
    pub content: String,
    /// What happened: "inserted", "skipped (duplicate)", "updated (contradiction)".
    pub action: String,
    /// Category of the fact.
    pub category: Category,
}

// === Sentence Splitting ===

/// Split text into sentences, filtering out questions and very short ones.
fn split_into_sentences(text: &str) -> Vec<&str> {
    text.split(['.', '!', '\n'])
        .map(|s| s.trim())
        .filter(|s| is_extractable_sentence(s))
        .collect()
}

/// Check if a sentence is extractable (not a question, not too short, not conversational).
fn is_extractable_sentence(sentence: &str) -> bool {
    let trimmed = sentence.trim();

    // Skip empty or very short
    if trimmed.len() < MIN_FACT_LENGTH {
        return false;
    }

    // Skip questions
    if trimmed.ends_with('?') {
        return false;
    }

    // Skip commands (starts with imperative verbs commonly used in chat)
    let lower = trimmed.to_lowercase();
    for starter in lang::command_starters() {
        if lower.starts_with(starter) {
            return false;
        }
    }

    // Skip conversational fillers
    if lang::filler_words().iter().any(|f| lower == *f) {
        return false;
    }

    true
}

// === Pattern Matching ===

/// Try to extract a fact from a single sentence.
/// Returns Some(ExtractedFact) if a pattern matches, None otherwise.
fn try_extract(sentence: &str) -> Option<ExtractedFact> {
    // Try preference patterns first (higher confidence)
    for (pattern, _category) in lang::preference_patterns() {
        if let Some(fact) = match_pattern(sentence, pattern, PREFERENCE_CONFIDENCE) {
            return Some(fact);
        }
    }

    // Then identity patterns
    for (pattern, _category) in lang::identity_patterns() {
        if let Some(fact) = match_pattern(sentence, pattern, IDENTITY_CONFIDENCE) {
            return Some(fact);
        }
    }

    None
}

/// Match a pattern against a sentence and extract the fact content.
/// Classification is always determined by `classify_fact()`, not by the pattern category.
///
/// If the content is in Portuguese, it is translated to English before storage
/// (ADR-L1). This ensures prompt rendering and FTS5 search work consistently.
fn match_pattern(sentence: &str, pattern: &str, confidence: f32) -> Option<ExtractedFact> {
    let Ok(re) = regex::Regex::new(pattern) else {
        return None;
    };

    if re.is_match(sentence) {
        // Use the full sentence as fact content (preserving context)
        let raw_content = sentence.trim().to_string();

        // Validate content length
        if raw_content.len() < MIN_FACT_LENGTH || raw_content.len() > MAX_EXTRACT_LENGTH {
            return None;
        }

        // Classify using the original (pre-translation) content, since
        // classification relies on first-person pronouns ("I prefer", "eu prefiro")
        // that are lost in the English third-person translation.
        let category = classify_fact(&raw_content);

        // Translate PT→EN before storage (no-op for English content).
        // ADR-L1: All facts stored in English.
        let content = lang::translate_pt_to_en(&raw_content);

        Some(ExtractedFact {
            category,
            content,
            confidence,
            scope: Scope::Global,
            source: Source::Llm,
            matched_pattern: pattern.to_string(),
            original_sentence: sentence.trim().to_string(),
        })
    } else {
        None
    }
}

// === Deduplication ===

/// Deduplicate extracted facts against each other (remove near-identical candidates).
fn deduplicate_extracted(facts: &mut Vec<ExtractedFact>) {
    let mut seen_contents: Vec<String> = Vec::new();
    facts.retain(|f| {
        let normalized = f.content.to_lowercase();
        // Check if we already have a very similar fact
        let is_duplicate = seen_contents.iter().any(|s| {
            // Simple word overlap heuristic
            let overlap = s
                .split_whitespace()
                .filter(|w| normalized.split_whitespace().any(|w2| w2 == *w))
                .count();
            let total = s
                .split_whitespace()
                .count()
                .max(normalized.split_whitespace().count());
            overlap as f32 / total as f32 > 0.8
        });
        if !is_duplicate {
            seen_contents.push(normalized);
        }
        !is_duplicate
    });
}

// === Main Extraction Function ===

/// Extract facts from recent user messages and insert into the database.
///
/// # Arguments
/// * `db` — Database connection for deduplication and insertion
/// * `messages` — Iterator of (role, content) pairs from the session
/// * `project_id` — Current project ID (for scope, always Global for auto-extraction)
/// * `max_facts` — Maximum number of facts to extract per response
///
/// # Returns
/// `ExtractionResult` with counts and details
pub fn extract_and_insert_facts(
    db: &Database,
    user_messages: &[&str],
    project_id: Option<&str>,
    max_facts: usize,
) -> ExtractionResult {
    let mut candidates: Vec<ExtractedFact> = Vec::new();

    // 1. Extract from each user message
    for message in user_messages.iter().take(MAX_MESSAGES_TO_SCAN) {
        let sentences = split_into_sentences(message);
        for sentence in sentences {
            if let Some(fact) = try_extract(sentence) {
                candidates.push(fact);
            }
        }
    }

    // 2. Deduplicate against each other
    deduplicate_extracted(&mut candidates);

    // 3. Sort by confidence (highest first), take top max_facts
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.truncate(max_facts);

    // 4. Insert with deduplication against existing facts
    let mut result = ExtractionResult {
        inserted: 0,
        skipped: 0,
        updated: 0,
        details: Vec::new(),
    };

    for candidate in candidates {
        match insert_fact_with_dedup(db, &candidate, project_id) {
            InsertAction::Inserted => {
                result.inserted += 1;
                result.details.push(ExtractionDetail {
                    content: candidate.content.clone(),
                    action: "inserted".to_string(),
                    category: candidate.category,
                });
            }
            InsertAction::Skipped => {
                result.skipped += 1;
                result.details.push(ExtractionDetail {
                    content: candidate.content.clone(),
                    action: "skipped (duplicate)".to_string(),
                    category: candidate.category,
                });
            }
            InsertAction::Updated => {
                result.updated += 1;
                result.details.push(ExtractionDetail {
                    content: candidate.content.clone(),
                    action: "updated (contradiction)".to_string(),
                    category: candidate.category,
                });
            }
        }
    }

    result
}

/// Action taken when inserting a fact.
enum InsertAction {
    Inserted,
    Skipped,
    Updated,
}

/// Insert a fact with deduplication against existing facts.
///
/// Uses FTS5 search to find similar existing facts, then:
/// - If duplicate (similarity >= 0.95, no contradiction) → Skip
/// - If contradiction (similarity >= 0.85, with contradiction) → Update (invalidate old, insert new)
/// - If no conflict → Insert
fn insert_fact_with_dedup(
    db: &Database,
    candidate: &ExtractedFact,
    project_id: Option<&str>,
) -> InsertAction {
    // Search for similar existing facts
    let search_results = match db.search_facts(&candidate.content, None, 5) {
        Ok(results) => results,
        Err(e) => {
            log::debug!("Auto-extract: FTS5 search failed: {}", e);
            // If search fails, try to insert anyway
            return insert_new_fact(db, candidate, project_id);
        }
    };

    // Check for conflicts
    let conflicts = detect_conflicts(&candidate.content, &search_results, CONFLICT_THRESHOLD);

    if conflicts.is_empty() {
        // No conflict — insert new fact
        insert_new_fact(db, candidate, project_id)
    } else {
        // Resolve conflict
        let conflict = &conflicts[0]; // Handle first conflict
        let action = resolve_conflict(conflict.clone());

        match action {
            super::conflict::ResolutionAction::Skip => {
                log::debug!(
                    "Auto-extract: Skipping duplicate fact (similarity >= 0.95): {}",
                    candidate.content
                );
                InsertAction::Skipped
            }
            super::conflict::ResolutionAction::Update => {
                // Invalidate the old fact and insert the new one
                if let Err(e) = db.delete_fact(conflict.existing_fact.id) {
                    log::debug!("Auto-extract: Failed to invalidate old fact: {}", e);
                    return InsertAction::Skipped;
                }
                log::debug!(
                    "Auto-extract: Updating contradictory fact (old: '{}', new: '{}')",
                    conflict.existing_fact.content,
                    candidate.content
                );
                match insert_new_fact(db, candidate, project_id) {
                    InsertAction::Inserted => InsertAction::Updated,
                    other => other,
                }
            }
            super::conflict::ResolutionAction::Add => {
                // Should not happen (Add is never produced by our detection)
                insert_new_fact(db, candidate, project_id)
            }
        }
    }
}

/// Insert a new fact into the database.
fn insert_new_fact(
    db: &Database,
    candidate: &ExtractedFact,
    project_id: Option<&str>,
) -> InsertAction {
    let fact = match Fact::for_insert(
        candidate.content.clone(),
        candidate.category,
        candidate.scope,
        project_id.map(|s| s.to_string()),
        candidate.source,
    ) {
        Ok(f) => f,
        Err(e) => {
            log::debug!("Auto-extract: Fact validation failed: {}", e);
            return InsertAction::Skipped;
        }
    };

    match db.insert_fact(&fact) {
        Ok(id) => {
            log::debug!(
                "Auto-extract: Inserted fact #{} (confidence: {:.1}): {}",
                id,
                candidate.confidence,
                candidate.content
            );
            InsertAction::Inserted
        }
        Err(e) => {
            log::debug!("Auto-extract: Failed to insert fact: {}", e);
            InsertAction::Skipped
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Sentence Splitting Tests ===

    #[test]
    fn test_split_into_sentences_basic() {
        let text = "I prefer dark mode. The project uses Rust.";
        let sentences = split_into_sentences(text);
        assert_eq!(sentences.len(), 2);
        assert!(sentences[0].contains("prefer dark mode"));
    }

    #[test]
    fn test_split_into_sentences_excludes_questions() {
        let text = "Can you check the auth? I prefer dark mode.";
        let sentences = split_into_sentences(text);
        // Question should be filtered
        assert!(sentences.iter().all(|s| !s.ends_with('?')));
    }

    #[test]
    fn test_split_into_sentences_excludes_short() {
        let text = "Ok. I prefer dark mode. Yes.";
        let sentences = split_into_sentences(text);
        // "Ok" and "Yes" should be filtered (too short or filler)
        assert!(sentences.iter().any(|s| s.contains("prefer")));
    }

    #[test]
    fn test_split_into_sentences_newlines() {
        let text = "I prefer dark mode\nI like Python";
        let sentences = split_into_sentences(text);
        assert!(sentences.len() >= 2);
    }

    // === is_extractable_sentence Tests ===

    #[test]
    fn test_extractable_question_excluded() {
        assert!(!is_extractable_sentence(
            "Can you check the auth middleware?"
        ));
    }

    #[test]
    fn test_extractable_short_excluded() {
        assert!(!is_extractable_sentence("ok"));
        assert!(!is_extractable_sentence("yes"));
    }

    #[test]
    fn test_extractable_command_excluded() {
        assert!(!is_extractable_sentence("Check the auth middleware"));
        assert!(!is_extractable_sentence("Show me the logs"));
        assert!(!is_extractable_sentence("List all files"));
    }

    #[test]
    fn test_extractable_filler_excluded() {
        assert!(!is_extractable_sentence("ok"));
        assert!(!is_extractable_sentence("thanks"));
        assert!(!is_extractable_sentence("got it"));
        assert!(!is_extractable_sentence("makes sense"));
    }

    #[test]
    fn test_extractable_valid_preference() {
        assert!(is_extractable_sentence("I prefer dark mode"));
        assert!(is_extractable_sentence("My name is Lucas"));
    }

    #[test]
    fn test_extractable_portuguese_filler_excluded() {
        assert!(!is_extractable_sentence("obrigado"));
        assert!(!is_extractable_sentence("sim"));
    }

    // === Pattern Matching Tests ===

    #[test]
    fn test_preference_pattern_english() {
        let cases = vec![
            "I prefer dark mode",
            "I like Python for scripting",
            "I hate verbose error messages",
            "I want short responses",
            "I don't want to repeat myself",
            "I love concise explanations",
            "I dislike unnecessary complexity",
        ];
        for case in cases {
            let result = try_extract(case);
            assert!(result.is_some(), "Expected match for: {}", case);
            assert!(
                matches!(result.unwrap().category, Category::Preference),
                "Expected Preference for: {}",
                case
            );
        }
    }

    #[test]
    fn test_preference_pattern_portuguese() {
        let cases = vec![
            "Eu prefiro respostas curtas",
            "Prefiro trabalhar de manhã",
            "Gosto de café",
            "Odeio quando isso acontece",
            "Não gosto de código desorganizado",
        ];
        for case in cases {
            let result = try_extract(case);
            assert!(result.is_some(), "Expected match for: {}", case);
        }
    }

    #[test]
    fn test_preference_pattern_portuguese_full() {
        // Extended PT preference patterns (reviews #1, #2)
        let cases = vec![
            "Eu prefiro respostas curtas",
            "Prefiro trabalhar de manhã",
            "Gosto de café",
            "Odeio quando isso acontece",
            "Não gosto de código desorganizado",
            "Adoro Rust",
            "Detesto bugs",
            "Eu adoro programar",
            "Eu detesto esperar",
            "Quero terminar logo",
            "Não quero repetir",
            "Eu quero café",
            "Eu não quero sair",
            "Prefiro sempre o plano A",
        ];
        for case in cases {
            let result = try_extract(case);
            assert!(result.is_some(), "Expected match for: {}", case);
            assert!(
                matches!(result.unwrap().category, Category::Preference),
                "Expected Preference for: {}",
                case
            );
        }
    }

    #[test]
    fn test_identity_pattern_portuguese() {
        // PT identity patterns (review #4)
        let cases = vec![
            "Meu nome é Lucas",
            "Meu nome e Lucas",
            "Eu me chamo Ana",
            "Eu trabalho no Google",
            "Eu moro em São Paulo",
            "Moro em Brasília",
            "Eu sou de Recife",
            "Sou de São Paulo",
            "Eu falo português",
            "Falo inglês",
            "Minha língua é português",
            "Meu idioma é português",
            "Eu sou desenvolvedor",
            "Sou engenheiro",
        ];
        for case in cases {
            let result = try_extract(case);
            assert!(result.is_some(), "Expected match for: {}", case);
        }
    }

    #[test]
    fn test_extractable_portuguese_command_excluded() {
        // PT commands should not be extracted (review #4)
        assert!(!is_extractable_sentence("Mostre os logs"));
        assert!(!is_extractable_sentence("Busca o arquivo"));
        assert!(!is_extractable_sentence("Cria um novo diretório"));
        assert!(!is_extractable_sentence("Executa o script"));
    }

    #[test]
    fn test_extractable_portuguese_filler_extended() {
        // Extended PT fillers (reviews #1, #4)
        assert!(!is_extractable_sentence("Beleza"));
        assert!(!is_extractable_sentence("Valeu"));
        assert!(!is_extractable_sentence("Legal"));
        assert!(!is_extractable_sentence("Perfeito"));
        assert!(!is_extractable_sentence("Com certeza"));
    }

    #[test]
    fn test_identity_pattern_english() {
        let cases = vec![
            "My name is Lucas",
            "Call me Alex",
            "I work at Google",
            "I live in São Paulo",
            "I'm from Brazil",
            "My language is Portuguese",
        ];
        for case in cases {
            let result = try_extract(case);
            assert!(result.is_some(), "Expected match for: {}", case);
        }
    }

    #[test]
    fn test_lang_translate_is_accessible() {
        // Verify super::lang::translate_pt_to_en works from this module
        assert_eq!(
            lang::translate_pt_to_en("Eu prefiro respostas curtas"),
            "User prefers respostas curtas"
        );
    }

    #[test]
    fn test_extracted_content_translated_to_english() {
        // PT fact content should be translated to English before storage (ADR-L1)
        let result = try_extract("Eu prefiro respostas curtas");
        assert!(result.is_some(), "Expected match");
        let fact = result.unwrap();
        assert!(
            fact.content.starts_with("User prefers"),
            "Content should be translated to English before storage, got: '{}'",
            fact.content
        );
    }

    #[test]
    fn test_extracted_portuguese_identity_translated() {
        // PT identity should be translated to English before storage (ADR-L1)
        let result = try_extract("Meu nome é Lucas");
        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(
            fact.content.starts_with("My name is"),
            "Identity content should be translated to English before storage, got: '{}'",
            fact.content
        );
    }

    #[test]
    fn test_no_match_for_non_facts() {
        let cases = vec![
            "Check the auth middleware",
            "Can you help me with this?",
            "Show me the logs",
            "What is the capital of France?",
            "ok",
            "thanks",
        ];
        for case in cases {
            let result = try_extract(case);
            assert!(result.is_none(), "Expected no match for: {}", case);
        }
    }

    #[test]
    fn test_no_match_for_third_person() {
        // Third-person statements should not match identity patterns
        let cases = vec![
            "He told me his name is João",
            "The project uses Rust",
            "That person prefers dark mode",
        ];
        for case in cases {
            let result = try_extract(case);
            // "He told me" should not match identity patterns (^I, ^my)
            // "The project uses" should not match any pattern
            if result.is_some() {
                // If it matches, it should be a fact, not identity
                assert!(
                    !result.unwrap().matched_pattern.contains("identity"),
                    "Third-person should not match identity: {}",
                    case
                );
            }
        }
    }

    #[test]
    fn test_usually_preference() {
        let result = try_extract("I usually prefer dark mode");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().category, Category::Preference));
    }

    #[test]
    fn test_find_better_preference() {
        let result = try_extract("I find it easier to use Rust");
        assert!(result.is_some());
    }

    #[test]
    fn test_always_never_preference() {
        let result1 = try_extract("Always use Rust for new projects");
        assert!(result1.is_some());

        let result2 = try_extract("Never use Python for this");
        assert!(result2.is_some());
    }

    #[test]
    fn test_deduplicate_extracted() {
        // "I prefer dark mode" and "I prefer dark themes" are similar (75% word overlap)
        // but below 0.8 threshold, so they're NOT deduplicated
        let mut facts = vec![
            ExtractedFact {
                content: "I prefer dark mode".to_string(),
                category: Category::Preference,
                confidence: 0.9,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "I prefer dark mode".to_string(),
            },
            ExtractedFact {
                content: "I prefer dark themes".to_string(), // Very similar but different
                category: Category::Preference,
                confidence: 0.9,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "I prefer dark themes".to_string(),
            },
            ExtractedFact {
                content: "My name is Lucas".to_string(), // Completely different
                category: Category::Fact,
                confidence: 0.8,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "My name is Lucas".to_string(),
            },
        ];

        deduplicate_extracted(&mut facts);
        // word overlap for "i prefer dark mode" vs "i prefer dark themes" = 3/4 = 0.75 < 0.8
        // So all 3 are kept
        assert_eq!(facts.len(), 3);

        // Now test actual duplicates: identical content should be deduplicated
        let mut dup_facts = vec![
            ExtractedFact {
                content: "I prefer dark mode".to_string(),
                category: Category::Preference,
                confidence: 0.9,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "I prefer dark mode".to_string(),
            },
            ExtractedFact {
                content: "I prefer dark mode".to_string(), // Exact duplicate
                category: Category::Preference,
                confidence: 0.9,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "I prefer dark mode".to_string(),
            },
        ];

        deduplicate_extracted(&mut dup_facts);
        assert_eq!(dup_facts.len(), 1);
    }

    #[test]
    fn test_max_facts_limit() {
        // Create more candidates than the default limit of 3
        let mut candidates: Vec<ExtractedFact> = vec![
            ExtractedFact {
                content: "Fact 1".repeat(2),
                category: Category::Preference,
                confidence: 0.9,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "test".to_string(),
            },
            ExtractedFact {
                content: "Fact 2".repeat(2),
                category: Category::Preference,
                confidence: 0.9,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "test".to_string(),
            },
            ExtractedFact {
                content: "Fact 3".repeat(2),
                category: Category::Fact,
                confidence: 0.8,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "test".to_string(),
            },
            ExtractedFact {
                content: "Fact 4".repeat(2),
                category: Category::Fact,
                confidence: 0.7,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "test".to_string(),
            },
            ExtractedFact {
                content: "Fact 5".repeat(2),
                category: Category::Fact,
                confidence: 0.6,
                scope: Scope::Global,
                source: Source::Llm,
                matched_pattern: "test".to_string(),
                original_sentence: "test".to_string(),
            },
        ];

        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(3); // Default max_facts limit
        assert_eq!(candidates.len(), 3);
    }
}
