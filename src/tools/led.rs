//! LED control tools for Raspberry Pi Pico W NeoPixel server
//!
//! Provides tools to control NeoPixel LED strips via HTTP REST API.
//! Requires configuration in config.toml:
//!
//! ```toml
//! [led]
//! ip = "192.168.1.100"  # Required for LED tools
//! port = 80             # Optional, default: 80
//! ```

use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::normalize_input;
use sprachspiel_tool_derive::tool;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::RwLock;

/// Global LED endpoint (set from settings at startup)
static LED_ENDPOINT: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

/// Set the LED endpoint from settings
pub fn set_led_endpoint(endpoint: Option<String>) {
    if let Ok(mut guard) = LED_ENDPOINT.write() {
        *guard = endpoint;
    }
}

/// Check if LED is configured (using settings)
#[derive(Debug, Deserialize, Default)]
struct LedStatus {
    /// Whether the LEDs are on (true) or off (false)
    blinking: bool,
    /// Current program: 0=Christmas, 1=Trail, 2=Lamp
    program: u8,
    /// Brightness level (0.02 to 1.0)
    dim: f32,
    /// Current color as hex string (e.g., "ffa648")
    color: String,
}

impl LedStatus {
    /// Parse color hex string to RGB values
    fn to_rgb(&self) -> (u8, u8, u8) {
        if self.color.len() == 6 {
            let r = u8::from_str_radix(&self.color[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&self.color[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&self.color[4..6], 16).unwrap_or(0);
            (r, g, b)
        } else {
            (0, 0, 0)
        }
    }

    fn program_name(&self) -> &'static str {
        match self.program {
            0 => "Christmas",
            1 => "Trail",
            2 => "Lamp",
            _ => "Unknown",
        }
    }
}

/// Get the LED endpoint, returning an error message if not configured
fn get_endpoint() -> Result<String, String> {
    if let Ok(guard) = LED_ENDPOINT.read() {
        guard.clone().ok_or_else(|| {
            "LED tools not configured. Add [led] ip = \"<IP>\" to config.toml".to_string()
        })
    } else {
        Err("LED configuration error".to_string())
    }
}

/// Make an HTTP request to the LED device
async fn led_request(endpoint: &str, path: &str) -> Result<LedStatus, String> {
    let url = format!("{}{}", endpoint, path);
    let client = reqwest::Client::new();

    let response = match client
        .request(
            if path.starts_with("/led/on")
                || path.starts_with("/led/off")
                || path.starts_with("/led/toggle")
                || path.starts_with("/led/dim")
                || path.starts_with("/led/color")
                || path.starts_with("/led/program")
                || path.starts_with("/led/change")
            {
                reqwest::Method::POST
            } else {
                reqwest::Method::GET
            },
            &url,
        )
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(format!(
                "Could not connect to LED device. Please check if the device is powered on and connected to the network. Error: {}",
                e
            ));
        }
    };

    if !response.status().is_success() {
        return Err(format!(
            "LED device returned error: {}. Please check the device status.",
            response.status()
        ));
    }

    match response.json::<LedStatus>().await {
        Ok(status) => Ok(status),
        Err(e) => Err(format!("Failed to parse LED response: {}", e)),
    }
}

/// Format LED status for display
fn format_status(status: &LedStatus) -> String {
    let (r, g, b) = status.to_rgb();
    format!(
        "**LED Status**\n\
         Power: {}\n\
         Program: {} ({})\n\
         Brightness: {:.2}\n\
         Color: #{} (R: {}, G: {}, B: {})",
        if status.blinking { "ON" } else { "OFF" },
        status.program_name(),
        status.program,
        status.dim,
        status.color,
        r,
        g,
        b
    )
}

/// Parse program string to program number
fn parse_program(program: &str) -> Result<u8, String> {
    let program_lower = normalize_input(program);
    match program_lower.as_str() {
        "0" | "christmas" => Ok(0),
        "1" | "trail" => Ok(1),
        "2" | "lamp" => Ok(2),
        "next" | "cycle" => Ok(255), // Special value for cycle
        _ => Err(format!(
            "Invalid program '{}'. Use 0 (Christmas), 1 (Trail), 2 (Lamp), or 'next'.",
            program
        )),
    }
}

/// Parse brightness string to f32 (0.02 to 1.0)
fn parse_brightness(value: &str) -> Result<f32, String> {
    let brightness: f32 = value
        .trim()
        .parse()
        .map_err(|_| format!("Invalid brightness '{}'. Must be a number.", value))?;

    if !(0.02..=1.0).contains(&brightness) {
        Err(format!(
            "Brightness {:.2} out of range. Must be between 0.02 and 1.0.",
            brightness
        ))
    } else {
        Ok(brightness)
    }
}

/// Parse color component (0-255)
fn parse_color_component(value: &str, name: &str) -> Result<u8, String> {
    let num: u16 = value
        .trim()
        .parse()
        .map_err(|_| format!("Invalid {} value '{}'. Must be a number.", name, value))?;

    if num > 255 {
        Err(format!(
            "{} value {} out of range. Must be between 0 and 255.",
            name, num
        ))
    } else {
        Ok(num as u8)
    }
}

/// Parse hex color string
fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8), String> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return Err(format!(
            "Invalid hex color '{}'. Must be 6 characters (e.g., ff5500).",
            hex
        ));
    }

    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| format!("Invalid hex color '{}'. Use characters 0-9 and a-f.", hex))?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| format!("Invalid hex color '{}'. Use characters 0-9 and a-f.", hex))?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| format!("Invalid hex color '{}'. Use characters 0-9 and a-f.", hex))?;

    Ok((r, g, b))
}

/// Get the current status of the LED strip.
///
/// Returns power state, current program, brightness level, and color.
/// Use this to check the current state before making adjustments.
///
/// # Arguments
/// No arguments required.
///
/// # Returns
/// Formatted status including:
/// - Power: ON or OFF
/// - Program: Christmas (0), Trail (1), or Lamp (2)
/// - Brightness: 0.02 to 1.0
/// - Color: Both hex (#ffa648) and RGB components
///
/// # Example
/// ```ignore
/// led_get_status()
/// ```
#[tool]
pub async fn led_get_status() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("led_get_status", &[]);

    let endpoint = match get_endpoint() {
        Ok(e) => e,
        Err(e) => {
            log_tool_result("led_get_status", &e);
            return Ok(e);
        }
    };

    match led_request(&endpoint, "/led").await {
        Ok(status) => {
            let result = format_status(&status);
            log_tool_result("led_get_status", &result);
            Ok(result)
        }
        Err(e) => {
            log_tool_result("led_get_status", &e);
            Ok(e)
        }
    }
}

/// Turn the LED strip on, off, or toggle the power state.
///
/// # Arguments
/// * `action` - Power action to perform: "on", "off", or "toggle"
///
/// # Returns
/// Updated LED status after the action.
///
/// # Example
/// ```ignore
/// led_set_power(action: "on")      // Turn on
/// led_set_power(action: "off")     // Turn off
/// led_set_power(action: "toggle")  // Toggle state
/// ```
#[tool]
pub async fn led_set_power(
    action: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("led_set_power", &[("action".to_string(), action.clone())]);

    let endpoint = match get_endpoint() {
        Ok(e) => e,
        Err(e) => {
            log_tool_result("led_set_power", &e);
            return Ok(e);
        }
    };

    let action_lower = normalize_input(&action);
    let path = match action_lower.as_str() {
        "on" => "/led/on",
        "off" => "/led/off",
        "toggle" => "/led/toggle",
        _ => {
            let err = format!("Invalid action '{}'. Use 'on', 'off', or 'toggle'.", action);
            log_tool_result("led_set_power", &err);
            return Ok(err);
        }
    };

    match led_request(&endpoint, path).await {
        Ok(status) => {
            let result = format_status(&status);
            log_tool_result("led_set_power", &result);
            Ok(result)
        }
        Err(e) => {
            log_tool_result("led_set_power", &e);
            Ok(e)
        }
    }
}

/// Set the LED program mode.
///
/// Programs control how the LEDs behave:
/// - 0 or "christmas": Color cycling pattern
/// - 1 or "trail": Back-and-forth trail effect
/// - 2 or "lamp": Static color (uses brightness and color settings)
/// - "next" or "cycle": Advance to the next program
///
/// # Arguments
/// * `program` - Program to set: "0", "1", "2", "christmas", "trail", "lamp", or "next"
///
/// # Returns
/// Updated LED status after changing the program.
///
/// # Example
/// ```ignore
/// led_set_program(program: "lamp")      // Set to lamp mode
/// led_set_program(program: "2")         // Set to lamp mode (numeric)
/// led_set_program(program: "next")      // Cycle to next program
/// ```
#[tool]
pub async fn led_set_program(
    program: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "led_set_program",
        &[("program".to_string(), program.clone())],
    );

    let endpoint = match get_endpoint() {
        Ok(e) => e,
        Err(e) => {
            log_tool_result("led_set_program", &e);
            return Ok(e);
        }
    };

    let prog = match parse_program(&program) {
        Ok(p) => p,
        Err(e) => {
            log_tool_result("led_set_program", &e);
            return Ok(e);
        }
    };

    let path = if prog == 255 {
        "/led/change"
    } else {
        &format!("/led/program/{}", prog)
    };

    match led_request(&endpoint, path).await {
        Ok(status) => {
            let result = format_status(&status);
            log_tool_result("led_set_program", &result);
            Ok(result)
        }
        Err(e) => {
            log_tool_result("led_set_program", &e);
            Ok(e)
        }
    }
}

/// Set the LED brightness level.
///
/// Brightness affects all programs and is applied after color calculations.
/// Lower values produce dimmer light.
///
/// # Arguments
/// * `brightness` - Brightness level as a string: "0.02" to "1.0"
///   - "0.02" = Very dim (minimum)
///   - "0.5" = 50% brightness
///   - "1.0" = Full brightness
///
/// # Returns
/// Updated LED status after changing brightness.
///
/// # Example
/// ```ignore
/// led_set_brightness(brightness: "0.5")   # 50% brightness
/// led_set_brightness(brightness: "1.0")   # Full brightness
/// led_set_brightness(brightness: "0.1")   # 10% brightness (dim)
/// ```
#[tool]
pub async fn led_set_brightness(
    brightness: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "led_set_brightness",
        &[("brightness".to_string(), brightness.clone())],
    );

    let endpoint = match get_endpoint() {
        Ok(e) => e,
        Err(e) => {
            log_tool_result("led_set_brightness", &e);
            return Ok(e);
        }
    };

    let level = match parse_brightness(&brightness) {
        Ok(l) => l,
        Err(e) => {
            log_tool_result("led_set_brightness", &e);
            return Ok(e);
        }
    };

    let path = format!("/led/dim/{:.4}", level);

    match led_request(&endpoint, &path).await {
        Ok(status) => {
            let result = format_status(&status);
            log_tool_result("led_set_brightness", &result);
            Ok(result)
        }
        Err(e) => {
            log_tool_result("led_set_brightness", &e);
            Ok(e)
        }
    }
}

/// Set the LED color for Lamp mode.
///
/// The color setting is used when the program is set to "lamp" (mode 2).
/// You can specify color using either hex format or separate RGB values.
///
/// # Arguments
/// * `hex` - Optional: Color in hex format (e.g., "ff5500" or "#ff5500")
/// * `r` - Optional: Red component (0-255) as string
/// * `g` - Optional: Green component (0-255) as string
/// * `b` - Optional: Blue component (0-255) as string
///
/// If hex is provided, it takes precedence over RGB values.
/// If RGB values are provided, all three must be specified.
///
/// # Returns
/// Updated LED status after changing color.
///
/// # Example
/// ```ignore
/// // Using hex color
/// led_set_color(hex: "ff5500")                    // Orange
/// led_set_color(hex: "#00ff00")                   // Green (with # prefix)
///
/// // Using RGB values (easier for calculations)
/// led_set_color(r: "255", g: "85", b: "0")        // Orange
/// led_set_color(r: "255", g: "255", b: "255")     // White
///
/// // After getting current status, adjust values:
/// // Current: r=255, g=100, b=50
/// // To make more red: increase R or decrease G/B
/// led_set_color(r: "255", g: "50", b: "0")        // More red-orange
/// ```
#[tool]
pub async fn led_set_color(
    hex: Option<String>,
    r: Option<String>,
    g: Option<String>,
    b: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "led_set_color",
        &[
            ("hex".to_string(), hex.clone().unwrap_or_default()),
            ("r".to_string(), r.clone().unwrap_or_default()),
            ("g".to_string(), g.clone().unwrap_or_default()),
            ("b".to_string(), b.clone().unwrap_or_default()),
        ],
    );

    let endpoint = match get_endpoint() {
        Ok(e) => e,
        Err(e) => {
            log_tool_result("led_set_color", &e);
            return Ok(e);
        }
    };

    let (red, green, blue) = if let Some(hex_val) = hex {
        if !hex_val.trim().is_empty() {
            match parse_hex_color(&hex_val) {
                Ok(rgb) => rgb,
                Err(e) => {
                    log_tool_result("led_set_color", &e);
                    return Ok(e);
                }
            }
        } else {
            // Empty hex string, try RGB
            match (r, g, b) {
                (Some(r_val), Some(g_val), Some(b_val)) => {
                    let red = match parse_color_component(&r_val, "Red") {
                        Ok(v) => v,
                        Err(e) => {
                            log_tool_result("led_set_color", &e);
                            return Ok(e);
                        }
                    };
                    let green = match parse_color_component(&g_val, "Green") {
                        Ok(v) => v,
                        Err(e) => {
                            log_tool_result("led_set_color", &e);
                            return Ok(e);
                        }
                    };
                    let blue = match parse_color_component(&b_val, "Blue") {
                        Ok(v) => v,
                        Err(e) => {
                            log_tool_result("led_set_color", &e);
                            return Ok(e);
                        }
                    };
                    (red, green, blue)
                }
                _ => {
                    let err = "Either hex color or all RGB values (r, g, b) must be provided.";
                    log_tool_result("led_set_color", err);
                    return Ok(err.to_string());
                }
            }
        }
    } else {
        // No hex, use RGB
        match (r, g, b) {
            (Some(r_val), Some(g_val), Some(b_val)) => {
                let red = match parse_color_component(&r_val, "Red") {
                    Ok(v) => v,
                    Err(e) => {
                        log_tool_result("led_set_color", &e);
                        return Ok(e);
                    }
                };
                let green = match parse_color_component(&g_val, "Green") {
                    Ok(v) => v,
                    Err(e) => {
                        log_tool_result("led_set_color", &e);
                        return Ok(e);
                    }
                };
                let blue = match parse_color_component(&b_val, "Blue") {
                    Ok(v) => v,
                    Err(e) => {
                        log_tool_result("led_set_color", &e);
                        return Ok(e);
                    }
                };
                (red, green, blue)
            }
            _ => {
                let err = "Either hex color or all RGB values (r, g, b) must be provided.";
                log_tool_result("led_set_color", err);
                return Ok(err.to_string());
            }
        }
    };

    let color_hex = format!("{:02x}{:02x}{:02x}", red, green, blue);
    let path = format!("/led/color/{}", color_hex);

    match led_request(&endpoint, &path).await {
        Ok(status) => {
            let result = format_status(&status);
            log_tool_result("led_set_color", &result);
            Ok(result)
        }
        Err(e) => {
            log_tool_result("led_set_color", &e);
            Ok(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_brightness() {
        assert!(parse_brightness("0.5").is_ok());
        assert!(parse_brightness("1.0").is_ok());
        assert!(parse_brightness("0.02").is_ok());
        assert!(parse_brightness("0.01").is_err()); // Too low
        assert!(parse_brightness("1.1").is_err()); // Too high
    }

    #[test]
    fn test_parse_program() {
        assert_eq!(parse_program("0").unwrap(), 0);
        assert_eq!(parse_program("1").unwrap(), 1);
        assert_eq!(parse_program("2").unwrap(), 2);
        assert_eq!(parse_program("christmas").unwrap(), 0);
        assert_eq!(parse_program("trail").unwrap(), 1);
        assert_eq!(parse_program("lamp").unwrap(), 2);
        assert_eq!(parse_program("next").unwrap(), 255);
        assert!(parse_program("invalid").is_err());
    }

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_hex_color("ff5500").unwrap(), (255, 85, 0));
        assert_eq!(parse_hex_color("#00ff00").unwrap(), (0, 255, 0));
        assert!(parse_hex_color("invalid").is_err());
        assert!(parse_hex_color("fff").is_err()); // Too short
    }

    #[test]
    fn test_parse_color_component() {
        assert_eq!(parse_color_component("255", "Red").unwrap(), 255);
        assert_eq!(parse_color_component("0", "Green").unwrap(), 0);
        assert_eq!(parse_color_component("128", "Blue").unwrap(), 128);
        assert!(parse_color_component("256", "Red").is_err()); // Out of range
        assert!(parse_color_component("abc", "Green").is_err()); // Not a number
    }
}
