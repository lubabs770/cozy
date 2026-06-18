#!/usr/bin/env bash
#
# Boot a headless sway, build + run cozy against it, and capture frames with grim.
#
# Env knobs (all optional):
#   COZY_ARGS    extra args passed to the cozy binary
#   FRAMES       number of screenshots to take         (default 3)
#   FRAME_GAP    seconds between screenshots           (default 0.6)
#   WARMUP       seconds to wait before first capture  (default 1.2)
#   CARGO_FLAGS  extra flags for `cargo build`         (e.g. --release)
#   COZY_SWAP_TO path passed to `cozy set <path>` right after frame 0, to
#                exercise the live wallpaper swap (default: unset = no swap)
#   COZY_POST    a cozy client subcommand run right after frame 0, e.g.
#                "effect snow" or "weather --wind 0.6 --precip 1.0"
set -euo pipefail

FRAMES="${FRAMES:-3}"
FRAME_GAP="${FRAME_GAP:-0.6}"
WARMUP="${WARMUP:-1.2}"
OUT=/out

mkdir -p "$XDG_RUNTIME_DIR" "$OUT"
chmod 0700 "$XDG_RUNTIME_DIR"
rm -f "$OUT"/frame_*.png

echo "==> building cozy"
cargo build --manifest-path /work/Cargo.toml ${CARGO_FLAGS:-}
BIN=/work/target/debug/cozy
[ -n "${CARGO_FLAGS:-}" ] && [[ "$CARGO_FLAGS" == *--release* ]] && BIN=/work/target/release/cozy

cleanup() {
    [ -n "${COZY_PID:-}" ] && kill "$COZY_PID" 2>/dev/null || true
    [ -n "${SWAY_PID:-}" ] && kill "$SWAY_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> starting headless sway"
sway -c /etc/cozy/sway.conf >/tmp/sway.log 2>&1 &
SWAY_PID=$!

# Wait for the Wayland socket to appear.
for _ in $(seq 1 50); do
    [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break
    sleep 0.2
done
if [ ! -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]; then
    echo "!! sway never created $WAYLAND_DISPLAY — log:" >&2
    cat /tmp/sway.log >&2
    exit 1
fi

# Optional solid background behind cozy — useful for verifying --overlay, where
# cozy is transparent except where rain falls and the background should show
# through. e.g. SWAY_BG="#FF00FF".
if [ -n "${SWAY_BG:-}" ]; then
    echo "==> setting sway background to $SWAY_BG"
    # swaymsg finds the IPC socket via $SWAYSOCK; sway names it in XDG_RUNTIME_DIR.
    SWAYSOCK="$(ls -t "$XDG_RUNTIME_DIR"/sway-ipc.*.sock 2>/dev/null | head -1)"
    export SWAYSOCK
    swaymsg output HEADLESS-1 bg "$SWAY_BG" solid_color || echo "!! swaymsg bg failed" >&2
fi

echo "==> running cozy ${COZY_ARGS:-}"
"$BIN" ${COZY_ARGS:-} >/tmp/cozy.log 2>&1 &
COZY_PID=$!

sleep "$WARMUP"
if ! kill -0 "$COZY_PID" 2>/dev/null; then
    echo "!! cozy exited early — log:" >&2
    cat /tmp/cozy.log >&2
    exit 1
fi

echo "==> capturing $FRAMES frame(s)"
for n in $(seq 0 $((FRAMES - 1))); do
    f=$(printf "%s/frame_%03d.png" "$OUT" "$n")
    if grim "$f"; then
        echo "   wrote $f"
    else
        echo "!! grim failed (frame $n)" >&2
    fi
    # After the first frame, optionally drive the running daemon so later frames
    # prove a live change (wallpaper swap / effect switch / weather) took effect.
    if [ "$n" -eq 0 ]; then
        if [ -n "${COZY_SWAP_TO:-}" ]; then
            echo "==> cozy set $COZY_SWAP_TO"
            "$BIN" set "$COZY_SWAP_TO" || echo "!! cozy set failed" >&2
        fi
        if [ -n "${COZY_POST:-}" ]; then
            echo "==> cozy $COZY_POST"
            # shellcheck disable=SC2086
            "$BIN" $COZY_POST || echo "!! cozy $COZY_POST failed" >&2
        fi
    fi
    sleep "$FRAME_GAP"
done

echo "==> cozy log:"
cat /tmp/cozy.log
echo "==> done; frames in $OUT"
