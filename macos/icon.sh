#!/bin/sh
# Regenerates macos/AppIcon.icns from icon.swift.
set -e
dir=$(mktemp -d)
swift "$(dirname "$0")/icon.swift" "$dir/icon.png"
mkdir "$dir/AppIcon.iconset"
for s in 16 32 128 256 512; do
  sips -z $s $s "$dir/icon.png" --out "$dir/AppIcon.iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$dir/icon.png" --out "$dir/AppIcon.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$dir/AppIcon.iconset" -o "$(dirname "$0")/AppIcon.icns"
rm -rf "$dir"
