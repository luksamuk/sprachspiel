//! Prompt builder for TranslateGemma
//!
//! Formats prompts according to TranslateGemma's expected format:
//! <https://ollama.com/library/translategemma>

use super::language::LanguageCode;
use super::style::TranslationStyle;

/// Build a translation prompt for TranslateGemma
///
/// Format when source is specified:
/// ```text
/// You are a professional {SOURCE_LANG} ({SOURCE_CODE}) to {TARGET_LANG} ({TARGET_CODE}) translator...
/// {STYLE_INSTRUCTION}
/// Produce only the {TARGET_LANG} translation...
///
///
/// {TEXT}
/// ```
///
/// Format when source is not specified (auto-detect):
/// ```text
/// You are a professional translator. First identify the language of the provided text,
/// then translate it into {TARGET_LANG} ({TARGET_CODE})...
/// {STYLE_INSTRUCTION}
/// Produce only the translation...
///
///
/// {TEXT}
/// ```
pub fn build_translation_prompt(
    source: Option<&LanguageCode>,
    target: &LanguageCode,
    text: &str,
    style: Option<&TranslationStyle>,
) -> String {
    let mut prompt = String::new();

    if let Some(src) = source {
        // Source language is known
        prompt.push_str(&format!(
            "You are a professional {} ({}) to {} ({}) translator. Your goal is to accurately convey the meaning and nuances of the original {} text while adhering to {} grammar, vocabulary, and cultural sensitivities.",
            src.name, src.code, target.name, target.code, src.name, target.name
        ));
    } else {
        // Auto-detect source language
        prompt.push_str(&format!(
            "You are a professional translator. First identify the language of the provided text, then translate it into {} ({}). Your goal is to accurately convey the meaning and nuances while adhering to {} grammar, vocabulary, and cultural sensitivities.",
            target.name, target.code, target.name
        ));
    }

    // Add style instruction if provided
    if let Some(s) = style {
        prompt.push('\n');
        prompt.push_str(s.to_instruction());
    }

    // Closing statement
    prompt.push('\n');
    prompt.push_str(&format!(
        "Produce only the {} translation, without any additional explanations or commentary.",
        target.name
    ));

    if source.is_none() {
        prompt.push_str(" If the text is already in the target language, return it unchanged.");
    }

    prompt.push_str(&format!(
        " Please translate the following text into {}:\n\n\n{}",
        target.name, text
    ));

    prompt
}

/// Build a simple translation prompt without all the ceremony
/// Used for internal testing or simpler use cases
#[allow(dead_code)]
pub fn build_simple_prompt(source: Option<&str>, target: &str, text: &str) -> String {
    let mut prompt = String::new();

    if let Some(src) = source {
        prompt.push_str(&format!(
            "Translate from {} to {}:\n\n{}",
            src, target, text
        ));
    } else {
        prompt.push_str(&format!(
            "Detect the language and translate to {}:\n\n{}",
            target, text
        ));
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::super::language::LanguageMapper;
    use super::*;

    #[test]
    fn test_prompt_with_source() {
        let mapper = LanguageMapper::new();
        let en = mapper.resolve("en").unwrap();
        let pt = mapper.resolve("pt-BR").unwrap();

        let prompt = build_translation_prompt(Some(&en), &pt, "Hello world", None);

        assert!(prompt.contains("English"));
        assert!(prompt.contains("pt-BR"));
        assert!(prompt.contains("Portuguese (Brazil)"));
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("Produce only the Portuguese (Brazil) translation"));
    }

    #[test]
    fn test_prompt_without_source() {
        let mapper = LanguageMapper::new();
        let pt = mapper.resolve("pt-BR").unwrap();

        let prompt = build_translation_prompt(None, &pt, "Hello world", None);

        assert!(prompt.contains("First identify the language"));
        assert!(prompt.contains("Portuguese (Brazil)"));
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("If the text is already in the target language"));
    }

    #[test]
    fn test_prompt_with_style() {
        let mapper = LanguageMapper::new();
        let en = mapper.resolve("en").unwrap();
        let pt = mapper.resolve("pt-BR").unwrap();
        let style = TranslationStyle::Formal;

        let prompt = build_translation_prompt(Some(&en), &pt, "Hello", Some(&style));

        assert!(prompt.contains("formal"));
        assert!(prompt.contains("professional"));
        assert!(prompt.contains("Hello"));
    }

    #[test]
    fn test_prompt_with_custom_style() {
        let mapper = LanguageMapper::new();
        let en = mapper.resolve("en").unwrap();
        let pt = mapper.resolve("pt-BR").unwrap();
        let style = TranslationStyle::Custom("use very formal academic language".to_string());

        let prompt = build_translation_prompt(Some(&en), &pt, "Hello", Some(&style));

        assert!(prompt.contains("use very formal academic language"));
    }

    #[test]
    fn test_simple_prompt() {
        let prompt = build_simple_prompt(Some("English"), "Portuguese", "Hello");
        assert!(prompt.contains("English"));
        assert!(prompt.contains("Portuguese"));
        assert!(prompt.contains("Hello"));
    }
}
