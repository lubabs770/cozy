//! OpenWeatherMap *Current Weather* client: build the request, fetch over HTTPS,
//! and parse the JSON into the typed [`Observation`] that [`mapping`] consumes.
//!
//! Only the handful of fields cozy uses are modelled; everything else in the
//! (large) OWM payload is ignored. The network fetch is a thin wrapper so the
//! interesting part — turning JSON into an [`Observation`] — stays pure and
//! unit-testable via [`parse`].
//!
//! [`mapping`]: super::mapping

use anyhow::{Context, Result};
use serde::Deserialize;

use super::mapping::Observation;

const ENDPOINT: &str = "https://api.openweathermap.org/data/2.5/weather";

/// Where to fetch weather for: a city query (`q=`) or explicit coordinates.
#[derive(Debug, Clone)]
pub enum Location {
    /// A free-form place query, e.g. `"London,GB"`.
    City(String),
    /// Explicit latitude/longitude.
    Coords { lat: f64, lon: f64 },
}

/// Raw OWM response, narrowed to the fields cozy reads.
#[derive(Debug, Deserialize)]
struct OwmResponse {
    weather: Vec<OwmCondition>,
    #[serde(default)]
    wind: OwmWind,
    #[serde(default)]
    rain: Option<OwmPrecip>,
    #[serde(default)]
    snow: Option<OwmPrecip>,
    #[serde(default)]
    clouds: OwmClouds,
}

#[derive(Debug, Deserialize)]
struct OwmCondition {
    id: u16,
}

#[derive(Debug, Default, Deserialize)]
struct OwmWind {
    #[serde(default)]
    speed: f32,
}

#[derive(Debug, Default, Deserialize)]
struct OwmClouds {
    #[serde(default)]
    all: u8,
}

/// `rain` / `snow` objects: OWM reports volume under a `"1h"` (and sometimes
/// `"3h"`) key. We only need the 1-hour figure.
#[derive(Debug, Default, Deserialize)]
struct OwmPrecip {
    #[serde(rename = "1h", default)]
    one_h: f32,
}

/// Parse an OWM *Current Weather* JSON body into an [`Observation`].
///
/// Absent `rain`/`snow` objects (the common dry case) map to 0 mm/h. An empty
/// `weather` array is an error — without a condition code there is nothing to
/// map.
pub fn parse(body: &str) -> Result<Observation> {
    let resp: OwmResponse =
        serde_json::from_str(body).context("parse OpenWeatherMap response JSON")?;
    let condition_id = resp
        .weather
        .first()
        .context("OpenWeatherMap response had no weather conditions")?
        .id;
    Ok(Observation {
        condition_id,
        wind_mps: resp.wind.speed,
        rain_mmh: resp.rain.map(|p| p.one_h).unwrap_or(0.0),
        snow_mmh: resp.snow.map(|p| p.one_h).unwrap_or(0.0),
        clouds_pct: resp.clouds.all,
    })
}

/// Fetch the current weather for `location` from OWM and parse it.
///
/// Blocking HTTPS GET via `ureq`. `units` is passed through (`metric` keeps
/// `wind.speed` in m/s, matching [`mapping`]'s assumptions).
///
/// [`mapping`]: super::mapping
pub fn fetch(api_key: &str, location: &Location, units: &str) -> Result<Observation> {
    let mut req = ureq::get(ENDPOINT)
        .query("appid", api_key)
        .query("units", units);
    req = match location {
        Location::City(q) => req.query("q", q),
        Location::Coords { lat, lon } => req
            .query("lat", &lat.to_string())
            .query("lon", &lon.to_string()),
    };
    let body = req
        .call()
        .context("OpenWeatherMap request failed")?
        .into_string()
        .context("read OpenWeatherMap response body")?;
    parse(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A trimmed but realistic OWM Current Weather payload: rainy and windy.
    const RAIN_SAMPLE: &str = r#"{
        "coord": {"lon": -0.13, "lat": 51.51},
        "weather": [{"id": 500, "main": "Rain", "description": "light rain"}],
        "main": {"temp": 9.2, "humidity": 88},
        "wind": {"speed": 6.0, "deg": 200},
        "rain": {"1h": 4.0},
        "clouds": {"all": 90},
        "name": "London"
    }"#;

    // Clear-sky payload with no rain/snow objects at all (the dry common case).
    const CLEAR_SAMPLE: &str = r#"{
        "weather": [{"id": 800, "main": "Clear", "description": "clear sky"}],
        "main": {"temp": 24.0},
        "wind": {"speed": 1.5},
        "clouds": {"all": 0},
        "name": "Madrid"
    }"#;

    // Snow payload: snow volume under "1h", no rain object.
    const SNOW_SAMPLE: &str = r#"{
        "weather": [{"id": 601, "main": "Snow", "description": "snow"}],
        "wind": {"speed": 3.0},
        "snow": {"1h": 2.0},
        "clouds": {"all": 95}
    }"#;

    #[test]
    fn parses_rain_sample() {
        let o = parse(RAIN_SAMPLE).unwrap();
        assert_eq!(o.condition_id, 500);
        assert_eq!(o.wind_mps, 6.0);
        assert_eq!(o.rain_mmh, 4.0);
        assert_eq!(o.snow_mmh, 0.0);
        assert_eq!(o.clouds_pct, 90);
    }

    #[test]
    fn parses_clear_sample_without_precip_objects() {
        let o = parse(CLEAR_SAMPLE).unwrap();
        assert_eq!(o.condition_id, 800);
        assert_eq!(o.wind_mps, 1.5);
        assert_eq!(o.rain_mmh, 0.0);
        assert_eq!(o.snow_mmh, 0.0);
        assert_eq!(o.clouds_pct, 0);
    }

    #[test]
    fn parses_snow_sample() {
        let o = parse(SNOW_SAMPLE).unwrap();
        assert_eq!(o.condition_id, 601);
        assert_eq!(o.snow_mmh, 2.0);
        assert_eq!(o.rain_mmh, 0.0);
    }

    #[test]
    fn empty_weather_array_is_an_error() {
        let body = r#"{"weather": [], "wind": {"speed": 1.0}, "clouds": {"all": 0}}"#;
        assert!(parse(body).is_err());
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse("not json").is_err());
    }
}
