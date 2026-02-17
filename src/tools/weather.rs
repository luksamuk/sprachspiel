//! Weather tool using Open-Meteo API (free, no API key required)
//!
//! Provides weather information for any location using the Open-Meteo API.
//! Supports current weather, forecasts, and geocoding.

use crate::debug_tools::{log_tool_call, log_tool_result};
use ollama_rs::function;
use serde::Deserialize;

/// Base URL for Open-Meteo API
const OPEN_METEO_BASE: &str = "https://api.open-meteo.com/v1/forecast";
/// Base URL for Open-Meteo Geocoding API
const GEOCODING_BASE: &str = "https://geocoding-api.open-meteo.com/v1/search";

/// Get coordinates for a location name using Open-Meteo geocoding
async fn get_coordinates(
    location: &str,
) -> Result<(f64, f64), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}?name={}&count=1&language=pt&format=json",
        GEOCODING_BASE,
        urlencoding::encode(location)
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("Geocoding API error: {}", response.status()).into());
    }

    let geo_response: GeocodingResponse = response.json().await?;

    if let Some(results) = geo_response.results {
        if let Some(first) = results.first() {
            Ok((first.latitude, first.longitude))
        } else {
            Err(format!("Location '{}' not found", location).into())
        }
    } else {
        Err(format!("Location '{}' not found", location).into())
    }
}

/// Fetch current weather and forecast for a location
#[function]
pub async fn get_weather(
    location: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("get_weather", &[("location".to_string(), location.clone())]);
    
    // First, get coordinates for the location
    let (lat, lon) = match get_coordinates(&location).await {
        Ok(coords) => coords,
        Err(e) => {
            let err = format!("Could not find location '{}': {}", location, e);
            log_tool_result("get_weather", &err);
            return Ok(err);
        }
    };

    // Build Open-Meteo API URL
    let url = format!(
        "{}?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,weather_code,wind_speed_10m,wind_direction_10m&daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto",
        OPEN_METEO_BASE, lat, lon
    );

    // Make the request
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("Weather API error: {}", response.status()).into());
    }

    let weather: WeatherResponse = response.json().await?;

    // Format the response
    let location_name = format!(
        "{}, {}",
        weather
            .timezone
            .split('/')
            .next_back()
            .unwrap_or(&weather.timezone),
        weather.timezone
    );

    let current = &weather.current;
    let daily = &weather.daily;

    let weather_desc = get_weather_description(current.weather_code);
    let wind_dir = get_wind_direction(current.wind_direction_10m as u16);

    let mut forecast = String::new();
    if !daily.time.is_empty() {
        forecast.push_str("\n**Previsão para os próximos dias:**\n");
        for i in 0..daily.time.len().min(3) {
            let date = &daily.time[i];
            let max_temp = daily.temperature_2m_max[i];
            let min_temp = daily.temperature_2m_min[i];
            let precip_prob = daily.precipitation_probability_max[i];

            forecast.push_str(&format!(
                "- {}: Máx {}°C, Mín {}°C, Chuva {}%\n",
                date, max_temp, min_temp, precip_prob
            ));
        }
    }

    let result = format!(
        r#"**Clima em {}**

**Agora:**
- Temperatura: {}°C (sensação térmica: {}°C)
- Condição: {}
- Umidade: {}%
- Vento: {} km/h {}
- Precipitação: {} mm

{}
Fonte: Open-Meteo"#,
        location_name,
        current.temperature_2m,
        current.apparent_temperature,
        weather_desc,
        current.relative_humidity_2m,
        current.wind_speed_10m,
        wind_dir,
        current.precipitation,
        forecast
    );
    
    log_tool_result("get_weather", &result);
    Ok(result)
}

/// Get current weather only (simpler response)
#[function]
pub async fn get_current_weather(
    location: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Get current weather data directly
    let (lat, lon) = match get_coordinates(&location).await {
        Ok(coords) => coords,
        Err(e) => return Ok(format!("Could not find location '{}': {}", location, e)),
    };

    let url = format!(
        "{}?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,weather_code,wind_speed_10m,wind_direction_10m&timezone=auto",
        OPEN_METEO_BASE, lat, lon
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("Weather API error: {}", response.status()).into());
    }

    let weather: WeatherResponse = response.json().await?;
    let current = &weather.current;

    let location_name = format!(
        "{}, {}",
        weather
            .timezone
            .split('/')
            .next_back()
            .unwrap_or(&weather.timezone),
        weather.timezone
    );

    let weather_desc = get_weather_description(current.weather_code);
    let wind_dir = get_wind_direction(current.wind_direction_10m as u16);

    Ok(format!(
        r#"**Clima em {}**

**Agora:**
- Temperatura: {}°C (sensação térmica: {}°C)
- Condição: {}
- Umidade: {}%
- Vento: {} km/h {}
- Precipitação: {} mm

Fonte: Open-Meteo"#,
        location_name,
        current.temperature_2m,
        current.apparent_temperature,
        weather_desc,
        current.relative_humidity_2m,
        current.wind_speed_10m,
        wind_dir,
        current.precipitation
    ))
}

/// Get weather forecast for a location
#[function]
pub async fn get_weather_forecast(
    location: String,
    days: Option<u8>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let days = days.unwrap_or(5).min(7) as usize;

    // First, get coordinates for the location
    let (lat, lon) = match get_coordinates(&location).await {
        Ok(coords) => coords,
        Err(e) => return Ok(format!("Could not find location '{}': {}", location, e)),
    };

    // Build Open-Meteo API URL with extended forecast
    let url = format!(
        "{}?latitude={}&longitude={}&daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max,weather_code&timezone=auto&forecast_days={}",
        OPEN_METEO_BASE, lat, lon, days
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("Weather API error: {}", response.status()).into());
    }

    let weather: WeatherResponse = response.json().await?;
    let daily = &weather.daily;

    let location_name = format!(
        "{}, {}",
        weather
            .timezone
            .split('/')
            .next_back()
            .unwrap_or(&weather.timezone),
        weather.timezone
    );

    let mut forecast_lines = vec![format!(
        "**Previsão do tempo para {} (próximos {} dias):**\n",
        location_name, days
    )];

    for i in 0..daily.time.len() {
        let date = &daily.time[i];
        let max_temp = daily.temperature_2m_max[i];
        let min_temp = daily.temperature_2m_min[i];
        let precip_prob = daily.precipitation_probability_max[i];
        let weather_code = daily.weather_code[i];
        let condition = get_weather_description(weather_code as i32);

        forecast_lines.push(format!(
            "**{}**: Máx {}°C | Mín {}°C | {} | Chuva {}%",
            date, max_temp, min_temp, condition, precip_prob
        ));
    }

    forecast_lines.push("\nFonte: Open-Meteo".to_string());

    Ok(forecast_lines.join("\n"))
}

// Weather code to description mapping
fn get_weather_description(code: i32) -> &'static str {
    match code {
        0 => "Céu limpo",
        1 => "Principalmente limpo",
        2 => "Parcialmente nublado",
        3 => "Nublado",
        45..=48 => "Nevoeiro",
        51 | 53 | 55 => "Garoa",
        56..=57 => "Garoa congelante",
        61 | 63 | 65 => "Chuva",
        66..=67 => "Chuva congelante",
        71 | 73 | 75 => "Neve",
        77 => "Grãos de neve",
        80..=82 => "Chuva forte",
        85..=86 => "Neve forte",
        95 => "Trovoada",
        96 | 99 => "Trovoada com granizo",
        _ => "Desconhecido",
    }
}

// Convert wind direction degrees to cardinal direction
fn get_wind_direction(degrees: u16) -> &'static str {
    let directions = ["N", "NE", "L", "SE", "S", "SO", "O", "NO"];
    let index = ((degrees as f64 + 22.5) / 45.0) as usize % 8;
    directions[index]
}

// Open-Meteo API Response Structures
#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    #[allow(dead_code)]
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    #[allow(dead_code)]
    country: String,
    #[serde(default)]
    #[allow(dead_code)]
    admin1: String, // State/region
}

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    timezone: String,
    current: CurrentWeather,
    daily: DailyForecast,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    #[allow(dead_code)]
    time: String,
    #[serde(rename = "temperature_2m")]
    temperature_2m: f64,
    #[serde(rename = "relative_humidity_2m")]
    relative_humidity_2m: u8,
    #[serde(rename = "apparent_temperature")]
    apparent_temperature: f64,
    precipitation: f64,
    #[serde(rename = "weather_code")]
    weather_code: i32,
    #[serde(rename = "wind_speed_10m")]
    wind_speed_10m: f64,
    #[serde(rename = "wind_direction_10m")]
    wind_direction_10m: u16,
}

#[derive(Debug, Deserialize)]
struct DailyForecast {
    time: Vec<String>,
    #[serde(rename = "temperature_2m_max")]
    temperature_2m_max: Vec<f64>,
    #[serde(rename = "temperature_2m_min")]
    temperature_2m_min: Vec<f64>,
    #[serde(rename = "precipitation_probability_max")]
    precipitation_probability_max: Vec<u8>,
    #[serde(default)]
    weather_code: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_descriptions() {
        assert_eq!(get_weather_description(0), "Céu limpo");
        assert_eq!(get_weather_description(61), "Chuva");
        assert_eq!(get_weather_description(95), "Trovoada");
    }

    #[test]
    fn test_wind_directions() {
        assert_eq!(get_wind_direction(0), "N");
        assert_eq!(get_wind_direction(90), "L");
        assert_eq!(get_wind_direction(180), "S");
        assert_eq!(get_wind_direction(270), "O");
    }
}
