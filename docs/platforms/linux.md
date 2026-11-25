# Kiến trúc nền tảng Linux (IBus, Fcitx5, GTK3 Settings)

VNType trên Linux hỗ trợ cả hai framework bộ gõ phổ biến nhất là **IBus** (thuần Rust qua D-Bus) và **Fcitx5** (C++ addon liên kết `libvntype_ffi`), dùng chung ứng dụng cấu hình GTK3.

---

## 1. Engine IBus (`platforms/linux/ibus/`)

- **Giao tiếp D-Bus (`src/ibus_dbus.rs`)**:
  - Sử dụng `zbus` đăng ký service `org.freedesktop.IBus.Factory` và đối tượng `org.freedesktop.IBus.Engine` (`/org/freedesktop/IBus/Engine/vntype/<id>`).
  - Mô hình **Engine Actor** (`spawn_engine_actor`): toàn bộ lệnh D-Bus (`ProcessKey`, `FocusIn`, `FocusOut`, `Reset`, `Enable`, `PropertyActivate`, `SetSurrounding`, `SetContentType`) đi tuần tự qua một `mpsc` channel tới thread sở hữu `IbusEngineService`, đảm bảo không race condition và giữ đúng thứ tự FIFO các signal phát ra.
- **Ánh xạ phím (`src/keymap.rs`)**:
  - Bỏ qua các phím nhả (`IBUS_RELEASE_MASK`) và phím đã được chính engine chuyển tiếp (`IBUS_IGNORED_MASK = 1 << 25`) để tránh xử lý lặp gây nuốt dấu.
- **Tối ưu hiệu năng & Giao thức (`src/dbus_service.rs`)**:
  - **Cắt cửa sổ `SetSurroundingText`**: `window_and_strip_surrounding` chỉ giữ tối đa 128 ký tự trước con trỏ (và loại bỏ ký tự rác `'\u{200b}'`), đồng thời bỏ qua tín hiệu trễ nếu `display_text()` đang có preedit.
  - **Tránh `HidePreeditText` thừa**: Theo dõi cờ `preedit_visible`; chỉ phát `HidePreeditText` khi trước đó đã có `UpdatePreeditText(visible = true)` (cắt giảm một nửa signal D-Bus khi giữ `Backspace` xóa từ đã adopt).
  - **Tránh kẹt auto-repeat khi giữ `Backspace`**: Cờ `holding_eaten_backspace` giúp chuyển tiếp `ForwardKeyEvent` press + release khi từ vừa xóa sạch mà người dùng vẫn đang giữ phím.
  - **Menu Top Bar**: Chỉ phát `RegisterProperties` khi `Enable`, không dựng lại menu mỗi lần `FocusIn`; `reload_config_if_needed(false)` khi `FocusIn` dùng debounce 2 giây để tránh I/O thừa trong các đợt focus bounce của GNOME/GTK.
  - **Thứ tự FIFO khi nhấn `Enter` / phím chuyển tiếp**: Luôn phát `CommitText` → `HidePreeditText` → `ForwardKeyEvent(press & release)` với `handled = true` để phím `Enter` không chạy trước chữ vừa commit.

---

## 2. Addon Fcitx5 (`platforms/linux/fcitx5/`)

- Gồm `src/engine.{h,cpp}` và `src/ic_state.{h,cpp}` xây dựng bằng CMake (`CMakeLists.txt`).
- Mỗi `InputContext` của Fcitx5 giữ một con trỏ `VntController *` từ `vntype-ffi` (`OutputMode::Preedit`), nhận `VntEffectList` và dịch tuần tự sang API của Fcitx5 (`inputPanel().setClientPreedit`, `commitString`, `deleteSurroundingText`, `forwardKey`).
- Có bộ kiểm thử CTest riêng trong `platforms/linux/fcitx5/tests/`.

---

## 3. Giao diện cài đặt GTK3 (`platforms/linux/settings/`)

- Binary `vntype-linux-settings` viết bằng Rust + `gtk` crate (GTK3 thuần), đọc/ghi cấu hình thông qua `vntype-settings-core` và `vntype-config` (`config.json`, `macros.txt`).
- Được tích hợp vào hệ thống desktop qua tệp `.desktop` (`dev.hephaestus.vntype.settings.desktop`) và biểu tượng `vntype.svg`.
