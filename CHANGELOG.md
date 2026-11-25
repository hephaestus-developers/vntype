# Changelog

Tất cả thay đổi đáng chú ý của **VNType** được ghi lại tại tệp này.

Định dạng tuân theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), và phiên bản sử dụng quy tắc `{year}.{month}.{number}`.

## [Unreleased]

## [2026.11.3] - 2026-11-25

### Fixed

- **Linux (`vntype-ibus`)**:
  - Bổ sung nhận diện `IBUS_MOD4_MASK` (`Super`/Windows), `IBUS_MOD5_MASK` (`AltGr`) và `IBUS_HYPER_MASK` trong `translate_ibus_key` để không ăn nhầm tổ hợp phím hệ thống như đang gõ chữ.
  - Không dùng `ForwardKeyEvent` + `handled = true` cho tổ hợp có modifier hệ thống (`Ctrl`, `Alt`, `Super`, `Hyper`, `Meta`) khi đang có preedit; vẫn `CommitText` + `HidePreeditText` rồi trả `handled = false` để sự kiện phím thật đi thẳng qua compositor/ứng dụng.
  - Bỏ chặn `Alt+Z` trong `vntype-ibus` (tương tự `Alt+,`) để nhường phím tắt cho ứng dụng và hệ thống.

### Added

- **Tài liệu & Quy trình GitHub**:
  - Bổ sung bộ tài liệu đầy đủ trong `docs/` (`OVERVIEW.md`, `ARCHITECTURE.md`, `CORE_ENGINE.md`, `ADAPTER_AND_FFI.md`, `TESTING_AND_DEBUGGING.md`, `RELEASE.md`, `TROUBLESHOOTING.md`, `adr/0001..0004`, và `platforms/{linux,windows,macos}.md`).
  - Bổ sung mẫu Issue/PR, `dependabot.yml`, `release.yml`, `SECURITY.md`, `CONTRIBUTING.md` và `CODE_OF_CONDUCT.md` trong `.github/`.

## [2026.11.2] - 2026-11-25

### Fixed

- **Linux (`vntype-ibus`)**: Bỏ qua sự kiện phím có cờ `IBUS_IGNORED_MASK` trong `translate_ibus_key` để tránh xử lý lặp các phím đã được chuyển tiếp, khắc phục hiện tượng nuốt dấu.
- **Lõi (`vntype-engine`)**: Đặt lại `FieldKind::Text` trong `ImeSession::focus_changed` khi chuyển tiêu điểm để tránh kẹt trạng thái ô nhập đặc biệt (URL, mật khẩu) từ cửa sổ trước.

### Changed

- **Linux (`vntype-ibus`)**:
  - Cắt cửa sổ `SetSurroundingText` tối đa 128 ký tự trước con trỏ khi xử lý và ghi log.
  - Bỏ phát tín hiệu `RegisterProperties` lặp lại trong `FocusIn` (chỉ đăng ký menu khi `Enable`) để giảm lag khi chuyển focus.
  - Theo dõi `preedit_visible` để không phát `HidePreeditText` thừa khi xóa từ đã adopt bằng `Backspace` hoặc trong các đợt `Reset`.
  - Áp dụng debounce 2 giây cho `reload_config_if_needed` khi `FocusIn` và tối ưu macro `debug_log!` (không cấp phát chuỗi ở bản release, giữ sẵn file handle ở bản debug).

## [2026.11.1] - 2026-11-24

### Added

- **Linux (`vntype-linux-settings`, `vntype-ibus`)**:
  - Bổ sung tệp `.desktop` (`dev.hephaestus.vntype.settings.desktop`) và biểu tượng `vntype.svg` vào `/usr/share/applications/` và `/usr/share/icons/hicolor/scalable/apps/` (tích hợp trong `package-deb.sh` và `install.sh`), kèm `icon_name` cho cửa sổ GTK3.

### Fixed

- Sửa cảnh báo `clippy::collapsible_if` trong `build.rs` của `vntype-settings` và `vntype-hook`, cùng định dạng re-export trong `vntype-engine`.

## [2026.11.0] - 2026-11-24

### Added

- **Lõi xử lý tiếng Việt (`vntype-syllable`, `vntype-core`, `vntype-engine`, `vntype-config`, `vntype-adapter`, `vntype-ffi`)**:
  - Hỗ trợ đầy đủ các kiểu gõ `Telex`, `Simple Telex`, `VNI`, `VIQR` và 18 bảng mã tiếng Việt (`Unicode NFC/NFD`, `TCVN3`, `VNI Windows`, `VIQR`, v.v.).
  - Hỗ trợ quy tắc đặt dấu mới/cũ, kiểm tra chính tả, cho phép gõ tự do (`free_typing`), tự động khôi phục phím với từ sai (`auto_restore_keys`), sửa từ trước bằng `Backspace`, tự viết hoa đầu câu, giữ chữ hoa 2 chữ cái đầu, và bảng gõ tắt (`macros.txt`).
  - Hỗ trợ bật/tắt theo từng ứng dụng (`per_app_mode`) và nhận diện ô nhập mật khẩu/URL.
  - Công cụ dòng lệnh `vntype-cli` và tầng C ABI (`vntype-ffi`).
- **Windows (`vntype-hook`, `vntype-settings`, `vntype-tsf`, `vntype-win-ipc`)**:
  - Trình nền `WH_KEYBOARD_LL` + TSF COM DLL, biểu tượng khay hệ thống `V`/`E` với menu phân nhóm (Bảng mã, Kiểu gõ).
  - Giao diện cài đặt Win32 thuần chuẩn Windows 11 (`ComCtl32 v6`), hỗ trợ `PerMonitorV2` DPI (`WM_DPICHANGED`), logo ứng dụng (`vntype.ico`), cơ chế single-instance, và chuyển đổi giao diện song ngữ Việt / Anh.
  - Phím tắt chuyển ngôn ngữ `Ctrl + Shift` và `Alt + Z`, tùy chọn gửi Unicode qua Clipboard, phát chuông và thông báo phản hồi khi chuyển trạng thái, khởi động cùng Windows, và mở hộp thoại khi khởi động.
  - Script đóng gói bản portable `.zip` (`packaging/windows/package-zip.ps1`).
- **Linux & macOS**:
  - Daemon `linux-ibus` (D-Bus), addon `linux-fcitx5`, giao diện cài đặt GTK3 thuần trên Linux và script đóng gói `.deb` / `.rpm`.
  - Module InputMethodKit và giao diện cài đặt SwiftUI thuần trên macOS cùng script đóng gói macOS.
- **Tài liệu & Benchmark**:
  - `README.md` kèm ảnh minh họa giao diện Windows & Linux, `DEVELOPMENT.md`, giấy phép `MIT` (`LICENSE`) và bộ benchmark Criterion.

### Changed

- Tách tầng `vntype-settings-core` dùng chung cho mọi nền tảng và đưa bảng mã tĩnh ra `data/charsets` sinh mã qua `build.rs`.
- Chuyển giao diện cài đặt Windows từ `eframe` sang Win32 API thuần siêu nhẹ.
- Áp dụng quy tắc phiên bản `{year}.{month}.{number}` bắt đầu từ `2026.11.0`.

[unreleased]: https://github.com/hephaestus-developers/vntype/compare/v2026.11.3...HEAD
[2026.11.3]: https://github.com/hephaestus-developers/vntype/compare/v2026.11.2...v2026.11.3
[2026.11.2]: https://github.com/hephaestus-developers/vntype/compare/v2026.11.1...v2026.11.2
[2026.11.1]: https://github.com/hephaestus-developers/vntype/compare/v2026.11.0...v2026.11.1
[2026.11.0]: https://github.com/hephaestus-developers/vntype/releases/tag/v2026.11.0
