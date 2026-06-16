#!/usr/bin/env bash
# Capture README screenshots / demo GIFs into docs/.
# Usage:
#   docs/capture.sh shot <name>          # region screenshot -> docs/screenshots/<name>.png
#   docs/capture.sh gif  <name> [secs]   # region recording  -> docs/demos/<name>.gif
# See docs/CAPTURING.md for the list of names.
set -euo pipefail
cd "$(dirname "$0")/.."

shot() {
  local name="$1" out="docs/screenshots/$1.png"
  echo "Select a region to capture for: $name"
  if command -v spectacle >/dev/null 2>&1; then
    spectacle -r -b -n -o "$out"
  elif command -v grim >/dev/null 2>&1 && command -v slurp >/dev/null 2>&1; then
    grim -g "$(slurp)" "$out"
  else
    echo "error: install 'spectacle' (KDE) or 'grim'+'slurp' (Wayland)" >&2
    exit 1
  fi
  echo "saved $out"
}

gif() {
  local name="$1" secs="${2:-8}" out="docs/demos/$1.gif" tmp
  tmp="$(mktemp --suffix=.mp4)"
  if ! { command -v wf-recorder >/dev/null 2>&1 && command -v slurp >/dev/null 2>&1; }; then
    echo "error: install 'wf-recorder'+'slurp' (Wayland) and 'ffmpeg' (+ optional 'gifski')" >&2
    exit 1
  fi
  echo "Select a region; recording ${secs}s for: $name"
  timeout "$secs" wf-recorder -g "$(slurp)" -f "$tmp" || true
  echo "Converting to GIF…"
  if command -v gifski >/dev/null 2>&1; then
    local fdir; fdir="$(mktemp -d)"
    ffmpeg -y -i "$tmp" -vf "fps=15,scale=1000:-1:flags=lanczos" "$fdir/f%04d.png" >/dev/null 2>&1
    gifski -o "$out" "$fdir"/f*.png >/dev/null 2>&1
    rm -rf "$fdir"
  else
    ffmpeg -y -i "$tmp" -vf "fps=15,scale=1000:-1:flags=lanczos" "$out" >/dev/null 2>&1
  fi
  rm -f "$tmp"
  echo "saved $out"
}

case "${1:-}" in
  shot) [ -n "${2:-}" ] || { echo "usage: $0 shot <name>"; exit 2; }; shot "$2" ;;
  gif)  [ -n "${2:-}" ] || { echo "usage: $0 gif <name> [secs]"; exit 2; }; gif "$2" "${3:-8}" ;;
  *)    echo "usage: $0 shot <name> | gif <name> [secs]   (names: see docs/CAPTURING.md)"; exit 2 ;;
esac
