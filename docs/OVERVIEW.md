# Tổng quan sản phẩm & Triết lý thiết kế (Overview)

**VNType** là bộ gõ tiếng Việt đa nền tảng viết bằng Rust (kết hợp lớp tích hợp hệ điều hành thuần cho Linux, Windows và macOS), hướng tới **độ trễ cực thấp, độ tin cậy cao trên mọi ứng dụng và tính bảo mật khi nhập liệu nhạy cảm**.

---

## 1. Mục tiêu cốt lõi

1. **Lõi thuần khiết, không phụ thuộc hệ điều hành (`no_std`)**:
   - Toàn bộ ngữ âm học (`vntype-syllable`) và máy trạng thái biến đổi tiếng Việt (`vntype-core`) là mã Rust thuần, đồng bộ, không gọi I/O hay cấp phát phụ thuộc OS.
   - Nhờ đó, cùng một bộ quy tắc gõ hoạt động nhất quán tuyệt đối trên Linux, Windows, macOS và công cụ CLI.
2. **Độ trễ thấp & Không nuốt chữ**:
   - Đáp ứng tức thì từng phím bấm (tránh giật lag khi gõ nhanh hoặc nhấn giữ `Backspace`).
   - Áp dụng cơ chế **Fail-Safe** (`AdapterController`): luôn chốt phần chữ đang gõ (`CommitText`) trước khi hủy/reset nếu xảy ra ngắt quãng bất ngờ.
3. **An toàn với văn bản xung quanh & Dữ liệu nhạy cảm**:
   - Tự động tắt xử lý tiếng Việt ở ô mật khẩu (`FieldKind::Password`) và chuyển sang chế độ an toàn ở ô URL/Email/Số (`FieldKind::Url`, `Email`, `Number`).
   - Xác minh chuỗi xung quanh con trỏ (`surrounding_guard`) trước khi thực hiện xóa lùi ký tự đã chốt.
4. **Giao diện cài đặt nhẹ, thuần hệ điều hành**:
   - Dùng trực tiếp API giao diện gốc của từng nền tảng (GTK3 trên Linux, Win32 API trên Windows, SwiftUI trên macOS) thay vì kéo theo runtime nặng nề.

---

## 2. Phạm vi hỗ trợ hiện tại

| Hạng mục | Hỗ trợ trong VNType |
| :--- | :--- |
| **Kiểu gõ** | `Telex`, `Simple Telex`, `VNI`, `VIQR` |
| **Bảng mã** | 16+ bảng mã & định dạng (`Unicode NFC/NFD`, `TCVN3`, `VNI-Windows`, `VIQR`, `CP1258`, `VISCII`, `VPS`, `BKHCM1/2`, `Vietware`, chuỗi escape...) |
| **Tính năng gõ** | Quy tắc dấu mới/cũ, kiểm tra chính tả, gõ tự do (`free_typing`), khôi phục phím thô (`auto_restore_keys`), sửa từ trước (`edit_previous_word`), tự viết hoa, gõ tắt (`macros.txt`), bật/tắt theo app (`AppProfile`) |
| **Linux** | IBus (`vntype-ibus` thuần Rust qua D-Bus) & Fcitx5 (`libvntype.so` qua `vntype-ffi`), GTK3 Settings (`vntype-linux-settings`) |
| **Windows** | `WH_KEYBOARD_LL` Hook + System Tray (`vntype-hook`), TSF COM DLL (`vntype_tsf.dll`), Win32 Settings (`vntype-settings`) |
| **macOS** | InputMethodKit (`VNTypeIM` bằng Swift qua `vntype-ffi`), SwiftUI Preferences |
| **Công cụ phụ trợ** | `vntype-cli`, C ABI (`vntype-ffi` / `vntype.h`) |

---

## 3. Tài liệu liên quan

- [Kiến trúc hệ thống](ARCHITECTURE.md) — Phân tầng crate, vòng đời phím bấm và các mô hình xuất chữ.
- [Lõi xử lý tiếng Việt](CORE_ENGINE.md) — Âm vị học, `Composer`, quy tắc gõ và bảng mã.
- [Adapter & C ABI (FFI)](ADAPTER_AND_FFI.md) — `AdapterController`, `surrounding_guard` và hợp đồng `vntype.h`.
- [Tài liệu nền tảng](platforms/linux.md) — Chi tiết triển khai cho Linux, [Windows](platforms/windows.md) và [macOS](platforms/macos.md).
- [Phát triển](DEVELOPMENT.md) | [Kiểm thử & Gỡ lỗi](TESTING_AND_DEBUGGING.md) | [Đóng gói & Phát hành](RELEASE.md) | [Khắc phục sự cố](TROUBLESHOOTING.md)
