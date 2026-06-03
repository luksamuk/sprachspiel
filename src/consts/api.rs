//! API URL constants
//!
//! Provides centralized constants for external API URLs to prevent
//! duplication and make updates easier.

// Open-Meteo Weather API
pub const OPEN_METEO_BASE: &str = "https://api.open-meteo.com/v1/forecast";
pub const OPEN_METEO_GEOCODING: &str = "https://geocoding-api.open-meteo.com/v1/search";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_urls_are_valid() {
        assert!(OPEN_METEO_BASE.starts_with("https://"));
        assert!(OPEN_METEO_GEOCODING.starts_with("https://"));
    }
}
