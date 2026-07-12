//! Hardcoded list of well-known embedding model names.
//!
//! Used by `sprach models upgrade` to warn users when a provider's
//! `/v1/models` advertises a known embedding model but the
//! `[provider.X]` block lacks `embedding = true`.
//!
//! **Design (per user policy):**
//! - Detection is opt-in via `sprach models upgrade` — never automatic.
//! - The list is **internal** and never exposed in error messages or
//!   UI to the user. Errors stay strict ("Provider 'X' is not
//!   declared as embedding-capable") and do not list model names.
//! - The list is **substring-matched** (case-insensitive) against model
//!   names returned by `/v1/models`.
//! - The list is **not** exhaustive — users can add new embedding
//!   models by setting `embedding = true` on the relevant provider
//!   in `models.toml`.
//!
//! See [the W2 migration notes](../development/roadmap.md) for context.

/// Well-known embedding model name fragments (case-insensitive substring match).
///
/// Each entry is matched as a substring of a model name returned by
/// `/v1/models`. A model is considered a "potential embedding model"
/// if **any** of these substrings appears (case-insensitively) in its
/// name. Examples:
///
/// - `nomic-embed-text-v2-moe:latest` → matches `"nomic-embed-text-v2-moe"`
/// - `BAAI/bge-small-en-v1.5` → matches `"bge-small-en-v1.5"`
/// - `text-embedding-3-small` → matches `"text-embedding-3-small"`
#[cfg(test)]
const KNOWN_EMBEDDING_MODEL_FRAGMENTS: &[&str] = &[
    // Nomic AI nomic-embed-text family
    "nomic-embed-text",
    // BAAI BGE family (case-insensitive: matches bge-*, BGE-*, Bge-*)
    "bge-",
    "bge_",
    // BAAI GTE family
    "gte-",
    "gte_",
    // OpenAI text-embedding family
    "text-embedding-3",
    "text-embedding-ada",
    // Snowflake Arctic embed
    "snowflake-arctic-embed",
    // mxbai embed
    "mxbai-embed",
    // Qwen3 embedding (if/when available)
    "qwen3-embed",
    "text-embedding",
];

/// Returns `true` if the given model name (e.g. `"nomic-embed-text-v2-moe:latest"`)
/// substring-matches (case-insensitively) any entry in the hardcoded
/// embedding model fragment list.
///
/// This is a **heuristic** currently used only by unit tests. It is
/// reserved for future use by `sprach models upgrade` to suppress
/// the `MissingEmbeddingFlag` warning for providers that don't serve
/// any well-known embedding model. The list is **internal only** —
/// never exposed in user-facing error messages (per user policy:
/// strict, no list in errors).
#[cfg(test)]
#[must_use]
fn is_potential_embedding_model(model_name: &str) -> bool {
    if model_name.is_empty() {
        return false;
    }
    let lower = model_name.to_ascii_lowercase();
    KNOWN_EMBEDDING_MODEL_FRAGMENTS
        .iter()
        .any(|frag| lower.contains(&frag.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nomic_embed_v2_moe_matches() {
        assert!(is_potential_embedding_model("nomic-embed-text-v2-moe"));
        assert!(is_potential_embedding_model(
            "nomic-embed-text-v2-moe:latest"
        ));
    }

    #[test]
    fn test_nomic_embed_v1_5_matches() {
        assert!(is_potential_embedding_model("nomic-embed-text-v1.5"));
        assert!(is_potential_embedding_model("nomic-embed-text-v1.5-fp16"));
    }

    #[test]
    fn test_bge_matches() {
        assert!(is_potential_embedding_model("bge-small-en-v1.5"));
        assert!(is_potential_embedding_model("BAAI/bge-base-en-v1.5"));
        assert!(is_potential_embedding_model("bge-large-en"));
    }

    #[test]
    fn test_gte_matches() {
        assert!(is_potential_embedding_model("gte-small"));
        assert!(is_potential_embedding_model("Alibaba-NLP/gte-large"));
    }

    #[test]
    fn test_openai_text_embedding_matches() {
        assert!(is_potential_embedding_model("text-embedding-3-small"));
        assert!(is_potential_embedding_model("text-embedding-3-large"));
        assert!(is_potential_embedding_model("text-embedding-ada-002"));
    }

    #[test]
    fn test_chat_models_do_not_match() {
        assert!(!is_potential_embedding_model("llama3.1:8b"));
        assert!(!is_potential_embedding_model("qwen3.5:4b"));
        assert!(!is_potential_embedding_model("gemma4-e2b:think"));
        assert!(!is_potential_embedding_model("mistral:7b"));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(is_potential_embedding_model("NOMIC-EMBED-TEXT-V2-MOE"));
        assert!(is_potential_embedding_model("BGE-Small-En"));
        assert!(is_potential_embedding_model("Text-Embedding-3-Small"));
    }

    #[test]
    fn test_empty_string_returns_false() {
        assert!(!is_potential_embedding_model(""));
    }

    #[test]
    fn test_substring_match_not_exact() {
        // "text-embedding" is a fragment; any model containing it matches.
        assert!(is_potential_embedding_model("custom-text-embedding-model"));
    }
}
