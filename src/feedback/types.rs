//! Types for the Feedback Signal System
//!
//! Defines the core types: FeedbackSignalType, FeedbackSource, and FeedbackSignal.

/// Type of feedback signal with associated base values (ADR-005)
#[allow(dead_code)] // Consumed by db/decay/prompt (Tasks 4-6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackSignalType {
    /// Positive signal from user or LLM
    Good,
    /// Negative signal from user or LLM
    Bad,
    /// Corrective signal with accompanying text
    Correction,
}

#[allow(dead_code)] // Consumed by db/decay/prompt (Tasks 4-6)
impl FeedbackSignalType {
    /// Returns the base value for this signal type (ADR-005).
    ///
    /// - Good: +1.0
    /// - Bad: -1.0
    /// - Correction: +1.0
    pub fn base_value(&self) -> f32 {
        match self {
            FeedbackSignalType::Good => 1.0,
            FeedbackSignalType::Bad => -1.0,
            FeedbackSignalType::Correction => 1.0,
        }
    }

    /// Returns the string representation for DB serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedbackSignalType::Good => "good",
            FeedbackSignalType::Bad => "bad",
            FeedbackSignalType::Correction => "correction",
        }
    }
}

impl std::fmt::Display for FeedbackSignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for FeedbackSignalType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "good" => Ok(FeedbackSignalType::Good),
            "bad" => Ok(FeedbackSignalType::Bad),
            "correction" => Ok(FeedbackSignalType::Correction),
            _ => Err(format!(
                "Unknown feedback signal type: '{}'. Expected: good, bad, or correction",
                s
            )),
        }
    }
}

/// Source of a feedback signal with associated weight factors (ADR-004)
#[allow(dead_code)] // Consumed by db/decay/prompt (Tasks 4-6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackSource {
    /// Direct user feedback (weight factor 1.0)
    User,
    /// LLM-generated feedback (weight factor 0.3)
    Llm,
}

#[allow(dead_code)] // Consumed by db/decay/prompt (Tasks 4-6)
impl FeedbackSource {
    /// Returns the weight factor for this source (ADR-004).
    ///
    /// - User: 1.0
    /// - Llm: 0.3
    pub fn weight_factor(&self) -> f32 {
        match self {
            FeedbackSource::User => 1.0,
            FeedbackSource::Llm => 0.3,
        }
    }

    /// Returns the string representation for DB serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedbackSource::User => "user",
            FeedbackSource::Llm => "llm",
        }
    }
}

impl std::fmt::Display for FeedbackSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for FeedbackSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(FeedbackSource::User),
            "llm" => Ok(FeedbackSource::Llm),
            _ => Err(format!(
                "Unknown feedback source: '{}'. Expected: user or llm",
                s
            )),
        }
    }
}

/// A feedback signal stored in the feedback system
#[allow(dead_code)] // Consumed by db/decay/prompt (Tasks 4-6)
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackSignal {
    /// ID of the target content item
    pub item_id: i64,
    /// Session ID where feedback was given
    pub session_id: Option<String>,
    /// Type of feedback signal
    pub signal_type: FeedbackSignalType,
    /// Base value of the signal (from signal_type.base_value())
    pub base_value: f32,
    /// Correction text (only for Correction signals)
    pub correction_text: Option<String>,
    /// Source of the feedback
    pub source: FeedbackSource,
    /// Unix timestamp of when feedback was created
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // --- FeedbackSignalType ---

    #[test]
    fn test_signal_type_base_value_good() {
        assert!((FeedbackSignalType::Good.base_value() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_signal_type_base_value_bad() {
        assert!((FeedbackSignalType::Bad.base_value() - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_signal_type_base_value_correction() {
        assert!((FeedbackSignalType::Correction.base_value() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_signal_type_as_str() {
        assert_eq!(FeedbackSignalType::Good.as_str(), "good");
        assert_eq!(FeedbackSignalType::Bad.as_str(), "bad");
        assert_eq!(FeedbackSignalType::Correction.as_str(), "correction");
    }

    #[test]
    fn test_signal_type_display() {
        assert_eq!(FeedbackSignalType::Good.to_string(), "good");
        assert_eq!(FeedbackSignalType::Bad.to_string(), "bad");
        assert_eq!(FeedbackSignalType::Correction.to_string(), "correction");
    }

    #[test]
    fn test_signal_type_from_str_valid() {
        assert_eq!(FeedbackSignalType::from_str("good").unwrap(), FeedbackSignalType::Good);
        assert_eq!(FeedbackSignalType::from_str("bad").unwrap(), FeedbackSignalType::Bad);
        assert_eq!(
            FeedbackSignalType::from_str("correction").unwrap(),
            FeedbackSignalType::Correction
        );
    }

    #[test]
    fn test_signal_type_from_str_case_insensitive() {
        assert_eq!(FeedbackSignalType::from_str("GOOD").unwrap(), FeedbackSignalType::Good);
        assert_eq!(FeedbackSignalType::from_str("Bad").unwrap(), FeedbackSignalType::Bad);
        assert_eq!(
            FeedbackSignalType::from_str("Correction").unwrap(),
            FeedbackSignalType::Correction
        );
    }

    #[test]
    fn test_signal_type_from_str_invalid() {
        let err = FeedbackSignalType::from_str("invalid").unwrap_err();
        assert!(err.contains("Unknown feedback signal type"));
        assert!(err.contains("invalid"));
        assert!(err.contains("good, bad, or correction"));
    }

    // --- FeedbackSource ---

    #[test]
    fn test_source_weight_factor_user() {
        assert!((FeedbackSource::User.weight_factor() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_source_weight_factor_llm() {
        assert!((FeedbackSource::Llm.weight_factor() - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_source_as_str() {
        assert_eq!(FeedbackSource::User.as_str(), "user");
        assert_eq!(FeedbackSource::Llm.as_str(), "llm");
    }

    #[test]
    fn test_source_display() {
        assert_eq!(FeedbackSource::User.to_string(), "user");
        assert_eq!(FeedbackSource::Llm.to_string(), "llm");
    }

    #[test]
    fn test_source_from_str_valid() {
        assert_eq!(FeedbackSource::from_str("user").unwrap(), FeedbackSource::User);
        assert_eq!(FeedbackSource::from_str("llm").unwrap(), FeedbackSource::Llm);
    }

    #[test]
    fn test_source_from_str_case_insensitive() {
        assert_eq!(FeedbackSource::from_str("USER").unwrap(), FeedbackSource::User);
        assert_eq!(FeedbackSource::from_str("LLM").unwrap(), FeedbackSource::Llm);
    }

    #[test]
    fn test_source_from_str_invalid() {
        let err = FeedbackSource::from_str("invalid").unwrap_err();
        assert!(err.contains("Unknown feedback source"));
        assert!(err.contains("invalid"));
        assert!(err.contains("user or llm"));
    }

    // --- FeedbackSignal ---

    #[test]
    fn test_feedback_signal_construction() {
        let signal = FeedbackSignal {
            item_id: 42,
            session_id: Some("sess_abc".to_string()),
            signal_type: FeedbackSignalType::Good,
            base_value: FeedbackSignalType::Good.base_value(),
            correction_text: None,
            source: FeedbackSource::User,
            created_at: 1713600000,
        };

        assert_eq!(signal.item_id, 42);
        assert_eq!(signal.session_id, Some("sess_abc".to_string()));
        assert_eq!(signal.signal_type, FeedbackSignalType::Good);
        assert!((signal.base_value - 1.0).abs() < f32::EPSILON);
        assert!(signal.correction_text.is_none());
        assert_eq!(signal.source, FeedbackSource::User);
        assert_eq!(signal.created_at, 1713600000);
    }

    #[test]
    fn test_feedback_signal_correction_with_text() {
        let signal = FeedbackSignal {
            item_id: 7,
            session_id: None,
            signal_type: FeedbackSignalType::Correction,
            base_value: FeedbackSignalType::Correction.base_value(),
            correction_text: Some("The capital is Canberra, not Sydney".to_string()),
            source: FeedbackSource::Llm,
            created_at: 1713600000,
        };

        assert_eq!(signal.signal_type, FeedbackSignalType::Correction);
        assert!((signal.base_value - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            signal.correction_text,
            Some("The capital is Canberra, not Sydney".to_string())
        );
        assert_eq!(signal.source, FeedbackSource::Llm);
    }

    #[test]
    fn test_feedback_signal_equality() {
        let a = FeedbackSignal {
            item_id: 1,
            session_id: None,
            signal_type: FeedbackSignalType::Bad,
            base_value: -1.0,
            correction_text: None,
            source: FeedbackSource::User,
            created_at: 100,
        };
        let b = FeedbackSignal {
            item_id: 1,
            session_id: None,
            signal_type: FeedbackSignalType::Bad,
            base_value: -1.0,
            correction_text: None,
            source: FeedbackSource::User,
            created_at: 100,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn test_feedback_signal_derives() {
        let signal = FeedbackSignal {
            item_id: 1,
            session_id: None,
            signal_type: FeedbackSignalType::Good,
            base_value: 1.0,
            correction_text: None,
            source: FeedbackSource::User,
            created_at: 0,
        };
        // Clone
        let cloned = signal.clone();
        assert_eq!(signal, cloned);
        // Debug
        let debug_str = format!("{:?}", signal);
        assert!(debug_str.contains("FeedbackSignal"));
    }
}