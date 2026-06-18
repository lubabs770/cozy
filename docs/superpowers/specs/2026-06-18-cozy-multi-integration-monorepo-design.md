# cozy multi-integration monorepo — design

**Date:** 2026-06-18
**Status:** approved-pending-review

## Goal

Restructure cozy so one engine cleanly supports three integration profiles
without their glue getting crossed, and add a Plan-A `--overlay` render mode so
cozy can run *alongside* a wallpaper daemon (swww / hyprpaper) instead of only
*instead of* one.

The three profiles:

1. **caelestia** — cozy owns the wallpaper (opaque mode); systemd user service;
   wires into Caelestia's `cli.json` postHook + `shell.json`; seeds from
   Caelestia's `path.txt`. (Exists today as `install.sh` + `dist/cozy-session` +
   `dist/cozy.service`.)
2. **standalone** — cozy owns the wallpaper (opaque mode); `exec-once` in
   hyprland.conf; `cozy.conf`; `cozy-wall` drives cozy only. (Exists today as
   `install-hyprland.sh` + `dist/cozy-session-hypr` + `dist/cozy-wall`.)
3. **swww-overlay** — NEW. cozy runs in `--overlay` mode (transparent, Bottom
   layer) above an existing swww/hyprpaper wallpaper; `cozy-wall` sets both the
   daemon's wallpaper and cozy's refraction source.

Key framing: #1 and #2 are the *same* render mode (opaque); they differ only in
install glue. #3 is the only profile needing a new binary capability.

## Non-goals

- No Cargo workspace / multi-crate split — the engine is shared, one binary.
- The swww-overlay installer does NOT install or autostart swww/hyprpaper; it
  assumes the user already runs their daemon.
- No screen-capture (`wlr-screencopy`) refraction. Overlay refraction uses a
  user-supplied copy of the wallpaper image (Plan A).

## Repo layout (target)

```
cozy/
  src/                         the one engine (+ --overlay mode, Phase 2)
  shaders/
    rain.vert
    effects/*.frag             gain a premultiplied-alpha path for overlay (Phase 2)
  integrations/
    common.sh                  shared installer lib (preflight, fetch, build, install binary)
    caelestia/
      install.sh   cozy-session   cozy.service
    standalone/
      install.sh   cozy-session   cozy-wall
    swww-overlay/
      install.sh   cozy-session   cozy-wall
  install.sh                   dispatcher: detect -> confirm -> run integrations/<n>/install.sh
  llms.txt                     project map for LLM/tool consumers
  AGENTS.md                    build/verify/conventions for coding agents
  README.md
```

`dist/` is removed; its files move into the matching integration folders:
- `dist/cozy-session` + `dist/cozy.service` -> `integrations/caelestia/`
- `dist/cozy-session-hypr` -> `integrations/standalone/cozy-session`
- `dist/cozy-wall` -> `integrations/standalone/cozy-wall`

Anti-cross-contamination rule: each integration owns its own copy of
`cozy-session` / `cozy-wall`, and only one integration is ever installed, so
there is no name collision in `~/.local/bin`. The three `cozy-session`
launchers are kept SEPARATE (not factored) — they are short and self-contained,
which is the point.

## Shared installer lib — `integrations/common.sh`

The parts identical across all three installers are factored here and sourced by
each `integrations/<n>/install.sh`:

- pretty-output helpers (`step`/`info`/`ok`/`warn`/`die`)
- preflight: Linux check, `git`/`cargo` presence (+ distro install hints), Wayland warning
- fetch sources (clone/update into `$XDG_DATA_HOME/cozy/src`, honoring `COZY_REPO`/`COZY_REF`)
- build (`cargo build --release` + dev-header hints on failure)
- install the `cozy` binary into `$COZY_PREFIX/bin`

Each integration's `install.sh` then does only its own glue. This single source
of truth prevents drift (e.g. the `gamma`->`main` ref bug would be a one-line
fix in one place).

## Binary: `--overlay` mode (Plan A) — Phase 2

One new flag. Default unchanged (opaque) so #1/#2 are byte-for-byte behavior.

| aspect | opaque (default) | `--overlay` |
|---|---|---|
| layer-shell layer | Background | Bottom |
| opaque region (`app.rs`) | whole screen | not set |
| clear color | (opaque draw) | transparent `(0,0,0,0)` |
| shader final output | `vec4(rgb, 1.0)` | `vec4(rgb*a, a)` (premultiplied) |
| wallpaper texture | drawn as opaque base | loaded only as refraction source |

Implementation notes:
- EGL config already requests `ALPHA_SIZE 8` (`render/egl.rs`), so no config
  change is needed for an alpha framebuffer.
- Add a `u_overlay` uniform (or shader `#define` variant) consumed by each
  effect. Each effect must define its **coverage alpha**: where it is opaque
  (droplets, streaks, flakes) vs transparent (gaps). Refraction effects keep
  sampling `u_wallpaper`, so droplets look the same — they just composite over
  the daemon's wallpaper via the compositor instead of cozy's own base.
- Wayland expects premultiplied alpha; outputs must be `rgb*a`.

Verification of this mode needs the Linux container harness (`make verify`) —
the only part of the project that needs real GPU/compositor checking.

> **Status (done):** every effect ships overlay coverage. `snow` carries alpha
> on the flakes; `droplet` and `ripple` derive coverage from their own rain
> signal (drop mask + trails + lens slope; ripple height + displacement +
> streaks). Earlier generic `length(color - plain)` coverage was abandoned — it
> amplified wallpaper detail into grid/X artifacts; internal-signal coverage is
> clean (verified visually in the container).
>
> (The hand-built `classic`, `pouring`, and `sleet` effects were later removed in
> favour of the ported `droplet`/`snow` + `ripple`; they also had overlay paths.)

## swww-overlay integration — Phase 2

- launcher `cozy-session`: sources `cozy.conf`, runs `cozy --overlay
  --wallpaper "$wallpaper"`, waits for the socket, applies `effect`/`weather`.
- `cozy-wall <path>`: auto-detects the running daemon and drives it, then always
  sets cozy's refraction source:
  ```sh
  if swww is running/installed:   swww img "$wall"
  elif hyprpaper:                 hyprctl hyprpaper wallpaper ",$wall"
  cozy set "$wall"                # refraction source (always)
  ```
  Also persists `wallpaper=` into `cozy.conf` (same as standalone).
- `install.sh`: appends the one-line `source = ~/.config/cozy/hyprland.conf`
  into the user's hyprland.conf (backed up, idempotent), writes cozy's hyprland
  snippet with `exec-once = .../cozy-session` and keybinds (preshipped/custom
  prompt, as in standalone).

## Dispatcher `install.sh`

Detect -> confirm -> delegate.

- Detection: Caelestia config dir present -> suggest #1; `swww`/`hyprpaper`
  running or installed -> suggest #3; else -> #2.
- Shows the guess; user confirms or picks 1/2/3 (reads `/dev/tty` for
  `curl|bash`). `COZY_INTEGRATION=caelestia|standalone|swww` skips the prompt
  for scripted installs; non-interactive with no env -> default to standalone
  with a warning.
- Runs `integrations/<choice>/install.sh` (passing through env like
  `COZY_REF`).

## README & URLs

- One dispatcher quick-start: `curl -fsSL .../main/install.sh | bash`.
- A short subsection per integration with its direct
  `integrations/<n>/install.sh` URL for users who want to skip detection.
- The old top-level `install-hyprland.sh` URL is removed (replaced by the
  dispatcher or `integrations/standalone/install.sh`). The old top-level
  `install.sh` (was Caelestia) becomes the dispatcher — an old Caelestia
  `curl|bash` now lands on detect-then-confirm, which will still suggest #1.

## LLM docs (Phase 1)

- `llms.txt` — concise project map: what cozy is, the architecture, the three
  integrations, links to key files. Reader/tool-facing.
- `AGENTS.md` — coding-agent guide: build (`cargo build --release`), lint/verify
  (`make verify`, `make lint`), the integrations layout, conventions, and
  gotchas (Linux/Wayland only; verified via container harness; cozy owns the
  wallpaper in opaque mode).

## Phasing

**Phase 1 — restructure (no binary changes):**
- create `integrations/` tree, move `dist/` files in, write `common.sh`,
  refactor the two existing installers to source it.
- add the dispatcher `install.sh`.
- add a `swww-overlay/` stub installer that errors with "requires Phase 2 /
  `--overlay` (not yet released)" so the tree is complete but honest.
- add `llms.txt` + `AGENTS.md`; restructure README.
- verify both existing installers still work (syntax + logic; full run needs
  Linux).

**Phase 2 — overlay mode:**
- implement `--overlay` in the binary (layer, opaque region, clear color).
- add premultiplied-alpha / coverage path to each effect shader behind
  `u_overlay`.
- flesh out the `swww-overlay` integration (launcher with `--overlay`,
  dual-daemon `cozy-wall`, installer).
- verify in the container harness (`make verify`) that overlay composites over a
  background and that droplets still refract.

## Open risks

- Overlay shader coverage-alpha tuning per effect (especially additive streaks
  over arbitrary wallpapers) may need visual iteration in the container.
- Two-sources-of-truth for the wallpaper in overlay mode (daemon + cozy copy):
  mitigated by `cozy-wall` setting both atomically; documented as a known
  constraint.
