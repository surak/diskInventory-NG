#!/bin/sh
# Builds "Disk Inventory NG.app" (with icon) into target/release/bundle/.
#   scripts/bundle-macos.sh            native architecture
#   scripts/bundle-macos.sh universal  arm64 + x86_64 (needs both rustup targets)
set -eu

cd "$(dirname "$0")/.."
NAME="Disk Inventory NG"
BIN=disk-inventory-ng
BUNDLE_ID=com.github.disk-inventory-ng
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

if [ "${1:-}" = universal ]; then
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    EXE=target/release/$BIN-universal
    lipo -create -output "$EXE" \
        target/aarch64-apple-darwin/release/$BIN target/x86_64-apple-darwin/release/$BIN
else
    cargo build --release
    EXE=target/release/$BIN
fi

APP="target/release/bundle/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$EXE" "$APP/Contents/MacOS/$BIN"

# Icon: the app draws it itself; iconutil packs all sizes into an .icns.
ICONSET=$(mktemp -d)/AppIcon.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    "$EXE" --write-icon "$ICONSET/icon_${s}x${s}.png" $s
    "$EXE" --write-icon "$ICONSET/icon_${s}x${s}@2x.png" $((s * 2))
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$(dirname "$ICONSET")"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$NAME</string>
    <key>CFBundleDisplayName</key><string>$NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>$BIN</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>MIT License</string>
</dict>
</plist>
PLIST

# Ad-hoc signature so Gatekeeper/TCC can identify the app on this machine.
codesign --force --deep --sign - "$APP"
echo "Built $APP"
