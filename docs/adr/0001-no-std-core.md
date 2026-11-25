# ADR-0001: Tách lõi `vntype-syllable` và `vntype-core` theo chuẩn `no_std`

- **Trạng thái**: Đã chấp nhận (Accepted)
- **Phạm vi**: `crates/vntype-syllable`, `crates/vntype-core`

## Bối cảnh

VNType cần chạy trên nhiều môi trường khác nhau: daemon Linux (IBus/Fcitx5), tiến trình nền và COM DLL trên Windows (`vntype_tsf.dll`), bundle macOS (InputMethodKit), cũng như có khả năng nhúng vào các môi trường hạn chế (embedded/WASM/kernel-adjacent hoặc thư viện C nhỏ gọn).

## Quyết định

Giữ `#![cfg_attr(not(feature = "std"), no_std)]` cùng `extern crate alloc;` cho hai crate nền tảng:

- `vntype-syllable`: âm vị học, thanh điệu, kiểm tra chính tả, chuyển hoa/thường.
- `vntype-core`: `Composer`, `InputMethodSpec`, `rebuild`, `diff`, và bảng mã (`charset` có thể bật/tắt qua feature).

Toàn bộ I/O tệp (`std::fs`), đường dẫn OS và quản lý cấu hình được đẩy lên `vntype-config` và `vntype-engine`.

## Hệ quả

- **Tích cực**:
  - Logic xử lý tiếng Việt hoàn toàn thuần hàm, tất định (deterministic), dễ viết unit test và benchmark tốc độ nano-giây.
  - Đảm bảo kiểm tra biên dịch `no_std` tự động trong `just check`.
- **Đánh đổi**:
  - Phải tự quản lý cấu trúc dữ liệu không dùng `std::collections::HashMap` ở tầng lõi (dùng slice tĩnh và `alloc::vec::Vec` / `alloc::string::String`).
