#!/usr/bin/env bash
# Tập lệnh cài đặt hoặc gỡ cài đặt addon VNType cho Fcitx5 trên Linux.
# Cách dùng:
#   ./platforms/linux-fcitx5/install.sh            # Biên dịch và cài đặt vào /usr
#   ./platforms/linux-fcitx5/install.sh --uninstall # Gỡ cài đặt
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-/usr}"

if [ "${1:-}" = "--uninstall" ]; then
    echo "==> Gỡ cài đặt VNType Fcitx5 khỏi $PREFIX"
    sudo rm -f "$PREFIX"/lib/*/fcitx5/libvntype.so "$PREFIX"/lib/fcitx5/libvntype.so
    sudo rm -f "$PREFIX/share/fcitx5/addon/vntype.conf"
    sudo rm -f "$PREFIX/share/fcitx5/inputmethod/vntype.conf"
    echo "==> Đã gỡ cài đặt hoàn tất."
    exit 0
fi

"$HERE/build.sh"
echo "==> Cài đặt VNType Fcitx5 vào $PREFIX"
sudo cmake --install "$HERE/build" --prefix "$PREFIX"
if command -v fcitx5 >/dev/null 2>&1; then
    fcitx5 -r -d >/dev/null 2>&1 || true
fi
echo "==> Đã cài đặt xong. Mở Fcitx5 Configuration để thêm bộ gõ VNType."
