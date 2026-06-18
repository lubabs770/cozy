


# cozy


https://github.com/user-attachments/assets/21bdbe17-fc5d-43c2-938d-a2aa5675c229







this project was written by claude

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

## Effects

cozy ships four swappable effects, switched live with `cozy effect <name>`. Here they are cycled over the same wallpaper — `droplet`, `ripple`, `snow`, then `clouds`:

![cozy cycling through its four effects: droplet, ripple, snow, clouds](docs/effects.gif)

`droplet` refracts the wallpaper through rain on glass (ported from BigWings' "Heartfelt"); `ripple` treats the wallpaper as a water surface struck by drops; `snow` is multi-layer parallax snowfall with depth-of-field (ported from Andrew Baldwin's "Just Snow"); `clouds` drifts soft fractal clouds across the wallpaper (ported from drift's "2D Clouds"). Each also has a transparent [overlay](#3-alongside-swwwhyprpaper-overlay) variant.

<br>

## Install

One command builds cozy, **detects your setup**, lets you confirm, and runs the matching integration:

```sh
curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash
```

It is idempotent (re-run to update) and needs no root. To skip the prompt (e.g. scripted installs), set `COZY_INTEGRATION=caelestia|standalone|swww`. cozy supports three integrations:

### 1. Caelestia

For the [Caelestia](https://github.com/caelestia-dots) dotfiles — cozy takes over wallpaper duties transparently. The installer:

- installs `cozy` + `cozy-session` to `~/.local/bin` and enables a `systemd --user` service,
- appends `cozy set "$WALLPAPER_PATH"` to your Caelestia wallpaper `postHook` (`cli.json`, backed up), so `caelestia wallpaper` flows into cozy live,
- turns off the Caelestia shell's own wallpaper (`background.wallpaperEnabled = false` in `shell.json`, backed up) so cozy is the sole renderer — clock and visualiser untouched, and
- seeds cozy with your current Caelestia wallpaper.

Change wallpaper the way you always have (`caelestia wallpaper`); switch rain with `cozy effect <name>`. Undo: restore the `.cozy-bak` backups next to `cli.json` / `shell.json` and `systemctl --user disable --now cozy.service`.

### 2. Standalone (plain Hyprland)

For vanilla Hyprland with no dotfiles — cozy owns the wallpaper. The installer:

- installs `cozy` + `cozy-session` + `cozy-wall` to `~/.local/bin`,
- writes a starter `~/.config/cozy/cozy.conf` (wallpaper, effect, weather) only if absent,
- writes `~/.config/cozy/hyprland.conf` (an `exec-once` + keybinds), **asking** whether to use preshipped keybinds or leave them commented for you to set, and
- adds exactly **one** `source = …` line to your real `hyprland.conf` (backed up to `*.cozy-bak`, skipped if present).

cozy owns the wallpaper, so don't run `hyprpaper`/`swww` alongside it. Change wallpaper with:

```sh
cozy-wall ~/Pictures/sunset.jpg     # swaps live (no restart) AND remembers it for next login
cozy effect snow                    # switch effect live
```

`cozy-wall` is the one command you need: it applies the change to the running daemon *and* records it in `cozy.conf` so `cozy-session` restores it next login. Undo: delete the `source` line from `hyprland.conf` (or restore `.cozy-bak`) and remove `~/.config/cozy` + the binaries.

### 3. Alongside swww/hyprpaper (overlay)

Keep your existing `swww`/`hyprpaper` daemon drawing the wallpaper, and run cozy as a transparent rain **overlay** on top (`cozy --overlay`). The installer:

- installs `cozy` + `cozy-session` (launches `cozy --overlay`) + `cozy-wall` to `~/.local/bin`,
- writes `~/.config/cozy/cozy.conf` and a sourced `~/.config/cozy/hyprland.conf` (`exec-once` + keybinds), and
- adds the same single `source = …` line to your `hyprland.conf` (backed up).

`cozy-wall` sets the wallpaper on **both** your daemon (auto-detecting swww or hyprpaper) and cozy's refraction copy, keeping them in sync:

```sh
cozy-wall ~/Pictures/sunset.jpg     # swww/hyprpaper + cozy, in one command
cozy effect droplet                 # any effect works in overlay
```

> **Overlay effect support:** every effect composites transparently over your wallpaper — `snow` carries alpha only where flakes fall, while `droplet` and `ripple` refract your daemon's wallpaper through the rain and let it show through the dry surface between drops. Each effect derives its own coverage from its internal rain signal, so the wallpaper daemon keeps drawing everything cozy leaves transparent.

> Advanced: each integration is also runnable directly from a checkout — `git clone` the repo and run `integrations/<name>/install.sh`.

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
cozy effect snow                        # switch effect (droplet | ripple | snow | clouds)
cozy weather --wind 0.4 --precip 0.9    # set wind skew + rain intensity
```

The socket lives at `$XDG_RUNTIME_DIR/cozy/cozy.sock`.

Stop it with `Ctrl-C` (or `kill`); the layer surfaces and GL contexts are torn down on exit.

> **Note:** cozy is Linux/Wayland only. On other platforms, develop and test it through the container harness below.

<br>

## Configuration

A TOML config file (`cozy.toml`) is planned and will expose the tunables below. Until then `wind` and rain intensity are set live with `cozy weather`, and each effect's parameters live as named constants at the top of its shader in `shaders/effects/`.

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

One layer surface per output, each owning its own EGL/GLES context and renderer. In the default **opaque** mode cozy draws the wallpaper as the base and composites the rain inside the shader — so there is no compositor-level transparency to fight. In `--overlay` mode it instead outputs premultiplied alpha and stays transparent between drops, letting an external wallpaper daemon show through (see the swww-overlay integration above).

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
    droplet.frag     rain on glass, refracting the wallpaper (ported from Heartfelt)
    ripple.frag      rain on a water surface, expanding rings
    snow.frag        multi-layer parallax snow with DoF (ported from Just Snow)
    clouds.frag      soft fractal clouds drifting over the wallpaper (ported from 2D Clouds)
```

Each effect is a fragment shader honouring a shared uniform contract (`u_resolution`, `u_tex_resolution`, `u_wallpaper`, `u_time`, `u_wind`, `u_intensity`, `u_overlay`), registered in `gl.rs` and switched live — so adding an effect is one shader file plus one table entry. In overlay mode each effect also derives its own coverage alpha from its internal rain signal, so it composites cleanly over an external wallpaper.

<br>

## License

cozy's own code is **MIT**.

Some effects are ported from well-known community shaders and keep their original **CC BY-NC-SA 3.0** license (attribution, non-commercial, share-alike) — **not** MIT. That license governs each listed file and any derivative of it:

| Effect | File | Original | Author |
|---|---|---|---|
| `droplet` | `shaders/effects/droplet.frag` | ["Heartfelt"](https://www.shadertoy.com/view/ltffzl) | Martijn Steinrucken (BigWings) |
| `snow` | `shaders/effects/snow.frag` | ["Just Snow"](https://www.shadertoy.com/view/ldsGDn) | Andrew Baldwin (baldand) |
| `clouds` | `shaders/effects/clouds.frag` | ["2D Clouds"](https://www.shadertoy.com/view/4tdSWr) | drift |

All other effects (e.g. `ripple`) are hand-built and MIT.
