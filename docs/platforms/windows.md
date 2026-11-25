# Kiến trúc nền tảng Windows (`hook`, `tsf`, `settings`, `ipc`)

VNType trên Windows được chia thành 4 crate chuyên biệt trong `platforms/windows/` nhằm hỗ trợ đồng thời cơ chế Hook truyền thống (chạy mọi ứng dụng Win32/Electron/Game) và TSF (Text Services Framework).

---

## 1. `vntype-hook` (`platforms/windows/hook/`)

- **`keyboard_hook.rs`**: Cài đặt `WH_KEYBOARD_LL` ở mức hệ thống, chặn sự kiện phím thô và đưa qua `AdapterController` (`OutputMode::Replace`).
  - Lọc bỏ sự kiện do chính bộ gõ tự inject (`LLKHF_INJECTED` / tag riêng) để tránh vòng lặp vô tận.
  - Hỗ trợ phím tắt chuyển ngôn ngữ (`Ctrl + Shift`, `Alt + Z`) và gửi ký tự Unicode qua `SendInput` hoặc Clipboard fallback (`send_via_clipboard`) cho các ứng dụng kén Unicode.
- **`tray.rs`**: Tạo biểu tượng khay hệ thống (`V` / `E`) kèm menu chuột phải (chọn Kiểu gõ, Bảng mã, mở Cài đặt, thoát).
- **`main.rs`**: Cơ chế single-instance (Named Mutex), phát âm thanh/thông báo phản hồi khi chuyển chế độ, tùy chọn khởi động cùng Windows và đồng bộ cấu hình qua `vntype-win-ipc`.

---

## 2. `vntype-tsf` (`platforms/windows/tsf/`)

- Biên dịch thành COM In-Process DLL (`vntype_tsf.dll`, xuất symbol qua `vntype_tsf.def`).
- **`text_service.rs`**: Cài đặt các interface COM của Windows TSF (`ITfTextInputProcessor`, `ITfThreadMgrEventSink`, `ITfKeyEventSink`).
- **`composition.rs` & `edit_session.rs`**: Quản lý `ITfComposition` và `ITfEditSession` để chèn/cập nhật văn bản trực tiếp trong mô hình TSF của Windows 10/11.
- **`input_scope.rs`**: Đọc `ITfContext` / `InputScope` để ánh xạ sang `FieldKind` (nhận diện ô mật khẩu, URL, số, email).
- **`registration.rs`**: Đăng ký / hủy đăng ký CLSID và Language Profile (`DllRegisterServer` / `DllUnregisterServer`).

---

## 3. `vntype-win-ipc` (`platforms/windows/ipc/`)

- Cung cấp cơ chế giao tiếp liên tiến trình / bộ nhớ chia sẻ nguyên tử (atomic shared memory) để `vntype-hook`, `vntype_tsf.dll` và `vntype-settings` đồng bộ trạng thái bật/tắt tiếng Việt và cấu hình tức thì.

---

## 4. `vntype-settings` (`platforms/windows/settings/`)

- Giao diện Win32 API thuần (`ComCtl32 v6`) hỗ trợ `PerMonitorV2` DPI (`WM_DPICHANGED`), nhúng icon/manifest qua `build.rs` + `vntype.rc` + `vntype.ico`.
- Chia sẻ toàn bộ logic kiểm tra hợp lệ và trạng thái cấu hình từ `vntype-settings-core`.
