# gamma branch is (for) now the main!!


### cozy
vid from gamma:

https://github.com/user-attachments/assets/21bdbe17-fc5d-43c2-938d-a2aa5675c229







this was written by claude

### the end goal of this project is to have it hooked up to a weather api taking params like precipitation
### wind speed wind direction etc. and applying the shaders accordingly

<br>
<br>


> below written by claude


Animated rain over your Wayland wallpaper.

cozy is a `wlr-layer-shell` client that sits on the **background** layer, renders your wallpaper itself, and composites animated rain on top of it — glass **droplets** that refract the wallpaper behind them. Clicks fall straight through to the desktop.

Because cozy owns the wallpaper (you can't refract pixels you don't have), it runs **instead of** a wallpaper daemon, not alongside one. It switches wallpaper live over its own control socket, so it never needs a restart.

The rain is a swappable **effect**; switch effects (and, later, let local weather drive the wind and intensity) on a running instance.

![cozy rendering the droplet effect over a sunset wallpaper](docs/droplet.png)

<br>

## Install (Caelestia + Hyprland)

If you run the [Caelestia](https://github.com/caelestia-dots) dotfiles, one command builds cozy and wires it in so it takes over wallpaper duties transparently:

```sh
curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/gamma/install.sh | bash
```

The installer is idempotent (re-run it to update) and needs no root. It:

- builds the release binary and installs `cozy` + `cozy-session` to `~/.local/bin`,

- installs and enables a `systemd --user` service so cozy starts with your graphical session,

- appends `cozy set "$WALLPAPER_PATH"` to your Caelestia wallpaper `postHook` (`~/.config/caelestia/cli.json`, backed up first), so `caelestia wallpaper` changes flow into cozy live,

- turns off the Caelestia shell's own wallpaper rendering (`background.wallpaperEnabled = false` in `shell.json`, also backed up) so cozy is the sole wallpaper renderer — the desktop clock and visualiser are left untouched, and

- seeds cozy with your current Caelestia wallpaper.

After that, change wallpaper the way you always have — `caelestia wallpaper` — and cozy follows along. Switch the rain with `cozy effect <name>`.

To undo everything, restore the `.cozy-bak` backups next to `cli.json` / `shell.json` and `systemctl --user disable --now cozy.service`.

<br>

## Requirements

- A Wayland compositor that implements `wlr-layer-shell` (Hyprland, sway, river, …).

- Mesa / EGL with OpenGL ES 3.0 (software rendering via llvmpipe is fine).

- Rust (stable) and the usual Wayland/EGL development headers.

<br>

## Build & run

```sh
cargo build --release
./target/release/cozy
```

cozy binds one background surface per output and starts drawing immediately. A test wallpaper is embedded, so it renders out of the box.

Drive a running instance with the same binary (point your wallpaper keybind / rotation script at it — cozy is wallpaper-daemon-agnostic):

```sh
cozy --wallpaper ~/walls/now.jpg        # start with a wallpaper
cozy set ~/walls/next.jpg               # switch wallpaper live, no restart
cozy effect classic                     # switch rain effect (droplet | classic)
cozy weather --wind 0.4 --precip 0.9    # set wind skew + rain intensity
```

The socket lives at `$XDG_RUNTIME_DIR/cozy/cozy.sock`.

Stop it with `Ctrl-C` (or `kill`); the layer surfaces and GL contexts are torn down on exit.

> **Note:** cozy is Linux/Wayland only. On other platforms, develop and test it through the container harness below.

<br>

## Configuration

A TOML config file (`cozy.toml`) is planned and will expose the tunables below. Until then `wind` and rain intensity are set live with `cozy weather`, and the `classic` effect's parameters live as named constants at the top of each stage in `shaders/effects/classic.frag`.

| Knob | Meaning |
|---|---|
| `wallpaper_path` | Image to render as the background. |
| `streak_density` | How many rain streaks. |
| `droplet_density` | How many glass droplets. |
| `wind` | Horizontal skew shared by all effects. |
| `refraction_strength` | How strongly droplets bend the wallpaper. |
| `tint` | Streak color. |
| `fps_cap` | Upper bound on redraw rate. |

<br>

## How verification works

The dev machine here is macOS, but cozy is Linux-only — so it is built and **visually verified inside a Linux container**.

The harness runs a headless [sway](https://swaywm.org/) compositor with Mesa's software renderer, launches cozy against it, and captures screenshots with [grim](https://sr.ht/~emersion/grim/) into `./out/`.

```sh
make verify          # build image, run cozy under headless sway, capture frames
make verify ARGS=…   # pass extra args to the cozy binary
make lint            # rustfmt --check + clippy -D warnings
make shell           # drop into the container to poke around
```

Each milestone is confirmed by reading the captured PNGs: a solid clear color (EGL works), the wallpaper (texture + cover-fit), then streaks and droplets that move between frames.

<br>

## Architecture

One layer surface per output, each owning its own EGL/GLES context and renderer. cozy draws the wallpaper as the **opaque** base and composites the rain inside the shader — so there is no compositor-level transparency to fight.

```
src/
  main.rs            bootstrap + CLI: run the daemon, or send a control command
  control.rs         Unix-socket control protocol (set / effect / weather)
  app.rs             app state + all Wayland event handlers
  surface.rs         one background layer surface per output, + its drawing
  render/
    egl.rs           EGL display/context setup on a Wayland surface
    gl.rs            effect registry, fullscreen-triangle draw, uniforms
    texture.rs       decode an image → mipmapped RGBA8 GL texture
shaders/
  rain.vert          fullscreen triangle
  effects/
    droplet.frag     rain on glass with refraction (ported from Heartfelt)
    classic.frag     the original hand-built streaks + droplets
```

Each effect is a fragment shader honouring a shared uniform contract (`u_resolution`, `u_tex_resolution`, `u_wallpaper`, `u_time`, `u_wind`, `u_intensity`), registered in `gl.rs` and switched live — so adding an effect is one shader file plus one table entry.

<br>

## License

cozy's own code is **MIT**.

The `droplet` effect (`shaders/effects/droplet.frag`) is ported from **"Heartfelt"** by **Martijn Steinrucken (BigWings)** — <https://www.shadertoy.com/view/ltffzl> — and is licensed **CC BY-NC-SA 3.0**, not MIT. That license (attribution, non-commercial, share-alike) governs that file and any derivative of it.
