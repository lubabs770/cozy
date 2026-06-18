# AGENTS.md

Guide for coding agents working in this repo. Humans: see [README.md](README.md).

## What cozy is

A Rust `wlr-layer-shell` client that renders your Wayland wallpaper and
composites animated rain on top of it (OpenGL ES via EGL). It owns the
wallpaper so it can refract it, so it runs *instead of* a wallpaper daemon. See
[llms.txt](llms.txt) for a file-by-file map.

## Build / lint / verify

- **Build:** `cargo build --release`
- **Lint:** `make lint` — `rustfmt --check` + `clippy -D warnings`. Keep it clean.
- **Verify (the important one):** the dev machine is macOS but **cozy is
  Linux/Wayland-only** and needs a GPU/compositor, so it cannot run here. It is
  built and visually checked inside a Linux container:
  - `make verify` — builds the image, runs cozy under headless sway with Mesa's
    software renderer, captures frames with grim into `./out/`.
  - `make verify ARGS=…` — pass extra args to the cozy binary.
  - `make shell` — drop into the container.
  Confirm rendering by reading the captured PNGs (clear color → wallpaper →
  moving streaks/droplets). Do not claim a render change works without a
  container capture.

## Layout

```
src/            the engine (one binary)
shaders/        rain.vert + effects/*.frag (shared uniform contract)
integrations/   install glue: common.sh + caelestia/ standalone/ swww-overlay/
install.sh      top-level dispatcher (detect → confirm → delegate)
docs/superpowers/specs/   design docs
```

One binary serves all integrations. #1 caelestia and #2 standalone are the same
render mode (opaque, cozy owns the wallpaper); #3 swww-overlay runs the
`--overlay` mode (transparent premultiplied-alpha rain over an external
daemon's wallpaper). Adding an effect = one `shaders/effects/<name>.frag` + one
row in the `EFFECTS` table in `src/render/gl.rs`; in overlay mode it must also
output a coverage alpha (`u_overlay`).

## Conventions

- **Installers:** shared logic (preflight, fetch, build, install binary) lives
  once in `integrations/common.sh`; each integration's `install.sh` sources it
  and adds only its own glue. Do not duplicate that logic back into the
  integrations. The three `cozy-session` launchers are intentionally separate.
- **Touching the user's config is sacred:** never overwrite a user's
  `hyprland.conf` / Caelestia JSON. Append a single sourced line or edit one
  key, back up to `*.cozy-bak` first, and stay idempotent (detect-and-skip on
  re-run). Files cozy fully owns (e.g. `~/.config/cozy/hyprland.conf`) may be
  rewritten.
- **Shell:** scripts are `set -euo pipefail` bash (or POSIX `sh` for the small
  launchers/wrappers). Interactive prompts read from `/dev/tty` (they run under
  `curl | bash`). Run `bash -n` / `sh -n` and shellcheck before claiming done.
- **License:** cozy's code is MIT, **except** `shaders/effects/droplet.frag`
  (ported from "Heartfelt" by BigWings) which is **CC BY-NC-SA 3.0** — keep that
  attribution and don't relicense derivatives of it.

## Gotchas

- `cozy set/effect/weather` only talk to a *running* daemon over the socket
  (`$XDG_RUNTIME_DIR/cozy/cozy.sock`); the daemon takes its wallpaper as a
  startup arg and does **not** persist it — persistence is the installers' job
  (`cozy.conf` + `cozy-wall`, or the Caelestia `path.txt`/postHook).
- Default branch is `main`. Installer/script URLs and `COZY_REF` must point at
  `main`, never a feature branch.
