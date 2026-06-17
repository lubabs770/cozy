//! Local control socket: lets a second `cozy` invocation drive a running one.
//!
//! cozy runs as the wallpaper daemon (it owns and renders the wallpaper). To
//! change the wallpaper without restarting — the common case on setups that
//! switch wallpapers often — the *same binary* in client mode connects to a
//! Unix domain socket and sends a one-line command, which the daemon applies
//! live.
//!
//! The interface is deliberately environment-agnostic: cozy defines its own
//! protocol and the user points whatever changes their wallpaper (a keybind, a
//! rotation script, a desktop-shell action) at `cozy set <path>`. cozy never
//! inspects or depends on which compositor or wallpaper tooling is in use.
//!
//! The wire format is intentionally trivial — newline-terminated UTF-8, one
//! command per line, space-separated — so anything (even `socat`/`nc`) can
//! drive cozy. The [`Command`] enum is built to grow: later phases add an
//! effect switch and live weather parameters over this same socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{env, fs, thread};

use anyhow::{anyhow, Context, Result};

/// A command sent over the control socket.
///
/// Designed to grow: Phase 2 will add an effect switch, Phase 3 live weather
/// parameters. Each variant maps to one wire verb in [`Command::parse`] /
/// [`Command::to_line`].
#[derive(Debug, Clone)]
// All variants are setters; the shared `Set` prefix is intentional.
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Replace the wallpaper with the image at this path.
    SetWallpaper { path: PathBuf },
    /// Switch the active rain effect (e.g. `droplet`, `classic`).
    SetEffect { name: String },
    /// Set the weather-driven parameters: `wind` (horizontal skew) and `precip`
    /// (rain intensity, 0..1). Sent live by a weather poller in a later phase.
    SetWeather { wind: f32, precip: f32 },
}

impl Command {
    /// Parse one wire line into a command. Verbs:
    /// * `set <path>`
    /// * `effect <name>`
    /// * `weather --wind <f> --precip <f>`
    fn parse(line: &str) -> Result<Self> {
        let line = line.trim();
        let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match verb {
            "set" => {
                let rest = rest.trim();
                if rest.is_empty() {
                    return Err(anyhow!("`set` requires a wallpaper path"));
                }
                Ok(Command::SetWallpaper {
                    path: PathBuf::from(rest),
                })
            }
            "effect" => {
                let name = rest.trim();
                if name.is_empty() {
                    return Err(anyhow!("`effect` requires a name"));
                }
                Ok(Command::SetEffect {
                    name: name.to_string(),
                })
            }
            "weather" => {
                let wind = flag_value(rest, "--wind")?;
                let precip = flag_value(rest, "--precip")?;
                Ok(Command::SetWeather { wind, precip })
            }
            other => Err(anyhow!("unknown command: {other:?}")),
        }
    }

    /// Render this command as a wire line (no trailing newline).
    fn to_line(&self) -> String {
        match self {
            Command::SetWallpaper { path } => format!("set {}", path.display()),
            Command::SetEffect { name } => format!("effect {name}"),
            Command::SetWeather { wind, precip } => {
                format!("weather --wind {wind} --precip {precip}")
            }
        }
    }
}

/// Parse `--flag <value>` (an `f32`) out of a whitespace-separated argument
/// string. Errors if the flag is missing or its value doesn't parse.
fn flag_value(args: &str, flag: &str) -> Result<f32> {
    let mut it = args.split_whitespace();
    while let Some(tok) = it.next() {
        if tok == flag {
            let v = it
                .next()
                .ok_or_else(|| anyhow!("{flag} requires a numeric value"))?;
            return v
                .parse::<f32>()
                .map_err(|e| anyhow!("invalid {flag} value {v:?}: {e}"));
        }
    }
    Err(anyhow!("missing {flag}"))
}

/// Path to the control socket: `$XDG_RUNTIME_DIR/cozy/cozy.sock`, falling back
/// to `/tmp/cozy/cozy.sock` if the runtime dir is unset.
pub fn socket_path() -> PathBuf {
    let dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join("cozy").join("cozy.sock")
}

/// Bind the control socket and spawn a listener thread, returning the receiving
/// end of a channel that yields parsed [`Command`]s.
///
/// The thread only moves owned data across the channel, so the GL/Wayland side
/// stays single-threaded. Per-connection errors are logged and never bring the
/// daemon down.
pub fn spawn_listener() -> Result<Receiver<Command>> {
    let path = socket_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create socket dir {}", parent.display()))?;
    }
    // A stale socket from a previous run would block bind; remove it first.
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("bind control socket {}", path.display()))?;

    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("cozy-control".into())
        .spawn(move || listen_loop(listener, tx))
        .context("spawn control listener thread")?;
    Ok(rx)
}

fn listen_loop(listener: UnixListener, tx: Sender<Command>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_client(stream, &tx),
            Err(e) => eprintln!("cozy: control accept error: {e}"),
        }
    }
}

fn handle_client(stream: UnixStream, tx: &Sender<Command>) {
    for line in BufReader::new(stream).lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("cozy: control read error: {e}");
                return;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        match Command::parse(&line) {
            // If the daemon has gone away the send fails; nothing more to do.
            Ok(cmd) => {
                if tx.send(cmd).is_err() {
                    return;
                }
            }
            Err(e) => eprintln!("cozy: ignoring control command: {e:#}"),
        }
    }
}

/// Client side: connect to a running daemon and send one command.
pub fn send(cmd: &Command) -> Result<()> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|e| {
        anyhow!(
            "could not reach a running cozy at {} ({e}); is the cozy daemon running?",
            path.display()
        )
    })?;
    let mut line = cmd.to_line();
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .context("send control command")?;
    stream.flush().context("flush control command")?;
    Ok(())
}
