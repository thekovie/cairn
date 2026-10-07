#!/bin/sh
# Makes the app icons from the Cairn logo (assets/favicon.svg). Run on a Mac
# with rsvg-convert (brew install librsvg) after changing the logo, and
# commit the results; release builds use the committed files.
#   installer/icons/cairn.ico     Windows program and installer
#   installer/macos/Cairn.icns    the Mac app
#   installer/linux/cairn.png     the Linux AppImage
set -eu
cd "$(dirname "$0")/../.."
svg=assets/favicon.svg
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

png() { rsvg-convert -w "$1" -h "$1" "$svg" -o "$2"; }

for s in 16 24 32 48 64 128 256; do png "$s" "$work/$s.png"; done
python3 - "$work" installer/icons/cairn.ico <<'PY'
import struct, sys
work, out = sys.argv[1], sys.argv[2]
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [open(f"{work}/{s}.png", "rb").read() for s in sizes]
header = struct.pack("<HHH", 0, 1, len(sizes))
offset = 6 + 16 * len(sizes)
entries = b""
for s, data in zip(sizes, images):
    entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
open(out, "wb").write(header + entries + b"".join(images))
PY

set_dir="$work/Cairn.iconset"
mkdir "$set_dir"
for s in 16 32 128 256 512; do
  png "$s" "$set_dir/icon_${s}x${s}.png"
  png $((s * 2)) "$set_dir/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$set_dir" -o installer/macos/Cairn.icns

png 256 installer/linux/cairn.png
echo "Icons made."
