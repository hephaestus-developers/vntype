# VNType - Hệ thống bộ gõ tiếng Việt đa nền tảng

VNType là bộ gõ tiếng Việt viết bằng Rust, hỗ trợ đa nền tảng:

- **Linux**: Fcitx5 (`vntype-fcitx5`), IBus (`vntype-ibus`), Wayland thuần (`vntype-wayland`), CLI (`vntype`) và GUI cấu hình (`vntype-settings`).
- **Windows**: Chế độ Portable (`VNType.exe` với `WH_KEYBOARD_LL` + System Tray + GUI) và TSF COM DLL (`vntype_tsf.dll`).
- **macOS**: InputMethodKit bundle (`VNType.app` Universal `arm64` + `x86_64`).

## Tài liệu hướng dẫn cài đặt

Xem chi tiết cách cài đặt và kích hoạt cho từng hệ điều hành và từng Desktop Environment của Linux (GNOME, KDE Plasma, XFCE, Hyprland, Sway...) tại:

- [Hướng dẫn cài đặt đầy đủ (docs/INSTALL.md)](docs/INSTALL.md)

## Đóng gói nhanh

- **Linux (`.deb`)**: `./packaging/linux/package-deb.sh`
- **Linux (`.rpm`)**: `./packaging/linux/package-rpm.sh`
- **Linux (từ mã nguồn)**: `./packaging/linux/install-source.sh`
- **Windows (Portable `.zip`)**: `.\packaging\windows\package-zip.ps1`
- **Windows (TSF `.zip`)**: `.\packaging\windows\package-tsf.ps1`
- **macOS (Universal `.pkg` & `.dmg`)**: `./packaging/macos/package-macos.sh`
