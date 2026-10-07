#!/bin/sh
# Builds Cairn.app and the release zip from an Apple silicon and an Intel
# build of cairn (run on a Mac by the release workflow):
#   installer/macos/build-app.sh <version> <arm64 cairn> <x86_64 cairn> <out dir>
# Makes <out dir>/cairn-<version>-macos-universal.zip holding Cairn.app.
set -eu
version=$1 arm=$2 intel=$3 out=$4
here=$(cd "$(dirname "$0")" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

app="$stage/Cairn.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"
cp "$here/Cairn.icns" "$app/Contents/Resources/Cairn.icns"
cp "$here/start-cairn" "$app/Contents/MacOS/start-cairn"
lipo -create "$arm" "$intel" -output "$app/Contents/MacOS/cairn"
chmod 755 "$app/Contents/MacOS/cairn" "$app/Contents/MacOS/start-cairn"
# Apple silicon only runs programs with at least this (ad-hoc) signature.
# The app itself isn't sealed: Cairn replaces its program when it updates,
# which would break a seal. (No developer signature either; see
# docs/getting-started.md for the first start.)
codesign --force --sign - "$app/Contents/MacOS/cairn"

mkdir -p "$out"
zip="$out/cairn-$version-macos-universal.zip"
# ditto keeps the app's permissions and structure, as Finder would.
ditto -c -k --keepParent "$app" "$zip"
echo "$zip"
