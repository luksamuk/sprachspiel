//! Feedback tools for the LLM to submit feedback on conversation messages.
//!
//! Provides the `feedback_submit` tool which allows the LLM to rate messages
//! as good, bad, or provide corrections. Feedback is weighted at 30% of user
//! feedback (configurable via `llm_feedback_weight`) and adjusts message importance.

use std::str::FromStr;

use crate::db::feedback_ops::insert_feedback_signal;
use crate::debug_tools::{RESET, TOOL_DIM, log_tool_call, log_tool_result};
use crate::feedback::types::{FeedbackSignalType, FeedbackSource};
use crate::spinner::suspend_for_print;
use crate::tools::context::{get_db, get_settings};

/// Submit feedback on a message from the LLM's perspective.
///
/// Allows the LLM to provide feedback on messages it generates or receives.
/// Feedback is weighted at 30% (configurable via llm_feedback_weight) relative to user feedback.
///
/// # Arguments
/// * `item_id` - The content item ID to give feedback on. String type for LLM compatibility. Required.
///   - Accepts: numeric string like "42"
///   - Example: "42"
/// * `signal_type` - Type of feedback: "good", "bad", or "correction". Required.
///   - "good" - Positive feedback on the message
///   - "bad" - Negative feedback on the message
///   - "correction" - Corrective feedback (requires correction_text)
/// * `correction_text` - Correction text when signal_type is "correction". Optional.
///   - Normalize empty strings to None with `.filter(|s| !s.is_empty())`
///
/// # Returns
/// Success message with item ID and signal type, or helpful error message.
/// Returns Ok(String) always — never Err() (AGENTS.md tool error handling).
///
/// # Example
/// ```ignore
/// feedback_submit("42", "good", None)
/// feedback_submit("15", "bad", None)
/// feedback_submit("7", "correction", Some("The capital is Canberra, not Sydney"))
/// ```
#[ollama_rs::function]
pub async fn feedback_submit(
    item_id: String,
    signal_type: String,
    correction_text: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None — LLMs may send "" instead of omitting
    let correction_text = correction_text.filter(|s| !s.is_empty());

    log_tool_call(
        "feedback_submit",
        &[
            ("item_id".to_string(), item_id.clone()),
            ("signal_type".to_string(), signal_type.clone()),
            (
                "correction_text".to_string(),
                correction_text.as_deref().unwrap_or("None").to_string(),
            ),
        ],
    );

    // Parse item_id from String (AGENTS.md: LLMs send strings for numeric params)
    let item_id_parsed: i64 = match item_id.trim().parse() {
        Ok(n) => n,
        Err(_) => {
            let err = format!(
                "Error: Invalid item_id '{}'. Must be a positive integer. \
                 Use remember(query=\"...\") to find message IDs.",
                item_id
            );
            log_tool_result("feedback_submit", &err);
            return Ok(err);
        }
    };

    if item_id_parsed <= 0 {
        let err = "Error: item_id must be a positive integer. \
                   Use remember(query=\"...\") to find message IDs."
            .to_string();
        log_tool_result("feedback_submit", &err);
        return Ok(err);
    }

    // Parse signal_type
    let parsed_signal = match FeedbackSignalType::from_str(&signal_type) {
        Ok(s) => s,
        Err(e) => {
            let err = format!("Error: {}. Use 'good', 'bad', or 'correction'.", e);
            log_tool_result("feedback_submit", &err);
            return Ok(err);
        }
    };

    // Validate correction_text requirement
    if parsed_signal == FeedbackSignalType::Correction && correction_text.is_none() {
        let err = "Error: correction_text is required when signal_type is 'correction'. \
                   Provide the corrected text (e.g., feedback_submit(\"42\", \"correction\", Some(\"The correct answer is...\")))"
            .to_string();
        log_tool_result("feedback_submit", &err);
        return Ok(err);
    }

    // Get database — anonymous sessions have no DB
    let db = match get_db() {
        Some(d) => d,
        None => {
            let err = "Error: Feedback storage not available.\n\n\
                        This can happen if:\n\
                        1. You're in an anonymous session (--anonymous flag)\n\
                        2. The database is not initialized\n\n\
                        Start a regular chat session to submit feedback.";
            log_tool_result("feedback_submit", err);
            return Ok(err.to_string());
        }
    };

    // Get settings — check if feedback system is enabled
    let settings = match get_settings() {
        Some(s) => s,
        None => {
            let err = "Error: Settings not available. Cannot verify feedback configuration.";
            log_tool_result("feedback_submit", err);
            return Ok(err.to_string());
        }
    };

    if !settings.feedback.enabled {
        let err = "Error: Feedback system is disabled. Enable it in config.toml with \
                   [feedback] enabled = true.";
        log_tool_result("feedback_submit", err);
        return Ok(err.to_string());
    }

    // Get LLM feedback weight from settings
    let llm_weight = settings.feedback.llm_feedback_weight;
    let base_value = parsed_signal.base_value();

    // Insert feedback signal via with_connection
    // insert_feedback_signal returns Result<i64, String>, but with_connection expects
    // Result<T, rusqlite::Error>, so we bridge with ToSqlConversionFailure
    let source = FeedbackSource::Llm;
    let correction_text_ref = correction_text.as_deref();

    let insert_result = db.with_connection(|conn| {
        insert_feedback_signal(
            conn,
            item_id_parsed,
            None, // session_id — not available in tool context
            parsed_signal,
            base_value,
            correction_text_ref,
            source,
            chrono::Utc::now().timestamp(),
        )
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e))))
    });

    let signal_id = match insert_result {
        Ok(id) => id,
        Err(e) => {
            // with_connection wraps our String error as rusqlite::Error
            // Try to extract meaningful message
            let err_msg = format!(
                "Error: Could not submit feedback for item {}. {}",
                item_id_parsed,
                e.to_string()
                    .replace("error returned from ToSqlConversionFailure: ", "")
            );
            log_tool_result("feedback_submit", &err_msg);
            return Ok(err_msg);
        }
    };

    // Adjust importance based on signal type
    // Good feedback: importance + 0.05, Bad feedback: importance - 0.1, Correction: no change
    let importance_delta = match parsed_signal {
        FeedbackSignalType::Good => 0.05,
        FeedbackSignalType::Bad => -0.1,
        FeedbackSignalType::Correction => 0.0,
    };

    if importance_delta != 0.0
        && let Err(e) = db.adjust_importance(item_id_parsed, importance_delta)
    {
        // Log warning but don't fail — the feedback signal was still recorded
        eprintln!(
            "Warning: Failed to adjust importance for item {}: {}",
            item_id_parsed, e
        );
    }

    // Visual indicator for feedback submission
    let emoji = match parsed_signal {
        FeedbackSignalType::Good => "👍",
        FeedbackSignalType::Bad => "👎",
        FeedbackSignalType::Correction => "✎",
    };
    suspend_for_print(|| {
        eprintln!(
            "{TOOL_DIM}{} feedback submitted (msg:{}, weight: {:.0}%){RESET}",
            emoji,
            item_id_parsed,
            llm_weight * 100.0
        );
    });

    // Build success message
    let signal_label = match parsed_signal {
        FeedbackSignalType::Good => "good",
        FeedbackSignalType::Bad => "bad",
        FeedbackSignalType::Correction => "correction",
    };

    let mut result = format!(
        "Feedback submitted: {} signal for item {} (weight: {:.0}%)\n\n\
         Signal ID: {}\n\
         Type: {}\n\
         Source: llm (weight factor: {})",
        signal_label,
        item_id_parsed,
        llm_weight * 100.0,
        signal_id,
        signal_label,
        source.weight_factor()
    );

    if let Some(ref text) = correction_text {
        result.push_str(&format!("\nCorrection: {}", text));
    }

    if importance_delta != 0.0 {
        result.push_str(&format!(
            "\nImportance adjustment: {:+.2}",
            importance_delta
        ));
    }

    log_tool_result("feedback_submit", &result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_type_parsing() {
        // Valid values
        assert_eq!(
            FeedbackSignalType::from_str("good").unwrap(),
            FeedbackSignalType::Good
        );
        assert_eq!(
            FeedbackSignalType::from_str("bad").unwrap(),
            FeedbackSignalType::Bad
        );
        assert_eq!(
            FeedbackSignalType::from_str("correction").unwrap(),
            FeedbackSignalType::Correction
        );

        // Case insensitive
        assert_eq!(
            FeedbackSignalType::from_str("GOOD").unwrap(),
            FeedbackSignalType::Good
        );
        assert_eq!(
            FeedbackSignalType::from_str("Bad").unwrap(),
            FeedbackSignalType::Bad
        );
    }

    #[test]
    fn test_signal_type_base_values() {
        assert!((FeedbackSignalType::Good.base_value() - 1.0).abs() < f32::EPSILON);
        assert!((FeedbackSignalType::Bad.base_value() - (-1.0)).abs() < f32::EPSILON);
        assert!((FeedbackSignalType::Correction.base_value() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_source_weight() {
        assert!((FeedbackSource::Llm.weight_factor() - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_invalid_signal_type() {
        let err = FeedbackSignalType::from_str("invalid").unwrap_err();
        assert!(err.contains("Unknown feedback signal type"));
    }

    #[test]
    fn test_importance_deltas() {
        // Good: +0.05
        let good_delta: f32 = match FeedbackSignalType::Good {
            FeedbackSignalType::Good => 0.05,
            FeedbackSignalType::Bad => -0.1,
            FeedbackSignalType::Correction => 0.0,
        };
        assert!((good_delta - 0.05_f32).abs() < f32::EPSILON);

        // Bad: -0.1
        let bad_delta: f32 = match FeedbackSignalType::Bad {
            FeedbackSignalType::Good => 0.05,
            FeedbackSignalType::Bad => -0.1,
            FeedbackSignalType::Correction => 0.0,
        };
        assert!((bad_delta - (-0.1_f32)).abs() < f32::EPSILON);

        // Correction: 0.0
        let correction_delta: f32 = match FeedbackSignalType::Correction {
            FeedbackSignalType::Good => 0.05,
            FeedbackSignalType::Bad => -0.1,
            FeedbackSignalType::Correction => 0.0,
        };
        assert!((correction_delta - 0.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn test_correction_text_normalization() {
        // Some("") should become None after normalization
        let empty: Option<String> = Some("".to_string());
        assert!(empty.filter(|s| !s.is_empty()).is_none());

        // Some("text") should remain Some
        let with_text: Option<String> = Some("The correct answer is...".to_string());
        assert!(with_text.filter(|s| !s.is_empty()).is_some());

        // None should remain None
        let none: Option<String> = None;
        assert!(none.filter(|s| !s.is_empty()).is_none());
    }

    #[test]
    fn test_item_id_parsing() {
        // Valid numeric IDs
        assert_eq!("42".trim().parse::<i64>(), Ok(42));
        assert_eq!("  7  ".trim().parse::<i64>(), Ok(7));

        // Invalid IDs
        assert!("abc".trim().parse::<i64>().is_err());
        assert!("-1".trim().parse::<i64>().is_ok()); // Parse succeeds, but we check <= 0
    }
}
