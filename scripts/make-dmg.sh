#!/bin/sh
# Builds the .app and packs it into a styled, compressed DMG:
#   target/release/bundle/DiskInventoryNG-<version>.dmg
# Pass "universal" to build for arm64 + x86_64.
set -eu

cd "$(dirname "$0")/.."
scripts/bundle-macos.sh "${1:-}"

NAME="Disk Inventory NG"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
OUT="target/release/bundle/DiskInventoryNG-$VERSION.dmg"
APP="target/release/bundle/$NAME.app"
WORK=$(mktemp -d)
MNT=
trap '[ -n "$MNT" ] && hdiutil detach "$MNT" -quiet 2>/dev/null; rm -rf "$WORK"' EXIT

# Staging folder: app, Applications link, hidden background + volume icon.
STAGE="$WORK/stage"
mkdir -p "$STAGE/.background"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
swift scripts/dmg-background.swift "$WORK/bg.png" 1
swift scripts/dmg-background.swift "$WORK/bg@2x.png" 2
tiffutil -cathidpicheck "$WORK/bg.png" "$WORK/bg@2x.png" -out "$STAGE/.background/background.tiff" 2>/dev/null

# Writable image to lay out the Finder window, then compress.
hdiutil create -quiet -srcfolder "$STAGE" -volname "$NAME" -fs HFS+ -format UDRW -ov "$WORK/rw.dmg"
# Use the real mount point: it gets a suffix if the name is already taken.
MNT=$(hdiutil attach -readwrite -noverify -noautoopen "$WORK/rw.dmg" | awk -F'\t' '/Apple_HFS/ {print $NF}')
DISK=$(basename "$MNT")

layout() {
osascript <<APPLESCRIPT
tell application "Finder"
    tell disk "$DISK"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set opts to the icon view options of container window
        set arrangement of opts to not arranged
        set icon size of opts to 128
        set text size of opts to 13
        set background picture of opts to file ".background:background.tiff"
        set position of item "$NAME.app" of container window to {170, 200}
        set position of item "Applications" of container window to {470, 200}
        update without registering applications
        -- Set the size last: Finder may resize the window while it lays out.
        set the bounds of container window to {200, 120, 840, 548}
        delay 2
        set the bounds of container window to {200, 120, 840, 548}
        delay 1
        close
    end tell
end tell
APPLESCRIPT
}

# Finder sometimes needs a moment to notice the new volume.
sleep 2
if ! (layout || { sleep 3; layout; } || { sleep 5; layout; }); then
    echo "warning: Finder layout failed (allow Automation for your terminal); DMG will be unstyled" >&2
fi

# Volume icon goes in after the Finder step, which would otherwise drop it.
cp "$APP/Contents/Resources/AppIcon.icns" "$MNT/.VolumeIcon.icns"
SetFile -a C "$MNT" 2>/dev/null || true   # use the custom volume icon
rm -rf "$MNT/.fseventsd"
[ -f "$MNT/.VolumeIcon.icns" ] || { echo "error: volume icon missing" >&2; exit 1; }
sync
hdiutil detach "$MNT" -quiet
MNT=
rm -f "$OUT"
hdiutil convert -quiet "$WORK/rw.dmg" -format UDZO -imagekey zlib-level=9 -o "$OUT"
echo "Built $OUT ($(du -h "$OUT" | cut -f1))"
