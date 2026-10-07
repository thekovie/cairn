#!/bin/sh
# Builds the Linux downloads from a release build of cairn (run on Linux
# by the release workflow, with appimagetool on PATH):
#   installer/linux/build-linux.sh <version> <cairn program> <out dir>
# Makes, in <out dir>:
#   cairn-<version>-linux-x86_64.AppImage   double-click to run
#   cairn-<version>-linux-x86_64.tar.gz     the program, docs, README, LICENSE
#   cairn-<version>-linux-x86_64            the program alone (for updates)
set -eu
version=$1 program=$2 out=$3
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"
base="cairn-$version-linux-x86_64"

install -m 755 "$program" "$out/$base"

mkdir "$work/$base"
install -m 755 "$program" "$work/$base/cairn"
cp "$root/README.md" "$root/LICENSE" "$root/CHANGELOG.md" "$work/$base/"
cp -R "$root/docs" "$work/$base/docs"
tar -C "$work" -czf "$out/$base.tar.gz" "$base"

app="$work/Cairn.AppDir"
mkdir -p "$app/usr/bin"
install -m 755 "$program" "$app/usr/bin/cairn"
install -m 755 "$here/AppRun" "$app/AppRun"
cp "$here/cairn.desktop" "$here/cairn.png" "$app/"
ARCH="${ARCH:-x86_64}" appimagetool --no-appstream "$app" "$out/$base.AppImage" >/dev/null
chmod 755 "$out/$base.AppImage"
ls -1 "$out"
