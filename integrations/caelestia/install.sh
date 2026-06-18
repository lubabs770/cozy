#!/usr/bin/env bash
#
# cozy — Caelestia integration installer.
#
# Builds cozy and wires it into a Caelestia + Hyprland setup so it transparently
# takes over wallpaper duties (cozy owns the wallpaper / opaque render mode).
#
# Normally selected for you by the top-level dispatcher:
#   curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash
# Or run directly from a checkout:  integrations/caelestia/install.sh
#
# What it does (all idempotent — safe to re-run, e.g. to update):
#   1. Builds + installs the cozy binary (via common.sh)
#   2. Installs the cozy-session launcher + a systemd --user unit, and enables it
#   3. Appends `cozy set "$WALLPAPER_PATH"` to your Caelestia wallpaper postHook
#   4. Turns off the Caelestia shell's own wallpaper rendering
#   5. Seeds cozy with your current Caelestia wallpaper
#
# Nothing here needs root. Env overrides: COZY_REPO / COZY_REF / COZY_PREFIX.

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

# --- caelestia-specific config ---------------------------------------------
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
CAELESTIA_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/caelestia/cli.json"
CAELESTIA_SHELL_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/caelestia/shell.json"
CAELESTIA_WALL="${XDG_STATE_HOME:-$HOME/.local/state}/caelestia/wallpaper/path.txt"
COZY_HOOK='cozy set "$WALLPAPER_PATH"'

# Apply a jq filter to a JSON file in place. We write *through* the path with
# `>` (which follows a symlink to its target) rather than `mv` (which would
# replace the symlink with a regular file) — so a dotfiles-managed / symlinked
# shell.json or cli.json keeps its link.
write_json() {
    local file="$1" filter="$2"; shift 2
    local tmp; tmp="$(mktemp)"
    if jq "$@" "$filter" "$file" > "$tmp"; then
        cat "$tmp" > "$file"; rm -f "$tmp"
    else
        rm -f "$tmp"; return 1
    fi
}

# --- build + install the binary (shared) -----------------------------------
cozy_preflight jq
cozy_fetch_sources
cozy_build
cozy_install_binary

# --- install the launcher --------------------------------------------------
step "Installing the cozy-session launcher"
install -m 0755 "$SRC_DIR/integrations/caelestia/cozy-session" "$BIN_DIR/cozy-session"
ok "cozy-session -> $BIN_DIR/cozy-session"

# --- systemd user service --------------------------------------------------
step "Installing systemd --user service"
mkdir -p "$UNIT_DIR"
install -m 0644 "$SRC_DIR/integrations/caelestia/cozy.service" "$UNIT_DIR/cozy.service"
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
info "Switch the rain effect:      ${BOLD}cozy effect droplet|ripple|snow|clouds${RST}"
info "Tune wind / intensity:       ${BOLD}cozy weather --wind 0.4 --precip 0.9${RST}"
echo
info "The shell's own wallpaper was turned off (background.wallpaperEnabled=false) so"
info "cozy is the sole wallpaper renderer. Re-enable it any time by restoring the"
info "${DIM}.cozy-bak${RST} backups next to your Caelestia ${BOLD}cli.json${RST} / ${BOLD}shell.json${RST}."
