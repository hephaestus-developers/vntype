#!/usr/bin/env bash
# Tập lệnh biên dịch và cài đặt trình nền Wayland (vntype-wayland) cùng dịch vụ systemd user.
# Cách dùng:
#   ./platforms/linux-wayland/install.sh             # Cài đặt vào ~/.local/bin
#   ./platforms/linux-wayland/install.sh --uninstall # Gỡ cài đặt
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BINDIR="${BINDIR:-$HOME/.local/bin}"
SYSTEMD_USER_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

if [ "${1:-}" = "--uninstall" ]; then
    echo "==> Dừng và gỡ cài đặt dịch vụ vntype-wayland"
    systemctl --user disable --now vntype-wayland.service >/dev/null 2>&1 || true
    rm -f "$BINDIR/vntype-wayland" "$SYSTEMD_USER_DIR/vntype-wayland.service"
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    echo "==> Đã gỡ cài đặt vntype-wayland."
    exit 0
fi

CARGO_FLAGS=()
if [ "${VNTYPE_OFFLINE:-0}" = "1" ] || ! timeout 1 bash -c "</dev/tcp/index.crates.io/443" 2>/dev/null; then
    CARGO_FLAGS+=("--offline")
fi

echo "==> Biên dịch vntype-wayland"
(cd "$ROOT" && cargo build --release -p vntype-wayland "${CARGO_FLAGS[@]}")

install -Dm755 "$ROOT/target/release/vntype-wayland" "$BINDIR/vntype-wayland"
install -Dm644 "$HERE/data/vntype-wayland.service" "$SYSTEMD_USER_DIR/vntype-wayland.service"
systemctl --user daemon-reload >/dev/null 2>&1 || true
echo "==> Đã cài đặt vntype-wayland vào $BINDIR/vntype-wayland."
echo "    Kích hoạt bằng lệnh: systemctl --user enable --now vntype-wayland.service"
