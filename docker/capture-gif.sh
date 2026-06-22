#!/usr/bin/env bash
#
# Capture an animated showcase of every cozy effect for docs/effects.gif.
#
# Boots a headless sway + one long-lived cozy daemon, then cycles through the
# effects over the control socket, grabbing a burst of `grim` frames of each so
# the motion is visible. Frames land in /out as f_0000.png, f_0001.png, … in
# playback order; the host stitches them into the GIF with ffmpeg.
#
# Env knobs:
#   PER     frames captured per normal effect      (default 10)
#   GAP     seconds between frames                  (default 0.18)
#   BOLTS   frames captured for the lightning pass  (default 26)
set -euo pipefail

OUT=/out
PER="${PER:-10}"
GAP="${GAP:-0.18}"
BOLTS="${BOLTS:-26}"

mkdir -p "$XDG_RUNTIME_DIR" "$OUT"
chmod 0700 "$XDG_RUNTIME_DIR"
rm -f "$OUT"/f_*.png

BIN=/work/target/debug/cozy
if [ ! -x "$BIN" ]; then
    echo "==> building cozy"
    cargo build --manifest-path /work/Cargo.toml
fi

cleanup() {
    [ -n "${COZY_PID:-}" ] && kill "$COZY_PID" 2>/dev/null || true
    [ -n "${SWAY_PID:-}" ] && kill "$SWAY_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> starting headless sway"
sway -c /etc/cozy/sway.conf >/tmp/sway.log 2>&1 &
SWAY_PID=$!
for _ in $(seq 1 50); do
    [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break
    sleep 0.2
done
[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] || { echo "!! no wayland socket"; cat /tmp/sway.log; exit 1; }

echo "==> starting cozy"
"$BIN" >/tmp/cozy.log 2>&1 &
COZY_PID=$!
sleep 1.5
kill -0 "$COZY_PID" 2>/dev/null || { echo "!! cozy died"; cat /tmp/cozy.log; exit 1; }

# The effect whose name is burned into the frame label; set before each burst.
LABEL=""
FONT=/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf

n=0
# Capture one frame and burn the active effect's name into its bottom-left
# corner (white on a translucent slab) so the GIF says which shader is running.
cap() {
    local f
    f=$(printf "%s/f_%04d.png" "$OUT" "$n")
    grim "$f" || return 0
    if [ -n "$LABEL" ]; then
        convert "$f" -font "$FONT" -pointsize 30 -gravity SouthWest \
            -fill white -undercolor '#000000A0' -annotate +22+22 "  $LABEL  " "$f" \
            || echo "!! label failed for $f" >&2
    fi
    n=$((n + 1))
}

# A little wind so the clouds visibly drift across the frame; healthy cover.
"$BIN" weather --wind 0.5 --precip 0.7 || true

for e in droplet ripple snow clouds cirrus cumulus cumulonimbus stratus sunrays; do
    echo "==> $e"
    "$BIN" effect "$e" || true
    LABEL="$e"
    sleep 0.5
    for _ in $(seq 1 "$PER"); do cap; sleep "$GAP"; done
done

# Lightning last: crank intensity so strikes are frequent and bright, and grab a
# longer burst so frames actually land during the flashes.
echo "==> lightning"
"$BIN" effect lightning || true
"$BIN" weather --wind 0.3 --precip 1.0 || true
LABEL="lightning"
sleep 0.3
for _ in $(seq 1 "$BOLTS"); do cap; sleep 0.14; done

echo "==> captured $n frames"
grep -i "draw error" /tmp/cozy.log && echo "!! shader errors above" || echo "==> no shader errors"
