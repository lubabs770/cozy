//! Weather integration: fetch local conditions from OpenWeatherMap and map them
//! onto cozy's shader knobs (effect + `wind` + `precip`).
//!
//! The core is split so the decision logic is testable without a network:
//! * [`mapping`] — pure `Observation -> WeatherState`.
//! * `owm` — the HTTP fetch + JSON parse into an `Observation`.

pub mod mapping;
pub mod owm;

use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::control::Command;
use mapping::WeatherState;

/// Fetch the current weather described by `config` and map it to cozy's shader
/// knobs. Shared by the one-shot `weather-sync` subcommand and the daemon's
/// background poller, so both behave identically.
pub fn sync_once(config: &Config) -> Result<WeatherState> {
    let obs = owm::fetch(&config.api_key, &config.location, &config.units)?;
    Ok(mapping::map(&obs))
}

/// Turn a mapped [`WeatherState`] into the two control commands that apply it:
/// switch the effect, then set the wind/precip knobs.
pub fn commands_for(state: &WeatherState) -> [Command; 2] {
    [
        Command::SetEffect {
            name: state.effect.to_string(),
        },
        Command::SetWeather {
            wind: state.wind,
            precip: state.precip,
        },
    ]
}

/// Spawn the daemon's background weather poller.
///
/// Every `config.interval` seconds it fetches the weather, maps it, and pushes
/// the resulting commands onto the same channel the control socket feeds. A
/// failed fetch (network down, bad key, …) is logged and the daemon keeps its
/// last good state until the next interval — it never crashes the daemon.
pub fn spawn_poller(config: Config, tx: Sender<Command>) -> Result<()> {
    let interval = Duration::from_secs(config.interval.max(1));
    thread::Builder::new()
        .name("cozy-weather".into())
        .spawn(move || loop {
            match sync_once(&config) {
                Ok(state) => {
                    for cmd in commands_for(&state) {
                        // Daemon gone (receiver dropped): stop polling.
                        if tx.send(cmd).is_err() {
                            return;
                        }
                    }
                }
                Err(e) => eprintln!("cozy: weather sync failed (keeping last state): {e:#}"),
            }
            thread::sleep(interval);
        })
        .context("spawn weather poller thread")?;
    Ok(())
}
