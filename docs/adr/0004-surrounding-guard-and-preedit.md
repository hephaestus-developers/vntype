# ADR-0004: Kiểm soát xóa lùi bằng `surrounding_guard` và `OutputMode::Preedit`

- **Trạng thái**: Đã chấp nhận (Accepted)
- **Phạm vi**: `crates/vntype-adapter/src/surrounding_guard.rs`, `crates/vntype-adapter/src/planner.rs`, `crates/vntype-adapter/src/controller.rs`

## Bối cảnh

Khi bộ gõ cần sửa một từ đã commit vào ứng dụng (như sửa dấu từ trước hoặc thay thế trong chế độ `Replace`), ứng dụng trên Linux/macOS/Windows thường gửi sự kiện `SetSurroundingText` bất đồng bộ qua IPC/D-Bus. Nếu dữ liệu `SurroundingText` bị trễ, lệch offset UTF-16/UTF-8, hoặc người dùng đang bôi đen văn bản, việc phát lệnh xóa lùi mù quáng có thể xóa nhầm chữ của người dùng.

## Quyết định

1. **Ưu tiên `OutputMode::Preedit`** ở mọi nền tảng hỗ trợ tốt preedit (IBus, Fcitx5, macOS IMK) để hạn chế tối đa thao tác xóa xung quanh khi đang gõ từ mới.
2. **Xác minh qua `surrounding_guard::check_replace`**:
   - Chặn thao tác `Replace` nếu đang có vùng chọn (`cursor_utf16 != anchor_utf16`).
   - Kiểm tra văn bản trước con trỏ thực sự kết thúc bằng `before_display` trước khi cho phép `DeleteSurroundingText` (trừ khi cờ `allow_unverified_replace` hoặc đang trong phiên `adopted` đã xác nhận).
3. **Chuẩn hóa độ dài xóa lùi đa đơn vị (`offset.rs`)**: Tính đồng thời `delete_utf16`, `delete_chars` (Unicode scalar) và `delete_bytes` (UTF-8) để mỗi nền tảng dùng đúng đơn vị mình cần (IBus dùng `chars`, macOS/Windows dùng `UTF-16`, Fcitx5/terminal dùng `UTF-8` hoặc `chars`).

## Hệ quả

- **Tích cực**:
  - Ngăn chặn triệt để lỗi xóa nhầm ký tự khi IPC trễ hoặc lệch mã hóa UTF-8/UTF-16.
  - Toàn bộ logic dễ gây lỗi nhất của IME được kiểm thử tập trung trong `vntype-adapter`.
- **Đánh đổi**:
  - Các backend nền tảng cần cung cấp `set_surrounding_utf16` chính xác và kịp thời để tính năng sửa từ cũ (`edit_previous_word`) hoạt động mượt mà.
