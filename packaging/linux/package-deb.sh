#!/usr/bin/env bash
set -euo pipefail
umask 022

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/dist/linux}"
OFFLINE="${VNTYPE_OFFLINE:-0}"
PROFILE="${PROFILE:-release}"

for arg in "$@"; do
    case "$arg" in
        --debug) PROFILE="debug" ;;
        --release) PROFILE="release" ;;
        *)
            echo "Usage: $(basename "$0") [--debug|--release]" >&2
            exit 1
            ;;
    esac
done

for cmd in cargo cmake dpkg-deb; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Missing required tool: $cmd" >&2
        exit 1
    fi
done

VERSION="${VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)}"
ARCH="$(dpkg --print-architecture)"
if command -v dpkg-architecture >/dev/null 2>&1; then
    MULTIARCH="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
else
    case "$ARCH" in
        amd64) MULTIARCH="x86_64-linux-gnu" ;;
        arm64) MULTIARCH="aarch64-linux-gnu" ;;
        *)     MULTIARCH="$(uname -m)-linux-gnu" ;;
    esac
fi

CARGO_FLAGS=()
CMAKE_BUILD_TYPE="Release"
if [[ "$PROFILE" == "release" ]]; then
    CARGO_FLAGS+=(--release)
else
    CMAKE_BUILD_TYPE="Debug"
fi
[[ "$OFFLINE" == "1" ]] && CARGO_FLAGS+=(--offline)

echo "Building VNType $VERSION ($ARCH) $PROFILE binaries..."
cargo build "${CARGO_FLAGS[@]}" \
    -p vntype-cli \
    -p vntype-ibus \
    -p vntype-linux-settings \
    -p vntype-ffi \
    --manifest-path "$ROOT_DIR/Cargo.toml"

STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGE_DIR"' EXIT

FCITX_BUILD="$STAGE_DIR/fcitx5-build"
cmake -B "$FCITX_BUILD" -S "$ROOT_DIR/platforms/linux/fcitx5" \
    -DCMAKE_BUILD_TYPE="$CMAKE_BUILD_TYPE" \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DCMAKE_INSTALL_LIBDIR="lib/$MULTIARCH" \
    -DVNTYPE_STATIC_LIB="$ROOT_DIR/target/$PROFILE/libvntype_ffi.a"
cmake --build "$FCITX_BUILD" -j"$(nproc)"

mkdir -p "$OUT_DIR"

build_deb() {
    local pkg="$1"
    local deps="$2"
    local desc="$3"
    local pkg_root="$STAGE_DIR/$pkg"

    mkdir -p "$pkg_root/DEBIAN"
    chmod 0755 "$pkg_root" "$pkg_root/DEBIAN"
    {
        echo "Package: $pkg"
        echo "Version: $VERSION"
        echo "Section: utils"
        echo "Priority: optional"
        echo "Architecture: $ARCH"
        echo "Maintainer: Hephaestus Developers <haiphamngoc.tech@gmail.com>"
        if [[ -n "$deps" ]]; then
            echo "Depends: $deps"
        fi
        echo "Description: $desc"
    } > "$pkg_root/DEBIAN/control"
    chmod 0644 "$pkg_root/DEBIAN/control"

    dpkg-deb --build --root-owner-group "$pkg_root" \
        "$OUT_DIR/${pkg}_${VERSION}_${ARCH}.deb"
}

# 1. vntype-cli (optional)
CLI_ROOT="$STAGE_DIR/vntype-cli"
install -Dm755 "$ROOT_DIR/target/$PROFILE/vntype-cli" "$CLI_ROOT/usr/bin/vntype-cli"
build_deb "vntype-cli" "" "VNType Vietnamese input method CLI"

# 2. vntype-ibus
IBUS_ROOT="$STAGE_DIR/vntype-ibus"
install -Dm755 "$ROOT_DIR/target/$PROFILE/vntype-ibus" "$IBUS_ROOT/usr/libexec/vntype-ibus"
install -Dm755 "$ROOT_DIR/target/$PROFILE/vntype-linux-settings" "$IBUS_ROOT/usr/bin/vntype-linux-settings"
install -Dm644 \
    "$ROOT_DIR/platforms/linux/settings/data/dev.hephaestus.vntype.settings.desktop" \
    "$IBUS_ROOT/usr/share/applications/dev.hephaestus.vntype.settings.desktop"
install -Dm644 \
    "$ROOT_DIR/docs/assets/logo.svg" \
    "$IBUS_ROOT/usr/share/icons/hicolor/scalable/apps/vntype.svg"
mkdir -p "$IBUS_ROOT/usr/share/ibus/component"
sed 's|@LIBEXECDIR@|/usr/libexec|g' \
    "$ROOT_DIR/platforms/linux/ibus/data/vntype.xml.in" \
    > "$IBUS_ROOT/usr/share/ibus/component/vntype.xml"
build_deb "vntype-ibus" "ibus, libgtk-3-0 | libgtk-3-0t64" "VNType IBus input method engine"

# 2. vntype-fcitx5
FCITX_ROOT="$STAGE_DIR/vntype-fcitx5"
DESTDIR="$FCITX_ROOT" cmake --install "$FCITX_BUILD"
build_deb "vntype-fcitx5" "fcitx5" "VNType Fcitx5 input method addon"

echo "Created .deb packages in $OUT_DIR:"
ls -lh "$OUT_DIR"/*.deb
