use std::time::Duration;

use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct CurrentWeather {
    pub temperature_celsius: f32,
    pub weather_code: u8,
    pub is_day: bool,
}

#[derive(Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Deserialize)]
struct GeocodingResult {
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: ForecastCurrent,
}

#[derive(Deserialize)]
struct ForecastCurrent {
    temperature_2m: f32,
    weather_code: u8,
    is_day: u8,
}

pub fn fetch_current(city: &str, language: &str) -> Result<CurrentWeather, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let encoded_city = encode_query_value(city);
    let geocoding_url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={encoded_city}&count=1&language={language}&format=json"
    );
    let geocoding: GeocodingResponse = agent
        .get(&geocoding_url)
        .call()
        .map_err(|error| error.to_string())?
        .into_json()
        .map_err(|error| error.to_string())?;
    let location = geocoding
        .results
        .and_then(|results| results.into_iter().next())
        .ok_or_else(|| match language {
            "en" => format!("No city found for \"{city}\"."),
            "es" => format!("No se encontró la ciudad « {city} »."),
            _ => format!("Aucune ville trouvée pour « {city} »."),
        })?;

    let forecast_url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,is_day&timezone=auto",
        location.latitude, location.longitude
    );
    let forecast: ForecastResponse = agent
        .get(&forecast_url)
        .call()
        .map_err(|error| error.to_string())?
        .into_json()
        .map_err(|error| error.to_string())?;

    Ok(CurrentWeather {
        temperature_celsius: forecast.current.temperature_2m,
        weather_code: forecast.current.weather_code,
        is_day: forecast.current.is_day != 0,
    })
}

fn encode_query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
