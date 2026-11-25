# Khắc phục sự cố thường gặp (Troubleshooting)

Tài liệu này tổng hợp các vấn đề hay gặp khi cài đặt, sử dụng hoặc phát triển **VNType** trên từng hệ điều hành.

---

## 1. Linux (IBus / Fcitx5)

### Không gõ được tiếng Việt trên một số app (Chrome, VS Code, Electron, Wayland)

- **Nguyên nhân**: App đang chạy ở chế độ không nhận Input Method mặc định hoặc xung đột giữa Wayland/XWayland.
- **Cách kiểm tra**:
  - Đảm bảo biến môi trường IME được cấu hình đúng (`GTK_IM_MODULE=ibus`, `QT_IM_MODULE=ibus`, `XMODIFIERS=@im=ibus` nếu dùng IBus trên X11/XWayland; hoặc `fcitx` nếu dùng Fcitx5).
  - Trên GNOME Wayland, kiểm tra lại Input Sources trong Settings đã chọn `Vietnamese (VNType)` chưa và chạy `ibus restart` sau khi nâng cấp gói `.deb`/`.rpm`.

### Gõ bị nuốt dấu hoặc lặp ký tự

- **Cách kiểm tra**: Cài bản debug của `vntype-ibus` (xem [TESTING_AND_DEBUGGING.md](TESTING_AND_DEBUGGING.md)) và xem `/tmp/vntype-ibus-debug.log` để kiểm tra xem desktop/app có gửi sự kiện phím lặp hoặc `Reset` bất thường hay không.

### Đổi `macros.txt` hoặc `config.json` nhưng chưa thấy áp dụng ngay

- `vntype-ibus` kiểm tra thay đổi file cấu hình với chu kỳ debounce **2 giây** khi gõ hoặc `FocusIn`. Hãy chờ ~2 giây hoặc chuyển focus sang cửa sổ khác rồi quay lại.

---

## 2. Windows (`vntype-hook` / `vntype-tsf`)

### Không gõ được tiếng Việt trong ứng dụng chạy quyền Administrator

- **Nguyên nhân**: Cơ chế bảo mật UIPI của Windows chặn `WH_KEYBOARD_LL` từ tiến trình quyền thường gửi phím vào tiến trình chạy với quyền Admin.
- **Cách xử lý**: Chạy `vntype-hook` với quyền Administrator nếu cần gõ trong các cửa sổ Admin, hoặc sử dụng `vntype_tsf.dll` (TSF COM In-Process).

### Lỗi hiển thị ký tự lạ ở một số phần mềm cũ / Game

- Thử bật tùy chọn gửi Unicode qua **Clipboard** trong `vntype-settings`, hoặc kiểm tra bảng mã đang chọn (ví dụ một số phần mềm cũ dùng `TCVN3` hoặc `VNI Windows` thay vì `Unicode`).

---

## 3. macOS (`VNTypeIM`)

### Không chọn được VNType trong Input Sources hoặc không nhận phím

- Đảm bảo bundle `VNTypeIM.app` đã nằm đúng trong `/Library/Input Methods/` hoặc `~/Library/Input Methods/`.
- Đăng xuất và đăng nhập lại (hoặc chạy `killall VNTypeIM`) sau khi cập nhật bản build mới để `IMKServer` nạp lại binary mới nhất.
- Kiểm tra quyền trong **System Settings → Privacy & Security** (Input Monitoring / Accessibility nếu được yêu cầu).
