#!/bin/sh
# Builds Linux release packages into target/release/bundle/:
#   DiskInventoryNG-<version>-linux-<arch>.tar.gz   binary, .desktop file, icon
#   DiskInventoryNG-<version>-<arch>.AppImage       if appimagetool is on PATH
#                                                   (or APPIMAGETOOL is set)
# Set TARGET (e.g. armv7-unknown-linux-gnueabihf) to cross-compile; the
# icons are then drawn by a native build.
set -eu

cd "$(dirname "$0")/.."
BIN=disk-inventory-ng
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
OUT=target/release/bundle
cargo build --release
HOST_EXE=target/release/$BIN
if [ -n "${TARGET:-}" ]; then
    cargo build --release --target "$TARGET"
    EXE=target/$TARGET/release/$BIN
    ARCH=${TARGET%%-*}
else
    EXE=$HOST_EXE
    ARCH=$(uname -m)
fi
# Debian-style names for 32-bit ARM (Raspberry Pi OS 32-bit).
case $ARCH in armv7*) ARCH=armhf ;; esac
mkdir -p "$OUT"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Tarball laid out like a /usr/local prefix.
NAME=DiskInventoryNG-$VERSION-linux-$ARCH
ROOT=$WORK/$NAME
mkdir -p "$ROOT/bin" "$ROOT/share/applications" "$ROOT/share/icons/hicolor/256x256/apps" "$ROOT/share/icons/hicolor/512x512/apps"
cp "$EXE" "$ROOT/bin/"
cp assets/linux/$BIN.desktop "$ROOT/share/applications/"
"$HOST_EXE" --write-icon "$ROOT/share/icons/hicolor/256x256/apps/$BIN.png" 256
"$HOST_EXE" --write-icon "$ROOT/share/icons/hicolor/512x512/apps/$BIN.png" 512
cp README.md LICENSE "$ROOT/"
tar -C "$WORK" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "Built $OUT/$NAME.tar.gz"

# AppImage: one self-contained file.
TOOL=${APPIMAGETOOL:-$(command -v appimagetool || true)}
if [ -z "$TOOL" ]; then
    echo "appimagetool not found; skipping AppImage" >&2
    exit 0
fi
APPDIR=$WORK/AppDir
mkdir -p "$APPDIR/usr"
cp -R "$ROOT/bin" "$ROOT/share" "$APPDIR/usr/"
cp assets/linux/$BIN.desktop "$APPDIR/"
cp "$ROOT/share/icons/hicolor/256x256/apps/$BIN.png" "$APPDIR/$BIN.png"
ln -s usr/bin/$BIN "$APPDIR/AppRun"
APPIMAGE=$OUT/DiskInventoryNG-$VERSION-$ARCH.AppImage
ARCH=$ARCH "$TOOL" --no-appstream "$APPDIR" "$APPIMAGE"
echo "Built $APPIMAGE"
