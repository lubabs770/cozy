#!/usr/bin/env bash
#
# cozy installer — builds cozy and wires it into a Caelestia + Hyprland setup so
# it transparently takes over wallpaper duties.
#
#   curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/gamma/install.sh | bash
#
# What it does (all idempotent — safe to re-run, e.g. to update):
#   1. Clones/updates the repo into  $XDG_DATA_HOME/cozy/src
#   2. Builds the release binary and installs it to  ~/.local/bin/cozy
#   3. Installs the  cozy-session  launcher + a systemd --user unit, and enables it
#   4. Appends  `cozy set "$WALLPAPER_PATH"`  to your Caelestia wallpaper postHook
#      (~/.config/caelestia/cli.json) so every wallpaper change flows into cozy
#   5. Seeds cozy with your current Caelestia wallpaper
#
# Nothing here needs root. Override defaults with env vars:
#   COZY_REPO   git URL           (default: https://github.com/lubabs770/cozy.git)
#   COZY_REF    branch/tag/commit (default: gamma)
#   COZY_PREFIX install dir       (default: ~/.local)

set -euo pipefail

# --- config ----------------------------------------------------------------
COZY_REPO="${COZY_REPO:-https://github.com/lubabs770/cozy.git}"
COZY_REF="${COZY_REF:-gamma}"
COZY_PREFIX="${COZY_PREFIX:-$HOME/.local}"

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/cozy"
SRC_DIR="$DATA_DIR/src"
BIN_DIR="$COZY_PREFIX/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

CAELESTIA_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/caelestia/cli.json"
CAELESTIA_SHELL_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/caelestia/shell.json"
CAELESTIA_WALL="${XDG_STATE_HOME:-$HOME/.local/state}/caelestia/wallpaper/path.txt"

COZY_HOOK='cozy set "$WALLPAPER_PATH"'

# --- pretty output ---------------------------------------------------------
if [ -t 1 ]; then
    BOLD=$'\033[1m'; DIM=$'\033[2m'; RED=$'\033[31m'; GRN=$'\033[32m'
    YLW=$'\033[33m'; BLU=$'\033[34m'; RST=$'\033[0m'
else
    BOLD=''; DIM=''; RED=''; GRN=''; YLW=''; BLU=''; RST=''
fi
step() { printf '%s==>%s %s\n' "$BLU$BOLD" "$RST$BOLD" "$*$RST"; }
info() { printf '    %s\n' "$*"; }
ok()   { printf '    %s✓%s %s\n' "$GRN" "$RST" "$*"; }
warn() { printf '    %s!%s %s\n' "$YLW" "$RST" "$*"; }
die()  { printf '%sError:%s %s\n' "$RED$BOLD" "$RST" "$*" >&2; exit 1; }

# Apply a jq filter to a JSON file in place. We write *through* the path with
# `>` (which follows a symlink to its target) rather than `mv` (which would
# replace the symlink with a regular file) — so a dotfiles-managed / symlinked
# shell.json or cli.json keeps its link.
#   write_json <file> <jq-filter> [extra jq args…]
write_json() {
    local file="$1" filter="$2"; shift 2
    local tmp; tmp="$(mktemp)"
    if jq "$@" "$filter" "$file" > "$tmp"; then
        cat "$tmp" > "$file"; rm -f "$tmp"
    else
        rm -f "$tmp"; return 1
    fi
}

# --- preflight -------------------------------------------------------------
step "Checking environment"

[ "$(uname -s)" = "Linux" ] || die "cozy is Linux/Wayland only (this is $(uname -s))."
if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    warn "WAYLAND_DISPLAY is unset — cozy needs a Wayland session at runtime."
fi

missing=""
for tool in git cargo jq; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
    info "Missing required tools:$missing"
    if   command -v pacman >/dev/null 2>&1; then info "Install:  sudo pacman -S --needed${missing/cargo/ rust}${missing:+ }git jq"
    elif command -v apt    >/dev/null 2>&1; then info "Install:  sudo apt install${missing/cargo/ cargo} git jq"
    elif command -v dnf    >/dev/null 2>&1; then info "Install:  sudo dnf install${missing/cargo/ cargo} git jq"
    fi
    die "Install the tools above and re-run."
fi
ok "git, cargo, jq present"

# --- fetch sources ---------------------------------------------------------
step "Fetching cozy ($COZY_REF)"
mkdir -p "$DATA_DIR"
if [ -d "$SRC_DIR/.git" ]; then
    info "Updating existing clone at $SRC_DIR"
    git -C "$SRC_DIR" remote set-url origin "$COZY_REPO"
    git -C "$SRC_DIR" fetch --depth 1 origin "$COZY_REF"
    git -C "$SRC_DIR" checkout -q FETCH_HEAD
else
    git clone --depth 1 --branch "$COZY_REF" "$COZY_REPO" "$SRC_DIR" 2>/dev/null \
        || git clone "$COZY_REPO" "$SRC_DIR"
    git -C "$SRC_DIR" checkout -q "$COZY_REF" 2>/dev/null || true
fi
ok "Source at $SRC_DIR"

# --- build -----------------------------------------------------------------
step "Building (cargo build --release)"
info "First build pulls crates and can take a few minutes…"
if ! cargo build --release --manifest-path "$SRC_DIR/Cargo.toml"; then
    echo
    warn "Build failed — usually missing Wayland/EGL development headers."
    if   command -v pacman >/dev/null 2>&1; then info "Try:  sudo pacman -S --needed wayland libxkbcommon mesa libglvnd"
    elif command -v apt    >/dev/null 2>&1; then info "Try:  sudo apt install libwayland-dev libwayland-egl1 libxkbcommon-dev wayland-protocols libegl-dev libgles-dev libgl1-mesa-dri libglvnd-dev pkg-config"
    elif command -v dnf    >/dev/null 2>&1; then info "Try:  sudo dnf install wayland-devel libxkbcommon-devel mesa-libEGL-devel mesa-libGLES-devel libglvnd-devel pkgconf-pkg-config"
    fi
    die "Install the dev headers above and re-run."
fi
ok "Built $SRC_DIR/target/release/cozy"

# --- install binary + launcher ---------------------------------------------
step "Installing into $COZY_PREFIX"
mkdir -p "$BIN_DIR"
install -m 0755 "$SRC_DIR/target/release/cozy" "$BIN_DIR/cozy"
install -m 0755 "$SRC_DIR/dist/cozy-session"   "$BIN_DIR/cozy-session"
ok "cozy + cozy-session -> $BIN_DIR"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) warn "$BIN_DIR is not on your PATH — add it (e.g. in ~/.profile) so the postHook can find cozy." ;;
esac

# --- systemd user service --------------------------------------------------
step "Installing systemd --user service"
mkdir -p "$UNIT_DIR"
install -m 0644 "$SRC_DIR/dist/cozy.service" "$UNIT_DIR/cozy.service"
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload
    systemctl --user enable cozy.service >/dev/null 2>&1 \
        && ok "cozy.service enabled (starts with your graphical session)" \
        || warn "Could not enable cozy.service automatically — run: systemctl --user enable --now cozy.service"
else
    warn "systemctl not found — add 'exec-once = $BIN_DIR/cozy-session' to hyprland.conf instead."
fi

# --- wire the Caelestia postHook -------------------------------------------
step "Wiring the Caelestia wallpaper hook"
mkdir -p "$(dirname "$CAELESTIA_CONFIG")"
[ -f "$CAELESTIA_CONFIG" ] || echo '{}' > "$CAELESTIA_CONFIG"

if jq -e '.wallpaper.postHook // "" | test("cozy set")' "$CAELESTIA_CONFIG" >/dev/null 2>&1; then
    ok "postHook already calls cozy — left as is"
else
    [ -f "$CAELESTIA_CONFIG.cozy-bak" ] || cp "$CAELESTIA_CONFIG" "$CAELESTIA_CONFIG.cozy-bak"
    write_json "$CAELESTIA_CONFIG" '
        .wallpaper //= {}
        | .wallpaper.postHook =
            ( (.wallpaper.postHook // "") as $e
              | if   $e == "" then $hook
                else $e + "\n" + $hook end )
    ' --arg hook "$COZY_HOOK"
    ok "Appended 'cozy set \"\$WALLPAPER_PATH\"' (backup: $(basename "$CAELESTIA_CONFIG").cozy-bak)"
fi

# --- stop the shell from drawing its own wallpaper -------------------------
# The Caelestia shell renders the wallpaper itself (modules/background/Wallpaper.qml).
# cozy must be the sole wallpaper renderer, so turn off just that — BackgroundConfig
# .wallpaperEnabled — which leaves the rest of the background module (desktop clock,
# visualiser) intact. shell.json is live-reloaded, so this needs no restart.
step "Disabling the Caelestia shell's own wallpaper"
mkdir -p "$(dirname "$CAELESTIA_SHELL_CONFIG")"
[ -f "$CAELESTIA_SHELL_CONFIG" ] || echo '{}' > "$CAELESTIA_SHELL_CONFIG"

# Note: a plain `// false` test is wrong here — jq's // treats the literal
# `false` as empty — so compare against false explicitly.
if jq -e '.background.wallpaperEnabled == false' "$CAELESTIA_SHELL_CONFIG" >/dev/null 2>&1; then
    ok "Shell wallpaper already disabled — left as is"
else
    # Only back up the pristine original; never let a re-run clobber it.
    [ -f "$CAELESTIA_SHELL_CONFIG.cozy-bak" ] || cp "$CAELESTIA_SHELL_CONFIG" "$CAELESTIA_SHELL_CONFIG.cozy-bak"
    write_json "$CAELESTIA_SHELL_CONFIG" '.background //= {} | .background.wallpaperEnabled = false'
    ok "Set background.wallpaperEnabled=false (backup: $(basename "$CAELESTIA_SHELL_CONFIG").cozy-bak)"
fi

# --- start now + seed current wallpaper ------------------------------------
step "Starting cozy"
if command -v systemctl >/dev/null 2>&1 && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    systemctl --user restart cozy.service \
        && ok "cozy is running" \
        || warn "Could not start now — it will start on next login, or run: systemctl --user start cozy.service"
else
    info "Not in a Wayland session — cozy will start on next graphical login."
fi

if [ -r "$CAELESTIA_WALL" ]; then
    ok "Found current Caelestia wallpaper: $(cat "$CAELESTIA_WALL")"
else
    info "No Caelestia wallpaper recorded yet — cozy shows its embedded image until you set one."
fi

# --- done ------------------------------------------------------------------
echo
printf '%scozy is installed and wired into Caelestia.%s\n' "$GRN$BOLD" "$RST"
echo
info "Change wallpaper as usual:   ${BOLD}caelestia wallpaper${RST}   (cozy picks it up live)"
info "Switch the rain effect:      ${BOLD}cozy effect droplet|classic|pouring|snow|sleet${RST}"
info "Tune wind / intensity:       ${BOLD}cozy weather --wind 0.4 --precip 0.9${RST}"
echo
info "The shell's own wallpaper was turned off (background.wallpaperEnabled=false) so"
info "cozy is the sole wallpaper renderer. Re-enable it any time by restoring the"
info "${DIM}.cozy-bak${RST} backups next to your Caelestia ${BOLD}cli.json${RST} / ${BOLD}shell.json${RST}."
