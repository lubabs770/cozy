//! cozy's TOML config (`~/.config/cozy/config.toml`), currently just the bits
//! the weather integration needs: the OpenWeatherMap API key, where to fetch
//! weather for, the unit system, and the daemon poll interval.
//!
//! The key is a secret, so it lives here rather than on the command line (where
//! it would leak into shell history / process listings).

use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::weather::owm::Location;

/// Default daemon poll interval (seconds) when the config omits `interval`.
const DEFAULT_INTERVAL: u64 = 600;

/// Parsed, validated cozy configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// OpenWeatherMap API key.
    pub api_key: String,
    /// Where to fetch weather for.
    pub location: Location,
    /// OWM unit system (`metric` keeps wind in m/s).
    pub units: String,
    /// Daemon weather poll interval, in seconds.
    pub interval: u64,
}

/// The on-disk shape, before validation. Mirrors the TOML keys directly so
/// serde does the parsing; [`Config::from_toml`] turns it into a validated
/// [`Config`].
#[derive(Debug, Deserialize)]
struct RawConfig {
    api_key: Option<String>,
    location: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    units: Option<String>,
    interval: Option<u64>,
}

impl Config {
    /// Parse and validate config from a TOML string.
    ///
    /// `api_key` is required, and exactly one of `location` or (`lat` + `lon`)
    /// must be given. `units` defaults to `metric`, `interval` to 600s.
    pub fn from_toml(s: &str) -> Result<Config> {
        let raw: RawConfig = toml::from_str(s).context("parse cozy config TOML")?;

        let api_key = raw
            .api_key
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| anyhow!("config: `api_key` is required"))?;

        let location = match (raw.location, raw.lat, raw.lon) {
            (Some(city), None, None) => Location::City(city),
            (None, Some(lat), Some(lon)) => Location::Coords { lat, lon },
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                return Err(anyhow!(
                    "config: set either `location` or `lat`/`lon`, not both"
                ));
            }
            (None, None, _) | (None, _, None) => {
                return Err(anyhow!(
                    "config: a `location` (city) or both `lat` and `lon` are required"
                ));
            }
        };

        Ok(Config {
            api_key,
            location,
            units: raw.units.unwrap_or_else(|| "metric".to_string()),
            interval: raw.interval.unwrap_or(DEFAULT_INTERVAL),
        })
    }

    /// Load and validate the config from `path`.
    pub fn load(path: &Path) -> Result<Config> {
        let s = fs::read_to_string(path)
            .with_context(|| format!("read cozy config {}", path.display()))?;
        Config::from_toml(&s)
    }
}

/// Default config path: `$XDG_CONFIG_HOME/cozy/config.toml`, falling back to
/// `~/.config/cozy/config.toml`.
pub fn config_path() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("cozy").join("config.toml");
    }
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".config").join("cozy").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_city_config_with_defaults() {
        let cfg = Config::from_toml(
            r#"
            api_key = "abc123"
            location = "London,GB"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.api_key, "abc123");
        assert!(matches!(cfg.location, Location::City(ref c) if c == "London,GB"));
        assert_eq!(cfg.units, "metric");
        assert_eq!(cfg.interval, 600);
    }

    #[test]
    fn parses_latlon_config_and_overrides() {
        let cfg = Config::from_toml(
            r#"
            api_key = "k"
            lat = 51.51
            lon = -0.13
            units = "imperial"
            interval = 300
            "#,
        )
        .unwrap();
        assert!(matches!(
            cfg.location,
            Location::Coords { lat, lon } if (lat - 51.51).abs() < 1e-9 && (lon + 0.13).abs() < 1e-9
        ));
        assert_eq!(cfg.units, "imperial");
        assert_eq!(cfg.interval, 300);
    }

    #[test]
    fn missing_api_key_is_an_error() {
        let err = Config::from_toml("location = \"Paris\"").unwrap_err();
        assert!(err.to_string().contains("api_key"));
    }

    #[test]
    fn empty_api_key_is_an_error() {
        assert!(Config::from_toml("api_key = \"\"\nlocation = \"Paris\"").is_err());
    }

    #[test]
    fn missing_location_is_an_error() {
        assert!(Config::from_toml("api_key = \"k\"").is_err());
    }

    #[test]
    fn both_city_and_latlon_is_an_error() {
        let s = r#"
            api_key = "k"
            location = "Paris"
            lat = 48.85
            lon = 2.35
        "#;
        assert!(Config::from_toml(s).is_err());
    }

    #[test]
    fn partial_coords_is_an_error() {
        assert!(Config::from_toml("api_key = \"k\"\nlat = 48.85").is_err());
    }
}
