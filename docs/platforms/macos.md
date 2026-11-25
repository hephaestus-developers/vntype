# Kiến trúc nền tảng macOS (`VNTypeIM`)

VNType trên macOS nằm tại `platforms/macos/`, được xây dựng bằng **Swift** (SwiftPM trong `Package.swift`) kết hợp với thư viện tĩnh/động Rust **`vntype-ffi`** thông qua `Bridging-Header.h`.

---

## 1. Thành phần mã nguồn (`platforms/macos/Sources/`)

- **`main.swift`**: Khởi tạo `NSApplication` và đăng ký `IMKServer` theo `Info.plist` / `Entitlements.plist`.
- **`InputController.swift`**: Kế thừa `IMKInputController`, nhận sự kiện phím từ macOS, chuyển qua `KeyMapper.swift` rồi gọi `VntBridge.swift` (`VntController` của `vntype-ffi` ở chế độ `VNT_MODE_PREEDIT`).
- **`ImkTextClient.swift`**: Thực thi các `VntEffect` trả về lên `IMKTextInput` (`setMarkedText`, `insertText`, xử lý thay thế surrounding text qua UTF-16).
- **`ConfigWatcher.swift`**: Theo dõi thay đổi tệp `config.json` / `macros.txt` để nạp lại cấu hình realtime.
- **`PreferencesView.swift`**: Giao diện cài đặt SwiftUI thuần gắn với `VNTypeIM`.

---

## 2. Quyền hệ thống & Kiểm thử

- macOS yêu cầu ứng dụng Input Method nằm trong `/Library/Input Methods/` hoặc `~/Library/Input Methods/` và được cấp quyền trong **System Settings → Keyboard → Input Sources**.
- Các test adapter trên macOS nằm tại `platforms/macos/Tests/MacAdapterTests.swift`.
- Đóng gói `.app` / `.dmg` hoặc cài đặt bằng script `packaging/macos/package-macos.sh`.
