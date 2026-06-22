//! Pure mapping from a parsed weather [`Observation`] to the shader knobs
//! cozy actually drives: which effect to show, plus `wind` and `precip`.
//!
//! This module is deliberately free of I/O so it can be unit-tested without a
//! network or a running daemon — the whole "weather drives the shaders"
//! decision table lives here in one place, easy to read and tweak.

/// A weather reading reduced to just the fields cozy cares about, parsed from
/// the OpenWeatherMap *Current Weather* response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observation {
    /// OWM condition code (`weather[0].id`): 2xx thunderstorm, 3xx drizzle,
    /// 5xx rain, 6xx snow, 7xx atmosphere, 800 clear, 80x clouds.
    pub condition_id: u16,
    /// Wind speed in m/s (`wind.speed`, metric units).
    pub wind_mps: f32,
    /// Rain volume mm/h (`rain.1h`), 0 when absent.
    pub rain_mmh: f32,
    /// Snow volume mm/h (`snow.1h`), 0 when absent.
    pub snow_mmh: f32,
    /// Cloudiness percentage (`clouds.all`), 0..100.
    pub clouds_pct: u8,
}

/// The shader knobs derived from an [`Observation`]: the effect to select and
/// the two live weather parameters cozy already understands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherState {
    /// Name of the effect to switch to (a key in the effect registry).
    pub effect: &'static str,
    /// Horizontal wind skew shared by every effect.
    pub wind: f32,
    /// Precipitation / coverage intensity, 0..1.
    pub precip: f32,
}

/// Map a weather observation to cozy's shader knobs.
///
/// `wind` is derived from wind speed for every condition; `effect` and `precip`
/// come from the condition group (see the README / design spec for the table).
pub fn map(obs: &Observation) -> WeatherState {
    // Wind is shared by every effect: m/s normalised so a stiff ~12 m/s breeze
    // reaches the shaders' nominal 1.0, with a little headroom and a floor at 0.
    let wind = (obs.wind_mps / 12.0).clamp(0.0, 1.2);

    // mm/h of precipitation → 0..1; ~8 mm/h (heavy rain) saturates.
    let rain = (obs.rain_mmh / 8.0).clamp(0.0, 1.0);
    let snow = (obs.snow_mmh / 8.0).clamp(0.0, 1.0);
    let cloud = (obs.clouds_pct as f32 / 100.0).clamp(0.0, 1.0);

    let (effect, precip) = match obs.condition_id {
        200..=299 => ("lightning", (0.8 + rain).clamp(0.0, 1.0)),
        300..=399 | 500..=599 => ("droplet", rain),
        600..=699 => ("snow", snow),
        700..=799 => ("stratus", cloud),
        800 => ("sunrays", 0.2),
        801 => ("cirrus", cloud),
        802 | 803 => ("cumulus", cloud),
        804 => ("stratus", cloud),
        // Unknown code: a calm, neutral overcast rather than panicking.
        _ => ("stratus", cloud),
    };

    WeatherState {
        effect,
        wind,
        precip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(condition_id: u16) -> Observation {
        Observation {
            condition_id,
            wind_mps: 0.0,
            rain_mmh: 0.0,
            snow_mmh: 0.0,
            clouds_pct: 0,
        }
    }

    #[test]
    fn thunderstorm_selects_lightning() {
        assert_eq!(map(&obs(211)).effect, "lightning");
    }

    #[test]
    fn drizzle_selects_droplet() {
        assert_eq!(map(&obs(301)).effect, "droplet");
    }

    #[test]
    fn rain_selects_droplet() {
        assert_eq!(map(&obs(500)).effect, "droplet");
    }

    #[test]
    fn snow_selects_snow() {
        assert_eq!(map(&obs(601)).effect, "snow");
    }

    #[test]
    fn atmosphere_selects_stratus() {
        assert_eq!(map(&obs(741)).effect, "stratus"); // fog
    }

    #[test]
    fn clear_selects_sunrays() {
        assert_eq!(map(&obs(800)).effect, "sunrays");
    }

    #[test]
    fn few_clouds_selects_cirrus() {
        assert_eq!(map(&obs(801)).effect, "cirrus");
    }

    #[test]
    fn scattered_clouds_selects_cumulus() {
        assert_eq!(map(&obs(802)).effect, "cumulus");
    }

    #[test]
    fn broken_clouds_selects_cumulus() {
        assert_eq!(map(&obs(803)).effect, "cumulus");
    }

    #[test]
    fn overcast_selects_stratus() {
        assert_eq!(map(&obs(804)).effect, "stratus");
    }

    #[test]
    fn wind_scales_with_speed() {
        // 6 m/s / 12 = 0.5
        let s = map(&Observation {
            wind_mps: 6.0,
            ..obs(800)
        });
        assert!((s.wind - 0.5).abs() < 1e-4, "wind was {}", s.wind);
    }

    #[test]
    fn wind_clamps_at_high_speed() {
        let s = map(&Observation {
            wind_mps: 100.0,
            ..obs(800)
        });
        assert!((s.wind - 1.2).abs() < 1e-4, "wind was {}", s.wind);
    }

    #[test]
    fn wind_never_negative() {
        let s = map(&Observation {
            wind_mps: -5.0,
            ..obs(800)
        });
        assert!(s.wind >= 0.0, "wind was {}", s.wind);
    }

    #[test]
    fn rain_intensity_scales_and_clamps() {
        // 4 mm/h / 8 = 0.5
        let mid = map(&Observation {
            rain_mmh: 4.0,
            ..obs(500)
        });
        assert!((mid.precip - 0.5).abs() < 1e-4, "precip was {}", mid.precip);
        // torrential rain saturates to 1.0
        let heavy = map(&Observation {
            rain_mmh: 50.0,
            ..obs(500)
        });
        assert!(
            (heavy.precip - 1.0).abs() < 1e-4,
            "precip was {}",
            heavy.precip
        );
    }

    #[test]
    fn snow_intensity_scales() {
        let s = map(&Observation {
            snow_mmh: 4.0,
            ..obs(601)
        });
        assert!((s.precip - 0.5).abs() < 1e-4, "precip was {}", s.precip);
    }

    #[test]
    fn cloud_intensity_from_coverage() {
        let s = map(&Observation {
            clouds_pct: 75,
            ..obs(803)
        });
        assert!((s.precip - 0.75).abs() < 1e-4, "precip was {}", s.precip);
    }

    #[test]
    fn clear_has_faint_intensity() {
        let s = map(&obs(800));
        assert!(s.precip > 0.0 && s.precip < 0.5, "precip was {}", s.precip);
    }

    #[test]
    fn thunderstorm_is_intense() {
        let s = map(&obs(211));
        assert!(s.precip >= 0.8, "precip was {}", s.precip);
    }
}
