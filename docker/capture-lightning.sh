#!/usr/bin/env bash
# Dense lightning-only capture: grab frames as fast as grim allows so the brief
# flashes and bolts actually get sampled. Frames -> /out/l_####.png.
set -euo pipefail
OUT=/out
SHOTS="${SHOTS:-80}"
mkdir -p "$XDG_RUNTIME_DIR" "$OUT"; chmod 0700 "$XDG_RUNTIME_DIR"
rm -f "$OUT"/l_*.png
BIN=/work/target/debug/cozy
[ -x "$BIN" ] || cargo build --manifest-path /work/Cargo.toml
trap 'kill ${COZY_PID:-0} ${SWAY_PID:-0} 2>/dev/null || true' EXIT
sway -c /etc/cozy/sway.conf >/tmp/sway.log 2>&1 & SWAY_PID=$!
for _ in $(seq 1 50); do [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break; sleep 0.2; done
"$BIN" >/tmp/cozy.log 2>&1 & COZY_PID=$!
sleep 1.5
"$BIN" effect lightning || true
"$BIN" weather --wind 0.3 --precip 1.0 || true
sleep 0.3
n=0
for _ in $(seq 1 "$SHOTS"); do
    f=$(printf "%s/l_%04d.png" "$OUT" "$n"); grim "$f" && n=$((n + 1))
done
echo "==> captured $n lightning frames"
