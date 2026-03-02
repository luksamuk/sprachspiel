//! Calculator tool using ollama-rs built-in implementation
//!
//! Provides mathematical expression evaluation using the `calc` crate.
//! Supports basic arithmetic, exponents, percentages, and mathematical functions.

use crate::debug_tools::{log_tool_call, log_tool_result};
use ollama_rs::function;

/// Evaluate a mathematical expression.
///
/// Performs mathematical calculations and returns the result.
/// Use this tool for any arithmetic or mathematical computation.
///
/// # Arguments
/// * `expression` - The mathematical expression to evaluate.
///   - Basic arithmetic: "2 + 3 * 4", "100 / 5"
///   - Exponents: "2 ** 10", "5 ^ 3"
///   - Percentages: "15% of 850", "20% of 1500"
///   - Functions: "sqrt(144)", "sin(3.14)", "log(100)"
///   - Parentheses: "(2 + 3) * 4"
///
/// # Returns
/// The numeric result of the expression, formatted appropriately.
///
/// # Errors
/// Returns error message for invalid expressions, syntax errors, or math errors.
#[function]
pub async fn calculate(
    expression: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "calculate",
        &[("expression".to_string(), expression.clone())],
    );

    // Use the calc crate directly for expression evaluation
    let result = match eval_expression(&expression) {
        Ok(value) => format_number(value),
        Err(e) => format!("Error: {}.", e),
    };

    log_tool_result("calculate", &result);
    Ok(result)
}

/// Evaluate a mathematical expression using the calc crate.
fn eval_expression(expr: &str) -> Result<f64, String> {
    // Handle percentage expressions like "15% of 850"
    if let Some(result) = parse_percent_of(expr) {
        return Ok(result);
    }

    // Use calc crate for evaluation
    let mut ctx = calc::Context::default();
    ctx.evaluate(expr)
        .map_err(|e| format!("Calculation error: {}", e))
}

/// Parse expressions like "15% of 850" or "50% OF 200"
fn parse_percent_of(expr: &str) -> Option<f64> {
    let expr_lower = expr.to_lowercase();
    let parts: Vec<&str> = expr_lower.split("% of ").collect();

    if parts.len() == 2 {
        let percent: f64 = parts[0].trim().parse().ok()?;
        let value: f64 = parts[1].trim().parse().ok()?;
        return Some(value * percent / 100.0);
    }

    None
}

/// Format a number, removing unnecessary decimal places
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        format!("{}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(eval_expression("2 + 2").unwrap(), 4.0);
        assert_eq!(eval_expression("10 - 3").unwrap(), 7.0);
        assert_eq!(eval_expression("4 * 5").unwrap(), 20.0);
        assert_eq!(eval_expression("20 / 4").unwrap(), 5.0);
    }

    #[test]
    fn test_exponents() {
        assert_eq!(eval_expression("2 ** 8").unwrap(), 256.0);
    }

    #[test]
    fn test_percentage() {
        assert_eq!(eval_expression("15% of 850").unwrap(), 127.5);
        assert_eq!(eval_expression("50% of 200").unwrap(), 100.0);
    }

    #[test]
    fn test_functions() {
        assert_eq!(eval_expression("sqrt(144)").unwrap(), 12.0);
    }

    #[test]
    fn test_complex_expressions() {
        assert_eq!(eval_expression("(100 + 50) * 2").unwrap(), 300.0);
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(4.0), "4");
        assert_eq!(format_number(std::f64::consts::PI), std::f64::consts::PI.to_string());
    }
}
