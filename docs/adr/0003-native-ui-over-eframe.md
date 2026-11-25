# ADR-0003: Dùng giao diện Native (Win32 / GTK3 / SwiftUI) thay vì `eframe`

- **Trạng thái**: Đã chấp nhận (Accepted)
- **Phạm vi**: `crates/vntype-settings-core`, `platforms/linux/settings`, `platforms/windows/settings`, `platforms/macos`

## Bối cảnh

Bộ gõ là tiện ích chạy nền và hộp thoại cài đặt chỉ mở khi người dùng cấu hình nhanh. Việc sử dụng framework GUI vẽ GPU đa nền tảng như `eframe`/`egui` kéo theo nhiều dependency đồ họa (OpenGL/wgpu), tăng dung lượng file nhị phân, tăng mức tiêu thụ RAM và đôi khi không khớp giao diện gốc của hệ điều hành.

## Quyết định

- Gom toàn bộ logic nghiệp vụ màn hình cài đặt (validation, capabilities theo OS, đọc/ghi cấu hình, kiểm tra gõ thử) vào crate dùng chung `vntype-settings-core` (`SettingsViewModel`, `PlatformCapabilities`).
- Viết lớp giao diện mỏng bằng API gốc trên từng hệ điều hành:
  - **Windows**: Win32 API thuần (`ComCtl32 v6`, `PerMonitorV2` DPI).
  - **Linux**: GTK3 thuần (`gtk` crate).
  - **macOS**: SwiftUI tích hợp thẳng vào `VNTypeIM`.

## Hệ quả

- **Tích cực**:
  - File nhị phân nhỏ gọn, khởi động tức thì, không phụ thuộc driver OpenGL/Vulkan trên máy người dùng.
  - Giao diện tự nhiên, chuẩn mực trên từng hệ điều hành.
- **Đánh đổi**:
  - Cần bảo trì 3 lớp UI mỏng riêng biệt, nhưng nhờ `vntype-settings-core` nên logic cấu hình vẫn thống nhất ở một nơi.
