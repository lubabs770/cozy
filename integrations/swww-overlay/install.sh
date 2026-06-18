#!/usr/bin/env bash
#
# cozy — swww-overlay integration (cozy rendering *alongside* swww/hyprpaper).
#
# cozy runs in --overlay mode: a transparent rain layer above the wallpaper
# drawn by your existing swww/hyprpaper daemon, with droplets refracting a synced
# copy of the same image (Plan A). Your daemon keeps owning the wallpaper (and
# its transitions); cozy just rains on top.
#
# Normally selected for you by the top-level dispatcher:
#   curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash
# Or run directly from a checkout:  integrations/swww-overlay/install.sh
#
# Your hyprland.conf is touched in exactly one way: a single appended `source`
# line, backed up to *.cozy-bak first and skipped if already present.
#
# Env overrides: COZY_REPO / COZY_REF / COZY_PREFIX /
#   COZY_KEYBINDS=preshipped|custom  (skip the interactive keybind prompt)

set -euo pipefail

# --- locate + source the shared installer machinery ------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
if [ ! -r "$SCRIPT_DIR/../common.sh" ]; then
    echo "Error: run this from a cozy checkout, or use the top-level installer:" >&2
    echo "  curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash" >&2
    exit 1
fi
# shellcheck source=../common.sh
. "$SCRIPT_DIR/../common.sh"

# --- swww-overlay-specific config ------------------------------------------
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/cozy"
COZY_CONF="$CONFIG_DIR/cozy.conf"
COZY_HYPR="$CONFIG_DIR/hyprland.conf"
HYPR_CONF="${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hyprland.conf"

# --- build + install the binary (shared) -----------------------------------
cozy_preflight
command -v hyprctl >/dev/null 2>&1 || warn "hyprctl not found — this integration targets Hyprland (continuing)."
if ! command -v swww >/dev/null 2>&1 && ! command -v hyprpaper >/dev/null 2>&1; then
    warn "Neither swww nor hyprpaper found. This integration runs cozy *over* a"
    warn "wallpaper daemon — install and start one, or use the standalone integration."
fi
cozy_fetch_sources
cozy_build
cozy_install_binary

# --- install launcher + wallpaper helper -----------------------------------
step "Installing launcher + helper"
install -m 0755 "$SRC_DIR/integrations/swww-overlay/cozy-session" "$BIN_DIR/cozy-session"
install -m 0755 "$SRC_DIR/integrations/swww-overlay/cozy-wall"    "$BIN_DIR/cozy-wall"
ok "cozy-session, cozy-wall -> $BIN_DIR"

# --- starter config --------------------------------------------------------
step "Writing config"
mkdir -p "$CONFIG_DIR"
if [ -f "$COZY_CONF" ]; then
    ok "Config already exists — left as is ($COZY_CONF)"
else
    cat > "$COZY_CONF" <<'EOF'
# cozy configuration (swww-overlay) — sourced by cozy-session at startup.
# Plain shell "key=value" pairs; edit freely.

# Wallpaper cozy refracts. Keep this the SAME image your swww/hyprpaper daemon
# shows, or droplets will refract a different picture than the background.
# Change both at once (and update this file) with:  cozy-wall <path>
wallpaper=""

# Rain/snow effect. In overlay mode these composite over your wallpaper:
#   snow | classic | sleet
# (droplet | ripple | pouring currently render opaque in overlay — they cover
#  the wallpaper — pending per-effect transparency.)
effect="classic"

# Weather knobs applied at startup (also settable live: cozy weather …).
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
# cozy — Hyprland integration, swww-overlay (managed by the cozy installer).
# Sourced from your main hyprland.conf via a single \`source\` line; safe to edit.

# Start cozy as a transparent rain overlay above your wallpaper daemon. Keep
# running swww/hyprpaper — cozy does NOT replace it in this mode.
exec-once = $BIN_DIR/cozy-session

# --- cozy keybinds (preshipped — edit keys/paths to taste) ------------------
# Change the wallpaper on BOTH the daemon and cozy, and remember it:
bind = \$mainMod, W, exec, $BIN_DIR/cozy-wall ~/Pictures/wallpaper.jpg
# Switch the rain effect live (overlay-friendly: snow | classic | sleet):
bind = \$mainMod, R, exec, cozy effect snow
bind = \$mainMod SHIFT, R, exec, cozy effect classic
EOF
    ok "Wrote $COZY_HYPR (with preshipped keybinds)"
else
    cat > "$COZY_HYPR" <<EOF
# cozy — Hyprland integration, swww-overlay (managed by the cozy installer).
# Sourced from your main hyprland.conf via a single \`source\` line; safe to edit.

# Start cozy as a transparent rain overlay above your wallpaper daemon. Keep
# running swww/hyprpaper — cozy does NOT replace it in this mode.
exec-once = $BIN_DIR/cozy-session

# --- cozy keybinds (examples — uncomment / edit, or define your own) --------
# Change the wallpaper on BOTH the daemon and cozy, and remember it:
# bind = \$mainMod, W, exec, $BIN_DIR/cozy-wall ~/Pictures/wallpaper.jpg
# Switch the rain effect live (overlay-friendly: snow | classic | sleet):
# bind = \$mainMod, R, exec, cozy effect snow
# bind = \$mainMod SHIFT, R, exec, cozy effect classic
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
    printf '\n# cozy rain overlay (added by the cozy installer)\n%s\n' "$SOURCE_LINE" >> "$HYPR_CONF"
    ok "Appended source line (backup: $(basename "$HYPR_CONF").cozy-bak)"
fi

# --- done ------------------------------------------------------------------
echo
printf '%scozy is installed as a rain overlay (swww-overlay).%s\n' "$GRN$BOLD" "$RST"
info "Keep running swww/hyprpaper — cozy rains on top of it."
info "Change wallpaper (daemon + cozy):  cozy-wall ~/Pictures/your-wall.jpg"
info "Then reload Hyprland (hyprctl reload) or relog to start cozy."
info "Overlay-friendly effects:          cozy effect snow|classic|sleet"
info "Config:                            $COZY_CONF"
