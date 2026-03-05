//! API URL constants
//!
//! Provides centralized constants for external API URLs to prevent
//! duplication and make updates easier.

// Open-Meteo Weather API
pub const OPEN_METEO_BASE: &str = "https://api.open-meteo.com/v1/forecast";
pub const OPEN_METEO_GEOCODING: &str = "https://geocoding-api.open-meteo.com/v1/search";

// Serper (Google Search) API
pub const SERPER_API_URL: &str = "https://google.serper.dev/search";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_urls_are_valid() {
        assert!(OPEN_METEO_BASE.starts_with("https://"));
        assert!(OPEN_METEO_GEOCODING.starts_with("https://"));
        assert!(SERPER_API_URL.starts_with("https://"));
    }
}
