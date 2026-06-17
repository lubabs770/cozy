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
mod control;
mod render;
mod surface;

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
        // Client mode: switch the active effect.
        Some("effect") => {
            args.next();
            let name = args.next().context("usage: cozy effect <name>")?;
            control::send(&Command::SetEffect { name })
        }
        // Client mode: set weather-driven parameters.
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

fn print_usage() {
    println!(
        "cozy — animated rain over your Wayland wallpaper\n\n\
         USAGE:\n  \
         cozy [--wallpaper <path>]            run the daemon\n  \
         cozy set <path>                      switch the wallpaper (running daemon)\n  \
         cozy effect <name>                   switch the rain effect (e.g. droplet, classic)\n  \
         cozy weather --wind <f> --precip <f> set weather-driven parameters\n  \
         cozy --help                          show this help"
    );
}

/// Parse the daemon's arguments: an optional `--wallpaper <path>`.
fn parse_daemon_args(
    mut args: std::iter::Peekable<impl Iterator<Item = String>>,
) -> Result<Option<PathBuf>> {
    let mut wallpaper = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wallpaper" | "-w" => {
                let path = args
                    .next()
                    .context("--wallpaper requires a path argument")?;
                wallpaper = Some(PathBuf::from(path));
            }
            other => anyhow::bail!("unexpected argument: {other:?} (try `cozy --help`)"),
        }
    }
    Ok(wallpaper)
}

fn run_daemon(initial_wallpaper: Option<PathBuf>) -> Result<()> {
    // Load the initial wallpaper bytes: explicit path, else embedded fallback.
    let wallpaper = match initial_wallpaper {
        Some(path) => {
            std::fs::read(&path).with_context(|| format!("read wallpaper {}", path.display()))?
        }
        None => DEFAULT_WALLPAPER.to_vec(),
    };

    // Start the control socket before Wayland so a switch sent right at startup
    // is already queued by the time we begin drawing.
    let commands = control::spawn_listener().context("start control socket")?;

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
