<br>
<br>

# cozy

<br>

https://github.com/user-attachments/assets/21bdbe17-fc5d-43c2-938d-a2aa5675c229

<br>

> Written by Claude. The original goal — hook it up to a weather API (precipitation, wind speed, wind direction) and drive the shaders from real local conditions — is now live; see [Weather](#weather).

<br>
<br>

Animated weather over your Wayland wallpaper.

<br>

cozy is a `wlr-layer-shell` client that sits on the **background** layer, renders your wallpaper itself, and composites animated weather on top of it — glass **droplets** that refract the wallpaper behind them, drifting clouds, snow, sun rays, lightning. Clicks fall straight through to the desktop.

<br>

Because cozy owns the wallpaper (you can't refract pixels you don't have), it runs **instead of** a wallpaper daemon, not alongside one. It switches wallpaper and effect live over its own control socket, so it never needs a restart — and it can read your local weather to pick the effect and set the wind and intensity for you.

<br>

![cozy rendering the droplet effect over a sunset wallpaper](docs/droplet.png)

<br>
<br>

## Effects

<br>

cozy ships ten swappable effects, switched live with `cozy effect <name>`. Here they are cycled over the same wallpaper — `droplet`, `ripple`, `snow`, `clouds`, `cirrus`, `cumulus`, `cumulonimbus`, `stratus`, `sunrays`, then `lightning`:

<br>

![cozy cycling through its ten effects, from rain and snow through the cloud types to sun rays and lightning](docs/effects.gif)

<br>

**Rain & snow** — `droplet` refracts the wallpaper through rain on glass (ported from BigWings' "Heartfelt"); `ripple` treats the wallpaper as a water surface struck by drops; `snow` is multi-layer parallax snowfall with depth-of-field (ported from Andrew Baldwin's "Just Snow").

<br>

**Clouds** — `clouds` drifts soft fractal clouds across the wallpaper (ported from drift's "2D Clouds"), plus four hand-built types: `cirrus` (high, thin, wispy streaks), `cumulus` (fluffy fair-weather puffs with sunlit tops and shaded undersides), `cumulonimbus` (heavy, dark, towering storm clouds), and `stratus` (a flat, featureless grey overcast layer).

<br>

**Light & storm** — `sunrays` casts volumetric god rays fanning out from the sun, broken into shafts by a drifting occluder; `lightning` broods as a dark storm sky that periodically flares with a flash and a forking bolt.

<br>

Every effect honours the shared weather inputs (`u_wind` drives drift, `u_intensity` drives cover) and has a transparent [overlay](#3-alongside-swwwhyprpaper-overlay) variant. The hand-built effects are MIT-licensed like the rest of cozy; the three ports are CC BY-NC-SA 3.0 (see [License](#license)).

<br>
<br>

## Install

<br>

One command builds cozy, **detects your setup**, lets you confirm, and runs the matching integration:

<br>

```sh
curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash
```

<br>

It is idempotent (re-run to update) and needs no root. To skip the prompt (e.g. scripted installs), set `COZY_INTEGRATION=caelestia|standalone|swww`. cozy supports three integrations; each is also runnable directly from a checkout via `integrations/<name>/install.sh`.

<br>
<br>

### 1. Caelestia

<br>

For the [Caelestia](https://github.com/caelestia-dots) dotfiles — cozy takes over wallpaper duties transparently. The installer:

<br>

- installs `cozy` + `cozy-session` to `~/.local/bin` and enables a `systemd --user` service,

<br>

- appends `cozy set "$WALLPAPER_PATH"` to your Caelestia wallpaper `postHook` (`cli.json`, backed up), so `caelestia wallpaper` flows into cozy live,

<br>

- turns off the Caelestia shell's own wallpaper (`background.wallpaperEnabled = false` in `shell.json`, backed up) so cozy is the sole renderer — clock and visualiser untouched, and

<br>

- seeds cozy with your current Caelestia wallpaper.

<br>

Change wallpaper the way you always have (`caelestia wallpaper`); switch effects with `cozy effect <name>`. Undo: restore the `.cozy-bak` backups next to `cli.json` / `shell.json` and `systemctl --user disable --now cozy.service`.

<br>
<br>

### 2. Standalone (plain Hyprland)

<br>

For vanilla Hyprland with no dotfiles — cozy owns the wallpaper. The installer:

<br>

- installs `cozy` + `cozy-session` + `cozy-wall` to `~/.local/bin`,

<br>

- writes a starter `~/.config/cozy/cozy.conf` (wallpaper, effect, weather) only if absent,

<br>

- writes `~/.config/cozy/hyprland.conf` (an `exec-once` + keybinds), **asking** whether to use preshipped keybinds or leave them commented for you to set, and

<br>

- adds exactly **one** `source = …` line to your real `hyprland.conf` (backed up to `*.cozy-bak`, skipped if present).

<br>

cozy owns the wallpaper, so don't run `hyprpaper`/`swww` alongside it. Change it with `cozy-wall` — the one command you need, which applies the change to the running daemon *and* records it in `cozy.conf` so `cozy-session` restores it next login:

<br>

```sh
cozy-wall ~/Pictures/sunset.jpg     # swaps live (no restart) AND remembers it for next login
cozy effect snow                    # switch effect live
```

<br>

Undo: delete the `source` line from `hyprland.conf` (or restore `.cozy-bak`) and remove `~/.config/cozy` + the binaries.

<br>
<br>

### 3. Alongside swww/hyprpaper (overlay)

<br>

Keep your existing `swww`/`hyprpaper` daemon drawing the wallpaper, and run cozy as a transparent **overlay** on top (`cozy --overlay`). The installer:

<br>

- installs `cozy` + `cozy-session` (launching `cozy --overlay`) + `cozy-wall` to `~/.local/bin`,

<br>

- writes `~/.config/cozy/cozy.conf` and a sourced `~/.config/cozy/hyprland.conf` (`exec-once` + keybinds), and

<br>

- adds the same single `source = …` line to your `hyprland.conf` (backed up).

<br>

`cozy-wall` sets the wallpaper on **both** your daemon (auto-detecting swww or hyprpaper) and cozy's refraction copy, keeping them in sync:

<br>

```sh
cozy-wall ~/Pictures/sunset.jpg     # swww/hyprpaper + cozy, in one command
cozy effect droplet                 # any effect works in overlay
```

<br>

Every effect composites transparently over your wallpaper: `snow` carries alpha only where flakes fall, while `droplet` and `ripple` refract your daemon's wallpaper through the rain and let it show through the dry surface between drops. Each effect derives its own coverage from its internal rain signal, so the wallpaper daemon keeps drawing everything cozy leaves transparent.

<br>
<br>

## Build & run

<br>

cozy is Linux/Wayland only. On other platforms, build and test it through the [container harness](#verification).

<br>

```sh
cargo build --release
./target/release/cozy
```

<br>

cozy binds one background surface per output and starts drawing immediately. A test wallpaper is embedded, so it renders out of the box. Stop it with `Ctrl-C` (or `kill`); the layer surfaces and GL contexts are torn down on exit.

<br>

Drive a running instance with the same binary (point your wallpaper keybind / rotation script at it — cozy is wallpaper-daemon-agnostic):

<br>

```sh
cozy --wallpaper ~/walls/now.jpg        # start with a wallpaper
cozy --weather                          # also drive effects from local weather
cozy set ~/walls/next.jpg               # switch wallpaper live, no restart
cozy effect snow                        # switch effect live
cozy weather --wind 0.4 --precip 0.9    # set wind skew + intensity by hand
cozy weather-sync                       # fetch local weather once, apply to daemon
```

<br>

The control socket lives at `$XDG_RUNTIME_DIR/cozy/cozy.sock`.

<br>
<br>

## Weather

<br>

cozy can drive itself from your local weather via [OpenWeatherMap](https://openweathermap.org/api): the current conditions pick the effect and set the wind and intensity, so a rainy, blustery day actually looks like one on your desktop.

<br>

Copy [`config.toml.example`](config.toml.example) to `~/.config/cozy/config.toml` and add your (free) OWM API key and location:

<br>

```toml
api_key  = "…"          # OpenWeatherMap key (kept out of the command line)
location = "London,GB"  # or set lat/lon instead
units    = "metric"     # metric keeps wind in m/s
interval = 600          # daemon poll seconds
```

<br>

Then either let the daemon poll in the background, or sync once from a timer:

<br>

```sh
cozy --weather       # daemon polls every `interval` seconds and applies live
cozy weather-sync    # fetch once now and push to a running daemon (cron/systemd timer)
```

<br>

The condition code maps to an effect, with `wind` from wind speed and intensity from precipitation / cloud cover:

<br>

| Conditions | Effect |
|---|---|
| Thunderstorm | `lightning` |
| Drizzle / rain | `droplet` |
| Snow | `snow` |
| Mist / fog / haze | `stratus` |
| Clear sky | `sunrays` |
| Few clouds | `cirrus` |
| Scattered / broken clouds | `cumulus` |
| Overcast | `stratus` |

<br>

A failed fetch (network down, bad key) is logged and the daemon keeps its last good state — it never crashes. The mapping table lives in one place (`src/weather/mapping.rs`) and is easy to tweak.

<br>
<br>

## Configuration

<br>

Beyond the weather config above, each effect's tunables live as named constants at the top of its shader in `shaders/effects/`, and `wind` / intensity can be set live by hand with `cozy weather --wind <f> --precip <f>`. A fuller config surface is planned:

<br>

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
<br>

## Verification

<br>

The dev machine here is macOS, but cozy is Linux-only — so it is built and **visually verified inside a Linux container**. The harness runs a headless [sway](https://swaywm.org/) compositor with Mesa's software renderer, launches cozy against it, and captures screenshots with [grim](https://sr.ht/~emersion/grim/) into `./out/`:

<br>

```sh
make verify          # build image, run cozy under headless sway, capture frames
make verify ARGS=…   # pass extra args to the cozy binary
make lint            # rustfmt --check + clippy -D warnings
make shell           # drop into the container to poke around
```

<br>

Each milestone is confirmed by reading the captured PNGs: a solid clear color (EGL works), the wallpaper (texture + cover-fit), then streaks and droplets that move between frames.

<br>

**Requirements:** a Wayland compositor implementing `wlr-layer-shell` (Hyprland, sway, river, …); Mesa / EGL with OpenGL ES 3.0 (llvmpipe software rendering is fine); Rust (stable) and the usual Wayland/EGL development headers.

<br>
<br>

## Architecture

<br>

One layer surface per output, each owning its own EGL/GLES context and renderer. In the default **opaque** mode cozy draws the wallpaper as the base and composites the weather inside the shader — so there is no compositor-level transparency to fight. In `--overlay` mode it instead outputs premultiplied alpha and stays transparent between drops, letting an external wallpaper daemon show through.

<br>

```
src/
  main.rs            bootstrap + CLI: run the daemon, or send a control command
  control.rs         Unix-socket control protocol (set / effect / weather)
  config.rs          load ~/.config/cozy/config.toml (weather settings)
  app.rs             app state + all Wayland event handlers
  surface.rs         one background layer surface per output, + its drawing
  weather/
    mod.rs           sync_once + the background poller thread
    owm.rs           OpenWeatherMap fetch + JSON → Observation
    mapping.rs       pure Observation → effect + wind + intensity
  render/
    egl.rs           EGL display/context setup on a Wayland surface
    gl.rs            effect registry, fullscreen-triangle draw, uniforms
    texture.rs       decode an image → mipmapped RGBA8 GL texture
shaders/
  rain.vert          fullscreen triangle
  effects/*.frag     one fragment shader per effect (droplet, ripple, snow,
                     clouds, cirrus, cumulus, cumulonimbus, stratus, sunrays,
                     lightning)
```

<br>

Each effect is a fragment shader honouring a shared uniform contract (`u_resolution`, `u_tex_resolution`, `u_wallpaper`, `u_time`, `u_wind`, `u_intensity`, `u_overlay`), registered in `gl.rs` and switched live — so adding an effect is one shader file plus one table entry. In overlay mode each effect also derives its own coverage alpha from its internal rain signal, so it composites cleanly over an external wallpaper.

<br>
<br>

## License

<br>

cozy's own code is **MIT**. Three effects are ported from well-known community shaders and keep their original **CC BY-NC-SA 3.0** license (attribution, non-commercial, share-alike) — **not** MIT. That license governs each listed file and any derivative of it; all other effects are hand-built and MIT.

<br>

| Effect | File | Original | Author |
|---|---|---|---|
| `droplet` | `shaders/effects/droplet.frag` | ["Heartfelt"](https://www.shadertoy.com/view/ltffzl) | Martijn Steinrucken (BigWings) |
| `snow` | `shaders/effects/snow.frag` | ["Just Snow"](https://www.shadertoy.com/view/ldsGDn) | Andrew Baldwin (baldand) |
| `clouds` | `shaders/effects/clouds.frag` | ["2D Clouds"](https://www.shadertoy.com/view/4tdSWr) | drift |

<br>
<br>
