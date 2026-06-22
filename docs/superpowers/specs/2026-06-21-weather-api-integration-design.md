# Weather API integration — design

**Date:** 2026-06-21
**Status:** implemented (2026-06-22)

## Goal

Drive cozy's shader knobs from real local weather via OpenWeatherMap (OWM):
pick the effect, set wind, set intensity from current conditions. This realises
the README's stated end goal ("hooked up to a weather api taking params like
precipitation, wind speed, wind direction etc. and applying the shaders").

## Architecture

A self-contained `weather` module with a network-free, unit-testable core,
exposed two ways over cozy's existing control socket.

- `src/config.rs` — load `~/.config/cozy/config.toml` (`toml` crate + serde).
- `src/weather/owm.rs` — build the OWM *Current Weather* request, blocking HTTPS
  GET (`ureq`), parse JSON (`serde`) into a typed `Observation`.
- `src/weather/mapping.rs` — **pure** `map(&Observation) -> WeatherState`. No I/O.
- `src/weather/mod.rs` — `sync_once(&Config) -> Result<WeatherState>`: fetch + map.

Entry points:
- `cozy weather-sync` (client subcommand): `sync_once`, then `control::send`
  `SetEffect` + `SetWeather`. One-shot; nonzero exit on failure. Driven by the
  user's systemd timer / cron.
- `cozy --weather` (daemon flag): a poller thread loops
  `{ sync_once; push commands; sleep interval }`, feeding the **same** mpsc
  channel the control listener uses. `control::spawn_listener` is refactored to
  expose a `Sender<Command>` clone for the poller.

Both paths share `sync_once`, so behaviour is identical.

## OpenWeatherMap

- Endpoint: `GET https://api.openweathermap.org/data/2.5/weather`
- Params: `appid` (key), `units` (metric/imperial), and either `lat`+`lon` or
  `q={city}`.
- Relevant response fields: `weather[0].id` (condition code), `wind.speed`
  (m/s metric), `rain.1h` / `snow.1h` (mm/h, where present), `clouds.all` (%).
  The `1h` keys need serde `rename`.

### `Observation` (parsed)

```
struct Observation {
    condition_id: u16,   // weather[0].id
    wind_mps:     f32,   // wind.speed (metric)
    rain_mmh:     f32,   // rain.1h or 0
    snow_mmh:     f32,   // snow.1h or 0
    clouds_pct:   u8,    // clouds.all
}
```

## Config (`~/.config/cozy/config.toml`)

```toml
api_key  = "…"          # required
location = "London,GB"  # city query; or set lat/lon instead
# lat = 51.51
# lon = -0.13
units    = "metric"     # metric | imperial   (default metric)
interval = 600          # daemon poll seconds (default 600)
```

Validation: `api_key` required; exactly one of `location` or (`lat`+`lon`)
required. Missing/invalid config is a clear error.

## Condition → effect & knobs mapping

`map(&Observation) -> WeatherState { effect: &'static str, wind: f32, precip: f32 }`

| OWM condition group | effect | precip (intensity) source |
|---|---|---|
| Thunderstorm (2xx) | `lightning` | high (0.8 + rain) |
| Drizzle (3xx), Rain (5xx) | `droplet` | `rain_mmh` → 0..1 |
| Snow (6xx) | `snow` | `snow_mmh` → 0..1 |
| Atmosphere — mist/fog/haze (7xx) | `stratus` | `clouds_pct` → 0..1 |
| Clear (800) | `sunrays` | 0.2 (faint) |
| Few clouds (801) | `cirrus` | `clouds_pct` → 0..1 |
| Scattered/broken (802, 803) | `cumulus` | `clouds_pct` → 0..1 |
| Overcast (804) | `stratus` | `clouds_pct` → 0..1 |

- `wind` = `clamp(wind_mps / 12.0, 0.0, 1.2)` for every condition.
- `precip` clamped to `0..1`. Precip normalisers: rain/snow `mm/h ÷ 8` (≈heavy
  rain → 1.0); clouds `pct ÷ 100`.
- `ripple` and `cumulonimbus` are **not** auto-selected (alternate rain/storm
  looks); reachable via `cozy effect`. The table lives in one place for easy
  tweaking.

## Error handling

- Daemon poller: log network / auth / parse errors, keep the last good state,
  retry next interval. Never crashes the daemon.
- `weather-sync`: print a clear error and exit nonzero.

## Testing

All network-free so they run in CI (the Docker harness; the crate doesn't build
on the macOS host due to Wayland deps):

- `mapping::map` — one case per condition class (thunderstorm, drizzle, rain,
  snow, fog, clear, each cloud level 801–804) plus wind and precip extremes
  (clamping at both ends).
- `owm` JSON parse — against a captured sample OWM response, incl. the
  rain/snow-absent case.
- `config` parse — valid file, missing key, city-vs-latlon validation.

Real-endpoint check (`cozy weather-sync` against live OWM with a key) is a
documented manual step.

## Out of scope (YAGNI)

Smoothing/interpolation between readings (weather changes slowly), forecast
data, multiple locations, caching beyond last-good-state.
