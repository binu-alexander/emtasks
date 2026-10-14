#!/bin/bash
# Regenerate the README screenshots in docs/screenshots/ from made-up sample tasks.
# Each shot runs emTasks with HOME pointed at a throwaway folder, so your real tasks are never shown or touched.
# Needs: a release build (cargo build --release), and Screen Recording permission for your terminal/editor
# (System Settings → Privacy & Security → Screen & System Audio Recording).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CRATE="$(dirname "$(dirname "$HERE")")"
ROOT="$(dirname "$CRATE")"
BIN="$CRATE/target/release/emtasks"
OUT="$ROOT/docs/screenshots"
WORK="$(mktemp -d)"
PID=""
trap '[ -n "$PID" ] && kill "$PID" 2>/dev/null; rm -rf "$WORK"' EXIT
[ -x "$BIN" ] || { echo "Build first: (cd emtasks && cargo build --release)" >&2; exit 1; }
swiftc -O "$HERE/winid.swift" -o "$WORK/winid"
# Park the pointer at the left edge (not a corner, so Hot Corners don't fire) so no hover/tooltip shows up.
printf 'import CoreGraphics\nCGWarpMouseCursorPosition(CGPoint(x: 2, y: 400))\n' > "$WORK/park.swift"
swiftc -O "$WORK/park.swift" -o "$WORK/park"
mkdir -p "$OUT"

shoot() { # theme name view
  local home="$WORK/home-$2"
  mkdir -p "$home/Library/Application Support/emTasks"
  python3 -I "$HERE/make_sample.py" "$1" "$home/Library/Application Support/emTasks/emtasks-data.ron" "$3"
  HOME="$home" "$BIN" >/dev/null 2>&1 &
  PID=$!
  "$WORK/park"
  sleep 3
  # The window can take a moment to become capturable; retry for up to ~10 s.
  local ok=""
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    local win; win="$("$WORK/winid" "$PID" || true)"
    if [ -n "$win" ] && screencapture -x -l "$win" "$WORK/$2.png" 2>/dev/null && [ -s "$WORK/$2.png" ]; then ok=1; break; fi
    sleep 1
  done
  kill "$PID"; wait "$PID" 2>/dev/null || true; PID=""
  [ -n "$ok" ] || { echo "Couldn't capture $2 (is Screen Recording allowed for this app?)" >&2; exit 1; }
  sips -Z 1600 "$WORK/$2.png" --out "$OUT/$2.png" >/dev/null
  echo "  $OUT/$2.png"
}
shoot Daylight matrix-light Matrix
shoot Midnight matrix-dark Matrix
shoot Paper list-paper List
