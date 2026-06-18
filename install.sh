#!/usr/bin/env bash
#
# cozy — top-level installer / dispatcher.
#
#   curl -fsSL https://raw.githubusercontent.com/lubabs770/cozy/main/install.sh | bash
#
# cozy supports three integrations:
#   1. caelestia     cozy owns the wallpaper, wired into the Caelestia dotfiles
#   2. standalone    cozy owns the wallpaper on plain Hyprland
#   3. swww-overlay  cozy rains *over* swww/hyprpaper (overlay mode)  [Phase 2]
#
# This script clones the repo, detects your likely setup, lets you confirm or
# override, and then runs the matching integration installer. Skip the prompt
# with  COZY_INTEGRATION=caelestia|standalone|swww .
#
# Env overrides: COZY_REPO / COZY_REF / COZY_PREFIX / COZY_INTEGRATION.

set -euo pipefail

COZY_REPO="${COZY_REPO:-https://github.com/lubabs770/cozy.git}"
COZY_REF="${COZY_REF:-main}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/cozy"
SRC_DIR="$DATA_DIR/src"

# --- bare bootstrap --------------------------------------------------------
# We need the repo on disk (for common.sh + the integration installers) before
# anything else, so this much is duplicated from common.sh by necessity.
command -v git >/dev/null 2>&1 || { echo "Error: git is required." >&2; exit 1; }
[ "$(uname -s)" = "Linux" ] || { echo "Error: cozy is Linux/Wayland only (this is $(uname -s))." >&2; exit 1; }

if [ -d "$SRC_DIR/.git" ]; then
    git -C "$SRC_DIR" remote set-url origin "$COZY_REPO"
    git -C "$SRC_DIR" fetch --depth 1 origin "$COZY_REF" >/dev/null 2>&1
    git -C "$SRC_DIR" checkout -q FETCH_HEAD
else
    mkdir -p "$DATA_DIR"
    git clone --depth 1 --branch "$COZY_REF" "$COZY_REPO" "$SRC_DIR" >/dev/null 2>&1 \
        || git clone "$COZY_REPO" "$SRC_DIR" >/dev/null 2>&1 \
        || { echo "Error: failed to clone $COZY_REPO" >&2; exit 1; }
fi

# Now the shared machinery is available. The integration installer we delegate
# to will reuse this checkout instead of fetching again.
# shellcheck source=integrations/common.sh
. "$SRC_DIR/integrations/common.sh"
export COZY_SKIP_FETCH=1

# --- detect the likely integration -----------------------------------------
# Echoes a key (caelestia|standalone|swww) and sets DETECT_REASON.
DETECT_REASON=""
detect_integration() {
    if [ -d "${XDG_CONFIG_HOME:-$HOME/.config}/caelestia" ]; then
        DETECT_REASON="Caelestia config found at ${XDG_CONFIG_HOME:-$HOME/.config}/caelestia"
        echo caelestia; return
    fi
    if pgrep -x swww-daemon >/dev/null 2>&1 || pgrep -x hyprpaper >/dev/null 2>&1 \
       || command -v swww >/dev/null 2>&1 || command -v hyprpaper >/dev/null 2>&1; then
        DETECT_REASON="a wallpaper daemon (swww/hyprpaper) is present"
        echo swww; return
    fi
    DETECT_REASON="no Caelestia and no wallpaper daemon detected"
    echo standalone
}

label_of() {
    case "$1" in
        caelestia)  echo "caelestia    — cozy owns the wallpaper, wired into Caelestia" ;;
        standalone) echo "standalone   — cozy owns the wallpaper on plain Hyprland" ;;
        swww)       echo "swww-overlay — cozy rains as a transparent overlay over swww/hyprpaper" ;;
    esac
}

num_to_key() { case "$1" in 1) echo caelestia ;; 2) echo standalone ;; 3) echo swww ;; *) echo "" ;; esac; }
key_to_num() { case "$1" in caelestia) echo 1 ;; standalone) echo 2 ;; swww) echo 3 ;; *) echo "" ;; esac; }

# --- choose ----------------------------------------------------------------
choice="${COZY_INTEGRATION:-}"
suggested="$(detect_integration)"

if [ -n "$choice" ]; then
    case "$choice" in
        caelestia|standalone|swww) ok "Using COZY_INTEGRATION=$choice" ;;
        *) die "COZY_INTEGRATION must be one of: caelestia | standalone | swww (got '$choice')" ;;
    esac
elif [ -r /dev/tty ]; then
    step "Choose an integration"
    info "Detected: $DETECT_REASON"
    info "Suggested: ${BOLD}[$(key_to_num "$suggested")] $(label_of "$suggested")${RST}"
    echo
    info "  [1] $(label_of caelestia)"
    info "  [2] $(label_of standalone)"
    info "  [3] $(label_of swww)"
    echo
    printf '    Choose 1/2/3 [default %s]: ' "$(key_to_num "$suggested")"
    read -r reply </dev/tty || reply=""
    if [ -z "$reply" ]; then
        choice="$suggested"
    else
        choice="$(num_to_key "$reply")"
        [ -n "$choice" ] || die "Invalid choice: '$reply' (expected 1, 2, or 3)."
    fi
else
    choice="$suggested"
    warn "Non-interactive — using detected integration: $choice"
fi

# --- delegate --------------------------------------------------------------
case "$choice" in
    caelestia)  dir=caelestia ;;
    standalone) dir=standalone ;;
    swww)       dir=swww-overlay ;;
esac

step "Running the $choice integration installer"
echo
exec bash "$SRC_DIR/integrations/$dir/install.sh"
