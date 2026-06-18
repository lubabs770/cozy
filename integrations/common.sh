# shellcheck shell=bash
#
# integrations/common.sh — shared installer machinery for every cozy
# integration (caelestia / standalone / swww-overlay) and the top-level
# dispatcher.
#
# This is sourced, not executed. It provides:
#   - pretty-output helpers: step / info / ok / warn / die
#   - config vars + env overrides: COZY_REPO / COZY_REF / COZY_PREFIX / …
#   - cozy_preflight [extra-tools…]   Linux + git/cargo (+ extras) checks
#   - cozy_fetch_sources              clone/update the repo into $SRC_DIR
#   - cozy_build                      cargo build --release (+ header hints)
#   - cozy_install_binary             install the cozy binary into $BIN_DIR
#
# The clone/build/install steps are identical across integrations, so they live
# here once — a single source of truth that keeps the installers from drifting.

# --- config (env-overridable) ----------------------------------------------
COZY_REPO="${COZY_REPO:-https://github.com/lubabs770/cozy.git}"
COZY_REF="${COZY_REF:-main}"
COZY_PREFIX="${COZY_PREFIX:-$HOME/.local}"

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/cozy"
SRC_DIR="$DATA_DIR/src"
BIN_DIR="$COZY_PREFIX/bin"

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
# Usage: cozy_preflight [extra-required-tool …]
# Always requires git + cargo; pass e.g. "jq" for integrations that need more.
cozy_preflight() {
    step "Checking environment"

    [ "$(uname -s)" = "Linux" ] || die "cozy is Linux/Wayland only (this is $(uname -s))."
    if [ -z "${WAYLAND_DISPLAY:-}" ]; then
        warn "WAYLAND_DISPLAY is unset — cozy needs a Wayland session at runtime."
    fi

    local missing="" tool
    for tool in git cargo "$@"; do
        command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
    done
    if [ -n "$missing" ]; then
        info "Missing required tools:$missing"
        if   command -v pacman >/dev/null 2>&1; then info "Install:  sudo pacman -S --needed${missing/cargo/ rust} git${missing#* }"
        elif command -v apt    >/dev/null 2>&1; then info "Install:  sudo apt install${missing/cargo/ cargo}"
        elif command -v dnf    >/dev/null 2>&1; then info "Install:  sudo dnf install${missing/cargo/ cargo}"
        fi
        die "Install the tools above and re-run."
    fi
    ok "required tools present (git, cargo${*:+, }${*})"
}

# --- fetch sources ---------------------------------------------------------
# Clone or update the repo into $SRC_DIR. Skipped when COZY_SKIP_FETCH=1 (set by
# the dispatcher, which has already cloned to delegate to us).
cozy_fetch_sources() {
    if [ "${COZY_SKIP_FETCH:-0}" = "1" ] && [ -d "$SRC_DIR/.git" ]; then
        ok "Source already fetched at $SRC_DIR"
        return 0
    fi
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
}

# --- build -----------------------------------------------------------------
cozy_build() {
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
}

# --- install the binary ----------------------------------------------------
cozy_install_binary() {
    step "Installing cozy into $COZY_PREFIX"
    mkdir -p "$BIN_DIR"
    install -m 0755 "$SRC_DIR/target/release/cozy" "$BIN_DIR/cozy"
    ok "cozy -> $BIN_DIR/cozy"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) warn "$BIN_DIR is not on your PATH — add it (e.g. in ~/.profile)." ;;
    esac
}
