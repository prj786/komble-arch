#!/usr/bin/env bash
# Rasterise the app's one SVG source into the PNGs Tauri and the package need.
#   packaging/komble.svg → src-tauri/icons/{32,64,128,256}x*.png,
#                          128x128@2x.png (Tauri's name for 256), icon.png (512)
# Needs rsvg-convert (librsvg). Run after editing the SVG; the PNGs are
# committed so a checkout builds without librsvg.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v rsvg-convert >/dev/null || { echo "rsvg-convert (librsvg) is required" >&2; exit 1; }
src=packaging/komble.svg
out=src-tauri/icons
mkdir -p "$out"
for s in 32 64 128 256; do
    rsvg-convert -w "$s" -h "$s" "$src" -o "$out/${s}x${s}.png"
done
cp "$out/256x256.png" "$out/128x128@2x.png"
rsvg-convert -w 512 -h 512 "$src" -o "$out/icon.png"
ls -1 "$out"
