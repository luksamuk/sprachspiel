//! Language-specific patterns and translation for fact extraction
//!
//! Centralizes all extraction patterns, normalization rules, classification
//! patterns, filler words, and PT→EN translation used by the fact extraction
//! pipeline. This ensures no string duplication across modules.
//!
//! # Architecture
//!
//! ```text
//! extract.rs  → preference_patterns(), identity_patterns(), filler_words(), command_starters()
//! prompt.rs    → normalize_replacements()
//! classify.rs  → preference_keywords()
//! lang.rs      → PT→EN translation for storage (translate_pt_to_en)
//! ```
//!
//! # Design Decisions
//!
//! - **ADR-L1:** All fact content is stored in English. PT input is translated
//!   to EN before storage via heuristic pattern-based translation. This ensures
//!   prompt rendering and FTS5 search work consistently regardless of input language.
//! - **ADR-L2:** Normalization output is always English ("User prefers", not "User prefere").
//! - **ADR-L3:** Classification keywords include both EN and PT — the classification
//!   happens before translation, so PT patterns must be recognized.

// === Extraction Patterns ===

/// Preference extraction patterns (first-person only).
///
/// Each tuple is `(regex_pattern, category_hint)`.
/// `category_hint` is used for logging only; actual classification uses `classify_fact()`.
pub fn preference_patterns() -> Vec<(&'static str, &'static str)> {
    vec![
        // ── English ──────────────────────────────────────────────
        // Strong preference
        (
            r"(?i)^i\s+(prefer|like|love|hate|dislike|want|don'?t\s+want|don'?t\s+like)\s+",
            "preference",
        ),
        // "I usually prefer/like/…"
        (
            r"(?i)^i\s+usually\s+(prefer|like|love|hate|dislike)\s+",
            "preference",
        ),
        // "I find it better/easier/…"
        (
            r"(?i)^i\s+find\s+(it\s+)?(better|worse|easier|harder|nicer)\s+",
            "preference",
        ),
        // "Always use X" / "Never prefer X"
        (
            r"(?i)^(always|never)\s+(use|prefer|choose|opt\s+for)\s+",
            "preference",
        ),
        // "I can't stand X"
        (r"(?i)^i\s+can'?t\s+stand\s+", "preference"),
        // ── Portuguese ───────────────────────────────────────────
        // "Eu prefiro X" / "Prefiro X"
        (r"(?i)^(eu\s+)?prefiro\s+", "preference"),
        // "Eu adoro X" / "Adoro X"
        (r"(?i)^(eu\s+)?adoro\s+", "preference"),
        // "Eu detesto X" / "Detesto X"
        (r"(?i)^(eu\s+)?detesto\s+", "preference"),
        // "Eu gosto de X" / "Gosto de X"
        (r"(?i)^(eu\s+)?gosto\s+de\s+", "preference"),
        // "Eu odeio X" / "Odeio X"
        (r"(?i)^(eu\s+)?odeio\s+", "preference"),
        // "Eu não gosto de X" / "Não gosto de X"
        (r"(?i)^(eu\s+)?n[aã]o\s+gosto\s+de\s+", "preference"),
        // "Eu quero X" / "Quero X"
        (r"(?i)^(eu\s+)?quero\s+", "preference"),
        // "Eu não quero X" / "Não quero X"
        (r"(?i)^(eu\s+)?n[aã]o\s+quero\s+", "preference"),
        // "Eu prefiro sempre X" / "Prefiro sempre X"
        (r"(?i)^(eu\s+)?prefiro\s+sempre\s+", "preference"),
    ]
}

/// Identity extraction patterns (first-person only).
///
/// Each tuple is `(regex_pattern, category_hint)`.
pub fn identity_patterns() -> Vec<(&'static str, &'static str)> {
    vec![
        // ── English ──────────────────────────────────────────────
        // Name
        (r"(?i)^my\s+name\s+is\s+", "identity"),
        (r"(?i)^i'?m\s+([A-Z][a-z]+)\b", "identity"), // "I'm Lucas"
        (r"(?i)^call\s+me\s+", "identity"),
        // Language
        (r"(?i)^my\s+(main\s+)?language\s+is\s+", "identity"),
        (r"(?i)^i\s+speak\s+", "identity"),
        // Work
        (r"(?i)^i\s+(work|am\s+working)\s+(at|for|in)\s+", "identity"),
        (r"(?i)^i'?m\s+(a|an)\s+\w+", "identity"), // "I'm a developer"
        // Location
        (
            r"(?i)^i\s+(live|am\s+based|am\s+from)\s+(in|at|near)\s+",
            "identity",
        ),
        (r"(?i)^i'?m\s+from\s+", "identity"),
        // Role
        (r"(?i)^i'?m\s+the\s+", "identity"),
        // ── Portuguese ───────────────────────────────────────────
        // "Meu nome é X" / "Meu nome é X"
        (r"(?i)^meu\s+nome\s+[eé]\s+", "identity"),
        // "Eu me chamo X"
        (r"(?i)^eu\s+me\s+chamo\s+", "identity"),
        // "Eu trabalho em/no X"
        (r"(?i)^eu\s+trabalho\s+(em|no|na|para)\s+", "identity"),
        // "Eu moro em X" / "Moro em X"
        (r"(?i)^(eu\s+)?moro\s+em\s+", "identity"),
        // "Eu sou de X" / "Sou de X"
        (r"(?i)^(eu\s+)?sou\s+de\s+", "identity"),
        // "Eu sou X" / "Sou X"  (profession/role)
        (r"(?i)^(eu\s+)?sou\s+(um|uma|[a-z]+)", "identity"),
        // "Eu falo X" / "Falo X"
        (r"(?i)^(eu\s+)?falo\s+", "identity"),
        // "Minha língua é X" / "Meu idioma é X"
        (
            r"(?i)^(minh[a-z]+|meu)\s+(l[ií]ngua|idioma)\s+[eé]\s+",
            "identity",
        ),
    ]
}

// === Classification Keywords ===

/// Preference keywords used by `classify_fact()`.
///
/// Returns a list of (keyword, language) pairs. The language tag is for
/// documentation only; classification treats all languages equally.
pub fn preference_keywords() -> Vec<(&'static str, &'static str)> {
    vec![
        // ── English ──────────────────────────────────────────────
        ("i prefer", "en"),
        ("i like", "en"),
        ("i hate", "en"),
        ("i usually prefer", "en"),
        ("i usually like", "en"),
        ("i usually hate", "en"),
        ("i always", "en"),
        ("i never", "en"),
        ("i want", "en"),
        ("i don't want", "en"),
        ("i dont want", "en"),
        ("i love", "en"),
        ("i dislike", "en"),
        ("i find", "en"),
        ("i find it", "en"),
        // ── Portuguese ───────────────────────────────────────────
        ("prefiro", "pt"),
        ("prefere", "pt"),
        ("gosto de", "pt"),
        ("gosta de", "pt"),
        ("odeio", "pt"),
        ("não gosto", "pt"),
        ("nao gosto", "pt"),
        ("quero", "pt"),
        ("não quero", "pt"),
        ("nao quero", "pt"),
        ("adoro", "pt"),
        ("detesto", "pt"),
        // "sempre prefiro" / "prefiro sempre"
        ("sempre", "pt"),
    ]
}

// === Normalization Replacements ===

/// Replacement pairs for `normalize_to_third_person()`.
///
/// All output is in **English**, even when the input is Portuguese.
/// Order matters: longer patterns must come first to avoid partial matches
/// (e.g., "i don't want" must match before "i want").
pub fn normalize_replacements() -> Vec<(&'static str, &'static str)> {
    vec![
        // ── English contractions and negations (longest first) ───
        ("i don't want ", "User doesn't want "),
        ("i dont want ", "User doesn't want "),
        ("i don't like ", "User doesn't like "),
        ("i dont like ", "User doesn't like "),
        ("i can't stand ", "User can't stand "),
        // ── English preferences ──────────────────────────────────
        ("i usually prefer ", "User usually prefers "),
        ("i usually like ", "User usually likes "),
        ("i usually hate ", "User usually hates "),
        ("i usually love ", "User usually loves "),
        ("i prefer ", "User prefers "),
        ("i like ", "User likes "),
        ("i hate ", "User hates "),
        ("i love ", "User loves "),
        ("i want ", "User wants "),
        ("i dislike ", "User dislikes "),
        ("i find it ", "User finds it "),
        // ── English identity ─────────────────────────────────────
        ("my name is ", "User's name is "),
        ("i live in ", "User lives in "),
        ("i work at ", "User works at "),
        ("i work for ", "User works for "),
        ("i'm from ", "User is from "),
        ("i speak ", "User speaks "),
        ("call me ", "User's name is "),
        ("i'm ", "User is "),
        ("i am ", "User is "),
        ("my ", "User's "),
        // ── Portuguese → English normalization ────────────────────
        // Negations first (longer patterns)
        ("eu não gosto de ", "User doesn't like "),
        ("eu nao gosto de ", "User doesn't like "),
        ("eu não quero ", "User doesn't want "),
        ("eu nao quero ", "User doesn't want "),
        ("não gosto de ", "Doesn't like "),
        ("nao gosto de ", "Doesn't like "),
        ("não quero ", "Doesn't want "),
        ("nao quero ", "Doesn't want "),
        // Preferences
        ("eu prefiro ", "User prefers "),
        ("eu adoro ", "User loves "),
        ("eu detesto ", "User hates "),
        ("eu gosto de ", "User likes "),
        ("eu odeio ", "User hates "),
        ("eu quero ", "User wants "),
        ("prefiro ", "User prefers "),
        ("adoro ", "User loves "),
        ("detesto ", "User hates "),
        ("gosto de ", "User likes "),
        ("odeio ", "User hates "),
        ("quero ", "User wants "),
        // Identity
        ("meu nome é ", "User's name is "),
        ("meu nome e ", "User's name is "),
        ("eu me chamo ", "User's name is "),
        ("eu trabalho em ", "User works in "),
        ("eu trabalho no ", "User works at "),
        ("eu trabalho na ", "User works at "),
        ("eu trabalho para ", "User works for "),
        ("eu moro em ", "User lives in "),
        ("moro em ", "User lives in "),
        ("eu sou de ", "User is from "),
        ("sou de ", "User is from "),
        ("eu sou ", "User is "),
        ("sou ", "User is "),
        ("eu falo ", "User speaks "),
        ("falo ", "User speaks "),
        ("minha língua é ", "User's language is "),
        ("minha lingua e ", "User's language is "),
        ("meu idioma é ", "User's language is "),
        ("meu idioma e ", "User's language is "),
    ]
}

// === Filler and Command Words ===

/// Conversational fillers that should not be extracted as facts.
///
/// These are short, content-free utterances in both EN and PT.
pub fn filler_words() -> Vec<&'static str> {
    vec![
        // ── English ──────────────────────────────────────────────
        "ok",
        "okay",
        "thanks",
        "thank you",
        "yes",
        "no",
        "sure",
        "right",
        "correct",
        "exactly",
        "perfect",
        "great",
        "cool",
        "nice",
        "good",
        "got it",
        "understood",
        "makes sense",
        "i see",
        "agreed",
        "true",
        // ── Portuguese ───────────────────────────────────────────
        "exatamente",
        "obrigado",
        "obrigada",
        "sim",
        "não",
        "claro",
        "certo",
        "verdade",
        "entendi",
        "compreendi",
        // ── Portuguese extended ───────────────────────────────────
        "tá",
        "ta",
        "beleza",
        "valeu",
        "legal",
        "massa",
        "show",
        "perfeito",
        "com certeza",
        "isso",
        "isso mesmo",
        "ah sim",
        "então",
        "bom",
        "boa",
    ]
}

/// Command starters that indicate a user request, not a fact.
///
/// Sentences starting with these words are typically commands to the assistant.
pub fn command_starters() -> Vec<&'static str> {
    vec![
        // ── English ──────────────────────────────────────────────
        "check ",
        "show ",
        "list ",
        "run ",
        "tell ",
        "give ",
        "find ",
        "search ",
        "look ",
        "get ",
        "help ",
        "explain ",
        "describe ",
        "compare ",
        "create ",
        "delete ",
        "remove ",
        "update ",
        "write ",
        "read ",
        "open ",
        "close ",
        "stop ",
        "start ",
        "retry ",
        "redo ",
        // ── Portuguese ───────────────────────────────────────────
        "verifique ",
        "verifica ",
        "mostre ",
        "mostra ",
        "liste ",
        "lista ",
        "rode ",
        "roda ",
        "execute ",
        "executa ",
        "diga ",
        "diz ",
        "busque ",
        "busca ",
        "procure ",
        "procura ",
        "encontre ",
        "encontra ",
        "ajude ",
        "ajuda ",
        "explique ",
        "explica ",
        "descreva ",
        "descreve ",
        "compare ",
        "crie ",
        "cria ",
        "delete ",
        "deleta ",
        "remova ",
        "remove ",
        "atualize ",
        "atualiza ",
        "escreva ",
        "escreve ",
        "pare ",
        "para ",
        "comece ",
        "começa ",
        "tente ",
        "tenta ",
    ]
}

// === PT→EN Translation for Storage ===

/// Translate PT preference verbs to EN equivalents for storage.
///
/// This is a heuristic translation that handles the most common PT
/// preference patterns. It translates the *prefix* of a sentence,
/// preserving the rest unchanged.
///
/// # Design Decision (ADR-L1)
///
/// All fact content is stored in English. PT input is translated before
/// storage via this function, which is called from `extract.rs` after
/// pattern matching succeeds.
///
/// # Examples
///
/// ```
/// use ask_ai::facts::lang::translate_pt_to_en;
///
/// assert_eq!(translate_pt_to_en("Eu prefiro respostas curtas"), "User prefers respostas curtas");
/// assert_eq!(translate_pt_to_en("Eu gosto de café"), "User likes café");
/// assert_eq!(translate_pt_to_en("I prefer dark mode"), "I prefer dark mode");
/// ```
pub fn translate_pt_to_en(content: &str) -> String {
    let lower = content.to_lowercase();

    // Try PT→EN prefix replacements (longer patterns first)
    let translations: &[(&str, &str)] = &[
        // Negations (longest first)
        ("eu não gosto de ", "User doesn't like "),
        ("eu nao gosto de ", "User doesn't like "),
        ("não gosto de ", "User doesn't like "),
        ("nao gosto de ", "User doesn't like "),
        ("eu não quero ", "User doesn't want "),
        ("eu nao quero ", "User doesn't want "),
        ("não quero ", "User doesn't want "),
        ("nao quero ", "User doesn't want "),
        // Preferences with "eu"
        ("eu prefiro ", "User prefers "),
        ("eu adoro ", "User loves "),
        ("eu detesto ", "User hates "),
        ("eu gosto de ", "User likes "),
        ("eu odeio ", "User hates "),
        ("eu quero ", "User wants "),
        // Without "eu"
        ("prefiro ", "User prefers "),
        ("adoro ", "User loves "),
        ("detesto ", "User hates "),
        ("gosto de ", "User likes "),
        ("odeio ", "User hates "),
        ("quero ", "User wants "),
        // Identity with "eu"
        ("meu nome é ", "My name is "),
        ("meu nome e ", "My name is "),
        ("eu me chamo ", "My name is "),
        ("eu trabalho em ", "I work in "),
        ("eu trabalho no ", "I work at "),
        ("eu trabalho na ", "I work at "),
        ("eu trabalho para ", "I work for "),
        ("eu moro em ", "I live in "),
        ("moro em ", "I live in "),
        ("eu sou de ", "I'm from "),
        ("sou de ", "I'm from "),
        ("eu falo ", "I speak "),
        ("falo ", "I speak "),
        ("minha língua é ", "My language is "),
        ("minha lingua e ", "My language is "),
        ("meu idioma é ", "My language is "),
        ("meu idioma e ", "My language is "),
        // "eu sou um/uma" → "I am a"
        ("eu sou um ", "I am a "),
        ("eu sou uma ", "I am a "),
        ("eu sou ", "I am "),
        ("sou um ", "I am a "),
        ("sou uma ", "I am a "),
        ("sou ", "I am "),
    ];

    for (pt_prefix, en_prefix) in translations {
        if lower.starts_with(pt_prefix) {
            let rest = &content[pt_prefix.len()..];
            return format!("{}{}", en_prefix, rest);
        }
    }

    // No PT pattern matched — return as-is (likely already English)
    content.to_string()
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;

    // ── Preference Patterns ────────────────────────────────────────

    #[test]
    fn test_preference_patterns_en_coverage() {
        let patterns = preference_patterns();
        let en_patterns: Vec<_> = patterns
            .iter()
            .filter(|(_, lang)| *lang == "preference")
            .collect();
        // Should have at least 5 EN preference patterns
        assert!(en_patterns.len() >= 5);
    }

    #[test]
    fn test_preference_patterns_pt_coverage() {
        let patterns = preference_patterns();
        let pt_patterns: Vec<_> = patterns
            .iter()
            .filter(|(p, _)| {
                p.contains("prefiro")
                    || p.contains("gosto")
                    || p.contains("odeio")
                    || p.contains("adoro")
                    || p.contains("detesto")
            })
            .collect();
        assert!(
            pt_patterns.len() >= 5,
            "Should have at least 5 PT preference patterns"
        );
    }

    // ── Identity Patterns ─────────────────────────────────────────

    #[test]
    fn test_identity_patterns_pt_coverage() {
        let patterns = identity_patterns();
        let pt_patterns: Vec<_> = patterns
            .iter()
            .filter(|(p, _)| {
                p.contains("nome")
                    || p.contains("chamo")
                    || p.contains("trabalho")
                    || p.contains("moro")
                    || p.contains("sou")
                    || p.contains("falo")
                    || p.contains("língua")
                    || p.contains("lingua")
                    || p.contains("idioma")
            })
            .collect();
        assert!(
            pt_patterns.len() >= 8,
            "Should have at least 8 PT identity patterns, got {}",
            pt_patterns.len()
        );
    }

    // ── Normalization Replacements ─────────────────────────────────

    #[test]
    fn test_normalize_replacements_no_pt_in_output() {
        for (_, to) in normalize_replacements() {
            // All replacement targets should be in English
            // (starting with "User", "I", "My", "Doesn't")
            let starts_with_valid = to.starts_with("User")
                || to.starts_with("I ")
                || to.starts_with("I'")
                || to.starts_with("My")
                || to.starts_with("Doesn't");
            assert!(
                starts_with_valid,
                "Normalization output '{}' should start with English prefix",
                to
            );
        }
    }

    // ── PT→EN Translation ──────────────────────────────────────────

    #[test]
    fn test_translate_pt_preference_prefiro() {
        assert_eq!(
            translate_pt_to_en("Eu prefiro respostas curtas"),
            "User prefers respostas curtas"
        );
    }

    #[test]
    fn test_translate_pt_preference_gosto() {
        assert_eq!(translate_pt_to_en("Eu gosto de café"), "User likes café");
    }

    #[test]
    fn test_translate_pt_preference_odeio() {
        assert_eq!(
            translate_pt_to_en("Eu odeio código desorganizado"),
            "User hates código desorganizado"
        );
    }

    #[test]
    fn test_translate_pt_preference_adoro() {
        assert_eq!(translate_pt_to_en("Eu adoro Rust"), "User loves Rust");
    }

    #[test]
    fn test_translate_pt_preference_detesto() {
        assert_eq!(translate_pt_to_en("Eu detesto bugs"), "User hates bugs");
    }

    #[test]
    fn test_translate_pt_negation_nao_gosto() {
        assert_eq!(
            translate_pt_to_en("Eu não gosto de código desorganizado"),
            "User doesn't like código desorganizado"
        );
    }

    #[test]
    fn test_translate_pt_negation_nao_quero() {
        assert_eq!(
            translate_pt_to_en("Eu não quero repetir isso"),
            "User doesn't want repetir isso"
        );
    }

    #[test]
    fn test_translate_pt_identity_nome() {
        assert_eq!(translate_pt_to_en("Meu nome é Lucas"), "My name is Lucas");
    }

    #[test]
    fn test_translate_pt_identity_chamo() {
        assert_eq!(translate_pt_to_en("Eu me chamo Ana"), "My name is Ana");
    }

    #[test]
    fn test_translate_pt_identity_trabalho() {
        assert_eq!(
            translate_pt_to_en("Eu trabalho no Google"),
            "I work at Google"
        );
    }

    #[test]
    fn test_translate_pt_identity_moro() {
        assert_eq!(
            translate_pt_to_en("Eu moro em São Paulo"),
            "I live in São Paulo"
        );
    }

    #[test]
    fn test_translate_pt_identity_sou() {
        assert_eq!(
            translate_pt_to_en("Eu sou desenvolvedor"),
            "I am desenvolvedor"
        );
    }

    #[test]
    fn test_translate_pt_identity_falo() {
        assert_eq!(translate_pt_to_en("Eu falo português"), "I speak português");
    }

    #[test]
    fn test_translate_pt_identity_lingua() {
        assert_eq!(
            translate_pt_to_en("Minha língua é inglês"),
            "My language is inglês"
        );
    }

    #[test]
    fn test_translate_pt_short_form() {
        // Without "eu"
        assert_eq!(
            translate_pt_to_en("Prefiro respostas curtas"),
            "User prefers respostas curtas"
        );
        assert_eq!(translate_pt_to_en("Gosto de café"), "User likes café");
    }

    #[test]
    fn test_translate_en_passthrough() {
        // English content should pass through unchanged
        assert_eq!(
            translate_pt_to_en("I prefer dark mode"),
            "I prefer dark mode"
        );
        assert_eq!(translate_pt_to_en("My name is Lucas"), "My name is Lucas");
    }

    #[test]
    fn test_translate_pt_without_eu() {
        assert_eq!(
            translate_pt_to_en("Moro em São Paulo"),
            "I live in São Paulo"
        );
        assert_eq!(translate_pt_to_en("Sou de Recife"), "I'm from Recife");
    }

    // ── Filler and Command Words ───────────────────────────────────

    #[test]
    fn test_filler_words_include_pt() {
        let fillers = filler_words();
        assert!(fillers.contains(&"obrigado"));
        assert!(fillers.contains(&"sim"));
        assert!(fillers.contains(&"não"));
        assert!(fillers.contains(&"claro"));
        assert!(fillers.contains(&"entendi"));
    }

    #[test]
    fn test_command_starters_include_pt() {
        let starters = command_starters();
        assert!(starters.contains(&"mostra "));
        assert!(starters.contains(&"busca "));
        assert!(starters.contains(&"explica "));
    }
}
