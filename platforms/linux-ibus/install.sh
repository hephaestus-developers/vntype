#!/usr/bin/env bash
# Tập lệnh biên dịch và cài đặt trình nền IBus (vntype-ibus) trên Linux.
# Cách dùng:
#   ./platforms/linux-ibus/install.sh             # Cài đặt vào /usr (cần sudo)
#   ./platforms/linux-ibus/install.sh --uninstall # Gỡ cài đặt
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PREFIX="${PREFIX:-/usr}"
LIBEXECDIR="${LIBEXECDIR:-$PREFIX/libexec}"
COMPONENT_DIR="$PREFIX/share/ibus/component"

if [ "${1:-}" = "--uninstall" ]; then
    echo "==> Gỡ cài đặt vntype-ibus khỏi $PREFIX"
    sudo rm -f "$LIBEXECDIR/vntype-ibus" "$COMPONENT_DIR/vntype.xml"
    ibus write-cache >/dev/null 2>&1 || true
    echo "==> Đã gỡ cài đặt vntype-ibus."
    exit 0
fi

CARGO_FLAGS=()
if [ "${VNTYPE_OFFLINE:-0}" = "1" ] || ! timeout 1 bash -c "</dev/tcp/index.crates.io/443" 2>/dev/null; then
    CARGO_FLAGS+=("--offline")
fi

echo "==> Biên dịch vntype-ibus"
(cd "$ROOT" && cargo build --release -p vntype-ibus "${CARGO_FLAGS[@]}")

echo "==> Cài đặt tệp thực thi và mô tả thành phần IBus vào $PREFIX"
sudo install -Dm755 "$ROOT/target/release/vntype-ibus" "$LIBEXECDIR/vntype-ibus"
TMP_XML="$(mktemp)"
sed "s|@LIBEXECDIR@|$LIBEXECDIR|g" "$HERE/data/vntype.xml.in" > "$TMP_XML"
sudo install -Dm644 "$TMP_XML" "$COMPONENT_DIR/vntype.xml"
rm -f "$TMP_XML"

ibus write-cache >/dev/null 2>&1 || true
ibus restart >/dev/null 2>&1 || true
echo "==> Đã cài đặt xong vntype-ibus."
