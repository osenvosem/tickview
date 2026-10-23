#!/usr/bin/env bash
# Build a TickView.app bundle and wrap it in a .dmg plus a plain .zip.
#
#   crates/app/pack/macos/make-dmg.sh <binary> <version> <arch> <out-dir>
#
# The version goes into CFBundleShortVersionString/CFBundleVersion, the arch
# only into the file name (the binary itself is already built for that arch).
# The bundle is signed ad-hoc: macOS on Apple Silicon refuses to run unsigned
# code, and TickView has no Developer ID certificate (see README, alpha status).
set -euo pipefail

if [ "$#" -ne 4 ]; then
        echo "usage: $0 <binary> <version> <arch> <out-dir>" >&2
        exit 2
fi

BIN="$1"
VERSION="$2"
ARCH="$3"
OUT="$4"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
PACK="$ROOT/crates/app/pack"

[ -x "$BIN" ] || { echo "no such binary: $BIN" >&2; exit 1; }
[ -f "$PACK/tickview-mark.png" ] || {
        echo "missing $PACK/tickview-mark.png (run: cargo run -p xtask -- icon --write)" >&2
        exit 1
}

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
APP="$STAGE/dmg/TickView.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/tickview"
sed -e "s/@VERSION@/$VERSION/g" "$PACK/macos/Info.plist.in" >"$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"
cp "$ROOT/LICENSE" "$ROOT/THIRD_PARTY_NOTICES.md" "$APP/Contents/Resources/"

ICONSET="$STAGE/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
        sips -z "$size" "$size" "$PACK/tickview-mark.png" \
                --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
        double=$((size * 2))
        sips -z "$double" "$double" "$PACK/tickview-mark.png" \
                --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

codesign --force -s - "$APP/Contents/MacOS/tickview"
codesign --verify --strict "$APP/Contents/MacOS/tickview"

# Drag-to-Applications layout.
ln -s /Applications "$STAGE/dmg/Applications"

mkdir -p "$OUT"
DMG="$OUT/tickview-${VERSION}-macos-${ARCH}.dmg"
rm -f "$DMG"
hdiutil create -quiet -volname "TickView ${VERSION}" -srcfolder "$STAGE/dmg" -ov -format UDZO "$DMG"
hdiutil verify -quiet "$DMG"
echo "$DMG ($(du -h "$DMG" | cut -f1))"
ZIP="$OUT/tickview-${VERSION}-macos-${ARCH}.zip"
rm -f "$ZIP"
# --keepParent so the archive opens as TickView.app, not loose Contents/.
ditto -c -k --keepParent "$STAGE/dmg/TickView.app" "$ZIP"
echo "$ZIP ($(du -h "$ZIP" | cut -f1))"
