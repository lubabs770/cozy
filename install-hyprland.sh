#!/usr/bin/env bash
#
# cozy installer for plain Hyprland (no Caelestia, no other dotfiles).
#
#   curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install-hyprland.sh | bash
#
# What it does (all idempotent — safe to re-run, e.g. to update):
#   1. Clones/updates the repo into  $XDG_DATA_HOME/cozy/src
#   2. Builds the release binary and installs  cozy + cozy-session + cozy-wall
#      to  ~/.local/bin
#   3. Writes a starter config at  ~/.config/cozy/cozy.conf  (only if absent)
#   4. Writes  ~/.config/cozy/hyprland.conf  (exec-once + optional keybinds)
#   5. Adds ONE `source = …` line to your real hyprland.conf (backed up first)
#
# Your hyprland.conf is touched in exactly one way: a single appended `source`
# line. It is backed up to *.cozy-bak first, and re-running detects the line
# and skips it. To uninstall, delete that line and the ~/.config/cozy files.
#
# Nothing here needs root. Override defaults with env vars:
#   COZY_REPO     git URL              (default: https://github.com/lubabs770/cozy.git)
#   COZY_REF      branch/tag/commit    (default: main)
#   COZY_PREFIX   install dir          (default: ~/.local)
#   COZY_KEYBINDS preshipped | custom  (skip the interactive prompt)

set -euo pipefail

# --- config ----------------------------------------------------------------
COZY_REPO="${COZY_REPO:-https://github.com/lubabs770/cozy.git}"
COZY_REF="${COZY_REF:-main}"
COZY_PREFIX="${COZY_PREFIX:-$HOME/.local}"

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/cozy"
SRC_DIR="$DATA_DIR/src"
BIN_DIR="$COZY_PREFIX/bin"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/cozy"
COZY_CONF="$CONFIG_DIR/cozy.conf"
COZY_HYPR="$CONFIG_DIR/hyprland.conf"
HYPR_CONF="${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hyprland.conf"

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

# --- preflight -------------------------------------------------------------
step "Checking environment"

[ "$(uname -s)" = "Linux" ] || die "cozy is Linux/Wayland only (this is $(uname -s))."
if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    warn "WAYLAND_DISPLAY is unset — cozy needs a Wayland session at runtime."
fi
command -v hyprctl >/dev/null 2>&1 || warn "hyprctl not found — this installer targets Hyprland (it will still install)."

missing=""
for tool in git cargo; do
    command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
    info "Missing required tools:$missing"
    if   command -v pacman >/dev/null 2>&1; then info "Install:  sudo pacman -S --needed${missing/cargo/ rust} git"
    elif command -v apt    >/dev/null 2>&1; then info "Install:  sudo apt install${missing/cargo/ cargo} git"
    elif command -v dnf    >/dev/null 2>&1; then info "Install:  sudo dnf install${missing/cargo/ cargo} git"
    fi
    die "Install the tools above and re-run."
fi
ok "git, cargo present"

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

# --- install binary + helpers ----------------------------------------------
step "Installing into $COZY_PREFIX"
mkdir -p "$BIN_DIR"
install -m 0755 "$SRC_DIR/target/release/cozy" "$BIN_DIR/cozy"
install -m 0755 "$SRC_DIR/dist/cozy-session-hypr" "$BIN_DIR/cozy-session"
install -m 0755 "$SRC_DIR/dist/cozy-wall"         "$BIN_DIR/cozy-wall"
ok "cozy, cozy-session, cozy-wall -> $BIN_DIR"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) warn "$BIN_DIR is not on your PATH — add it (e.g. in ~/.profile) so keybinds can find cozy-wall." ;;
esac

# --- starter config --------------------------------------------------------
step "Writing config"
mkdir -p "$CONFIG_DIR"
if [ -f "$COZY_CONF" ]; then
    ok "Config already exists — left as is ($COZY_CONF)"
else
    cat > "$COZY_CONF" <<'EOF'
# cozy configuration — sourced by cozy-session at startup.
# Plain shell "key=value" pairs; edit freely.

# Wallpaper to show. Absolute path or ~/...  Empty = cozy's embedded image.
# Tip: change this live (and update this file) with:  cozy-wall <path>
wallpaper=""

# Rain/snow effect: droplet | classic | pouring | ripple | sleet | snow
effect="droplet"

# Weather knobs applied at startup (also settable live: cozy weather …).
#   wind   = horizontal skew of the rain
#   precip = rain/snow intensity
wind="0.0"
precip="0.6"
EOF
    ok "Wrote starter config ($COZY_CONF)"
fi

# --- keybinds choice -------------------------------------------------------
step "Hyprland keybinds"
choice="${COZY_KEYBINDS:-}"
if [ -z "$choice" ]; then
    if [ -r /dev/tty ]; then
        printf '    Use cozy'\''s preshipped keybinds? You can edit them anytime in\n'
        printf '    %s\n' "$COZY_HYPR"
        printf '    %s[P]%s preshipped   %s[c]%s I'\''ll set my own  ' "$BOLD" "$RST" "$BOLD" "$RST"
        read -r reply </dev/tty || reply=""
        case "$reply" in
            c|C|custom) choice="custom" ;;
            *)          choice="preshipped" ;;
        esac
    else
        choice="custom"
        warn "Non-interactive — defaulting to commented (custom) keybinds."
    fi
fi

# --- cozy's own hyprland snippet (we fully own this file) ------------------
if [ "$choice" = "preshipped" ]; then
    cat > "$COZY_HYPR" <<EOF
# cozy — Hyprland integration (managed by install-hyprland.sh; safe to edit).
# This file is sourced from your main hyprland.conf via a single \`source\` line.

# Start the cozy wallpaper engine. cozy owns the wallpaper, so do NOT also run
# hyprpaper / swww.
exec-once = $BIN_DIR/cozy-session

# --- cozy keybinds (preshipped — edit keys/paths to taste) ------------------
# Change the wallpaper live AND remember it across reboots:
bind = \$mainMod, W, exec, $BIN_DIR/cozy-wall ~/Pictures/wallpaper.jpg
# Switch the rain effect live (droplet | classic | pouring | ripple | sleet | snow):
bind = \$mainMod, R, exec, cozy effect classic
bind = \$mainMod SHIFT, R, exec, cozy effect droplet
EOF
    ok "Wrote $COZY_HYPR (with preshipped keybinds)"
else
    cat > "$COZY_HYPR" <<EOF
# cozy — Hyprland integration (managed by install-hyprland.sh; safe to edit).
# This file is sourced from your main hyprland.conf via a single \`source\` line.

# Start the cozy wallpaper engine. cozy owns the wallpaper, so do NOT also run
# hyprpaper / swww.
exec-once = $BIN_DIR/cozy-session

# --- cozy keybinds (examples — uncomment / edit, or define your own) --------
# Change the wallpaper live AND remember it across reboots:
# bind = \$mainMod, W, exec, $BIN_DIR/cozy-wall ~/Pictures/wallpaper.jpg
# Switch the rain effect live (droplet | classic | pouring | ripple | sleet | snow):
# bind = \$mainMod, R, exec, cozy effect classic
# bind = \$mainMod SHIFT, R, exec, cozy effect droplet
EOF
    ok "Wrote $COZY_HYPR (keybinds commented — set your own)"
fi

# --- wire it into the real hyprland.conf (one line) ------------------------
step "Linking into hyprland.conf"
SOURCE_LINE="source = $COZY_HYPR"
if [ ! -f "$HYPR_CONF" ]; then
    warn "No hyprland.conf at $HYPR_CONF — not creating one."
    info "Add this line to your Hyprland config yourself:"
    info "    $SOURCE_LINE"
elif grep -qF "$COZY_HYPR" "$HYPR_CONF"; then
    ok "hyprland.conf already sources cozy — left as is"
else
    [ -f "$HYPR_CONF.cozy-bak" ] || cp "$HYPR_CONF" "$HYPR_CONF.cozy-bak"
    printf '\n# cozy wallpaper engine (added by install-hyprland.sh)\n%s\n' "$SOURCE_LINE" >> "$HYPR_CONF"
    ok "Appended source line (backup: $(basename "$HYPR_CONF").cozy-bak)"
fi

# --- done ------------------------------------------------------------------
echo
printf '%scozy is installed for Hyprland.%s\n' "$GRN$BOLD" "$RST"
info "Set a wallpaper now:   cozy-wall ~/Pictures/your-wall.jpg"
info "Then reload Hyprland (hyprctl reload) or relog to start cozy."
info "Switch effects live:   cozy effect snow"
info "Config:                $COZY_CONF"
