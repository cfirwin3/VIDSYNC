#!/usr/bin/env bash
# Build a self-contained VIDSYNC AppImage.
#
#   ./scripts/build-appimage.sh
#
# It downloads a static ffmpeg/ffprobe build and appimagetool into build/cache
# the first time, stages build/VIDSYNC.AppDir, and writes dist/*.AppImage.
# No FUSE is needed: the tool is run with APPIMAGE_EXTRACT_AND_RUN=1.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP="VIDSYNC"
ARCH="${ARCH:-x86_64}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
if [ -z "$VERSION" ]; then
    echo "Could not read the version from Cargo.toml" >&2
    exit 1
fi

case "$ARCH" in
    x86_64)
        FFMPEG_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
        APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
        ;;
    *)
        echo "Only x86_64 AppImages are supported by this script (got ARCH=$ARCH)." >&2
        echo "Set ARCH=x86_64 or adapt the download URLs." >&2
        exit 1
        ;;
esac

BUILD="$ROOT/build"
CACHE="$BUILD/cache"
APPDIR="$BUILD/$APP.AppDir"
DIST="$ROOT/dist"
BIN="$ROOT/target/release/vidsync"
FF_DIR="$CACHE/ffmpeg-static"
FF_ARCHIVE="$CACHE/ffmpeg-static.tar.xz"
APPIMAGETOOL="$CACHE/appimagetool-$ARCH.AppImage"
OUT="$DIST/$APP-$VERSION-$ARCH.AppImage"

need() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "Missing required command: $1" >&2
        exit 1
    }
}

need cargo
need curl
need tar
need xz

echo "==> Building release binary"
cargo build --release --locked

echo "==> Preparing build folders"
mkdir -p "$CACHE" "$DIST"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
         "$APPDIR/usr/share/doc/vidsync"

echo "==> Fetching static ffmpeg and ffprobe (cached)"
if [ ! -x "$FF_DIR/ffmpeg" ] || [ ! -x "$FF_DIR/ffprobe" ]; then
    if [ ! -f "$FF_ARCHIVE" ]; then
        curl -fL --retry 3 --retry-delay 2 -o "$FF_ARCHIVE" "$FFMPEG_URL"
    fi
    rm -rf "$FF_DIR"
    mkdir -p "$FF_DIR"
    tar -xJf "$FF_ARCHIVE" -C "$FF_DIR" --strip-components=1
fi

echo "==> Fetching appimagetool (cached)"
if [ ! -x "$APPIMAGETOOL" ]; then
    if [ ! -f "$APPIMAGETOOL" ]; then
        curl -fL --retry 3 --retry-delay 2 -o "$APPIMAGETOOL" "$APPIMAGETOOL_URL"
    fi
    chmod +x "$APPIMAGETOOL"
fi

echo "==> Staging AppDir"
install -Dm755 "$BIN" "$APPDIR/usr/bin/vidsync"
install -Dm755 "$FF_DIR/ffmpeg" "$APPDIR/usr/bin/ffmpeg"
install -Dm755 "$FF_DIR/ffprobe" "$APPDIR/usr/bin/ffprobe"
install -Dm644 "$ROOT/assets/vidsync.png" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps/vidsync.png"
install -Dm644 "$ROOT/scripts/vidsync.desktop" \
    "$APPDIR/usr/share/applications/vidsync.desktop"
install -Dm644 "$ROOT/README.md" "$APPDIR/usr/share/doc/vidsync/README.md"
install -Dm644 "$ROOT/MECHANICS.md" "$APPDIR/usr/share/doc/vidsync/MECHANICS.md"
install -Dm644 "$ROOT/LICENSE" "$APPDIR/usr/share/doc/vidsync/LICENSE"
install -Dm644 "$ROOT/assets/fonts/LICENSE-DejaVu.txt" \
    "$APPDIR/usr/share/doc/vidsync/LICENSE-DejaVu.txt"
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/vidsync.png" "$APPDIR/vidsync.png"
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/vidsync.png" "$APPDIR/.DirIcon"
cp "$APPDIR/usr/share/applications/vidsync.desktop" "$APPDIR/vidsync.desktop"

cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
# AppRun for VIDSYNC. Keep ffmpeg and ffprobe next to the binary.
set -eu
HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
export APPDIR="$HERE"
export PATH="$HERE/usr/bin:${PATH:-/usr/bin:/bin}"
exec "$HERE/usr/bin/vidsync" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$APPDIR/usr/share/applications/vidsync.desktop" || true
fi

echo "==> Packing AppImage"
rm -f "$OUT"
APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" \
    "$APPIMAGETOOL" --no-appstream "$APPDIR" "$OUT"
chmod +x "$OUT"

echo
echo "Built $OUT"
ls -lh "$OUT"
