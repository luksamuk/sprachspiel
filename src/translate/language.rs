//! Language code handling for translation
//!
//! Supports 55 languages with regional variants as defined by TranslateGemma.
//! Includes corrections for known inconsistencies and user-friendly aliases.

use std::collections::HashMap;

/// Error type for language-related operations
#[derive(Debug, Clone)]
pub enum LanguageError {
    Unknown(String),
    Ambiguous(String, Vec<String>),
}

impl std::fmt::Display for LanguageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanguageError::Unknown(lang) => write!(f, "Unknown language: '{}'", lang),
            LanguageError::Ambiguous(lang, variants) => {
                write!(
                    f,
                    "'{}' is ambiguous. Options: {}",
                    lang,
                    variants.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for LanguageError {}

/// Language code with metadata
#[derive(Debug, Clone)]
pub struct LanguageCode {
    pub code: String,
    pub name: String,
    pub aliases: Vec<&'static str>,
}

impl LanguageCode {
    fn new(code: &str, name: &str, aliases: Vec<&'static str>) -> Self {
        Self {
            code: code.to_string(),
            name: name.to_string(),
            aliases,
        }
    }
}

/// Maps user inputs to canonical language codes
pub struct LanguageMapper {
    codes: HashMap<String, LanguageCode>,
    alias_map: HashMap<String, String>,
    corrections: HashMap<String, String>,
    ambiguous: HashMap<String, Vec<String>>,
}

impl LanguageMapper {
    /// Create a new LanguageMapper with all TranslateGemma languages
    pub fn new() -> Self {
        let mut mapper = Self {
            codes: HashMap::new(),
            alias_map: HashMap::new(),
            corrections: HashMap::new(),
            ambiguous: HashMap::new(),
        };

        mapper.init_languages();
        mapper.init_corrections();
        mapper.init_ambiguous();
        mapper.init_aliases();

        mapper
    }

    /// Resolve user input to a canonical language code
    pub fn resolve(&self, input: &str) -> Result<LanguageCode, LanguageError> {
        let input_lower = input.to_lowercase().trim().to_string();

        if input_lower.is_empty() {
            return Err(LanguageError::Unknown(String::new()));
        }

        // Apply corrections first (zh-CN -> zh-Hans, etc.)
        let corrected = self
            .corrections
            .get(&input_lower)
            .map(|s| s.as_str())
            .unwrap_or(&input_lower);

        // Check if this is an ambiguous term
        if let Some(variants) = self.ambiguous.get(corrected) {
            return Err(LanguageError::Ambiguous(
                input.to_string(),
                variants.clone(),
            ));
        }

        // Try exact code match
        if let Some(code) = self.codes.get(corrected) {
            return Ok(code.clone());
        }

        // Try alias match
        if let Some(canonical_code) = self.alias_map.get(corrected) {
            if let Some(code) = self.codes.get(canonical_code) {
                return Ok(code.clone());
            }
        }

        Err(LanguageError::Unknown(input.to_string()))
    }

    /// List all supported languages, optionally filtered by substring
    pub fn list(&self, filter: Option<&str>) -> Vec<&LanguageCode> {
        let mut result: Vec<&LanguageCode> = self.codes.values().collect();

        if let Some(f) = filter {
            let filter_lower = f.to_lowercase();
            result.retain(|code| {
                code.code.to_lowercase().contains(&filter_lower)
                    || code.name.to_lowercase().contains(&filter_lower)
                    || code
                        .aliases
                        .iter()
                        .any(|a| a.to_lowercase().contains(&filter_lower))
            });
        }

        // Sort by code name
        result.sort_by(|a, b| a.code.cmp(&b.code));
        result
    }

    fn init_languages(&mut self) {
        // Common European languages
        self.add_language("en", "English", vec!["english", "ingles"]);
        self.add_language("en-US", "English (United States)", vec![]);
        self.add_language(
            "en-GB",
            "English (United Kingdom)",
            vec!["british", "uk", "british english"],
        );
        self.add_language("es", "Spanish", vec!["spanish", "espanol", "español"]);
        self.add_language("es-ES", "Spanish (Spain)", vec![]);
        self.add_language("es-MX", "Spanish (Mexico)", vec![]);
        self.add_language("fr", "French", vec!["french", "frances", "français"]);
        self.add_language("fr-FR", "French (France)", vec![]);
        self.add_language("fr-CA", "French (Canada)", vec![]);
        self.add_language(
            "de",
            "German",
            vec!["german", "alemao", "alemão", "deutsch"],
        );
        self.add_language("de-DE", "German (Germany)", vec![]);
        self.add_language("it", "Italian", vec!["italian", "italiano"]);
        self.add_language(
            "pt",
            "Portuguese",
            vec!["portuguese", "portugues", "português"],
        );
        self.add_language(
            "pt-BR",
            "Portuguese (Brazil)",
            vec![
                "br",
                "brazil",
                "brazilian",
                "pt-br",
                "brasileiro",
                "pt-brasil",
            ],
        );
        self.add_language("pt-PT", "Portuguese (Portugal)", vec![]);
        self.add_language("nl", "Dutch", vec!["dutch", "holland", "holandes"]);
        self.add_language("ru", "Russian", vec!["russian", "russo"]);
        self.add_language("pl", "Polish", vec!["polish", "polones", "polaco"]);
        self.add_language("tr", "Turkish", vec!["turkish", "turco"]);
        self.add_language("cs", "Czech", vec!["czech", "tcheco", "checo"]);
        self.add_language("el", "Greek", vec!["greek", "grego", "grek"]);
        self.add_language("el-GR", "Greek (Greece)", vec![]);

        // Nordic
        self.add_language("sv", "Swedish", vec!["swedish", "sueco"]);
        self.add_language("da", "Danish", vec!["danish", "dinamarques"]);
        self.add_language("no", "Norwegian", vec!["norwegian", "noruegues"]);
        self.add_language("fi", "Finnish", vec!["finnish", "finlandes"]);

        // Asian languages - Chinese variants
        self.add_language("zh", "Chinese", vec![]); // Will be ambiguous
        self.add_language(
            "zh-Hans",
            "Chinese Simplified",
            vec!["zh-cn", "chinese simplified", "simplified chinese", "cn"],
        );
        self.add_language(
            "zh-Hant",
            "Chinese Traditional",
            vec![
                "zh-tw",
                "chinese traditional",
                "traditional chinese",
                "tw",
                "taiwan",
            ],
        );
        self.add_language("zh-Hans-HK", "Chinese Simplified (Hong Kong)", vec![]);
        self.add_language("zh-Hant-HK", "Chinese Traditional (Hong Kong)", vec![]);

        // Japanese and Korean
        self.add_language("ja", "Japanese", vec!["japanese", "japones", "nihongo"]);
        self.add_language("ko", "Korean", vec!["korean", "coreano"]);

        // South Asian
        self.add_language("hi", "Hindi", vec!["hindi"]);
        self.add_language("hi-Latn", "Hindi (Latin script)", vec![]);
        self.add_language("bn", "Bengali", vec!["bengali", "bengali"]);
        self.add_language("ta", "Tamil", vec!["tamil"]);
        self.add_language("te", "Telugu", vec!["telugu"]);
        self.add_language("mr", "Marathi", vec!["marathi"]);
        self.add_language("ur", "Urdu", vec!["urdu"]);

        // Middle Eastern
        self.add_language("ar", "Arabic", vec!["arabic", "arabe", "árabe"]);
        self.add_language("he", "Hebrew", vec!["hebrew", "hebraico", "iw"]);
        self.add_language("he-IL", "Hebrew (Israel)", vec![]);
        self.add_language("fa", "Persian", vec!["persian", "farsi", "persa"]);
        self.add_language("fa-IR", "Persian (Iran)", vec![]);

        // Southeast Asian
        self.add_language("vi", "Vietnamese", vec!["vietnamese", "vietnamita"]);
        self.add_language("th", "Thai", vec!["thai", "tailandes"]);
        self.add_language("id", "Indonesian", vec!["indonesian", "indonesio"]);
        self.add_language("ms", "Malay", vec!["malay", "malaio"]);
        self.add_language("fil", "Filipino", vec!["filipino", "tagalog"]);
        self.add_language("tl", "Tagalog", vec![]);

        // Eastern European
        self.add_language("uk", "Ukrainian", vec!["ukrainian", "ucraniano"]);
        self.add_language("ro", "Romanian", vec!["romanian", "romeno"]);
        self.add_language("hu", "Hungarian", vec!["hungarian", "hungaro"]);
        self.add_language("bg", "Bulgarian", vec!["bulgarian", "bulgaro"]);
        self.add_language("sr", "Serbian", vec!["serbian", "servio"]);
        self.add_language("sr-Latn", "Serbian (Latin)", vec![]);
        self.add_language("sr-Cyrl", "Serbian (Cyrillic)", vec![]);
        self.add_language("hr", "Croatian", vec!["croatian", "croata"]);
        self.add_language("sk", "Slovak", vec!["slovak", "eslovaco"]);

        // Other European
        self.add_language("ca", "Catalan", vec!["catalan", "catalão"]);
        self.add_language("eu", "Basque", vec!["basque", "vasco"]);
        self.add_language("ga", "Irish", vec!["irish", "irlandes"]);
        self.add_language("cy", "Welsh", vec!["welsh", "gales"]);

        // Slavic
        self.add_language("be", "Belarusian", vec!["belarusian", "bielorusso"]);
        self.add_language("mk", "Macedonian", vec!["macedonian", "macedonio"]);

        // African and other
        self.add_language("af", "Afrikaans", vec!["afrikaans"]);
        self.add_language("sw", "Swahili", vec!["swahili"]);
        self.add_language("am", "Amharic", vec!["amharic", "amárico"]);
        self.add_language("ne", "Nepali", vec!["nepali"]);
        self.add_language("si", "Sinhala", vec!["sinhala", "cingales"]);

        // Additional common codes from TranslateGemma list
        self.add_language("az", "Azerbaijani", vec![]);
        self.add_language("hy", "Armenian", vec![]);
        self.add_language("ka", "Georgian", vec![]);
        self.add_language("km", "Khmer", vec![]);
        self.add_language("lo", "Lao", vec![]);
        self.add_language("my", "Burmese", vec![]);
        self.add_language("pa", "Punjabi", vec![]);
        self.add_language("gu", "Gujarati", vec![]);
        self.add_language("kn", "Kannada", vec![]);
        self.add_language("ml", "Malayalam", vec![]);
        self.add_language("or", "Odia", vec![]);
        self.add_language("as", "Assamese", vec![]);
        self.add_language("sa", "Sanskrit", vec![]);
    }

    fn add_language(&mut self, code: &str, name: &str, aliases: Vec<&'static str>) {
        let lang = LanguageCode::new(code, name, aliases.clone());
        self.codes.insert(code.to_lowercase(), lang);
    }

    fn init_corrections(&mut self) {
        // Corrections for known inconsistencies in TranslateGemma
        // zh-CH (incorrect) -> zh-Hans (correct Simplified Chinese)
        self.corrections
            .insert("zh-ch".to_string(), "zh-hans".to_string());
        self.corrections
            .insert("zh-cn".to_string(), "zh-hans".to_string());

        // Deprecated codes -> current codes
        self.corrections.insert("iw".to_string(), "he".to_string());
        self.corrections.insert("ji".to_string(), "yi".to_string());
        self.corrections.insert("in".to_string(), "id".to_string());

        // Common mistypes
        self.corrections
            .insert("chinese".to_string(), "zh".to_string()); // Will hit ambiguous
        self.corrections
            .insert("portuguese".to_string(), "pt".to_string());
        self.corrections
            .insert("pt-br".to_string(), "pt-br".to_string()); // Already correct
        self.corrections
            .insert("pt-brasil".to_string(), "pt-br".to_string());

        // Normalize codes with dashes
        self.corrections
            .insert("zhhans".to_string(), "zh-hans".to_string());
        self.corrections
            .insert("zhhant".to_string(), "zh-hant".to_string());
        self.corrections
            .insert("ptbr".to_string(), "pt-br".to_string());
        self.corrections
            .insert("ptpt".to_string(), "pt-pt".to_string());
        self.corrections
            .insert("enus".to_string(), "en-us".to_string());
        self.corrections
            .insert("engb".to_string(), "en-gb".to_string());
        self.corrections
            .insert("eses".to_string(), "es-es".to_string());
        self.corrections
            .insert("esmx".to_string(), "es-mx".to_string());
        self.corrections
            .insert("frfr".to_string(), "fr-fr".to_string());
        self.corrections
            .insert("frca".to_string(), "fr-ca".to_string());
        self.corrections
            .insert("dede".to_string(), "de-de".to_string());
    }

    fn init_ambiguous(&mut self) {
        // Terms that require disambiguation
        self.ambiguous.insert(
            "chinese".to_string(),
            vec![
                "zh-Hans (Simplified - China mainland)".to_string(),
                "zh-Hant (Traditional - Taiwan/HK)".to_string(),
            ],
        );
        self.ambiguous.insert(
            "zh".to_string(),
            vec![
                "zh-Hans (Simplified)".to_string(),
                "zh-Hant (Traditional)".to_string(),
            ],
        );
        self.ambiguous.insert(
            "pt".to_string(),
            vec!["pt-BR (Brazil)".to_string(), "pt-PT (Portugal)".to_string()],
        );
        self.ambiguous.insert(
            "portuguese".to_string(),
            vec!["pt-BR (Brazil)".to_string(), "pt-PT (Portugal)".to_string()],
        );
        self.ambiguous.insert(
            "en".to_string(),
            vec![
                "en-US (American)".to_string(),
                "en-GB (British)".to_string(),
            ],
        );
        self.ambiguous.insert(
            "english".to_string(),
            vec![
                "en-US (American)".to_string(),
                "en-GB (British)".to_string(),
            ],
        );
    }

    fn init_aliases(&mut self) {
        // Populate alias_map from the aliases stored in each LanguageCode
        for (code, lang) in &self.codes {
            for alias in &lang.aliases {
                self.alias_map.insert(alias.to_lowercase(), code.clone());
            }
        }
    }
}

impl Default for LanguageMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a language pair string like "en:pt" or ":pt" or just "pt"
pub fn parse_language_pair(
    input: &str,
    mapper: &LanguageMapper,
) -> Result<(Option<LanguageCode>, LanguageCode), LanguageError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(LanguageError::Unknown(String::new()));
    }

    if trimmed.contains(':') {
        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
        let source_str = parts[0].trim();
        let target_str = parts[1].trim();

        if target_str.is_empty() {
            return Err(LanguageError::Unknown(
                "No target language specified".to_string(),
            ));
        }

        let source = if source_str.is_empty() {
            None
        } else {
            Some(mapper.resolve(source_str)?)
        };

        let target = mapper.resolve(target_str)?;
        Ok((source, target))
    } else {
        // No colon - just target language
        let target = mapper.resolve(trimmed)?;
        Ok((None, target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_exact_code() {
        let mapper = LanguageMapper::new();
        assert!(mapper.resolve("en").is_ok());
        assert!(mapper.resolve("pt-BR").is_ok());
        assert!(mapper.resolve("zh-Hans").is_ok());
    }

    #[test]
    fn test_resolve_aliases() {
        let mapper = LanguageMapper::new();

        // Test Brazilian Portuguese aliases
        let br = mapper.resolve("br").unwrap();
        assert_eq!(br.code, "pt-BR");

        let brazil = mapper.resolve("brazil").unwrap();
        assert_eq!(brazil.code, "pt-BR");

        // Test Hebrew
        let he = mapper.resolve("hebrew").unwrap();
        assert_eq!(he.code, "he");

        // Test deprecated iw -> he mapping
        let iw = mapper.resolve("iw").unwrap();
        assert_eq!(iw.code, "he");
    }

    #[test]
    fn test_corrections() {
        let mapper = LanguageMapper::new();

        // zh-CN should map to zh-Hans
        let cn = mapper.resolve("zh-cn").unwrap();
        assert_eq!(cn.code, "zh-Hans");
    }

    #[test]
    fn test_ambiguous() {
        let mapper = LanguageMapper::new();

        // "chinese" should be ambiguous
        match mapper.resolve("chinese") {
            Err(LanguageError::Ambiguous(_, _)) => (),
            _ => panic!("Expected ambiguous error for 'chinese'"),
        }
    }

    #[test]
    fn test_parse_language_pair() {
        let mapper = LanguageMapper::new();

        // en:pt
        let (src, tgt) = parse_language_pair("en:pt", &mapper).unwrap();
        assert!(src.is_some());
        assert_eq!(src.as_ref().unwrap().code, "en");
        assert_eq!(tgt.code, "pt");

        // :pt (auto-detect source)
        let (src, tgt) = parse_language_pair(":pt", &mapper).unwrap();
        assert!(src.is_none());
        assert_eq!(tgt.code, "pt");

        // just "pt"
        let (src, tgt) = parse_language_pair("pt", &mapper).unwrap();
        assert!(src.is_none());
        assert_eq!(tgt.code, "pt");
    }

    #[test]
    fn test_list_languages() {
        let mapper = LanguageMapper::new();

        // All languages
        let all = mapper.list(None);
        assert!(!all.is_empty());

        // Filter by "pt"
        let pt = mapper.list(Some("pt"));
        assert!(pt.iter().any(|l| l.code == "pt"));
        assert!(pt.iter().any(|l| l.code == "pt-BR"));

        // Filter by "port"
        let port = mapper.list(Some("port"));
        assert!(port.iter().any(|l| l.code == "pt"));
    }

    #[test]
    fn test_unknown_language() {
        let mapper = LanguageMapper::new();

        match mapper.resolve("klingon") {
            Err(LanguageError::Unknown(_)) => (),
            _ => panic!("Expected unknown error for 'klingon'"),
        }
    }
}
