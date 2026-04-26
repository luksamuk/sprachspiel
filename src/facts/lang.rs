//! Language-specific patterns and translation for fact extraction
//!
//! Centralizes all extraction patterns, normalization rules, classification
//! patterns, filler words, and PT→EN translation used by the fact extraction
//! pipeline. This ensures no string duplication across modules.
//!
//! # Architecture
//!
//! ```text
//! extract.rs    → preference_patterns(), identity_patterns(), filler_words(), command_starters()
//! prompt.rs      → normalize_replacements() (render-time defense-in-depth)
//! classify.rs    → preference_keywords()
//! lang.rs        → normalize_to_storage_format() (storage-time normalization)
//!                  translate_pt_to_en() (PT→EN + EN 1st→3rd, core logic)
//!                  normalize_for_comparison() (dedup)
//! ```
//!
//! # Design Decisions
//!
//! - **ADR-L1:** All fact content is stored in English. PT input is translated
//!   to EN before storage via heuristic pattern-based translation. This ensures
//!   prompt rendering and FTS5 search work consistently regardless of input language.
//!   Noun translation (e.g., "respostas curtas" → "short responses") is deferred
//!   to LLM-mode (issue #106, M2).
//! - **ADR-L2:** Normalization output is always English ("User prefers", not "User prefere").
//! - **ADR-L3:** Classification keywords include both EN and PT — the classification
//!   happens before translation, so PT patterns must be recognized.
//! - **ADR-E4 (revised):** Third-person normalization is applied at storage time,
//!   not just render time. All facts are stored in third person ("User prefers X",
//!   not "I prefer X"). Render-time normalization in `prompt.rs` remains as a
//!   defense-in-depth layer for any legacy first-person data.

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

// === Adverb + Verb Expansion for Storage Normalization ===

/// English adverb modifiers that can appear between subject and verb.
///
/// These modify the verb intensity but don't change its meaning for
/// dedup purposes. Used by `normalize_adverb_verb()` to catch patterns
/// like "I really like X" → "User really likes X".
const EN_ADVERBS: &[(&str, &str)] = &[
    ("really", "really"),
    ("usually", "usually"),
    ("always", "always"),
    ("never", "never"),
    ("generally", "generally"),
    ("mostly", "mostly"),
    ("definitely", "definitely"),
    ("absolutely", "absolutely"),
    ("personally", "personally"),
    ("often", "often"),
    ("sometimes", "sometimes"),
    ("quite", "quite"),
    ("particularly", "particularly"),
    ("especially", "especially"),
    ("strongly", "strongly"),
];

/// Portuguese adverb modifiers with their English translations.
///
/// Used by `normalize_adverb_verb()` to transform PT adverb+verb
/// patterns like "Eu sempre prefiro X" → "User always prefers X".
const PT_ADVERBS: &[(&str, &str)] = &[
    ("realmente", "really"),
    ("sempre", "always"),
    ("nunca", "never"),
    ("geralmente", "generally"),
    ("definitivamente", "definitely"),
    ("absolutamente", "absolutely"),
    ("pessoalmente", "personally"),
    ("frequentemente", "often"),
    ("às vezes", "sometimes"),
    ("as vezes", "sometimes"),
    ("bastante", "quite"),
    ("particularmente", "particularly"),
    ("especialmente", "especially"),
];

/// English first-person verbs and their third-person forms.
///
/// Maps base form (first-person) → third-person form.
const EN_VERBS_FP_TP: &[(&str, &str)] = &[
    ("prefer", "prefers"),
    ("like", "likes"),
    ("love", "loves"),
    ("hate", "hates"),
    ("dislike", "dislikes"),
    ("want", "wants"),
    ("find", "finds"),
    ("use", "uses"),
];

/// Portuguese first-person verbs and their English third-person translations.
///
/// Maps PT first-person form → EN third-person form.
/// "gosto de" is special because it takes "de" before the object.
const PT_VERBS_EN_TP: &[(&str, &str)] = &[
    ("prefiro", "prefers"),
    ("adoro", "loves"),
    ("detesto", "hates"),
    ("odeio", "hates"),
    ("quero", "wants"),
    ("gosto de", "likes"),
];

/// Regex-based expansion for adverb + verb patterns missed by static lists.
///
/// This function handles patterns like:
/// - `"I really like X"` → `"User really likes X"` (EN adverb + verb)
/// - `"I always prefer X"` → `"User always prefers X"` (EN adverb + verb)
/// - `"I never want X"` → `"User never wants X"` (EN adverb + verb)
/// - `"Eu sempre prefiro X"` → `"User always prefers X"` (PT adverb + PT verb → EN)
/// - `"Eu realmente gosto de X"` → `"User really likes X"` (PT adverb + PT verb → EN)
///
/// Returns `None` if no pattern matches, so callers can fall through
/// to the default (return content as-is).
///
/// # Strategy
///
/// 1. Try PT patterns: `(Eu|eu) (adverb) (verb) rest` → `User {adverb_en} {verb_en_tp} rest`
/// 2. Try EN patterns: `(I|i) (adverb) (verb) rest` → `User {adverb} {verb_tp} rest`
/// 3. Try EN contraction patterns: `(I|i) (adverb) don't (verb) rest` → `User {adverb} doesn't {verb} rest`
fn normalize_adverb_verb(content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    let trimmed = content.trim();

    // ── PT adverb + verb expansion: "Eu sempre prefiro X" ─────────
    // Match: (Eu|eu) (pt_adverb) (pt_verb) rest
    if let Some(after_eu_lower) = lower.strip_prefix("eu ") {
        let after_eu_original = &trimmed[3..]; // case-insensitive match confirmed above

        for (pt_adverb, en_adverb) in PT_ADVERBS {
            let adverb_with_space = format!("{} ", pt_adverb);
            if after_eu_lower.starts_with(&adverb_with_space) {
                let after_adverb = &after_eu_lower[pt_adverb.len() + 1..];
                let after_adverb_orig = &after_eu_original[pt_adverb.len() + 1..];

                for (pt_verb, en_verb_tp) in PT_VERBS_EN_TP {
                    let verb_with_space = format!("{} ", pt_verb);
                    if after_adverb.starts_with(&verb_with_space) {
                        let rest = &after_adverb_orig[pt_verb.len() + 1..];
                        return Some(format!("User {} {} {}", en_adverb, en_verb_tp, rest));
                    } else if after_adverb == *pt_verb {
                        // Verb at end with no rest
                        return Some(format!("User {} {}", en_adverb, en_verb_tp));
                    }
                }
            }
        }
    }

    // ── EN adverb + verb expansion: "I really like X" ─────────────
    // Match: (I|i) (adverb) (verb) rest
    if let Some(after_i_lower) = lower.strip_prefix("i ") {
        let after_i_original = &trimmed[2..]; // case-insensitive match confirmed above

        for (en_adverb, en_adverb_output) in EN_ADVERBS {
            let adverb_with_space = format!("{} ", en_adverb);
            if after_i_lower.starts_with(&adverb_with_space) {
                let after_adverb = &after_i_lower[en_adverb.len() + 1..];
                let after_adverb_orig = &after_i_original[en_adverb.len() + 1..];

                // Check for negation pattern: "I usually don't like X" → "User usually doesn't like X"
                if after_adverb.starts_with("don't ") || after_adverb.starts_with("dont ") {
                    let skip = if after_adverb.starts_with("don't ") {
                        6
                    } else {
                        5
                    };
                    if after_adverb_orig.len() > skip {
                        let neg_rest = after_adverb_orig.get(skip..).unwrap_or("");
                        return Some(format!("User {} doesn't {}", en_adverb_output, neg_rest));
                    }
                }

                for (en_verb_fp, en_verb_tp) in EN_VERBS_FP_TP {
                    let verb_with_space = format!("{} ", en_verb_fp);
                    if after_adverb.starts_with(&verb_with_space) {
                        let rest = &after_adverb_orig[en_verb_fp.len() + 1..];
                        return Some(format!("User {} {} {}", en_adverb_output, en_verb_tp, rest));
                    } else if after_adverb == *en_verb_fp {
                        // Verb at end with no rest
                        return Some(format!("User {} {}", en_adverb_output, en_verb_tp));
                    }
                }
            }
        }
    }

    None
}
///
/// Used by `normalize_for_comparison()` to lemmatize verbs after stripping
/// the subject pronoun, ensuring "prefers dark mode" matches "prefer dark mode".
const VERB_LEMMAS: &[(&str, &str)] = &[
    ("prefers", "prefer"),
    ("likes", "like"),
    ("hates", "hate"),
    ("loves", "love"),
    ("wants", "want"),
    ("dislikes", "dislike"),
    ("finds", "find"),
    ("works", "work"),
    ("lives", "live"),
    ("speaks", "speak"),
    ("uses", "use"),
    ("doesn't like", "doesn't like"), // no change — already lemma form
    ("doesn't want", "doesn't want"), // no change — already lemma form
    ("can't stand", "can't stand"),   // no change — already lemma form
    ("usually prefers", "usually prefer"),
    ("usually likes", "usually like"),
    ("usually hates", "usually hate"),
    ("usually loves", "usually love"),
    ("always prefers", "always prefer"),
    ("always likes", "always like"),
    ("always hates", "always hate"),
    ("always loves", "always love"),
    ("never prefers", "never prefer"),
    ("never likes", "never like"),
    ("never hates", "never hate"),
    ("never wants", "never want"),
];

/// Normalize fact content for deduplication comparison.
///
/// Strips subject pronouns, lemmatizes verbs, and produces a canonical
/// form that maximizes token overlap for FTS5 search and exact string
/// comparison between semantically equivalent facts.
///
/// # How it works
///
/// Uses THREE strategies in priority order:
///
/// 1. **Identity/copula prefixes** — Strip the entire prefix including the
///    copula ("is", "am", "name is") because the key content follows.
///    - "My name is Lucas" → "lucas"
///    - "I am from Brazil" → "brazil"
///    - "User's name is Lucas" → "lucas"
///
/// 2. **Verb prefixes with lemmatization** — Strip the subject pronoun,
///    then lemmatize the verb (3rd person → base form) so that
///    "prefers dark mode" matches "prefer dark mode" (Bug #2 fix).
///    - "I prefer dark mode" → "prefer dark mode"
///    - "User prefers dark mode" → "prefer dark mode"
///    - "Eu prefiro dark mode" → "prefiro dark mode" (PT lemma not applied)
///
/// 3. **Fallback** — Lowercase and trim.
///
/// # Examples
///
/// ```
/// use ask_ai::facts::lang::normalize_for_comparison;
///
/// // Verb lemmatization: 3rd person → base form
/// assert_eq!(normalize_for_comparison("I prefer dark mode"), "prefer dark mode");
/// assert_eq!(normalize_for_comparison("User prefers dark mode"), "prefer dark mode");
/// assert_eq!(normalize_for_comparison("I like Python"), "like python");
/// assert_eq!(normalize_for_comparison("User likes Python"), "like python");
///
/// // PT verbs: subject stripped but PT verb preserved (FTS5 tokenizes)
/// assert_eq!(normalize_for_comparison("Eu prefiro dark mode"), "prefiro dark mode");
///
/// // Identity: full prefix strip (name, location, etc.)
/// assert_eq!(normalize_for_comparison("I am a developer"), "a developer");
/// assert_eq!(normalize_for_comparison("User is a developer"), "a developer");
/// assert_eq!(normalize_for_comparison("My name is Lucas"), "lucas");
///
/// // No prefix: lowercase trimmed
/// assert_eq!(normalize_for_comparison("The project uses SQLite"), "the project uses sqlite");
/// ```
pub fn normalize_for_comparison(content: &str) -> String {
    let lower = content.to_lowercase();

    // ── Identity/copula prefixes (full strip) ──────────────────────
    // These contain copula verbs ("is", "am", "name is") — the key
    // content (name, location, language) follows, so we strip everything.
    let identity_prefixes: &[&str] = &[
        // Third-person
        "user's name is ",
        "user's language is ",
        "user is from ",
        "user is a ",
        "user is ",
        "user lives in ",
        "user works in ",
        "user works at ",
        "user works for ",
        "user speaks ",
        "user's ",
        // First-person EN
        "my name is ",
        "my language is ",
        "i'm from ",
        "i'm a ",
        "i'm ",
        "i am ",
        "i live in ",
        "i work at ",
        "i work for ",
        "i work in ",
        "i speak ",
        "my ",
        // First-person PT — identity forms strip fully
        "meu nome é ",
        "meu nome e ",
        "eu me chamo ",
        "eu moro em ",
        "moro em ",
        "eu sou de ",
        "sou de ",
        "eu sou um ",
        "eu sou uma ",
        "eu sou ",
        "sou um ",
        "sou uma ",
        "sou ",
        "eu falo ",
        "falo ",
        "minha língua é ",
        "minha lingua e ",
        "meu idioma é ",
        "meu idioma e ",
        "meu ",
        "minha ",
        "eu trabalho em ",
        "eu trabalho no ",
        "eu trabalho na ",
        "eu trabalho para ",
    ];

    // ── Verb prefixes (subject-only strip + lemmatization) ─────────
    // These contain important verbs that we KEEP for FTS5 matching.
    // After stripping the subject, we lemmatize the verb so that
    // "prefers dark mode" → "prefer dark mode" matches "prefer dark mode".
    let verb_subject_only: &[&str] = &[
        "user ", // Third-person — "User prefers X" → strip "user " → lemmatize
        "i ",    // First-person EN — "I prefer X" → strip "i " → keep verb as-is
        "eu ",   // First-person PT — "Eu prefiro X" → strip "eu " → keep PT verb
    ];

    // Try identity prefixes first (they're more specific)
    for prefix in identity_prefixes {
        if lower.starts_with(prefix) {
            let rest = &content[prefix.len()..];
            return rest.to_lowercase().trim().to_string();
        }
    }

    // Try verb prefixes (strip subject + lemmatize verb)
    for prefix in verb_subject_only {
        if lower.starts_with(prefix) {
            let rest = &content[prefix.len()..];
            let result = rest.to_lowercase().trim().to_string();
            // Lemmatize verb: try known multi-word phrases first (longer match wins),
            // then single-word verb forms.
            // Only applies after third-person subject ("user"), since first-person
            // verbs are already in base form ("prefer", "like", etc.).
            if *prefix == "user " {
                return lemmatize_verb(&result);
            }
            return result;
        }
    }

    // No known prefix — return lowercase trimmed
    lower.trim().to_string()
}

/// Lemmatize the first verb in a string from 3rd person to base form.
///
/// Tries multi-word phrase matches first (e.g., "doesn't like" before "doesn't"),
/// then single-word matches. If no known pattern matches, applies a generic
/// rule: strip trailing 's' from the first word if it looks like a 3rd-person
/// verb (lowercase, ends in 's' but not 'ss').
///
/// This ensures that "prefers dark mode" and "prefer dark mode" produce the
/// same normalized output for Layer 2 dedup comparison.
fn lemmatize_verb(s: &str) -> String {
    // Try known multi-word phrases first (longer match wins)
    // Sort by length descending so "usually prefers" matches before "prefers"
    let mut phrases: Vec<(&str, &str)> = VERB_LEMMAS.to_vec();
    phrases.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

    let lower = s.to_lowercase();
    for (third_person, lemma) in &phrases {
        if lower.starts_with(third_person) {
            let rest = &s[third_person.len()..];
            return format!("{}{}", lemma, rest);
        }
    }

    // Generic rule: if first word ends in 's' (but not 'ss'), strip the 's'
    // This catches verbs like "works"→"work", "lives"→"live", "speaks"→"speak"
    // while avoiding "class"→"clas" or "address"→"addres"
    if let Some(space_pos) = lower.find(' ') {
        let first_word = &lower[..space_pos];
        if first_word.ends_with('s') && !first_word.ends_with("ss") && first_word.len() > 2 {
            let lemma = &first_word[..first_word.len() - 1];
            let rest = &s[space_pos..];
            return format!("{}{}", lemma, rest);
        }
    } else if lower.ends_with('s') && !lower.ends_with("ss") && lower.len() > 2 {
        // Single word: "prefers" → "prefer"
        return lower[..lower.len() - 1].to_string();
    }

    s.to_string()
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

/// Translate PT→EN and normalize EN first-person to third-person for storage.
///
/// This function is the core of storage normalization (ADR-E4 revised: all facts
/// stored in third person). It applies transformations in priority order:
///
/// 1. **PT→EN translation** — Portuguese prefix patterns produce English third-person
///    output (e.g., `"Eu prefiro X"` → `"User prefers X"`).
///
/// 2. **EN first-person → third-person** — If no PT pattern matched, English
///    first-person patterns are normalized (e.g., `"I prefer X"` → `"User prefers X"`).
///
/// If neither transformation matches, the content is returned as-is.
///
/// # Important: Use `normalize_to_storage_format()` instead
///
/// This function is kept public for backward compatibility and test coverage,
/// but new code should prefer `normalize_to_storage_format()` which delegates
/// to this function. The name `translate_pt_to_en` is a historical artifact;
/// the function now handles EN normalization as well.
///
/// # Limitations (deferred to issue #106)
///
/// Only prefixes are translated. Nouns and adjectives after the prefix remain
/// in their original language. Full PT→EN noun translation will be handled
/// by LLM-mode (issue #106, M2).
///
/// # Examples
///
/// ```
/// use ask_ai::facts::lang::translate_pt_to_en;
///
/// // PT→EN (third-person output)
/// assert_eq!(translate_pt_to_en("Eu prefiro respostas curtas"), "User prefers respostas curtas");
/// assert_eq!(translate_pt_to_en("Meu nome é Ana"), "My name is Ana");
///
/// // EN first-person → third-person (new behavior)
/// assert_eq!(translate_pt_to_en("I prefer dark mode"), "User prefers dark mode");
/// assert_eq!(translate_pt_to_en("My name is Lucas"), "User's name is Lucas");
///
/// // Already third person or factual — no change
/// assert_eq!(translate_pt_to_en("The project uses Rust"), "The project uses Rust");
/// ```
pub fn translate_pt_to_en(content: &str) -> String {
    let lower = content.to_lowercase();

    // Try PT→EN prefix replacements (longer patterns first)
    let translations: &[(&str, &str)] = &[
        // ── Negations (longest first) ────────────────────────────────
        ("eu não gosto de ", "User doesn't like "),
        ("eu nao gosto de ", "User doesn't like "),
        ("não gosto de ", "User doesn't like "),
        ("nao gosto de ", "User doesn't like "),
        ("ele não gosta de ", "User doesn't like "),
        ("ela não gosta de ", "User doesn't like "),
        ("não gosta de ", "User doesn't like "),
        ("nao gosta de ", "User doesn't like "),
        ("eu não quero ", "User doesn't want "),
        ("eu nao quero ", "User doesn't want "),
        ("não quero ", "User doesn't want "),
        ("nao quero ", "User doesn't want "),
        // ── First-person preferences ──────────────────────────────────
        ("eu prefiro ", "User prefers "),
        ("eu adoro ", "User loves "),
        ("eu detesto ", "User hates "),
        ("eu gosto de ", "User likes "),
        ("eu odeio ", "User hates "),
        ("eu quero ", "User wants "),
        // ── Third-person PT preferences (LLM-generated hybrids) ──────
        // The LLM sometimes generates third-person PT like "prefere"
        // or "adora" without "eu" prefix.
        ("prefere ", "User prefers "),
        ("prefiro ", "User prefers "),
        ("adora ", "User loves "),
        ("adoro ", "User loves "),
        ("detesta ", "User hates "),
        ("detesto ", "User hates "),
        ("gosta de ", "User likes "),
        ("gosto de ", "User likes "),
        ("odeia ", "User hates "),
        ("odeio ", "User hates "),
        ("quer ", "User wants "),
        ("quero ", "User wants "),
        // ── PT identity patterns ──────────────────────────────────────
        // Second/third-person identity that the LLM might generate
        ("o nome do usuário é ", "User's name is "),
        ("o nome do usuario e ", "User's name is "),
        // First-person identity with "eu"
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

    // No PT pattern matched — try EN first-person → third-person normalization
    // (ADR-E4: All facts are stored in third person, not first person)
    let lower = content.to_lowercase();

    // Try EN first-person → third-person replacements
    // Order matters: longer patterns first to avoid partial matching
    // (e.g., "I don't like" must match before "I like")
    for (from, to) in normalize_replacements() {
        if lower.starts_with(from) {
            let rest = &content[from.len()..];
            return format!("{}{}", to, rest);
        }
    }

    // No static pattern matched — try regex adverb+verb expansion.
    // This catches patterns like "I really like X" → "User really likes X",
    // "I always prefer X" → "User always prefers X",
    // "Eu sempre prefiro X" → "User always prefers X", etc.
    // that aren't in the static prefix lists.
    if let Some(result) = normalize_adverb_verb(content) {
        return result;
    }

    // No pattern matched — return as-is
    content.to_string()
}

// === Storage Normalization ===

/// Normalize fact content for storage (ADR-E4: third-person only).
///
/// This is the primary normalization function called before storing any fact,
/// whether from auto-extraction (`extract.rs`) or the LLM tool (`fact_add`).
///
/// It applies two transformations in priority order:
///
/// 1. **PT→EN translation** — Portuguese prefix patterns are translated to
///    English third-person output. E.g., `"Eu prefiro X"` → `"User prefers X"`.
///
/// 2. **EN first-person → third-person** — English first-person patterns are
///    normalized to third-person. E.g., `"I prefer X"` → `"User prefers X"`,
///    `"My name is X"` → `"User's name is X"`.
///
/// # Third-person only (ADR-E4 revised)
///
/// All facts are stored in third person. This prevents the LLM from confusing
/// user preferences with its own identity when facts appear in system prompts.
/// The prompt-rendering function `normalize_to_third_person()` remains as a
/// defense-in-depth layer for any legacy data.
///
/// # Limitations (Bug #2, deferred to issue #106)
///
/// Only prefixes are translated. Nouns and adjectives after the prefix remain
/// in their original language. For example:
/// - `"Eu prefiro respostas curtas"` → `"User prefers respostas curtas"`
///   (not "User prefers short responses" — noun translation deferred to LLM-mode)
/// - `"Detesto café"` → `"User hates café"`
///   (not "User hates coffee" — noun translation deferred to LLM-mode)
///
/// Full PT→EN noun translation will be handled by LLM-mode (issue #106, M2).
///
/// # Examples
///
/// ```
/// use ask_ai::facts::lang::normalize_to_storage_format;
///
/// // EN first-person → third person
/// assert_eq!(normalize_to_storage_format("I prefer dark mode"), "User prefers dark mode");
/// assert_eq!(normalize_to_storage_format("My name is Lucas"), "User's name is Lucas");
/// assert_eq!(normalize_to_storage_format("I work at Google"), "User works at Google");
///
/// // PT → EN third person
/// assert_eq!(normalize_to_storage_format("Eu prefiro respostas curtas"), "User prefers respostas curtas");
/// assert_eq!(normalize_to_storage_format("Meu nome é Ana"), "My name is Ana");
///
/// // Already third person or factual — no change
/// assert_eq!(normalize_to_storage_format("The project uses Rust"), "The project uses Rust");
/// ```
pub fn normalize_to_storage_format(content: &str) -> String {
    // translate_pt_to_en now handles both PT→EN and EN 1st→3rd person
    // in priority order (PT patterns checked first, then EN patterns)
    translate_pt_to_en(content)
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
    fn test_translate_en_first_person_to_third_person() {
        // English first-person content should be normalized to third-person (ADR-E4 revised)
        assert_eq!(
            translate_pt_to_en("I prefer dark mode"),
            "User prefers dark mode"
        );
        assert_eq!(
            translate_pt_to_en("My name is Lucas"),
            "User's name is Lucas"
        );
        assert_eq!(
            translate_pt_to_en("I work at Google"),
            "User works at Google"
        );
        assert_eq!(
            translate_pt_to_en("I live in São Paulo"),
            "User lives in São Paulo"
        );
    }

    #[test]
    fn test_translate_en_third_person_passthrough() {
        // English third-person/factual content should pass through unchanged
        assert_eq!(
            translate_pt_to_en("The project uses Rust"),
            "The project uses Rust"
        );
        assert_eq!(
            translate_pt_to_en("User prefers dark mode"),
            "User prefers dark mode"
        );
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

    // ── normalize_for_comparison ────────────────────────────────────

    #[test]
    fn test_normalize_comparison_first_person() {
        // "I prefer dark mode" → "prefer dark mode"
        assert_eq!(
            normalize_for_comparison("I prefer dark mode"),
            "prefer dark mode"
        );
    }

    #[test]
    fn test_normalize_comparison_third_person() {
        // "User prefers dark mode" → "prefer dark mode" (lemmatized, Bug #2 fix)
        assert_eq!(
            normalize_for_comparison("User prefers dark mode"),
            "prefer dark mode"
        );
    }

    #[test]
    fn test_normalize_comparison_pt_preference() {
        // "Eu prefiro dark mode" → "prefiro dark mode" (PT verb preserved for FTS5)
        assert_eq!(
            normalize_for_comparison("Eu prefiro dark mode"),
            "prefiro dark mode"
        );
    }

    #[test]
    fn test_normalize_comparison_identity() {
        // Identity/copula forms strip the full prefix (subject + copula verb)
        // because the key content follows: "I am a developer" → "developer"
        assert_eq!(normalize_for_comparison("I am a developer"), "a developer");
        // "User is a developer" → "developer" (identity prefix strip)
        assert_eq!(normalize_for_comparison("User is a developer"), "developer");
        // "My name is Lucas" → "Lucas"
        assert_eq!(normalize_for_comparison("My name is Lucas"), "lucas");
    }

    #[test]
    fn test_normalize_comparison_no_prefix() {
        // Factual content without subject prefix stays as-is (lowercased)
        assert_eq!(
            normalize_for_comparison("The project uses SQLite"),
            "the project uses sqlite"
        );
    }

    #[test]
    fn test_normalize_comparison_shared_tokens() {
        // Both forms should produce identical normalized queries (Bug #2 fix)
        // "I prefer dark mode" → "prefer dark mode"
        // "User prefers dark mode" → "prefer dark mode" (lemmatized)
        let first = normalize_for_comparison("I prefer dark mode");
        let third = normalize_for_comparison("User prefers dark mode");
        assert_eq!(first, "prefer dark mode");
        assert_eq!(third, "prefer dark mode");
        // Exact match — Layer 2 dedup now catches these as duplicates
        assert_eq!(first, third);
    }

    #[test]
    fn test_normalize_comparison_lemma_third_person() {
        // Bug #2: Lemmatization of third-person verbs for dedup
        assert_eq!(
            normalize_for_comparison("User prefers dark mode"),
            "prefer dark mode"
        );
        assert_eq!(normalize_for_comparison("User likes Python"), "like python");
        assert_eq!(
            normalize_for_comparison("User hates verbose errors"),
            "hate verbose errors"
        );
        assert_eq!(normalize_for_comparison("User loves Rust"), "love rust");
        assert_eq!(
            normalize_for_comparison("User works at Google"),
            // "works" → "work" after identity prefix "user works at " doesn't match
            // since "user works at " is an identity prefix (full strip)
            "google"
        );
        assert_eq!(
            normalize_for_comparison("User wants concise responses"),
            "want concise responses"
        );
    }

    #[test]
    fn test_normalize_comparison_lemma_adverb_phrases() {
        // Adverb + verb phrases: "usually prefers" → "usually prefer"
        assert_eq!(
            normalize_for_comparison("User usually prefers dark mode"),
            "usually prefer dark mode"
        );
        assert_eq!(
            normalize_for_comparison("User always likes Python"),
            "always like python"
        );
        assert_eq!(
            normalize_for_comparison("User never hates errors"),
            "never hate errors"
        );
    }

    #[test]
    fn test_normalize_comparison_lemma_generic_verbs() {
        // Generic 's' stripping for verbs not in explicit map
        assert_eq!(normalize_for_comparison("User uses Rust"), "use rust");
        // "class" should NOT be lemmatized (ends in 'ss')
        assert_eq!(
            normalize_for_comparison("User class is active"),
            "class is active"
        );
    }

    // ── Third-person PT translation (LLM hybrid output) ───────────

    #[test]
    fn test_translate_pt_third_person_prefere() {
        // LLM might generate "Prefere respostas curtas" (3rd person PT)
        assert_eq!(
            translate_pt_to_en("Prefere respostas curtas"),
            "User prefers respostas curtas"
        );
    }

    #[test]
    fn test_translate_pt_third_person_adora() {
        // LLM might generate "Adora Rust"
        assert_eq!(translate_pt_to_en("Adora Rust"), "User loves Rust");
    }

    #[test]
    fn test_translate_pt_third_person_gosta() {
        // LLM might generate "Gosta de café"
        assert_eq!(translate_pt_to_en("Gosta de café"), "User likes café");
    }

    #[test]
    fn test_translate_pt_third_person_odeia() {
        // LLM might generate "Odeia bugs"
        assert_eq!(translate_pt_to_en("Odeia bugs"), "User hates bugs");
    }

    #[test]
    fn test_translate_pt_username() {
        // LLM might generate "O nome do usuário é Ana"
        assert_eq!(
            translate_pt_to_en("O nome do usuário é Ana"),
            "User's name is Ana"
        );
    }

    #[test]
    fn test_translate_pt_username_ascii() {
        // LLM might generate "O nome do usuario e Ana" (without accents)
        assert_eq!(
            translate_pt_to_en("O nome do usuario e Ana"),
            "User's name is Ana"
        );
    }

    #[test]
    fn test_translate_pt_negation_third_person() {
        // "Não gosta de bugs" (3rd person negation)
        assert_eq!(
            translate_pt_to_en("Não gosta de bugs"),
            "User doesn't like bugs"
        );
    }

    // ── normalize_to_storage_format ──────────────────────────────────

    #[test]
    fn test_storage_format_en_first_person() {
        // English first-person → third-person (ADR-E4 revised)
        assert_eq!(
            normalize_to_storage_format("I prefer dark mode"),
            "User prefers dark mode"
        );
        assert_eq!(
            normalize_to_storage_format("I like Python"),
            "User likes Python"
        );
        assert_eq!(
            normalize_to_storage_format("I hate verbose errors"),
            "User hates verbose errors"
        );
        assert_eq!(
            normalize_to_storage_format("My name is Lucas"),
            "User's name is Lucas"
        );
        assert_eq!(
            normalize_to_storage_format("I work at Google"),
            "User works at Google"
        );
        assert_eq!(
            normalize_to_storage_format("I live in São Paulo"),
            "User lives in São Paulo"
        );
    }

    #[test]
    fn test_storage_format_pt_to_third_person() {
        // PT → EN third-person
        assert_eq!(
            normalize_to_storage_format("Eu prefiro respostas curtas"),
            "User prefers respostas curtas"
        );
        assert_eq!(
            normalize_to_storage_format("Meu nome é Ana"),
            "My name is Ana"
        );
    }

    #[test]
    fn test_storage_format_third_person_passthrough() {
        // Already third-person or factual — no change
        assert_eq!(
            normalize_to_storage_format("The project uses Rust"),
            "The project uses Rust"
        );
        assert_eq!(
            normalize_to_storage_format("User prefers dark mode"),
            "User prefers dark mode"
        );
    }

    #[test]
    fn test_storage_format_en_negation() {
        // English negation patterns → third-person
        assert_eq!(
            normalize_to_storage_format("I don't like verbose errors"),
            "User doesn't like verbose errors"
        );
        assert_eq!(
            normalize_to_storage_format("I don't want to repeat myself"),
            "User doesn't want to repeat myself"
        );
    }

    // ── Adverb + Verb Expansion (Bug #1) ────────────────────────────────

    #[test]
    fn test_adverb_en_really_like() {
        assert_eq!(
            normalize_to_storage_format("I really like dark mode"),
            "User really likes dark mode"
        );
    }

    #[test]
    fn test_adverb_en_always_prefer() {
        assert_eq!(
            normalize_to_storage_format("I always prefer concise answers"),
            "User always prefers concise answers"
        );
    }

    #[test]
    fn test_adverb_en_never_want() {
        assert_eq!(
            normalize_to_storage_format("I never want verbose output"),
            "User never wants verbose output"
        );
    }

    #[test]
    fn test_adverb_en_usually_hate() {
        assert_eq!(
            normalize_to_storage_format("I usually hate boilerplate code"),
            "User usually hates boilerplate code"
        );
    }

    #[test]
    fn test_adverb_en_definitely_love() {
        assert_eq!(
            normalize_to_storage_format("I definitely love Rust"),
            "User definitely loves Rust"
        );
    }

    #[test]
    fn test_adverb_en_personally_dislike() {
        assert_eq!(
            normalize_to_storage_format("I personally dislike JavaScript"),
            "User personally dislikes JavaScript"
        );
    }

    #[test]
    fn test_adverb_en_often_find() {
        assert_eq!(
            normalize_to_storage_format("I often find Python readable"),
            "User often finds Python readable"
        );
    }

    #[test]
    fn test_adverb_en_sometimes_use() {
        assert_eq!(
            normalize_to_storage_format("I sometimes use Vim"),
            "User sometimes uses Vim"
        );
    }

    #[test]
    fn test_adverb_en_strongly_prefer() {
        assert_eq!(
            normalize_to_storage_format("I strongly prefer dark themes"),
            "User strongly prefers dark themes"
        );
    }

    #[test]
    fn test_adverb_en_adverb_dont() {
        // "I usually don't like X" → "User usually doesn't like X"
        assert_eq!(
            normalize_to_storage_format("I usually don't like verbose output"),
            "User usually doesn't like verbose output"
        );
    }

    #[test]
    fn test_adverb_pt_sempre_prefiro() {
        // "Eu sempre prefiro X" → "User always prefers X"
        assert_eq!(
            normalize_to_storage_format("Eu sempre prefiro respostas curtas"),
            "User always prefers respostas curtas"
        );
    }

    #[test]
    fn test_adverb_pt_realmente_gosto() {
        // "Eu realmente gosto de X" → "User really likes X"
        assert_eq!(
            normalize_to_storage_format("Eu realmente gosto de café"),
            "User really likes café"
        );
    }

    #[test]
    fn test_adverb_pt_nunca_quero() {
        // "Eu nunca quero X" → "User never wants X"
        assert_eq!(
            normalize_to_storage_format("Eu nunca quero repetir isso"),
            "User never wants repetir isso"
        );
    }

    #[test]
    fn test_adverb_pt_geralmente_adoro() {
        // "Eu geralmente adoro X" → "User generally loves X"
        assert_eq!(
            normalize_to_storage_format("Eu geralmente adoro Rust"),
            "User generally loves Rust"
        );
    }

    #[test]
    fn test_adverb_pt_definitivamente_odeio() {
        // "Eu definitivamente odeio X" → "User definitely hates X"
        assert_eq!(
            normalize_to_storage_format("Eu definitivamente odeio bugs"),
            "User definitely hates bugs"
        );
    }

    #[test]
    fn test_adverb_pt_as_vezes_detesto() {
        // "Eu às vezes detesto X" → "User sometimes hates X"
        assert_eq!(
            normalize_to_storage_format("Eu às vezes detesto código desorganizado"),
            "User sometimes hates código desorganizado"
        );
    }

    #[test]
    fn test_adverb_static_list_still_works() {
        // Ensure static prefix list is still checked first (adverb regex is a fallback)
        assert_eq!(
            normalize_to_storage_format("I usually prefer dark mode"),
            "User usually prefers dark mode"
        );
        assert_eq!(
            normalize_to_storage_format("I prefer dark mode"),
            "User prefers dark mode"
        );
        assert_eq!(
            normalize_to_storage_format("Eu prefiro dark mode"),
            "User prefers dark mode"
        );
    }

    #[test]
    fn test_adverb_no_match_passthrough() {
        // Patterns that don't match any adverb+verb combo should be unchanged
        assert_eq!(
            normalize_to_storage_format("The project uses Rust"),
            "The project uses Rust"
        );
        assert_eq!(
            normalize_to_storage_format("I think this is fine"),
            "I think this is fine" // "think" is not in our verb list
        );
    }
}
