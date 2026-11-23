#!/usr/bin/env bash
set -euo pipefail
umask 022

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MACOS_DIR="$ROOT_DIR/platforms/macos"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/dist/macos}"
OFFLINE="${VNTYPE_OFFLINE:-0}"
CODESIGN_IDENTITY="${CODESIGN_IDENTITY:--}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "This script must be run on macOS (Darwin)." >&2
    exit 1
fi

for cmd in cargo rustup swift lipo codesign pkgbuild hdiutil; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Missing required tool: $cmd" >&2
        exit 1
    fi
done

VERSION="${VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)}"
CARGO_FLAGS=(--release -p vntype-ffi --manifest-path "$ROOT_DIR/Cargo.toml")
[[ "$OFFLINE" == "1" ]] && CARGO_FLAGS+=(--offline)

echo "Building universal libvntype_ffi.a (arm64 + x86_64)..."
cargo build "${CARGO_FLAGS[@]}" --target aarch64-apple-darwin
cargo build "${CARGO_FLAGS[@]}" --target x86_64-apple-darwin

mkdir -p "$ROOT_DIR/target/release"
lipo -create \
    "$ROOT_DIR/target/aarch64-apple-darwin/release/libvntype_ffi.a" \
    "$ROOT_DIR/target/x86_64-apple-darwin/release/libvntype_ffi.a" \
    -output "$ROOT_DIR/target/release/libvntype_ffi.a"

echo "Building universal VNTypeIM via SwiftPM..."
pushd "$MACOS_DIR" >/dev/null
swift build -c release --arch arm64 --arch x86_64
BIN_DIR="$(swift build -c release --arch arm64 --arch x86_64 --show-bin-path)"
popd >/dev/null

STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGE_DIR"' EXIT
mkdir -p "$OUT_DIR"

APP_BUNDLE="$STAGE_DIR/VNType.app"
mkdir -p "$APP_BUNDLE/Contents/MacOS" "$APP_BUNDLE/Contents/Resources"
cp "$MACOS_DIR/Info.plist" "$APP_BUNDLE/Contents/Info.plist"
cp "$BIN_DIR/VNTypeIM" "$APP_BUNDLE/Contents/MacOS/VNTypeIM"
chmod 755 "$APP_BUNDLE/Contents/MacOS/VNTypeIM"

echo "Signing VNType.app (identity: $CODESIGN_IDENTITY)..."
codesign --force --deep --options runtime \
    --entitlements "$MACOS_DIR/Entitlements.plist" \
    --sign "$CODESIGN_IDENTITY" \
    "$APP_BUNDLE"

echo "Creating .pkg installer..."
PKG_PATH="$OUT_DIR/VNType-${VERSION}-universal.pkg"
pkgbuild \
    --component "$APP_BUNDLE" \
    --install-location "/Library/Input Methods" \
    --identifier "dev.hephaestus.inputmethod.VNType" \
    --version "$VERSION" \
    "$PKG_PATH"

echo "Creating .dmg disk image..."
DMG_STAGE="$STAGE_DIR/dmg"
mkdir -p "$DMG_STAGE"
cp -R "$APP_BUNDLE" "$DMG_STAGE/VNType.app"
ln -s "/Library/Input Methods" "$DMG_STAGE/Input Methods"

DMG_PATH="$OUT_DIR/VNType-${VERSION}-universal.dmg"
rm -f "$DMG_PATH"
hdiutil create \
    -volname "VNType $VERSION" \
    -srcfolder "$DMG_STAGE" \
    -ov \
    -format UDZO \
    "$DMG_PATH"

echo "macOS packages created in $OUT_DIR:"
ls -lh "$PKG_PATH" "$DMG_PATH"
