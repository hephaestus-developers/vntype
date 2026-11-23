#!/usr/bin/env bash
set -euo pipefail
umask 022

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PREFIX="${PREFIX:-/usr/local}"
UNINSTALL=0
WITH_IBUS=1
WITH_FCITX5=1
OFFLINE="${VNTYPE_OFFLINE:-0}"

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Build and install VNType from source on Linux.

Options:
  --prefix <dir>       Install prefix (default: /usr/local)
  --uninstall          Uninstall VNType files from <prefix>
  --without-ibus       Skip building/installing vntype-ibus
  --without-fcitx5     Skip building/installing vntype-fcitx5
  --offline            Pass --offline to cargo
  -h, --help           Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --uninstall)
            UNINSTALL=1
            shift
            ;;
        --without-ibus)
            WITH_IBUS=0
            shift
            ;;
        --without-fcitx5)
            WITH_FCITX5=0
            shift
            ;;
        --offline)
            OFFLINE=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ "$UNINSTALL" -eq 1 ]]; then
    echo "Uninstalling VNType from $PREFIX..."
    rm -f "$PREFIX/bin/vntype-cli" \
          "$PREFIX/libexec/vntype-ibus" \
          "$PREFIX/share/ibus/component/vntype.xml" \
          "$PREFIX/share/fcitx5/addon/vntype.conf" \
          "$PREFIX/share/fcitx5/inputmethod/vntype.conf"
    find "$PREFIX" -name 'libvntype.so' -path '*/fcitx5/*' -delete 2>/dev/null || true
    echo "Uninstall complete."
    exit 0
fi

CARGO_FLAGS=(--release)
if [[ "$OFFLINE" == "1" ]]; then
    CARGO_FLAGS+=(--offline)
fi

PACKAGES=(-p vntype-cli)
[[ "$WITH_IBUS" -eq 1 ]] && PACKAGES+=(-p vntype-ibus)

echo "Building Rust workspace packages: ${PACKAGES[*]}"
cargo build "${CARGO_FLAGS[@]}" "${PACKAGES[@]}" --manifest-path "$ROOT_DIR/Cargo.toml"

echo "Installing common binaries to $PREFIX/bin..."
install -Dm755 "$ROOT_DIR/target/release/vntype-cli" "$PREFIX/bin/vntype-cli"

if [[ "$WITH_IBUS" -eq 1 ]]; then
    echo "Installing vntype-ibus..."
    install -Dm755 "$ROOT_DIR/target/release/vntype-ibus" "$PREFIX/libexec/vntype-ibus"
    mkdir -p "$PREFIX/share/ibus/component"
    sed "s|@LIBEXECDIR@|$PREFIX/libexec|g" \
        "$ROOT_DIR/platforms/linux/ibus/data/vntype.xml.in" \
        > "$PREFIX/share/ibus/component/vntype.xml"
fi

if [[ "$WITH_FCITX5" -eq 1 ]]; then
    echo "Building and installing vntype-fcitx5..."
    cargo build "${CARGO_FLAGS[@]}" -p vntype-ffi --manifest-path "$ROOT_DIR/Cargo.toml"
    FCITX_BUILD="$(mktemp -d)"
    trap 'rm -rf "$FCITX_BUILD"' EXIT
    cmake -B "$FCITX_BUILD" -S "$ROOT_DIR/platforms/linux/fcitx5" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$PREFIX"
    cmake --build "$FCITX_BUILD" -j"$(nproc)"
    cmake --install "$FCITX_BUILD"
fi

echo "VNType installed to $PREFIX."
