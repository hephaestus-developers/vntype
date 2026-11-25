# ADR-0002: Mô hình Strategy hướng dữ liệu (`InputMethodSpec`) cho các kiểu gõ

- **Trạng thái**: Đã chấp nhận (Accepted)
- **Phạm vi**: `crates/vntype-core/src/rules.rs`, `crates/vntype-core/src/composer.rs`

## Bối cảnh

VNType hỗ trợ nhiều kiểu gõ phổ biến (`Telex`, `Simple Telex`, `VNI`, `VIQR`). Câu hỏi đặt ra là nên tách mỗi kiểu gõ thành một module/file cài đặt thuật toán riêng (ví dụ `telex.rs`, `vni.rs`) hay dùng một bộ máy thống nhất đọc bảng luật.

## Quyết định

Áp dụng mẫu thiết kế **Strategy hướng dữ liệu**:

- Định nghĩa struct `InputMethodSpec` trong `crates/vntype-core/src/rules.rs` mô tả đầy đủ bảng phím thanh (`tones`), phím xóa dấu (`remove_tone`), danh sách quy tắc dấu phụ ưu tiên (`marks`), phím đứng riêng (`standalone`), phím ngoặc chèn trực tiếp (`insert_keys`) và cờ `digit_keys`.
- Khai báo `TELEX`, `SIMPLE_TELEX`, `VNI`, `VIQR` dưới dạng các hằng `pub static InputMethodSpec`.
- Một `Composer` duy nhất trong `crates/vntype-core/src/composer.rs` thực thi biến đổi, kiểm tra chính tả, hoàn nguyên khi lặp phím dựa trên `&'static InputMethodSpec`.

## Hệ quả

- **Tích cực**:
  - Tránh trùng lặp thuật toán phức tạp (đặt dấu, hoàn nguyên, tách cụm nguyên âm) giữa 4 kiểu gõ.
  - Thêm biến thể kiểu gõ mới rất gọn (~10–15 dòng khai báo bảng luật) và tự động hưởng toàn bộ test/fix từ `Composer`.
- **Đánh đổi**:
  - Nếu sau này có kiểu gõ có thuật toán hoàn toàn khác (ví dụ gõ tốc ký theo cụm phím), cần mở rộng `Composer` hoặc chuyển sang trait chuyên biệt.
