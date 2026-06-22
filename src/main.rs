//! cozy — animated rain over your Wayland wallpaper.
//!
//! cozy is a `wlr-layer-shell` client that sits on the **Background** layer,
//! renders the wallpaper itself, and composites rain on top of it (additive
//! falling streaks and refracting glass droplets) in one fragment shader.
//!
//! Because cozy owns and draws the wallpaper, it runs *instead of* a separate
//! wallpaper daemon (swww / hyprpaper / …), not alongside one. Wallpaper
//! changes arrive over cozy's own control socket so it never needs to restart:
//!
//! ```text
//! cozy                      # run the daemon (embedded fallback wallpaper)
//! cozy --wallpaper a.png    # run the daemon with an initial wallpaper
//! cozy set b.png            # tell a running daemon to switch wallpaper
//! ```
//!
//! ## Layout
//!
//! * [`app`] — application state and all Wayland event handlers.
//! * [`control`] — the Unix-socket control protocol (daemon listener + client).
//! * [`surface`] — one Background layer surface per output, plus its GL drawing.
//! * [`render`] — EGL/GLES context management and the shader pipeline.

mod app;
mod config;
mod control;
mod render;
mod surface;
mod weather;

use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState,
    shell::wlr_layer::LayerShell,
};
use wayland_client::{globals::registry_queue_init, Connection};

use app::Cozy;
use control::Command;
use render::egl::Egl;

/// The wallpaper used when none is given on the command line, embedded so cozy
/// renders something out of the box.
const DEFAULT_WALLPAPER: &[u8] = include_bytes!("../assets/test-wallpaper.png");

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).peekable();

    match args.peek().map(String::as_str) {
        // Client mode: hand a command to a running daemon and exit.
        Some("set") => {
            args.next();
            let raw = args.next().context("usage: cozy set <wallpaper-path>")?;
            // Resolve to an absolute path now, while we're in the caller's cwd —
            // the daemon reads the file from a different working directory.
            let path = std::fs::canonicalize(&raw)
                .with_context(|| format!("wallpaper not found: {raw}"))?;
            control::send(&Command::SetWallpaper { path })
        }
        // Client mode: switch the active effect, or list effects when given no
        // name. Validate client-side so a typo gets an instant, helpful error
        // instead of silently failing inside the daemon.
        Some("effect") => {
            args.next();
            match args.next() {
                None => {
                    print_effects();
                    Ok(())
                }
                Some(name) => {
                    if !render::gl::effect_exists(&name) {
                        anyhow::bail!(
                            "unknown effect {name:?}\nknown effects: {}",
                            render::gl::effect_names()
                        );
                    }
                    control::send(&Command::SetEffect { name })
                }
            }
        }
        // Client mode: fetch local weather once and push it to a running daemon.
        // Driven by the user's systemd timer / cron; the secret API key lives in
        // the config file, never on the command line.
        Some("weather-sync") => {
            args.next();
            run_weather_sync()
        }
        // Client mode: set weather-driven parameters by hand.
        Some("weather") => {
            args.next();
            let rest: Vec<String> = args.collect();
            let wind = flag_value(&rest, "--wind")?;
            let precip = flag_value(&rest, "--precip")?;
            control::send(&Command::SetWeather { wind, precip })
        }
        Some("--help") | Some("-h") => {
            print_usage();
            Ok(())
        }
        // Daemon mode.
        _ => run_daemon(parse_daemon_args(args)?),
    }
}

/// Pull `--flag <value>` (an `f32`) out of CLI args for the `weather` subcommand.
fn flag_value(args: &[String], flag: &str) -> Result<f32> {
    let mut it = args.iter();
    while let Some(tok) = it.next() {
        if tok == flag {
            let v = it
                .next()
                .with_context(|| format!("{flag} requires a numeric value"))?;
            return v
                .parse::<f32>()
                .with_context(|| format!("invalid {flag} value {v:?}"));
        }
    }
    anyhow::bail!("usage: cozy weather --wind <f> --precip <f> (missing {flag})")
}

/// `cozy weather-sync`: load config, fetch local weather once, and push the
/// resulting effect + wind/precip to a running daemon. Exits nonzero on any
/// failure so a timer/cron job surfaces the problem.
fn run_weather_sync() -> Result<()> {
    let path = config::config_path();
    let cfg = config::Config::load(&path)?;
    let state = weather::sync_once(&cfg)?;
    for cmd in weather::commands_for(&state) {
        control::send(&cmd)?;
    }
    eprintln!(
        "cozy: weather → effect={} wind={:.2} precip={:.2}",
        state.effect, state.wind, state.precip
    );
    Ok(())
}

fn print_usage() {
    println!(
        "cozy — animated rain over your Wayland wallpaper\n\n\
         USAGE:\n  \
         cozy [--wallpaper <path>] [--overlay] [--weather]  run the daemon\n  \
         cozy set <path>                      switch the wallpaper (running daemon)\n  \
         cozy effect [<name>]                 switch the rain effect, or list effects\n  \
         cozy weather --wind <f> --precip <f> set weather-driven parameters by hand\n  \
         cozy weather-sync                    fetch local weather (config) → daemon\n  \
         cozy --help                          show this help\n\n  \
         --weather polls OpenWeatherMap (see ~/.config/cozy/config.toml) and drives\n  \
         the effect, wind and intensity from local conditions.\n"
    );
    print_effects();
}

/// List every registered effect with its description, marking the default.
fn print_effects() {
    println!("EFFECTS:");
    for (name, desc) in render::gl::effect_descriptions() {
        let marker = if name == render::gl::DEFAULT_EFFECT {
            "*"
        } else {
            " "
        };
        println!("  {marker} {name:<9} {desc}");
    }
    println!("\n  * default   ·   switch live with `cozy effect <name>`");
}

/// Parsed daemon arguments.
struct DaemonArgs {
    /// Initial wallpaper, or `None` to use the embedded fallback.
    wallpaper: Option<PathBuf>,
    /// Run as a transparent overlay above an external wallpaper daemon
    /// (`--overlay`) instead of owning the wallpaper itself.
    overlay: bool,
    /// Poll local weather (from the config file) and drive the shaders from it
    /// (`--weather`).
    weather: bool,
}

/// Parse the daemon's arguments: an optional `--wallpaper <path>`, `--overlay`,
/// and `--weather`.
fn parse_daemon_args(
    mut args: std::iter::Peekable<impl Iterator<Item = String>>,
) -> Result<DaemonArgs> {
    let mut wallpaper = None;
    let mut overlay = false;
    let mut weather = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wallpaper" | "-w" => {
                let path = args
                    .next()
                    .context("--wallpaper requires a path argument")?;
                wallpaper = Some(PathBuf::from(path));
            }
            "--overlay" => overlay = true,
            "--weather" => weather = true,
            other => anyhow::bail!("unexpected argument: {other:?} (try `cozy --help`)"),
        }
    }
    Ok(DaemonArgs {
        wallpaper,
        overlay,
        weather,
    })
}

fn run_daemon(args: DaemonArgs) -> Result<()> {
    // Load the initial wallpaper bytes: explicit path, else embedded fallback.
    // In overlay mode this image is used only as a refraction source, never
    // drawn as an opaque base.
    let wallpaper = match args.wallpaper {
        Some(path) => {
            std::fs::read(&path).with_context(|| format!("read wallpaper {}", path.display()))?
        }
        None => DEFAULT_WALLPAPER.to_vec(),
    };

    // Start the control socket before Wayland so a switch sent right at startup
    // is already queued by the time we begin drawing.
    let (command_tx, commands) = control::spawn_listener().context("start control socket")?;

    // Optionally drive the shaders from local weather: a background poller feeds
    // the same command channel the control socket uses. Config errors here are
    // fatal (the user explicitly asked for weather); fetch errors later are not.
    if args.weather {
        let path = config::config_path();
        let cfg = config::Config::load(&path)
            .with_context(|| format!("--weather needs a config at {}", path.display()))?;
        weather::spawn_poller(cfg, command_tx).context("start weather poller")?;
    } else {
        drop(command_tx);
    }

    let conn = Connection::connect_to_env().context("connect to Wayland display")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("initialize Wayland registry")?;
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    let layer_shell =
        LayerShell::bind(&globals, &qh).context("zwlr_layer_shell_v1 not available")?;
    let egl = Rc::new(Egl::new(&conn).context("initialize EGL")?);

    let mut state = Cozy::new(
        RegistryState::new(&globals),
        OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        egl,
        wallpaper,
        args.overlay,
        commands,
        qh.clone(),
    );

    // Surfaces are created from OutputHandler::new_output as outputs are
    // announced; pump the queue until we have at least one.
    loop {
        event_queue.blocking_dispatch(&mut state)?;
        if state.has_surfaces() {
            break;
        }
    }

    // Steady state: the animated rain keeps frame callbacks (and thus dispatch)
    // firing, so queued control commands are drained each frame.
    loop {
        event_queue.blocking_dispatch(&mut state)?;
    }
}
